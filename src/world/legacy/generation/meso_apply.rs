use crate::world::atlas::{
    CoastalCliffBandSurfaceSample, CraterSurfaceSample, DuneFieldSurfaceSample,
    HillClusterSurfaceSample, RavineSurfaceSample, RiverPathKind, build_coastal_cliff_band_window,
    build_crater_window, build_dune_field_window, build_hill_cluster_window, build_ravine_window,
    region_archetype_def, sample_coastal_cliff_band_surface_from_window,
    sample_crater_surface_from_window, sample_dune_field_surface_from_window,
    sample_hill_cluster_surface_from_window, sample_meso_guides, sample_ravine_surface_from_window,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::corridors::sample_corridor_axis;
use super::{
    BaseHeightfieldPrototype, ChunkCorridorWindow, ChunkGenerationInputs, RegionSampleWeight,
    RiverCorridorConstraint, sample_region_weights,
};

const MAX_FEATURE_SPEND_FRACTION: f32 = 0.72;
const MIN_COLUMN_RELIEF_SCALE: f32 = 0.30;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MesoAppliedColumn {
    pub height: f32,
    pub remaining_relief_budget: f32,
    pub material_support: super::RealizationMaterialSupport,
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
    inputs: &ChunkGenerationInputs,
    corridor_window: &ChunkCorridorWindow,
    prototype: &BaseHeightfieldPrototype,
) -> MesoAppliedPrototype {
    build_chunk_meso_applied_prototype_for_feature(chunk, inputs, corridor_window, prototype, None)
}

pub fn build_chunk_meso_applied_prototype_for_feature(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    corridor_window: &ChunkCorridorWindow,
    prototype: &BaseHeightfieldPrototype,
    feature_filter: Option<&str>,
) -> MesoAppliedPrototype {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    debug_assert_eq!(prototype.chunk, chunk);

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity(prototype.columns.len());
    let enable_hill_cluster = feature_enabled(feature_filter, "hill_cluster");
    let enable_shallow_basin = feature_enabled(feature_filter, "shallow_basin");
    let enable_escarpment_band = feature_enabled(feature_filter, "escarpment_band");
    let enable_upland_terrace = feature_enabled(feature_filter, "upland_terrace");
    let enable_ravine = feature_enabled(feature_filter, "ravine");
    let enable_coastal_cliff_band = feature_enabled(feature_filter, "coastal_cliff_band");
    let enable_dune_field = feature_enabled(feature_filter, "dune_field");
    let enable_crater = feature_enabled(feature_filter, "crater");
    let mut applied_hill_cluster = false;
    let mut applied_shallow_basin = false;
    let mut applied_escarpment_band = false;
    let mut applied_upland_terrace = false;
    let mut applied_ravine = false;
    let mut applied_coastal_cliff_band = false;
    let mut applied_dune_field = false;
    let mut applied_crater = false;
    let hill_cluster_window = if enable_hill_cluster {
        Some(build_hill_cluster_window(&inputs.meso_guides, chunk))
    } else {
        None
    };
    let ravine_window = if enable_ravine {
        Some(build_ravine_window(&inputs.meso_guides, chunk))
    } else {
        None
    };
    let coastal_cliff_band_window = if enable_coastal_cliff_band {
        Some(build_coastal_cliff_band_window(&inputs.meso_guides, chunk))
    } else {
        None
    };
    let dune_field_window = if enable_dune_field {
        Some(build_dune_field_window(&inputs.meso_guides, chunk))
    } else {
        None
    };
    let crater_window = if enable_crater {
        Some(build_crater_window(&inputs.meso_guides, chunk))
    } else {
        None
    };

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize;
            let base = prototype.columns[index];
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let meso = sample_meso_guides(&inputs.meso_guides, world_x, world_z);
            let region_samples = sample_region_weights(
                &inputs.region_classes,
                world_x as f32 + 0.5,
                world_z as f32 + 0.5,
            );
            let corridor_avoidance = corridor_avoidance_factor(
                local_x as f32 + 0.5,
                local_z as f32 + 0.5,
                &corridor_window.corridors,
            );
            let column_relief_scale = relief_budget_scale(base.relief_budget);
            let hill_cluster_allowed = if enable_hill_cluster {
                allowed_feature_weight(&region_samples, "hill_cluster")
            } else {
                0.0
            };
            let shallow_basin_allowed = if enable_shallow_basin {
                allowed_feature_weight(&region_samples, "shallow_basin")
            } else {
                0.0
            };
            let escarpment_band_allowed = if enable_escarpment_band {
                allowed_feature_weight(&region_samples, "escarpment_band")
            } else {
                0.0
            };
            let upland_terrace_allowed = if enable_upland_terrace {
                allowed_feature_weight(&region_samples, "upland_terrace")
            } else {
                0.0
            };
            let ravine_allowed = if enable_ravine {
                allowed_feature_weight(&region_samples, "ravine")
            } else {
                0.0
            };
            let coastal_cliff_band_allowed = if enable_coastal_cliff_band {
                allowed_feature_weight(&region_samples, "coastal_cliff_band")
            } else {
                0.0
            };
            let dune_field_allowed = if enable_dune_field {
                allowed_feature_weight(&region_samples, "dune_field")
            } else {
                0.0
            };
            let crater_allowed = if enable_crater {
                allowed_feature_weight(&region_samples, "crater")
            } else {
                0.0
            };
            let hill_cluster_surface = if let Some(window) = hill_cluster_window.as_ref() {
                sample_hill_cluster_surface_from_window(
                    window,
                    &inputs.meso_guides,
                    world_x,
                    world_z,
                    base.base_height,
                    base.relief_budget,
                )
            } else {
                HillClusterSurfaceSample::flat(base.base_height)
            };
            let ravine_surface = if let Some(window) = ravine_window.as_ref() {
                sample_ravine_surface_from_window(
                    window,
                    &inputs.meso_guides,
                    world_x,
                    world_z,
                    base.base_height,
                    base.relief_budget,
                )
            } else {
                RavineSurfaceSample::flat(base.base_height)
            };
            let coastal_cliff_band_surface =
                if let Some(window) = coastal_cliff_band_window.as_ref() {
                    sample_coastal_cliff_band_surface_from_window(
                        window,
                        &inputs.meso_guides,
                        world_x,
                        world_z,
                        base.base_height,
                        base.relief_budget,
                    )
                } else {
                    CoastalCliffBandSurfaceSample::flat(base.base_height)
                };
            let dune_field_surface = if let Some(window) = dune_field_window.as_ref() {
                sample_dune_field_surface_from_window(
                    window,
                    &inputs.meso_guides,
                    world_x,
                    world_z,
                    base.base_height,
                    base.relief_budget,
                )
            } else {
                DuneFieldSurfaceSample::flat(base.base_height)
            };
            let crater_surface = if let Some(window) = crater_window.as_ref() {
                sample_crater_surface_from_window(
                    window,
                    &inputs.meso_guides,
                    world_x,
                    world_z,
                    base.base_height,
                    base.relief_budget,
                )
            } else {
                CraterSurfaceSample::flat(base.base_height)
            };

            let hill_cluster = if enable_hill_cluster {
                hill_cluster_surface_delta(
                    base.base_height,
                    hill_cluster_surface,
                    hill_cluster_allowed,
                    corridor_avoidance,
                )
            } else {
                0.0
            };
            let basin_hill_conflict = (hill_cluster_surface.core_coverage * 0.82
                + hill_cluster_surface.shoulder_coverage * 0.46)
                .clamp(0.0, 0.92);
            let ravine_basin_conflict = (ravine_surface.trench_coverage * 0.78
                + ravine_surface.shoulder_coverage * 0.24)
                .clamp(0.0, 0.92);
            let crater_basin_conflict = (crater_surface.bowl_coverage * 0.72
                + crater_surface.rim_coverage * 0.18)
                .clamp(0.0, 0.94);
            let coastal_escarpment_conflict = (coastal_cliff_band_surface.face_coverage * 0.74
                + coastal_cliff_band_surface.plateau_coverage * 0.20)
                .clamp(0.0, 0.90);
            let dune_terrace_conflict = (dune_field_surface.crest_coverage * 0.68
                + dune_field_surface.field_coverage * 0.26)
                .clamp(0.0, 0.88);

            let shallow_basin = if enable_shallow_basin {
                shallow_basin_delta(
                    &meso,
                    shallow_basin_allowed,
                    column_relief_scale,
                    corridor_avoidance,
                ) * (1.0_f32
                    - basin_hill_conflict * 0.84
                    - ravine_basin_conflict * 0.52
                    - crater_basin_conflict * 0.60)
                    .clamp(0.18, 1.0)
            } else {
                0.0
            };
            let escarpment_band = if enable_escarpment_band {
                escarpment_band_delta(
                    &meso,
                    escarpment_band_allowed,
                    column_relief_scale,
                    corridor_avoidance,
                ) * (1.0_f32 - coastal_escarpment_conflict * 0.58).clamp(0.20, 1.0)
            } else {
                0.0
            };
            let upland_terrace = if enable_upland_terrace {
                upland_terrace_delta(
                    &meso,
                    upland_terrace_allowed,
                    column_relief_scale,
                    corridor_avoidance,
                ) * (1.0_f32 - dune_terrace_conflict * 0.54).clamp(0.24, 1.0)
            } else {
                0.0
            };
            let ravine = if enable_ravine {
                ravine_surface_delta(
                    base.base_height,
                    ravine_surface,
                    ravine_allowed,
                    corridor_avoidance,
                )
            } else {
                0.0
            };
            let coastal_cliff_band = if enable_coastal_cliff_band {
                coastal_cliff_band_surface_delta(
                    base.base_height,
                    coastal_cliff_band_surface,
                    coastal_cliff_band_allowed,
                    corridor_avoidance,
                )
            } else {
                0.0
            };
            let dune_field = if enable_dune_field {
                dune_field_surface_delta(
                    base.base_height,
                    dune_field_surface,
                    dune_field_allowed,
                    corridor_avoidance,
                )
            } else {
                0.0
            };
            let crater = if enable_crater {
                crater_surface_delta(base.base_height, crater_surface, crater_allowed)
            } else {
                0.0
            };

            let unclamped_delta = hill_cluster
                + shallow_basin
                + escarpment_band
                + upland_terrace
                + ravine
                + coastal_cliff_band
                + dune_field
                + crater;
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
            applied_ravine |= ravine.abs() >= 0.10;
            applied_coastal_cliff_band |= coastal_cliff_band.abs() >= 0.10;
            applied_dune_field |= dune_field.abs() >= 0.10;
            applied_crater |= crater.abs() >= 0.10;

            let material_support = refine_meso_material_support(
                base.material_support,
                applied_delta,
                remaining_relief_budget,
            );

            columns.push(MesoAppliedColumn {
                height: base.base_height + applied_delta,
                remaining_relief_budget,
                material_support,
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
    if applied_ravine {
        applied_feature_keys.push("ravine");
    }
    if applied_coastal_cliff_band {
        applied_feature_keys.push("coastal_cliff_band");
    }
    if applied_dune_field {
        applied_feature_keys.push("dune_field");
    }
    if applied_crater {
        applied_feature_keys.push("crater");
    }

    MesoAppliedPrototype {
        chunk,
        columns,
        applied_feature_keys,
    }
}

fn refine_meso_material_support(
    mut support: super::RealizationMaterialSupport,
    applied_delta: f32,
    remaining_relief_budget: f32,
) -> super::RealizationMaterialSupport {
    let raised_or_cut = smoothstep_range(0.75, 5.5, applied_delta.abs());
    let lowered = smoothstep_range(0.45, 4.0, -applied_delta);
    let depleted_budget = 1.0 - (remaining_relief_budget / 18.0).clamp(0.0, 1.0);

    support.exposure =
        (support.exposure + raised_or_cut * 0.10 + depleted_budget * 0.08).clamp(0.0, 1.0);
    support.wetness = (support.wetness + lowered * 0.08).clamp(0.0, 1.0);
    support.sediment = (support.sediment + lowered * 0.08 + raised_or_cut * 0.06).clamp(0.0, 1.0);
    support.soil_cover = (support.soil_cover * (1.0 - raised_or_cut * 0.10)
        + (1.0 - depleted_budget) * 0.04)
        .clamp(0.0, 1.0);
    support.finalized()
}

fn relief_budget_scale(relief_budget: f32) -> f32 {
    (relief_budget / 20.0).clamp(MIN_COLUMN_RELIEF_SCALE, 1.0)
}

fn feature_enabled(feature_filter: Option<&str>, key: &str) -> bool {
    feature_filter.is_none_or(|selected| selected == key)
}

fn allowed_feature_weight(region_samples: &[RegionSampleWeight], key: &str) -> f32 {
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
        (hill_cluster_surface.core_coverage * 0.78 + hill_cluster_surface.shoulder_coverage * 0.22)
            .clamp(0.0, 1.0),
    )
}

fn ravine_surface_delta(
    base_height: f32,
    ravine_surface: RavineSurfaceSample,
    allowed_weight: f32,
    corridor_avoidance: f32,
) -> f32 {
    let surface_weight = ravine_surface.blend_weight.max(
        (ravine_surface.trench_coverage * 0.78 + ravine_surface.shoulder_coverage * 0.18)
            .clamp(0.0, 0.94),
    );
    blended_surface_delta(
        base_height,
        ravine_surface.target_surface_y,
        surface_weight,
        allowed_weight,
        corridor_avoidance.clamp(0.0, 1.0).powf(0.92),
    )
}

fn coastal_cliff_band_surface_delta(
    base_height: f32,
    coastal_cliff_band_surface: CoastalCliffBandSurfaceSample,
    allowed_weight: f32,
    corridor_avoidance: f32,
) -> f32 {
    let surface_weight = coastal_cliff_band_surface.blend_weight.max(
        (coastal_cliff_band_surface.face_coverage * 0.60
            + coastal_cliff_band_surface.plateau_coverage * 0.24
            + coastal_cliff_band_surface.bench_coverage * 0.12)
            .clamp(0.0, 0.96),
    );
    blended_surface_delta(
        base_height,
        coastal_cliff_band_surface.target_surface_y,
        surface_weight,
        allowed_weight,
        corridor_avoidance.clamp(0.0, 1.0).powf(0.75),
    )
}

fn dune_field_surface_delta(
    base_height: f32,
    dune_field_surface: DuneFieldSurfaceSample,
    allowed_weight: f32,
    corridor_avoidance: f32,
) -> f32 {
    let surface_weight = dune_field_surface.blend_weight.max(
        (dune_field_surface.crest_coverage * 0.58 + dune_field_surface.field_coverage * 0.32)
            .clamp(0.0, 0.90),
    );
    blended_surface_delta(
        base_height,
        dune_field_surface.target_surface_y,
        surface_weight,
        allowed_weight,
        corridor_avoidance.clamp(0.0, 1.0).powf(0.62),
    )
}

fn crater_surface_delta(
    base_height: f32,
    crater_surface: CraterSurfaceSample,
    allowed_weight: f32,
) -> f32 {
    let surface_weight = crater_surface.blend_weight.max(
        (crater_surface.bowl_coverage * 0.46 + crater_surface.rim_coverage * 0.40).clamp(0.0, 0.94),
    );
    blended_surface_delta(
        base_height,
        crater_surface.target_surface_y,
        surface_weight,
        allowed_weight,
        1.0,
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

    let transition =
        smoothstep01((-meso.escarpment_signed_distance_cells / 0.78 + 0.5).clamp(0.0, 1.0));
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
        let projected = sample_corridor_axis(*corridor, local_x, local_z);
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

fn blended_surface_delta(
    base_height: f32,
    target_surface_y: f32,
    blend_weight: f32,
    allowed_weight: f32,
    attenuation: f32,
) -> f32 {
    let weight = blend_weight * allowed_weight * attenuation;
    if weight <= f32::EPSILON {
        return 0.0;
    }

    let blended_surface_y = lerp_f32(base_height, target_surface_y, weight);
    blended_surface_y - base_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::{MesoGuideSample, RegionArchetype, RegionClassCell};

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
        assert_eq!(allowed_feature_weight(&region_samples, "ravine"), 0.0);
        assert_eq!(
            allowed_feature_weight(&region_samples, "upland_terrace"),
            0.0
        );
    }

    #[test]
    fn ravine_allowance_skips_broad_launch_fallback_targets() {
        let broad_fallback_targets = [
            RegionArchetype::ColdWetLowland,
            RegionArchetype::TemperatePlain,
            RegionArchetype::DesertPlain,
            RegionArchetype::SavannaPlain,
            RegionArchetype::TropicalRainforestLowland,
            RegionArchetype::GlaciatedAlpine,
        ];

        for archetype in broad_fallback_targets {
            let mut cell = RegionClassCell::default();
            cell.archetype = archetype;
            let region_samples = [
                RegionSampleWeight { cell, weight: 1.0 },
                RegionSampleWeight {
                    cell: RegionClassCell::default(),
                    weight: 0.0,
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

            assert_eq!(
                allowed_feature_weight(&region_samples, "ravine"),
                0.0,
                "expected broad launch fallback target {archetype:?} not to allow ravine by default"
            );
        }

        let mut hill_cell = RegionClassCell::default();
        hill_cell.archetype = RegionArchetype::TropicalRainforestHills;
        let hill_samples = [
            RegionSampleWeight {
                cell: hill_cell,
                weight: 1.0,
            },
            RegionSampleWeight {
                cell: RegionClassCell::default(),
                weight: 0.0,
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

        assert_eq!(allowed_feature_weight(&hill_samples, "ravine"), 1.0);
    }

    #[test]
    fn corridor_avoidance_suppresses_nearby_columns() {
        let corridor = RiverCorridorConstraint {
            river_id: 1,
            basin_id: 1,
            main_stem_river_id: 1,
            parent_river_id: None,
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
            downstream_cells_start: 0.0,
            downstream_cells_end: 1.0,
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
}
