use crate::world::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, CoastalContext,
    ElevationBand, HydrologyContext, RegionClassCell, RegionClassMap,
    RiverPathKind, RiverPathSegment, ReliefClass, TerrainFormFamily,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::inputs::ChunkGenerationV2Inputs;
use super::sample_atlas_fields_fractional;

const ATLAS_CELL_BLOCK_SPAN: f32 = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32) as f32;
const MAX_PROTOTYPE_CORRIDOR_WIDTH_SCALE: f32 = 2.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverCorridorConstraint {
    pub river_id: u32,
    pub kind: RiverPathKind,
    pub order: u8,
    pub start_x: f32,
    pub start_z: f32,
    pub end_x: f32,
    pub end_z: f32,
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
    let chunk_rect = ChunkBlockRect::for_chunk(chunk);
    let mut corridors = Vec::new();

    for segment in inputs.atlas_structure.drainage().segments() {
        let segment_world = segment_world_segment(*segment);
        let midpoint = midpoint(segment_world.start, segment_world.end);
        let sampled_field = sample_atlas_fields_fractional(&inputs.atlas_fields, midpoint.0, midpoint.1);
        let sampled_region = sample_region_cell(
            &inputs.region_classes,
            atlas_coord_for_world_point(midpoint.0, midpoint.1),
        );
        let start_field =
            sample_atlas_fields_fractional(&inputs.atlas_fields, segment_world.start.0, segment_world.start.1);
        let end_field =
            sample_atlas_fields_fractional(&inputs.atlas_fields, segment_world.end.0, segment_world.end.1);
        let half_width_blocks = corridor_half_width_blocks(*segment, sampled_field, sampled_region);
        let selection_radius = corridor_selection_radius(half_width_blocks);

        if !expanded_segment_bounds_overlap_chunk_rect(segment_world, selection_radius, chunk_rect) {
            continue;
        }

        let downstream_grade_per_block = corridor_downstream_grade_per_block(
            *segment,
            sampled_field,
            sampled_region,
            start_field,
            end_field,
        );

        corridors.push(build_corridor_constraint(
            *segment,
            chunk_origin_x,
            chunk_origin_z,
            segment_world,
            half_width_blocks,
            downstream_grade_per_block,
        ));
    }

    corridors.sort_by(|left, right| {
        left.river_id
            .cmp(&right.river_id)
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| kind_rank(left.kind).cmp(&kind_rank(right.kind)))
            .then_with(|| left.start_x.total_cmp(&right.start_x))
            .then_with(|| left.start_z.total_cmp(&right.start_z))
            .then_with(|| left.end_x.total_cmp(&right.end_x))
            .then_with(|| left.end_z.total_cmp(&right.end_z))
    });

    ChunkCorridorWindow { chunk, corridors }
}

#[derive(Debug, Clone, Copy)]
struct SegmentWorldLine {
    start: (f32, f32),
    end: (f32, f32),
}

#[derive(Debug, Clone, Copy)]
struct ChunkBlockRect {
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
}

impl ChunkBlockRect {
    fn for_chunk(chunk: ChunkCoord) -> Self {
        let min_x = (chunk.0 * CHUNK_EDGE_I32) as f32;
        let min_z = (chunk.2 * CHUNK_EDGE_I32) as f32;
        Self {
            min_x,
            max_x: min_x + CHUNK_EDGE_I32 as f32,
            min_z,
            max_z: min_z + CHUNK_EDGE_I32 as f32,
        }
    }
}

fn build_corridor_constraint(
    segment: RiverPathSegment,
    chunk_origin_x: i32,
    chunk_origin_z: i32,
    segment_world: SegmentWorldLine,
    half_width_blocks: f32,
    downstream_grade_per_block: f32,
) -> RiverCorridorConstraint {
    let midpoint = midpoint(segment_world.start, segment_world.end);

    RiverCorridorConstraint {
        river_id: segment.river_id.0,
        kind: segment.kind,
        order: segment.order,
        start_x: segment_world.start.0 - chunk_origin_x as f32,
        start_z: segment_world.start.1 - chunk_origin_z as f32,
        end_x: segment_world.end.0 - chunk_origin_x as f32,
        end_z: segment_world.end.1 - chunk_origin_z as f32,
        center_x: midpoint.0 - chunk_origin_x as f32,
        center_z: midpoint.1 - chunk_origin_z as f32,
        half_width_blocks,
        downstream_grade_per_block,
    }
}

fn corridor_selection_radius(half_width_blocks: f32) -> f32 {
    half_width_blocks.max(16.0) * MAX_PROTOTYPE_CORRIDOR_WIDTH_SCALE
        + CHUNK_EDGE_I32 as f32 * 4.0
        + 32.0
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
        * ATLAS_CELL_BLOCK_SPAN
        * width_scale
        * region_scale
        * field_scale)
        .clamp(16.0, ATLAS_CELL_BLOCK_SPAN * 2.5)
}

