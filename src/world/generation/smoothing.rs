use crate::world::atlas::RiverPathKind;
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::corridors::sample_corridor_axis;
use super::{ChunkCorridorWindow, MesoAppliedPrototype, RiverCorridorConstraint};

const BASE_SMOOTHING_BLEND: f32 = 0.34;
const MAX_COLUMN_ADJUSTMENT: f32 = 1.5;
const MAX_RELIEF_SPEND_FRACTION: f32 = 0.24;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothedColumn {
    pub height: f32,
    pub remaining_relief_budget: f32,
    pub local_slope: f32,
    pub concavity: f32,
    pub material_support: super::RealizationMaterialSupport,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmoothedPrototype {
    pub chunk: ChunkCoord,
    pub columns: Vec<SmoothedColumn>,
    pub preserved_corridors: usize,
}

pub fn empty_smoothed_prototype(chunk: ChunkCoord) -> SmoothedPrototype {
    SmoothedPrototype {
        chunk,
        columns: Vec::new(),
        preserved_corridors: 0,
    }
}

pub fn build_chunk_smoothed_prototype(
    chunk: ChunkCoord,
    corridor_window: &ChunkCorridorWindow,
    meso: &MesoAppliedPrototype,
) -> SmoothedPrototype {
    debug_assert_eq!(corridor_window.chunk, chunk);
    debug_assert_eq!(meso.chunk, chunk);

    let mut smoothed_heights = Vec::with_capacity(meso.columns.len());
    let mut remaining_budgets = Vec::with_capacity(meso.columns.len());
    let mut preserved_corridors = 0usize;

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = column_index(local_x, local_z);
            let center = meso.columns[index];
            let corridor_preservation = corridor_preservation_factor(
                local_x as f32 + 0.5,
                local_z as f32 + 0.5,
                &corridor_window.corridors,
            );

            if corridor_preservation >= 0.55 {
                preserved_corridors += 1;
            }

            if is_chunk_border(local_x, local_z) {
                smoothed_heights.push(center.height);
                remaining_budgets.push(center.remaining_relief_budget);
                continue;
            }

            let orthogonal_average = orthogonal_average_height(&meso.columns, local_x, local_z);
            let local_relief =
                orthogonal_relief_range(&meso.columns, local_x, local_z, center.height);
            let shape_preservation = smoothstep_range(1.0, 6.0, local_relief);
            let form_preservation =
                smoothstep_range(0.75, 4.0, (center.height - orthogonal_average).abs());
            let budget_scale = (center.remaining_relief_budget / 18.0).clamp(0.18, 1.0);
            let blend = BASE_SMOOTHING_BLEND
                * budget_scale
                * (1.0 - corridor_preservation)
                * (1.0 - shape_preservation * 0.80)
                * (1.0 - form_preservation * 0.45);

            let target_delta = (orthogonal_average - center.height) * blend.clamp(0.0, 1.0);
            let max_adjustment = (center.remaining_relief_budget * MAX_RELIEF_SPEND_FRACTION)
                .clamp(0.35, MAX_COLUMN_ADJUSTMENT);
            let applied_delta = target_delta.clamp(-max_adjustment, max_adjustment);
            let spent_relief = applied_delta
                .abs()
                .min(center.remaining_relief_budget * MAX_RELIEF_SPEND_FRACTION);

            smoothed_heights.push(center.height + applied_delta);
            remaining_budgets.push((center.remaining_relief_budget - spent_relief).max(0.0));
        }
    }

    let columns = smoothed_heights
        .iter()
        .enumerate()
        .map(|(index, height)| {
            let local_x = (index % CHUNK_EDGE_I32 as usize) as i32;
            let local_z = (index / CHUNK_EDGE_I32 as usize) as i32;

            let local_slope = local_slope(&smoothed_heights, local_x, local_z);
            let concavity = local_concavity(&smoothed_heights, local_x, local_z);

            SmoothedColumn {
                height: *height,
                remaining_relief_budget: remaining_budgets[index],
                local_slope,
                concavity,
                material_support: refine_smoothed_material_support(
                    meso.columns[index].material_support,
                    local_slope,
                    concavity,
                    remaining_budgets[index],
                ),
            }
        })
        .collect();

    SmoothedPrototype {
        chunk,
        columns,
        preserved_corridors,
    }
}

fn refine_smoothed_material_support(
    support: super::RealizationMaterialSupport,
    _local_slope: f32,
    _concavity: f32,
    _remaining_relief_budget: f32,
) -> super::RealizationMaterialSupport {
    support.finalized()
}

