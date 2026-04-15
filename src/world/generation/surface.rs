use super::context::ColumnAtlasSample;
use super::profile::{TerrainProfile, resolve_profile};
use super::profiles::surface_height_for_sample;
use super::sampler::sample_column_atlas;
use super::super::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasFieldMap, AtlasStructureMap, AtlasTuning,
    MountainChainScale, MountainSpineSegment, RiverPathKind, RiverPathSegment,
};
use super::super::coord::{CHUNK_EDGE_I32, ChunkCoord};
use super::super::meta::WorldMeta;

const ATLAS_CELL_SPAN_BLOCKS_F32: f32 = ATLAS_CELL_SIZE_IN_CHUNKS as f32 * CHUNK_EDGE_I32 as f32;
const SURFACE_SMOOTH_RADIUS: i32 = 2;
const SURFACE_CONCAVITY_RADIUS: i32 = 1;
const SURFACE_FIELD_PADDING: i32 = SURFACE_SMOOTH_RADIUS + SURFACE_CONCAVITY_RADIUS;
const LOCAL_CONCAVITY_NORMALIZER: f32 = 3.0;
const SURFACE_SMOOTH_KERNEL: [[f32; 5]; 5] = [
    [1.0, 2.0, 3.0, 2.0, 1.0],
    [2.0, 4.0, 6.0, 4.0, 2.0],
    [3.0, 6.0, 9.0, 6.0, 3.0],
    [2.0, 4.0, 6.0, 4.0, 2.0],
    [1.0, 2.0, 3.0, 2.0, 1.0],
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreparedSurfaceColumn {
    pub atlas_sample: ColumnAtlasSample,
    pub profile: TerrainProfile,
    pub raw_surface_y: f32,
    pub surface_y: f32,
    pub local_concavity: f32,
    pub structure: PreparedStructureGuide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreparedStructureGuide {
    pub ridge_distance_cells: f32,
    pub ridge_weight: f32,
    pub ridge_strength: f32,
    pub ridge_heading_x: f32,
    pub ridge_heading_z: f32,
    pub channel_distance_cells: f32,
    pub channel_weight: f32,
    pub channel_core: f32,
    pub channel_order: u8,
    pub channel_heading_x: f32,
    pub channel_heading_z: f32,
    pub channel_bankfull_hint: f32,
    pub along_channel_cells: f32,
}

impl Default for PreparedStructureGuide {
    fn default() -> Self {
        Self {
            ridge_distance_cells: f32::INFINITY,
            ridge_weight: 0.0,
            ridge_strength: 0.0,
            ridge_heading_x: 0.0,
            ridge_heading_z: 0.0,
            channel_distance_cells: f32::INFINITY,
            channel_weight: 0.0,
            channel_core: 0.0,
            channel_order: 0,
            channel_heading_x: 0.0,
            channel_heading_z: 0.0,
            channel_bankfull_hint: 0.0,
            along_channel_cells: 0.0,
        }
    }
}

impl Default for PreparedSurfaceColumn {
    fn default() -> Self {
        Self {
            atlas_sample: ColumnAtlasSample::default(),
            profile: TerrainProfile::Plain,
            raw_surface_y: 0.0,
            surface_y: 0.0,
            local_concavity: 0.0,
            structure: PreparedStructureGuide::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ChunkSurfaceField {
    span: i32,
    padding: i32,
    columns: Vec<PreparedSurfaceColumn>,
}

impl ChunkSurfaceField {
    pub(super) fn column(&self, local_x: u8, local_z: u8) -> PreparedSurfaceColumn {
        self.column_i32(i32::from(local_x), i32::from(local_z))
    }

    fn column_i32(&self, local_x: i32, local_z: i32) -> PreparedSurfaceColumn {
        let grid_x = local_x + self.padding;
        let grid_z = local_z + self.padding;
        self.columns[self.index(grid_x, grid_z)]
    }

    pub(super) fn central_columns(&self) -> impl Iterator<Item = PreparedSurfaceColumn> + '_ {
        let field = self;
        (0..CHUNK_EDGE_I32).flat_map(move |local_z| {
            (0..CHUNK_EDGE_I32).map(move |local_x| field.column_i32(local_x, local_z))
        })
    }

    fn index(&self, grid_x: i32, grid_z: i32) -> usize {
        debug_assert!(grid_x >= 0 && grid_x < self.span);
        debug_assert!(grid_z >= 0 && grid_z < self.span);
        (grid_z as usize) * self.span as usize + grid_x as usize
    }
}

pub(super) fn build_chunk_surface_field(
    coord: ChunkCoord,
    meta: &WorldMeta,
    atlas_fields: &AtlasFieldMap,
    atlas_structure: &AtlasStructureMap,
) -> ChunkSurfaceField {
    let land_threshold = AtlasTuning::default().normalization.land_threshold;
    let span = CHUNK_EDGE_I32 + SURFACE_FIELD_PADDING * 2;
    let chunk_origin_x = coord.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = coord.2 * CHUNK_EDGE_I32;
    let mut columns = vec![PreparedSurfaceColumn::default(); (span * span) as usize];

    for grid_z in 0..span {
        let world_z = chunk_origin_z + grid_z - SURFACE_FIELD_PADDING;
        for grid_x in 0..span {
            let world_x = chunk_origin_x + grid_x - SURFACE_FIELD_PADDING;
            let atlas_sample = sample_column_atlas(atlas_fields, world_x, world_z);
            let profile = resolve_profile(atlas_sample, land_threshold);
            let structure = sample_structure_guide(world_x, world_z, atlas_structure);
            let base_surface_y =
                surface_height_for_sample(meta.seed, world_x, world_z, atlas_sample, land_threshold);
            let raw_surface_y =
                apply_structure_to_surface(base_surface_y, atlas_sample, profile, structure);
            let index = (grid_z as usize) * span as usize + grid_x as usize;
            columns[index] = PreparedSurfaceColumn {
                atlas_sample,
                profile,
                raw_surface_y,
                surface_y: raw_surface_y,
                local_concavity: 0.0,
                structure,
            };
        }
    }

    let raw_surface_y = columns.iter().map(|column| column.raw_surface_y).collect::<Vec<_>>();
    for grid_z in SURFACE_SMOOTH_RADIUS..(span - SURFACE_SMOOTH_RADIUS) {
        for grid_x in SURFACE_SMOOTH_RADIUS..(span - SURFACE_SMOOTH_RADIUS) {
            let mut weighted_sum = 0.0_f32;
            let mut total_weight = 0.0_f32;

            for kernel_z in -SURFACE_SMOOTH_RADIUS..=SURFACE_SMOOTH_RADIUS {
                for kernel_x in -SURFACE_SMOOTH_RADIUS..=SURFACE_SMOOTH_RADIUS {
                    let weight = SURFACE_SMOOTH_KERNEL
                        [(kernel_z + SURFACE_SMOOTH_RADIUS) as usize]
                        [(kernel_x + SURFACE_SMOOTH_RADIUS) as usize];
                    let sample_index = ((grid_z + kernel_z) as usize) * span as usize
                        + (grid_x + kernel_x) as usize;
                    weighted_sum += raw_surface_y[sample_index] * weight;
                    total_weight += weight;
                }
            }

            let index = (grid_z as usize) * span as usize + grid_x as usize;
            columns[index].surface_y = weighted_sum / total_weight.max(f32::EPSILON);
        }
    }

    for grid_z in SURFACE_CONCAVITY_RADIUS..(span - SURFACE_CONCAVITY_RADIUS) {
        for grid_x in SURFACE_CONCAVITY_RADIUS..(span - SURFACE_CONCAVITY_RADIUS) {
            let index = (grid_z as usize) * span as usize + grid_x as usize;
            let center = columns[index].surface_y;
            let mut neighbor_sum = 0.0_f32;
            let mut neighbor_count = 0_u32;

            for neighbor_z in -SURFACE_CONCAVITY_RADIUS..=SURFACE_CONCAVITY_RADIUS {
                for neighbor_x in -SURFACE_CONCAVITY_RADIUS..=SURFACE_CONCAVITY_RADIUS {
                    if neighbor_x == 0 && neighbor_z == 0 {
                        continue;
                    }

                    let neighbor_index = ((grid_z + neighbor_z) as usize) * span as usize
                        + (grid_x + neighbor_x) as usize;
                    neighbor_sum += columns[neighbor_index].surface_y;
                    neighbor_count += 1;
                }
            }

            let neighbor_mean = neighbor_sum / neighbor_count.max(1) as f32;
            columns[index].local_concavity =
                ((neighbor_mean - center) / LOCAL_CONCAVITY_NORMALIZER).clamp(0.0, 1.0);
        }
    }

    ChunkSurfaceField {
        span,
        padding: SURFACE_FIELD_PADDING,
        columns,
    }
}

fn apply_structure_to_surface(
    base_surface_y: f32,
    atlas_sample: ColumnAtlasSample,
    profile: TerrainProfile,
    structure: PreparedStructureGuide,
) -> f32 {
    let ridge_scale = match profile {
        TerrainProfile::DeepOcean => 0.5,
        TerrainProfile::Shelf => 1.0,
        TerrainProfile::Coast => 2.0,
        TerrainProfile::Plain => 5.0,
        TerrainProfile::Upland => 8.0,
        TerrainProfile::Ridge => 12.0,
    };
    let ridge_raise = structure.ridge_weight
        * structure.ridge_strength
        * ridge_scale
        * (0.55 + atlas_sample.macro_elevation * 0.45 + atlas_sample.mountain_mass * 0.30);
    let channel_scale = match structure.channel_order {
        0 => 0.0,
        1 => 4.0,
        _ => 7.0,
    };
    let channel_drop = structure.channel_weight
        * channel_scale
        * (0.60
            + structure.channel_core * 0.70
            + atlas_sample.river_flow_potential * 0.40
            + atlas_sample.lake_potential * 0.18);

    base_surface_y + ridge_raise - channel_drop
}

fn sample_structure_guide(
    world_x: i32,
    world_z: i32,
    atlas_structure: &AtlasStructureMap,
) -> PreparedStructureGuide {
    let point = (
        (world_x as f32 + 0.5) / ATLAS_CELL_SPAN_BLOCKS_F32,
        (world_z as f32 + 0.5) / ATLAS_CELL_SPAN_BLOCKS_F32,
    );
    let mut guide = PreparedStructureGuide::default();
    let mut best_ridge_score = f32::INFINITY;
    let mut best_channel_score = f32::INFINITY;

    for segment in atlas_structure.mountain_chains().segments() {
        let projection = project_point_onto_segment(point, segment.start, segment.end);
        let influence_radius = ridge_influence_radius_cells(*segment);
        let score = projection.distance_cells / influence_radius.max(f32::EPSILON);
        if score < best_ridge_score {
            best_ridge_score = score;
            guide.ridge_distance_cells = projection.distance_cells;
            guide.ridge_weight = (1.0 - score).clamp(0.0, 1.0);
            guide.ridge_strength = segment.strength;
            guide.ridge_heading_x = projection.heading_x;
            guide.ridge_heading_z = projection.heading_z;
        }
    }

    for segment in atlas_structure.drainage().segments() {
        let projection = project_point_onto_segment(point, segment.start, segment.end);
        let influence_radius = channel_influence_radius_cells(*segment);
        let score = projection.distance_cells / influence_radius.max(f32::EPSILON);
        if score < best_channel_score {
            best_channel_score = score;
            guide.channel_distance_cells = projection.distance_cells;
            guide.channel_weight = (1.0 - score).clamp(0.0, 1.0);
            guide.channel_core = (1.0
                - projection.distance_cells
                    / channel_core_radius_cells(*segment).max(f32::EPSILON))
            .clamp(0.0, 1.0);
            guide.channel_order = segment.order;
            guide.channel_heading_x = projection.heading_x;
            guide.channel_heading_z = projection.heading_z;
            guide.channel_bankfull_hint = segment.bankfull_width_cells;
            guide.along_channel_cells = projection.along_cells;
        }
    }

    guide
}

fn ridge_influence_radius_cells(segment: MountainSpineSegment) -> f32 {
    match segment.scale {
        MountainChainScale::Major => 0.55 + segment.half_width_cells * 1.15,
        MountainChainScale::Minor => 0.34 + segment.half_width_cells * 0.72,
    }
}

fn channel_core_radius_cells(segment: RiverPathSegment) -> f32 {
    let kind_base = match segment.kind {
        RiverPathKind::Trunk => 0.020,
        RiverPathKind::Tributary => 0.014,
    };
    kind_base + segment.bankfull_width_cells * 0.012 + segment.order as f32 * 0.004
}

fn channel_influence_radius_cells(segment: RiverPathSegment) -> f32 {
    channel_core_radius_cells(segment) * 3.6
        + match segment.kind {
            RiverPathKind::Trunk => 0.085,
            RiverPathKind::Tributary => 0.045,
        }
}

#[derive(Debug, Clone, Copy)]
struct SegmentProjection {
    distance_cells: f32,
    along_cells: f32,
    heading_x: f32,
    heading_z: f32,
}

fn project_point_onto_segment(
    point: (f32, f32),
    start: super::super::atlas::AtlasCoord,
    end: super::super::atlas::AtlasCoord,
) -> SegmentProjection {
    let start_x = start.x as f32 + 0.5;
    let start_z = start.z as f32 + 0.5;
    let end_x = end.x as f32 + 0.5;
    let end_z = end.z as f32 + 0.5;
    let seg_x = end_x - start_x;
    let seg_z = end_z - start_z;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return SegmentProjection {
            distance_cells: ((point.0 - start_x).powi(2) + (point.1 - start_z).powi(2)).sqrt(),
            along_cells: 0.0,
            heading_x: 1.0,
            heading_z: 0.0,
        };
    }

    let t = (((point.0 - start_x) * seg_x + (point.1 - start_z) * seg_z) / length_sq).clamp(0.0, 1.0);
    let nearest_x = start_x + seg_x * t;
    let nearest_z = start_z + seg_z * t;
    let length = length_sq.sqrt();

    SegmentProjection {
        distance_cells: ((point.0 - nearest_x).powi(2) + (point.1 - nearest_z).powi(2)).sqrt(),
        along_cells: length * t,
        heading_x: seg_x / length.max(f32::EPSILON),
        heading_z: seg_z / length.max(f32::EPSILON),
    }
}
