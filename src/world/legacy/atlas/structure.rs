use std::f32::consts::TAU;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use crate::world::WorldMeta;

use super::AtlasCoord;
use super::scale::AtlasArea;
use super::seed::{domain_warp, fbm, hash01, lattice_hash, ridged_fbm};
use super::tuning::AtlasTuning;

pub const ATLAS_STRUCTURE_REGION_EDGE_CELLS: u32 = 8;
pub const ATLAS_STRUCTURE_REGION_PADDING_CELLS: u32 = 2;

const CHAIN_SPAWN_SIGNAL_SALT: u64 = 0xB711_A5C1_1000_0001;
const CHAIN_ID_SALT: u64 = 0xB711_A5C1_1000_0002;
const CHAIN_ANCHOR_X_SALT: u64 = 0xB711_A5C1_1000_0003;
const CHAIN_ANCHOR_Z_SALT: u64 = 0xB711_A5C1_1000_0004;
const CHAIN_HEADING_PRIMARY_SALT: u64 = 0xB711_A5C1_1000_0005;
const CHAIN_HEADING_SECONDARY_SALT: u64 = 0xB711_A5C1_1000_0006;
const CHAIN_STEP_JITTER_SALT: u64 = 0xB711_A5C1_1000_0007;
const CHAIN_STRENGTH_SALT: u64 = 0xB711_A5C1_1000_0008;
const CHAIN_WIDTH_SALT: u64 = 0xB711_A5C1_1000_0009;
const RIVER_ID_SALT: u64 = 0xB711_A5C1_1000_0010;
const RIVER_OUTLET_PRIMARY_SALT: u64 = 0xB711_A5C1_1000_0011;
const RIVER_OUTLET_SECONDARY_SALT: u64 = 0xB711_A5C1_1000_0012;
const RIVER_STEP_JITTER_SALT: u64 = 0xB711_A5C1_1000_0013;
const RIVER_BANKFULL_WIDTH_SALT: u64 = 0xB711_A5C1_1000_0014;
const RIVER_SIDE_SELECTION_SALT: u64 = 0xB711_A5C1_1000_0015;

const MAJOR_CHAIN_REGION_SLOTS: u32 = 2;
const MINOR_CHAIN_REGION_SLOTS: u32 = 2;
const MAJOR_CHAIN_STEP_LENGTH_CELLS: f32 = 2.4;
const MINOR_CHAIN_STEP_LENGTH_CELLS: f32 = 1.8;
const MAJOR_CHAIN_BRANCH_STEPS: u32 = 12;
const MINOR_CHAIN_BRANCH_STEPS: u32 = 7;
const MAJOR_CHAIN_MAX_REACH_CELLS: i32 = 36;
const MINOR_CHAIN_MAX_REACH_CELLS: i32 = 16;
const MAJOR_RIVER_BRANCH_STEPS: u32 = 20;
const MINOR_RIVER_BRANCH_STEPS: u32 = 12;
const MAJOR_RIVER_STEP_LENGTH_CELLS: f32 = 1.9;
const MINOR_RIVER_STEP_LENGTH_CELLS: f32 = 1.5;
const MAJOR_RIVER_MAX_REACH_CELLS: i32 = 44;
const MINOR_RIVER_MAX_REACH_CELLS: i32 = 26;
const TRIBUTARY_CONFLUENCE_SNAP_RADIUS_CELLS: f32 = 1.15;
const OWNERSHIP_PARENT_SNAP_RADIUS_CELLS: f32 = 0.85;
const SEGMENT_INTERSECTION_EPSILON: f32 = 0.0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasStructureRegionCoord {
    pub x: i32,
    pub z: i32,
}

impl AtlasStructureRegionCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasStructureRegion {
    coord: AtlasStructureRegionCoord,
}

impl AtlasStructureRegion {
    pub fn new(coord: AtlasStructureRegionCoord) -> Self {
        Self { coord }
    }

    pub fn coord(self) -> AtlasStructureRegionCoord {
        self.coord
    }

    pub fn core_area(self) -> AtlasArea {
        let origin = AtlasCoord::new(
            self.coord.x * ATLAS_STRUCTURE_REGION_EDGE_CELLS as i32,
            self.coord.z * ATLAS_STRUCTURE_REGION_EDGE_CELLS as i32,
        );
        AtlasArea::new(
            origin,
            ATLAS_STRUCTURE_REGION_EDGE_CELLS,
            ATLAS_STRUCTURE_REGION_EDGE_CELLS,
        )
        .expect("structure region core area must have a non-zero extent")
    }

    pub fn padded_area(self) -> AtlasArea {
        expand_area(
            self.core_area(),
            ATLAS_STRUCTURE_REGION_PADDING_CELLS as i32,
        )
    }
}

pub fn atlas_structure_region_coord_for_atlas(coord: AtlasCoord) -> AtlasStructureRegionCoord {
    AtlasStructureRegionCoord::new(
        div_floor_i32(coord.x, ATLAS_STRUCTURE_REGION_EDGE_CELLS as i32),
        div_floor_i32(coord.z, ATLAS_STRUCTURE_REGION_EDGE_CELLS as i32),
    )
}

