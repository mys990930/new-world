use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::world::WorldMeta;

use super::scale::{AtlasArea, AtlasCoord, AtlasGrid};
use super::seed::{
    SALT_CONTINENT_PRIMARY, SALT_CONTINENT_SECONDARY, SALT_COAST_ROUGHNESS, SALT_DETAIL,
    SALT_HUMIDITY, SALT_MOUNTAIN_CLUSTER, SALT_RIDGE_PRIMARY, SALT_RIDGE_SECONDARY,
    SALT_TEMPERATURE, domain_warp, fbm, ridged_fbm,
};

const LAND_THRESHOLD: f32 = 0.53;
const OCEAN_DISTANCE_NORMALIZER: f32 = 24.0;
const COAST_DISTANCE_NORMALIZER: f32 = 12.0;
const CONTINENT_CORE_NORMALIZER: f32 = 22.0;
const RIVER_DISTANCE_NORMALIZER: f32 = 10.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ThermalWeights {
    pub polar: f32,
    pub cold: f32,
    pub temperate: f32,
    pub warm: f32,
    pub hot: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoistureWeights {
    pub arid: f32,
    pub semi_arid: f32,
    pub subhumid: f32,
    pub humid: f32,
    pub wet: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TerrainFormWeights {
    pub plain: f32,
    pub hill: f32,
    pub mountain: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OverlayWeights {
    pub ocean: f32,
    pub coast: f32,
    pub riverine: f32,
    pub wetland: f32,
    pub alpine: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CoverPotentials {
    pub openness: f32,
    pub grass_potential: f32,
    pub shrub_potential: f32,
    pub canopy_potential: f32,
    pub forest_potential: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AtlasCell {
    pub landness: f32,
    pub ocean_distance: f32,
    pub coast_distance: f32,
    pub continent_id: u32,
    pub continent_core_factor: f32,
    pub macro_elevation: f32,
    pub slope: f32,
    pub ruggedness: f32,
    pub ridge_factor: f32,
    pub mountain_mass: f32,
    pub basinness: f32,
    pub pass_potential: f32,
    pub river_source_potential: f32,
    pub river_flow_potential: f32,
    pub river_distance_estimate: f32,
    pub lake_potential: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub inlandness: f32,
    pub aridity: f32,
    pub wetness: f32,
    pub polar_factor: f32,
    pub alpine_factor: f32,
    pub coast_factor: f32,
    pub wetland_factor: f32,
    pub riverine_factor: f32,
    pub ecotone_strength: f32,
    pub thermal: ThermalWeights,
    pub moisture: MoistureWeights,
    pub form: TerrainFormWeights,
    pub overlay: OverlayWeights,
    pub cover: CoverPotentials,
}

#[derive(Debug, Clone)]
pub struct AtlasFieldMap {
    cells: AtlasGrid<AtlasCell>,
}

impl AtlasFieldMap {
    pub fn cells(&self) -> &AtlasGrid<AtlasCell> {
        &self.cells
    }

    pub fn area(&self) -> AtlasArea {
        self.cells.area()
    }

    pub fn get(&self, coord: AtlasCoord) -> Option<&AtlasCell> {
        self.cells.get(coord)
    }
}

pub fn generate_atlas_fields(meta: &WorldMeta, area: AtlasArea) -> AtlasFieldMap {
    let width = area.width();
    let height = area.height();
    let len = area.len();

    let mut cells = vec![AtlasCell::default(); len];
    let mut land_mask = vec![false; len];
    let mut temperature_noise = vec![0.0_f32; len];
    let mut humidity_noise = vec![0.0_f32; len];
    let mut height_field = vec![0.0_f32; len];

    for coord in area.coords() {
        let index = area.index_of(coord).expect("atlas coord must index into area");
        let landness = sample_landness(meta.seed, coord);
        let is_land = landness >= LAND_THRESHOLD;

        cells[index].landness = landness;
        cells[index].ridge_factor = sample_ridge_factor(meta.seed, coord) * smoothstep(0.35, 0.95, landness);
        land_mask[index] = is_land;
        temperature_noise[index] = fbm(meta.seed, coord.x as f64 / 64.0, coord.z as f64 / 64.0, 4, 2.0, 0.5, SALT_TEMPERATURE);
        humidity_noise[index] = fbm(meta.seed, coord.x as f64 / 48.0, coord.z as f64 / 48.0, 5, 2.0, 0.5, SALT_HUMIDITY);
    }

    let ocean_distance_raw = compute_distance_to_value(&land_mask, width, height, false);
    let land_distance_raw = compute_distance_to_value(&land_mask, width, height, true);
    let continent_ids = assign_continent_ids(&land_mask, width, height);

    for index in 0..len {
        let coord = area.coord_at(index).expect("index should map to coord");
        let is_land = land_mask[index];
        let ocean_distance_cells = if is_land {
            ocean_distance_raw[index] as f32 / 1000.0
        } else {
            0.0
        };
        let coast_distance_cells = coast_distance_cells(&land_mask, &ocean_distance_raw, &land_distance_raw, index);
        let continent_core = if is_land {
            clamp01(ocean_distance_cells / CONTINENT_CORE_NORMALIZER)
        } else {
            0.0
        };
        let coast_factor = if is_land {
            1.0 - clamp01(coast_distance_cells / 6.0)
        } else {
            0.0
        };
        let mountain_mass = sample_mountain_mass(
            meta.seed,
            coord,
            cells[index].ridge_factor,
            continent_core,
            cells[index].landness,
        );
        let continental_lift = smoothstep(LAND_THRESHOLD, 1.0, cells[index].landness);
        let macro_elevation = if is_land {
            clamp01(
                0.06
                    + continental_lift * 0.18
                    + continent_core * 0.42
                    + mountain_mass * 0.26
                    - coast_factor * 0.08,
            )
        } else {
            clamp01(cells[index].landness * 0.08)
        };

        cells[index].continent_id = continent_ids[index];
        cells[index].ocean_distance = clamp01(ocean_distance_cells / OCEAN_DISTANCE_NORMALIZER);
        cells[index].coast_distance = clamp01(coast_distance_cells / COAST_DISTANCE_NORMALIZER);
        cells[index].continent_core_factor = continent_core;
        cells[index].coast_factor = coast_factor;
        cells[index].mountain_mass = mountain_mass;
        cells[index].macro_elevation = macro_elevation;
        height_field[index] = macro_elevation;
    }

    for index in 0..len {
        if !land_mask[index] {
            continue;
        }

        let coord = area.coord_at(index).expect("index should map to coord");
        let (neighbor_mean, mean_diff, max_diff) = local_neighbor_stats(area, &height_field, index);
        let detail = fbm(meta.seed, coord.x as f64 / 12.0, coord.z as f64 / 12.0, 3, 2.0, 0.5, SALT_DETAIL);

        cells[index].slope = clamp01(max_diff * 3.25);
        cells[index].ruggedness = clamp01(cells[index].ridge_factor * 0.52 + cells[index].slope * 0.33 + detail * 0.15);
        cells[index].basinness = clamp01((neighbor_mean - height_field[index] + 0.05) * 4.0);
        cells[index].pass_potential = clamp01(
            cells[index].mountain_mass * (1.0 - cells[index].ridge_factor * 0.68) * (1.0 - cells[index].slope * 0.45)
                + mean_diff * 0.25,
        );
    }

    for index in 0..len {
        let coord = area.coord_at(index).expect("index should map to coord");
        let equator_heat = 1.0 - ((coord.z as f32) / 320.0).abs().tanh();

        cells[index].inlandness = cells[index].ocean_distance;
        cells[index].temperature = clamp01(
            equator_heat * 0.62
                + temperature_noise[index] * 0.38
                - cells[index].macro_elevation * 0.46
                - cells[index].mountain_mass * 0.08,
        );
        cells[index].polar_factor = smoothstep(0.18, 0.02, cells[index].temperature);
        cells[index].humidity = clamp01(
            humidity_noise[index] * 0.52
                + (1.0 - cells[index].ocean_distance) * 0.32
                + cells[index].basinness * 0.16
                - cells[index].inlandness * 0.26
                - cells[index].mountain_mass * cells[index].inlandness * 0.18,
        );
        cells[index].river_source_potential = clamp01(
            cells[index].mountain_mass * 0.42 + cells[index].humidity * 0.34 + cells[index].slope * 0.24,
        );
    }

    let downhill = compute_downhill_targets(area, &land_mask, &height_field);
    let mut flow_accumulation = vec![0.0_f32; len];
    for index in 0..len {
        if !land_mask[index] {
            continue;
        }

        flow_accumulation[index] = clamp01(
            0.12
                + cells[index].humidity * 0.58
                + cells[index].mountain_mass * 0.20
                + (1.0 - cells[index].temperature) * 0.10,
        );
    }

    let mut order = (0..len).collect::<Vec<_>>();
    order.sort_by(|left, right| height_field[*right].total_cmp(&height_field[*left]));
    for &index in &order {
        if let Some(target) = downhill[index] {
            flow_accumulation[target] += flow_accumulation[index];
        }
    }

    let max_flow = flow_accumulation
        .iter()
        .copied()
        .fold(0.0_f32, f32::max)
        .max(1.0);

    let mut river_mask = vec![false; len];
    let mut lake_mask = vec![false; len];
    for index in 0..len {
        if !land_mask[index] {
            continue;
        }

        let flow = flow_accumulation[index] / max_flow;
        cells[index].river_flow_potential = clamp01(flow);
        cells[index].lake_potential = clamp01(
            cells[index].basinness * 0.54
                + flow * 0.26
                + if downhill[index].is_none() { 0.20 } else { 0.0 }
                + (1.0 - cells[index].slope) * 0.10,
        );

        river_mask[index] = flow > 0.045
            && (cells[index].river_source_potential > 0.38 || flow > 0.10)
            && cells[index].macro_elevation > 0.10;
        lake_mask[index] =
            cells[index].lake_potential > 0.62 && cells[index].macro_elevation > 0.08 && cells[index].slope < 0.45;
    }

    let mut waterline_mask = vec![false; len];
    for index in 0..len {
        waterline_mask[index] = river_mask[index] || lake_mask[index];
    }
    let river_distance_raw = compute_distance_to_value(&waterline_mask, width, height, true);

    for index in 0..len {
        let river_distance_cells = river_distance_raw[index] as f32 / 1000.0;
        cells[index].river_distance_estimate = clamp01(river_distance_cells / RIVER_DISTANCE_NORMALIZER);
        cells[index].riverine_factor = if land_mask[index] {
            clamp01((1.0 - cells[index].river_distance_estimate) * 0.74 + cells[index].river_flow_potential * 0.26)
        } else {
            0.0
        };
        cells[index].humidity = clamp01(cells[index].humidity + cells[index].riverine_factor * 0.18 + cells[index].lake_potential * 0.08);
        cells[index].wetness = clamp01(
            cells[index].humidity * 0.55
                + cells[index].riverine_factor * 0.25
                + cells[index].lake_potential * 0.10
                + cells[index].basinness * 0.10,
        );
        cells[index].aridity = clamp01(
            (1.0 - cells[index].humidity) * 0.60
                + cells[index].inlandness * 0.30
                + (1.0 - cells[index].wetness) * 0.10
                - cells[index].riverine_factor * 0.15,
        );
        cells[index].alpine_factor = clamp01(
            smoothstep(0.60, 0.82, cells[index].macro_elevation) * 0.45
                + smoothstep(0.52, 0.76, cells[index].mountain_mass) * 0.55
                - cells[index].polar_factor * 0.10,
        );
        cells[index].wetland_factor = clamp01(
            cells[index].wetness * 0.45
                + cells[index].basinness * 0.35
                + cells[index].riverine_factor * 0.20
                - cells[index].slope * 0.25,
        );
        cells[index].overlay = overlay_weights(land_mask[index], &cells[index]);
        cells[index].thermal = thermal_weights(cells[index].temperature, cells[index].polar_factor);
        cells[index].moisture = moisture_weights(cells[index].humidity, cells[index].aridity, cells[index].wetness);
        cells[index].form = form_weights(cells[index].macro_elevation, cells[index].ruggedness, cells[index].mountain_mass);
        cells[index].cover = cover_potentials(land_mask[index], &cells[index]);
        cells[index].ecotone_strength = ecotone_strength(&cells[index]);
    }

    AtlasFieldMap {
        cells: AtlasGrid::from_values(area, cells),
    }
}

fn sample_landness(seed: u64, coord: AtlasCoord) -> f32 {
    let x = coord.x as f64;
    let z = coord.z as f64;
    let (wx, wz) = domain_warp(seed, x, z, 1.0 / 96.0, 6.5);
    let primary = fbm(seed, wx / 96.0, wz / 96.0, 5, 2.0, 0.55, SALT_CONTINENT_PRIMARY);
    let secondary = fbm(seed, wx / 28.0, wz / 28.0, 4, 2.1, 0.55, SALT_CONTINENT_SECONDARY);
    let islands = fbm(seed, x / 14.0, z / 14.0, 3, 2.0, 0.5, SALT_DETAIL);
    let coast = ridged_fbm(seed, wx / 20.0, wz / 20.0, 4, 2.0, 0.55, SALT_COAST_ROUGHNESS);

    clamp01(primary * 0.62 + secondary * 0.26 + islands * 0.12 - coast * 0.18 + 0.02)
}

fn sample_ridge_factor(seed: u64, coord: AtlasCoord) -> f32 {
    let x = coord.x as f64;
    let z = coord.z as f64;
    let (wx, wz) = domain_warp(seed, x, z, 1.0 / 18.0, 2.6);
    let primary = ridged_fbm(seed, wx / 18.0, wz / 18.0, 5, 2.03, 0.53, SALT_RIDGE_PRIMARY);
    let secondary = ridged_fbm(seed, wx / 42.0, wz / 42.0, 3, 2.0, 0.5, SALT_RIDGE_SECONDARY);

    clamp01(primary * 0.72 + secondary * 0.28)
}

fn sample_mountain_mass(seed: u64, coord: AtlasCoord, ridge_factor: f32, continent_core: f32, landness: f32) -> f32 {
    let cluster = fbm(
        seed,
        coord.x as f64 / 40.0,
        coord.z as f64 / 40.0,
        4,
        2.0,
        0.52,
        SALT_MOUNTAIN_CLUSTER,
    );
    let base = ridge_factor * 0.72 + cluster * 0.28;

    smoothstep(0.48, 0.82, base) * smoothstep(0.08, 0.35, continent_core + landness * 0.35)
}

fn overlay_weights(is_land: bool, cell: &AtlasCell) -> OverlayWeights {
    if !is_land {
        return OverlayWeights {
            ocean: 1.0,
            coast: 0.0,
            riverine: 0.0,
            wetland: 0.0,
            alpine: 0.0,
        };
    }

    OverlayWeights {
        ocean: 0.0,
        coast: cell.coast_factor,
        riverine: cell.riverine_factor,
        wetland: cell.wetland_factor,
        alpine: cell.alpine_factor,
    }
}

fn thermal_weights(temperature: f32, polar_factor: f32) -> ThermalWeights {
    let mut values = triangular_weights(temperature, [0.04, 0.22, 0.50, 0.72, 0.92], [0.18, 0.22, 0.24, 0.22, 0.18]);
    values[0] = values[0].max(polar_factor);
    normalize_weights(&mut values);

    ThermalWeights {
        polar: values[0],
        cold: values[1],
        temperate: values[2],
        warm: values[3],
        hot: values[4],
    }
}

fn moisture_weights(humidity: f32, aridity: f32, wetness: f32) -> MoistureWeights {
    let moisture_signal = clamp01(humidity * 0.72 + wetness * 0.18 - aridity * 0.20 + 0.15);
    let mut values = triangular_weights(moisture_signal, [0.06, 0.24, 0.50, 0.74, 0.94], [0.16, 0.20, 0.24, 0.20, 0.16]);
    values[0] = values[0].max(aridity * 0.72);
    values[4] = values[4].max(wetness * 0.72);
    normalize_weights(&mut values);

    MoistureWeights {
        arid: values[0],
        semi_arid: values[1],
        subhumid: values[2],
        humid: values[3],
        wet: values[4],
    }
}

fn form_weights(macro_elevation: f32, ruggedness: f32, mountain_mass: f32) -> TerrainFormWeights {
    let mountain = clamp01(smoothstep(0.54, 0.78, mountain_mass * 0.68 + ruggedness * 0.32));
    let hill = clamp01(
        smoothstep(0.24, 0.52, ruggedness + macro_elevation * 0.18) * (1.0 - mountain * 0.60),
    );
    let mut values = [clamp01(1.0 - mountain * 0.85 - hill * 0.55), hill, mountain];
    normalize_weights(&mut values);

    TerrainFormWeights {
        plain: values[0],
        hill: values[1],
        mountain: values[2],
    }
}

fn cover_potentials(is_land: bool, cell: &AtlasCell) -> CoverPotentials {
    if !is_land {
        return CoverPotentials::default();
    }

    let canopy = clamp01(
        cell.humidity * 0.44
            + cell.wetness * 0.20
            + (cell.thermal.temperate + cell.thermal.warm * 0.35) * 0.20
            - cell.aridity * 0.34
            - cell.overlay.alpine * 0.20,
    );
    let forest = clamp01(canopy * 0.70 + cell.overlay.riverine * 0.15 + cell.moisture.humid * 0.15);
    let openness = clamp01(0.45 + cell.aridity * 0.25 + cell.form.plain * 0.15 - canopy * 0.35 - cell.moisture.wet * 0.10);
    let grass = clamp01(
        openness * 0.45
            + cell.moisture.subhumid * 0.20
            + cell.moisture.humid * 0.15
            + cell.thermal.temperate * 0.10,
    );
    let shrub = clamp01(cell.aridity * 0.35 + cell.moisture.semi_arid * 0.25 + cell.form.hill * 0.15 + 0.15);

    CoverPotentials {
        openness,
        grass_potential: grass,
        shrub_potential: shrub,
        canopy_potential: canopy,
        forest_potential: forest,
    }
}

fn ecotone_strength(cell: &AtlasCell) -> f32 {
    let thermal_competition = 1.0 - max5([
        cell.thermal.polar,
        cell.thermal.cold,
        cell.thermal.temperate,
        cell.thermal.warm,
        cell.thermal.hot,
    ]);
    let moisture_competition = 1.0 - max5([
        cell.moisture.arid,
        cell.moisture.semi_arid,
        cell.moisture.subhumid,
        cell.moisture.humid,
        cell.moisture.wet,
    ]);
    let overlay_competition = second_highest([
        cell.overlay.ocean,
        cell.overlay.coast,
        cell.overlay.riverine,
        cell.overlay.wetland,
        cell.overlay.alpine,
    ]);

    clamp01(thermal_competition * 0.40 + moisture_competition * 0.40 + overlay_competition * 0.20)
}

fn triangular_weights(value: f32, centers: [f32; 5], widths: [f32; 5]) -> [f32; 5] {
    let mut values = [0.0_f32; 5];
    for index in 0..5 {
        values[index] = clamp01(1.0 - ((value - centers[index]).abs() / widths[index]));
    }
    values
}

fn normalize_weights<const N: usize>(values: &mut [f32; N]) {
    let sum = values.iter().copied().sum::<f32>();
    if sum <= f32::EPSILON {
        let default = 1.0 / N as f32;
        for value in values.iter_mut() {
            *value = default;
        }
        return;
    }

    for value in values.iter_mut() {
        *value /= sum;
    }
}

fn local_neighbor_stats(area: AtlasArea, field: &[f32], index: usize) -> (f32, f32, f32) {
    let center = field[index];
    let mut sum = 0.0_f32;
    let mut sum_diff = 0.0_f32;
    let mut max_diff = 0.0_f32;
    let mut count = 0_u32;

    for_each_neighbor(index, area.width(), area.height(), |neighbor, _| {
        let value = field[neighbor];
        sum += value;
        let diff = (center - value).abs();
        sum_diff += diff;
        max_diff = max_diff.max(diff);
        count += 1;
    });

    if count == 0 {
        return (center, 0.0, 0.0);
    }

    (sum / count as f32, sum_diff / count as f32, max_diff)
}

fn assign_continent_ids(mask: &[bool], width: u32, height: u32) -> Vec<u32> {
    let mut ids = vec![0_u32; mask.len()];
    let mut next_id = 1_u32;

    for start in 0..mask.len() {
        if !mask[start] || ids[start] != 0 {
            continue;
        }

        let mut queue = VecDeque::new();
        queue.push_back(start);
        ids[start] = next_id;

        while let Some(index) = queue.pop_front() {
            for_each_cardinal_neighbor(index, width, height, |neighbor| {
                if mask[neighbor] && ids[neighbor] == 0 {
                    ids[neighbor] = next_id;
                    queue.push_back(neighbor);
                }
            });
        }

        next_id += 1;
    }

    ids
}

fn compute_distance_to_value(mask: &[bool], width: u32, height: u32, source_value: bool) -> Vec<u32> {
    let len = mask.len();
    let mut distances = vec![u32::MAX / 4; len];
    let mut queue = BinaryHeap::new();
    let mut source_count = 0_usize;

    for index in 0..len {
        if mask[index] == source_value {
            distances[index] = 0;
            queue.push((Reverse(0_u32), index));
            source_count += 1;
        }
    }

    if source_count == 0 {
        return vec![0_u32; len];
    }

    while let Some((Reverse(cost), index)) = queue.pop() {
        if cost != distances[index] {
            continue;
        }

        for_each_neighbor(index, width, height, |neighbor, step_cost| {
            let next = cost.saturating_add(step_cost);
            if next < distances[neighbor] {
                distances[neighbor] = next;
                queue.push((Reverse(next), neighbor));
            }
        });
    }

    distances
}

fn compute_downhill_targets(area: AtlasArea, land_mask: &[bool], field: &[f32]) -> Vec<Option<usize>> {
    let mut downhill = vec![None; field.len()];

    for index in 0..field.len() {
        if !land_mask[index] {
            continue;
        }

        let current = field[index];
        let mut best_target = None;
        let mut best_height = current;

        for_each_neighbor(index, area.width(), area.height(), |neighbor, _| {
            let candidate = field[neighbor];
            if candidate + 0.0001 < best_height {
                best_height = candidate;
                best_target = Some(neighbor);
            }
        });

        downhill[index] = best_target;
    }

    downhill
}

fn coast_distance_cells(land_mask: &[bool], ocean_distance_raw: &[u32], land_distance_raw: &[u32], index: usize) -> f32 {
    let raw = if land_mask[index] {
        ocean_distance_raw[index]
    } else {
        land_distance_raw[index]
    };
    raw as f32 / 1000.0
}

fn for_each_neighbor(index: usize, width: u32, height: u32, mut visit: impl FnMut(usize, u32)) {
    let width_usize = width as usize;
    let x = (index % width_usize) as i32;
    let z = (index / width_usize) as i32;

    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }

            let nx = x + dx;
            let nz = z + dz;
            if nx < 0 || nz < 0 || nx >= width as i32 || nz >= height as i32 {
                continue;
            }

            let neighbor = nz as usize * width_usize + nx as usize;
            let step_cost = if dx != 0 && dz != 0 { 1414 } else { 1000 };
            visit(neighbor, step_cost);
        }
    }
}

fn for_each_cardinal_neighbor(index: usize, width: u32, height: u32, mut visit: impl FnMut(usize)) {
    let width_usize = width as usize;
    let x = (index % width_usize) as i32;
    let z = (index / width_usize) as i32;

    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let nx = x + dx;
        let nz = z + dz;
        if nx < 0 || nz < 0 || nx >= width as i32 || nz >= height as i32 {
            continue;
        }

        let neighbor = nz as usize * width_usize + nx as usize;
        visit(neighbor);
    }
}

fn max5(values: [f32; 5]) -> f32 {
    values.into_iter().fold(0.0_f32, f32::max)
}

fn second_highest(values: [f32; 5]) -> f32 {
    let mut highest = 0.0_f32;
    let mut second = 0.0_f32;

    for value in values {
        if value >= highest {
            second = highest;
            highest = value;
        } else if value > second {
            second = value;
        }
    }

    second
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let delta = edge1 - edge0;
    if delta.abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = clamp01((value - edge0) / delta);
    t * t * (3.0 - 2.0 * t)
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(fields: &AtlasFieldMap) -> u64 {
        let mut hash = 0_u64;
        for cell in fields.cells().values() {
            for bits in [
                cell.landness.to_bits(),
                cell.macro_elevation.to_bits(),
                cell.river_flow_potential.to_bits(),
                cell.temperature.to_bits(),
                cell.humidity.to_bits(),
                cell.ecotone_strength.to_bits(),
            ] {
                hash = hash.rotate_left(7) ^ u64::from(bits);
            }
        }
        hash
    }

    #[test]
    fn atlas_generation_is_deterministic() {
        let meta = WorldMeta::new(77);
        let area = AtlasArea::new(AtlasCoord::new(-8, -8), 16, 16).unwrap();
        let a = generate_atlas_fields(&meta, area);
        let b = generate_atlas_fields(&meta, area);

        assert_eq!(signature(&a), signature(&b));
    }

    #[test]
    fn atlas_fields_stay_normalized() {
        let meta = WorldMeta::new(9);
        let area = AtlasArea::new(AtlasCoord::new(-6, -6), 12, 12).unwrap();
        let atlas = generate_atlas_fields(&meta, area);

        for cell in atlas.cells().values() {
            for value in [
                cell.landness,
                cell.ocean_distance,
                cell.coast_distance,
                cell.continent_core_factor,
                cell.macro_elevation,
                cell.slope,
                cell.ruggedness,
                cell.ridge_factor,
                cell.mountain_mass,
                cell.basinness,
                cell.pass_potential,
                cell.river_source_potential,
                cell.river_flow_potential,
                cell.river_distance_estimate,
                cell.lake_potential,
                cell.temperature,
                cell.humidity,
                cell.inlandness,
                cell.aridity,
                cell.wetness,
                cell.polar_factor,
                cell.alpine_factor,
                cell.coast_factor,
                cell.wetland_factor,
                cell.riverine_factor,
                cell.ecotone_strength,
            ] {
                assert!(!value.is_nan());
                assert!((0.0..=1.0).contains(&value));
            }
        }
    }

    #[test]
    fn reference_seed_area_contains_both_land_and_ocean() {
        let meta = WorldMeta::new(42);
        let area = AtlasArea::new(AtlasCoord::new(-32, -32), 64, 64).unwrap();
        let atlas = generate_atlas_fields(&meta, area);

        let land_cells = atlas
            .cells()
            .values()
            .iter()
            .filter(|cell| cell.landness >= LAND_THRESHOLD)
            .count();

        assert!(land_cells > 0, "reference preview should contain some land");
        assert!(
            land_cells < atlas.cells().values().len(),
            "reference preview should contain some ocean"
        );
    }
}
