use crate::world::atlas::{AtlasCoord, MesoGuideMap, MesoRegionCoord, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::super::super::lerp_f32;
use super::{
    CliffSource, CoastalCliffBandApplySample, CoastalCliffBandSurfaceSample,
    SOURCE_APRON_WIDTH_SALT, SOURCE_BENCH_LIFT_SALT, SOURCE_FACE_WIDTH_SALT, SOURCE_LENGTH_SALT,
    SOURCE_PLATEAU_DEPTH_SALT, cliff_hash01, collect_peak_sources, coverage_union,
    distance_between_points, heading_alignment, smoothstep_range, smoothstep01, soft_cap_positive,
};

const WINDOW_OWNER_PADDING_REGIONS: i32 = 1;
const REGION_RESOLVE_PADDING_REGIONS: i32 = 1;
const MAX_RESOLVED_CLIFFS_PER_REGION: usize = 4;
const OWNER_KEEP_DISTANCE_MULTIPLIER: f32 = 1.08;
const CLIFF_BOUNDS_PAD_BLOCKS: f32 = 8.0;
const ALIGNED_NEIGHBOR_CROSS_MULTIPLIER: f32 = 0.72;
const ALIGNED_NEIGHBOR_ALONG_MULTIPLIER: f32 = 2.40;

#[derive(Debug, Clone)]
pub(crate) struct CoastalCliffBandWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    cliffs: Vec<ResolvedCoastalCliffObject>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg(test)]
pub(crate) struct CoastalCliffBandResolvedObjectDebug {
    pub owner_region: MesoRegionCoord,
    pub center_x: f32,
    pub center_z: f32,
    pub heading_x: f32,
    pub heading_z: f32,
    pub half_length_blocks: f32,
    pub face_width_blocks: f32,
    pub plateau_depth_blocks: f32,
    pub apron_width_blocks: f32,
    pub crest_raise_blocks: f32,
    pub bench_lift_blocks: f32,
}

#[derive(Debug, Clone)]
struct ResolvedCoastalCliffObject {
    #[cfg(test)]
    owner_region: MesoRegionCoord,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    normal_x: f32,
    normal_z: f32,
    half_length_blocks: f32,
    face_width_blocks: f32,
    plateau_depth_blocks: f32,
    apron_width_blocks: f32,
    crest_raise_blocks: f32,
    bench_lift_blocks: f32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
}

pub(crate) fn build_coastal_cliff_band_window(
    guides: &MesoGuideMap,
    chunk: ChunkCoord,
) -> CoastalCliffBandWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let chunk_bounds = chunk_bounds(chunk, 0.0);
    let candidates = collect_peak_sources(guides, meso_span_blocks);
    let (min_region_x, max_region_x, min_region_z, max_region_z) =
        owner_region_range_for_chunk(chunk, WINDOW_OWNER_PADDING_REGIONS);
    let mut cliffs = Vec::new();

    for region_z in min_region_z..=max_region_z {
        for region_x in min_region_x..=max_region_x {
            cliffs.extend(resolve_owned_cliffs(
                &candidates,
                MesoRegionCoord::new(region_x, region_z),
            ));
        }
    }

    cliffs.retain(|cliff| {
        bounds_overlap(
            chunk_bounds,
            cliff.min_x,
            cliff.max_x,
            cliff.min_z,
            cliff.max_z,
        )
    });

    CoastalCliffBandWindow { chunk, cliffs }
}

#[cfg(test)]
pub(crate) fn debug_coastal_cliff_band_resolved_objects_from_window(
    window: &CoastalCliffBandWindow,
) -> Vec<CoastalCliffBandResolvedObjectDebug> {
    window
        .cliffs
        .iter()
        .map(|cliff| CoastalCliffBandResolvedObjectDebug {
            owner_region: cliff.owner_region,
            center_x: cliff.center_x,
            center_z: cliff.center_z,
            heading_x: cliff.heading_x,
            heading_z: cliff.heading_z,
            half_length_blocks: cliff.half_length_blocks,
            face_width_blocks: cliff.face_width_blocks,
            plateau_depth_blocks: cliff.plateau_depth_blocks,
            apron_width_blocks: cliff.apron_width_blocks,
            crest_raise_blocks: cliff.crest_raise_blocks,
            bench_lift_blocks: cliff.bench_lift_blocks,
        })
        .collect()
}

