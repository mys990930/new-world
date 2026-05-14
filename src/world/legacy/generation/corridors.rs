use crate::world::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, CoastalContext, ElevationBand,
    HydrologyContext, RegionClassCell, RegionClassMap, ReliefClass, RiverPathKind,
    RiverPathSegment, TerrainFormFamily,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::inputs::ChunkGenerationInputs;
use super::sample_atlas_fields_fractional;

use std::f32::consts::TAU;

const ATLAS_CELL_BLOCK_SPAN: f32 = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32) as f32;
const MAX_PROTOTYPE_CORRIDOR_WIDTH_SCALE: f32 = 2.6;
const CORRIDOR_AXIS_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const CORRIDOR_AXIS_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const CORRIDOR_AXIS_SALT: u64 = 0xCA11_D0A5_A751_0001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverCorridorConstraint {
    pub river_id: u32,
    pub basin_id: u32,
    pub main_stem_river_id: u32,
    pub parent_river_id: Option<u32>,
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
    pub downstream_cells_start: f32,
    pub downstream_cells_end: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorridorAxisSample {
    pub raw_signed_distance_blocks: f32,
    pub signed_distance_blocks: f32,
    pub distance_blocks: f32,
    pub longitudinal_error_blocks: f32,
    pub segment_t: f32,
    pub downstream_cells: f32,
    pub downstream_blocks: f32,
    pub segment_length_blocks: f32,
    pub axis_offset_blocks: f32,
    pub tangent_x: f32,
    pub tangent_z: f32,
    pub normal_x: f32,
    pub normal_z: f32,
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
    inputs: &ChunkGenerationInputs,
) -> ChunkCorridorWindow {
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let chunk_rect = ChunkBlockRect::for_chunk(chunk);
    let mut corridors = Vec::new();

    for segment in inputs.atlas_structure.drainage().segments() {
        let segment_world = segment_world_segment(*segment);
        let midpoint = midpoint(segment_world.start, segment_world.end);
        let sampled_field =
            sample_atlas_fields_fractional(&inputs.atlas_fields, midpoint.0, midpoint.1);
        let sampled_region = sample_region_cell(
            &inputs.region_classes,
            atlas_coord_for_world_point(midpoint.0, midpoint.1),
        );
        let start_field = sample_atlas_fields_fractional(
            &inputs.atlas_fields,
            segment_world.start.0,
            segment_world.start.1,
        );
        let end_field = sample_atlas_fields_fractional(
            &inputs.atlas_fields,
            segment_world.end.0,
            segment_world.end.1,
        );
        let half_width_blocks = corridor_half_width_blocks(*segment, sampled_field, sampled_region);
        let selection_radius = corridor_selection_radius(half_width_blocks);

        if !expanded_segment_bounds_overlap_chunk_rect(segment_world, selection_radius, chunk_rect)
        {
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
        basin_id: segment.basin_id.0,
        main_stem_river_id: segment.main_stem_river_id.0,
        parent_river_id: segment.parent_river_id.map(|river_id| river_id.0),
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
        downstream_cells_start: segment.downstream_cells_start,
        downstream_cells_end: segment.downstream_cells_end,
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
    let segment_length_cells =
        (segment.downstream_cells_end - segment.downstream_cells_start).max(0.5);
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

pub fn sample_corridor_axis(
    corridor: RiverCorridorConstraint,
    local_x: f32,
    local_z: f32,
) -> CorridorAxisSample {
    let seg_x = corridor.end_x - corridor.start_x;
    let seg_z = corridor.end_z - corridor.start_z;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        let distance =
            distance_between_points((local_x, local_z), (corridor.start_x, corridor.start_z));
        return CorridorAxisSample {
            raw_signed_distance_blocks: 0.0,
            signed_distance_blocks: 0.0,
            distance_blocks: distance,
            longitudinal_error_blocks: distance,
            segment_t: 0.0,
            downstream_cells: corridor.downstream_cells_start,
            downstream_blocks: corridor.downstream_cells_start * ATLAS_CELL_BLOCK_SPAN,
            segment_length_blocks: 0.0,
            axis_offset_blocks: 0.0,
            tangent_x: 1.0,
            tangent_z: 0.0,
            normal_x: 0.0,
            normal_z: 1.0,
        };
    }

    let segment_length_blocks = length_sq.sqrt();
    let tangent_x = seg_x / segment_length_blocks;
    let tangent_z = seg_z / segment_length_blocks;
    let normal_x = -tangent_z;
    let normal_z = tangent_x;
    let point_rel_x = local_x - corridor.start_x;
    let point_rel_z = local_z - corridor.start_z;
    let raw_signed_distance_blocks = point_rel_x * normal_x + point_rel_z * normal_z;
    let projected_blocks = point_rel_x * tangent_x + point_rel_z * tangent_z;
    let clamped_blocks = projected_blocks.clamp(0.0, segment_length_blocks);
    let segment_t = (clamped_blocks / segment_length_blocks).clamp(0.0, 1.0);
    let longitudinal_error_blocks = if projected_blocks < 0.0 {
        -projected_blocks
    } else if projected_blocks > segment_length_blocks {
        projected_blocks - segment_length_blocks
    } else {
        0.0
    };
    let downstream_cells = lerp_f32(
        corridor.downstream_cells_start,
        corridor.downstream_cells_end,
        segment_t,
    );
    let downstream_blocks = downstream_cells * ATLAS_CELL_BLOCK_SPAN;
    let axis_offset_blocks =
        corridor_axis_offset_blocks(corridor, downstream_blocks, segment_length_blocks);
    let signed_distance_blocks = raw_signed_distance_blocks - axis_offset_blocks;
    let distance_blocks = (signed_distance_blocks * signed_distance_blocks
        + longitudinal_error_blocks.powi(2))
    .sqrt();

    CorridorAxisSample {
        raw_signed_distance_blocks,
        signed_distance_blocks,
        distance_blocks,
        longitudinal_error_blocks,
        segment_t,
        downstream_cells,
        downstream_blocks,
        segment_length_blocks,
        axis_offset_blocks,
        tangent_x,
        tangent_z,
        normal_x,
        normal_z,
    }
}

fn corridor_axis_offset_blocks(
    corridor: RiverCorridorConstraint,
    downstream_blocks: f32,
    segment_length_blocks: f32,
) -> f32 {
    if segment_length_blocks <= f32::EPSILON {
        return 0.0;
    }

    let seed = corridor_axis_seed(corridor);
    let phase = hash_unit(seed) * TAU;
    let half_width = corridor.half_width_blocks.max(1.0);
    let kind_scale = match corridor.kind {
        RiverPathKind::Trunk => 1.0,
        RiverPathKind::Tributary => 0.72,
    };
    let amplitude = (half_width * (0.15 + kind_scale * 0.07) + kind_scale * 3.5)
        .clamp(0.75, half_width * 0.34 + 8.0);
    let wavelength = (half_width * (5.8 + kind_scale * 2.2)
        + ATLAS_CELL_BLOCK_SPAN * (0.55 + kind_scale * 0.36))
        .clamp(96.0, 680.0);
    let primary = (downstream_blocks / wavelength * TAU + phase).sin();
    let secondary = (downstream_blocks / (wavelength * 0.53) * TAU + phase * 1.73).sin();
    let noise = value_noise_1d_signed(
        downstream_blocks + half_width * 13.0,
        wavelength * 0.74,
        seed.rotate_left(17),
    );
    let wave = (primary * 0.54 + secondary * 0.20 + noise * 0.26).clamp(-1.0, 1.0);

    amplitude * wave
}

fn corridor_axis_seed(corridor: RiverCorridorConstraint) -> u64 {
    let branch_bits = ((corridor.river_id as u64) << 17)
        ^ ((corridor.main_stem_river_id as u64) << 5)
        ^ ((corridor.order as u64) << 1)
        ^ kind_rank(corridor.kind) as u64;
    splitmix64(branch_bits ^ CORRIDOR_AXIS_SALT)
}

fn value_noise_1d_signed(x: f32, scale: f32, salt: u64) -> f32 {
    let sample = x / scale.max(1.0);
    let base = sample.floor() as i64;
    let t = smootherstep01(sample - base as f32);
    let a = hash_lattice_1d(base, salt);
    let b = hash_lattice_1d(base + 1, salt);

    lerp_f32(a, b, t) * 2.0 - 1.0
}

fn hash_lattice_1d(x: i64, salt: u64) -> f32 {
    hash_unit(salt ^ (x as u64).wrapping_mul(CORRIDOR_AXIS_HASH_K2))
}

fn hash_unit(value: u64) -> f32 {
    let bits = splitmix64(value) >> 40;
    (bits as u32 as f32) / ((1_u32 << 24) as f32)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(CORRIDOR_AXIS_HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn smootherstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
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
    AtlasCoord::new(
        coord.x.clamp(origin.x, max_x),
        coord.z.clamp(origin.z, max_z),
    )
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

    fn test_corridor(
        start_x: f32,
        end_x: f32,
        downstream_start: f32,
        downstream_end: f32,
    ) -> RiverCorridorConstraint {
        RiverCorridorConstraint {
            river_id: 0xA5A5_1357,
            basin_id: 0xA5A5_1357,
            main_stem_river_id: 0xA5A5_1357,
            parent_river_id: None,
            kind: RiverPathKind::Trunk,
            order: 2,
            start_x,
            start_z: 0.0,
            end_x,
            end_z: 0.0,
            center_x: (start_x + end_x) * 0.5,
            center_z: 0.0,
            half_width_blocks: 32.0,
            downstream_grade_per_block: 0.0018,
            downstream_cells_start: downstream_start,
            downstream_cells_end: downstream_end,
        }
    }

    #[test]
    fn corridor_axis_sample_keeps_downstream_phase_continuous_across_segments() {
        let first = test_corridor(0.0, 32.0, 0.0, 1.0);
        let second = test_corridor(32.0, 64.0, 1.0, 2.0);

        let first_end = sample_corridor_axis(first, 32.0, 0.0);
        let second_start = sample_corridor_axis(second, 32.0, 0.0);

        assert!((first_end.downstream_blocks - second_start.downstream_blocks).abs() <= 0.001);
        assert!((first_end.axis_offset_blocks - second_start.axis_offset_blocks).abs() <= 0.001);
        assert!(
            (first_end.signed_distance_blocks - second_start.signed_distance_blocks).abs() <= 0.001
        );
    }

    #[test]
    fn corridor_axis_sample_uses_downstream_progress_instead_of_segment_local_t() {
        let carried_segment = test_corridor(32.0, 64.0, 1.0, 2.0);
        let whole_branch_span = test_corridor(0.0, 64.0, 0.0, 2.0);

        let carried_mid = sample_corridor_axis(carried_segment, 48.0, 0.0);
        let whole_same_progress = sample_corridor_axis(whole_branch_span, 48.0, 0.0);

        assert!(
            (carried_mid.downstream_blocks - whole_same_progress.downstream_blocks).abs() <= 0.001
        );
        assert!((carried_mid.segment_t - whole_same_progress.segment_t).abs() >= 0.10);
        assert!(
            (carried_mid.axis_offset_blocks - whole_same_progress.axis_offset_blocks).abs()
                <= 0.001
        );
        assert!(
            (carried_mid.signed_distance_blocks - whole_same_progress.signed_distance_blocks).abs()
                <= 0.001
        );
    }

    #[test]
    fn corridor_axis_sample_moves_visible_axis_off_raw_segment() {
        let corridor = test_corridor(0.0, 192.0, 0.0, 3.0);
        let samples = [
            sample_corridor_axis(corridor, 48.0, 0.0),
            sample_corridor_axis(corridor, 96.0, 0.0),
            sample_corridor_axis(corridor, 144.0, 0.0),
        ];

        assert!(
            samples
                .iter()
                .any(|sample| sample.axis_offset_blocks.abs() >= 0.10),
            "expected at least one branch-progress sample to warp the visible axis"
        );
        for sample in samples {
            assert!(sample.raw_signed_distance_blocks.abs() <= 0.001);
            assert!((sample.signed_distance_blocks + sample.axis_offset_blocks).abs() <= 0.001);
        }
    }
}