pub fn atlas_structure_regions_covering_area(area: AtlasArea) -> Vec<AtlasStructureRegionCoord> {
    let origin = area.origin();
    let max_coord = AtlasCoord::new(
        origin.x + area.width() as i32 - 1,
        origin.z + area.height() as i32 - 1,
    );
    let min_region = atlas_structure_region_coord_for_atlas(origin);
    let max_region = atlas_structure_region_coord_for_atlas(max_coord);
    let mut regions = Vec::with_capacity(
        ((max_region.x - min_region.x + 1) * (max_region.z - min_region.z + 1)).max(0) as usize,
    );

    for region_z in min_region.z..=max_region.z {
        for region_x in min_region.x..=max_region.x {
            regions.push(AtlasStructureRegionCoord::new(region_x, region_z));
        }
    }

    regions
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountainChainId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountainChainScale {
    Major,
    Minor,
}

#[derive(Debug, Clone, Copy)]
struct SpawnedChain {
    chain_id: MountainChainId,
    owner_region: AtlasStructureRegionCoord,
    scale: MountainChainScale,
    anchor: AtlasCoord,
    ridge_heading: (f32, f32),
    strength: f32,
    half_width_cells: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountainSpineSegment {
    pub chain_id: MountainChainId,
    pub owner_region: AtlasStructureRegionCoord,
    pub branch_order: u8,
    pub scale: MountainChainScale,
    pub start: AtlasCoord,
    pub end: AtlasCoord,
    pub strength: f32,
    pub half_width_cells: f32,
}

impl MountainSpineSegment {
    pub fn touches_area(self, area: AtlasArea) -> bool {
        let min_x = self.start.x.min(self.end.x);
        let max_x = self.start.x.max(self.end.x);
        let min_z = self.start.z.min(self.end.z);
        let max_z = self.start.z.max(self.end.z);
        let area_origin = area.origin();
        let area_max_x = area_origin.x + area.width() as i32 - 1;
        let area_max_z = area_origin.z + area.height() as i32 - 1;

        max_x >= area_origin.x
            && min_x <= area_max_x
            && max_z >= area_origin.z
            && min_z <= area_max_z
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MountainChainGraph {
    segments: Vec<MountainSpineSegment>,
}

impl MountainChainGraph {
    pub fn segments(&self) -> &[MountainSpineSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn push_segment(&mut self, segment: MountainSpineSegment) {
        self.segments.push(segment);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RiverPathId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrainageBasinId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainageNodeKind {
    Headwater,
    Confluence,
    Outlet,
    Sink,
    PassOutlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainageNode {
    pub coord: AtlasCoord,
    pub kind: DrainageNodeKind,
    pub river_id: RiverPathId,
    pub order: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiverPathKind {
    Trunk,
    Tributary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverPathSegment {
    pub river_id: RiverPathId,
    pub basin_id: DrainageBasinId,
    pub main_stem_river_id: RiverPathId,
    pub parent_river_id: Option<RiverPathId>,
    pub owner_region: AtlasStructureRegionCoord,
    pub kind: RiverPathKind,
    pub order: u8,
    pub start: AtlasCoord,
    pub end: AtlasCoord,
    pub bankfull_width_cells: f32,
    pub downstream_cells_start: f32,
    pub downstream_cells_end: f32,
}

impl RiverPathSegment {
    pub fn touches_area(self, area: AtlasArea) -> bool {
        let min_x = self.start.x.min(self.end.x);
        let max_x = self.start.x.max(self.end.x);
        let min_z = self.start.z.min(self.end.z);
        let max_z = self.start.z.max(self.end.z);
        let area_origin = area.origin();
        let area_max_x = area_origin.x + area.width() as i32 - 1;
        let area_max_z = area_origin.z + area.height() as i32 - 1;

        max_x >= area_origin.x
            && min_x <= area_max_x
            && max_z >= area_origin.z
            && min_z <= area_max_z
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrainageGraph {
    nodes: Vec<DrainageNode>,
    segments: Vec<RiverPathSegment>,
}

impl DrainageGraph {
    pub fn nodes(&self) -> &[DrainageNode] {
        &self.nodes
    }

    pub fn segments(&self) -> &[RiverPathSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.segments.is_empty()
    }

    pub fn push_node(&mut self, node: DrainageNode) {
        self.nodes.push(node);
    }

    pub fn push_segment(&mut self, segment: RiverPathSegment) {
        self.segments.push(segment);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtlasStructureMap {
    area: AtlasArea,
    mountain_chains: MountainChainGraph,
    drainage: DrainageGraph,
}

impl AtlasStructureMap {
    pub fn empty(area: AtlasArea) -> Self {
        Self {
            area,
            mountain_chains: MountainChainGraph::default(),
            drainage: DrainageGraph::default(),
        }
    }

    pub fn area(&self) -> AtlasArea {
        self.area
    }

    pub fn mountain_chains(&self) -> &MountainChainGraph {
        &self.mountain_chains
    }

    pub fn mountain_chains_mut(&mut self) -> &mut MountainChainGraph {
        &mut self.mountain_chains
    }

    pub fn drainage(&self) -> &DrainageGraph {
        &self.drainage
    }

    pub fn drainage_mut(&mut self) -> &mut DrainageGraph {
        &mut self.drainage
    }

    pub fn is_empty(&self) -> bool {
        self.mountain_chains.is_empty() && self.drainage.is_empty()
    }
}

pub fn generate_atlas_structure(meta: &WorldMeta, area: AtlasArea) -> AtlasStructureMap {
    generate_atlas_structure_with_tuning(meta, area, &AtlasTuning::default())
}

pub fn generate_atlas_structure_with_tuning(
    meta: &WorldMeta,
    area: AtlasArea,
    _tuning: &AtlasTuning,
) -> AtlasStructureMap {
    let sample_area = expand_area(
        area,
        MAJOR_CHAIN_MAX_REACH_CELLS
            .max(MINOR_CHAIN_MAX_REACH_CELLS)
            .max(MAJOR_RIVER_MAX_REACH_CELLS)
            .max(MINOR_RIVER_MAX_REACH_CELLS),
    );
    let mut structure = AtlasStructureMap::empty(sample_area);

    for region_coord in atlas_structure_regions_covering_area(sample_area) {
        let region = AtlasStructureRegion::new(region_coord);
        generate_region_structure(meta.seed, region, sample_area, &mut structure);
    }
    resolve_drainage_confluences(sample_area, structure.drainage_mut());
    resolve_drainage_crossings(sample_area, structure.drainage_mut());
    assign_drainage_ownership(structure.drainage_mut());

    crop_structure_to_area(structure, area)
}

fn generate_region_structure(
    seed: u64,
    region: AtlasStructureRegion,
    requested_area: AtlasArea,
    structure: &mut AtlasStructureMap,
) {
    for slot in 0..MAJOR_CHAIN_REGION_SLOTS {
        if let Some(chain) = maybe_generate_chain(
            seed,
            region,
            slot,
            MountainChainScale::Major,
            requested_area,
            structure.mountain_chains_mut(),
        ) {
            emit_chain_drainage(
                seed ^ slot_seed(slot, MountainChainScale::Major),
                chain,
                requested_area,
                structure.drainage_mut(),
            );
        }
    }

    for slot in 0..MINOR_CHAIN_REGION_SLOTS {
        if let Some(chain) = maybe_generate_chain(
            seed,
            region,
            slot,
            MountainChainScale::Minor,
            requested_area,
            structure.mountain_chains_mut(),
        ) {
            emit_chain_drainage(
                seed ^ slot_seed(slot, MountainChainScale::Minor),
                chain,
                requested_area,
                structure.drainage_mut(),
            );
        }
    }
}

fn maybe_generate_chain(
    seed: u64,
    region: AtlasStructureRegion,
    slot: u32,
    scale: MountainChainScale,
    requested_area: AtlasArea,
    graph: &mut MountainChainGraph,
) -> Option<SpawnedChain> {
    let region_coord = region.coord();
    let slot_seed = slot_seed(slot, scale);
    let spawn_signal = chain_spawn_signal(seed ^ slot_seed, region, scale);
    let spawn_threshold = match scale {
        MountainChainScale::Major => 0.70,
        MountainChainScale::Minor => 0.60,
    };
    if spawn_signal < spawn_threshold {
        return None;
    }

    let anchor = chain_anchor(seed ^ slot_seed, region);
    let ridge_heading = sample_heading_vector(
        seed ^ slot_seed,
        anchor.x as f32 + 0.5,
        anchor.z as f32 + 0.5,
        scale,
    );
    let chain_id = MountainChainId(lattice_hash(
        seed ^ slot_seed,
        region_coord.x as i64,
        region_coord.z as i64,
        CHAIN_ID_SALT,
    ) as u32);
    let (branch_steps, base_step, base_width) = match scale {
        MountainChainScale::Major => (MAJOR_CHAIN_BRANCH_STEPS, MAJOR_CHAIN_STEP_LENGTH_CELLS, 2.6),
        MountainChainScale::Minor => (MINOR_CHAIN_BRANCH_STEPS, MINOR_CHAIN_STEP_LENGTH_CELLS, 1.6),
    };
    let base_strength = 0.72
        + hash01(
            seed ^ slot_seed,
            anchor.x as i64,
            anchor.z as i64,
            CHAIN_STRENGTH_SALT,
        ) * 0.28;

    emit_chain_branch(
        seed ^ slot_seed,
        region_coord,
        chain_id,
        scale,
        anchor,
        1.0,
        branch_steps,
        base_step,
        base_strength,
        base_width
            + hash01(
                seed ^ slot_seed,
                anchor.x as i64,
                anchor.z as i64,
                CHAIN_WIDTH_SALT,
            ) * 1.4,
        requested_area,
        graph,
    );
    emit_chain_branch(
        seed ^ slot_seed,
        region_coord,
        chain_id,
        scale,
        anchor,
        -1.0,
        branch_steps,
        base_step,
        base_strength,
        base_width
            + hash01(
                seed ^ slot_seed.wrapping_add(1),
                anchor.x as i64,
                anchor.z as i64,
                CHAIN_WIDTH_SALT,
            ) * 1.4,
        requested_area,
        graph,
    );

    Some(SpawnedChain {
        chain_id,
        owner_region: region_coord,
        scale,
        anchor,
        ridge_heading,
        strength: base_strength,
        half_width_cells: base_width,
    })
}

fn emit_chain_drainage(
    seed: u64,
    chain: SpawnedChain,
    requested_area: AtlasArea,
    graph: &mut DrainageGraph,
) {
    match chain.scale {
        MountainChainScale::Major => {
            let side = select_drainage_side_sign(
                seed ^ chain.chain_id.0 as u64,
                chain.anchor,
                RIVER_SIDE_SELECTION_SALT ^ 0x51,
            );
            emit_river_branch(
                seed,
                chain,
                RiverPathKind::Trunk,
                2,
                side,
                MAJOR_RIVER_BRANCH_STEPS,
                MAJOR_RIVER_STEP_LENGTH_CELLS,
                requested_area,
                graph,
            );
        }
        MountainChainScale::Minor => {
            let side = select_drainage_side_sign(seed, chain.anchor, RIVER_SIDE_SELECTION_SALT);
            emit_river_branch(
                seed,
                chain,
                RiverPathKind::Tributary,
                1,
                side,
                MINOR_RIVER_BRANCH_STEPS,
                MINOR_RIVER_STEP_LENGTH_CELLS,
                requested_area,
                graph,
            );
        }
    }
}

fn emit_river_branch(
    seed: u64,
    chain: SpawnedChain,
    kind: RiverPathKind,
    order: u8,
    side_sign: f32,
    branch_steps: u32,
    step_length: f32,
    requested_area: AtlasArea,
    graph: &mut DrainageGraph,
) {
    let perpendicular = (
        -chain.ridge_heading.1 * side_sign,
        chain.ridge_heading.0 * side_sign,
    );
    let headwater_offset = chain.half_width_cells.max(0.8) * (0.62 + chain.strength * 0.18) + 0.6;
    let mut position = (
        chain.anchor.x as f32 + 0.5 + perpendicular.0 * headwater_offset,
        chain.anchor.z as f32 + 0.5 + perpendicular.1 * headwater_offset,
    );
    let mut heading = sample_drainage_heading_vector(
        seed,
        position.0,
        position.1,
        chain.ridge_heading,
        side_sign,
        kind,
    );
    let river_id = RiverPathId(lattice_hash(
        seed,
        chain.owner_region.x as i64,
        chain.owner_region.z as i64,
        RIVER_ID_SALT
            ^ (chain.chain_id.0 as u64)
            ^ ((order as u64) << 12)
            ^ if side_sign.is_sign_negative() {
                1_u64
            } else {
                0_u64
            },
    ) as u32);
    let bankfull_width_cells = match kind {
        RiverPathKind::Trunk => {
            1.1 + chain.half_width_cells * 0.36
                + chain.strength * 0.45
                + hash01(
                    seed,
                    chain.anchor.x as i64,
                    chain.anchor.z as i64,
                    RIVER_BANKFULL_WIDTH_SALT,
                ) * 0.9
        }
        RiverPathKind::Tributary => {
            0.7 + chain.half_width_cells * 0.24
                + chain.strength * 0.20
                + hash01(
                    seed,
                    chain.anchor.x as i64,
                    chain.anchor.z as i64,
                    RIVER_BANKFULL_WIDTH_SALT,
                ) * 0.45
        }
    };
    let headwater = AtlasCoord::new(position.0.round() as i32, position.1.round() as i32);
    if coord_in_area(requested_area, headwater) {
        graph.push_node(DrainageNode {
            coord: headwater,
            kind: DrainageNodeKind::Headwater,
            river_id,
            order,
        });
    }
    let mut downstream_cells = 0.0_f32;

    for step_index in 0..branch_steps {
        let sampled_heading = sample_drainage_heading_vector(
            seed,
            position.0,
            position.1,
            chain.ridge_heading,
            side_sign,
            kind,
        );
        heading = normalize_vec2(
            heading.0 * 0.66 + sampled_heading.0 * 0.34,
            heading.1 * 0.66 + sampled_heading.1 * 0.34,
        );

        let jitter = (hash01(
            seed,
            position.0.round() as i64,
            position.1.round() as i64,
            RIVER_STEP_JITTER_SALT,
        ) - 0.5)
            * 0.45;
        let next_position = (
            position.0 + heading.0 * (step_length + jitter).max(0.9),
            position.1 + heading.1 * (step_length + jitter).max(0.9),
        );
        let segment_length_cells = ((next_position.0 - position.0).powi(2)
            + (next_position.1 - position.1).powi(2))
        .sqrt();
        let start = AtlasCoord::new(position.0.round() as i32, position.1.round() as i32);
        let end = AtlasCoord::new(
            next_position.0.round() as i32,
            next_position.1.round() as i32,
        );
        if start != end {
            let width = match kind {
                RiverPathKind::Trunk => {
                    bankfull_width_cells
                        * (1.0 + step_index as f32 / branch_steps.max(1) as f32 * 0.24)
                }
                RiverPathKind::Tributary => {
                    bankfull_width_cells
                        * (1.0 + step_index as f32 / branch_steps.max(1) as f32 * 0.12)
                }
            };
            let segment = RiverPathSegment {
                river_id,
                basin_id: DrainageBasinId(river_id.0),
                main_stem_river_id: river_id,
                parent_river_id: None,
                owner_region: chain.owner_region,
                kind,
                order,
                start,
                end,
                bankfull_width_cells: width,
                downstream_cells_start: downstream_cells,
                downstream_cells_end: downstream_cells + segment_length_cells,
            };
            if segment.touches_area(requested_area) {
                graph.push_segment(segment);
            }
        }

        downstream_cells += segment_length_cells;
        position = next_position;
    }

    let outlet = AtlasCoord::new(position.0.round() as i32, position.1.round() as i32);
    if coord_in_area(requested_area, outlet) {
        graph.push_node(DrainageNode {
            coord: outlet,
            kind: DrainageNodeKind::Outlet,
            river_id,
            order,
        });
    }
}

fn resolve_drainage_confluences(requested_area: AtlasArea, graph: &mut DrainageGraph) {
    let target_segments = graph.segments.clone();
    let mut tributary_last_segment_indices = Vec::<usize>::new();

    for (index, segment) in graph.segments.iter().enumerate() {
        if segment.kind != RiverPathKind::Tributary {
            continue;
        }

        if let Some(existing_index) =
            tributary_last_segment_indices
                .iter()
                .position(|candidate_index| {
                    graph.segments[*candidate_index].river_id == segment.river_id
                })
        {
            let candidate_index = tributary_last_segment_indices[existing_index];
            if segment.downstream_cells_end > graph.segments[candidate_index].downstream_cells_end {
                tributary_last_segment_indices[existing_index] = index;
            }
        } else {
            tributary_last_segment_indices.push(index);
        }
    }

    for source_index in tributary_last_segment_indices {
        let source = graph.segments[source_index];
        let outlet_point = (source.end.x as f32 + 0.5, source.end.z as f32 + 0.5);
        let mut best_target = None::<(AtlasCoord, RiverPathSegment, f32)>;

        for target in &target_segments {
            if target.river_id == source.river_id || target.order < source.order {
                continue;
            }

            let projection = project_point_onto_river_segment(outlet_point, *target);
            if projection.distance_cells > TRIBUTARY_CONFLUENCE_SNAP_RADIUS_CELLS {
                continue;
            }

            if best_target
                .as_ref()
                .is_none_or(|(_, _, best_distance)| projection.distance_cells < *best_distance)
            {
                best_target = Some((projection.coord, *target, projection.distance_cells));
            }
        }

        let Some((confluence_coord, target, _)) = best_target else {
            continue;
        };
        if !coord_in_area(requested_area, confluence_coord) {
            continue;
        }

        let snapped_length =
            distance_between_coords(graph.segments[source_index].start, confluence_coord);
        if snapped_length <= f32::EPSILON {
            continue;
        }

        graph.segments[source_index].end = confluence_coord;
        graph.segments[source_index].downstream_cells_end =
            graph.segments[source_index].downstream_cells_start + snapped_length;

        promote_river_outlet_to_confluence(
            requested_area,
            graph,
            source.river_id,
            target.river_id,
            source.order.max(target.order),
            confluence_coord,
        );
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
struct RiverSegmentIntersection {
    coord: AtlasCoord,
    point: (f32, f32),
    source_t: f32,
}

#[derive(Debug, Clone, Copy)]
struct DrainageCrossingResolution {
    source_river_id: RiverPathId,
    source_segment_index: usize,
    source_order_index: usize,
    source_order: u8,
    target_river_id: RiverPathId,
    target_order: u8,
    intersection_coord: AtlasCoord,
}

fn resolve_drainage_crossings(requested_area: AtlasArea, graph: &mut DrainageGraph) {
    let max_passes = graph.segments.len().saturating_mul(4).max(32);
    for _ in 0..max_passes {
        let Some(resolution) = find_next_drainage_crossing(requested_area, graph) else {
            return;
        };
        apply_drainage_crossing_resolution(requested_area, graph, resolution);
    }
}

fn assign_drainage_ownership(graph: &mut DrainageGraph) {
    let ordered_segments = ordered_river_segment_indices(graph);
    if ordered_segments.is_empty() {
        return;
    }

    let parent_rivers = infer_parent_river_ids(graph, &ordered_segments);
    let river_kinds = ordered_segments
        .iter()
        .filter_map(|(river_id, indices)| {
            indices
                .first()
                .map(|segment_index| (*river_id, graph.segments[*segment_index].kind))
        })
        .collect::<BTreeMap<_, _>>();
    let mut main_stem_rivers = BTreeMap::<u32, u32>::new();

    for river_id in ordered_segments.keys().copied() {
        let mut visiting = BTreeSet::new();
        let main_stem = resolve_main_stem_river_id(
            river_id,
            &parent_rivers,
            &river_kinds,
            &mut main_stem_rivers,
            &mut visiting,
        );
        main_stem_rivers.insert(river_id, main_stem);
    }

    for segment in graph.segments.iter_mut() {
        let river_id = segment.river_id.0;
        let parent_river_id = parent_rivers
            .get(&river_id)
            .copied()
            .flatten()
            .map(RiverPathId);
        let main_stem_river_id = RiverPathId(*main_stem_rivers.get(&river_id).unwrap_or(&river_id));

        segment.parent_river_id = parent_river_id;
        segment.main_stem_river_id = main_stem_river_id;
        segment.basin_id = DrainageBasinId(main_stem_river_id.0);
    }
}

fn ordered_river_segment_indices(graph: &DrainageGraph) -> BTreeMap<u32, Vec<usize>> {
    let mut ordered_segments = BTreeMap::<u32, Vec<usize>>::new();

    for (index, segment) in graph.segments.iter().enumerate() {
        ordered_segments
            .entry(segment.river_id.0)
            .or_default()
            .push(index);
    }

    for indices in ordered_segments.values_mut() {
        indices.sort_by(|left, right| {
            graph.segments[*left]
                .downstream_cells_start
                .total_cmp(&graph.segments[*right].downstream_cells_start)
        });
    }

    ordered_segments
}

fn infer_parent_river_ids(
    graph: &DrainageGraph,
    ordered_segments: &BTreeMap<u32, Vec<usize>>,
) -> BTreeMap<u32, Option<u32>> {
    let mut parent_rivers = ordered_segments
        .keys()
        .copied()
        .map(|river_id| (river_id, None))
        .collect::<BTreeMap<_, _>>();

    for (river_id, indices) in ordered_segments {
        let Some(last_index) = indices.last().copied() else {
            continue;
        };
        let source = graph.segments[last_index];
        let outlet_point = (source.end.x as f32 + 0.5, source.end.z as f32 + 0.5);
        let mut best_target = None::<(RiverPathSegment, f32)>;

        for target in &graph.segments {
            if target.river_id == source.river_id {
                continue;
            }
            if !should_source_branch_truncate_against_target(source, *target) {
                continue;
            }

            let projection = project_point_onto_river_segment(outlet_point, *target);
            if projection.distance_cells > OWNERSHIP_PARENT_SNAP_RADIUS_CELLS {
                continue;
            }

            let should_replace =
                best_target
                    .as_ref()
                    .is_none_or(|(best_segment, best_distance)| {
                        projection.distance_cells.total_cmp(best_distance) == Ordering::Less
                            || (projection.distance_cells.total_cmp(best_distance)
                                == Ordering::Equal
                                && compare_river_branch_priority(*target, *best_segment)
                                    == Ordering::Greater)
                            || (projection.distance_cells.total_cmp(best_distance)
                                == Ordering::Equal
                                && compare_river_branch_priority(*target, *best_segment)
                                    == Ordering::Equal
                                && target.river_id.0 < best_segment.river_id.0)
                    });
            if should_replace {
                best_target = Some((*target, projection.distance_cells));
            }
        }

        parent_rivers.insert(*river_id, best_target.map(|(target, _)| target.river_id.0));
    }

    parent_rivers
}

fn resolve_main_stem_river_id(
    river_id: u32,
    parent_rivers: &BTreeMap<u32, Option<u32>>,
    river_kinds: &BTreeMap<u32, RiverPathKind>,
    memoized_main_stems: &mut BTreeMap<u32, u32>,
    visiting: &mut BTreeSet<u32>,
) -> u32 {
    if let Some(main_stem) = memoized_main_stems.get(&river_id).copied() {
        return main_stem;
    }
    if river_kinds.get(&river_id).copied() == Some(RiverPathKind::Trunk) {
        memoized_main_stems.insert(river_id, river_id);
        return river_id;
    }
    if !visiting.insert(river_id) {
        return river_id;
    }

    let main_stem = parent_rivers
        .get(&river_id)
        .copied()
        .flatten()
        .map(|parent_river_id| {
            resolve_main_stem_river_id(
                parent_river_id,
                parent_rivers,
                river_kinds,
                memoized_main_stems,
                visiting,
            )
        })
        .unwrap_or(river_id);

    visiting.remove(&river_id);
    memoized_main_stems.insert(river_id, main_stem);
    main_stem
}

fn crop_structure_to_area(structure: AtlasStructureMap, area: AtlasArea) -> AtlasStructureMap {
    let AtlasStructureMap {
        area: _,
        mountain_chains,
        drainage,
    } = structure;

    AtlasStructureMap {
        area,
        mountain_chains: MountainChainGraph {
            segments: mountain_chains
                .segments
                .into_iter()
                .filter(|segment| segment.touches_area(area))
                .collect(),
        },
        drainage: DrainageGraph {
            nodes: drainage
                .nodes
                .into_iter()
                .filter(|node| coord_in_area(area, node.coord))
                .collect(),
            segments: drainage
                .segments
                .into_iter()
                .filter(|segment| segment.touches_area(area))
                .collect(),
        },
    }
}

fn find_next_drainage_crossing(
    requested_area: AtlasArea,
    graph: &DrainageGraph,
) -> Option<DrainageCrossingResolution> {
    let mut segments_by_river = BTreeMap::<u32, Vec<usize>>::new();
    for (index, segment) in graph.segments.iter().enumerate() {
        segments_by_river
            .entry(segment.river_id.0)
            .or_default()
            .push(index);
    }

    for indices in segments_by_river.values_mut() {
        indices.sort_by(|left, right| {
            graph.segments[*left]
                .downstream_cells_start
                .total_cmp(&graph.segments[*right].downstream_cells_start)
        });
    }

    for indices in segments_by_river.values() {
        for (source_order_index, source_index) in indices.iter().copied().enumerate() {
            let source = graph.segments[source_index];
            let source_has_later_segments = source_order_index + 1 < indices.len();
            let mut best_target = None::<(RiverSegmentIntersection, RiverPathSegment)>;

            for target in &graph.segments {
                if target.river_id == source.river_id {
                    continue;
                }
                if !should_source_branch_truncate_against_target(source, *target) {
                    continue;
                }

                let Some(intersection) = river_segments_intersection(source, *target) else {
                    continue;
                };
                if !coord_in_area(requested_area, intersection.coord) {
                    continue;
                }
                if !intersection_requires_resolution(
                    source,
                    *target,
                    intersection.coord,
                    source_has_later_segments,
                ) {
                    continue;
                }

                if best_target.as_ref().is_none_or(|(best, _)| {
                    intersection.source_t.total_cmp(&best.source_t) == Ordering::Less
                }) {
                    best_target = Some((intersection, *target));
                }
            }

            if let Some((intersection, target)) = best_target {
                return Some(DrainageCrossingResolution {
                    source_river_id: source.river_id,
                    source_segment_index: source_index,
                    source_order_index,
                    source_order: source.order,
                    target_river_id: target.river_id,
                    target_order: target.order,
                    intersection_coord: intersection.coord,
                });
            }
        }
    }

    None
}

fn apply_drainage_crossing_resolution(
    requested_area: AtlasArea,
    graph: &mut DrainageGraph,
    resolution: DrainageCrossingResolution,
) {
    let mut source_indices = graph
        .segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            (segment.river_id == resolution.source_river_id).then_some(index)
        })
        .collect::<Vec<_>>();
    source_indices.sort_by(|left, right| {
        graph.segments[*left]
            .downstream_cells_start
            .total_cmp(&graph.segments[*right].downstream_cells_start)
    });

    let Some(&source_index) = source_indices.get(resolution.source_order_index) else {
        return;
    };
    if source_index != resolution.source_segment_index {
        return;
    }

    let source = graph.segments[source_index];
    let snapped_length = distance_between_coords(source.start, resolution.intersection_coord);
    let remove_source_segment =
        snapped_length <= f32::EPSILON || resolution.intersection_coord == source.start;

    let mut remove_indices = source_indices
        .into_iter()
        .skip(resolution.source_order_index + 1)
        .collect::<Vec<_>>();
    if remove_source_segment {
        remove_indices.push(source_index);
    } else {
        graph.segments[source_index].end = resolution.intersection_coord;
        graph.segments[source_index].downstream_cells_end =
            graph.segments[source_index].downstream_cells_start + snapped_length;
    }

    remove_indices.sort_unstable();
    remove_indices.dedup();
    if !remove_indices.is_empty() {
        graph.segments = graph
            .segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| (!remove_indices.contains(&index)).then_some(*segment))
            .collect();
    }

    promote_river_outlet_to_confluence(
        requested_area,
        graph,
        resolution.source_river_id,
        resolution.target_river_id,
        resolution.source_order.max(resolution.target_order),
        resolution.intersection_coord,
    );
}

fn promote_river_outlet_to_confluence(
    requested_area: AtlasArea,
    graph: &mut DrainageGraph,
    source_river_id: RiverPathId,
    target_river_id: RiverPathId,
    merged_order: u8,
    confluence_coord: AtlasCoord,
) {
    if !coord_in_area(requested_area, confluence_coord) {
        return;
    }

    if let Some(node) = graph
        .nodes
        .iter_mut()
        .find(|node| node.kind == DrainageNodeKind::Outlet && node.river_id == source_river_id)
    {
        node.coord = confluence_coord;
        node.kind = DrainageNodeKind::Confluence;
        node.order = node.order.max(merged_order);
        node.river_id = target_river_id;
    } else {
        graph.push_node(DrainageNode {
            coord: confluence_coord,
            kind: DrainageNodeKind::Confluence,
            river_id: target_river_id,
            order: merged_order,
        });
    }
}

fn should_source_branch_truncate_against_target(
    source: RiverPathSegment,
    target: RiverPathSegment,
) -> bool {
    match (source.kind, target.kind) {
        (RiverPathKind::Trunk, RiverPathKind::Tributary) => false,
        (RiverPathKind::Tributary, RiverPathKind::Trunk) => true,
        _ => match compare_river_branch_priority(source, target) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => source.river_id.0 > target.river_id.0,
        },
    }
}

fn compare_river_branch_priority(left: RiverPathSegment, right: RiverPathSegment) -> Ordering {
    river_branch_priority(left).cmp(&river_branch_priority(right))
}

fn river_branch_priority(segment: RiverPathSegment) -> (u8, u8, u32) {
    let kind_rank = match segment.kind {
        RiverPathKind::Trunk => 2,
        RiverPathKind::Tributary => 1,
    };
    let width_rank = (segment.bankfull_width_cells.max(0.0) * 1024.0).round() as u32;
    (kind_rank, segment.order, width_rank)
}

#[allow(clippy::too_many_arguments)]
fn emit_chain_branch(
    seed: u64,
    owner_region: AtlasStructureRegionCoord,
    chain_id: MountainChainId,
    scale: MountainChainScale,
    anchor: AtlasCoord,
    direction_sign: f32,
    branch_steps: u32,
    base_step_length: f32,
    base_strength: f32,
    base_width: f32,
    requested_area: AtlasArea,
    graph: &mut MountainChainGraph,
) {
    let mut position = (anchor.x as f32 + 0.5, anchor.z as f32 + 0.5);
    let mut heading = sample_heading_vector(seed, position.0, position.1, scale);
    if direction_sign < 0.0 {
        heading = (-heading.0, -heading.1);
    }

    for step_index in 0..branch_steps {
        let sampled_heading = sample_heading_vector(seed, position.0, position.1, scale);
        let sampled_heading = if direction_sign < 0.0 {
            (-sampled_heading.0, -sampled_heading.1)
        } else {
            sampled_heading
        };
        heading = normalize_vec2(
            heading.0 * 0.72 + sampled_heading.0 * 0.28,
            heading.1 * 0.72 + sampled_heading.1 * 0.28,
        );

        let jitter = (hash01(
            seed,
            position.0.round() as i64,
            position.1.round() as i64,
            CHAIN_STEP_JITTER_SALT,
        ) - 0.5)
            * 0.7;
        let step_length = (base_step_length + jitter).max(1.1);
        let next_position = (
            position.0 + heading.0 * step_length,
            position.1 + heading.1 * step_length,
        );
        let start = AtlasCoord::new(position.0.round() as i32, position.1.round() as i32);
        let end = AtlasCoord::new(
            next_position.0.round() as i32,
            next_position.1.round() as i32,
        );
        if start != end {
            let progress = step_index as f32 / branch_steps.max(1) as f32;
            let segment = MountainSpineSegment {
                chain_id,
                owner_region,
                branch_order: 0,
                scale,
                start,
                end,
                strength: (base_strength * (1.0 - progress * 0.32)).max(0.25),
                half_width_cells: (base_width * (1.0 - progress * 0.20)).max(0.8),
            };
            if segment.touches_area(requested_area) {
                graph.push_segment(segment);
            }
        }

        position = next_position;
    }
}

fn chain_spawn_signal(seed: u64, region: AtlasStructureRegion, scale: MountainChainScale) -> f32 {
    let core = region.core_area();
    let center = AtlasCoord::new(
        core.origin().x + (core.width() as i32 / 2),
        core.origin().z + (core.height() as i32 / 2),
    );
    let (wx, wz) = domain_warp(seed, center.x as f64, center.z as f64, 1.0 / 28.0, 2.1);
    let ridge_signal = ridged_fbm(
        seed,
        wx / 26.0,
        wz / 26.0,
        4,
        2.0,
        0.5,
        CHAIN_SPAWN_SIGNAL_SALT,
    );
    let randomness = hash01(
        seed,
        region.coord().x as i64,
        region.coord().z as i64,
        CHAIN_SPAWN_SIGNAL_SALT.wrapping_add(match scale {
            MountainChainScale::Major => 0,
            MountainChainScale::Minor => 1,
        }),
    );
    match scale {
        MountainChainScale::Major => ridge_signal * 0.82 + randomness * 0.18,
        MountainChainScale::Minor => ridge_signal * 0.58 + randomness * 0.42,
    }
}

fn chain_anchor(seed: u64, region: AtlasStructureRegion) -> AtlasCoord {
    let core = region.core_area();
    let origin = core.origin();
    let local_x = (hash01(
        seed,
        region.coord().x as i64,
        region.coord().z as i64,
        CHAIN_ANCHOR_X_SALT,
    ) * core.width() as f32)
        .floor() as i32;
    let local_z = (hash01(
        seed,
        region.coord().x as i64,
        region.coord().z as i64,
        CHAIN_ANCHOR_Z_SALT,
    ) * core.height() as f32)
        .floor() as i32;

    AtlasCoord::new(
        origin.x + local_x.min(core.width() as i32 - 1),
        origin.z + local_z.min(core.height() as i32 - 1),
    )
}

fn sample_heading_vector(
    seed: u64,
    world_x: f32,
    world_z: f32,
    scale: MountainChainScale,
) -> (f32, f32) {
    let (wx, wz) = domain_warp(
        seed,
        world_x as f64,
        world_z as f64,
        match scale {
            MountainChainScale::Major => 1.0 / 34.0,
            MountainChainScale::Minor => 1.0 / 22.0,
        },
        match scale {
            MountainChainScale::Major => 2.6,
            MountainChainScale::Minor => 1.4,
        },
    );
    let primary = fbm(
        seed,
        wx / match scale {
            MountainChainScale::Major => 48.0,
            MountainChainScale::Minor => 28.0,
        },
        wz / match scale {
            MountainChainScale::Major => 48.0,
            MountainChainScale::Minor => 28.0,
        },
        4,
        2.0,
        0.5,
        CHAIN_HEADING_PRIMARY_SALT,
    );
    let secondary = fbm(
        seed,
        wx / match scale {
            MountainChainScale::Major => 96.0,
            MountainChainScale::Minor => 52.0,
        },
        wz / match scale {
            MountainChainScale::Major => 96.0,
            MountainChainScale::Minor => 52.0,
        },
        3,
        2.0,
        0.5,
        CHAIN_HEADING_SECONDARY_SALT,
    );
    let angle = primary * TAU
        + (secondary - 0.5)
            * match scale {
                MountainChainScale::Major => 0.8,
                MountainChainScale::Minor => 1.2,
            };

    normalize_vec2(angle.cos(), angle.sin())
}

fn sample_drainage_heading_vector(
    seed: u64,
    world_x: f32,
    world_z: f32,
    ridge_heading: (f32, f32),
    side_sign: f32,
    kind: RiverPathKind,
) -> (f32, f32) {
    let (wx, wz) = domain_warp(seed, world_x as f64, world_z as f64, 1.0 / 26.0, 2.0);
    let primary = fbm(
        seed,
        wx / match kind {
            RiverPathKind::Trunk => 72.0,
            RiverPathKind::Tributary => 44.0,
        },
        wz / match kind {
            RiverPathKind::Trunk => 72.0,
            RiverPathKind::Tributary => 44.0,
        },
        4,
        2.0,
        0.5,
        RIVER_OUTLET_PRIMARY_SALT,
    );
    let secondary = fbm(
        seed,
        wx / match kind {
            RiverPathKind::Trunk => 116.0,
            RiverPathKind::Tributary => 64.0,
        },
        wz / match kind {
            RiverPathKind::Trunk => 116.0,
            RiverPathKind::Tributary => 64.0,
        },
        3,
        2.0,
        0.5,
        RIVER_OUTLET_SECONDARY_SALT,
    );
    let outlet_angle = primary * TAU
        + (secondary - 0.5)
            * match kind {
                RiverPathKind::Trunk => 0.9,
                RiverPathKind::Tributary => 1.15,
            };
    let outlet_vector = normalize_vec2(outlet_angle.cos(), outlet_angle.sin());
    let off_ridge = (-ridge_heading.1 * side_sign, ridge_heading.0 * side_sign);

    match kind {
        RiverPathKind::Trunk => normalize_vec2(
            off_ridge.0 * 0.62 + outlet_vector.0 * 0.38,
            off_ridge.1 * 0.62 + outlet_vector.1 * 0.38,
        ),
        RiverPathKind::Tributary => normalize_vec2(
            off_ridge.0 * 0.74 + outlet_vector.0 * 0.26,
            off_ridge.1 * 0.74 + outlet_vector.1 * 0.26,
        ),
    }
}

fn select_drainage_side_sign(seed: u64, anchor: AtlasCoord, salt: u64) -> f32 {
    if hash01(seed, anchor.x as i64, anchor.z as i64, salt) >= 0.5 {
        1.0
    } else {
        -1.0
    }
}

fn slot_seed(slot: u32, scale: MountainChainScale) -> u64 {
    ((slot as u64) << 8)
        ^ match scale {
            MountainChainScale::Major => 0x11,
            MountainChainScale::Minor => 0x22,
        }
}

fn expand_area(area: AtlasArea, padding_cells: i32) -> AtlasArea {
    let origin = area.origin();
    let expanded_origin = AtlasCoord::new(origin.x - padding_cells, origin.z - padding_cells);
    let expanded_width = area.width() as i32 + padding_cells * 2;
    let expanded_height = area.height() as i32 + padding_cells * 2;
    AtlasArea::new(
        expanded_origin,
        expanded_width.max(1) as u32,
        expanded_height.max(1) as u32,
    )
    .expect("expanded atlas area must remain non-empty")
}

fn div_floor_i32(value: i32, divisor: i32) -> i32 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder != 0 && ((remainder > 0) != (divisor > 0)) {
        quotient - 1
    } else {
        quotient
    }
}

fn normalize_vec2(x: f32, z: f32) -> (f32, f32) {
    let length_sq = x * x + z * z;
    if length_sq <= f32::EPSILON {
        (1.0, 0.0)
    } else {
        let inv_length = length_sq.sqrt().recip();
        (x * inv_length, z * inv_length)
    }
}

fn coord_in_area(area: AtlasArea, coord: AtlasCoord) -> bool {
    area.contains(coord)
}

#[derive(Debug, Clone, Copy)]
struct RiverSegmentProjection {
    coord: AtlasCoord,
    distance_cells: f32,
}

fn project_point_onto_river_segment(
    point: (f32, f32),
    segment: RiverPathSegment,
) -> RiverSegmentProjection {
    let start_x = segment.start.x as f32 + 0.5;
    let start_z = segment.start.z as f32 + 0.5;
    let end_x = segment.end.x as f32 + 0.5;
    let end_z = segment.end.z as f32 + 0.5;
    let seg_x = end_x - start_x;
    let seg_z = end_z - start_z;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return RiverSegmentProjection {
            coord: segment.start,
            distance_cells: ((point.0 - start_x).powi(2) + (point.1 - start_z).powi(2)).sqrt(),
        };
    }

    let t =
        (((point.0 - start_x) * seg_x + (point.1 - start_z) * seg_z) / length_sq).clamp(0.0, 1.0);
    let projected_x = start_x + seg_x * t;
    let projected_z = start_z + seg_z * t;

    RiverSegmentProjection {
        coord: atlas_coord_from_center_point(projected_x, projected_z),
        distance_cells: ((point.0 - projected_x).powi(2) + (point.1 - projected_z).powi(2)).sqrt(),
    }
}

fn distance_between_coords(start: AtlasCoord, end: AtlasCoord) -> f32 {
    let dx = end.x as f32 - start.x as f32;
    let dz = end.z as f32 - start.z as f32;
    (dx * dx + dz * dz).sqrt()
}

fn river_segments_intersection(
    source: RiverPathSegment,
    target: RiverPathSegment,
) -> Option<RiverSegmentIntersection> {
    let source_start = (source.start.x as f32 + 0.5, source.start.z as f32 + 0.5);
    let source_end = (source.end.x as f32 + 0.5, source.end.z as f32 + 0.5);
    let target_start = (target.start.x as f32 + 0.5, target.start.z as f32 + 0.5);
    let target_end = (target.end.x as f32 + 0.5, target.end.z as f32 + 0.5);

    let source_dx = source_end.0 - source_start.0;
    let source_dz = source_end.1 - source_start.1;
    let target_dx = target_end.0 - target_start.0;
    let target_dz = target_end.1 - target_start.1;
    let denominator = cross2(source_dx, source_dz, target_dx, target_dz);
    if denominator.abs() <= SEGMENT_INTERSECTION_EPSILON {
        return None;
    }

    let offset_x = target_start.0 - source_start.0;
    let offset_z = target_start.1 - source_start.1;
    let source_t = cross2(offset_x, offset_z, target_dx, target_dz) / denominator;
    let target_t = cross2(offset_x, offset_z, source_dx, source_dz) / denominator;

    let valid_source =
        (-SEGMENT_INTERSECTION_EPSILON..=1.0 + SEGMENT_INTERSECTION_EPSILON).contains(&source_t);
    let valid_target =
        (-SEGMENT_INTERSECTION_EPSILON..=1.0 + SEGMENT_INTERSECTION_EPSILON).contains(&target_t);
    if !valid_source || !valid_target {
        return None;
    }

    let point = (
        source_start.0 + source_dx * source_t.clamp(0.0, 1.0),
        source_start.1 + source_dz * source_t.clamp(0.0, 1.0),
    );
    let coord = atlas_coord_from_center_point(point.0, point.1);

    Some(RiverSegmentIntersection {
        coord,
        point,
        source_t: source_t.clamp(0.0, 1.0),
    })
}

fn is_shared_endpoint_intersection(
    source: RiverPathSegment,
    target: RiverPathSegment,
    coord: AtlasCoord,
) -> bool {
    let source_endpoint = coord == source.start || coord == source.end;
    let target_endpoint = coord == target.start || coord == target.end;
    source_endpoint && target_endpoint
}

#[cfg_attr(not(test), allow(dead_code))]
fn is_branch_junction_intersection(
    source: RiverPathSegment,
    target: RiverPathSegment,
    coord: AtlasCoord,
) -> bool {
    let source_endpoint = coord == source.start || coord == source.end;
    let target_endpoint = coord == target.start || coord == target.end;
    source_endpoint || target_endpoint
}

fn intersection_requires_resolution(
    source: RiverPathSegment,
    target: RiverPathSegment,
    coord: AtlasCoord,
    source_has_later_segments: bool,
) -> bool {
    if coord == source.start || is_shared_endpoint_intersection(source, target, coord) {
        return false;
    }

    let source_endpoint = coord == source.end;
    let target_endpoint = coord == target.start || coord == target.end;
    if source_endpoint || target_endpoint {
        return source_endpoint && source_has_later_segments;
    }

    true
}

fn cross2(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    ax * bz - az * bx
}

fn atlas_coord_from_center_point(x: f32, z: f32) -> AtlasCoord {
    AtlasCoord::new((x - 0.5).round() as i32, (z - 0.5).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_coords_floor_negative_atlas_coordinates() {
        assert_eq!(
            atlas_structure_region_coord_for_atlas(AtlasCoord::new(-1, -1)),
            AtlasStructureRegionCoord::new(-1, -1)
        );
        assert_eq!(
            atlas_structure_region_coord_for_atlas(AtlasCoord::new(-8, 15)),
            AtlasStructureRegionCoord::new(-1, 1)
        );
    }

    #[test]
    fn region_covering_area_spans_negative_and_positive_regions() {
        let area = AtlasArea::new(AtlasCoord::new(-3, -2), 12, 10).unwrap();
        let regions = atlas_structure_regions_covering_area(area);

        assert!(regions.contains(&AtlasStructureRegionCoord::new(-1, -1)));
        assert!(regions.contains(&AtlasStructureRegionCoord::new(1, 0)));
    }

    #[test]
    #[ignore = "slow atlas-structure generation smoke test"]
    fn generated_structure_preserves_requested_area_and_is_deterministic() {
        let area = AtlasArea::new(AtlasCoord::new(-4, 7), 24, 20).unwrap();
        let first = generate_atlas_structure(&WorldMeta::new(42), area);
        let second = generate_atlas_structure(&WorldMeta::new(42), area);

        assert_eq!(first.area(), area);
        assert_eq!(first, second);
    }

    #[test]
    #[ignore = "slow atlas-structure generation smoke test"]
    fn generated_structure_emits_region_owned_mountain_segments_for_large_area() {
        let area = AtlasArea::new(AtlasCoord::new(-24, -24), 48, 48).unwrap();
        let structure = generate_atlas_structure(&WorldMeta::new(42), area);

        assert!(!structure.mountain_chains().is_empty());
        assert!(
            structure
                .mountain_chains()
                .segments()
                .iter()
                .all(|segment| segment.touches_area(area))
        );
    }

    #[test]
    #[ignore = "slow atlas-structure generation smoke test"]
    fn generated_structure_emits_drainage_segments_for_large_area() {
        let area = AtlasArea::new(AtlasCoord::new(-24, -24), 48, 48).unwrap();
        let structure = generate_atlas_structure(&WorldMeta::new(42), area);

        assert!(!structure.drainage().segments().is_empty());
        assert!(
            structure
                .drainage()
                .segments()
                .iter()
                .all(|segment| segment.touches_area(area))
        );
    }

    #[test]
    #[ignore = "slow atlas-structure generation smoke test"]
    fn generated_drainage_segments_keep_forward_downstream_progress() {
        let area = AtlasArea::new(AtlasCoord::new(-24, -24), 48, 48).unwrap();
        let structure = generate_atlas_structure(&WorldMeta::new(42), area);

        assert!(
            structure
                .drainage()
                .segments()
                .iter()
                .all(|segment| segment.downstream_cells_end > segment.downstream_cells_start)
        );
    }

    #[test]
    #[ignore = "slow atlas-structure crossing regression test"]
    fn generated_drainage_segments_only_intersect_at_branch_junctions() {
        let area = AtlasArea::new(AtlasCoord::new(-24, -24), 48, 48).unwrap();
        let structure = generate_atlas_structure(&WorldMeta::new(42), area);
        let segments = structure.drainage().segments();

        for left_index in 0..segments.len() {
            for right_index in (left_index + 1)..segments.len() {
                let left = segments[left_index];
                let right = segments[right_index];
                if left.river_id == right.river_id {
                    continue;
                }

                if let Some(intersection) = river_segments_intersection(left, right) {
                    assert!(
                        is_branch_junction_intersection(left, right, intersection.coord),
                        "drainage segments from distinct rivers crossed through each other's interiors: left={left:?} right={right:?} coord={:?} point={:?}",
                        intersection.coord,
                        intersection.point,
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "slow atlas-structure ownership smoke test"]
    fn generated_drainage_segments_carry_consistent_tree_ownership() {
        let area = AtlasArea::new(AtlasCoord::new(-24, -24), 48, 48).unwrap();
        let structure = generate_atlas_structure(&WorldMeta::new(42), area);
        let segments = structure.drainage().segments();
        let mut roots_by_basin = BTreeMap::<u32, BTreeSet<u32>>::new();

        assert!(!segments.is_empty());

        for segment in segments {
            assert_eq!(segment.basin_id.0, segment.main_stem_river_id.0);
            if let Some(parent_river_id) = segment.parent_river_id {
                assert_ne!(parent_river_id, segment.river_id);
            } else {
                roots_by_basin
                    .entry(segment.basin_id.0)
                    .or_default()
                    .insert(segment.river_id.0);
            }
        }

        assert!(roots_by_basin.values().all(|roots| roots.len() == 1));
    }

    #[test]
    fn mountain_segments_report_area_intersection_from_bounds() {
        let area = AtlasArea::new(AtlasCoord::new(10, 10), 4, 4).unwrap();
        let segment = MountainSpineSegment {
            chain_id: MountainChainId(1),
            owner_region: AtlasStructureRegionCoord::new(1, 1),
            branch_order: 0,
            scale: MountainChainScale::Major,
            start: AtlasCoord::new(8, 12),
            end: AtlasCoord::new(12, 12),
            strength: 1.0,
            half_width_cells: 1.5,
        };

        assert!(segment.touches_area(area));
        assert!(!segment.touches_area(AtlasArea::new(AtlasCoord::new(20, 20), 4, 4).unwrap()));
    }

    #[test]
    fn river_segments_report_area_intersection_from_bounds() {
        let area = AtlasArea::new(AtlasCoord::new(-2, -2), 3, 3).unwrap();
        let segment = RiverPathSegment {
            river_id: RiverPathId(7),
            basin_id: DrainageBasinId(7),
            main_stem_river_id: RiverPathId(7),
            parent_river_id: None,
            owner_region: AtlasStructureRegionCoord::new(-1, -1),
            kind: RiverPathKind::Trunk,
            order: 2,
            start: AtlasCoord::new(-4, -1),
            end: AtlasCoord::new(0, -1),
            bankfull_width_cells: 1.0,
            downstream_cells_start: 0.0,
            downstream_cells_end: 1.0,
        };

        assert!(segment.touches_area(area));
        assert!(!segment.touches_area(AtlasArea::new(AtlasCoord::new(4, 4), 2, 2).unwrap()));
    }
}