fn resolve_owned_cliffs(
    candidates: &[CliffSource],
    owner_region: MesoRegionCoord,
) -> Vec<ResolvedCoastalCliffObject> {
    let search_bounds = guide_region_bounds(owner_region, REGION_RESOLVE_PADDING_REGIONS);
    let local_candidates = candidates
        .iter()
        .copied()
        .filter(|candidate| bounds_contains_coord(search_bounds, candidate.coord))
        .collect::<Vec<_>>();
    let mut kept_sources = Vec::new();
    let mut cliffs = Vec::new();

    for candidate in local_candidates
        .iter()
        .copied()
        .filter(|candidate| owner_region_for_coord(candidate.coord) == owner_region)
    {
        if kept_sources
            .iter()
            .any(|existing: &CliffSource| sources_overlap(candidate, *existing))
        {
            continue;
        }

        if let Some(cliff) = resolve_coastal_cliff(candidate, owner_region, &local_candidates) {
            kept_sources.push(candidate);
            cliffs.push(cliff);
        }

        if cliffs.len() >= MAX_RESOLVED_CLIFFS_PER_REGION {
            break;
        }
    }

    cliffs
}

fn resolve_coastal_cliff(
    source: CliffSource,
    owner_region: MesoRegionCoord,
    nearby_sources: &[CliffSource],
) -> Option<ResolvedCoastalCliffObject> {
    #[cfg(not(test))]
    let _ = owner_region;

    let (heading_x, heading_z) = cliff_heading(source, nearby_sources);
    let normal_x = -heading_z;
    let normal_z = heading_x;
    let aligned_sources = aligned_sources(
        source,
        nearby_sources,
        heading_x,
        heading_z,
        normal_x,
        normal_z,
    );
    if aligned_sources.is_empty() {
        return None;
    }

    let mut weighted_center_along = 0.0_f32;
    let mut total_weight = 0.0_f32;
    let mut along_min = 0.0_f32;
    let mut along_max = 0.0_f32;

    for candidate in &aligned_sources {
        let delta_x = candidate.center_x - source.center_x;
        let delta_z = candidate.center_z - source.center_z;
        let along = delta_x * heading_x + delta_z * heading_z;
        weighted_center_along += along * candidate.weight;
        total_weight += candidate.weight;
        along_min = along_min.min(along);
        along_max = along_max.max(along);
    }

    let mean_along = if total_weight > f32::EPSILON {
        weighted_center_along / total_weight
    } else {
        0.0
    };
    let span_half = (along_max - along_min).abs() * 0.5;
    let center_x = source.center_x + heading_x * mean_along;
    let center_z = source.center_z + heading_z * mean_along;
    let base_half_length_blocks = lerp_f32(
        54.0,
        108.0,
        cliff_hash01(source.coord, source.cell, SOURCE_LENGTH_SALT),
    ) * (0.92 + source.cell.escarpment_weight * 0.24);
    let half_length_blocks = base_half_length_blocks
        .max(span_half + 24.0)
        .clamp(46.0, 176.0);
    let face_width_blocks = lerp_f32(
        8.0,
        18.0,
        cliff_hash01(source.coord, source.cell, SOURCE_FACE_WIDTH_SALT),
    ) * (0.92
        + source.cell.escarpment_weight * 0.22
        + (source.cell.escarpment_height / 20.0).clamp(0.0, 0.10));
    let plateau_depth_blocks = lerp_f32(
        28.0,
        64.0,
        cliff_hash01(source.coord, source.cell, SOURCE_PLATEAU_DEPTH_SALT),
    ) * (0.88 + source.cell.escarpment_weight * 0.28);
    let apron_width_blocks = lerp_f32(
        18.0,
        40.0,
        cliff_hash01(source.coord, source.cell, SOURCE_APRON_WIDTH_SALT),
    ) * (0.86 + source.cell.escarpment_weight * 0.18);
    let crest_raise_blocks = (source.cell.escarpment_height
        * (1.12 + source.cell.escarpment_weight * 0.42))
        .clamp(3.0, 14.0);
    let bench_lift_blocks = crest_raise_blocks
        * lerp_f32(
            0.12,
            0.26,
            cliff_hash01(source.coord, source.cell, SOURCE_BENCH_LIFT_SALT),
        );

    let total_cross = face_width_blocks * 1.30
        + plateau_depth_blocks
        + apron_width_blocks
        + CLIFF_BOUNDS_PAD_BLOCKS;
    let (half_extent_x, half_extent_z) =
        rotated_extents(heading_x, heading_z, half_length_blocks, total_cross);

    Some(ResolvedCoastalCliffObject {
        #[cfg(test)]
        owner_region,
        center_x,
        center_z,
        heading_x,
        heading_z,
        normal_x,
        normal_z,
        half_length_blocks,
        face_width_blocks,
        plateau_depth_blocks,
        apron_width_blocks,
        crest_raise_blocks,
        bench_lift_blocks,
        min_x: center_x - half_extent_x,
        max_x: center_x + half_extent_x,
        min_z: center_z - half_extent_z,
        max_z: center_z + half_extent_z,
    })
}