fn corridor_downstream_grade_per_block(
    segment: RiverPathSegment,
    sampled_field: AtlasCell,
    sampled_region: RegionClassCell,
    start_field: AtlasCell,
    end_field: AtlasCell,
) -> f32 {
    let segment_length_cells = (segment.downstream_cells_end - segment.downstream_cells_start).max(0.5);
    let segment_length_blocks = (segment_length_cells * ATLAS_CELL_BLOCK_SPAN).max(1.0);
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
    scale *= 1.0 + field.riverine_factor * 0.12 + field.wetness * 0.08 - field.aridity * 0.08;

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
    scale *= 1.0 + field.slope * 0.14 + field.river_flow_potential * 0.08 - field.wetness * 0.06;

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
    (
        coord.x as f32 * ATLAS_CELL_BLOCK_SPAN + ATLAS_CELL_BLOCK_SPAN * 0.5,
        coord.z as f32 * ATLAS_CELL_BLOCK_SPAN + ATLAS_CELL_BLOCK_SPAN * 0.5,
    )
}

fn midpoint(start: (f32, f32), end: (f32, f32)) -> (f32, f32) {
    ((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5)
}

fn atlas_coord_for_world_point(world_x: f32, world_z: f32) -> AtlasCoord {
    AtlasCoord::new(
        (world_x.floor() as i32).div_euclid(ATLAS_CELL_BLOCK_SPAN as i32),
        (world_z.floor() as i32).div_euclid(ATLAS_CELL_BLOCK_SPAN as i32),
    )
}

fn clamp_atlas_coord_to_area(coord: AtlasCoord, area: AtlasArea) -> AtlasCoord {
    let origin = area.origin();
    let max_x = origin.x + area.width() as i32 - 1;
    let max_z = origin.z + area.height() as i32 - 1;
    AtlasCoord::new(coord.x.clamp(origin.x, max_x), coord.z.clamp(origin.z, max_z))
}

fn sample_region_cell(regions: &RegionClassMap, coord: AtlasCoord) -> RegionClassCell {
    let area = regions.area();
    let clamped = clamp_atlas_coord_to_area(coord, area);
    *regions
        .get(clamped)
        .expect("sampled region classification must exist within the input area")
}

fn expanded_segment_bounds_overlap_chunk_rect(
    segment: SegmentWorldLine,
    padding_blocks: f32,
    rect: ChunkBlockRect,
) -> bool {
    let min_x = segment.start.0.min(segment.end.0) - padding_blocks;
    let max_x = segment.start.0.max(segment.end.0) + padding_blocks;
    let min_z = segment.start.1.min(segment.end.1) - padding_blocks;
    let max_z = segment.start.1.max(segment.end.1) + padding_blocks;

    max_x >= rect.min_x && min_x <= rect.max_x && max_z >= rect.min_z && min_z <= rect.max_z
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

    #[test]
    fn matching_corridors_keep_intrinsic_width_and_grade_across_neighboring_chunks() {
        let meta = WorldMeta::new(42);
        let chunk_pairs = [
            (ChunkCoord(15, 0, 15), ChunkCoord(16, 0, 15)),
            (ChunkCoord(15, 0, 16), ChunkCoord(16, 0, 16)),
            (ChunkCoord(0, 0, 0), ChunkCoord(1, 0, 0)),
            (ChunkCoord(-1, 0, -1), ChunkCoord(0, 0, -1)),
            (ChunkCoord(31, 0, -20), ChunkCoord(32, 0, -20)),
            (ChunkCoord(127, 0, 0), ChunkCoord(128, 0, 0)),
        ];

        for (left_chunk, right_chunk) in chunk_pairs {
            let left_inputs = prepare_chunk_v2_inputs(left_chunk, &meta);
            let right_inputs = prepare_chunk_v2_inputs(right_chunk, &meta);
            let left_window = build_chunk_corridor_window(left_chunk, &left_inputs);
            let right_window = build_chunk_corridor_window(right_chunk, &right_inputs);

            for left in &left_window.corridors {
                if let Some(right) = right_window.corridors.iter().find(|candidate| {
                    candidate.river_id == left.river_id
                        && candidate.kind == left.kind
                        && candidate.order == left.order
                        && same_absolute_segment(left_chunk, left, right_chunk, candidate)
                }) {
                    assert!((left.half_width_blocks - right.half_width_blocks).abs() <= f32::EPSILON);
                    assert!(
                        (left.downstream_grade_per_block - right.downstream_grade_per_block).abs()
                            <= f32::EPSILON
                    );
                    return;
                }
            }
        }

        panic!("expected neighboring chunk windows to share at least one corridor");
    }

    fn same_absolute_segment(
        left_chunk: ChunkCoord,
        left: &RiverCorridorConstraint,
        right_chunk: ChunkCoord,
        right: &RiverCorridorConstraint,
    ) -> bool {
        let left_origin_x = left_chunk.0 as f32 * CHUNK_EDGE_I32 as f32;
        let left_origin_z = left_chunk.2 as f32 * CHUNK_EDGE_I32 as f32;
        let right_origin_x = right_chunk.0 as f32 * CHUNK_EDGE_I32 as f32;
        let right_origin_z = right_chunk.2 as f32 * CHUNK_EDGE_I32 as f32;

        (left.start_x + left_origin_x - (right.start_x + right_origin_x)).abs() <= f32::EPSILON
            && (left.start_z + left_origin_z - (right.start_z + right_origin_z)).abs()
                <= f32::EPSILON
            && (left.end_x + left_origin_x - (right.end_x + right_origin_x)).abs() <= f32::EPSILON
            && (left.end_z + left_origin_z - (right.end_z + right_origin_z)).abs() <= f32::EPSILON
    }
}
