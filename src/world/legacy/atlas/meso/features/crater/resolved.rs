use crate::world::atlas::{AtlasCoord, MesoGuideMap, MesoRegionCoord, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::{
    CraterApplySample, CraterSurfaceSample, GuideSource, SOURCE_APRON_SALT, SOURCE_BOWL_MAJOR_SALT,
    SOURCE_BOWL_MINOR_SALT, SOURCE_CONTOUR_PRIMARY_SALT, SOURCE_CONTOUR_SECONDARY_SALT,
    SOURCE_DEPTH_SALT, SOURCE_FLOOR_SALT, SOURCE_RIM_OUTER_SALT, collect_crater_sources,
    coverage_union, crater_hash01, distance_between_points, fallback_axis_from_source,
    indexed_crater_hash01, insert_top2, lerp_f32, smoothstep_range, smoothstep01, soft_cap_signed,
};

const WINDOW_OWNER_PADDING_REGIONS: i32 = 1;
const REGION_RESOLVE_PADDING_REGIONS: i32 = 1;
const MAX_RESOLVED_CRATERS_PER_REGION: usize = 4;
const OWNER_KEEP_DISTANCE_MULTIPLIER: f32 = 1.12;
const CRATER_BOUNDS_PAD_BLOCKS: f32 = 10.0;

#[derive(Debug, Clone)]
pub(crate) struct CraterWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    craters: Vec<ResolvedCrater>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedCrater {
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    bowl_radius_x_blocks: f32,
    bowl_radius_z_blocks: f32,
    floor_radius_scale: f32,
    bowl_depth_blocks: f32,
    rim_raise_blocks: f32,
    rim_outer_scale: f32,
    apron_outer_scale: f32,
    contour_primary_phase: f32,
    contour_secondary_phase: f32,
    contour_primary_amp: f32,
    contour_secondary_amp: f32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct CraterContribution {
    bowl_coverage: f32,
    rim_coverage: f32,
    apron_coverage: f32,
    bowl_depth_blocks: f32,
    rim_raise_blocks: f32,
}

pub(crate) fn build_crater_window(guides: &MesoGuideMap, chunk: ChunkCoord) -> CraterWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let chunk_bounds = chunk_bounds(chunk, 0.0);
    let candidates = collect_crater_sources(guides, meso_span_blocks);
    let (min_region_x, max_region_x, min_region_z, max_region_z) =
        owner_region_range_for_chunk(chunk, WINDOW_OWNER_PADDING_REGIONS);
    let mut craters = Vec::new();

    for region_z in min_region_z..=max_region_z {
        for region_x in min_region_x..=max_region_x {
            craters.extend(resolve_owned_craters(
                &candidates,
                MesoRegionCoord::new(region_x, region_z),
            ));
        }
    }

    craters.retain(|crater| {
        bounds_overlap(
            chunk_bounds,
            crater.min_x,
            crater.max_x,
            crater.min_z,
            crater.max_z,
        )
    });

    CraterWindow { chunk, craters }
}

fn resolve_owned_craters(
    candidates: &[GuideSource],
    owner_region: MesoRegionCoord,
) -> Vec<ResolvedCrater> {
    let search_bounds = guide_region_bounds(owner_region, REGION_RESOLVE_PADDING_REGIONS);
    let local_candidates = candidates
        .iter()
        .copied()
        .filter(|candidate| bounds_contains_coord(search_bounds, candidate.coord))
        .collect::<Vec<_>>();
    let mut kept_sources = Vec::new();
    let mut craters = Vec::new();

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

        if let Some(crater) = resolve_crater(candidate, &local_candidates) {
            kept_sources.push(candidate);
            craters.push(crater);
        }

        if craters.len() >= MAX_RESOLVED_CRATERS_PER_REGION {
            break;
        }
    }

    craters
}

