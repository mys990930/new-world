use crate::world::atlas::{AtlasCoord, MesoGuideMap, MesoRegionCoord, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::{
    GuideSource, HillClusterApplySample, HillClusterSurfaceSample, MacroLobeDescriptor,
    dominant_apply_axis, ellipse_footprint, guide_source, indexed_lobe_hash01, insert_top3,
    irregular_lobe_footprint, is_local_source_peak, lobe_hash01, macro_lobe_descriptor,
    prune_cluster_sources, smoothstep01, smoothstep_range, soft_cap_positive, source_chain_heading,
};
use super::{
    SOURCE_CHAIN_SPACING_SALT, SOURCE_COUNT_SALT, SOURCE_ENVELOPE_FILL_SALT,
    SOURCE_ENVELOPE_RADIUS_SALT, SOURCE_ENVELOPE_SHOULDER_SALT, SOURCE_MAJOR_RADIUS_SALT,
    SOURCE_MINOR_RADIUS_SALT, SOURCE_SUMMIT_CAP_SALT, SOURCE_SUMMIT_HEIGHT_SALT,
    SOURCE_SUMMIT_PROFILE_SALT,
};
use super::super::super::lerp_f32;

const CLUSTER_ASSIGN_RADIUS_MULTIPLIER: f32 = 2.00;
const CLUSTER_BLOB_BOUND_PAD_BLOCKS: f32 = 10.0;
const CLUSTER_SHOULDER_BOUND_PAD_BLOCKS: f32 = 8.0;
const REGION_RESOLVE_PADDING_REGIONS: i32 = 1;
const WINDOW_OWNER_PADDING_REGIONS: i32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct HillClusterWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    clusters: Vec<ResolvedHillCluster>,
}

#[derive(Debug, Clone)]
struct ResolvedHillCluster {
    peak_height_hint: f32,
    raise_cap_blocks: f32,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    envelope: ResolvedClusterEnvelope,
    blobs: Vec<ResolvedHillBlob>,
    shoulders: Vec<ResolvedHillShoulder>,
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
struct ResolvedHillShoulder {
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
    strength_scale: f32,
    half_extent_x: f32,
    half_extent_z: f32,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedClusterEnvelope {
    seed_source: GuideSource,
    center_x: f32,
    center_z: f32,
    heading_x: f32,
    heading_z: f32,
    radius_x_blocks: f32,
    radius_z_blocks: f32,
    fill_scale: f32,
    shoulder_scale: f32,
    half_extent_x: f32,
    half_extent_z: f32,
}

#[derive(Debug, Clone)]
struct ClusterBuilder {
    anchor: GuideSource,
    members: Vec<GuideSource>,
}

pub(crate) fn build_window(guides: &MesoGuideMap, chunk: ChunkCoord) -> HillClusterWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let chunk_bounds = chunk_bounds(chunk, 0.0);
    let candidates = collect_candidate_sources(guides, meso_span_blocks);
    let (min_region_x, max_region_x, min_region_z, max_region_z) =
        owner_region_range_for_chunk(chunk, WINDOW_OWNER_PADDING_REGIONS);
    let mut clusters = Vec::new();

    for region_z in min_region_z..=max_region_z {
        for region_x in min_region_x..=max_region_x {
            clusters.extend(resolve_owned_clusters(
                &candidates,
                MesoRegionCoord::new(region_x, region_z),
            ));
        }
    }

    clusters.retain(|cluster| {
        bounds_overlap(chunk_bounds, cluster.min_x, cluster.max_x, cluster.min_z, cluster.max_z)
    });