fn is_chunk_border(local_x: i32, local_z: i32) -> bool {
    local_x == 0 || local_z == 0 || local_x == CHUNK_EDGE_I32 - 1 || local_z == CHUNK_EDGE_I32 - 1
}

fn orthogonal_average_height(
    columns: &[super::MesoAppliedColumn],
    local_x: i32,
    local_z: i32,
) -> f32 {
    let west = columns[column_index(local_x - 1, local_z)].height;
    let east = columns[column_index(local_x + 1, local_z)].height;
    let north = columns[column_index(local_x, local_z - 1)].height;
    let south = columns[column_index(local_x, local_z + 1)].height;

    (west + east + north + south) * 0.25
}

fn orthogonal_relief_range(
    columns: &[super::MesoAppliedColumn],
    local_x: i32,
    local_z: i32,
    center_height: f32,
) -> f32 {
    let west = (columns[column_index(local_x - 1, local_z)].height - center_height).abs();
    let east = (columns[column_index(local_x + 1, local_z)].height - center_height).abs();
    let north = (columns[column_index(local_x, local_z - 1)].height - center_height).abs();
    let south = (columns[column_index(local_x, local_z + 1)].height - center_height).abs();

    west.max(east).max(north).max(south)
}

fn local_slope(heights: &[f32], local_x: i32, local_z: i32) -> f32 {
    let west = sampled_height(heights, local_x - 1, local_z);
    let east = sampled_height(heights, local_x + 1, local_z);
    let north = sampled_height(heights, local_x, local_z - 1);
    let south = sampled_height(heights, local_x, local_z + 1);
    let dx = (east - west) * 0.5;
    let dz = (south - north) * 0.5;

    (dx * dx + dz * dz).sqrt()
}

fn local_concavity(heights: &[f32], local_x: i32, local_z: i32) -> f32 {
    let center = sampled_height(heights, local_x, local_z);
    let west = sampled_height(heights, local_x - 1, local_z);
    let east = sampled_height(heights, local_x + 1, local_z);
    let north = sampled_height(heights, local_x, local_z - 1);
    let south = sampled_height(heights, local_x, local_z + 1);
    let orthogonal_average = (west + east + north + south) * 0.25;

    orthogonal_average - center
}

fn sampled_height(heights: &[f32], local_x: i32, local_z: i32) -> f32 {
    let clamped_x = local_x.clamp(0, CHUNK_EDGE_I32 - 1);
    let clamped_z = local_z.clamp(0, CHUNK_EDGE_I32 - 1);
    heights[column_index(clamped_x, clamped_z)]
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize
}

