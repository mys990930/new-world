use std::f32::consts::TAU;

use crate::world::atlas::{AtlasCoord, MesoGuideMap, MesoRegionCoord, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::super::super::lerp_f32;
use super::{
    GuideSource, RavineApplySample, RavineSurfaceSample, collect_ravine_sources, coverage_union,
    distance_between_points, smoothstep_range, smoothstep01, soft_cap_positive,
};
use super::{
    SOURCE_CENTER_X_SALT, SOURCE_CENTER_Z_SALT, SOURCE_DEPTH_HINT_SALT,
    SOURCE_FALLBACK_HEADING_SALT, SOURCE_HEADING_BLEND_SALT, SOURCE_KEEP_DISTANCE_SALT,
};

const WINDOW_OWNER_PADDING_REGIONS: i32 = 1;
const REGION_RESOLVE_PADDING_REGIONS: i32 = 1;
const MAX_RESOLVED_RAVINES_PER_REGION: usize = 2;
const OWNER_KEEP_DISTANCE_MULTIPLIER: f32 = 1.48;
const RAVINE_BOUNDS_PAD_BLOCKS: f32 = 18.0;

const RESOLVED_LENGTH_SALT: u64 = 0xD811_B6D2_2601_0001;
const RESOLVED_FLOOR_WIDTH_SALT: u64 = 0xD811_B6D2_2601_0002;
const RESOLVED_SHOULDER_WIDTH_SALT: u64 = 0xD811_B6D2_2601_0003;
const RESOLVED_DEPTH_SCALE_SALT: u64 = 0xD811_B6D2_2601_0004;
const RESOLVED_SHOULDER_DEPTH_SALT: u64 = 0xD811_B6D2_2601_0005;
const RESOLVED_MEANDER_PHASE_SALT: u64 = 0xD811_B6D2_2601_0006;
const RESOLVED_MEANDER_SCALE_SALT: u64 = 0xD811_B6D2_2601_0007;
const RESOLVED_MEANDER_WAVELENGTH_SALT: u64 = 0xD811_B6D2_2601_0008;
const RESOLVED_WALL_BALANCE_SALT: u64 = 0xD811_B6D2_2601_0009;
const RESOLVED_FLOOR_SHARPNESS_SALT: u64 = 0xD811_B6D2_2601_000A;
const RESOLVED_CUT_CAP_SALT: u64 = 0xD811_B6D2_2601_000B;

#[derive(Debug, Clone)]
pub(crate) struct RavineWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    ravines: Vec<ResolvedRavine>,
}

#[derive(Debug, Clone)]
struct ResolvedRavine {
    cut_cap_blocks: f32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    half_length_blocks: f32,
    floor_half_width_blocks: f32,
    shoulder_half_width_blocks: f32,
    trench_depth_blocks: f32,
    shoulder_depth_blocks: f32,
    meander_amplitude_blocks: f32,
    meander_wavelength_blocks: f32,
    meander_phase: f32,
    wall_balance: f32,
    floor_sharpness: f32,
}

pub(crate) fn build_ravine_window(guides: &MesoGuideMap, chunk: ChunkCoord) -> RavineWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let chunk_bounds = chunk_bounds(chunk, 0.0);
    let candidates = collect_ravine_sources(guides, meso_span_blocks);
    let (min_region_x, max_region_x, min_region_z, max_region_z) =
        owner_region_range_for_chunk(chunk, WINDOW_OWNER_PADDING_REGIONS);
    let mut ravines = Vec::new();

    for region_z in min_region_z..=max_region_z {
        for region_x in min_region_x..=max_region_x {
            ravines.extend(resolve_owned_ravines(
                &candidates,
                MesoRegionCoord::new(region_x, region_z),
            ));
        }
    }

    ravines.retain(|ravine| {
        bounds_overlap(
            chunk_bounds,
            ravine.min_x,
            ravine.max_x,
            ravine.min_z,
            ravine.max_z,
        )
    });

    RavineWindow { chunk, ravines }
}

