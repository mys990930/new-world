use super::super::macro_field::MacroFieldSample;
use super::stats::neighbor_indices;
use super::{HeightfieldColumn, HeightfieldTerrainKind};

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