fn resolve_crater(source: GuideSource, nearby_sources: &[GuideSource]) -> Option<ResolvedCrater> {
    if source.weight <= f32::EPSILON {
        return None;
    }

    let (heading_x, heading_z) = crater_heading(source, nearby_sources);
    let major_radius = lerp_f32(
        source.estimated_radius_blocks * 0.84,
        source.estimated_radius_blocks * 1.22,
        crater_hash01(source.coord, source.cell, SOURCE_BOWL_MAJOR_SALT),
    ) * (0.96
        + source.cell.basin_weight * 0.16
        + (source.cell.basin_depth / 16.0).clamp(0.0, 0.12));
    let minor_radius = major_radius
        * lerp_f32(
            0.82,
            1.08,
            crater_hash01(source.coord, source.cell, SOURCE_BOWL_MINOR_SALT),
        );
    let bowl_depth_blocks = source.cell.basin_depth
        * (1.18 + source.cell.basin_weight * 0.22)
        * lerp_f32(
            1.06,
            1.42,
            crater_hash01(source.coord, source.cell, SOURCE_DEPTH_SALT),
        );
    let floor_radius_scale = lerp_f32(
        0.32,
        0.56,
        crater_hash01(source.coord, source.cell, SOURCE_FLOOR_SALT),
    );
    let rim_outer_scale = lerp_f32(
        1.22,
        1.52,
        crater_hash01(source.coord, source.cell, SOURCE_RIM_OUTER_SALT),
    );
    let apron_outer_scale = lerp_f32(
        1.78,
        2.22,
        crater_hash01(source.coord, source.cell, SOURCE_APRON_SALT),
    );
    let rim_raise_blocks = source.rim_strength_blocks
        * lerp_f32(
            0.82,
            1.26,
            crater_hash01(source.coord, source.cell, SOURCE_RIM_OUTER_SALT ^ 0x55AA),
        );
    let contour_primary_phase =
        crater_hash01(source.coord, source.cell, SOURCE_CONTOUR_PRIMARY_SALT)
            * std::f32::consts::TAU;
    let contour_secondary_phase =
        crater_hash01(source.coord, source.cell, SOURCE_CONTOUR_SECONDARY_SALT)
            * std::f32::consts::TAU;
    let contour_primary_amp = lerp_f32(
        0.04,
        0.10,
        indexed_crater_hash01(
            source.coord,
            source.cell,
            0,
            SOURCE_CONTOUR_PRIMARY_SALT ^ 0x1337,
        ),
    );
    let contour_secondary_amp = lerp_f32(
        0.03,
        0.07,
        indexed_crater_hash01(
            source.coord,
            source.cell,
            1,
            SOURCE_CONTOUR_SECONDARY_SALT ^ 0x7331,
        ),
    );
    let outer_radius_x = major_radius * apron_outer_scale + CRATER_BOUNDS_PAD_BLOCKS;
    let outer_radius_z = minor_radius * apron_outer_scale + CRATER_BOUNDS_PAD_BLOCKS;
    let (half_extent_x, half_extent_z) =
        rotated_ellipse_half_extents(heading_x, heading_z, outer_radius_x, outer_radius_z);

    Some(ResolvedCrater {
        center_x: source.center_x,
        center_z: source.center_z,
        heading_x,
        heading_z,
        bowl_radius_x_blocks: major_radius.max(18.0),
        bowl_radius_z_blocks: minor_radius.max(16.0),
        floor_radius_scale,
        bowl_depth_blocks: bowl_depth_blocks.clamp(3.6, 18.0),
        rim_raise_blocks: rim_raise_blocks.clamp(1.8, 10.5),
        rim_outer_scale,
        apron_outer_scale,
        contour_primary_phase,
        contour_secondary_phase,
        contour_primary_amp,
        contour_secondary_amp,
        min_x: source.center_x - half_extent_x,
        max_x: source.center_x + half_extent_x,
        min_z: source.center_z - half_extent_z,
        max_z: source.center_z + half_extent_z,
    })
}

fn crater_heading(source: GuideSource, nearby_sources: &[GuideSource]) -> (f32, f32) {
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
        if distance > source.keepout_radius_blocks * 2.2 || distance >= nearest_distance {
            continue;
        }
        nearest = Some(*candidate);
        nearest_distance = distance;
    }

    if let Some(neighbor) = nearest {
        let delta_x = neighbor.center_x - source.center_x;
        let delta_z = neighbor.center_z - source.center_z;
        let length = (delta_x * delta_x + delta_z * delta_z)
            .sqrt()
            .max(f32::EPSILON);
        return (delta_x / length, delta_z / length);
    }

    fallback_axis_from_source(source)
}

