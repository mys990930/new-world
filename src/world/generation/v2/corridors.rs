use crate::world::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, AtlasFieldMap, CoastalContext,
    ElevationBand, HydrologyContext, RegionArchetype, RegionClassCell, RegionClassMap,
    RiverPathKind, RiverPathSegment, ReliefClass, TerrainFormFamily,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::inputs::ChunkGenerationV2Inputs;

const ATLAS_CELL_BLOCK_SPAN: i32 = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
const CHUNK_HALF_EDGE_F32: f32 = CHUNK_EDGE_I32 as f32 * 0.5;
const MAX_CORRIDORS_PER_CHUNK: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverCorridorConstraint {
    pub center_x: f32,
    pub center_z: f32,
    pub half_width_blocks: f32,
    pub downstream_grade_per_block: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkCorridorWindow {
    pub chunk: ChunkCoord,
    pub corridors: Vec<RiverCorridorConstraint>,
}

pub fn empty_chunk_corridor_window(chunk: ChunkCoord) -> ChunkCorridorWindow {
    ChunkCorridorWindow {
        chunk,
        corridors: Vec::new(),
    }
}

pub fn build_chunk_corridor_window(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
) -> ChunkCorridorWindow {
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let chunk_center = (
        chunk_origin_x as f32 + CHUNK_HALF_EDGE_F32,
        chunk_origin_z as f32 + CHUNK_HALF_EDGE_F32,
    );
    let selection_area = chunk_atlas_selection_area(chunk);
    let mut candidates = Vec::new();

    for segment in inputs.atlas_structure.drainage().segments() {
        let segment_world = segment_world_segment(*segment);
        let projection = project_point_onto_segment(chunk_center, segment_world.start, segment_world.end);
        let selection_radius = corridor_selection_radius(*segment);
        let touches_chunk_window = segment.touches_area(selection_area);

        if !touches_chunk_window && projection.distance_cells > selection_radius {
            continue;
        }

        let sample_coord = atlas_coord_for_world_point(projection.point.0, projection.point.1);
        let sampled_field = sample_atlas_field(&inputs.atlas_fields, sample_coord);
        let sampled_region = sample_region_cell(&inputs.region_classes, sample_coord);
        let start_field = sample_atlas_field(&inputs.atlas_fields, segment.start);
        let end_field = sample_atlas_field(&inputs.atlas_fields, segment.end);
        let corridor = build_corridor_constraint(
            *segment,
            chunk_origin_x,
            chunk_origin_z,
            projection,
            sampled_field,
            sampled_region,
            start_field,
            end_field,
        );
        let score = corridor_priority_score(
            *segment,
            projection.distance_cells,
            selection_radius,
            touches_chunk_window,
            sampled_field,
            sampled_region,
        );

        candidates.push(ScoredCorridor {
            score,
            river_id: segment.river_id.0,
            kind_rank: kind_rank(segment.kind),
            order: segment.order,
            start_x: segment.start.x,
            start_z: segment.start.z,
            end_x: segment.end.x,
            end_z: segment.end.z,
            corridor,
        });
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.river_id.cmp(&right.river_id))
            .then_with(|| left.kind_rank.cmp(&right.kind_rank))
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| left.start_x.cmp(&right.start_x))
            .then_with(|| left.start_z.cmp(&right.start_z))
            .then_with(|| left.end_x.cmp(&right.end_x))
            .then_with(|| left.end_z.cmp(&right.end_z))
    });
    candidates.truncate(MAX_CORRIDORS_PER_CHUNK);

    ChunkCorridorWindow {
        chunk,
        corridors: candidates.into_iter().map(|candidate| candidate.corridor).collect(),
    }
}