fn cliff_heading(source: CliffSource, nearby_sources: &[CliffSource]) -> (f32, f32) {
    let mut sum_x = source.heading_x * source.weight;
    let mut sum_z = source.heading_z * source.weight;
    let mut total_weight = source.weight;

    for candidate in nearby_sources {
        if candidate.coord == source.coord {
            continue;
        }
        let alignment = heading_alignment(
            (source.heading_x, source.heading_z),
            (candidate.heading_x, candidate.heading_z),
        );
        if alignment < 0.68 {
            continue;
        }

        let delta_x = candidate.center_x - source.center_x;
        let delta_z = candidate.center_z - source.center_z;
        let across = delta_x * source.normal_x + delta_z * source.normal_z;
        let along = delta_x * source.heading_x + delta_z * source.heading_z;
        if across.abs() > source.keep_distance_blocks * ALIGNED_NEIGHBOR_CROSS_MULTIPLIER
            || along.abs() > source.keep_distance_blocks * ALIGNED_NEIGHBOR_ALONG_MULTIPLIER
        {
            continue;
        }

        let sign = if source.heading_x * candidate.heading_x
            + source.heading_z * candidate.heading_z
            >= 0.0
        {
            1.0
        } else {
            -1.0
        };
        sum_x += candidate.heading_x * sign * candidate.weight;
        sum_z += candidate.heading_z * sign * candidate.weight;
        total_weight += candidate.weight;
    }

    if total_weight <= f32::EPSILON {
        return (source.heading_x, source.heading_z);
    }

    let heading_length = (sum_x * sum_x + sum_z * sum_z).sqrt();
    if heading_length <= f32::EPSILON {
        return (source.heading_x, source.heading_z);
    }

    (sum_x / heading_length, sum_z / heading_length)
}

fn aligned_sources(
    source: CliffSource,
    nearby_sources: &[CliffSource],
    heading_x: f32,
    heading_z: f32,
    normal_x: f32,
    normal_z: f32,
) -> Vec<CliffSource> {
    let mut aligned = Vec::new();

    for candidate in nearby_sources {
        let alignment = heading_alignment(
            (heading_x, heading_z),
            (candidate.heading_x, candidate.heading_z),
        );
        if alignment < 0.68 {
            continue;
        }
        let delta_x = candidate.center_x - source.center_x;
        let delta_z = candidate.center_z - source.center_z;
        let across = delta_x * normal_x + delta_z * normal_z;
        let along = delta_x * heading_x + delta_z * heading_z;
        if across.abs() > source.keep_distance_blocks * ALIGNED_NEIGHBOR_CROSS_MULTIPLIER
            || along.abs() > source.keep_distance_blocks * ALIGNED_NEIGHBOR_ALONG_MULTIPLIER
        {
            continue;
        }
        aligned.push(*candidate);
    }

    if !aligned
        .iter()
        .any(|candidate| candidate.coord == source.coord)
    {
        aligned.push(source);
    }
    aligned
}

