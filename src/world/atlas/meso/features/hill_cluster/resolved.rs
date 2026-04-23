use crate::world::atlas::{AtlasCoord, MesoGuideMap, MesoRegionCoord, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::{
    GuideSource, HillClusterApplySample, HillClusterSurfaceSample, MacroLobeDescriptor,
    collect_peak_sources, distance_between_points, indexed_lobe_hash01,
    insert_top3, irregular_lobe_footprint, lobe_hash01, smoothstep01, smoothstep_range,
    soft_cap_positive,
};
use super::{
    SOURCE_CHAIN_SPACING_SALT, SOURCE_COUNT_SALT, SOURCE_ENVELOPE_FILL_SALT,
    SOURCE_ENVELOPE_RADIUS_SALT, SOURCE_ENVELOPE_SHOULDER_SALT, SOURCE_MAJOR_RADIUS_SALT,
    SOURCE_MINOR_RADIUS_SALT, SOURCE_SUMMIT_CAP_SALT, SOURCE_SUMMIT_HEIGHT_SALT,
    SOURCE_SUMMIT_PROFILE_SALT,
};
use super::super::super::lerp_f32;

const WINDOW_OWNER_PADDING_REGIONS: i32 = 1;
const REGION_RESOLVE_PADDING_REGIONS: i32 = 1;
const MAX_RESOLVED_HILLS_PER_REGION: usize = 6;
const OWNER_PEAK_KEEP_DISTANCE_MULTIPLIER: f32 = 1.18;
const HILL_BOUNDS_PAD_BLOCKS: f32 = 8.0;
const SUPPORT_BOUNDS_PAD_BLOCKS: f32 = 6.0;

#[derive(Debug, Clone)]
pub(crate) struct HillClusterWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    hills: Vec<ResolvedHill>,
}

#[derive(Debug, Clone)]
struct ResolvedHill {
    peak_height_hint: f32,
    raise_cap_blocks: f32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    primary: ResolvedHillBlob,
    secondary: Option<ResolvedHillBlob>,
    support: ResolvedHillSupport,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedHillBlob {
    lobe: MacroLobeDescriptor,
    source: GuideSource,
    lobe_index: usize,
    core_scale: f32,
    peak_exponent: f32,
    raise_cap_blocks: f32,
    half_extent_x: f32,
    half_extent_z: f32,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedHillSupport {
    source: GuideSource,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
    support_height_blocks: f32,
    half_extent_x: f32,
    half_extent_z: f32,
}

pub(crate) fn build_window(guides: &MesoGuideMap, chunk: ChunkCoord) -> HillClusterWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let chunk_bounds = chunk_bounds(chunk, 0.0);
    let candidates = collect_peak_sources(guides, meso_span_blocks);
    let (min_region_x, max_region_x, min_region_z, max_region_z) =
        owner_region_range_for_chunk(chunk, WINDOW_OWNER_PADDING_REGIONS);
    let mut hills = Vec::new();

    for region_z in min_region_z..=max_region_z {
        for region_x in min_region_x..=max_region_x {
            hills.extend(resolve_owned_hills(
                &candidates,
                MesoRegionCoord::new(region_x, region_z),
            ));
        }
    }

    hills.retain(|hill| bounds_overlap(chunk_bounds, hill.min_x, hill.max_x, hill.min_z, hill.max_z));

    HillClusterWindow { chunk, hills }
}

fn resolve_owned_hills(
    candidates: &[GuideSource],
    owner_region: MesoRegionCoord,
) -> Vec<ResolvedHill> {
    let search_bounds = guide_region_bounds(owner_region, REGION_RESOLVE_PADDING_REGIONS);
    let local_candidates = candidates
        .iter()
        .copied()
        .filter(|candidate| bounds_contains_coord(search_bounds, candidate.coord))
        .collect::<Vec<_>>();
    let mut kept_sources = Vec::new();
    let mut hills = Vec::new();

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
                .keepout_radius_blocks
                .max(existing.keepout_radius_blocks)
                * OWNER_PEAK_KEEP_DISTANCE_MULTIPLIER
        });
        if overlaps {
            continue;
        }

        if let Some(hill) = resolve_hill(candidate, &local_candidates) {
            kept_sources.push(candidate);
            hills.push(hill);
        }

        if hills.len() >= MAX_RESOLVED_HILLS_PER_REGION {
            break;
        }
    }

    hills
}