    HillClusterWindow { chunk, clusters }
}

fn collect_candidate_sources(guides: &MesoGuideMap, meso_span_blocks: f32) -> Vec<GuideSource> {
    let mut candidates = Vec::new();

    for coord in guides.area().coords() {
        let Some(cell) = guides.cells().get(coord).copied() else {
            continue;
        };
        if !is_local_source_peak(guides, coord, cell) {
            continue;
        }
        let Some(source) = guide_source(coord, cell, meso_span_blocks) else {
            continue;
        };
        candidates.push(source);
    }

    candidates.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    candidates
}

fn resolve_owned_clusters(
    candidates: &[GuideSource],
    owner_region: MesoRegionCoord,
) -> Vec<ResolvedHillCluster> {
    let search_bounds = guide_region_bounds(owner_region, REGION_RESOLVE_PADDING_REGIONS);
    let mut builders: Vec<ClusterBuilder> = Vec::new();

    for candidate in candidates
        .iter()
        .copied()
        .filter(|candidate| bounds_contains_coord(search_bounds, candidate.coord))
    {
        if let Some(index) = assign_cluster_index(&builders, candidate) {
            builders[index].members.push(candidate);
        } else {
            builders.push(ClusterBuilder {
                anchor: candidate,
                members: vec![candidate],
            });
        }
    }

    builders
        .into_iter()
        .filter(|builder| owner_region_for_coord(builder.anchor.coord) == owner_region)
        .filter_map(resolve_cluster)
        .collect::<Vec<_>>()
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
    let mut raise_cap_weighted_sum = 0.0_f32;
    let mut raise_cap_weight = 0.0_f32;

    for cluster in &window.clusters {
        if sample_x < cluster.min_x
            || sample_x > cluster.max_x
            || sample_z < cluster.min_z
            || sample_z > cluster.max_z
        {
            continue;
        }

        let mut cluster_strongest = 0.0_f32;
        let mut cluster_second = 0.0_f32;
        let mut cluster_third = 0.0_f32;
        let mut cluster_coverage = 0.0_f32;
        let mut cluster_shoulder_coverage = 0.0_f32;
        let mut cluster_raise_cap_weighted_sum = 0.0_f32;
        let mut cluster_raise_cap_weight = 0.0_f32;

        for shoulder in &cluster.shoulders {
            if (sample_x - shoulder.center_x).abs() > shoulder.half_extent_x
                || (sample_z - shoulder.center_z).abs() > shoulder.half_extent_z
            {
                continue;
            }

            let delta_x = sample_x - shoulder.center_x;
            let delta_z = sample_z - shoulder.center_z;
            let along = delta_x * shoulder.heading_x + delta_z * shoulder.heading_z;
            let across = delta_x * -shoulder.heading_z + delta_z * shoulder.heading_x;
            let footprint = ellipse_footprint(
                along,
                across,
                shoulder.radius_x_blocks,
                shoulder.radius_z_blocks,
            );
            cluster_shoulder_coverage = coverage_union(
                cluster_shoulder_coverage,
                (footprint * shoulder.strength_scale).clamp(0.0, 0.90),
            );
        }

        if (sample_x - cluster.envelope.center_x).abs() <= cluster.envelope.half_extent_x
            && (sample_z - cluster.envelope.center_z).abs() <= cluster.envelope.half_extent_z
        {
            let footprint = cluster_envelope_footprint(cluster.envelope, sample_x, sample_z);
            cluster_shoulder_coverage = coverage_union(
                cluster_shoulder_coverage,
                (footprint * cluster.envelope.shoulder_scale).clamp(0.0, 0.96),
            );
        }

        for blob in &cluster.blobs {
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
                &mut cluster_strongest,
                &mut cluster_second,
                &mut cluster_third,
            );
            cluster_coverage = coverage_union(
                cluster_coverage,
                (footprint * blob.core_scale).clamp(0.0, 1.0),
            );
            cluster_shoulder_coverage = coverage_union(
                cluster_shoulder_coverage,
                (footprint * 0.18).clamp(0.0, 0.72),
            );
            let cap_weight = contribution.max(footprint * blob.lobe.height_blocks * 0.18);
            cluster_raise_cap_weighted_sum += blob.raise_cap_blocks * cap_weight;
            cluster_raise_cap_weight += cap_weight;
        }

        if cluster_strongest <= f32::EPSILON && cluster_shoulder_coverage <= f32::EPSILON {
            continue;
        }

        let envelope_footprint = if (sample_x - cluster.envelope.center_x).abs() <= cluster.envelope.half_extent_x
            && (sample_z - cluster.envelope.center_z).abs() <= cluster.envelope.half_extent_z
        {
            cluster_envelope_footprint(cluster.envelope, sample_x, sample_z)
        } else {
            0.0
        };
        let merge_support = smoothstep_range(
            0.10,
            0.76,
            cluster_coverage.max(envelope_footprint).max(cluster_shoulder_coverage),
        );
        let interior_fill_fade =
            1.0 - smoothstep_range(0.26, 0.84, cluster_coverage.max(cluster_strongest / cluster.peak_height_hint.max(1.0)));
        let cluster_fill = cluster.peak_height_hint
            * envelope_footprint
            * cluster.envelope.fill_scale
            * interior_fill_fade
            * (0.20 + merge_support * 0.34 + cluster_shoulder_coverage * 0.18);
        let second_weight = lerp_f32(0.40, 0.74, merge_support);
        let third_weight = lerp_f32(0.10, 0.24, merge_support);
        let cluster_contribution = cluster_strongest
            + cluster_second * second_weight
            + cluster_third * third_weight
            + cluster_fill;
        let cluster_raise_cap = if cluster_raise_cap_weight > f32::EPSILON {
            cluster_raise_cap_weighted_sum / cluster_raise_cap_weight
        } else {
            cluster.raise_cap_blocks
        };
        let cluster_cap_presence =
            cluster_contribution.max(envelope_footprint * cluster.peak_height_hint * 0.18);

        insert_top3(cluster_contribution, &mut strongest, &mut second, &mut third);
        coverage = coverage_union(
            coverage,
            (cluster_coverage * 0.84 + envelope_footprint * 0.24).clamp(0.0, 1.0),
        );
        shoulder_coverage = coverage_union(
            shoulder_coverage,
            cluster_shoulder_coverage.max(cluster_coverage * 0.92 + envelope_footprint * 0.12),
        );
        raise_cap_weighted_sum += cluster_raise_cap * cluster_cap_presence.max(0.0);
        raise_cap_weight += cluster_cap_presence.max(0.0);
    }

