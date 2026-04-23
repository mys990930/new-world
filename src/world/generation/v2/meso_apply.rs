use crate::world::atlas::{
    HillClusterSurfaceSample, RiverPathKind, build_hill_cluster_window, region_archetype_def,
    sample_hill_cluster_surface_from_window, sample_meso_guides,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::{
    BaseHeightfieldPrototype, ChunkCorridorWindow, ChunkGenerationV2Inputs, RegionSampleWeight,
    RiverCorridorConstraint, sample_region_weights,
};

const MAX_FEATURE_SPEND_FRACTION: f32 = 0.72;
const MIN_COLUMN_RELIEF_SCALE: f32 = 0.30;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MesoAppliedColumn {
    pub height: f32,
    pub remaining_relief_budget: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MesoAppliedPrototype {
    pub chunk: ChunkCoord,
    pub columns: Vec<MesoAppliedColumn>,
    pub applied_feature_keys: Vec<&'static str>,
}

pub fn empty_meso_applied_prototype(chunk: ChunkCoord) -> MesoAppliedPrototype {
    MesoAppliedPrototype {
        chunk,
        columns: Vec::new(),
        applied_feature_keys: Vec::new(),
    }
}

pub fn build_chunk_meso_applied_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
    prototype: &BaseHeightfieldPrototype,
) -> MesoAppliedPrototype {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    debug_assert_eq!(prototype.chunk, chunk);

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity(prototype.columns.len());
    let mut applied_hill_cluster = false;
    let mut applied_shallow_basin = false;
    let mut applied_escarpment_band = false;
    let mut applied_upland_terrace = false;
    let hill_cluster_window = build_hill_cluster_window(&inputs.meso_guides, chunk);

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize;
            let base = prototype.columns[index];
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let meso = sample_meso_guides(&inputs.meso_guides, world_x, world_z);
            let region_samples =
                sample_region_weights(&inputs.region_classes, world_x as f32 + 0.5, world_z as f32 + 0.5);
            let corridor_avoidance = corridor_avoidance_factor(
                local_x as f32 + 0.5,
                local_z as f32 + 0.5,
                &corridor_window.corridors,
            );
            let column_relief_scale = relief_budget_scale(base.relief_budget);
            let hill_cluster_allowed = allowed_feature_weight(&region_samples, "hill_cluster");
            let hill_cluster_surface = sample_hill_cluster_surface_from_window(
                &hill_cluster_window,
                &inputs.meso_guides,
                world_x,
                world_z,
                base.base_height,
                base.relief_budget,
            );

            let hill_cluster = hill_cluster_surface_delta(
                base.base_height,
                hill_cluster_surface,
                hill_cluster_allowed,
                corridor_avoidance,
            );
            let basin_hill_conflict = (hill_cluster_surface.core_coverage * 0.82
                + hill_cluster_surface.shoulder_coverage * 0.46)
                .clamp(0.0, 0.92);
            let shallow_basin = shallow_basin_delta(
                &meso,
                allowed_feature_weight(&region_samples, "shallow_basin"),
                column_relief_scale,
                corridor_avoidance,
            ) * (1.0_f32 - basin_hill_conflict * 0.84).clamp(0.22, 1.0);
            let escarpment_band = escarpment_band_delta(
                &meso,
                allowed_feature_weight(&region_samples, "escarpment_band"),
                column_relief_scale,
                corridor_avoidance,
            );
            let upland_terrace = upland_terrace_delta(
                &meso,
                allowed_feature_weight(&region_samples, "upland_terrace"),
                column_relief_scale,
                corridor_avoidance,
            );

            let unclamped_delta = hill_cluster + shallow_basin + escarpment_band + upland_terrace;
            let max_raise = base.relief_budget
                * hill_cluster_raise_cap_fraction(hill_cluster_surface)
                    .max(MAX_FEATURE_SPEND_FRACTION);
            let max_lower = base.relief_budget * MAX_FEATURE_SPEND_FRACTION;
            let applied_delta = unclamped_delta.clamp(-max_lower, max_raise);
            let spent_relief = applied_delta.abs().min(base.relief_budget * 0.86);
            let remaining_relief_budget = (base.relief_budget - spent_relief).max(0.0);

            applied_hill_cluster |= hill_cluster.abs() >= 0.10;
            applied_shallow_basin |= shallow_basin.abs() >= 0.10;
            applied_escarpment_band |= escarpment_band.abs() >= 0.10;
            applied_upland_terrace |= upland_terrace.abs() >= 0.10;

            columns.push(MesoAppliedColumn {
                height: base.base_height + applied_delta,
                remaining_relief_budget,
            });
        }
    }

    let mut applied_feature_keys = Vec::new();
    if applied_hill_cluster {
        applied_feature_keys.push("hill_cluster");
    }
    if applied_shallow_basin {
        applied_feature_keys.push("shallow_basin");
    }
    if applied_escarpment_band {
        applied_feature_keys.push("escarpment_band");
    }
    if applied_upland_terrace {
        applied_feature_keys.push("upland_terrace");
    }

    MesoAppliedPrototype {
        chunk,
        columns,
        applied_feature_keys,
    }
}

