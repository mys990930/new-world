use super::super::macro_field::MacroFieldSample;
use super::stats::neighbor_indices;
use super::{HeightfieldColumn, HeightfieldTerrainKind};

const MAX_SAME_CONTEXT_RIVER_CORE_STEP_BLOCKS: i32 = 1;
const RIVER_CORE_WATER_STRENGTH: f32 = 0.0;
const MAX_SUPPORTED_DOWNSTREAM_WATER_STEP_BLOCKS: i32 = 1;

pub(super) fn river_core_depth_blocks(sample: &MacroFieldSample) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let core_depth = sample.river_core_depth_hint.clamp(0.0, 1.0);
    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    (core_depth * 40.0)
        .max(1.0 + flow * 1.5 + rough * 0.4)
        .clamp(1.0, 40.0)
}

pub(super) fn river_core_water_depth_blocks(sample: &MacroFieldSample) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let core_depth = river_core_depth_blocks(sample);
    let estuary_depth = estuary_water_depth_blocks(sample);
    if estuary_depth > 0.0 && sample.estuary_water_strength > sample.river_core_strength {
        return estuary_depth.clamp(1.0, core_depth.max(1.0));
    }
    let fill_ratio = (0.70 + flow * 0.10 - rough * 0.05).clamp(0.58, 0.82);
    let river_depth = (core_depth * fill_ratio).clamp(1.0, core_depth.max(1.0));
    river_depth.max(estuary_depth)
}

fn is_river_core_terrain(kind: HeightfieldTerrainKind) -> bool {
    matches!(kind, HeightfieldTerrainKind::RiverCore)
}

fn is_river_corridor_terrain(kind: HeightfieldTerrainKind) -> bool {
    matches!(
        kind,
        HeightfieldTerrainKind::RiverCore | HeightfieldTerrainKind::RiverBed
    )
}

fn estuary_water_depth_blocks(sample: &MacroFieldSample) -> f32 {
    if sample.estuary_water_strength <= 0.0 {
        return 0.0;
    }
    (sample.estuary_water_depth_hint.clamp(0.0, 1.0) * 40.0).max(0.0)
}

pub(super) fn apply_river_water_descent(
    columns: &mut [HeightfieldColumn],
    width: usize,
    height: usize,
    sea_level_blocks: f32,
) {
    if columns.is_empty() || width == 0 || height == 0 {
        return;
    }

    let mut water_y = columns
        .iter()
        .map(|column| {
            if is_river_core_terrain(column.terrain_kind) {
                column.water_y
            } else if matches!(column.terrain_kind, HeightfieldTerrainKind::RiverBed) {
                None
            } else {
                column.water_y
            }
        })
        .collect::<Vec<_>>();
    let sea_level_y = sea_level_blocks.round() as i32;
    clamp_river_water_to_local_bank(columns, &mut water_y, width, height, sea_level_y);
    limit_river_water_neighbor_delta(columns, &mut water_y, width, height);
    clamp_river_water_to_local_bank(columns, &mut water_y, width, height, sea_level_y);
    enforce_river_water_lateral_support(columns, &mut water_y, width, height, sea_level_y);
    let river_water_depths = river_core_water_depths(columns, &water_y);
    apply_same_context_river_core_step_limit(columns, width, height);
    align_river_water_to_smoothed_core_bed(columns, &mut water_y, &river_water_depths, sea_level_y);
    limit_river_water_neighbor_delta(columns, &mut water_y, width, height);
    clamp_river_water_to_local_bank(columns, &mut water_y, width, height, sea_level_y);
    enforce_river_water_lateral_support(columns, &mut water_y, width, height, sea_level_y);

    for (index, column) in columns.iter_mut().enumerate() {
        if matches!(column.terrain_kind, HeightfieldTerrainKind::RiverBed) {
            column.water_y = None;
            column.water_level_blocks = None;
            column.river_core_water_height_blocks = None;
            continue;
        }
        if !is_river_core_terrain(column.terrain_kind) {
            continue;
        }
        let Some(water) = water_y[index] else {
            column.water_y = None;
            column.water_level_blocks = None;
            column.river_core_water_height_blocks = None;
            continue;
        };
        column.water_y = Some(water);
        column.water_level_blocks = Some(water as f32);
        column.river_core_water_height_blocks = Some(water as f32);
        if column.surface_y >= water {
            column.water_y = None;
            column.water_level_blocks = None;
            column.river_core_water_height_blocks = None;
        }
    }
}

fn river_core_water_depths(
    columns: &[HeightfieldColumn],
    water_y: &[Option<i32>],
) -> Vec<Option<i32>> {
    columns
        .iter()
        .zip(water_y.iter().copied())
        .map(|(column, water)| {
            if !is_river_core_terrain(column.terrain_kind) {
                return None;
            }
            water
                .map(|water| water.saturating_sub(column.surface_y))
                .filter(|depth| *depth > 0)
        })
        .collect()
}