    if strongest <= f32::EPSILON {
        return HillClusterApplySample::default();
    }

    HillClusterApplySample {
        coverage: coverage.clamp(0.0, 1.0),
        shoulder_coverage: shoulder_coverage.max(coverage).clamp(0.0, 1.0),
        lobe_height_blocks: strongest + second * 0.66 + third * 0.30,
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
    if shoulder <= f32::EPSILON || apply.lobe_height_blocks <= f32::EPSILON {
        return HillClusterSurfaceSample::flat(base_surface_y);
    }

    let meso = sample_meso_guides(guides, world_x, world_z);
    let shoulder_raise = meso.hill_height
        * (1.34 + meso.hilliness * 0.52 + shoulder * 0.60)
        * smoothstep_range(0.04, 1.02, shoulder);
    let core_raise = apply.lobe_height_blocks
        * (2.26 + meso.hilliness * 0.38 + coverage * 0.24)
        * smoothstep_range(0.02, 0.98, coverage);
    let raw_target_raise =
        shoulder_raise * (1.10 + shoulder * 0.26) + core_raise * (1.12 + coverage * 0.14);
    let raise_cap = apply
        .peak_raise_cap_blocks
        .max((relief_budget * 1.08).max(16.0))
        .min((relief_budget * 3.20).max(52.0));
    let target_raise = soft_cap_positive(raw_target_raise, raise_cap);
    if target_raise <= f32::EPSILON {
        return HillClusterSurfaceSample::flat(base_surface_y);
    }

    let blend_weight = smoothstep01(
        (shoulder * 0.92 + coverage * 0.44 + meso.hilliness * 0.18).clamp(0.0, 1.0),
    );

    HillClusterSurfaceSample {
        target_surface_y: base_surface_y + target_raise,
        blend_weight,
        relief_spend: target_raise * blend_weight,
        core_coverage: coverage,
        shoulder_coverage: shoulder,
    }
}

fn assign_cluster_index(builders: &[ClusterBuilder], candidate: GuideSource) -> Option<usize> {
    let mut best_index = None;
    let mut best_distance = f32::INFINITY;

    for (index, builder) in builders.iter().enumerate() {
        let distance = super::distance_between_points(
            (candidate.center_x, candidate.center_z),
            (builder.anchor.center_x, builder.anchor.center_z),
        );
        let merge_radius = builder
            .anchor
            .keepout_radius_blocks
            .max(candidate.keepout_radius_blocks)
            * CLUSTER_ASSIGN_RADIUS_MULTIPLIER;
        if distance <= merge_radius && distance < best_distance {
            best_index = Some(index);
            best_distance = distance;
        }
    }

    best_index
}

fn resolve_cluster(builder: ClusterBuilder) -> Option<ResolvedHillCluster> {
    let anchor = builder.anchor;
    let sources = prune_cluster_sources(builder.members);
    if sources.is_empty() {
        return None;
    }

    let cluster_heading = dominant_apply_axis(&sources);
    let (centroid_x, centroid_z) = weighted_centroid(&sources);
    let mut blobs = Vec::new();
    let mut shoulders = Vec::new();
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    let cluster_peak_scale = lerp_f32(
        0.72,
        1.72,
        lobe_hash01(anchor.coord, anchor.cell, SOURCE_SUMMIT_HEIGHT_SALT),
    ) * (0.90 + anchor.cell.hilliness * 0.12 + (anchor.cell.hill_height / 20.0).clamp(0.0, 0.16));

    for source in &sources {
        let (heading_x, heading_z) = source_chain_heading(*source, cluster_heading);
        let normal_x = -heading_z;
        let normal_z = heading_x;
        let major_scale =
            (0.94 + source.cell.hilliness * 0.22 + (source.cell.hill_height / 16.0).clamp(0.0, 0.36))
                .clamp(0.94, 1.52);
        let minor_scale =
            (0.92 + source.cell.hilliness * 0.18 + (source.cell.hill_height / 20.0).clamp(0.0, 0.20))
                .clamp(0.92, 1.26);
        let base_major = lerp_f32(
            38.0,
            62.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
        ) * major_scale;
        let base_minor = lerp_f32(
            28.0,
            46.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
        ) * minor_scale;
        let chain_spacing = lerp_f32(
            18.0,
            28.0,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT),
        ) * (1.02 + source.cell.hilliness * 0.18);
        let lobe_count = if lobe_hash01(source.coord, source.cell, SOURCE_COUNT_SALT) >= 0.84
            && source.cell.hilliness >= 0.92
            && source.cell.hill_height >= 13.0
        {
            3
        } else {
            2
        };
        let chain_span = chain_spacing * (lobe_count - 1) as f32;
        let shoulder = resolve_shoulder(*source, heading_x, heading_z, base_major, base_minor, chain_span);
        include_bounds(
            &mut min_x,
            &mut max_x,
            &mut min_z,
            &mut max_z,
            shoulder.center_x,
            shoulder.center_z,
            shoulder.half_extent_x,
            shoulder.half_extent_z,
        );
        shoulders.push(shoulder);

        for lobe_index in 0..lobe_count {
            let progress = if lobe_count <= 1 {
                0.5
            } else {
                lobe_index as f32 / (lobe_count - 1) as f32
            };
            let center_bias = 1.0 - (progress * 2.0 - 1.0).abs();
            let mut lobe = macro_lobe_descriptor(
                *source,
                heading_x,
                heading_z,
                normal_x,
                normal_z,
                base_major,
                base_minor,
                chain_span,
                chain_spacing,
                lobe_index,
                progress,
                center_bias,
            );
            let summit_scale =
                lerp_f32(0.78, 1.42, lobe_hash01(source.coord, source.cell, SOURCE_SUMMIT_HEIGHT_SALT));
            let summit_bonus = source.cell.hill_height
                * lerp_f32(0.00, 0.32, lobe_hash01(source.coord, source.cell, SOURCE_SUMMIT_HEIGHT_SALT));
            lobe.height_blocks = lobe.height_blocks * cluster_peak_scale * summit_scale + summit_bonus;
            let (half_extent_x, half_extent_z) = blob_half_extents(lobe);
            include_bounds(
                &mut min_x,
                &mut max_x,
                &mut min_z,
                &mut max_z,
                lobe.center_x,
                lobe.center_z,
                half_extent_x,
                half_extent_z,
            );
            blobs.push(ResolvedHillBlob {
                lobe,
                source: *source,
                lobe_index,
                core_scale: (0.38 + source.cell.hilliness * 0.46).clamp(0.0, 1.0),
                peak_exponent: lerp_f32(
                    1.18,
                    1.92,
                    indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SUMMIT_PROFILE_SALT),
                ),
                raise_cap_blocks: (lobe.height_blocks
                    * lerp_f32(
                        1.08,
                        1.78,
                        indexed_lobe_hash01(source.coord, source.cell, lobe_index, SOURCE_SUMMIT_CAP_SALT),
                    ))
                .clamp(18.0, 48.0),
                half_extent_x,
                half_extent_z,
            });
        }
    }

    if blobs.is_empty() {
        return None;
    }

    let envelope = resolve_cluster_envelope(anchor, cluster_heading, centroid_x, centroid_z, &blobs);
    include_bounds(
        &mut min_x,
        &mut max_x,
        &mut min_z,
        &mut max_z,
        envelope.center_x,
        envelope.center_z,
        envelope.half_extent_x,
        envelope.half_extent_z,
    );

    Some(ResolvedHillCluster {
        peak_height_hint: blobs
            .iter()
            .map(|blob| blob.lobe.height_blocks)
            .fold(0.0_f32, f32::max),
        raise_cap_blocks: lerp_f32(
            20.0,
            44.0,
            lobe_hash01(anchor.coord, anchor.cell, SOURCE_SUMMIT_CAP_SALT),
        ),
        min_x,
        max_x,
        min_z,
        max_z,
        envelope,
        blobs,
        shoulders,
    })
}

