use super::{HeightfieldColumn, HeightfieldConfig, HeightfieldTerrainKind};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HeightfieldTileStats {
    pub column_count: usize,
    pub min_surface_height_blocks: f32,
    pub max_surface_height_blocks: f32,
    pub average_surface_height_blocks: f32,
    pub contour_step_blocks: f32,
    pub contour_min_gap_blocks: f32,
    pub contour_river_min_gap_blocks: f32,
    pub contour_band_smoothing: f32,
    pub max_raw_neighbor_delta_blocks: f32,
    pub max_contour_guided_neighbor_delta_blocks: f32,
    pub max_constrained_neighbor_delta_blocks: f32,
    pub max_snapped_neighbor_delta_blocks: f32,
    pub max_visible_neighbor_delta_blocks: f32,
    pub max_shore_visible_neighbor_delta_blocks: f32,
    pub min_ocean_visible_surface_blocks: f32,
    pub max_ocean_visible_surface_blocks: f32,
    pub min_land_near_water_surface_blocks: f32,
    pub max_river_water_neighbor_delta_blocks: f32,
    pub river_uphill_flow_neighbor_count: usize,
    pub water_column_count: usize,
    pub ocean_column_count: usize,
    pub lake_column_count: usize,
    pub river_core_column_count: usize,
    pub dry_basin_column_count: usize,
    pub ridge_column_count: usize,
}

pub(super) fn neighbor_indices(
    index: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = usize> {
    let x = index % width;
    let z = index / width;
    let mut neighbors = [None; 4];
    if x > 0 {
        neighbors[0] = Some(index - 1);
    }
    if x + 1 < width {
        neighbors[1] = Some(index + 1);
    }
    if z > 0 {
        neighbors[2] = Some(index - width);
    }
    if z + 1 < height {
        neighbors[3] = Some(index + width);
    }
    neighbors
        .into_iter()
        .flatten()
        .filter(move |neighbor| *neighbor < width * height)
}

pub(super) fn is_standing_water(column: HeightfieldColumn) -> bool {
    matches!(
        column.terrain_kind,
        HeightfieldTerrainKind::Ocean | HeightfieldTerrainKind::Lake
    )
}

pub(super) fn heightfield_stats(
    columns: &[HeightfieldColumn],
    config: HeightfieldConfig,
) -> HeightfieldTileStats {
    if columns.is_empty() {
        return HeightfieldTileStats::default();
    }

    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut sum = 0.0;
    let mut water = 0usize;
    let mut ocean = 0usize;
    let mut lake = 0usize;
    let mut dry = 0usize;
    let mut ridge = 0usize;
    let mut min_ocean_visible = f32::INFINITY;
    let mut max_ocean_visible = f32::NEG_INFINITY;
    let mut found_ocean = false;

    for column in columns {
        min = min.min(column.surface_height_blocks);
        max = max.max(column.surface_height_blocks);
        sum += column.surface_height_blocks;
        if column.has_water_column() {
            water += 1;
        }
        match column.terrain_kind {
            HeightfieldTerrainKind::Ocean => {
                ocean += 1;
                found_ocean = true;
                let visible = column.visible_surface_height_blocks();
                min_ocean_visible = min_ocean_visible.min(visible);
                max_ocean_visible = max_ocean_visible.max(visible);
            }
            HeightfieldTerrainKind::Lake => lake += 1,
            HeightfieldTerrainKind::RiverCore => {}
            HeightfieldTerrainKind::RiverBed => {}
            HeightfieldTerrainKind::DryBasin => dry += 1,
            HeightfieldTerrainKind::Ridge => ridge += 1,
            HeightfieldTerrainKind::Coast | HeightfieldTerrainKind::Land => {}
        }
    }

    HeightfieldTileStats {
        column_count: columns.len(),
        min_surface_height_blocks: min,
        max_surface_height_blocks: max,
        average_surface_height_blocks: sum / columns.len() as f32,
        contour_step_blocks: config.contour.step_blocks,
        contour_min_gap_blocks: config.contour.min_gap_blocks,
        contour_river_min_gap_blocks: config.contour.river_min_gap_blocks,
        contour_band_smoothing: config.contour.band_smoothing,
        max_raw_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.raw_surface_height_blocks
        }),
        max_contour_guided_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.contour_guided_surface_height_blocks
        }),
        max_constrained_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.constrained_surface_height_blocks
        }),
        max_snapped_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.surface_height_blocks
        }),
        max_visible_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.visible_surface_height_blocks()
        }),
        max_shore_visible_neighbor_delta_blocks: max_shore_neighbor_delta(columns),
        min_ocean_visible_surface_blocks: found_ocean.then_some(min_ocean_visible).unwrap_or(0.0),
        max_ocean_visible_surface_blocks: found_ocean.then_some(max_ocean_visible).unwrap_or(0.0),
        min_land_near_water_surface_blocks: min_land_near_standing_water(columns),
        max_river_water_neighbor_delta_blocks: max_river_water_neighbor_delta(columns),
        river_uphill_flow_neighbor_count: river_uphill_flow_neighbor_count(columns),
        water_column_count: water,
        ocean_column_count: ocean,
        lake_column_count: lake,
        river_core_column_count: 0,
        dry_basin_column_count: dry,
        ridge_column_count: ridge,
    }
}

