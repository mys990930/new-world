use super::super::graph::WorldPlanePoint;
use super::super::macro_field::MacroFieldSample;
use super::mapping::{heightfield_value_noise_2d, lerp, smoothstep01};
use super::stats::neighbor_indices;
use super::{HeightfieldColumn, HeightfieldConfig, HeightfieldTerrainKind};

const MAX_SAME_CONTEXT_RIVER_BED_STEP_BLOCKS: i32 = 1;

pub(super) fn river_bed_depth_blocks(sample: &MacroFieldSample) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let bed = sample.river_bed_depth_hint.clamp(0.0, 1.0);
    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    (bed * 40.0)
        .max(1.0 + flow * 1.5 + rough * 0.4)
        .clamp(1.0, 40.0)
}

pub(super) fn river_water_depth_blocks(sample: &MacroFieldSample) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let bed_depth = river_bed_depth_blocks(sample);
    (bed_depth * (0.78 + flow * 0.17)).clamp(1.0, bed_depth.max(1.0))
}

pub(super) fn river_core_center_profile_factor(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> f32 {
    let core = sample.river_core_strength.clamp(0.0, 1.0);
    let threshold = config.river_water_threshold.clamp(0.0, 0.98);
    if sample.river_flow_hint <= 0.0 || core <= threshold {
        return 0.0;
    }

    smoothstep01((core - threshold) / (1.0 - threshold).max(f32::EPSILON))
}

pub(super) fn river_core_center_downcut_blocks(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
    river_bed_depth_blocks: f32,
) -> f32 {
    if river_bed_depth_blocks <= 0.0 {
        return 0.0;
    }

    let center = river_core_center_profile_factor(sample, config);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let flow_gate = smoothstep01((flow - 0.08) / 0.34);
    let activity = center * flow_gate;
    if activity <= f32::EPSILON {
        return 0.0;
    }

    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = sample.river_gravel_hint.clamp(0.0, 1.0);
    let fraction = (lerp(0.14, 0.36, flow_gate) + rough * 0.04 + gravel * 0.03).clamp(0.12, 0.48);
    let visible_floor = lerp(0.0, 2.0, flow_gate);
    let downcut = (river_bed_depth_blocks * fraction)
        .max(visible_floor.min(river_bed_depth_blocks * 0.35))
        .min(river_bed_depth_blocks * 0.50);

    downcut * activity
}

pub(super) fn river_bed_raise_variation_limit_blocks(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> f32 {
    let center = river_core_center_profile_factor(sample, config);
    let water_depth = river_water_depth_blocks(sample);
    let center_raise_scale = lerp(1.0, 0.34, center);

    (water_depth - 1.0).max(0.0) * center_raise_scale
}

pub(super) fn deterministic_river_bed_variation_blocks(sample: &MacroFieldSample) -> f32 {
    let valley = sample.river_core_strength.clamp(0.0, 1.0);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    if valley <= 0.0 || flow < 0.12 {
        return 0.0;
    }

    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = sample.river_gravel_hint.clamp(0.0, 1.0);
    let cutbank = sample.river_cutbank_hint.clamp(0.0, 1.0);
    let bed_depth = river_bed_depth_blocks(sample);
    let broad = heightfield_value_noise_2d(sample.position, 47.0, 0xB4D0_0001);
    let medium = heightfield_value_noise_2d(
        WorldPlanePoint::new(sample.position.x + 17.0, sample.position.z - 29.0),
        19.0,
        0xB4D0_0002,
    );
    let small = heightfield_value_noise_2d(
        WorldPlanePoint::new(sample.position.x - 5.0, sample.position.z + 41.0),
        7.0,
        0xB4D0_0003,
    );
    let longitudinal = river_bed_longitudinal_ripple(sample.position);
    let flow_scale = lerp(0.45, 1.0, smoothstep01(flow));
    let amplitude = (0.75 + rough * 1.15 + gravel * 0.7 + cutbank * 0.45)
        * (1.05 - flow * 0.22)
        * valley
        * flow_scale
        * (0.75 + bed_depth.sqrt() * 0.18);

    (broad * 0.42 + medium * 0.34 + small * 0.16 + longitudinal * 0.08).clamp(-1.0, 1.0) * amplitude
}

pub(super) fn river_bed_downcut_variation_limit_blocks(
    sample: &MacroFieldSample,
    river_bed_depth_blocks: f32,
) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = sample.river_gravel_hint.clamp(0.0, 1.0);
    let flow_gate = smoothstep01((flow - 0.04) / 0.24);
    let fraction = (0.18 + flow * 0.16 + rough * 0.08 + gravel * 0.08).clamp(0.12, 0.48);
    let visible_floor = lerp(0.45, 1.35, flow_gate);

    (river_bed_depth_blocks * fraction * flow_gate)
        .max((visible_floor * flow_gate).min(river_bed_depth_blocks * 0.45))
        .min(river_bed_depth_blocks * 0.52)
}

fn river_bed_longitudinal_ripple(position: WorldPlanePoint) -> f32 {
    let diagonal = (position.x * 0.071 + position.z * 0.041).sin();
    let counter = (position.x * -0.037 + position.z * 0.083).sin();

    (diagonal * 0.65 + counter * 0.35).clamp(-1.0, 1.0)
}

pub(super) fn deterministic_river_bank_variation_blocks(sample: &MacroFieldSample) -> f32 {
    let valley = sample.river_shoulder_strength.clamp(0.0, 1.0);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    if valley < 0.34
        || flow <= 0.0
        || !sample.river_distance_blocks.is_finite()
        || sample.river_distance_blocks > 112.0
    {
        return 0.0;
    }

    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = sample.river_gravel_hint.clamp(0.0, 1.0);
    let bank = (1.0 - valley).clamp(0.0, 1.0);
    let shoulder = (bank * 1.25).clamp(0.0, 1.0);
    let broad = heightfield_value_noise_2d(sample.position, 17.0, 0xBA11_0001);
    let medium = heightfield_value_noise_2d(
        WorldPlanePoint::new(sample.position.x - 23.0, sample.position.z + 11.0),
        7.0,
        0xBA11_0002,
    );
    let amplitude = (0.45 + rough * 1.4 + gravel * 0.55) * shoulder * (1.0 - flow * 0.22);

    (broad * 0.6 + medium * 0.4).clamp(-1.0, 1.0) * amplitude
}

pub(super) fn river_bank_slope_lowering_blocks(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> f32 {
    let valley = sample.river_shoulder_strength.clamp(0.0, 1.0);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    if flow <= 0.0 || !sample.river_distance_blocks.is_finite() {
        return 0.0;
    }
    if sample.river_core_strength >= config.river_water_threshold {
        return 0.0;
    }

    let shoulder_gate = smoothstep01(
        (valley - config.river_water_threshold * 0.58)
            / (config.river_water_threshold * 0.34).max(f32::EPSILON),
    );
    let local_gate = river_bank_relief_factor(sample, config);
    let flow_gate = smoothstep01(flow);
    if shoulder_gate <= f32::EPSILON || local_gate <= f32::EPSILON {
        return 0.0;
    }

    let depth = river_bed_depth_blocks(sample);
    let lowering_fraction = lerp(0.34, 0.62, flow_gate);
    (depth * lowering_fraction * shoulder_gate * local_gate).clamp(0.0, depth * 0.72)
}

pub(super) fn river_bank_relief_factor(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> f32 {
    let valley = sample.river_shoulder_strength.clamp(0.0, 1.0);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    if flow <= 0.0 || !sample.river_distance_blocks.is_finite() {
        return 0.0;
    }

    let near_bank_min = config.river_water_threshold * 0.38;
    let near_bank_full = config.river_water_threshold * 0.58;
    let strength_gate =
        smoothstep01((valley - near_bank_min) / (near_bank_full - near_bank_min).max(f32::EPSILON));
    let local_distance_gate = 1.0 - smoothstep01((sample.river_distance_blocks - 64.0) / 48.0);

    (strength_gate * local_distance_gate).clamp(0.0, 1.0)
}

pub(super) fn apply_river_water_descent(
    columns: &mut [HeightfieldColumn],
    width: usize,
    height: usize,
) {
    if columns.is_empty() || width == 0 || height == 0 {
        return;
    }

    let mut water_y = columns
        .iter()
        .map(|column| column.water_y)
        .collect::<Vec<_>>();
    limit_river_water_neighbor_delta(columns, &mut water_y, width, height);
    apply_same_context_river_bed_step_limit(columns, width, height);

    for (index, column) in columns.iter_mut().enumerate() {
        if !matches!(column.terrain_kind, HeightfieldTerrainKind::River) {
            continue;
        }
        let Some(water) = water_y[index] else {
            continue;
        };
        column.water_y = Some(water);
        column.water_level_blocks = Some(water as f32);
        column.river_water_height_blocks = Some(water as f32);
        if column.surface_y >= water {
            column.water_y = None;
            column.water_level_blocks = None;
            column.river_water_height_blocks = None;
        }
    }
}

fn limit_river_water_neighbor_delta(
    columns: &[HeightfieldColumn],
    water_y: &mut [Option<i32>],
    width: usize,
    height: usize,
) {
    for _ in 0..(width + height).max(1) {
        let mut changed = false;
        for index in 0..columns.len() {
            if !matches!(columns[index].terrain_kind, HeightfieldTerrainKind::River) {
                continue;
            }
            let Some(current) = water_y[index] else {
                continue;
            };
            let mut allowed = current;
            for neighbor in neighbor_indices(index, width, height) {
                let Some(neighbor_water) = water_y[neighbor] else {
                    continue;
                };
                if matches!(
                    columns[neighbor].terrain_kind,
                    HeightfieldTerrainKind::River
                ) && columns[index].river_flow_hint > columns[neighbor].river_flow_hint + 0.01
                {
                    allowed = allowed.min(neighbor_water);
                } else {
                    allowed = allowed.min(neighbor_water.saturating_add(1));
                }
            }
            if allowed < current {
                water_y[index] = Some(allowed);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn apply_same_context_river_bed_step_limit(
    columns: &mut [HeightfieldColumn],
    width: usize,
    height: usize,
) {
    let mut bed_y = columns
        .iter()
        .map(|column| column.surface_y)
        .collect::<Vec<_>>();
    for _ in 0..(width + height).max(1) {
        let mut changed = false;
        for index in 0..columns.len() {
            if !is_same_context_river_bed_candidate(columns[index]) {
                continue;
            }
            let mut allowed = bed_y[index];
            for neighbor in neighbor_indices(index, width, height) {
                if !same_context_river_bed_pair(columns[index], columns[neighbor]) {
                    continue;
                }
                allowed = allowed
                    .min(bed_y[neighbor].saturating_add(MAX_SAME_CONTEXT_RIVER_BED_STEP_BLOCKS));
            }
            if allowed < bed_y[index] {
                bed_y[index] = allowed;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for (index, column) in columns.iter_mut().enumerate() {
        if bed_y[index] >= column.surface_y {
            continue;
        }
        column.surface_y = bed_y[index];
        column.surface_height_blocks = bed_y[index] as f32;
        column.constrained_surface_height_blocks = column
            .constrained_surface_height_blocks
            .min(column.surface_height_blocks);
    }
}

fn is_same_context_river_bed_candidate(column: HeightfieldColumn) -> bool {
    matches!(column.terrain_kind, HeightfieldTerrainKind::River)
        && column.water_y.is_some()
        && column.river_core_strength >= 0.88
        && column.river_flow_hint > 0.0
        && column.river_distance_blocks.is_finite()
}

fn same_context_river_bed_pair(a: HeightfieldColumn, b: HeightfieldColumn) -> bool {
    is_same_context_river_bed_candidate(a)
        && is_same_context_river_bed_candidate(b)
        && (a.river_flow_hint - b.river_flow_hint).abs() <= 0.03
        && (a.river_core_strength - b.river_core_strength).abs() <= 0.08
        && (a.river_distance_blocks - b.river_distance_blocks).abs() <= 2.0
}