fn resolve_shoulder(
    source: GuideSource,
    heading_x: f32,
    heading_z: f32,
    base_major: f32,
    base_minor: f32,
    chain_span: f32,
) -> ResolvedHillShoulder {
    let radius_x_blocks = base_major * 0.82 + chain_span * 0.68 + 8.0;
    let radius_z_blocks = base_minor * 1.34 + 10.0;
    let (half_extent_x, half_extent_z) =
        rotated_ellipse_half_extents(heading_x, heading_z, radius_x_blocks, radius_z_blocks);

    ResolvedHillShoulder {
        center_x: source.center_x,
        center_z: source.center_z,
        heading_x,
        heading_z,
        radius_x_blocks,
        radius_z_blocks,
        strength_scale: (0.14 + source.cell.hilliness * 0.22).clamp(0.0, 0.72),
        half_extent_x: half_extent_x + CLUSTER_SHOULDER_BOUND_PAD_BLOCKS,
        half_extent_z: half_extent_z + CLUSTER_SHOULDER_BOUND_PAD_BLOCKS,
    }
}

fn resolve_cluster_envelope(
    seed_source: GuideSource,
    cluster_heading: (f32, f32),
    center_x: f32,
    center_z: f32,
    blobs: &[ResolvedHillBlob],
) -> ResolvedClusterEnvelope {
    let mut along_extent = 0.0_f32;
    let mut across_extent = 0.0_f32;

    for blob in blobs {
        let delta_x = blob.lobe.center_x - center_x;
        let delta_z = blob.lobe.center_z - center_z;
        let along = delta_x * cluster_heading.0 + delta_z * cluster_heading.1;
        let across = delta_x * -cluster_heading.1 + delta_z * cluster_heading.0;
        along_extent = along_extent.max(along.abs() + blob.lobe.radius_x_blocks * 0.62);
        across_extent = across_extent.max(across.abs() + blob.lobe.radius_z_blocks * 0.92);
    }

    let radius_scale = lerp_f32(
        1.00,
        1.16,
        lobe_hash01(seed_source.coord, seed_source.cell, SOURCE_ENVELOPE_RADIUS_SALT),
    );
    let radius_x_blocks = along_extent.max(24.0) * radius_scale + 10.0;
    let radius_z_blocks = across_extent.max(20.0) * (radius_scale * 1.04) + 8.0;
    let (half_extent_x, half_extent_z) = rotated_ellipse_half_extents(
        cluster_heading.0,
        cluster_heading.1,
        radius_x_blocks + CLUSTER_BLOB_BOUND_PAD_BLOCKS,
        radius_z_blocks + CLUSTER_BLOB_BOUND_PAD_BLOCKS,
    );

    ResolvedClusterEnvelope {
        seed_source,
        center_x,
        center_z,
        heading_x: cluster_heading.0,
        heading_z: cluster_heading.1,
        radius_x_blocks,
        radius_z_blocks,
        fill_scale: lerp_f32(
            0.14,
            0.28,
            lobe_hash01(seed_source.coord, seed_source.cell, SOURCE_ENVELOPE_FILL_SALT),
        ),
        shoulder_scale: lerp_f32(
            0.26,
            0.46,
            lobe_hash01(seed_source.coord, seed_source.cell, SOURCE_ENVELOPE_SHOULDER_SALT),
        ),
        half_extent_x,
        half_extent_z,
    }
}