fn resolve_hill(source: GuideSource, nearby_sources: &[GuideSource]) -> Option<ResolvedHill> {
    let (heading_x, heading_z) = hill_heading(source, nearby_sources);
    let normal_x = -heading_z;
    let normal_z = heading_x;
    let primary_major = lerp_f32(
        56.0,
        88.0,
        lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
    ) * (0.98 + source.cell.hilliness * 0.24 + (source.cell.hill_height / 18.0).clamp(0.0, 0.20));
    let primary_minor = lerp_f32(
        44.0,
        72.0,
        lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
    ) * (1.00 + source.cell.hilliness * 0.22 + (source.cell.hill_height / 22.0).clamp(0.0, 0.16));
    let primary_height = source.cell.hill_height
        * (1.32 + source.cell.hilliness * 0.36)
        * lerp_f32(
            1.12,
            1.60,
            lobe_hash01(source.coord, source.cell, SOURCE_SUMMIT_HEIGHT_SALT),
        );
    let primary = make_blob(
        source,
        0,
        source.center_x,
        source.center_z,
        heading_x,
        heading_z,
        primary_major,
        primary_minor,
        primary_height,
    );
    let secondary = resolve_secondary_blob(
        source,
        heading_x,
        heading_z,
        normal_x,
        normal_z,
        primary.lobe,
    );
    let support = resolve_support(source, heading_x, heading_z, primary.lobe, secondary.map(|blob| blob.lobe));

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    include_bounds(
        &mut min_x,
        &mut max_x,
        &mut min_z,
        &mut max_z,
        primary.lobe.center_x,
        primary.lobe.center_z,
        primary.half_extent_x,
        primary.half_extent_z,
    );
    if let Some(secondary) = secondary {
        include_bounds(
            &mut min_x,
            &mut max_x,
            &mut min_z,
            &mut max_z,
            secondary.lobe.center_x,
            secondary.lobe.center_z,
            secondary.half_extent_x,
            secondary.half_extent_z,
        );
    }
    include_bounds(
        &mut min_x,
        &mut max_x,
        &mut min_z,
        &mut max_z,
        support.center_x,
        support.center_z,
        support.half_extent_x,
        support.half_extent_z,
    );

    let secondary_height = secondary
        .as_ref()
        .map(|blob| blob.lobe.height_blocks)
        .unwrap_or(0.0);
    let peak_height_hint = primary.lobe.height_blocks.max(secondary_height);
    let raise_cap_blocks = primary
        .raise_cap_blocks
        .max(secondary.as_ref().map(|blob| blob.raise_cap_blocks).unwrap_or(0.0))
        .max(peak_height_hint * 1.22)
        .clamp(18.0, 42.0);

    Some(ResolvedHill {
        peak_height_hint,
        raise_cap_blocks,
        min_x,
        max_x,
        min_z,
        max_z,
        primary,
        secondary,
        support,
    })
}

fn hill_heading(source: GuideSource, nearby_sources: &[GuideSource]) -> (f32, f32) {
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
        if distance > source.keepout_radius_blocks * 2.0 || distance >= nearest_distance {
            continue;
        }
        nearest = Some(*candidate);
        nearest_distance = distance;
    }

    if let Some(neighbor) = nearest {
        let delta_x = neighbor.center_x - source.center_x;
        let delta_z = neighbor.center_z - source.center_z;
        let distance = (delta_x * delta_x + delta_z * delta_z).sqrt().max(f32::EPSILON);
        return (delta_x / distance, delta_z / distance);
    }

    let angle = lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT) * std::f32::consts::TAU;
    (angle.cos(), angle.sin())
}

fn make_blob(
    source: GuideSource,
    lobe_index: usize,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
    height_blocks: f32,
) -> ResolvedHillBlob {
    let lobe = MacroLobeDescriptor {
        center_x,
        center_z,
        heading_x,
        heading_z,
        radius_x_blocks,
        radius_z_blocks,
        height_blocks,
    };
    let (half_extent_x, half_extent_z) = blob_half_extents(lobe);

    ResolvedHillBlob {
        lobe,
        source,
        lobe_index,
        core_scale: (0.58 + source.cell.hilliness * 0.22).clamp(0.0, 0.92),
        peak_exponent: lerp_f32(
            1.10,
            1.56,
            indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SUMMIT_PROFILE_SALT),
        ),
        raise_cap_blocks: (height_blocks
            * lerp_f32(
                1.14,
                1.58,
                indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SUMMIT_CAP_SALT),
            ))
        .clamp(16.0, 42.0),
        half_extent_x,
        half_extent_z,
    }
}