fn sources_overlap(candidate: CliffSource, existing: CliffSource) -> bool {
    let alignment = heading_alignment(
        (candidate.heading_x, candidate.heading_z),
        (existing.heading_x, existing.heading_z),
    );
    if alignment < 0.72 {
        return false;
    }

    let delta_x = candidate.center_x - existing.center_x;
    let delta_z = candidate.center_z - existing.center_z;
    let normal_x = -existing.heading_z;
    let normal_z = existing.heading_x;
    let along = (delta_x * existing.heading_x + delta_z * existing.heading_z).abs();
    let across = (delta_x * normal_x + delta_z * normal_z).abs();
    let keep_distance = candidate
        .keep_distance_blocks
        .max(existing.keep_distance_blocks);
    let euclidean = distance_between_points(
        (candidate.center_x, candidate.center_z),
        (existing.center_x, existing.center_z),
    );

    across <= keep_distance * 0.62
        && along <= keep_distance * 1.32
        && euclidean <= keep_distance * OWNER_KEEP_DISTANCE_MULTIPLIER * 1.42
}

pub(crate) fn sample_coastal_cliff_band_apply_signal_from_window(
    window: &CoastalCliffBandWindow,
    world_x: i32,
    world_z: i32,
) -> CoastalCliffBandApplySample {
    let sample_x = world_x as f32 + 0.5;
    let sample_z = world_z as f32 + 0.5;
    let mut face_coverage = 0.0_f32;
    let mut plateau_coverage = 0.0_f32;
    let mut bench_coverage = 0.0_f32;
    let mut crest_raise_blocks = 0.0_f32;
    let mut bench_lift_blocks = 0.0_f32;
    let mut signed_distance_weighted_sum = 0.0_f32;
    let mut signed_distance_weight = 0.0_f32;

    for cliff in &window.cliffs {
        if sample_x < cliff.min_x
            || sample_x > cliff.max_x
            || sample_z < cliff.min_z
            || sample_z > cliff.max_z
        {
            continue;
        }

        let delta_x = sample_x - cliff.center_x;
        let delta_z = sample_z - cliff.center_z;
        let along = delta_x * cliff.heading_x + delta_z * cliff.heading_z;
        let across = delta_x * cliff.normal_x + delta_z * cliff.normal_z;
        let along_coverage = smoothstep_range(
            cliff.half_length_blocks + 18.0,
            cliff.half_length_blocks - 4.0,
            along.abs(),
        );
        if along_coverage <= f32::EPSILON {
            continue;
        }

        let face =
            along_coverage * smoothstep_range(cliff.face_width_blocks * 1.24, 0.0, across.abs());
        let crest =
            along_coverage * smoothstep_range(cliff.face_width_blocks * 0.58, 0.0, across.abs());
        let plateau = if across >= -cliff.face_width_blocks * 0.12 {
            let land_distance = across.max(0.0);
            let rise = smoothstep_range(
                cliff.face_width_blocks * 0.14,
                cliff.face_width_blocks * 0.92,
                land_distance,
            );
            let falloff = smoothstep_range(
                cliff.plateau_depth_blocks + cliff.face_width_blocks * 0.78,
                cliff.plateau_depth_blocks * 0.22,
                land_distance,
            );
            along_coverage * rise * falloff
        } else {
            0.0
        };
        let bench = if across <= cliff.face_width_blocks * 0.26 {
            let sea_distance = (-across).max(0.0);
            let rise = smoothstep_range(
                cliff.face_width_blocks * 0.06,
                cliff.face_width_blocks * 0.60,
                sea_distance,
            );
            let falloff = smoothstep_range(
                cliff.apron_width_blocks + cliff.face_width_blocks * 0.64,
                cliff.apron_width_blocks * 0.18,
                sea_distance,
            );
            along_coverage * rise * falloff
        } else {
            0.0
        };

        face_coverage = coverage_union(face_coverage, face);
        plateau_coverage = coverage_union(plateau_coverage, plateau);
        bench_coverage = coverage_union(bench_coverage, bench);
        crest_raise_blocks = crest_raise_blocks
            .max(cliff.crest_raise_blocks * (plateau * 0.84 + face * 0.46 + crest * 0.22));
        bench_lift_blocks = bench_lift_blocks.max(cliff.bench_lift_blocks * bench);
        signed_distance_weighted_sum += across * face.max(plateau * 0.45 + bench * 0.25);
        signed_distance_weight += face.max(plateau * 0.45 + bench * 0.25);
    }

    if face_coverage <= f32::EPSILON
        && plateau_coverage <= f32::EPSILON
        && bench_coverage <= f32::EPSILON
    {
        return CoastalCliffBandApplySample::default();
    }

    CoastalCliffBandApplySample {
        face_coverage: face_coverage.clamp(0.0, 1.0),
        plateau_coverage: plateau_coverage.clamp(0.0, 1.0),
        bench_coverage: bench_coverage.clamp(0.0, 1.0),
        crest_raise_blocks,
        bench_lift_blocks,
        signed_distance_blocks: if signed_distance_weight > f32::EPSILON {
            signed_distance_weighted_sum / signed_distance_weight
        } else {
            0.0
        },
    }
}