pub(crate) fn sample_crater_apply_signal_from_window(
    window: &CraterWindow,
    world_x: i32,
    world_z: i32,
) -> CraterApplySample {
    let sample_x = world_x as f32 + 0.5;
    let sample_z = world_z as f32 + 0.5;
    let mut strongest_bowl = 0.0_f32;
    let mut second_bowl = 0.0_f32;
    let mut strongest_rim = 0.0_f32;
    let mut second_rim = 0.0_f32;
    let mut bowl_coverage = 0.0_f32;
    let mut rim_coverage = 0.0_f32;
    let mut apron_coverage = 0.0_f32;

    for crater in &window.craters {
        if sample_x < crater.min_x
            || sample_x > crater.max_x
            || sample_z < crater.min_z
            || sample_z > crater.max_z
        {
            continue;
        }

        let contribution = sample_crater_contribution(*crater, sample_x, sample_z);
        if contribution.bowl_depth_blocks <= f32::EPSILON
            && contribution.rim_raise_blocks <= f32::EPSILON
            && contribution.apron_coverage <= f32::EPSILON
        {
            continue;
        }

        insert_top2(
            contribution.bowl_depth_blocks,
            &mut strongest_bowl,
            &mut second_bowl,
        );
        insert_top2(
            contribution.rim_raise_blocks,
            &mut strongest_rim,
            &mut second_rim,
        );
        bowl_coverage = coverage_union(bowl_coverage, contribution.bowl_coverage);
        rim_coverage = coverage_union(rim_coverage, contribution.rim_coverage);
        apron_coverage = coverage_union(apron_coverage, contribution.apron_coverage);
    }

    if strongest_bowl <= f32::EPSILON && strongest_rim <= f32::EPSILON {
        return CraterApplySample::default();
    }

    CraterApplySample {
        bowl_coverage: bowl_coverage.clamp(0.0, 1.0),
        rim_coverage: rim_coverage.clamp(0.0, 1.0),
        apron_coverage: apron_coverage.clamp(0.0, 1.0),
        bowl_depth_blocks: strongest_bowl + second_bowl * 0.16,
        rim_raise_blocks: strongest_rim + second_rim * 0.14,
    }
}