fn weighted_centroid(sources: &[GuideSource]) -> (f32, f32) {
    let mut total_weight = 0.0_f32;
    let mut center_x = 0.0_f32;
    let mut center_z = 0.0_f32;

    for source in sources {
        total_weight += source.weight;
        center_x += source.center_x * source.weight;
        center_z += source.center_z * source.weight;
    }

    if total_weight <= f32::EPSILON {
        return (0.0, 0.0);
    }

    (center_x / total_weight, center_z / total_weight)
}

fn blob_half_extents(lobe: MacroLobeDescriptor) -> (f32, f32) {
    let radius_x = lobe.radius_x_blocks * 1.26 + CLUSTER_BLOB_BOUND_PAD_BLOCKS;
    let radius_z = lobe.radius_z_blocks * 1.32 + CLUSTER_BLOB_BOUND_PAD_BLOCKS;
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

fn cluster_envelope_footprint(
    envelope: ResolvedClusterEnvelope,
    sample_x: f32,
    sample_z: f32,
) -> f32 {
    let delta_x = sample_x - envelope.center_x;
    let delta_z = sample_z - envelope.center_z;
    let along = delta_x * envelope.heading_x + delta_z * envelope.heading_z;
    let across = delta_x * -envelope.heading_z + delta_z * envelope.heading_x;
    let normalized_along = along / envelope.radius_x_blocks.max(f32::EPSILON);
    let normalized_across = across / envelope.radius_z_blocks.max(f32::EPSILON);
    let contour_angle = normalized_across.atan2(normalized_along);
    let contour_scale = 1.0
        + (contour_angle * 2.0
            + lobe_hash01(envelope.seed_source.coord, envelope.seed_source.cell, SOURCE_ENVELOPE_RADIUS_SALT)
                * std::f32::consts::TAU)
            .sin()
            * 0.16
        + (contour_angle * 3.0
            + lobe_hash01(
                envelope.seed_source.coord,
                envelope.seed_source.cell,
                SOURCE_ENVELOPE_FILL_SALT,
            ) * std::f32::consts::TAU)
            .sin()
            * 0.11
        + (contour_angle * 5.0
            + lobe_hash01(
                envelope.seed_source.coord,
                envelope.seed_source.cell,
                SOURCE_ENVELOPE_SHOULDER_SALT,
            ) * std::f32::consts::TAU)
            .cos()
            * 0.07;
    let warped_along = along
        + (((across / envelope.radius_z_blocks.max(1.0)) * 1.18)
            + lobe_hash01(
                envelope.seed_source.coord,
                envelope.seed_source.cell,
                SOURCE_ENVELOPE_FILL_SALT,
            ) * std::f32::consts::TAU)
            .sin()
            * envelope.radius_x_blocks
            * 0.07;
    let warped_across = across
        + (((along / envelope.radius_x_blocks.max(1.0)) * 1.42)
            + lobe_hash01(
                envelope.seed_source.coord,
                envelope.seed_source.cell,
                SOURCE_ENVELOPE_SHOULDER_SALT,
            ) * std::f32::consts::TAU)
            .sin()
            * envelope.radius_z_blocks
            * 0.08;
    let radial = ((warped_along / envelope.radius_x_blocks.max(f32::EPSILON)).powi(2)
        + (warped_across / envelope.radius_z_blocks.max(f32::EPSILON)).powi(2))
    .sqrt()
        / contour_scale.max(0.66);
    let outer = smoothstep_range(1.06, 0.0, radial);
    let inner = smoothstep_range(0.76, 0.0, radial);
    (outer * 0.68 + inner * 0.32).clamp(0.0, 1.0)
}

fn summit_profile(footprint: f32, exponent: f32) -> f32 {
    if footprint <= f32::EPSILON {
        return 0.0;
    }

    let exponent = exponent.max(1.0);
    let t = footprint.clamp(0.0, 1.0);
    let base = smoothstep01(t);
    let mid = smoothstep01(base);
    let shoulder = base.powf(0.92 + (exponent - 1.0) * 0.16);
    let dome = mid.powf(1.12 + (exponent - 1.0) * 0.60);
    let apex = smoothstep_range(0.80, 1.0, t);
    (shoulder * 0.42 + dome * 0.42 + apex * 0.16).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::summit_profile;

    #[test]
    fn summit_profile_keeps_sigmoid_upper_dome() {
        let lower_mid = summit_profile(0.60, 1.6);
        let upper_mid = summit_profile(0.85, 1.6);
        let apex_band = summit_profile(0.97, 1.6);

        assert!(
            upper_mid - lower_mid >= 0.24,
            "expected hill sides to steepen meaningfully through the mid band, lower_mid={lower_mid:.3}, upper_mid={upper_mid:.3}"
        );
        assert!(
            apex_band - upper_mid >= 0.10,
            "expected summit profile to keep gaining into the apex instead of flattening too early, upper_mid={upper_mid:.3}, apex_band={apex_band:.3}"
        );
    }
}

fn coverage_union(current: f32, addition: f32) -> f32 {
    let current = current.clamp(0.0, 1.0);
    let addition = addition.clamp(0.0, 1.0);
    (1.0 - (1.0 - current) * (1.0 - addition)).clamp(0.0, 1.0)
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