fn resolve_secondary_blob(
    source: GuideSource,
    heading_x: f32,
    heading_z: f32,
    normal_x: f32,
    normal_z: f32,
    primary: MacroLobeDescriptor,
) -> Option<ResolvedHillBlob> {
    if lobe_hash01(source.coord, source.cell, SOURCE_COUNT_SALT) < 0.42 {
        return None;
    }

    let along_sign = if lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT) >= 0.5 {
        1.0
    } else {
        -1.0
    };
    let offset_along = along_sign
        * lerp_f32(
            primary.radius_x_blocks * 0.14,
            primary.radius_x_blocks * 0.28,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT ^ 0x55AA),
        );
    let offset_across = lerp_f32(
        -primary.radius_z_blocks * 0.18,
        primary.radius_z_blocks * 0.18,
        lobe_hash01(source.coord, source.cell, SOURCE_ENVELOPE_SHOULDER_SALT),
    );
    let center_x = source.center_x + heading_x * offset_along + normal_x * offset_across;
    let center_z = source.center_z + heading_z * offset_along + normal_z * offset_across;
    let radius_x_blocks = primary.radius_x_blocks
        * lerp_f32(
            0.72,
            0.92,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT ^ 0xAA55),
        );
    let radius_z_blocks = primary.radius_z_blocks
        * lerp_f32(
            0.78,
            0.98,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT ^ 0xAA55),
        );
    let height_blocks = primary.height_blocks
        * lerp_f32(
            0.38,
            0.62,
            lobe_hash01(source.coord, source.cell, SOURCE_SUMMIT_HEIGHT_SALT ^ 0xAA55),
        );

    Some(make_blob(
        source,
        1,
        center_x,
        center_z,
        heading_x,
        heading_z,
        radius_x_blocks,
        radius_z_blocks,
        height_blocks,
    ))
}

fn resolve_support(
    source: GuideSource,
    heading_x: f32,
    heading_z: f32,
    primary: MacroLobeDescriptor,
    secondary: Option<MacroLobeDescriptor>,
) -> ResolvedHillSupport {
    let (center_x, center_z) = if let Some(secondary) = secondary {
        (
            (primary.center_x + secondary.center_x) * 0.5,
            (primary.center_z + secondary.center_z) * 0.5,
        )
    } else {
        (primary.center_x, primary.center_z)
    };
    let delta_x = secondary.map(|blob| (blob.center_x - primary.center_x).abs()).unwrap_or(0.0);
    let delta_z = secondary.map(|blob| (blob.center_z - primary.center_z).abs()).unwrap_or(0.0);
    let radius_x_blocks = primary.radius_x_blocks.max(
        secondary.map(|blob| blob.radius_x_blocks).unwrap_or(primary.radius_x_blocks),
    ) * 1.42
        + delta_x * 0.42
        + 14.0;
    let radius_z_blocks = primary.radius_z_blocks.max(
        secondary.map(|blob| blob.radius_z_blocks).unwrap_or(primary.radius_z_blocks),
    ) * 1.48
        + delta_z * 0.50
        + 16.0;
    let support_height_blocks = primary
        .height_blocks
        .max(secondary.map(|blob| blob.height_blocks).unwrap_or(0.0))
        * lerp_f32(
            0.06,
            0.11,
            lobe_hash01(source.coord, source.cell, SOURCE_ENVELOPE_FILL_SALT),
        );
    let (half_extent_x, half_extent_z) =
        rotated_ellipse_half_extents(heading_x, heading_z, radius_x_blocks, radius_z_blocks);

    ResolvedHillSupport {
        source,
        center_x,
        center_z,
        heading_x,
        heading_z,
        radius_x_blocks,
        radius_z_blocks,
        support_height_blocks,
        half_extent_x: half_extent_x + SUPPORT_BOUNDS_PAD_BLOCKS,
        half_extent_z: half_extent_z + SUPPORT_BOUNDS_PAD_BLOCKS,
    }
}