pub(crate) fn sample_coastal_cliff_band_surface_from_window(
    window: &CoastalCliffBandWindow,
    guides: &MesoGuideMap,
    world_x: i32,
    world_z: i32,
    base_surface_y: f32,
    relief_budget: f32,
) -> CoastalCliffBandSurfaceSample {
    let apply = sample_coastal_cliff_band_apply_signal_from_window(window, world_x, world_z);
    if apply.face_coverage <= f32::EPSILON
        && apply.plateau_coverage <= f32::EPSILON
        && apply.bench_coverage <= f32::EPSILON
    {
        return CoastalCliffBandSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let guide_presence = smoothstep_range(0.08, 0.82, meso.escarpment_weight);
    let face_presence = smoothstep_range(0.08, 0.88, apply.face_coverage);
    let plateau_presence = smoothstep_range(0.08, 0.92, apply.plateau_coverage);
    let bench_presence = smoothstep_range(0.08, 0.80, apply.bench_coverage);
    let guide_raise =
        meso.escarpment_height * guide_presence * (face_presence * 0.12 + plateau_presence * 0.18);
    let raw_target_raise = apply.crest_raise_blocks
        * (plateau_presence * 0.82 + face_presence * 0.54)
        + apply.bench_lift_blocks * bench_presence * 0.30
        + guide_raise;
    let raise_cap = apply
        .crest_raise_blocks
        .max(relief_budget * 1.24)
        .max(8.0)
        .min(relief_budget.max(12.0) * 2.20);
    let target_raise = soft_cap_positive(raw_target_raise, raise_cap);
    if target_raise <= f32::EPSILON {
        return CoastalCliffBandSurfaceSample::flat(base_surface_y);
    }

    let blend_weight = smoothstep01(
        (apply.face_coverage * 0.44 + apply.plateau_coverage * 0.42 + apply.bench_coverage * 0.14)
            .clamp(0.0, 1.0),
    );

    CoastalCliffBandSurfaceSample {
        target_surface_y: base_surface_y + target_raise,
        blend_weight,
        relief_spend: target_raise * blend_weight,
        face_coverage: apply.face_coverage,
        plateau_coverage: apply.plateau_coverage,
        bench_coverage: apply.bench_coverage,
    }
}

fn rotated_extents(
    heading_x: f32,
    heading_z: f32,
    half_length_blocks: f32,
    half_cross_blocks: f32,
) -> (f32, f32) {
    (
        heading_x.abs() * half_length_blocks
            + heading_z.abs() * half_cross_blocks
            + CLIFF_BOUNDS_PAD_BLOCKS,
        heading_z.abs() * half_length_blocks
            + heading_x.abs() * half_cross_blocks
            + CLIFF_BOUNDS_PAD_BLOCKS,
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
        CoastalCliffBandResolvedObjectDebug, build_coastal_cliff_band_window,
        debug_coastal_cliff_band_resolved_objects_from_window,
        sample_coastal_cliff_band_apply_signal_from_window,
        sample_coastal_cliff_band_surface_from_window,
    };
    use crate::world::atlas::{AtlasArea, AtlasCoord, AtlasGrid, MesoGuideCell, MesoGuideMap};
    use crate::world::coord::ChunkCoord;
    use crate::world::{CHUNK_EDGE_I32, MESO_GUIDE_CELL_SIZE_IN_CHUNKS};

    fn assert_debug_close(
        left: CoastalCliffBandResolvedObjectDebug,
        right: CoastalCliffBandResolvedObjectDebug,
    ) {
        assert_eq!(left.owner_region, right.owner_region);
        assert!((left.center_x - right.center_x).abs() <= 0.001);
        assert!((left.center_z - right.center_z).abs() <= 0.001);
        assert!((left.heading_x - right.heading_x).abs() <= 0.001);
        assert!((left.heading_z - right.heading_z).abs() <= 0.001);
        assert!((left.half_length_blocks - right.half_length_blocks).abs() <= 0.001);
        assert!((left.face_width_blocks - right.face_width_blocks).abs() <= 0.001);
        assert!((left.plateau_depth_blocks - right.plateau_depth_blocks).abs() <= 0.001);
        assert!((left.apron_width_blocks - right.apron_width_blocks).abs() <= 0.001);
        assert!((left.crest_raise_blocks - right.crest_raise_blocks).abs() <= 0.001);
        assert!((left.bench_lift_blocks - right.bench_lift_blocks).abs() <= 0.001);
    }

    fn strong_guides() -> MesoGuideMap {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 8, 5).unwrap();
        let mut cells = AtlasGrid::defaulted(area);
        let cell = MesoGuideCell {
            escarpment_weight: 0.93,
            escarpment_height: 6.0,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.0,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(1, 1)).unwrap() = cell;
        *cells.get_mut(AtlasCoord::new(2, 1)).unwrap() = MesoGuideCell {
            escarpment_weight: 0.88,
            escarpment_height: 5.2,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: 0.06,
            ..MesoGuideCell::default()
        };
        *cells.get_mut(AtlasCoord::new(3, 1)).unwrap() = MesoGuideCell {
            escarpment_weight: 0.84,
            escarpment_height: 4.8,
            escarpment_heading_x: 1.0,
            escarpment_heading_z: 0.0,
            escarpment_signed_distance_cells: -0.04,
            ..MesoGuideCell::default()
        };
        MesoGuideMap { area, cells }
    }

    #[test]
    fn build_window_is_deterministic_for_same_chunk() {
        let guides = strong_guides();
        let chunk = ChunkCoord(2, 0, 1);
        let first = build_coastal_cliff_band_window(&guides, chunk);
        let second = build_coastal_cliff_band_window(&guides, chunk);
        let first_debug = debug_coastal_cliff_band_resolved_objects_from_window(&first);
        let second_debug = debug_coastal_cliff_band_resolved_objects_from_window(&second);

        assert_eq!(first_debug.len(), second_debug.len());
        for (left, right) in first_debug.into_iter().zip(second_debug) {
            assert_debug_close(left, right);
        }
    }

    #[test]
    fn neighboring_chunk_windows_share_same_resolved_cliff_object() {
        let guides = strong_guides();
        let left = build_coastal_cliff_band_window(&guides, ChunkCoord(3, 0, 1));
        let right = build_coastal_cliff_band_window(&guides, ChunkCoord(4, 0, 1));
        let left_debug = debug_coastal_cliff_band_resolved_objects_from_window(&left);
        let right_debug = debug_coastal_cliff_band_resolved_objects_from_window(&right);

        let shared_left = left_debug
            .first()
            .copied()
            .expect("expected left window to resolve at least one cliff");
        let shared_right = right_debug
            .iter()
            .find(|candidate| {
                (candidate.center_x - shared_left.center_x).abs() <= 0.001
                    && (candidate.center_z - shared_left.center_z).abs() <= 0.001
            })
            .copied()
            .expect("expected neighboring chunk window to keep the same cliff object");

        assert_debug_close(shared_left, shared_right);
    }

    #[test]
    fn sample_surface_stays_close_across_chunk_boundary() {
        let guides = strong_guides();
        let left_window = build_coastal_cliff_band_window(&guides, ChunkCoord(3, 0, 1));
        let right_window = build_coastal_cliff_band_window(&guides, ChunkCoord(4, 0, 1));
        let boundary_world_x = CHUNK_EDGE_I32 * 4;
        let sample_world_z = CHUNK_EDGE_I32 * MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32 + 20;
        let left = sample_coastal_cliff_band_surface_from_window(
            &left_window,
            &guides,
            boundary_world_x - 1,
            sample_world_z,
            100.0,
            12.0,
        );
        let right = sample_coastal_cliff_band_surface_from_window(
            &right_window,
            &guides,
            boundary_world_x,
            sample_world_z,
            100.0,
            12.0,
        );

        assert!(
            (left.target_surface_y - right.target_surface_y).abs() <= 1.2,
            "expected cliff samples across neighboring chunk windows to stay close, left={left:?}, right={right:?}"
        );
    }

    #[test]
    fn sample_surface_returns_flat_when_guides_are_absent() {
        let area = AtlasArea::new(AtlasCoord::new(0, 0), 4, 4).unwrap();
        let guides = MesoGuideMap {
            area,
            cells: AtlasGrid::defaulted(area),
        };
        let window = build_coastal_cliff_band_window(&guides, ChunkCoord(0, 0, 0));
        let sample =
            sample_coastal_cliff_band_surface_from_window(&window, &guides, 96, 96, 100.0, 12.0);

        assert_eq!(sample, super::CoastalCliffBandSurfaceSample::flat(100.0));
    }

    #[test]
    fn landward_plateau_stays_higher_than_seaward_bench() {
        let guides = strong_guides();
        let window = build_coastal_cliff_band_window(&guides, ChunkCoord(2, 0, 1));
        let debug = debug_coastal_cliff_band_resolved_objects_from_window(&window);
        let cliff = debug
            .first()
            .copied()
            .expect("expected resolved cliff object");
        let normal_x = -cliff.heading_z;
        let normal_z = cliff.heading_x;
        let landward_x =
            (cliff.center_x + normal_x * (cliff.plateau_depth_blocks * 0.55)).round() as i32;
        let landward_z =
            (cliff.center_z + normal_z * (cliff.plateau_depth_blocks * 0.55)).round() as i32;
        let seaward_x =
            (cliff.center_x - normal_x * (cliff.apron_width_blocks * 0.55)).round() as i32;
        let seaward_z =
            (cliff.center_z - normal_z * (cliff.apron_width_blocks * 0.55)).round() as i32;
        let landward = sample_coastal_cliff_band_surface_from_window(
            &window, &guides, landward_x, landward_z, 100.0, 12.0,
        );
        let seaward = sample_coastal_cliff_band_surface_from_window(
            &window, &guides, seaward_x, seaward_z, 100.0, 12.0,
        );

        assert!(
            landward.target_surface_y >= seaward.target_surface_y + 2.0,
            "expected landward cliff-top plateau to stay materially higher than the seaward bench, landward={landward:?}, seaward={seaward:?}, cliff={cliff:?}"
        );
    }

    #[test]
    fn strong_cliff_keeps_visible_raise_and_apply_signal() {
        let guides = strong_guides();
        let window = build_coastal_cliff_band_window(&guides, ChunkCoord(2, 0, 1));
        let debug = debug_coastal_cliff_band_resolved_objects_from_window(&window);
        let cliff = debug
            .first()
            .copied()
            .expect("expected resolved cliff object");
        let sample_x = cliff.center_x.round() as i32;
        let sample_z = cliff.center_z.round() as i32;
        let apply = sample_coastal_cliff_band_apply_signal_from_window(&window, sample_x, sample_z);
        let surface = sample_coastal_cliff_band_surface_from_window(
            &window, &guides, sample_x, sample_z, 100.0, 12.0,
        );

        assert!(
            apply.face_coverage >= 0.35,
            "expected resolved cliff face to stay readable at crest, got {apply:?}"
        );
        assert!(
            surface.target_surface_y - 100.0 >= 3.0,
            "expected strong coastal cliff surface resolve to keep visible uplift, got {surface:?}"
        );
        assert!(
            surface.blend_weight >= 0.35,
            "expected strong coastal cliff sample to keep meaningful blend support, got {surface:?}"
        );
    }
}