fn resolve_owned_ravines(
    candidates: &[GuideSource],
    owner_region: MesoRegionCoord,
) -> Vec<ResolvedRavine> {
    let search_bounds = guide_region_bounds(owner_region, REGION_RESOLVE_PADDING_REGIONS);
    let local_candidates = candidates
        .iter()
        .copied()
        .filter(|candidate| bounds_contains_coord(search_bounds, candidate.coord))
        .collect::<Vec<_>>();
    let mut kept_sources = Vec::new();
    let mut ravines = Vec::new();

    for candidate in local_candidates
        .iter()
        .copied()
        .filter(|candidate| owner_region_for_coord(candidate.coord) == owner_region)
    {
        let overlaps = kept_sources.iter().any(|existing: &GuideSource| {
            distance_between_points(
                (candidate.center_x, candidate.center_z),
                (existing.center_x, existing.center_z),
            ) < candidate
                .keep_distance_blocks
                .max(existing.keep_distance_blocks)
                * OWNER_KEEP_DISTANCE_MULTIPLIER
        });
        if overlaps {
            continue;
        }

        kept_sources.push(candidate);
        ravines.push(resolve_ravine(candidate, &local_candidates));
        if ravines.len() >= MAX_RESOLVED_RAVINES_PER_REGION {
            break;
        }
    }

    ravines
}

fn resolve_ravine(source: GuideSource, nearby_sources: &[GuideSource]) -> ResolvedRavine {
    let (heading_x, heading_z) = ravine_heading(source, nearby_sources);
    let normal_x = -heading_z;
    let normal_z = heading_x;
    let half_length_blocks = lerp_f32(
        96.0,
        216.0,
        guide_hash01(source.coord, RESOLVED_LENGTH_SALT),
    ) * (0.98 + source.weight * 0.14);
    let floor_half_width_blocks =
        lerp_f32(
            4.8,
            9.8,
            guide_hash01(source.coord, RESOLVED_FLOOR_WIDTH_SALT),
        ) * (0.90 + source.cell.escarpment_weight * 0.10 + source.cell.basin_weight * 0.06);
    let shoulder_half_width_blocks = floor_half_width_blocks
        + lerp_f32(
            22.0,
            48.0,
            guide_hash01(source.coord, RESOLVED_SHOULDER_WIDTH_SALT),
        );
    let trench_depth_blocks = source.depth_hint_blocks
        * lerp_f32(
            1.18,
            1.82,
            guide_hash01(source.coord, RESOLVED_DEPTH_SCALE_SALT),
        );
    let shoulder_depth_blocks = trench_depth_blocks
        * lerp_f32(
            0.14,
            0.28,
            guide_hash01(source.coord, RESOLVED_SHOULDER_DEPTH_SALT),
        );
    let meander_amplitude_blocks = floor_half_width_blocks
        * lerp_f32(
            0.42,
            0.92,
            guide_hash01(source.coord, RESOLVED_MEANDER_SCALE_SALT),
        )
        + shoulder_half_width_blocks * 0.025;
    let meander_wavelength_blocks = half_length_blocks
        * lerp_f32(
            0.30,
            0.58,
            guide_hash01(source.coord, RESOLVED_MEANDER_WAVELENGTH_SALT),
        );
    let meander_phase = guide_hash01(source.coord, RESOLVED_MEANDER_PHASE_SALT) * TAU;
    let wall_balance = lerp_f32(
        -0.24,
        0.24,
        guide_hash01(source.coord, RESOLVED_WALL_BALANCE_SALT),
    );
    let floor_sharpness = lerp_f32(
        1.30,
        2.08,
        guide_hash01(source.coord, RESOLVED_FLOOR_SHARPNESS_SALT),
    );
    let cut_cap_blocks = trench_depth_blocks
        * lerp_f32(
            1.28,
            1.76,
            guide_hash01(source.coord, RESOLVED_CUT_CAP_SALT),
        )
        + shoulder_depth_blocks * 0.34;

    let (half_extent_x, half_extent_z) = rotated_ellipse_half_extents(
        heading_x,
        heading_z,
        half_length_blocks + shoulder_half_width_blocks * 0.58,
        shoulder_half_width_blocks + meander_amplitude_blocks + RAVINE_BOUNDS_PAD_BLOCKS,
    );

    ResolvedRavine {
        cut_cap_blocks,
        min_x: source.center_x - half_extent_x,
        max_x: source.center_x + half_extent_x,
        min_z: source.center_z - half_extent_z,
        max_z: source.center_z + half_extent_z,
        center_x: source.center_x + normal_x * wall_balance * floor_half_width_blocks * 0.12,
        center_z: source.center_z + normal_z * wall_balance * floor_half_width_blocks * 0.12,
        heading_x,
        heading_z,
        half_length_blocks,
        floor_half_width_blocks,
        shoulder_half_width_blocks,
        trench_depth_blocks,
        shoulder_depth_blocks,
        meander_amplitude_blocks,
        meander_wavelength_blocks,
        meander_phase,
        wall_balance,
        floor_sharpness,
    }
}

