use crate::world::atlas::{MesoGuideMap, sample_meso_guides};
use crate::world::coord::ChunkCoord;

use super::{
    GuideSource, HillClusterApplySample, HillClusterSurfaceSample, MacroLobeDescriptor,
    dominant_apply_axis, ellipse_footprint, guide_source, insert_top3, irregular_lobe_footprint,
    is_local_source_peak, lobe_hash01, macro_lobe_descriptor, prune_cluster_sources, smoothstep01,
    smoothstep_range, soft_cap_positive, source_chain_heading,
};
use super::{
    SOURCE_CHAIN_SPACING_SALT, SOURCE_COUNT_SALT, SOURCE_MAJOR_RADIUS_SALT, SOURCE_MINOR_RADIUS_SALT,
};
use super::super::super::lerp_f32;

const CLUSTER_ASSIGN_RADIUS_MULTIPLIER: f32 = 2.35;
const CLUSTER_BLOB_BOUND_PAD_BLOCKS: f32 = 18.0;
const CLUSTER_SHOULDER_BOUND_PAD_BLOCKS: f32 = 12.0;

#[derive(Debug, Clone)]
pub(crate) struct HillClusterWindow {
    #[allow(dead_code)]
    chunk: ChunkCoord,
    clusters: Vec<ResolvedHillCluster>,
}

#[derive(Debug, Clone)]
struct ResolvedHillCluster {
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    blobs: Vec<ResolvedHillBlob>,
    shoulders: Vec<ResolvedHillShoulder>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedHillBlob {
    lobe: MacroLobeDescriptor,
    source: GuideSource,
    lobe_index: usize,
    core_scale: f32,
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

#[derive(Debug, Clone)]
struct ClusterBuilder {
    anchor: GuideSource,
    members: Vec<GuideSource>,
}

pub(crate) fn build_window(guides: &MesoGuideMap, chunk: ChunkCoord) -> HillClusterWindow {
    let meso_span_blocks =
        (crate::world::CHUNK_EDGE_I32 * crate::world::MESO_GUIDE_CELL_SIZE_IN_CHUNKS as i32) as f32;
    let source_padding_blocks = (super::APPLY_SCAN_RADIUS_CELLS as f32 + 1.0) * meso_span_blocks;
    let source_bounds = chunk_bounds(chunk, source_padding_blocks);
    let chunk_bounds = chunk_bounds(chunk, 0.0);
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
        if !bounds_contains_point(source_bounds, source.center_x, source.center_z) {
            continue;
        }
        candidates.push(source);
    }

    candidates.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    let mut builders: Vec<ClusterBuilder> = Vec::new();

    for candidate in candidates {
        if let Some(index) = assign_cluster_index(&builders, candidate) {
            builders[index].members.push(candidate);
        } else {
            builders.push(ClusterBuilder {
                anchor: candidate,
                members: vec![candidate],
            });
        }
    }

    let clusters = builders
        .into_iter()
        .filter_map(resolve_cluster)
        .filter(|cluster| {
            bounds_overlap(chunk_bounds, cluster.min_x, cluster.max_x, cluster.min_z, cluster.max_z)
        })
        .collect::<Vec<_>>();

    HillClusterWindow { chunk, clusters }
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

    for cluster in &window.clusters {
        if sample_x < cluster.min_x
            || sample_x > cluster.max_x
            || sample_z < cluster.min_z
            || sample_z > cluster.max_z
        {
            continue;
        }

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
            shoulder_coverage = shoulder_coverage.max(
                (footprint * shoulder.strength_scale).clamp(0.0, 0.90),
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

            let contribution = blob.lobe.height_blocks * footprint;
            insert_top3(contribution, &mut strongest, &mut second, &mut third);
            coverage = coverage.max((footprint * blob.core_scale).clamp(0.0, 1.0));
            shoulder_coverage = shoulder_coverage.max((footprint * 0.24).clamp(0.0, 1.0));
        }
    }

    if strongest <= f32::EPSILON {
        return HillClusterApplySample::default();
    }

    HillClusterApplySample {
        coverage: coverage.clamp(0.0, 1.0),
        shoulder_coverage: shoulder_coverage.max(coverage).clamp(0.0, 1.0),
        lobe_height_blocks: strongest + second * 0.66 + third * 0.30,
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
        * (1.52 + meso.hilliness * 0.62 + shoulder * 0.72)
        * smoothstep_range(0.03, 0.98, shoulder);
    let core_raise = apply.lobe_height_blocks
        * (2.82 + meso.hilliness * 0.56 + coverage * 0.54)
        * smoothstep_range(0.02, 0.90, coverage);
    let raw_target_raise =
        shoulder_raise * (1.18 + shoulder * 0.40) + core_raise * (1.34 + coverage * 0.28);
    let target_raise = soft_cap_positive(raw_target_raise, (relief_budget * 2.35).max(26.0));
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
    let sources = prune_cluster_sources(builder.members);
    if sources.is_empty() {
        return None;
    }

    let cluster_heading = dominant_apply_axis(&sources);
    let mut blobs = Vec::new();
    let mut shoulders = Vec::new();
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;

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
            52.0,
            86.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MAJOR_RADIUS_SALT),
        ) * major_scale;
        let base_minor = lerp_f32(
            38.0,
            62.0,
            lobe_hash01(source.coord, source.cell, SOURCE_MINOR_RADIUS_SALT),
        ) * minor_scale;
        let chain_spacing = lerp_f32(
            24.0,
            38.0,
            lobe_hash01(source.coord, source.cell, SOURCE_CHAIN_SPACING_SALT),
        ) * (1.02 + source.cell.hilliness * 0.18);
        let lobe_count = if lobe_hash01(source.coord, source.cell, SOURCE_COUNT_SALT) >= 0.70
            || source.cell.hilliness >= 0.86
            || source.cell.hill_height >= 12.4
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
            let lobe = macro_lobe_descriptor(
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
                half_extent_x,
                half_extent_z,
            });
        }
    }

    if blobs.is_empty() {
        return None;
    }

    Some(ResolvedHillCluster {
        min_x,
        max_x,
        min_z,
        max_z,
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
    let radius_x_blocks = base_major + chain_span * 1.16 + 12.0;
    let radius_z_blocks = base_minor * 2.18 + 16.0;
    let (half_extent_x, half_extent_z) =
        rotated_ellipse_half_extents(heading_x, heading_z, radius_x_blocks, radius_z_blocks);

    ResolvedHillShoulder {
        center_x: source.center_x,
        center_z: source.center_z,
        heading_x,
        heading_z,
        radius_x_blocks,
        radius_z_blocks,
        strength_scale: (0.20 + source.cell.hilliness * 0.32).clamp(0.0, 0.90),
        half_extent_x: half_extent_x + CLUSTER_SHOULDER_BOUND_PAD_BLOCKS,
        half_extent_z: half_extent_z + CLUSTER_SHOULDER_BOUND_PAD_BLOCKS,
    }
}

fn blob_half_extents(lobe: MacroLobeDescriptor) -> (f32, f32) {
    let radius_x = lobe.radius_x_blocks * 1.46 + CLUSTER_BLOB_BOUND_PAD_BLOCKS;
    let radius_z = lobe.radius_z_blocks * 1.58 + CLUSTER_BLOB_BOUND_PAD_BLOCKS;
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

fn bounds_contains_point(bounds: (f32, f32, f32, f32), x: f32, z: f32) -> bool {
    x >= bounds.0 && x <= bounds.1 && z >= bounds.2 && z <= bounds.3
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