fn relief_budget_scale(relief_budget: f32) -> f32 {
    (relief_budget / 20.0).clamp(MIN_COLUMN_RELIEF_SCALE, 1.0)
}

fn allowed_feature_weight(region_samples: &[RegionSampleWeight; 4], key: &str) -> f32 {
    let mut allowed = 0.0_f32;

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }

        let Some(def) = region_archetype_def(sample.cell.archetype) else {
            continue;
        };

        if def.allowed_meso_keys.contains(&key) {
            allowed += sample.weight;
        }
    }

    allowed.clamp(0.0, 1.0)
}

fn hill_cluster_surface_delta(
    base_height: f32,
    hill_cluster_surface: HillClusterSurfaceSample,
    allowed_weight: f32,
    corridor_avoidance: f32,
) -> f32 {
    let surface_weight = hill_cluster_surface.blend_weight.max(
        (hill_cluster_surface.shoulder_coverage * 0.44 + hill_cluster_surface.core_coverage * 0.54)
            .clamp(0.0, 0.88),
    );
    let corridor_weight = corridor_avoidance.clamp(0.0, 1.0).powf(0.5);
    let weight = surface_weight * allowed_weight * corridor_weight;
    if weight <= f32::EPSILON {
        return 0.0;
    }

    let blended_surface_y = lerp_f32(base_height, hill_cluster_surface.target_surface_y, weight);
    blended_surface_y - base_height
}

fn hill_cluster_raise_cap_fraction(hill_cluster_surface: HillClusterSurfaceSample) -> f32 {
    lerp_f32(
        MAX_FEATURE_SPEND_FRACTION,
        0.90,
        (hill_cluster_surface.core_coverage * 0.78
            + hill_cluster_surface.shoulder_coverage * 0.22)
            .clamp(0.0, 1.0),
    )
}

fn shallow_basin_delta(
    meso: &crate::world::atlas::MesoGuideSample,
    allowed_weight: f32,
    relief_scale: f32,
    corridor_avoidance: f32,
) -> f32 {
    let weight = meso.basin_weight * allowed_weight * corridor_avoidance.powf(1.35);
    if weight <= f32::EPSILON {
        return 0.0;
    }

    -meso.basin_depth * weight * relief_scale * 0.88
}

fn escarpment_band_delta(
    meso: &crate::world::atlas::MesoGuideSample,
    allowed_weight: f32,
    relief_scale: f32,
    corridor_avoidance: f32,
) -> f32 {
    let weight = meso.escarpment_weight * allowed_weight * corridor_avoidance;
    if weight <= f32::EPSILON {
        return 0.0;
    }

    let transition = smoothstep01((-meso.escarpment_signed_distance_cells / 0.78 + 0.5).clamp(0.0, 1.0));
    let signed_step = transition - 0.38;
    meso.escarpment_height * weight * relief_scale * signed_step * 0.92
}

fn upland_terrace_delta(
    meso: &crate::world::atlas::MesoGuideSample,
    allowed_weight: f32,
    relief_scale: f32,
    corridor_avoidance: f32,
) -> f32 {
    let weight = meso.terrace_weight * allowed_weight * corridor_avoidance;
    if weight <= f32::EPSILON {
        return 0.0;
    }

    let spacing = meso.terrace_spacing_cells.max(0.55);
    let scaled = -meso.terrace_signed_distance_cells / spacing;
    let base_step = scaled.floor();
    let frac = scaled - base_step;
    let smoothed_step = base_step + smoothstep01(frac);
    let centered_step = (smoothed_step + 0.5).clamp(-2.0, 2.0);

    meso.terrace_step_height * weight * relief_scale * centered_step * 0.34
}