fn ravine_heading(source: GuideSource, nearby_sources: &[GuideSource]) -> (f32, f32) {
    let mut nearest = None;
    let mut nearest_distance = f32::INFINITY;

    for candidate in nearby_sources {
        if candidate.coord == source.coord {
            continue;
        }
        let distance = distance_between_points(
            (source.center_x, source.center_z),
            (candidate.center_x, candidate.center_z),
        );
        if distance > source.keep_distance_blocks * 1.8 || distance >= nearest_distance {
            continue;
        }
        nearest = Some(*candidate);
        nearest_distance = distance;
    }

    if let Some(neighbor) = nearest {
        let delta_x = neighbor.center_x - source.center_x;
        let delta_z = neighbor.center_z - source.center_z;
        let neighbor_heading = normalize_vec2(delta_x, delta_z);
        return normalize_vec2(
            neighbor_heading.0 * 0.68 + source.heading_x * 0.32,
            neighbor_heading.1 * 0.68 + source.heading_z * 0.32,
        );
    }

    (source.heading_x, source.heading_z)
}

pub(crate) fn sample_ravine_apply_signal_from_window(
    window: &RavineWindow,
    world_x: i32,
    world_z: i32,
) -> RavineApplySample {
    let sample_x = world_x as f32 + 0.5;
    let sample_z = world_z as f32 + 0.5;
    let mut strongest_trench = 0.0_f32;
    let mut second_trench = 0.0_f32;
    let mut trench_coverage = 0.0_f32;
    let mut shoulder_coverage = 0.0_f32;
    let mut shoulder_depth_blocks = 0.0_f32;
    let mut cut_cap_weighted_sum = 0.0_f32;
    let mut cut_cap_weight = 0.0_f32;

    for ravine in &window.ravines {
        if sample_x < ravine.min_x
            || sample_x > ravine.max_x
            || sample_z < ravine.min_z
            || sample_z > ravine.max_z
        {
            continue;
        }

        let sample = sample_single_ravine(ravine, sample_x, sample_z);
        if sample.trench_depth_blocks <= f32::EPSILON
            && sample.shoulder_depth_blocks <= f32::EPSILON
            && sample.shoulder_coverage <= f32::EPSILON
        {
            continue;
        }

        if sample.trench_depth_blocks >= strongest_trench {
            second_trench = strongest_trench;
            strongest_trench = sample.trench_depth_blocks;
        } else if sample.trench_depth_blocks > second_trench {
            second_trench = sample.trench_depth_blocks;
        }
        trench_coverage = coverage_union(trench_coverage, sample.trench_coverage);
        shoulder_coverage = coverage_union(
            shoulder_coverage,
            sample.shoulder_coverage.max(sample.trench_coverage),
        );
        shoulder_depth_blocks = shoulder_depth_blocks.max(sample.shoulder_depth_blocks);
        let cap_weight = sample
            .trench_depth_blocks
            .max(sample.shoulder_depth_blocks * 0.42);
        cut_cap_weighted_sum += ravine.cut_cap_blocks * cap_weight;
        cut_cap_weight += cap_weight;
    }

    if strongest_trench <= f32::EPSILON && shoulder_depth_blocks <= f32::EPSILON {
        return RavineApplySample::default();
    }

    RavineApplySample {
        trench_coverage: trench_coverage.clamp(0.0, 1.0),
        shoulder_coverage: shoulder_coverage.max(trench_coverage).clamp(0.0, 1.0),
        trench_depth_blocks: strongest_trench + second_trench * 0.16,
        shoulder_depth_blocks,
        cut_cap_blocks: if cut_cap_weight > f32::EPSILON {
            cut_cap_weighted_sum / cut_cap_weight
        } else {
            0.0
        },
    }
}