pub(crate) fn sample_apply_signal_from_window(
    window: &HillClusterWindow,
    world_x: i32,
    world_z: i32,
) -> HillClusterApplySample {
    let sample_x = world_x as f32 + 0.5;
    let sample_z = world_z as f32 + 0.5;
    let mut strongest = 0.0_f32;
    let mut second = 0.0_f32;
    let mut third = 0.0_f32;
    let mut coverage = 0.0_f32;
    let mut shoulder_coverage = 0.0_f32;
    let mut support_height_blocks = 0.0_f32;
    let mut raise_cap_weighted_sum = 0.0_f32;
    let mut raise_cap_weight = 0.0_f32;

    for hill in &window.hills {
        if sample_x < hill.min_x || sample_x > hill.max_x || sample_z < hill.min_z || sample_z > hill.max_z {
            continue;
        }

        let mut hill_strongest = 0.0_f32;
        let mut hill_second = 0.0_f32;
        let mut hill_third = 0.0_f32;
        let mut hill_coverage = 0.0_f32;
        let mut hill_cap_weighted_sum = 0.0_f32;
        let mut hill_cap_weight = 0.0_f32;

        for blob in [Some(hill.primary), hill.secondary].into_iter().flatten() {
            if (sample_x - blob.lobe.center_x).abs() > blob.half_extent_x
                || (sample_z - blob.lobe.center_z).abs() > blob.half_extent_z
            {
                continue;
            }

            let footprint =
                irregular_lobe_footprint(blob.lobe, blob.source, blob.lobe_index, sample_x, sample_z);
            if footprint <= 0.0 {
                continue;
            }

            let contribution = blob.lobe.height_blocks * summit_profile(footprint, blob.peak_exponent);
            insert_top3(
                contribution,
                &mut hill_strongest,
                &mut hill_second,
                &mut hill_third,
            );
            hill_coverage = coverage_union(
                hill_coverage,
                (footprint * blob.core_scale).clamp(0.0, 1.0),
            );
            let cap_weight = contribution.max(footprint * blob.lobe.height_blocks * 0.12);
            hill_cap_weighted_sum += blob.raise_cap_blocks * cap_weight;
            hill_cap_weight += cap_weight;
        }

        let support_footprint = if (sample_x - hill.support.center_x).abs() <= hill.support.half_extent_x
            && (sample_z - hill.support.center_z).abs() <= hill.support.half_extent_z
        {
            support_footprint(hill.support, sample_x, sample_z)
        } else {
            0.0
        };
        let hill_support_height = hill.support.support_height_blocks
            * support_footprint
            * (1.0 - smoothstep_range(0.12, 0.54, hill_coverage));
        let hill_shoulder_coverage = coverage_union(
            hill_coverage * 0.72,
            (support_footprint * 0.58).clamp(0.0, 0.82),
        );
        let hill_core = hill_strongest + hill_second * 0.24 + hill_third * 0.08;
        if hill_core <= f32::EPSILON && hill_support_height <= f32::EPSILON && hill_shoulder_coverage <= f32::EPSILON {
            continue;
        }

        if hill_core > f32::EPSILON {
            insert_top3(hill_core, &mut strongest, &mut second, &mut third);
        }
        coverage = coverage_union(coverage, hill_coverage);
        shoulder_coverage = coverage_union(shoulder_coverage, hill_shoulder_coverage.max(coverage));
        support_height_blocks = support_height_blocks.max(hill_support_height);

        if hill_cap_weight > f32::EPSILON {
            let hill_raise_cap = hill_cap_weighted_sum / hill_cap_weight;
            raise_cap_weighted_sum += hill_raise_cap * hill_core.max(hill_coverage * hill.peak_height_hint * 0.10);
            raise_cap_weight += hill_core.max(hill_coverage * hill.peak_height_hint * 0.10);
        } else if hill_core > f32::EPSILON {
            raise_cap_weighted_sum += hill.raise_cap_blocks * hill_core;
            raise_cap_weight += hill_core;
        }
    }

    if strongest <= f32::EPSILON && support_height_blocks <= f32::EPSILON {
        return HillClusterApplySample::default();
    }

    HillClusterApplySample {
        coverage: coverage.clamp(0.0, 1.0),
        shoulder_coverage: shoulder_coverage.max(coverage).clamp(0.0, 1.0),
        lobe_height_blocks: strongest + second * 0.18 + third * 0.08,
        support_height_blocks,
        peak_raise_cap_blocks: if raise_cap_weight > f32::EPSILON {
            raise_cap_weighted_sum / raise_cap_weight
        } else {
            0.0
        },
    }
}