fn corridor_avoidance_factor(
    local_x: f32,
    local_z: f32,
    corridors: &[RiverCorridorConstraint],
) -> f32 {
    let mut strongest = 0.0_f32;

    for corridor in corridors {
        let projected = project_point_onto_segment(
            (local_x, local_z),
            (corridor.start_x, corridor.start_z),
            (corridor.end_x, corridor.end_z),
        );
        let keepout_radius = corridor.half_width_blocks
            * match corridor.kind {
                RiverPathKind::Trunk => 1.85,
                RiverPathKind::Tributary => 1.45,
            }
            + 10.0;
        let distance_ratio = projected.distance_blocks / keepout_radius.max(f32::EPSILON);
        let influence = smoothstep_range(1.05, 0.0, distance_ratio)
            * match corridor.kind {
                RiverPathKind::Trunk => 1.0,
                RiverPathKind::Tributary => 0.72,
            };
        strongest = strongest.max(influence);
    }

    (1.0 - strongest).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
struct ProjectedSegmentPoint {
    distance_blocks: f32,
}

fn project_point_onto_segment(
    point: (f32, f32),
    start: (f32, f32),
    end: (f32, f32),
) -> ProjectedSegmentPoint {
    let seg_x = end.0 - start.0;
    let seg_z = end.1 - start.1;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return ProjectedSegmentPoint {
            distance_blocks: distance_between_points(point, start),
        };
    }

    let t = (((point.0 - start.0) * seg_x + (point.1 - start.1) * seg_z) / length_sq).clamp(0.0, 1.0);
    let projected = (start.0 + seg_x * t, start.1 + seg_z * t);

    ProjectedSegmentPoint {
        distance_blocks: distance_between_points(point, projected),
    }
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
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

fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::{MesoGuideSample, RegionArchetype, RegionClassCell};
    use crate::world::generation::v2::{
        build_chunk_base_heightfield_prototype, build_chunk_corridor_window,
        build_chunk_realization_field_patch, prepare_chunk_v2_inputs,
    };
    use crate::world::meta::WorldMeta;

    fn assert_hill_cluster_samples_match(
        left: HillClusterSurfaceSample,
        right: HillClusterSurfaceSample,
        world_x: i32,
        world_z: i32,
    ) {
        let epsilon = 0.0001_f32;
        assert!(
            (left.target_surface_y - right.target_surface_y).abs() <= epsilon
                && (left.blend_weight - right.blend_weight).abs() <= epsilon
                && (left.relief_spend - right.relief_spend).abs() <= epsilon
                && (left.core_coverage - right.core_coverage).abs() <= epsilon
                && (left.shoulder_coverage - right.shoulder_coverage).abs() <= epsilon,
            "resolved hill-cluster surface drifted at world ({world_x}, {world_z})\nleft={left:?}\nright={right:?}"
        );
    }

    #[test]
    fn meso_apply_is_deterministic_and_emits_a_full_grid() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let realization_field = build_chunk_realization_field_patch(chunk, &inputs);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let prototype = build_chunk_base_heightfield_prototype(
            chunk,
            &inputs,
            &realization_field,
            &corridor_window,
        );
        let a = build_chunk_meso_applied_prototype(chunk, &inputs, &corridor_window, &prototype);
        let b = build_chunk_meso_applied_prototype(chunk, &inputs, &corridor_window, &prototype);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert_eq!(a.columns.len(), prototype.columns.len());
        assert!(a.columns.iter().all(|column| {
            column.height.is_finite()
                && column.remaining_relief_budget.is_finite()
                && column.remaining_relief_budget >= 0.0
        }));
    }

    #[test]
    fn meso_apply_modulates_some_chunk_when_guides_are_present() {
        let meta = WorldMeta::new(42);
        let candidates = [
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(24, 0, -8),
            ChunkCoord(30, 0, -20),
        ];

        for chunk in candidates {
            let inputs = prepare_chunk_v2_inputs(chunk, &meta);
            let realization_field = build_chunk_realization_field_patch(chunk, &inputs);
            let corridor_window = build_chunk_corridor_window(chunk, &inputs);
            let prototype = build_chunk_base_heightfield_prototype(
                chunk,
                &inputs,
                &realization_field,
                &corridor_window,
            );
            let meso = build_chunk_meso_applied_prototype(chunk, &inputs, &corridor_window, &prototype);
            let has_delta = prototype
                .columns
                .iter()
                .zip(meso.columns.iter())
                .any(|(base, applied)| (applied.height - base.base_height).abs() >= 0.10);

            if has_delta {
                assert!(!meso.applied_feature_keys.is_empty());
                return;
            }
        }

        panic!("expected at least one sampled chunk to receive meso deformation");
    }

    #[test]
    fn allowed_feature_weight_uses_archetype_allowlists() {
        let mut temperate_plain = RegionClassCell::default();
        temperate_plain.archetype = RegionArchetype::TemperatePlain;
        let mut desert_dune = RegionClassCell::default();
        desert_dune.archetype = RegionArchetype::DesertDuneField;
        let region_samples = [
            RegionSampleWeight {
                cell: temperate_plain,
                weight: 0.60,
            },
            RegionSampleWeight {
                cell: desert_dune,
                weight: 0.40,
            },
            RegionSampleWeight {
                cell: RegionClassCell::default(),
                weight: 0.0,
            },
            RegionSampleWeight {
                cell: RegionClassCell::default(),
                weight: 0.0,
            },
        ];

        assert!((allowed_feature_weight(&region_samples, "hill_cluster") - 0.60).abs() <= 0.0001);
        assert_eq!(allowed_feature_weight(&region_samples, "upland_terrace"), 0.0);
    }

    #[test]
    fn corridor_avoidance_suppresses_nearby_columns() {
        let corridor = RiverCorridorConstraint {
            river_id: 1,
            kind: RiverPathKind::Trunk,
            order: 3,
            start_x: 0.0,
            start_z: 8.0,
            end_x: 16.0,
            end_z: 8.0,
            center_x: 8.0,
            center_z: 8.0,
            half_width_blocks: 6.0,
            downstream_grade_per_block: 0.002,
        };

        let near = corridor_avoidance_factor(8.0, 8.0, &[corridor]);
        let far = corridor_avoidance_factor(8.0, 30.0, &[corridor]);

        assert!(near < far);
        assert!(near < 0.55);
    }

    #[test]
    fn wave_one_feature_operators_emit_expected_directionality() {
        let meso = MesoGuideSample {
            hilliness: 0.8,
            hill_height: 7.0,
            basin_weight: 0.7,
            basin_depth: 3.0,
            escarpment_weight: 0.9,
            escarpment_height: 5.0,
            escarpment_signed_distance_cells: -0.25,
            terrace_weight: 0.8,
            terrace_step_height: 2.0,
            terrace_spacing_cells: 1.0,
            terrace_signed_distance_cells: -0.75,
            ..MesoGuideSample::default()
        };
        let hill_cluster_surface = HillClusterSurfaceSample {
            target_surface_y: 112.0,
            blend_weight: 0.84,
            relief_spend: 10.08,
            core_coverage: 0.82,
            shoulder_coverage: 0.93,
        };

        assert!(hill_cluster_surface_delta(100.0, hill_cluster_surface, 1.0, 1.0) > 0.0);
        assert!(shallow_basin_delta(&meso, 1.0, 1.0, 1.0) < 0.0);
        assert!(escarpment_band_delta(&meso, 1.0, 1.0, 1.0) > 0.0);
        assert!(upland_terrace_delta(&meso, 1.0, 1.0, 1.0) > 0.0);
    }

    #[test]
    fn hill_cluster_surface_delta_keeps_material_rise_at_partial_blend() {
        let hill_cluster_surface = HillClusterSurfaceSample {
            target_surface_y: 112.0,
            blend_weight: 0.70,
            relief_spend: 8.4,
            core_coverage: 0.68,
            shoulder_coverage: 0.86,
        };

        let delta = hill_cluster_surface_delta(100.0, hill_cluster_surface, 1.0, 1.0);
        assert!(
            delta >= 8.0,
            "expected partial-blend hill clusters to still raise terrain materially, got {delta}"
        );
    }

    #[test]
    fn hill_cluster_surface_delta_respects_external_gating() {
        let hill_cluster_surface = HillClusterSurfaceSample {
            target_surface_y: 113.5,
            blend_weight: 0.88,
            relief_spend: 11.88,
            core_coverage: 0.92,
            shoulder_coverage: 0.98,
        };

        let full = hill_cluster_surface_delta(100.0, hill_cluster_surface, 1.0, 1.0);
        let gated = hill_cluster_surface_delta(100.0, hill_cluster_surface, 0.45, 0.60);
        assert!(
            gated < full,
            "expected gating to attenuate feature-owned hill surfaces, full={full}, gated={gated}"
        );
    }

    #[test]
    fn hill_cluster_meso_apply_shared_edge_stays_close_to_neighboring_slope() {
        let meta = WorldMeta::new(42);
        let left_chunk = ChunkCoord(-467, 0, -396);
        let right_chunk = ChunkCoord(-466, 0, -396);
        let left_inputs = prepare_chunk_v2_inputs(left_chunk, &meta);
        let right_inputs = prepare_chunk_v2_inputs(right_chunk, &meta);
        let left_realization = build_chunk_realization_field_patch(left_chunk, &left_inputs);
        let right_realization = build_chunk_realization_field_patch(right_chunk, &right_inputs);
        let left_corridor = build_chunk_corridor_window(left_chunk, &left_inputs);
        let right_corridor = build_chunk_corridor_window(right_chunk, &right_inputs);
        let left_prototype = build_chunk_base_heightfield_prototype(
            left_chunk,
            &left_inputs,
            &left_realization,
            &left_corridor,
        );
        let right_prototype = build_chunk_base_heightfield_prototype(
            right_chunk,
            &right_inputs,
            &right_realization,
            &right_corridor,
        );
        let left_meso = build_chunk_meso_applied_prototype(
            left_chunk,
            &left_inputs,
            &left_corridor,
            &left_prototype,
        );
        let right_meso = build_chunk_meso_applied_prototype(
            right_chunk,
            &right_inputs,
            &right_corridor,
            &right_prototype,
        );
        let edge = CHUNK_EDGE_I32 as usize;

        for row in 0..edge {
            let left_edge = left_meso.columns[row * edge + (edge - 1)].height;
            let left_inner = left_meso.columns[row * edge + (edge - 2)].height;
            let right_edge = right_meso.columns[row * edge].height;
            let right_inner = right_meso.columns[row * edge + 1].height;
            let seam_delta = (right_edge - left_edge).abs();
            let local_delta = (left_edge - left_inner)
                .abs()
                .max((right_inner - right_edge).abs());

            assert!(
                seam_delta <= local_delta + 4.0,
                "shared meso edge delta {seam_delta:.3} should stay close to neighboring local slope {local_delta:.3} at row {row}"
            );
        }
    }

    #[test]
    fn hill_cluster_resolved_windows_match_across_context_boundary() {
        let meta = WorldMeta::new(42);
        let left_chunk = ChunkCoord(127, 0, 0);
        let right_chunk = ChunkCoord(128, 0, 0);
        let left_inputs = prepare_chunk_v2_inputs(left_chunk, &meta);
        let right_inputs = prepare_chunk_v2_inputs(right_chunk, &meta);
        let left_window = build_hill_cluster_window(&left_inputs.meso_guides, left_chunk);
        let right_window = build_hill_cluster_window(&right_inputs.meso_guides, right_chunk);
        let boundary_world_x = right_chunk.0 * CHUNK_EDGE_I32;

        for world_z in (0..CHUNK_EDGE_I32).step_by(4) {
            for world_x in ((boundary_world_x - 16)..=(boundary_world_x + 15)).step_by(4) {
                let left = sample_hill_cluster_surface_from_window(
                    &left_window,
                    &left_inputs.meso_guides,
                    world_x,
                    world_z,
                    100.0,
                    12.0,
                );
                let right = sample_hill_cluster_surface_from_window(
                    &right_window,
                    &right_inputs.meso_guides,
                    world_x,
                    world_z,
                    100.0,
                    12.0,
                );
                assert_hill_cluster_samples_match(left, right, world_x, world_z);
            }
        }
    }

    #[test]
    fn hill_cluster_reported_seam_zone_matches_across_neighboring_chunk_windows() {
        let meta = WorldMeta::new(42);
        let left_chunk = ChunkCoord(-48, 0, 90);
        let right_chunk = ChunkCoord(-47, 0, 90);
        let left_inputs = prepare_chunk_v2_inputs(left_chunk, &meta);
        let right_inputs = prepare_chunk_v2_inputs(right_chunk, &meta);
        let left_window = build_hill_cluster_window(&left_inputs.meso_guides, left_chunk);
        let right_window = build_hill_cluster_window(&right_inputs.meso_guides, right_chunk);

        for world_z in 2888..=2895 {
            for world_x in -1506..=-1502 {
                let left = sample_hill_cluster_surface_from_window(
                    &left_window,
                    &left_inputs.meso_guides,
                    world_x,
                    world_z,
                    100.0,
                    12.0,
                );
                let right = sample_hill_cluster_surface_from_window(
                    &right_window,
                    &right_inputs.meso_guides,
                    world_x,
                    world_z,
                    100.0,
                    12.0,
                );
                assert_hill_cluster_samples_match(left, right, world_x, world_z);
            }
        }
    }

    #[test]
    fn hill_cluster_preview_window_keeps_material_uplift_after_compositing() {
        let meta = WorldMeta::new(42);
        let center_chunk_x = -67;
        let center_chunk_z = 93;
        let radius = 3;
        let mut max_target_raise = 0.0_f32;
        let mut max_hill_delta = 0.0_f32;
        let mut max_net_delta = f32::NEG_INFINITY;
        let mut hill_columns = 0usize;
        let mut visible_hill_columns = 0usize;

        for chunk_z in (center_chunk_z - radius)..=(center_chunk_z + radius) {
            for chunk_x in (center_chunk_x - radius)..=(center_chunk_x + radius) {
                let chunk = ChunkCoord(chunk_x, 0, chunk_z);
                let inputs = prepare_chunk_v2_inputs(chunk, &meta);
                let realization = build_chunk_realization_field_patch(chunk, &inputs);
                let corridor_window = build_chunk_corridor_window(chunk, &inputs);
                let prototype = build_chunk_base_heightfield_prototype(
                    chunk,
                    &inputs,
                    &realization,
                    &corridor_window,
                );
                let meso = build_chunk_meso_applied_prototype(chunk, &inputs, &corridor_window, &prototype);
                let hill_cluster_window = build_hill_cluster_window(&inputs.meso_guides, chunk);

                for local_z in 0..CHUNK_EDGE_I32 {
                    for local_x in 0..CHUNK_EDGE_I32 {
                        let index = local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize;
                        let base = prototype.columns[index];
                        let applied = meso.columns[index];
                        let world_x = chunk.0 * CHUNK_EDGE_I32 + local_x;
                        let world_z = chunk.2 * CHUNK_EDGE_I32 + local_z;
                        let region_samples = sample_region_weights(
                            &inputs.region_classes,
                            world_x as f32 + 0.5,
                            world_z as f32 + 0.5,
                        );
                        let allowed_weight = allowed_feature_weight(&region_samples, "hill_cluster");
                        let corridor_avoidance = corridor_avoidance_factor(
                            local_x as f32 + 0.5,
                            local_z as f32 + 0.5,
                            &corridor_window.corridors,
                        );
                        let hill_surface = sample_hill_cluster_surface_from_window(
                            &hill_cluster_window,
                            &inputs.meso_guides,
                            world_x,
                            world_z,
                            base.base_height,
                            base.relief_budget,
                        );
                        let hill_delta = hill_cluster_surface_delta(
                            base.base_height,
                            hill_surface,
                            allowed_weight,
                            corridor_avoidance,
                        );
                        let net_delta = applied.height - base.base_height;
                        let target_raise = hill_surface.target_surface_y - base.base_height;

                        if hill_delta >= 1.0 {
                            hill_columns += 1;
                        }
                        if net_delta >= 1.0 {
                            visible_hill_columns += 1;
                        }

                        max_target_raise = max_target_raise.max(target_raise);
                        max_hill_delta = max_hill_delta.max(hill_delta);
                        max_net_delta = max_net_delta.max(net_delta);
                    }
                }
            }
        }

        assert!(
            max_target_raise >= 8.0,
            "expected hill-cluster surface solve to keep a visibly raised target in the preview window, got max target raise {max_target_raise:.3}"
        );
        assert!(
            max_hill_delta >= 3.0,
            "expected hill-cluster compositing to keep a material positive delta in the preview window, got max hill delta {max_hill_delta:.3}"
        );
        assert!(
            max_net_delta >= 2.5,
            "expected hill-cluster uplift to survive downstream compositing in the preview window, got max net delta {max_net_delta:.3}"
        );
        assert!(
            visible_hill_columns >= 96,
            "expected the preview window to keep a readable count of visibly raised hill columns, got {visible_hill_columns}"
        );
        assert!(
            hill_columns >= visible_hill_columns,
            "raw hill columns should not be fewer than visible hill columns"
        );
    }
}