pub(crate) fn sample_ravine_surface_from_window(
    window: &RavineWindow,
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> RavineSurfaceSample {
    let apply = sample_ravine_apply_signal_from_window(window, world_x, world_z);
    let trench = apply.trench_coverage.clamp(0.0, 1.0);
    let shoulder = apply.shoulder_coverage.max(trench).clamp(0.0, 1.0);
    let support = apply.shoulder_depth_blocks.max(0.0);
    if trench <= f32::EPSILON && support <= f32::EPSILON {
        return RavineSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let trench_presence = smoothstep_range(0.08, 0.84, trench);
    let shoulder_presence = smoothstep_range(0.12, 0.92, shoulder)
        * (1.0 - smoothstep_range(0.34, 0.90, trench) * 0.18);
    let guide_cut =
        (meso.basin_depth * 0.14 + meso.escarpment_height * 0.10 + meso.terrace_step_height * 0.04)
            * shoulder_presence;
    let shoulder_cut = support
        * (0.78 + meso.basin_weight * 0.10 + meso.escarpment_weight * 0.06)
        * shoulder_presence;
    let trench_cut = apply.trench_depth_blocks
        * (1.28 + meso.basin_weight * 0.14 + meso.escarpment_weight * 0.22)
        * trench_presence;
    let raw_cut_depth = guide_cut + shoulder_cut + trench_cut;
    let cut_cap = apply
        .cut_cap_blocks
        .max((relief_budget * 1.35).max(14.0))
        .min((relief_budget * 2.70).max(32.0));
    let cut_depth = soft_cap_positive(raw_cut_depth, cut_cap);
    if cut_depth <= f32::EPSILON {
        return RavineSurfaceSample::flat(base_surface_y);
    }

    let blend_weight =
        smoothstep01((trench * 0.94 + shoulder * 0.08 + trench_presence * 0.20).clamp(0.0, 1.0));

    RavineSurfaceSample {
        target_surface_y: base_surface_y - cut_depth,
        blend_weight,
        relief_spend: cut_depth * blend_weight,
        trench_coverage: trench,
        shoulder_coverage: shoulder,
    }
}

#[derive(Debug, Clone, Copy)]
struct SingleRavineSample {
    trench_coverage: f32,
    shoulder_coverage: f32,
    trench_depth_blocks: f32,
    shoulder_depth_blocks: f32,
}

fn sample_single_ravine(
    ravine: &ResolvedRavine,
    sample_x: f32,
    sample_z: f32,
) -> SingleRavineSample {
    let delta_x = sample_x - ravine.center_x;
    let delta_z = sample_z - ravine.center_z;
    let along = delta_x * ravine.heading_x + delta_z * ravine.heading_z;
    let raw_across = delta_x * -ravine.heading_z + delta_z * ravine.heading_x;
    let progress = downstream_progress(ravine, along);
    let centerline_offset = centerline_offset(ravine, along, progress);
    let head_taper = smoothstep_range(0.02, 0.18, progress);
    let outlet_taper = 1.0 - smoothstep_range(0.94, 1.08, progress);
    let shoulder_head_taper = smoothstep_range(-0.04, 0.12, progress);
    let shoulder_outlet_taper = 1.0 - smoothstep_range(0.96, 1.14, progress);
    let along_trench_mask = head_taper * outlet_taper;
    let along_shoulder_mask = shoulder_head_taper * shoulder_outlet_taper;
    let across = raw_across - centerline_offset;
    let side_scale = if across < 0.0 {
        (1.0 + ravine.wall_balance).clamp(0.70, 1.34)
    } else {
        (1.0 - ravine.wall_balance).clamp(0.70, 1.34)
    };
    let downstream = smoothstep01(progress);
    let mid_reach =
        smoothstep_range(0.18, 0.42, progress) * (1.0 - smoothstep_range(0.84, 1.04, progress));
    let roughness = incision_roughness(ravine, along, progress);
    let floor_half_width = ravine.floor_half_width_blocks
        * lerp_f32(0.56, 1.36, downstream)
        * lerp_f32(1.08, 0.78, roughness)
        * side_scale;
    let shoulder_half_width = ravine.shoulder_half_width_blocks
        * lerp_f32(0.70, 1.44, downstream)
        * lerp_f32(0.96, 1.10, roughness)
        * side_scale.max(0.82);
    let floor_radial = (across.abs() / floor_half_width.max(f32::EPSILON)).clamp(0.0, 3.0);
    let shoulder_radial = (across.abs() / shoulder_half_width.max(f32::EPSILON)).clamp(0.0, 3.0);
    let valley_wall = smoothstep_range(1.18, 0.0, shoulder_radial)
        * (1.0 - smoothstep_range(0.0, 0.82, floor_radial) * 0.24);
    let trench_coverage = smoothstep_range(1.0, 0.0, floor_radial) * along_trench_mask;
    let shoulder_shell = smoothstep_range(1.0, 0.0, shoulder_radial);
    let shoulder_coverage = coverage_union(
        trench_coverage * 0.78,
        valley_wall * along_shoulder_mask * (1.0 - trench_coverage * 0.42),
    );
    let depth_profile =
        (0.42 + downstream * 0.78 + mid_reach * 0.24 + roughness * 0.34).clamp(0.34, 1.48);
    let shoulder_profile =
        (0.48 + downstream * 0.36 + mid_reach * 0.14 + roughness * 0.12).clamp(0.40, 1.08);
    let trench_depth_blocks = ravine.trench_depth_blocks
        * depth_profile
        * trench_coverage.powf(ravine.floor_sharpness.max(1.0));
    let shoulder_depth_blocks = ravine.shoulder_depth_blocks
        * (shoulder_shell * 0.62 + valley_wall * 0.38)
        * along_shoulder_mask
        * shoulder_profile
        * (1.0 - trench_coverage * 0.68);

    SingleRavineSample {
        trench_coverage,
        shoulder_coverage,
        trench_depth_blocks,
        shoulder_depth_blocks,
    }
}

fn downstream_progress(ravine: &ResolvedRavine, along: f32) -> f32 {
    ((along / ravine.half_length_blocks.max(f32::EPSILON)) * 0.5 + 0.5).clamp(-0.12, 1.12)
}

fn centerline_offset(ravine: &ResolvedRavine, along: f32, progress: f32) -> f32 {
    let head_taper = smoothstep_range(0.02, 0.20, progress);
    let outlet_taper = 1.0 - smoothstep_range(0.94, 1.08, progress);
    let envelope = head_taper * outlet_taper;
    let phase = ravine.meander_phase / TAU;
    let primary = signed_triangle_wave(along / ravine.meander_wavelength_blocks.max(1.0) + phase);
    let secondary = signed_triangle_wave(
        along / (ravine.meander_wavelength_blocks * 0.47).max(1.0) + phase * 1.71,
    );
    let chip = signed_triangle_wave(
        along / (ravine.meander_wavelength_blocks * 0.23).max(1.0) + phase * 0.37,
    );
    let downstream_swing =
        (progress - 0.5).clamp(-0.5, 0.5) * ravine.wall_balance * ravine.floor_half_width_blocks;
    ((primary * 0.54 + secondary * 0.31 + chip * 0.15) * ravine.meander_amplitude_blocks
        + downstream_swing)
        * envelope
}

fn incision_roughness(ravine: &ResolvedRavine, along: f32, progress: f32) -> f32 {
    let envelope =
        smoothstep_range(0.04, 0.22, progress) * (1.0 - smoothstep_range(0.90, 1.08, progress));
    let phase = ravine.meander_phase / TAU;
    let primary = signed_triangle_wave(
        along / (ravine.meander_wavelength_blocks * 0.34).max(1.0) + phase * 1.23,
    );
    let secondary = signed_triangle_wave(
        along / (ravine.meander_wavelength_blocks * 0.19).max(1.0) + phase * 0.51,
    );
    (((primary * 0.62 + secondary * 0.38) * 0.5 + 0.5).clamp(0.0, 1.0)) * envelope
}

fn signed_triangle_wave(value: f32) -> f32 {
    let phase = value.rem_euclid(1.0);
    1.0 - (phase - 0.5).abs() * 4.0
}

fn rotated_ellipse_half_extents(
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
) -> (f32, f32) {
    (
        heading_x.abs() * radius_x_blocks + heading_z.abs() * radius_z_blocks,
        heading_z.abs() * radius_x_blocks + heading_x.abs() * radius_z_blocks,
    )
}

fn chunk_bounds(chunk: ChunkCoord, padding_blocks: f32) -> (f32, f32, f32, f32) {
    let min_x = (chunk.0 * crate::world::CHUNK_EDGE_I32) as f32 - padding_blocks;
    let max_x = ((chunk.0 + 1) * crate::world::CHUNK_EDGE_I32) as f32 + padding_blocks;
    let min_z = (chunk.2 * crate::world::CHUNK_EDGE_I32) as f32 - padding_blocks;
    let max_z = ((chunk.2 + 1) * crate::world::CHUNK_EDGE_I32) as f32 + padding_blocks;
    (min_x, max_x, min_z, max_z)
}

fn owner_region_range_for_chunk(chunk: ChunkCoord, padding_regions: i32) -> (i32, i32, i32, i32) {
    let region_span_blocks =
        crate::world::CHUNK_EDGE_I32 * crate::world::ATLAS_CELL_SIZE_IN_CHUNKS as i32;
    let min_world_x = chunk.0 * crate::world::CHUNK_EDGE_I32;
    let max_world_x = (chunk.0 + 1) * crate::world::CHUNK_EDGE_I32 - 1;
    let min_world_z = chunk.2 * crate::world::CHUNK_EDGE_I32;
    let max_world_z = (chunk.2 + 1) * crate::world::CHUNK_EDGE_I32 - 1;

    (
        min_world_x.div_euclid(region_span_blocks) - padding_regions,
        max_world_x.div_euclid(region_span_blocks) + padding_regions,
        min_world_z.div_euclid(region_span_blocks) - padding_regions,
        max_world_z.div_euclid(region_span_blocks) + padding_regions,
    )
}

fn owner_region_for_coord(coord: AtlasCoord) -> MesoRegionCoord {
    let region_span_cells = crate::world::MESO_GUIDE_CELLS_PER_ATLAS_CELL as i32;
    MesoRegionCoord::new(
        coord.x.div_euclid(region_span_cells),
        coord.z.div_euclid(region_span_cells),
    )
}

fn guide_region_bounds(region: MesoRegionCoord, padding_regions: i32) -> (i32, i32, i32, i32) {
    let region_span_cells = crate::world::MESO_GUIDE_CELLS_PER_ATLAS_CELL as i32;
    (
        (region.x - padding_regions) * region_span_cells,
        (region.x + padding_regions + 1) * region_span_cells - 1,
        (region.z - padding_regions) * region_span_cells,
        (region.z + padding_regions + 1) * region_span_cells - 1,
    )
}

fn bounds_contains_coord(bounds: (i32, i32, i32, i32), coord: AtlasCoord) -> bool {
    coord.x >= bounds.0 && coord.x <= bounds.1 && coord.z >= bounds.2 && coord.z <= bounds.3
}

fn bounds_overlap(
    bounds: (f32, f32, f32, f32),
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
) -> bool {
    max_x >= bounds.0 && min_x <= bounds.1 && max_z >= bounds.2 && min_z <= bounds.3
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

fn guide_hash01(coord: AtlasCoord, salt: u64) -> f32 {
    let mixed = salt
        ^ SOURCE_CENTER_X_SALT
        ^ SOURCE_CENTER_Z_SALT
        ^ SOURCE_DEPTH_HINT_SALT
        ^ SOURCE_FALLBACK_HEADING_SALT
        ^ SOURCE_HEADING_BLEND_SALT
        ^ SOURCE_KEEP_DISTANCE_SALT;
    super::super::super::hash01(0, coord.x as i64, coord.z as i64, mixed)
}

#[cfg(test)]
mod tests {
    use super::{
        RavineWindow, ResolvedRavine, build_ravine_window, sample_ravine_apply_signal_from_window,
        sample_ravine_surface_from_window, signed_triangle_wave,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::coord::ChunkCoord;
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    fn sample_guides() -> MesoGuideMap {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 6).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        let trench = MesoGuideCell {
            basin_weight: 0.86,
            basin_depth: 5.6,
            escarpment_weight: 0.68,
            escarpment_height: 6.4,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            terrace_weight: 0.22,
            terrace_step_height: 1.4,
            terrace_heading_x: 1.0,
            terrace_heading_z: 0.0,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(2, 2)).unwrap() = trench;
        *cells.get_mut(AtlasCoord::new(3, 2)).unwrap() = trench;
        *cells.get_mut(AtlasCoord::new(5, 3)).unwrap() = MesoGuideCell {
            basin_weight: 0.72,
            basin_depth: 4.8,
            escarpment_weight: 0.58,
            escarpment_height: 5.2,
            escarpment_heading_x: 0.0,
            escarpment_heading_z: 1.0,
            ..MesoGuideCell::default()
        };
        MesoGuideMap { area, cells }
    }

    #[test]
    fn build_window_is_deterministic_for_same_chunk() {
        let guides = sample_guides();
        let chunk = ChunkCoord(4, 0, 4);
        let first = build_ravine_window(&guides, chunk);
        let second = build_ravine_window(&guides, chunk);
        let world_x = chunk.0 * CHUNK_EDGE_I32 + 8;
        let world_z = chunk.2 * CHUNK_EDGE_I32 + 8;

        assert_eq!(
            sample_ravine_apply_signal_from_window(&first, world_x, world_z),
            sample_ravine_apply_signal_from_window(&second, world_x, world_z)
        );
    }

    #[test]
    fn neighboring_chunk_windows_sample_same_world_point_consistently() {
        let guides = sample_guides();
        let left_window = build_ravine_window(&guides, ChunkCoord(4, 0, 4));
        let right_window = build_ravine_window(&guides, ChunkCoord(5, 0, 4));
        let boundary_world_x = 5 * CHUNK_EDGE_I32;
        let sample_world_z = 4 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
        let left =
            sample_ravine_apply_signal_from_window(&left_window, boundary_world_x, sample_world_z);
        let right =
            sample_ravine_apply_signal_from_window(&right_window, boundary_world_x, sample_world_z);

        assert_eq!(
            left, right,
            "expected neighboring chunk windows to reuse the same resolved ravine objects at shared world coordinates"
        );
    }

    #[test]
    fn ravine_surface_stays_close_across_chunk_boundary() {
        let guides = sample_guides();
        let left_window = build_ravine_window(&guides, ChunkCoord(4, 0, 4));
        let right_window = build_ravine_window(&guides, ChunkCoord(5, 0, 4));
        let boundary_world_x = 5 * CHUNK_EDGE_I32;
        let sample_world_z = 4 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
        let left_outer = sample_ravine_surface_from_window(
            &left_window,
            &guides,
            boundary_world_x - 2,
            sample_world_z,
            100.0,
            18.0,
        );
        let left_edge = sample_ravine_surface_from_window(
            &left_window,
            &guides,
            boundary_world_x - 1,
            sample_world_z,
            100.0,
            18.0,
        );
        let right_edge = sample_ravine_surface_from_window(
            &right_window,
            &guides,
            boundary_world_x,
            sample_world_z,
            100.0,
            18.0,
        );
        let right_outer = sample_ravine_surface_from_window(
            &right_window,
            &guides,
            boundary_world_x + 2,
            sample_world_z,
            100.0,
            18.0,
        );
        let seam_delta = (right_edge.target_surface_y - left_edge.target_surface_y).abs();
        let local_delta = (left_edge.target_surface_y - left_outer.target_surface_y)
            .abs()
            .max((right_outer.target_surface_y - right_edge.target_surface_y).abs());

        assert!(
            seam_delta <= local_delta + 0.4,
            "expected ravine surface seam to stay within nearby slope change, seam_delta={seam_delta:.3}, local_delta={local_delta:.3}, left_outer={left_outer:?}, left_edge={left_edge:?}, right_edge={right_edge:?}, right_outer={right_outer:?}"
        );
    }

    #[test]
    fn ravine_surface_keeps_material_visible_cut() {
        let guides = sample_guides();
        let meso_span_blocks = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let mut strongest = 0.0_f32;

        for sample_z in (0..meso_span_blocks * 2).step_by(4) {
            for sample_x in (0..meso_span_blocks * 3).step_by(4) {
                let sample = sample_ravine_surface_from_window(
                    &build_ravine_window(&guides, ChunkCoord(4, 0, 4)),
                    &guides,
                    4 * CHUNK_EDGE_I32 + sample_x,
                    4 * CHUNK_EDGE_I32 + sample_z,
                    100.0,
                    18.0,
                );
                strongest = strongest.max(100.0 - sample.target_surface_y);
            }
        }

        assert!(
            strongest >= 4.0,
            "expected ravine runtime resolve to keep a visibly lowered trench, strongest_cut={strongest:.3}"
        );
    }

    #[test]
    fn resolved_ravines_keep_medium_scale_aspect_ratio() {
        let guides = sample_guides();
        let window = build_ravine_window(&guides, ChunkCoord(4, 0, 4));

        assert!(
            !window.ravines.is_empty(),
            "expected sample guides to resolve ravines"
        );
        for ravine in &window.ravines {
            let length = ravine.half_length_blocks * 2.0;
            let floor_width = ravine.floor_half_width_blocks * 2.0;
            assert!(
                length / floor_width.max(f32::EPSILON) >= 6.0,
                "expected ravine to read as a medium-scale valley reach, got length={length:.2}, floor_width={floor_width:.2}"
            );
        }
    }

    #[test]
    fn downstream_reach_is_deeper_than_upstream_head() {
        let ravine = ResolvedRavine {
            cut_cap_blocks: 18.0,
            min_x: -80.0,
            max_x: 208.0,
            min_z: 16.0,
            max_z: 112.0,
            center_x: 64.0,
            center_z: 64.0,
            heading_x: 1.0,
            heading_z: 0.0,
            half_length_blocks: 112.0,
            floor_half_width_blocks: 9.0,
            shoulder_half_width_blocks: 34.0,
            trench_depth_blocks: 9.0,
            shoulder_depth_blocks: 2.4,
            meander_amplitude_blocks: 0.0,
            meander_wavelength_blocks: 72.0,
            meander_phase: 0.0,
            wall_balance: 0.0,
            floor_sharpness: 1.28,
        };
        let window = RavineWindow {
            chunk: ChunkCoord(0, 0, 0),
            ravines: vec![ravine],
        };

        let upstream = sample_ravine_apply_signal_from_window(&window, 24, 64);
        let downstream = sample_ravine_apply_signal_from_window(&window, 104, 64);

        assert!(
            downstream.trench_depth_blocks > upstream.trench_depth_blocks,
            "expected downstream ravine reach to cut deeper than upstream head, upstream={upstream:?}, downstream={downstream:?}"
        );
        assert!(
            downstream.shoulder_coverage >= upstream.shoulder_coverage,
            "expected downstream reach to keep at least as much valley shoulder coverage, upstream={upstream:?}, downstream={downstream:?}"
        );
    }

    #[test]
    fn rough_centerline_primitive_has_angular_turns() {
        let peak = signed_triangle_wave(0.50);
        let before = signed_triangle_wave(0.42);
        let after = signed_triangle_wave(0.58);

        assert!(
            peak > before,
            "expected triangle centerline primitive to climb into a kink"
        );
        assert!(
            peak > after,
            "expected triangle centerline primitive to fall away from a kink"
        );
        assert!(
            signed_triangle_wave(0.0) < 0.0 && signed_triangle_wave(1.0) < 0.0,
            "expected primitive to return to the opposite corner at the next segment boundary"
        );
    }

    #[test]
    fn shoulder_only_lowering_does_not_fake_trench_depth() {
        let ravine = ResolvedRavine {
            cut_cap_blocks: 10.0,
            min_x: 20.0,
            max_x: 108.0,
            min_z: 20.0,
            max_z: 108.0,
            center_x: 64.0,
            center_z: 64.0,
            heading_x: 1.0,
            heading_z: 0.0,
            half_length_blocks: 42.0,
            floor_half_width_blocks: 10.0,
            shoulder_half_width_blocks: 24.0,
            trench_depth_blocks: 0.0,
            shoulder_depth_blocks: 3.6,
            meander_amplitude_blocks: 0.0,
            meander_wavelength_blocks: 64.0,
            meander_phase: 0.0,
            wall_balance: 0.0,
            floor_sharpness: 1.4,
        };
        let window = RavineWindow {
            chunk: ChunkCoord(0, 0, 0),
            ravines: vec![ravine],
        };
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 3, 3).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = MesoGuideCell {
            basin_weight: 0.72,
            basin_depth: 4.2,
            ..MesoGuideCell::default()
        };
        let guides = MesoGuideMap { area, cells };

        let apply = sample_ravine_apply_signal_from_window(&window, 64, 64);
        assert!(
            apply.trench_depth_blocks <= f32::EPSILON,
            "expected shoulder-only sample not to fake a trench core, got {apply:?}"
        );
        assert!(
            apply.shoulder_depth_blocks > 0.0,
            "expected shoulder-only ravine to keep broader lowering, got {apply:?}"
        );

        let surface = sample_ravine_surface_from_window(&window, &guides, 64, 64, 100.0, 16.0);
        assert!(
            surface.target_surface_y < 100.0,
            "expected shoulder-only lowering to still depress the surface, got {surface:?}"
        );
        assert!(
            surface.target_surface_y > 95.0,
            "expected shoulder-only lowering to stay shallower than a real ravine trench, got {surface:?}"
        );
    }
}