fn min_land_near_standing_water(columns: &[HeightfieldColumn]) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let height = columns.len().div_ceil(width);
    let mut min_land = f32::INFINITY;
    for (index, column) in columns.iter().enumerate() {
        if is_standing_water(*column) {
            continue;
        }
        if neighbor_indices(index, width, height)
            .any(|neighbor| is_standing_water(columns[neighbor]))
        {
            min_land = min_land.min(column.surface_height_blocks);
        }
    }
    min_land.is_finite().then_some(min_land).unwrap_or(0.0)
}

fn max_river_water_neighbor_delta(_columns: &[HeightfieldColumn]) -> f32 {
    0.0
}

fn river_uphill_flow_neighbor_count(_columns: &[HeightfieldColumn]) -> usize {
    0
}

fn max_shore_neighbor_delta(columns: &[HeightfieldColumn]) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let mut max_delta = 0.0f32;
    for (index, column) in columns.iter().enumerate() {
        if index + 1 < columns.len() && (index + 1) % width != 0 {
            let neighbor = &columns[index + 1];
            if is_standing_water(*column) || is_standing_water(*neighbor) {
                max_delta = max_delta.max(
                    (column.visible_surface_height_blocks()
                        - neighbor.visible_surface_height_blocks())
                    .abs(),
                );
            }
        }
        if index + width < columns.len() {
            let neighbor = &columns[index + width];
            if is_standing_water(*column) || is_standing_water(*neighbor) {
                max_delta = max_delta.max(
                    (column.visible_surface_height_blocks()
                        - neighbor.visible_surface_height_blocks())
                    .abs(),
                );
            }
        }
    }
    max_delta
}

fn max_neighbor_delta(
    columns: &[HeightfieldColumn],
    value: impl Fn(&HeightfieldColumn) -> f32,
) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let mut max_delta = 0.0f32;
    for (index, column) in columns.iter().enumerate() {
        if index + 1 < columns.len() && (index + 1) % width != 0 {
            max_delta = max_delta.max((value(column) - value(&columns[index + 1])).abs());
        }
        if index + width < columns.len() {
            max_delta = max_delta.max((value(column) - value(&columns[index + width])).abs());
        }
    }
    max_delta
}

fn infer_row_width(columns: &[HeightfieldColumn]) -> usize {
    if columns.len() < 2 {
        return columns.len().max(1);
    }
    let first_z = columns[0].position.z;
    columns
        .iter()
        .position(|column| (column.position.z - first_z).abs() > f32::EPSILON)
        .unwrap_or(columns.len())
        .max(1)
}