pub(crate) fn sample_crater_surface_from_window(
    window: &CraterWindow,
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> CraterSurfaceSample {
    let apply = sample_crater_apply_signal_from_window(window, world_x, world_z);
    if apply.bowl_depth_blocks <= f32::EPSILON && apply.rim_raise_blocks <= f32::EPSILON {
        return CraterSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let bowl_presence = smoothstep_range(0.08, 0.86, apply.bowl_coverage);
    let rim_presence = smoothstep_range(0.08, 0.88, apply.rim_coverage);
    let apron_presence = smoothstep_range(0.12, 0.92, apply.apron_coverage)
        * (1.0 - smoothstep_range(0.56, 0.94, apply.bowl_coverage));
    let bowl_depth = apply.bowl_depth_blocks
        * (0.94 + meso.basin_weight * 0.22 + (meso.basin_depth / 10.0).clamp(0.0, 0.14))
        * bowl_presence;
    let rim_raise = apply.rim_raise_blocks
        * (0.82 + meso.hilliness * 0.22 + (meso.hill_height / 14.0).clamp(0.0, 0.12))
        * rim_presence;
    let apron_raise = apply.rim_raise_blocks
        * 0.18
        * (0.78 + meso.hilliness * 0.16 + meso.basin_weight * 0.12)
        * apron_presence;
    let raw_offset = rim_raise + apron_raise - bowl_depth;
    let relief_cap = (relief_budget * 1.28)
        .max(12.0)
        .min((relief_budget * 2.6).max(28.0));
    let offset = soft_cap_signed(raw_offset, relief_cap);
    if offset.abs() <= f32::EPSILON {
        return CraterSurfaceSample::flat(base_surface_y);
    }

    let blend_weight = smoothstep01(
        (apply.bowl_coverage * 0.74 + apply.rim_coverage * 0.22 + apply.apron_coverage * 0.08)
            .clamp(0.0, 1.0),
    );

    CraterSurfaceSample {
        target_surface_y: base_surface_y + offset,
        blend_weight,
        relief_spend: offset.abs() * blend_weight,
        bowl_coverage: apply.bowl_coverage,
        rim_coverage: apply.rim_coverage,
    }
}

fn sample_crater_contribution(
    crater: ResolvedCrater,
    sample_x: f32,
    sample_z: f32,
) -> CraterContribution {
    let delta_x = sample_x - crater.center_x;
    let delta_z = sample_z - crater.center_z;
    let along = delta_x * crater.heading_x + delta_z * crater.heading_z;
    let across = delta_x * -crater.heading_z + delta_z * crater.heading_x;
    let radial = irregular_radial_distance(crater, along, across);
    let bowl_coverage = smoothstep_range(1.02, 0.0, radial);
    let floor_coverage = smoothstep_range(crater.floor_radius_scale, 0.0, radial);
    let rim_coverage = ring_profile(radial, 0.78, 1.04, crater.rim_outer_scale, 1.10);
    let apron_coverage = ring_profile(radial, 1.10, 1.44, crater.apron_outer_scale, 1.52);

    CraterContribution {
        bowl_coverage,
        rim_coverage,
        apron_coverage,
        bowl_depth_blocks: crater.bowl_depth_blocks
            * (bowl_coverage * 0.36 + floor_coverage * 0.64),
        rim_raise_blocks: crater.rim_raise_blocks
            * (rim_coverage * 0.78 + apron_coverage * 0.12)
            * (1.0 - bowl_coverage * 0.10),
    }
}

fn irregular_radial_distance(crater: ResolvedCrater, along: f32, across: f32) -> f32 {
    let normalized_along = along / crater.bowl_radius_x_blocks.max(f32::EPSILON);
    let normalized_across = across / crater.bowl_radius_z_blocks.max(f32::EPSILON);
    let angle = normalized_across.atan2(normalized_along);
    let contour_scale = 1.0
        + (angle * 2.0 + crater.contour_primary_phase).sin() * crater.contour_primary_amp
        + (angle * 3.0 + crater.contour_secondary_phase).cos() * crater.contour_secondary_amp;

    ((normalized_along * normalized_along) + (normalized_across * normalized_across)).sqrt()
        / contour_scale.max(0.78)
}

fn ring_profile(
    radial: f32,
    inner_start: f32,
    inner_end: f32,
    outer_start: f32,
    outer_end: f32,
) -> f32 {
    let rise = smoothstep_range(inner_start, inner_end, radial);
    let fall = smoothstep_range(outer_start, outer_end, radial);
    (rise * fall).clamp(0.0, 1.0)
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

#[cfg(test)]
mod tests {
    use super::{
        build_crater_window, ring_profile, sample_crater_apply_signal_from_window,
        sample_crater_surface_from_window,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::coord::ChunkCoord;
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    fn strong_guides() -> MesoGuideMap {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 8).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        *cells.get_mut(AtlasCoord::new(2, 2)).unwrap() = MesoGuideCell {
            basin_weight: 0.96,
            basin_depth: 7.4,
            hilliness: 0.52,
            hill_height: 5.8,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(5, 4)).unwrap() = MesoGuideCell {
            basin_weight: 0.88,
            basin_depth: 5.9,
            hilliness: 0.34,
            hill_height: 4.1,
            ..MesoGuideCell::default()
        };
        MesoGuideMap { area, cells }
    }

    #[test]
    fn resolved_window_is_stable_for_shared_world_point_across_neighboring_chunks() {
        let guides = strong_guides();
        let left = build_crater_window(&guides, ChunkCoord(1, 0, 2));
        let right = build_crater_window(&guides, ChunkCoord(2, 0, 2));
        let shared_world_x = CHUNK_EDGE_I32 * 2;
        let shared_world_z = CHUNK_EDGE_I32 * 2 + CHUNK_EDGE_I32 / 2;
        let left_sample =
            sample_crater_apply_signal_from_window(&left, shared_world_x, shared_world_z);
        let right_sample =
            sample_crater_apply_signal_from_window(&right, shared_world_x, shared_world_z);

        assert_eq!(
            left_sample, right_sample,
            "expected neighboring chunk windows to sample the same crater object at a shared world-space point, left={left_sample:?}, right={right_sample:?}"
        );
    }

    #[test]
    fn same_world_point_stays_close_across_meso_cell_boundary() {
        let guides = strong_guides();
        let chunk = ChunkCoord(1, 0, 1);
        let window = build_crater_window(&guides, chunk);
        let meso_span_blocks = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let boundary_world_x = meso_span_blocks * 3;
        let sample_world_z = meso_span_blocks * 2 + meso_span_blocks / 2;
        let left_outer =
            sample_crater_apply_signal_from_window(&window, boundary_world_x - 3, sample_world_z);
        let left_edge =
            sample_crater_apply_signal_from_window(&window, boundary_world_x - 1, sample_world_z);
        let right_edge =
            sample_crater_apply_signal_from_window(&window, boundary_world_x, sample_world_z);
        let right_outer =
            sample_crater_apply_signal_from_window(&window, boundary_world_x + 2, sample_world_z);
        let seam_delta = (right_edge.bowl_depth_blocks - left_edge.bowl_depth_blocks).abs();
        let local_delta = (left_edge.bowl_depth_blocks - left_outer.bowl_depth_blocks)
            .abs()
            .max((right_outer.bowl_depth_blocks - right_edge.bowl_depth_blocks).abs());

        assert!(
            seam_delta <= local_delta + 0.8,
            "expected crater bowl to stay continuous across meso-cell boundary, seam_delta={seam_delta:.3}, local_delta={local_delta:.3}, left_edge={left_edge:?}, right_edge={right_edge:?}"
        );
    }

    #[test]
    fn sample_surface_returns_flat_when_window_has_no_craters() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let guides = MesoGuideMap {
            area,
            cells: AtlasGrid::defaulted(area),
        };
        let window = build_crater_window(&guides, ChunkCoord(0, 0, 0));
        let sample = sample_crater_surface_from_window(&window, &guides, 64, 64, 100.0, 16.0);

        assert_eq!(sample.target_surface_y, 100.0);
        assert_eq!(sample.blend_weight, 0.0);
    }

    #[test]
    fn strong_crater_keeps_visible_rim_uplift_and_bowl_depth() {
        let guides = strong_guides();
        let chunk = ChunkCoord(2, 0, 2);
        let window = build_crater_window(&guides, chunk);
        let meso_span_blocks = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32;
        let center_x = meso_span_blocks * 2 + meso_span_blocks / 2;
        let center_z = meso_span_blocks * 2 + meso_span_blocks / 2;
        let center =
            sample_crater_surface_from_window(&window, &guides, center_x, center_z, 100.0, 18.0);
        let mut strongest_rim = center;

        for offset_z in (-80..=80).step_by(4) {
            for offset_x in (-80..=80).step_by(4) {
                let sample = sample_crater_surface_from_window(
                    &window,
                    &guides,
                    center_x + offset_x,
                    center_z + offset_z,
                    100.0,
                    18.0,
                );
                if sample.target_surface_y > strongest_rim.target_surface_y {
                    strongest_rim = sample;
                }
            }
        }

        assert!(
            center.target_surface_y <= 96.0,
            "expected crater bowl to cut below the prototype, got {center:?}"
        );
        assert!(
            strongest_rim.target_surface_y >= 101.6,
            "expected crater rim to keep visible uplift beside the bowl, got {strongest_rim:?}"
        );
    }

    #[test]
    fn ring_profile_stays_zero_inside_bowl_and_fades_outside_apron() {
        assert!(ring_profile(0.40, 0.78, 1.04, 1.52, 1.10) <= 0.01);
        assert!(ring_profile(1.16, 0.78, 1.04, 1.52, 1.10) >= 0.40);
        assert!(ring_profile(1.80, 0.78, 1.04, 1.52, 1.10) <= 0.06);
    }
}
