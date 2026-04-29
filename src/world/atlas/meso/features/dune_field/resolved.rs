use std::f32::consts::TAU;

use crate::world::atlas::{AtlasCoord, MesoGuideMap, MesoRegionCoord, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::{
    DuneFieldApplySample, DuneFieldSurfaceSample, GuideSource, collect_peak_sources, dune_hash01,
    indexed_dune_hash01, lerp_f32, normalize_vec2_or_fallback,
};

const WINDOW_OWNER_PADDING_REGIONS: i32 = 1;
const REGION_RESOLVE_PADDING_REGIONS: i32 = 1;
const MAX_RESOLVED_FIELDS_PER_REGION: usize = 4;
const OWNER_KEEP_DISTANCE_MULTIPLIER: f32 = 1.10;
const RIDGE_BOUNDS_PAD_BLOCKS: f32 = 8.0;
const SUPPORT_BOUNDS_PAD_BLOCKS: f32 = 6.0;
const RIDGE_LENGTH_SALT: u64 = 0xD811_B6D2_2800_0101;
const RIDGE_WINDWARD_SALT: u64 = 0xD811_B6D2_2800_0102;
const RIDGE_LEE_SALT: u64 = 0xD811_B6D2_2800_0103;
const RIDGE_HEIGHT_SALT: u64 = 0xD811_B6D2_2800_0104;
const RIDGE_SHIFT_SALT: u64 = 0xD811_B6D2_2800_0105;
const RIDGE_SHIFT_PHASE_SALT: u64 = 0xD811_B6D2_2800_0106;
const RIDGE_CENTER_ALONG_SALT: u64 = 0xD811_B6D2_2800_0107;
const RIDGE_CENTER_ACROSS_SALT: u64 = 0xD811_B6D2_2800_0108;
const SUPPORT_HEIGHT_SALT: u64 = 0xD811_B6D2_2800_0109;
const SUPPORT_RADIUS_SALT: u64 = 0xD811_B6D2_2800_010A;
const SUPPORT_STRETCH_SALT: u64 = 0xD811_B6D2_2800_010B;
const RAISE_CAP_SALT: u64 = 0xD811_B6D2_2800_010C;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DuneFieldWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    fields: Vec<ResolvedDuneField>,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedDuneField {
    crest_height_hint: f32,
    raise_cap_blocks: f32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    ridges: Vec<ResolvedDuneRidge>,
    support: ResolvedDuneSupport,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedDuneRidge {
    source: GuideSource,
    ridge_index: usize,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    half_length_blocks: f32,
    windward_width_blocks: f32,
    lee_width_blocks: f32,
    crest_half_width_blocks: f32,
    crest_height_blocks: f32,
    crest_shift_blocks: f32,
    crest_shift_phase: f32,
    half_extent_x: f32,
    half_extent_z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedDuneSupport {
    source: GuideSource,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
    support_raise_blocks: f32,
    half_extent_x: f32,
    half_extent_z: f32,
}

pub(crate) fn build_dune_field_window(guides: &MesoGuideMap, chunk: ChunkCoord) -> DuneFieldWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let chunk_bounds = chunk_bounds(chunk, 0.0);
    let candidates = collect_peak_sources(guides, meso_span_blocks);
    let (min_region_x, max_region_x, min_region_z, max_region_z) =
        owner_region_range_for_chunk(chunk, WINDOW_OWNER_PADDING_REGIONS);
    let mut fields = Vec::new();

    for region_z in min_region_z..=max_region_z {
        for region_x in min_region_x..=max_region_x {
            fields.extend(resolve_owned_fields(
                &candidates,
                MesoRegionCoord::new(region_x, region_z),
            ));
        }
    }

    fields.retain(|field| {
        bounds_overlap(
            chunk_bounds,
            field.min_x,
            field.max_x,
            field.min_z,
            field.max_z,
        )
    });

    DuneFieldWindow { chunk, fields }
}

fn resolve_owned_fields(
    candidates: &[GuideSource],
    owner_region: MesoRegionCoord,
) -> Vec<ResolvedDuneField> {
    let search_bounds = guide_region_bounds(owner_region, REGION_RESOLVE_PADDING_REGIONS);
    let local_candidates = candidates
        .iter()
        .copied()
        .filter(|candidate| bounds_contains_coord(search_bounds, candidate.coord))
        .collect::<Vec<_>>();
    let mut kept_sources = Vec::new();
    let mut fields = Vec::new();

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
                * OWNER_KEEP_DISTANCE_MULTIPLIER
        });
        if overlaps {
            continue;
        }

        if let Some(field) = resolve_dune_field(candidate, &local_candidates) {
            kept_sources.push(candidate);
            fields.push(field);
        }

        if fields.len() >= MAX_RESOLVED_FIELDS_PER_REGION {
            break;
        }
    }

    fields
}