fn align_river_water_to_smoothed_core_bed(
    columns: &[HeightfieldColumn],
    water_y: &mut [Option<i32>],
    river_water_depths: &[Option<i32>],
    sea_level_y: i32,
) {
    for ((column, water), depth) in columns
        .iter()
        .zip(water_y.iter_mut())
        .zip(river_water_depths.iter().copied())
    {
        if !is_river_core_terrain(column.terrain_kind) {
            continue;
        }
        let (Some(current), Some(depth)) = (*water, depth) else {
            continue;
        };
        let adjusted = column
            .surface_y
            .saturating_add(depth.max(1))
            .max(sea_level_y);
        *water = Some(current.min(adjusted));
    }
}

fn enforce_river_water_lateral_support(
    columns: &[HeightfieldColumn],
    water_y: &mut [Option<i32>],
    width: usize,
    height: usize,
    sea_level_y: i32,
) {
    for _ in 0..(width + height).max(1) {
        let mut changed = false;
        for index in 0..columns.len() {
            if !is_river_core_terrain(columns[index].terrain_kind) {
                continue;
            }
            let Some(current) = water_y[index] else {
                continue;
            };
            if river_water_has_same_level_or_downstream_support(
                columns, water_y, width, height, index, current,
            ) {
                continue;
            }
            let supported = supported_water_level_at_or_below(
                columns,
                water_y,
                width,
                height,
                index,
                current,
                sea_level_y,
            );
            if water_y[index] != supported {
                water_y[index] = supported;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

fn river_water_has_same_level_or_downstream_support(
    columns: &[HeightfieldColumn],
    water_y: &[Option<i32>],
    width: usize,
    height: usize,
    index: usize,
    level: i32,
) -> bool {
    neighbor_indices(index, width, height).any(|neighbor| {
        let neighbor_column = columns[neighbor];
        water_y[neighbor] == Some(level)
            || neighbor_column.surface_y >= level
            || is_supported_downstream_descent(
                columns[index],
                neighbor_column,
                level,
                water_y[neighbor],
            )
    })
}

fn supported_water_level_at_or_below(
    columns: &[HeightfieldColumn],
    water_y: &[Option<i32>],
    width: usize,
    height: usize,
    index: usize,
    current: i32,
    sea_level_y: i32,
) -> Option<i32> {
    let min_water = columns[index].surface_y.saturating_add(1).max(sea_level_y);
    let mut supported = None::<i32>;
    for neighbor in neighbor_indices(index, width, height) {
        let neighbor_column = columns[neighbor];
        if let Some(neighbor_water) = water_y[neighbor] {
            if neighbor_water <= current && neighbor_water >= min_water {
                supported =
                    Some(supported.map_or(neighbor_water, |value| value.max(neighbor_water)));
            }
            if is_supported_downstream_descent(
                columns[index],
                neighbor_column,
                current,
                Some(neighbor_water),
            ) {
                let downstream_supported = neighbor_water
                    .saturating_add(MAX_SUPPORTED_DOWNSTREAM_WATER_STEP_BLOCKS)
                    .min(current);
                if downstream_supported >= min_water {
                    supported = Some(supported.map_or(downstream_supported, |value| {
                        value.max(downstream_supported)
                    }));
                }
            }
        }
        let block_supported = neighbor_column.surface_y.min(current);
        if block_supported >= min_water {
            supported = Some(supported.map_or(block_supported, |value| value.max(block_supported)));
        }
    }
    supported
}

fn is_supported_downstream_descent(
    column: HeightfieldColumn,
    neighbor: HeightfieldColumn,
    level: i32,
    neighbor_water: Option<i32>,
) -> bool {
    let Some(neighbor_level) = neighbor_water else {
        return false;
    };
    if neighbor_level >= level {
        return false;
    }
    if level - neighbor_level > MAX_SUPPORTED_DOWNSTREAM_WATER_STEP_BLOCKS {
        return false;
    }
    if matches!(
        neighbor.terrain_kind,
        HeightfieldTerrainKind::Ocean | HeightfieldTerrainKind::Lake
    ) {
        return true;
    }
    is_river_core_terrain(neighbor.terrain_kind)
        && neighbor.river_flow_hint > column.river_flow_hint + 0.01
}

fn clamp_river_water_to_local_bank(
    columns: &[HeightfieldColumn],
    water_y: &mut [Option<i32>],
    width: usize,
    height: usize,
    sea_level_y: i32,
) {
    for index in 0..columns.len() {
        if !is_river_core_terrain(columns[index].terrain_kind) {
            continue;
        }
        let Some(current) = water_y[index] else {
            continue;
        };
        let mut ceiling = None::<i32>;
        for neighbor in neighbor_indices(index, width, height) {
            let neighbor_column = columns[neighbor];
            if is_local_bank_ceiling_candidate(neighbor_column, sea_level_y) {
                ceiling = Some(
                    ceiling
                        .map(|value| value.min(neighbor_column.surface_y))
                        .unwrap_or(neighbor_column.surface_y),
                );
            }
        }
        let Some(ceiling) = ceiling else {
            continue;
        };
        let clamped = current.min(water_ceiling_preserving_river_core(
            columns[index],
            ceiling.max(sea_level_y),
            0,
        ));
        water_y[index] = Some(clamped);
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
            if !is_river_core_terrain(columns[index].terrain_kind) {
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
                if is_river_core_terrain(columns[neighbor].terrain_kind) {
                    let step = if same_core_pool_context(columns[index], columns[neighbor]) {
                        0
                    } else if columns[index].river_flow_hint
                        > columns[neighbor].river_flow_hint + 0.01
                    {
                        0
                    } else {
                        1
                    };
                    allowed = allowed.min(water_ceiling_preserving_river_core(
                        columns[index],
                        neighbor_water,
                        step,
                    ));
                } else {
                    allowed = allowed.min(water_ceiling_preserving_river_core(
                        columns[index],
                        neighbor_water,
                        1,
                    ));
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

fn water_ceiling_preserving_river_core(
    column: HeightfieldColumn,
    neighbor_water: i32,
    step: i32,
) -> i32 {
    let ceiling = neighbor_water.saturating_add(step);
    if matches!(column.terrain_kind, HeightfieldTerrainKind::RiverCore)
        && ceiling <= column.surface_y
        && column.river_core_strength >= RIVER_CORE_WATER_STRENGTH
    {
        column.surface_y.saturating_add(1)
    } else {
        ceiling
    }
}

fn is_local_bank_ceiling_candidate(column: HeightfieldColumn, sea_level_y: i32) -> bool {
    !matches!(
        column.terrain_kind,
        HeightfieldTerrainKind::Ocean | HeightfieldTerrainKind::Lake
    ) && column.water_y.is_none()
        && !is_river_corridor_terrain(column.terrain_kind)
        && column.surface_y >= sea_level_y
}

fn same_core_pool_context(a: HeightfieldColumn, b: HeightfieldColumn) -> bool {
    let distance_delta =
        if a.river_distance_blocks.is_finite() && b.river_distance_blocks.is_finite() {
            (a.river_distance_blocks - b.river_distance_blocks).abs()
        } else {
            f32::INFINITY
        };
    matches!(a.terrain_kind, HeightfieldTerrainKind::RiverCore)
        && matches!(b.terrain_kind, HeightfieldTerrainKind::RiverCore)
        && a.river_core_strength >= RIVER_CORE_WATER_STRENGTH
        && b.river_core_strength >= RIVER_CORE_WATER_STRENGTH
        && (a.river_flow_hint - b.river_flow_hint).abs() <= 0.015
        && distance_delta >= 0.5
}

fn apply_same_context_river_core_step_limit(
    columns: &mut [HeightfieldColumn],
    width: usize,
    height: usize,
) {
    let mut core_y = columns
        .iter()
        .map(|column| column.surface_y)
        .collect::<Vec<_>>();
    for _ in 0..(width + height).max(1) {
        let mut changed = false;
        for index in 0..columns.len() {
            if !is_same_context_river_core_candidate(columns[index]) {
                continue;
            }
            let mut allowed = core_y[index];
            for neighbor in neighbor_indices(index, width, height) {
                if !same_context_river_core_pair(columns[index], columns[neighbor]) {
                    continue;
                }
                allowed = allowed
                    .min(core_y[neighbor].saturating_add(MAX_SAME_CONTEXT_RIVER_CORE_STEP_BLOCKS));
            }
            if allowed < core_y[index] {
                core_y[index] = allowed;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for (index, column) in columns.iter_mut().enumerate() {
        if core_y[index] >= column.surface_y {
            continue;
        }
        column.surface_y = core_y[index];
        column.surface_height_blocks = core_y[index] as f32;
        column.constrained_surface_height_blocks = column
            .constrained_surface_height_blocks
            .min(column.surface_height_blocks);
    }
}

fn is_same_context_river_core_candidate(column: HeightfieldColumn) -> bool {
    matches!(column.terrain_kind, HeightfieldTerrainKind::RiverCore)
        && column.water_y.is_some()
        && column.river_core_strength >= RIVER_CORE_WATER_STRENGTH
        && column.river_flow_hint > 0.0
        && column.river_distance_blocks.is_finite()
}

fn same_context_river_core_pair(a: HeightfieldColumn, b: HeightfieldColumn) -> bool {
    is_same_context_river_core_candidate(a)
        && is_same_context_river_core_candidate(b)
        && (a.river_flow_hint - b.river_flow_hint).abs() <= 0.03
        && (a.river_core_strength - b.river_core_strength).abs() <= 0.08
        && (a.river_distance_blocks - b.river_distance_blocks).abs() <= 2.0
}