fn corridor_preservation_factor(
    local_x: f32,
    local_z: f32,
    corridors: &[RiverCorridorConstraint],
) -> f32 {
    let mut strongest = 0.0_f32;

    for corridor in corridors {
        let projected = sample_corridor_axis(*corridor, local_x, local_z);
        let keep_radius = corridor.half_width_blocks
            * match corridor.kind {
                RiverPathKind::Trunk => 2.05,
                RiverPathKind::Tributary => 1.65,
            }
            + 8.0;
        let distance_ratio = projected.distance_blocks / keep_radius.max(f32::EPSILON);
        let influence = smoothstep_range(1.05, 0.0, distance_ratio)
            * match corridor.kind {
                RiverPathKind::Trunk => 1.0,
                RiverPathKind::Tributary => 0.80,
            };
        strongest = strongest.max(influence);
    }

    strongest.clamp(0.0, 1.0)
}

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_meso(chunk: ChunkCoord, base_height: f32, relief_budget: f32) -> MesoAppliedPrototype {
        let columns = vec![
            super::super::MesoAppliedColumn {
                height: base_height,
                remaining_relief_budget: relief_budget,
                material_support: super::super::RealizationMaterialSupport::default(),
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];

        MesoAppliedPrototype {
            chunk,
            columns,
            applied_feature_keys: Vec::new(),
        }
    }

    fn center_index() -> usize {
        column_index(CHUNK_EDGE_I32 / 2, CHUNK_EDGE_I32 / 2)
    }

    #[test]
    fn smoothing_is_deterministic_and_emits_a_full_grid() {
        let chunk = ChunkCoord(0, 0, 0);
        let corridor_window = ChunkCorridorWindow {
            chunk,
            corridors: Vec::new(),
        };
        let mut meso = flat_meso(chunk, 100.0, 12.0);
        meso.columns[center_index()].height = 108.0;

        let a = build_chunk_smoothed_prototype(chunk, &corridor_window, &meso);
        let b = build_chunk_smoothed_prototype(chunk, &corridor_window, &meso);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert_eq!(a.columns.len(), meso.columns.len());
        assert!(a.columns.iter().all(|column| {
            column.height.is_finite()
                && column.remaining_relief_budget.is_finite()
                && column.local_slope.is_finite()
                && column.concavity.is_finite()
                && column.remaining_relief_budget >= 0.0
        }));
    }

    #[test]
    fn smoothing_softens_an_isolated_interior_spike() {
        let chunk = ChunkCoord(0, 0, 0);
        let corridor_window = ChunkCorridorWindow {
            chunk,
            corridors: Vec::new(),
        };
        let mut meso = flat_meso(chunk, 100.0, 12.0);
        let index = center_index();
        meso.columns[index].height = 110.0;

        let smoothed = build_chunk_smoothed_prototype(chunk, &corridor_window, &meso);

        assert!(
            smoothed.columns[index].height < meso.columns[index].height,
            "expected smoothing to lower an isolated spike, before={} after={}",
            meso.columns[index].height,
            smoothed.columns[index].height
        );
        assert!(
            smoothed.columns[index].remaining_relief_budget
                < meso.columns[index].remaining_relief_budget
        );
    }

    #[test]
    fn smoothing_leaves_chunk_borders_unchanged() {
        let chunk = ChunkCoord(0, 0, 0);
        let corridor_window = ChunkCorridorWindow {
            chunk,
            corridors: Vec::new(),
        };
        let mut meso = flat_meso(chunk, 100.0, 12.0);
        meso.columns[column_index(0, 7)].height = 106.0;
        meso.columns[column_index(31, 12)].height = 94.0;

        let smoothed = build_chunk_smoothed_prototype(chunk, &corridor_window, &meso);

        assert_eq!(
            smoothed.columns[column_index(0, 7)].height,
            meso.columns[column_index(0, 7)].height
        );
        assert_eq!(
            smoothed.columns[column_index(31, 12)].height,
            meso.columns[column_index(31, 12)].height
        );
    }

    #[test]
    fn smoothing_preserves_corridor_columns_more_than_open_ground() {
        let chunk = ChunkCoord(0, 0, 0);
        let open_corridor_window = ChunkCorridorWindow {
            chunk,
            corridors: Vec::new(),
        };
        let protected_corridor_window = ChunkCorridorWindow {
            chunk,
            corridors: vec![RiverCorridorConstraint {
                river_id: 1,
                basin_id: 1,
                main_stem_river_id: 1,
                parent_river_id: None,
                kind: RiverPathKind::Trunk,
                order: 3,
                start_x: 0.0,
                start_z: 16.0,
                end_x: 32.0,
                end_z: 16.0,
                center_x: 16.0,
                center_z: 16.0,
                half_width_blocks: 5.0,
                downstream_grade_per_block: 0.002,
                downstream_cells_start: 0.0,
                downstream_cells_end: 1.0,
            }],
        };
        let mut meso = flat_meso(chunk, 100.0, 12.0);
        let index = center_index();
        meso.columns[index].height = 92.0;

        let open = build_chunk_smoothed_prototype(chunk, &open_corridor_window, &meso);
        let protected = build_chunk_smoothed_prototype(chunk, &protected_corridor_window, &meso);
        let open_delta = (open.columns[index].height - meso.columns[index].height).abs();
        let protected_delta = (protected.columns[index].height - meso.columns[index].height).abs();

        assert!(
            protected_delta < open_delta,
            "expected corridor-aware smoothing to preserve the local valley more strongly, open_delta={open_delta} protected_delta={protected_delta}"
        );
        assert!(protected.preserved_corridors > 0);
    }

    #[test]
    fn smoothing_derives_signed_concavity_from_smoothed_surface() {
        let chunk = ChunkCoord(0, 0, 0);
        let corridor_window = ChunkCorridorWindow {
            chunk,
            corridors: Vec::new(),
        };
        let mut meso = flat_meso(chunk, 100.0, 12.0);
        let index = center_index();
        meso.columns[index].height = 96.0;

        let smoothed = build_chunk_smoothed_prototype(chunk, &corridor_window, &meso);

        assert!(smoothed.columns[index].concavity > 0.0);
        assert!(smoothed.columns[index].local_slope >= 0.0);
    }
}