fn resolve_dune_field(
    source: GuideSource,
    nearby_sources: &[GuideSource],
) -> Option<ResolvedDuneField> {
    let heading = resolve_field_heading(source, nearby_sources);
    let heading_x = heading.0;
    let heading_z = heading.1;
    let normal_x = -heading_z;
    let normal_z = heading_x;

    let mut ridges = Vec::with_capacity(source.ridge_count);
    let ridge_count = source.ridge_count.max(2);
    let center_index = (ridge_count - 1) as f32 * 0.5;
    let field_width_blocks = source.crest_spacing_blocks * (ridge_count - 1) as f32;

    for ridge_index in 0..ridge_count {
        let centered_index = ridge_index as f32 - center_index;
        let center_bias = 1.0 - (centered_index / center_index.max(1.0)).abs();
        let center_along = lerp_f32(
            -source.crest_spacing_blocks * 0.35,
            source.crest_spacing_blocks * 0.35,
            indexed_dune_hash01(
                source.coord,
                source.cell,
                ridge_index,
                RIDGE_CENTER_ALONG_SALT,
            ),
        );
        let center_across = centered_index * source.crest_spacing_blocks
            + lerp_f32(
                -source.crest_spacing_blocks * 0.18,
                source.crest_spacing_blocks * 0.18,
                indexed_dune_hash01(
                    source.coord,
                    source.cell,
                    ridge_index,
                    RIDGE_CENTER_ACROSS_SALT,
                ),
            );
        let half_length_blocks = source.field_length_blocks
            * lerp_f32(
                0.42,
                0.62,
                indexed_dune_hash01(source.coord, source.cell, ridge_index, RIDGE_LENGTH_SALT),
            )
            * (0.92 + center_bias * 0.10);
        let windward_width_blocks = source.crest_spacing_blocks
            * lerp_f32(
                0.70,
                1.04,
                indexed_dune_hash01(source.coord, source.cell, ridge_index, RIDGE_WINDWARD_SALT),
            );
        let lee_width_blocks = windward_width_blocks
            * lerp_f32(
                0.38,
                0.60,
                indexed_dune_hash01(source.coord, source.cell, ridge_index, RIDGE_LEE_SALT),
            );
        let crest_half_width_blocks = lee_width_blocks.min(windward_width_blocks)
            * lerp_f32(
                0.18,
                0.28,
                indexed_dune_hash01(
                    source.coord,
                    source.cell,
                    ridge_index,
                    RIDGE_LEE_SALT ^ 0x55AA,
                ),
            );
        let crest_height_blocks = source.crest_height_blocks
            * lerp_f32(
                0.84,
                1.18,
                indexed_dune_hash01(source.coord, source.cell, ridge_index, RIDGE_HEIGHT_SALT),
            )
            * (0.88 + center_bias * 0.16);
        let crest_shift_blocks = windward_width_blocks
            * lerp_f32(
                0.08,
                0.18,
                indexed_dune_hash01(source.coord, source.cell, ridge_index, RIDGE_SHIFT_SALT),
            );
        let crest_shift_phase = indexed_dune_hash01(
            source.coord,
            source.cell,
            ridge_index,
            RIDGE_SHIFT_PHASE_SALT,
        ) * TAU;
        let center_x = source.center_x + heading_x * center_along + normal_x * center_across;
        let center_z = source.center_z + heading_z * center_along + normal_z * center_across;
        let max_width_blocks = windward_width_blocks.max(lee_width_blocks)
            + crest_shift_blocks
            + RIDGE_BOUNDS_PAD_BLOCKS;
        let (half_extent_x, half_extent_z) = rotated_ellipse_half_extents(
            heading_x,
            heading_z,
            half_length_blocks + RIDGE_BOUNDS_PAD_BLOCKS,
            max_width_blocks,
        );

        ridges.push(ResolvedDuneRidge {
            source,
            ridge_index,
            center_x,
            center_z,
            heading_x,
            heading_z,
            half_length_blocks,
            windward_width_blocks,
            lee_width_blocks,
            crest_half_width_blocks,
            crest_height_blocks,
            crest_shift_blocks,
            crest_shift_phase,
            half_extent_x,
            half_extent_z,
        });
    }

    if ridges.is_empty() {
        return None;
    }

    let support_radius_x = source.field_length_blocks
        * lerp_f32(
            0.56,
            0.82,
            dune_hash01(source.coord, source.cell, SUPPORT_STRETCH_SALT),
        );
    let support_radius_z = (field_width_blocks + source.crest_spacing_blocks * 2.2)
        * lerp_f32(
            0.72,
            1.02,
            dune_hash01(source.coord, source.cell, SUPPORT_RADIUS_SALT),
        )
        + 16.0;
    let (support_half_extent_x, support_half_extent_z) = rotated_ellipse_half_extents(
        heading_x,
        heading_z,
        support_radius_x + SUPPORT_BOUNDS_PAD_BLOCKS,
        support_radius_z + SUPPORT_BOUNDS_PAD_BLOCKS,
    );
    let support = ResolvedDuneSupport {
        source,
        center_x: source.center_x,
        center_z: source.center_z,
        heading_x,
        heading_z,
        radius_x_blocks: support_radius_x,
        radius_z_blocks: support_radius_z,
        support_raise_blocks: source.crest_height_blocks
            * lerp_f32(
                0.08,
                0.16,
                dune_hash01(source.coord, source.cell, SUPPORT_HEIGHT_SALT),
            ),
        half_extent_x: support_half_extent_x,
        half_extent_z: support_half_extent_z,
    };

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    let mut crest_height_hint = 0.0_f32;

    for ridge in &ridges {
        crest_height_hint = crest_height_hint.max(ridge.crest_height_blocks);
        include_bounds(
            &mut min_x,
            &mut max_x,
            &mut min_z,
            &mut max_z,
            ridge.center_x,
            ridge.center_z,
            ridge.half_extent_x,
            ridge.half_extent_z,
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

    Some(ResolvedDuneField {
        crest_height_hint,
        raise_cap_blocks: (crest_height_hint
            * lerp_f32(
                1.24,
                1.82,
                dune_hash01(source.coord, source.cell, RAISE_CAP_SALT),
            ))
        .clamp(4.0, 16.0),
        min_x,
        max_x,
        min_z,
        max_z,
        ridges,
        support,
    })
}

fn resolve_field_heading(source: GuideSource, nearby_sources: &[GuideSource]) -> (f32, f32) {
    let mut sum_x = source.heading_x * source.weight;
    let mut sum_z = source.heading_z * source.weight;

    for candidate in nearby_sources {
        if candidate.coord == source.coord {
            continue;
        }
        let distance = distance_between_points(
            (source.center_x, source.center_z),
            (candidate.center_x, candidate.center_z),
        );
        if distance > source.keepout_radius_blocks * 2.2 {
            continue;
        }

        let dot = source.heading_x * candidate.heading_x + source.heading_z * candidate.heading_z;
        if dot.abs() < 0.35 {
            continue;
        }
        let sign = if dot >= 0.0 { 1.0 } else { -1.0 };
        let weight = candidate.weight
            * (1.0 - distance / (source.keepout_radius_blocks * 2.2)).clamp(0.0, 1.0);
        sum_x += candidate.heading_x * sign * weight;
        sum_z += candidate.heading_z * sign * weight;
    }

    normalize_vec2_or_fallback(sum_x, sum_z, (source.heading_x, source.heading_z))
}

pub(crate) fn sample_dune_field_apply_signal_from_window(
    window: &DuneFieldWindow,
    world_x: i32,
    world_z: i32,
) -> DuneFieldApplySample {
    let sample_x = world_x as f32 + 0.5;
    let sample_z = world_z as f32 + 0.5;
    let mut strongest = 0.0_f32;
    let mut second = 0.0_f32;
    let mut third = 0.0_f32;
    let mut crest_coverage = 0.0_f32;
    let mut field_coverage = 0.0_f32;
    let mut support_height_blocks = 0.0_f32;
    let mut raise_cap_weighted_sum = 0.0_f32;
    let mut raise_cap_weight = 0.0_f32;

    for field in &window.fields {
        if sample_x < field.min_x
            || sample_x > field.max_x
            || sample_z < field.min_z
            || sample_z > field.max_z
        {
            continue;
        }

        let mut field_strongest = 0.0_f32;
        let mut field_second = 0.0_f32;
        let mut field_third = 0.0_f32;
        let mut field_crest_coverage = 0.0_f32;
        let mut field_ridge_coverage = 0.0_f32;
        let mut field_cap_weighted_sum = 0.0_f32;
        let mut field_cap_weight = 0.0_f32;

        for ridge in &field.ridges {
            if (sample_x - ridge.center_x).abs() > ridge.half_extent_x
                || (sample_z - ridge.center_z).abs() > ridge.half_extent_z
            {
                continue;
            }

            let sample = ridge_sample(*ridge, sample_x, sample_z);
            if sample.field_coverage <= f32::EPSILON {
                continue;
            }

            let contribution = ridge.crest_height_blocks * crest_profile(sample.crest_coverage);
            insert_top3(
                contribution,
                &mut field_strongest,
                &mut field_second,
                &mut field_third,
            );
            field_crest_coverage = coverage_union(field_crest_coverage, sample.crest_coverage);
            field_ridge_coverage = coverage_union(field_ridge_coverage, sample.field_coverage);
            let cap_weight =
                contribution.max(sample.crest_coverage * ridge.crest_height_blocks * 0.18);
            field_cap_weighted_sum += field.raise_cap_blocks * cap_weight;
            field_cap_weight += cap_weight;
        }

        let support_coverage = if (sample_x - field.support.center_x).abs()
            <= field.support.half_extent_x
            && (sample_z - field.support.center_z).abs() <= field.support.half_extent_z
        {
            support_footprint(field.support, sample_x, sample_z)
        } else {
            0.0
        };
        let field_support_height = field.support.support_raise_blocks
            * support_coverage
            * (1.0 - smoothstep_range(0.08, 0.62, field_crest_coverage));
        let field_field_coverage = coverage_union(field_ridge_coverage, support_coverage * 0.52);
        let field_core = field_strongest + field_second * 0.24 + field_third * 0.12;
        if field_core <= f32::EPSILON
            && field_support_height <= f32::EPSILON
            && field_field_coverage <= f32::EPSILON
        {
            continue;
        }

        if field_core > f32::EPSILON {
            insert_top3(field_core, &mut strongest, &mut second, &mut third);
        }
        crest_coverage = coverage_union(crest_coverage, field_crest_coverage);
        field_coverage = coverage_union(
            field_coverage,
            field_field_coverage.max(field_crest_coverage),
        );
        support_height_blocks = support_height_blocks.max(field_support_height);

        if field_cap_weight > f32::EPSILON {
            raise_cap_weighted_sum += (field_cap_weighted_sum / field_cap_weight)
                * field_core.max(field.crest_height_hint * field_crest_coverage * 0.18);
            raise_cap_weight +=
                field_core.max(field.crest_height_hint * field_crest_coverage * 0.18);
        } else if field_core > f32::EPSILON {
            raise_cap_weighted_sum += field.raise_cap_blocks * field_core;
            raise_cap_weight += field_core;
        }
    }

    if strongest <= f32::EPSILON && support_height_blocks <= f32::EPSILON {
        return DuneFieldApplySample::default();
    }

    DuneFieldApplySample {
        crest_coverage: crest_coverage.clamp(0.0, 1.0),
        field_coverage: field_coverage.max(crest_coverage).clamp(0.0, 1.0),
        crest_height_blocks: strongest + second * 0.18 + third * 0.08,
        support_height_blocks,
        raise_cap_blocks: if raise_cap_weight > f32::EPSILON {
            raise_cap_weighted_sum / raise_cap_weight
        } else {
            0.0
        },
    }
}

pub(crate) fn sample_dune_field_surface_from_window(
    window: &DuneFieldWindow,
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> DuneFieldSurfaceSample {
    let apply = sample_dune_field_apply_signal_from_window(window, world_x, world_z);
    let crest = apply.crest_coverage.clamp(0.0, 1.0);
    let field = apply.field_coverage.max(crest).clamp(0.0, 1.0);
    let support = apply.support_height_blocks.max(0.0);
    if crest <= f32::EPSILON && support <= f32::EPSILON {
        return DuneFieldSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let crest_presence = smoothstep_range(0.04, 0.82, crest);
    let field_presence = smoothstep_range(0.08, 0.92, field);
    let support_presence = field_presence * (1.0 - smoothstep_range(0.06, 0.54, crest));
    let guide_raise =
        meso.terrace_step_height * (0.08 + meso.terrace_weight * 0.08) * field_presence;
    let support_raise = support * (0.94 + meso.terrace_weight * 0.12) * support_presence;
    let crest_raise = apply.crest_height_blocks
        * (1.08 + meso.terrace_weight * 0.22 + crest_presence * 0.12)
        * crest_presence;
    let raw_target_raise = guide_raise + support_raise + crest_raise;
    let raise_cap = apply
        .raise_cap_blocks
        .max((relief_budget * 1.06).max(4.0))
        .min((relief_budget * 1.85).max(16.0));
    let target_raise = soft_cap_positive(raw_target_raise, raise_cap);
    if target_raise <= f32::EPSILON {
        return DuneFieldSurfaceSample::flat(base_surface_y);
    }

    let blend_weight =
        smoothstep01((field * 0.58 + crest * 0.28 + crest_presence * 0.14).clamp(0.0, 1.0));

    DuneFieldSurfaceSample {
        target_surface_y: base_surface_y + target_raise,
        blend_weight,
        relief_spend: target_raise * blend_weight,
        crest_coverage: crest,
        field_coverage: field,
    }
}

#[derive(Debug, Clone, Copy)]
struct RidgeSample {
    field_coverage: f32,
    crest_coverage: f32,
}

fn ridge_sample(ridge: ResolvedDuneRidge, sample_x: f32, sample_z: f32) -> RidgeSample {
    let delta_x = sample_x - ridge.center_x;
    let delta_z = sample_z - ridge.center_z;
    let along = delta_x * ridge.heading_x + delta_z * ridge.heading_z;
    let across = delta_x * -ridge.heading_z + delta_z * ridge.heading_x;
    let along_phase = along / ridge.half_length_blocks.max(f32::EPSILON);
    let crest_shift =
        ((along_phase * TAU * 0.5) + ridge.crest_shift_phase).sin() * ridge.crest_shift_blocks;
    let shifted_across = across - crest_shift;
    let along_mask = smoothstep_range(
        1.06,
        0.0,
        along.abs() / ridge.half_length_blocks.max(f32::EPSILON),
    );
    let field_side = if shifted_across <= 0.0 {
        smoothstep_range(
            1.04,
            0.0,
            (-shifted_across) / ridge.windward_width_blocks.max(f32::EPSILON),
        )
        .powf(0.88)
    } else {
        smoothstep_range(
            1.00,
            0.0,
            shifted_across / ridge.lee_width_blocks.max(f32::EPSILON),
        )
        .powf(1.28)
    };
    let crest_mask = smoothstep_range(
        1.0,
        0.0,
        shifted_across.abs() / ridge.crest_half_width_blocks.max(f32::EPSILON),
    );

    RidgeSample {
        field_coverage: (along_mask * field_side).clamp(0.0, 1.0),
        crest_coverage: (along_mask * smoothstep01(crest_mask)).clamp(0.0, 1.0),
    }
}

fn support_footprint(support: ResolvedDuneSupport, sample_x: f32, sample_z: f32) -> f32 {
    let delta_x = sample_x - support.center_x;
    let delta_z = sample_z - support.center_z;
    let along = delta_x * support.heading_x + delta_z * support.heading_z;
    let across = delta_x * -support.heading_z + delta_z * support.heading_x;
    let normalized_along = along / support.radius_x_blocks.max(f32::EPSILON);
    let normalized_across = across / support.radius_z_blocks.max(f32::EPSILON);
    let contour_angle = normalized_across.atan2(normalized_along);
    let contour_scale = 1.0
        + (contour_angle * 2.0
            + dune_hash01(
                support.source.coord,
                support.source.cell,
                SUPPORT_RADIUS_SALT,
            ) * TAU)
            .sin()
            * 0.06
        + (contour_angle * 3.0
            + dune_hash01(
                support.source.coord,
                support.source.cell,
                SUPPORT_STRETCH_SALT,
            ) * TAU)
            .cos()
            * 0.04;
    let radial = ((normalized_along * normalized_along) + (normalized_across * normalized_across))
        .sqrt()
        / contour_scale.max(0.84);
    let outer = smoothstep_range(1.08, 0.0, radial);
    let inner = smoothstep_range(0.78, 0.0, radial);
    (outer * 0.58 + inner * 0.42).clamp(0.0, 1.0)
}

fn crest_profile(coverage: f32) -> f32 {
    if coverage <= f32::EPSILON {
        return 0.0;
    }

    let t = coverage.clamp(0.0, 1.0);
    let base = smoothstep01(t);
    let shoulder = smoothstep01(base).powf(1.04);
    let crest = smoothstep_range(0.58, 1.0, t).powf(1.18);
    (base * 0.30 + shoulder * 0.42 + crest * 0.28).clamp(0.0, 1.0)
}

fn coverage_union(a: f32, b: f32) -> f32 {
    (a + b - a * b).clamp(0.0, 1.0)
}

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn soft_cap_positive(value: f32, cap: f32) -> f32 {
    if value <= 0.0 || cap <= f32::EPSILON {
        0.0
    } else {
        cap * (1.0 - (-value / cap).exp())
    }
}

fn insert_top3(value: f32, strongest: &mut f32, second: &mut f32, third: &mut f32) {
    if value > *strongest {
        *third = *second;
        *second = *strongest;
        *strongest = value;
    } else if value > *second {
        *third = *second;
        *second = value;
    } else if value > *third {
        *third = value;
    }
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
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

#[cfg(test)]
mod tests {
    use super::{
        ResolvedDuneRidge, build_dune_field_window, ridge_sample,
        sample_dune_field_apply_signal_from_window, sample_dune_field_surface_from_window,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::coord::ChunkCoord;
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    fn dune_guides(specs: &[(AtlasCoord, MesoGuideCell)]) -> MesoGuideMap {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 10, 10).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        for (coord, cell) in specs {
            *cells.get_mut(*coord).unwrap() = *cell;
        }
        MesoGuideMap { area, cells }
    }

    fn strong_dune_cell() -> MesoGuideCell {
        MesoGuideCell {
            terrace_weight: 0.94,
            terrace_step_height: 2.8,
            terrace_spacing_cells: 0.92,
            terrace_heading_x: 1.0,
            terrace_heading_z: 0.0,
            ..MesoGuideCell::default()
        }
    }

    #[test]
    fn build_window_is_deterministic_for_same_guides_and_chunk() {
        let guides = dune_guides(&[(AtlasCoord::new(3, 3), strong_dune_cell())]);
        let first = build_dune_field_window(&guides, ChunkCoord(1, 0, 1));
        let second = build_dune_field_window(&guides, ChunkCoord(1, 0, 1));

        assert_eq!(first, second);
    }

    #[test]
    fn neighboring_chunk_windows_sample_the_same_world_space_surface() {
        let guides = dune_guides(&[(AtlasCoord::new(3, 3), strong_dune_cell())]);
        let left = build_dune_field_window(&guides, ChunkCoord(0, 0, 0));
        let right = build_dune_field_window(&guides, ChunkCoord(1, 0, 0));
        let sample_world_x = CHUNK_EDGE_I32 - 1;
        let sample_world_z = CHUNK_EDGE_I32 * 2;
        let left_sample = sample_dune_field_surface_from_window(
            &left,
            &guides,
            sample_world_x,
            sample_world_z,
            100.0,
            8.0,
        );
        let right_sample = sample_dune_field_surface_from_window(
            &right,
            &guides,
            sample_world_x,
            sample_world_z,
            100.0,
            8.0,
        );

        assert_eq!(
            left_sample, right_sample,
            "expected neighboring chunk windows to reuse the same resolved dune objects at the same world point"
        );
    }

    #[test]
    fn strong_field_resolves_multiple_crest_peaks_across_the_crosswind_axis() {
        let guides = dune_guides(&[(AtlasCoord::new(3, 3), strong_dune_cell())]);
        let window = build_dune_field_window(&guides, ChunkCoord(4, 0, 4));
        let meso_span = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let base_world_x = meso_span * 3 + meso_span / 2;
        let base_world_z = meso_span * 3 + meso_span / 2;
        let mut local_maxima = 0;
        let mut previous = 0.0_f32;
        let mut rising = false;

        for offset in (-96..=96).step_by(4) {
            let sample = sample_dune_field_apply_signal_from_window(
                &window,
                base_world_x,
                base_world_z + offset,
            );
            if sample.crest_height_blocks > previous {
                rising = true;
            } else if rising && sample.crest_height_blocks < previous && previous >= 0.9 {
                local_maxima += 1;
                rising = false;
            }
            previous = sample.crest_height_blocks;
        }

        assert!(
            local_maxima >= 2,
            "expected a strong dune field to resolve multiple crest peaks instead of one mesa-like hump, found {local_maxima}"
        );
    }

    #[test]
    fn sample_surface_keeps_material_visible_uplift() {
        let guides = dune_guides(&[(AtlasCoord::new(4, 4), strong_dune_cell())]);
        let meso_span = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let window = build_dune_field_window(&guides, ChunkCoord(8, 0, 8));
        let mut strongest = super::DuneFieldSurfaceSample::flat(100.0);

        for offset_z in (-48..=48).step_by(4) {
            for offset_x in (-48..=48).step_by(4) {
                let sample = sample_dune_field_surface_from_window(
                    &window,
                    &guides,
                    meso_span * 4 + meso_span / 2 + offset_x,
                    meso_span * 4 + meso_span / 2 + offset_z,
                    100.0,
                    8.0,
                );
                if sample.target_surface_y > strongest.target_surface_y {
                    strongest = sample;
                }
            }
        }

        assert!(
            strongest.target_surface_y - 100.0 >= 2.0,
            "expected resolved dune crests to keep a materially visible uplift, got {strongest:?}"
        );
        assert!(
            strongest.blend_weight >= 0.25,
            "expected dune crest uplift to keep meaningful blend support, got {strongest:?}"
        );
    }

    #[test]
    fn windward_side_stays_broader_than_lee_side() {
        let ridge = ResolvedDuneRidge {
            source: super::GuideSource {
                coord: AtlasCoord::new(0, 0),
                cell: strong_dune_cell(),
                center_x: 0.0,
                center_z: 0.0,
                heading_x: 1.0,
                heading_z: 0.0,
                weight: 1.0,
                crest_spacing_blocks: 24.0,
                field_length_blocks: 96.0,
                crest_height_blocks: 3.6,
                ridge_count: 4,
                keepout_radius_blocks: 64.0,
            },
            ridge_index: 0,
            center_x: 0.0,
            center_z: 0.0,
            heading_x: 1.0,
            heading_z: 0.0,
            half_length_blocks: 60.0,
            windward_width_blocks: 18.0,
            lee_width_blocks: 8.0,
            crest_half_width_blocks: 3.0,
            crest_height_blocks: 3.6,
            crest_shift_blocks: 0.0,
            crest_shift_phase: 0.0,
            half_extent_x: 64.0,
            half_extent_z: 28.0,
        };

        let windward = ridge_sample(ridge, 0.0, -10.0);
        let lee = ridge_sample(ridge, 0.0, 10.0);

        assert!(
            windward.field_coverage > lee.field_coverage,
            "expected windward dune slope to stay broader than lee slope, windward={windward:?}, lee={lee:?}"
        );
    }
}