#[derive(Debug, Clone, Copy)]
struct ScoredCorridor {
    score: f32,
    river_id: u32,
    kind_rank: u8,
    order: u8,
    start_x: i32,
    start_z: i32,
    end_x: i32,
    end_z: i32,
    corridor: RiverCorridorConstraint,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedPoint {
    point: (f32, f32),
    distance_cells: f32,
}

#[derive(Debug, Clone, Copy)]
struct SegmentWorldLine {
    start: (f32, f32),
    end: (f32, f32),
}

fn build_corridor_constraint(
    segment: RiverPathSegment,
    chunk_origin_x: i32,
    chunk_origin_z: i32,
    projection: ProjectedPoint,
    sampled_field: AtlasCell,
    sampled_region: RegionClassCell,
    start_field: AtlasCell,
    end_field: AtlasCell,
) -> RiverCorridorConstraint {
    let half_width_blocks = corridor_half_width_blocks(segment, sampled_field, sampled_region);
    let downstream_grade_per_block =
        corridor_downstream_grade_per_block(
            segment,
            sampled_field,
            sampled_region,
            start_field,
            end_field,
            projection.distance_cells,
        );

    RiverCorridorConstraint {
        center_x: projection.point.0 - chunk_origin_x as f32,
        center_z: projection.point.1 - chunk_origin_z as f32,
        half_width_blocks,
        downstream_grade_per_block,
    }
}

fn corridor_priority_score(
    segment: RiverPathSegment,
    distance_cells: f32,
    selection_radius_cells: f32,
    touches_chunk_window: bool,
    sampled_field: AtlasCell,
    sampled_region: RegionClassCell,
) -> f32 {
    let proximity_score = if selection_radius_cells <= f32::EPSILON {
        0.0
    } else {
        (1.0 - distance_cells / selection_radius_cells).clamp(0.0, 1.0)
    };
    let role_score = match segment.kind {
        RiverPathKind::Trunk => 0.55,
        RiverPathKind::Tributary => 0.34,
    } + segment.order as f32 * 0.06;
    let context_score = region_corridor_priority(sampled_field, sampled_region);
    let continuity_score = (1.0 - downstream_progress(segment) * 0.24).max(0.72);

    (if touches_chunk_window { 1.9 } else { 0.0 })
        + proximity_score * 1.6
        + role_score
        + context_score
        + continuity_score * 0.25
}

fn corridor_selection_radius(segment: RiverPathSegment) -> f32 {
    let base = segment.bankfull_width_cells.max(0.35) * ATLAS_CELL_BLOCK_SPAN as f32 * 0.28;
    base + CHUNK_EDGE_I32 as f32 * 4.0 + 32.0
}

fn corridor_half_width_blocks(
    segment: RiverPathSegment,
    sampled_field: AtlasCell,
    sampled_region: RegionClassCell,
) -> f32 {
    let width_scale = match segment.kind {
        RiverPathKind::Trunk => 0.34,
        RiverPathKind::Tributary => 0.26,
    };
    let region_scale = region_width_scale(sampled_field, sampled_region);
    let field_scale = 0.88
        + sampled_field.riverine_factor * 0.28
        + sampled_field.wetness * 0.16
        + sampled_field.basinness * 0.12
        - sampled_field.aridity * 0.10;

    (segment.bankfull_width_cells.max(0.35)
        * ATLAS_CELL_BLOCK_SPAN as f32
        * width_scale
        * region_scale
        * field_scale)
        .clamp(16.0, ATLAS_CELL_BLOCK_SPAN as f32 * 2.5)
}

fn corridor_downstream_grade_per_block(
    segment: RiverPathSegment,
    sampled_field: AtlasCell,
    sampled_region: RegionClassCell,
    start_field: AtlasCell,
    end_field: AtlasCell,
    distance_cells: f32,
) -> f32 {
    let segment_length_blocks = (distance_cells.max(0.5) * ATLAS_CELL_BLOCK_SPAN as f32).max(1.0);
    let height_drop = (start_field.macro_elevation - end_field.macro_elevation).max(0.0);
    let flow_signal = (start_field.river_flow_potential + end_field.river_flow_potential) * 0.5;
    let slope_signal = (start_field.slope + end_field.slope) * 0.5;
    let wetness_signal = (start_field.wetness + end_field.wetness) * 0.5;
    let basin_signal = (start_field.basinness + end_field.basinness) * 0.5;
    let region_scale = region_grade_scale(sampled_field, sampled_region);
    let progress_scale = 1.0 - downstream_progress(segment) * 0.18;
    let mut grade = match segment.kind {
        RiverPathKind::Trunk => 0.0017,
        RiverPathKind::Tributary => 0.0029,
    };

    grade += height_drop / segment_length_blocks * 1.35;
    grade += slope_signal * 0.0010;
    grade += flow_signal * 0.0006;
    grade += basin_signal * 0.00018;
    grade += (1.0 - wetness_signal) * 0.00012;
    grade *= region_scale * progress_scale;

    grade.clamp(0.00020, 0.0120)
}

fn region_corridor_priority(field: AtlasCell, region: RegionClassCell) -> f32 {
    let mut score = field.river_flow_potential * 0.35
        + field.riverine_factor * 0.30
        + field.wetness * 0.18
        - field.aridity * 0.10;

    score += match region.hydrology_context {
        HydrologyContext::RiverCorridor => 0.55,
        HydrologyContext::WetLowland => 0.50,
        HydrologyContext::LakeBasin => 0.40,
        HydrologyContext::WellDrained => 0.12,
        HydrologyContext::Dryland => -0.08,
    };
    score += match region.coastal_context {
        CoastalContext::Marine => 0.32,
        CoastalContext::Coastal => 0.20,
        CoastalContext::NearCoast => 0.08,
        CoastalContext::Inland => 0.0,
    };
    score += match region.elevation_band {
        ElevationBand::Alpine => 0.20,
        ElevationBand::Highland => 0.12,
        ElevationBand::Upland => 0.05,
        ElevationBand::Low => 0.0,
    };
    score += match region.relief_class {
        ReliefClass::Mountain => 0.15,
        ReliefClass::Hill => 0.08,
        ReliefClass::Rolling => 0.03,
        ReliefClass::Plain => 0.0,
    };
    score += match region.terrain_form_family {
        TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland => 0.22,
        TerrainFormFamily::BroadValley | TerrainFormFamily::NarrowValley => 0.12,
        TerrainFormFamily::Basin => 0.10,
        TerrainFormFamily::Canyon | TerrainFormFamily::RavineCountry => 0.08,
        TerrainFormFamily::MarineShelf | TerrainFormFamily::BeachPlain => 0.06,
        TerrainFormFamily::RidgeCountry => 0.04,
        _ => 0.0,
    };
    score += match region.archetype {
        RegionArchetype::CoastalDelta
        | RegionArchetype::EstuaryLowland
        | RegionArchetype::MarshFloodplain
        | RegionArchetype::SwampLowland
        | RegionArchetype::FloodedForestFloodplain
        | RegionArchetype::FloodedForestAlluvialLowland => 0.20,
        RegionArchetype::GlaciatedAlpine
        | RegionArchetype::AlpineRavineCountry
        | RegionArchetype::BorealRidgeCountry => 0.08,
        RegionArchetype::DesertDuneField
        | RegionArchetype::DesertMesaCountry
        | RegionArchetype::SemiDesertPediment => -0.03,
        _ => 0.0,
    };

    score
}

fn region_width_scale(field: AtlasCell, region: RegionClassCell) -> f32 {
    let mut scale = 1.0;

    scale *= match region.hydrology_context {
        HydrologyContext::RiverCorridor => 1.10,
        HydrologyContext::WetLowland => 1.18,
        HydrologyContext::LakeBasin => 1.08,
        HydrologyContext::WellDrained => 1.0,
        HydrologyContext::Dryland => 0.86,
    };
    scale *= match region.coastal_context {
        CoastalContext::Marine => 1.12,
        CoastalContext::Coastal => 1.08,
        CoastalContext::NearCoast => 1.04,
        CoastalContext::Inland => 1.0,
    };
    scale *= match region.elevation_band {
        ElevationBand::Alpine => 0.88,
        ElevationBand::Highland => 0.94,
        ElevationBand::Upland => 0.98,
        ElevationBand::Low => 1.0,
    };
    scale *= match region.relief_class {
        ReliefClass::Mountain => 0.90,
        ReliefClass::Hill => 0.96,
        ReliefClass::Rolling => 1.0,
        ReliefClass::Plain => 1.03,
    };
    scale *= match region.terrain_form_family {
        TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland => 1.14,
        TerrainFormFamily::Basin => 1.06,
        TerrainFormFamily::BroadValley | TerrainFormFamily::NarrowValley => 1.02,
        TerrainFormFamily::Canyon | TerrainFormFamily::RavineCountry => 0.90,
        TerrainFormFamily::RidgeCountry => 0.92,
        TerrainFormFamily::MarineShelf | TerrainFormFamily::BeachPlain => 1.05,
        TerrainFormFamily::MesaCountry | TerrainFormFamily::Badlands => 0.88,
        _ => 1.0,
    };
    scale *= 1.0
        + field.riverine_factor * 0.12
        + field.wetness * 0.08
        - field.aridity * 0.08;

    scale.clamp(0.72, 1.45)
}

fn region_grade_scale(field: AtlasCell, region: RegionClassCell) -> f32 {
    let mut scale = 1.0;

    scale *= match region.hydrology_context {
        HydrologyContext::RiverCorridor => 1.04,
        HydrologyContext::WetLowland => 0.72,
        HydrologyContext::LakeBasin => 0.58,
        HydrologyContext::WellDrained => 1.0,
        HydrologyContext::Dryland => 1.10,
    };
    scale *= match region.coastal_context {
        CoastalContext::Marine => 0.84,
        CoastalContext::Coastal => 0.92,
        CoastalContext::NearCoast => 0.97,
        CoastalContext::Inland => 1.0,
    };
    scale *= match region.elevation_band {
        ElevationBand::Alpine => 1.18,
        ElevationBand::Highland => 1.10,
        ElevationBand::Upland => 1.03,
        ElevationBand::Low => 0.96,
    };
    scale *= match region.relief_class {
        ReliefClass::Mountain => 1.16,
        ReliefClass::Hill => 1.06,
        ReliefClass::Rolling => 1.00,
        ReliefClass::Plain => 0.96,
    };
    scale *= match region.terrain_form_family {
        TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland => 0.68,
        TerrainFormFamily::Basin => 0.84,
        TerrainFormFamily::BroadValley | TerrainFormFamily::NarrowValley => 0.90,
        TerrainFormFamily::Canyon | TerrainFormFamily::RavineCountry => 1.12,
        TerrainFormFamily::RidgeCountry => 1.08,
        TerrainFormFamily::MesaCountry | TerrainFormFamily::Badlands => 1.10,
        TerrainFormFamily::MarineShelf | TerrainFormFamily::BeachPlain => 0.88,
        _ => 1.0,
    };
    scale *= 1.0
        + field.slope * 0.14
        + field.river_flow_potential * 0.08
        - field.wetness * 0.06;

    scale.clamp(0.45, 1.40)
}

fn downstream_progress(segment: RiverPathSegment) -> f32 {
    let total = segment.downstream_cells_end.max(1.0);
    (segment.downstream_cells_start / total).clamp(0.0, 1.0)
}

fn kind_rank(kind: RiverPathKind) -> u8 {
    match kind {
        RiverPathKind::Trunk => 0,
        RiverPathKind::Tributary => 1,
    }
}

fn segment_world_segment(segment: RiverPathSegment) -> SegmentWorldLine {
    SegmentWorldLine {
        start: atlas_coord_to_world_center(segment.start),
        end: atlas_coord_to_world_center(segment.end),
    }
}

fn atlas_coord_to_world_center(coord: AtlasCoord) -> (f32, f32) {
    let span = ATLAS_CELL_BLOCK_SPAN as f32;
    (
        coord.x as f32 * span + span * 0.5,
        coord.z as f32 * span + span * 0.5,
    )
}

fn atlas_coord_for_world_point(world_x: f32, world_z: f32) -> AtlasCoord {
    AtlasCoord::new(
        (world_x.floor() as i32).div_euclid(ATLAS_CELL_BLOCK_SPAN),
        (world_z.floor() as i32).div_euclid(ATLAS_CELL_BLOCK_SPAN),
    )
}

fn clamp_atlas_coord_to_area(coord: AtlasCoord, area: AtlasArea) -> AtlasCoord {
    let origin = area.origin();
    let max_x = origin.x + area.width() as i32 - 1;
    let max_z = origin.z + area.height() as i32 - 1;
    AtlasCoord::new(coord.x.clamp(origin.x, max_x), coord.z.clamp(origin.z, max_z))
}

fn sample_atlas_field(fields: &AtlasFieldMap, coord: AtlasCoord) -> AtlasCell {
    let area = fields.area();
    let clamped = clamp_atlas_coord_to_area(coord, area);
    *fields
        .get(clamped)
        .expect("sampled atlas field must exist within the input area")
}

fn sample_region_cell(regions: &RegionClassMap, coord: AtlasCoord) -> RegionClassCell {
    let area = regions.area();
    let clamped = clamp_atlas_coord_to_area(coord, area);
    *regions
        .get(clamped)
        .expect("sampled region classification must exist within the input area")
}

fn chunk_atlas_selection_area(chunk: ChunkCoord) -> AtlasArea {
    let chunk_atlas_coord = AtlasCoord::new(
        chunk.0.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
        chunk.2.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
    );
    AtlasArea::new(
        AtlasCoord::new(chunk_atlas_coord.x - 1, chunk_atlas_coord.z - 1),
        3,
        3,
    )
    .expect("chunk atlas selection area must be valid")
}

fn project_point_onto_segment(
    point: (f32, f32),
    start: (f32, f32),
    end: (f32, f32),
) -> ProjectedPoint {
    let seg_x = end.0 - start.0;
    let seg_z = end.1 - start.1;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return ProjectedPoint {
            point: start,
            distance_cells: distance_between_points(point, start),
        };
    }