pub(crate) fn sample_surface_from_window(
    window: &HillClusterWindow,
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> HillClusterSurfaceSample {
    let apply = sample_apply_signal_from_window(window, world_x, world_z);
    let coverage = apply.coverage.clamp(0.0, 1.0);
    let shoulder = apply.shoulder_coverage.max(coverage).clamp(0.0, 1.0);
    let support = apply.support_height_blocks.max(0.0);
    if coverage <= f32::EPSILON && support <= f32::EPSILON {
        return HillClusterSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let core_presence = smoothstep_range(0.08, 0.84, coverage);
    let support_presence =
        smoothstep_range(0.10, 0.90, shoulder) * (1.0 - smoothstep_range(0.06, 0.52, coverage));
    let guide_raise = meso.hill_height
        * (0.10 + meso.hilliness * 0.06)
        * support_presence;
    let support_raise = support * (0.94 + meso.hilliness * 0.12) * support_presence;
    let core_raise = apply.lobe_height_blocks
        * (1.28 + meso.hilliness * 0.24 + coverage * 0.14)
        * core_presence;
    let raw_target_raise = guide_raise + support_raise + core_raise;
    let raise_cap = apply
        .peak_raise_cap_blocks
        .max((relief_budget * 1.18).max(16.0))
        .min((relief_budget * 2.90).max(44.0));
    let target_raise = soft_cap_positive(raw_target_raise, raise_cap);
    if target_raise <= f32::EPSILON {
        return HillClusterSurfaceSample::flat(base_surface_y);
    }

    let blend_weight = smoothstep01(
        (coverage * 0.82 + shoulder * 0.14 + core_presence * 0.16).clamp(0.0, 1.0),
    );

    HillClusterSurfaceSample {
        target_surface_y: base_surface_y + target_raise,
        blend_weight,
        relief_spend: target_raise * blend_weight,
        core_coverage: coverage,
        shoulder_coverage: shoulder,
    }
}

fn support_footprint(support: ResolvedHillSupport, sample_x: f32, sample_z: f32) -> f32 {
    let delta_x = sample_x - support.center_x;
    let delta_z = sample_z - support.center_z;
    let along = delta_x * support.heading_x + delta_z * support.heading_z;
    let across = delta_x * -support.heading_z + delta_z * support.heading_x;
    let normalized_along = along / support.radius_x_blocks.max(f32::EPSILON);
    let normalized_across = across / support.radius_z_blocks.max(f32::EPSILON);
    let contour_angle = normalized_across.atan2(normalized_along);
    let contour_scale = 1.0
        + (contour_angle * 2.0
            + lobe_hash01(support.source.coord, support.source.cell, SOURCE_ENVELOPE_RADIUS_SALT)
                * std::f32::consts::TAU)
            .sin()
            * 0.07
        + (contour_angle * 3.0
            + lobe_hash01(support.source.coord, support.source.cell, SOURCE_ENVELOPE_SHOULDER_SALT)
                * std::f32::consts::TAU)
            .cos()
            * 0.05;
    let radial = ((normalized_along * normalized_along) + (normalized_across * normalized_across)).sqrt()
        / contour_scale.max(0.82);
    let outer = smoothstep_range(1.10, 0.0, radial);
    let inner = smoothstep_range(0.80, 0.0, radial);
    (outer * 0.62 + inner * 0.38).clamp(0.0, 1.0)
}

fn coverage_union(a: f32, b: f32) -> f32 {
    (a + b - a * b).clamp(0.0, 1.0)
}

fn summit_profile(footprint: f32, exponent: f32) -> f32 {
    if footprint <= f32::EPSILON {
        return 0.0;
    }

    let exponent = exponent.max(1.0);
    let t = footprint.clamp(0.0, 1.0);
    let base = smoothstep01(t);
    let dome = smoothstep01(base).powf(1.06 + (exponent - 1.0) * 0.52);
    let apex = smoothstep_range(0.78, 1.0, t);
    (base.powf(0.96) * 0.34 + dome * 0.48 + apex * 0.18).clamp(0.0, 1.0)
}

fn blob_half_extents(lobe: MacroLobeDescriptor) -> (f32, f32) {
    let radius_x = lobe.radius_x_blocks * 1.18 + HILL_BOUNDS_PAD_BLOCKS;
    let radius_z = lobe.radius_z_blocks * 1.22 + HILL_BOUNDS_PAD_BLOCKS;
    rotated_ellipse_half_extents(lobe.heading_x, lobe.heading_z, radius_x, radius_z)
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

fn include_bounds(
    min_x: &mut f32,
    max_x: &mut f32,
    min_z: &mut f32,
    max_z: &mut f32,
    center_x: f32,
    center_z: f32,
    half_extent_x: f32,
    half_extent_z: f32,
) {
    *min_x = (*min_x).min(center_x - half_extent_x);
    *max_x = (*max_x).max(center_x + half_extent_x);
    *min_z = (*min_z).min(center_z - half_extent_z);
    *max_z = (*max_z).max(center_z + half_extent_z);
}

fn chunk_bounds(chunk: ChunkCoord, padding_blocks: f32) -> (f32, f32, f32, f32) {
    let min_x = (chunk.0 * crate::world::CHUNK_EDGE_I32) as f32 - padding_blocks;
    let max_x = ((chunk.0 + 1) * crate::world::CHUNK_EDGE_I32) as f32 + padding_blocks;
    let min_z = (chunk.2 * crate::world::CHUNK_EDGE_I32) as f32 - padding_blocks;
    let max_z = ((chunk.2 + 1) * crate::world::CHUNK_EDGE_I32) as f32 + padding_blocks;
    (min_x, max_x, min_z, max_z)
}

fn owner_region_range_for_chunk(
    chunk: ChunkCoord,
    padding_regions: i32,
) -> (i32, i32, i32, i32) {
    let region_span_blocks = crate::world::CHUNK_EDGE_I32 * crate::world::ATLAS_CELL_SIZE_IN_CHUNKS as i32;
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

#[cfg(test)]
mod tests {
    use super::{
        GuideSource, HillClusterWindow, ResolvedHill, ResolvedHillSupport, coverage_union,
        sample_apply_signal_from_window, sample_surface_from_window, summit_profile,
    };
    use crate::world::atlas::{
        AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap,
    };
    use crate::world::coord::ChunkCoord;

    #[test]
    fn summit_profile_keeps_sigmoid_upper_dome() {
        let lower_mid = summit_profile(0.60, 1.6);
        let upper_mid = summit_profile(0.85, 1.6);
        let apex_band = summit_profile(0.97, 1.6);

        assert!(
            upper_mid - lower_mid >= 0.22,
            "expected hill sides to steepen through the mid band, lower_mid={lower_mid:.3}, upper_mid={upper_mid:.3}"
        );
        assert!(
            apex_band - upper_mid >= 0.08,
            "expected summit profile to keep gaining into the apex, upper_mid={upper_mid:.3}, apex_band={apex_band:.3}"
        );
    }

    #[test]
    fn envelope_only_support_does_not_promote_into_core_height() {
        let source = GuideSource {
            coord: AtlasCoord::new(1, 1),
            cell: MesoGuideCell {
                hilliness: 0.88,
                hill_height: 8.0,
                ..MesoGuideCell::default()
            },
            center_x: 64.0,
            center_z: 64.0,
            weight: 1.0,
            keepout_radius_blocks: 36.0,
        };
        let hill = ResolvedHill {
            peak_height_hint: 16.0,
            raise_cap_blocks: 24.0,
            min_x: 18.0,
            max_x: 110.0,
            min_z: 20.0,
            max_z: 108.0,
            primary: super::make_blob(source, 0, 64.0, 64.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            secondary: None,
            support: ResolvedHillSupport {
                source,
                center_x: 64.0,
                center_z: 64.0,
                heading_x: 1.0,
                heading_z: 0.0,
                radius_x_blocks: 28.0,
                radius_z_blocks: 24.0,
                support_height_blocks: 4.0,
                half_extent_x: 40.0,
                half_extent_z: 36.0,
            },
        };
        let mut hill = hill;
        hill.primary.core_scale = 0.0;
        hill.primary.lobe.height_blocks = 0.0;
        hill.primary.raise_cap_blocks = 0.0;
        let window = HillClusterWindow {
            chunk: ChunkCoord(0, 0, 0),
            hills: vec![hill],
        };
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 3, 3).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = source.cell;
        let guides = MesoGuideMap { area, cells };

        let apply = sample_apply_signal_from_window(&window, 64, 64);
        assert!(apply.support_height_blocks > 0.0, "expected support-only hill to emit support, got {apply:?}");
        assert!(apply.lobe_height_blocks <= f32::EPSILON, "expected support-only hill to avoid core height, got {apply:?}");

        let surface = sample_surface_from_window(&window, &guides, 64, 64, 100.0, 24.0);
        assert!(surface.target_surface_y > 100.0, "expected support-only hill to still raise foothills, got {surface:?}");
        assert!(surface.target_surface_y < 105.0, "expected support-only hill to stay below summit-like uplift, got {surface:?}");
    }

    #[test]
    fn coverage_union_stays_bounded() {
        let value = coverage_union(0.62, 0.58);
        assert!(value <= 1.0 && value >= 0.0);
    }
}