    let t = (((point.0 - start.0) * seg_x + (point.1 - start.1) * seg_z) / length_sq).clamp(0.0, 1.0);
    let projected = (start.0 + seg_x * t, start.1 + seg_z * t);

    ProjectedPoint {
        point: projected,
        distance_cells: distance_between_points(point, projected),
    }
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::meta::WorldMeta;

    use super::super::inputs::prepare_chunk_v2_inputs;

    fn has_outside_edge_center(window: &ChunkCorridorWindow) -> bool {
        window.corridors.iter().any(|corridor| {
            corridor.center_x < 0.0
                || corridor.center_z < 0.0
                || corridor.center_x > CHUNK_EDGE_I32 as f32
                || corridor.center_z > CHUNK_EDGE_I32 as f32
        })
    }

    #[test]
    fn corridor_window_is_deterministic() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);

        let a = build_chunk_corridor_window(chunk, &inputs);
        let b = build_chunk_corridor_window(chunk, &inputs);

        assert_eq!(a, b);
    }

    #[test]
    fn corridor_window_finds_relevant_drainage_for_a_hit_chunk() {
        let meta = WorldMeta::new(42);
        let chunks = [
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
        ];

        let mut found = None;
        for chunk in chunks {
            let inputs = prepare_chunk_v2_inputs(chunk, &meta);
            let window = build_chunk_corridor_window(chunk, &inputs);
            if !window.corridors.is_empty() {
                found = Some((chunk, window));
                break;
            }
        }

        let Some((chunk, window)) = found else {
            panic!("expected at least one sampled chunk to intersect drainage");
        };

        assert_eq!(window.chunk, chunk);
        assert!(!window.corridors.is_empty());
    }

    #[test]
    fn corridor_centers_can_extend_beyond_the_strict_chunk_footprint() {
        let meta = WorldMeta::new(42);
        let chunks = [
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
            ChunkCoord(0, 0, 0),
        ];

        for chunk in chunks {
            let inputs = prepare_chunk_v2_inputs(chunk, &meta);
            let window = build_chunk_corridor_window(chunk, &inputs);
            if !window.corridors.is_empty() && has_outside_edge_center(&window) {
                return;
            }
        }

        panic!("expected at least one corridor center to allow outside-edge influence");
    }
}
