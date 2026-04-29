use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::world::WorldMeta;

use super::scale::{AtlasArea, AtlasCoord, AtlasGrid};
use super::seed::{
    SALT_COAST_ROUGHNESS, SALT_CONTINENT_PRIMARY, SALT_CONTINENT_SECONDARY, SALT_DETAIL,
    SALT_HUMIDITY, SALT_MOUNTAIN_CLUSTER, SALT_RIDGE_PRIMARY, SALT_RIDGE_SECONDARY,
    SALT_TEMPERATURE, domain_warp, fbm, ridged_fbm,
};
use super::tuning::AtlasTuning;

const NO_SOURCE_DISTANCE: u32 = 1_000_000_000;

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

    pub(crate) fn from_cells(area: AtlasArea, values: Vec<AtlasCell>) -> Self {
        Self {
            cells: AtlasGrid::from_values(area, values),
        }
    }
}

pub fn generate_atlas_fields(meta: &WorldMeta, area: AtlasArea) -> AtlasFieldMap {
    generate_atlas_fields_with_tuning(meta, area, &AtlasTuning::default())
}

pub fn generate_atlas_fields_with_tuning(
    meta: &WorldMeta,
    area: AtlasArea,
    tuning: &AtlasTuning,
) -> AtlasFieldMap {
    let width = area.width();
    let height = area.height();
    let len = area.len();
    let normalization = tuning.normalization;
    let terrain = tuning.terrain;
    let climate = tuning.climate;
    let hydrology = tuning.hydrology;

    let mut cells = vec![AtlasCell::default(); len];
    let mut land_mask = vec![false; len];
    let mut temperature_noise = vec![0.0_f32; len];
    let mut humidity_noise = vec![0.0_f32; len];
    let mut height_field = vec![0.0_f32; len];

    for coord in area.coords() {
        let index = area
            .index_of(coord)
            .expect("atlas coord must index into area");
        let landness = sample_landness(meta.seed, coord, tuning);
        let is_land = landness >= normalization.land_threshold;

        cells[index].landness = landness;
        cells[index].ridge_factor = sample_ridge_factor(meta.seed, coord, tuning)
            * smoothstep(
                normalization.ridge_landness_min,
                normalization.ridge_landness_max,
                landness,
            );
        land_mask[index] = is_land;
        temperature_noise[index] = fbm(
            meta.seed,
            coord.x as f64 / climate.temperature_field_scale,
            coord.z as f64 / climate.temperature_field_scale,
            climate.temperature_octaves,
            climate.temperature_lacunarity,
            climate.temperature_gain,
            SALT_TEMPERATURE,
        );
        humidity_noise[index] = fbm(
            meta.seed,
            coord.x as f64 / climate.humidity_field_scale,
            coord.z as f64 / climate.humidity_field_scale,
            climate.humidity_octaves,
            climate.humidity_lacunarity,
            climate.humidity_gain,
            SALT_HUMIDITY,
        );
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
        let coast_distance_cells =
            coast_distance_cells(&land_mask, &ocean_distance_raw, &land_distance_raw, index);
        let continent_core = if is_land {
            clamp01(ocean_distance_cells / normalization.continent_core_normalizer)
        } else {
            0.0
        };
        let coast_factor = if is_land {
            1.0 - clamp01(coast_distance_cells / normalization.coast_factor_distance)
        } else {
            0.0
        };
        let mountain_mass = sample_mountain_mass(
            meta.seed,
            coord,
            cells[index].ridge_factor,
            continent_core,
            cells[index].landness,
            tuning,
        );
        let continental_lift = smoothstep(normalization.land_threshold, 1.0, cells[index].landness);
        let macro_elevation = if is_land {
            clamp01(
                terrain.macro_base_height
                    + continental_lift * terrain.macro_landness_weight
                    + continent_core * terrain.macro_continent_core_weight
                    + mountain_mass * terrain.macro_mountain_weight
                    - coast_factor * terrain.macro_coast_penalty,
            )
        } else {
            clamp01(cells[index].landness * terrain.macro_ocean_floor_scale)
        };

        cells[index].continent_id = continent_ids[index];
        cells[index].ocean_distance =
            clamp01(ocean_distance_cells / normalization.ocean_distance_normalizer);
        cells[index].coast_distance =
            clamp01(coast_distance_cells / normalization.coast_distance_normalizer);
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
        let detail = fbm(
            meta.seed,
            coord.x as f64 / terrain.detail_scale,
            coord.z as f64 / terrain.detail_scale,
            terrain.detail_octaves,
            terrain.detail_lacunarity,
            terrain.detail_gain,
            SALT_DETAIL,
        );

        cells[index].slope = clamp01(max_diff * terrain.slope_scale);
        cells[index].ruggedness = clamp01(
            cells[index].ridge_factor * terrain.rugged_ridge_weight
                + cells[index].slope * terrain.rugged_slope_weight
                + detail * terrain.rugged_detail_weight,
        );
        cells[index].basinness = clamp01(
            (neighbor_mean - height_field[index] + terrain.basin_offset) * terrain.basin_scale,
        );
        cells[index].pass_potential = clamp01(
            cells[index].mountain_mass
                * (1.0 - cells[index].ridge_factor * terrain.pass_ridge_penalty)
                * (1.0 - cells[index].slope * terrain.pass_slope_penalty)
                + mean_diff * terrain.pass_mean_diff_weight,
        );
    }

    for index in 0..len {
        let coord = area.coord_at(index).expect("index should map to coord");
        let equator_heat = 1.0
            - ((coord.z as f32) / climate.equator_falloff_scale)
                .abs()
                .tanh();

        cells[index].inlandness = cells[index].ocean_distance;
        cells[index].temperature = clamp01(
            equator_heat * climate.temperature_equator_weight
                + temperature_noise[index] * climate.temperature_noise_weight
                - cells[index].macro_elevation * climate.temperature_elevation_cooling
                - cells[index].mountain_mass * climate.temperature_mountain_cooling,
        );
        cells[index].polar_factor = smoothstep(
            climate.polar_edge_warm,
            climate.polar_edge_cold,
            cells[index].temperature,
        );
        cells[index].humidity = clamp01(
            humidity_noise[index] * climate.humidity_noise_weight
                + (1.0 - cells[index].ocean_distance) * climate.humidity_ocean_bonus
                + cells[index].basinness * climate.humidity_basin_bonus
                - cells[index].inlandness * climate.humidity_inland_penalty
                - cells[index].mountain_mass
                    * cells[index].inlandness
                    * climate.humidity_rain_shadow_penalty,
        );
        cells[index].river_source_potential = clamp01(
            cells[index].mountain_mass * hydrology.river_source_mountain_weight
                + cells[index].humidity * hydrology.river_source_humidity_weight
                + cells[index].slope * hydrology.river_source_slope_weight,
        );
    }

    let downhill = compute_downhill_targets(area, &land_mask, &height_field);
    let mut flow_accumulation = vec![0.0_f32; len];
    for index in 0..len {
        if !land_mask[index] {
            continue;
        }

        flow_accumulation[index] = clamp01(
            hydrology.flow_base
                + cells[index].humidity * hydrology.flow_humidity_weight
                + cells[index].mountain_mass * hydrology.flow_mountain_weight
                + (1.0 - cells[index].temperature) * hydrology.flow_cold_weight,
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
            cells[index].basinness * hydrology.lake_basin_weight
                + flow * hydrology.lake_flow_weight
                + if downhill[index].is_none() {
                    hydrology.lake_sink_bonus
                } else {
                    0.0
                }
                + (1.0 - cells[index].slope) * hydrology.lake_flat_bonus,
        );

        river_mask[index] = flow > hydrology.river_threshold
            && (cells[index].river_source_potential > hydrology.river_source_threshold
                || flow > hydrology.river_override_threshold)
            && cells[index].macro_elevation > hydrology.river_min_macro_elevation;
        lake_mask[index] = cells[index].lake_potential > hydrology.lake_threshold
            && cells[index].macro_elevation > hydrology.lake_min_macro_elevation
            && cells[index].slope < hydrology.lake_max_slope;
    }

    let mut waterline_mask = vec![false; len];
    for index in 0..len {
        waterline_mask[index] = river_mask[index] || lake_mask[index];
    }
    let river_distance_raw = compute_distance_to_value(&waterline_mask, width, height, true);

    for index in 0..len {
        let river_distance_cells = river_distance_raw[index] as f32 / 1000.0;
        cells[index].river_distance_estimate =
            clamp01(river_distance_cells / normalization.river_distance_normalizer);
        cells[index].riverine_factor = if land_mask[index] {
            clamp01(
                (1.0 - cells[index].river_distance_estimate) * hydrology.riverine_distance_weight
                    + cells[index].river_flow_potential * hydrology.riverine_flow_weight,
            )
        } else {
            0.0
        };
        cells[index].humidity = clamp01(
            cells[index].humidity
                + cells[index].riverine_factor * climate.humidity_river_bonus
                + cells[index].lake_potential * climate.humidity_lake_bonus,
        );
        cells[index].wetness = clamp01(
            cells[index].humidity * hydrology.wetness_humidity_weight
                + cells[index].riverine_factor * hydrology.wetness_river_weight
                + cells[index].lake_potential * hydrology.wetness_lake_weight
                + cells[index].basinness * hydrology.wetness_basin_weight,
        );
        cells[index].aridity = clamp01(
            (1.0 - cells[index].humidity) * hydrology.aridity_dryness_weight
                + cells[index].inlandness * hydrology.aridity_inland_weight
                + (1.0 - cells[index].wetness) * hydrology.aridity_low_wetness_weight
                - cells[index].riverine_factor * hydrology.aridity_river_relief,
        );
        cells[index].alpine_factor = clamp01(
            smoothstep(
                hydrology.alpine_elevation_min,
                hydrology.alpine_elevation_max,
                cells[index].macro_elevation,
            ) * hydrology.alpine_elevation_weight
                + smoothstep(
                    hydrology.alpine_mountain_min,
                    hydrology.alpine_mountain_max,
                    cells[index].mountain_mass,
                ) * hydrology.alpine_mountain_weight
                - cells[index].polar_factor * hydrology.alpine_polar_penalty,
        );
        cells[index].wetland_factor = clamp01(
            cells[index].wetness * hydrology.wetland_wetness_weight
                + cells[index].basinness * hydrology.wetland_basin_weight
                + cells[index].riverine_factor * hydrology.wetland_river_weight
                - cells[index].slope * hydrology.wetland_slope_penalty,
        );
        cells[index].overlay = overlay_weights(land_mask[index], &cells[index]);
        cells[index].thermal =
            thermal_weights(cells[index].temperature, cells[index].polar_factor, tuning);
        cells[index].moisture = moisture_weights(
            cells[index].humidity,
            cells[index].aridity,
            cells[index].wetness,
            tuning,
        );
        cells[index].form = form_weights(
            cells[index].macro_elevation,
            cells[index].ruggedness,
            cells[index].mountain_mass,
            tuning,
        );
        cells[index].cover = cover_potentials(land_mask[index], &cells[index], tuning);
        cells[index].ecotone_strength = ecotone_strength(&cells[index], tuning);
    }

    AtlasFieldMap {
        cells: AtlasGrid::from_values(area, cells),
    }
}

fn sample_landness(seed: u64, coord: AtlasCoord, tuning: &AtlasTuning) -> f32 {
    let continent = tuning.continent;
    let x = coord.x as f64;
    let z = coord.z as f64;
    let (wx, wz) = domain_warp(seed, x, z, continent.warp_scale, continent.warp_amplitude);
    let primary = fbm(
        seed,
        wx / continent.primary_scale,
        wz / continent.primary_scale,
        continent.primary_octaves,
        continent.primary_lacunarity,
        continent.primary_gain,
        SALT_CONTINENT_PRIMARY,
    );
    let secondary = fbm(
        seed,
        wx / continent.secondary_scale,
        wz / continent.secondary_scale,
        continent.secondary_octaves,
        continent.secondary_lacunarity,
        continent.secondary_gain,
        SALT_CONTINENT_SECONDARY,
    );
    let islands = fbm(
        seed,
        x / continent.island_scale,
        z / continent.island_scale,
        continent.island_octaves,
        continent.island_lacunarity,
        continent.island_gain,
        SALT_DETAIL,
    );
    let coast = ridged_fbm(
        seed,
        wx / continent.coast_scale,
        wz / continent.coast_scale,
        continent.coast_octaves,
        continent.coast_lacunarity,
        continent.coast_gain,
        SALT_COAST_ROUGHNESS,
    );

    clamp01(
        primary * continent.primary_weight
            + secondary * continent.secondary_weight
            + islands * continent.island_weight
            - coast * continent.coast_penalty
            + continent.bias,
    )
}

fn sample_ridge_factor(seed: u64, coord: AtlasCoord, tuning: &AtlasTuning) -> f32 {
    let ridge = tuning.ridge;
    let x = coord.x as f64;
    let z = coord.z as f64;
    let (wx, wz) = domain_warp(seed, x, z, ridge.warp_scale, ridge.warp_amplitude);
    let primary = ridged_fbm(
        seed,
        wx / ridge.primary_scale,
        wz / ridge.primary_scale,
        ridge.primary_octaves,
        ridge.primary_lacunarity,
        ridge.primary_gain,
        SALT_RIDGE_PRIMARY,
    );
    let secondary = ridged_fbm(
        seed,
        wx / ridge.secondary_scale,
        wz / ridge.secondary_scale,
        ridge.secondary_octaves,
        ridge.secondary_lacunarity,
        ridge.secondary_gain,
        SALT_RIDGE_SECONDARY,
    );

    clamp01(primary * ridge.primary_weight + secondary * ridge.secondary_weight)
}

fn sample_mountain_mass(
    seed: u64,
    coord: AtlasCoord,
    ridge_factor: f32,
    continent_core: f32,
    landness: f32,
    tuning: &AtlasTuning,
) -> f32 {
    let terrain = tuning.terrain;
    let cluster = fbm(
        seed,
        coord.x as f64 / terrain.mountain_cluster_scale,
        coord.z as f64 / terrain.mountain_cluster_scale,
        terrain.mountain_cluster_octaves,
        terrain.mountain_cluster_lacunarity,
        terrain.mountain_cluster_gain,
        SALT_MOUNTAIN_CLUSTER,
    );
    let base =
        ridge_factor * terrain.mountain_ridge_weight + cluster * terrain.mountain_cluster_weight;

    smoothstep(terrain.mountain_base_min, terrain.mountain_base_max, base)
        * smoothstep(
            terrain.mountain_continent_min,
            terrain.mountain_continent_max,
            continent_core + landness * terrain.mountain_landness_bias,
        )
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

fn thermal_weights(temperature: f32, polar_factor: f32, tuning: &AtlasTuning) -> ThermalWeights {
    let weights = tuning.weights;
    let mut values =
        triangular_weights(temperature, weights.thermal_centers, weights.thermal_widths);
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

fn moisture_weights(
    humidity: f32,
    aridity: f32,
    wetness: f32,
    tuning: &AtlasTuning,
) -> MoistureWeights {
    let weights = tuning.weights;
    let moisture_signal = clamp01(
        humidity * weights.moisture_signal_humidity_weight
            + wetness * weights.moisture_signal_wetness_weight
            - aridity * weights.moisture_signal_aridity_penalty
            + weights.moisture_signal_bias,
    );
    let mut values = triangular_weights(
        moisture_signal,
        weights.moisture_centers,
        weights.moisture_widths,
    );
    values[0] = values[0].max(aridity * weights.moisture_arid_boost);
    values[4] = values[4].max(wetness * weights.moisture_wet_boost);
    normalize_weights(&mut values);

    MoistureWeights {
        arid: values[0],
        semi_arid: values[1],
        subhumid: values[2],
        humid: values[3],
        wet: values[4],
    }
}

fn form_weights(
    macro_elevation: f32,
    ruggedness: f32,
    mountain_mass: f32,
    tuning: &AtlasTuning,
) -> TerrainFormWeights {
    let weights = tuning.weights;
    let mountain = clamp01(smoothstep(
        weights.form_mountain_min,
        weights.form_mountain_max,
        mountain_mass * weights.form_mountain_mass_weight
            + ruggedness * weights.form_mountain_ruggedness_weight,
    ));
    let hill = clamp01(
        smoothstep(
            weights.form_hill_min,
            weights.form_hill_max,
            ruggedness + macro_elevation * weights.form_hill_macro_elevation_weight,
        ) * (1.0 - mountain * weights.form_hill_mountain_suppression),
    );
    let mut values = [
        clamp01(
            1.0 - mountain * weights.form_plain_mountain_penalty
                - hill * weights.form_plain_hill_penalty,
        ),
        hill,
        mountain,
    ];
    normalize_weights(&mut values);

    TerrainFormWeights {
        plain: values[0],
        hill: values[1],
        mountain: values[2],
    }
}

fn cover_potentials(is_land: bool, cell: &AtlasCell, tuning: &AtlasTuning) -> CoverPotentials {
    if !is_land {
        return CoverPotentials::default();
    }

    let weights = tuning.weights;
    let canopy = clamp01(
        cell.humidity * weights.cover_canopy_humidity_weight
            + cell.wetness * weights.cover_canopy_wetness_weight
            + (cell.thermal.temperate + cell.thermal.warm * weights.cover_canopy_warm_bonus_weight)
                * weights.cover_canopy_temperate_weight
            - cell.aridity * weights.cover_canopy_aridity_penalty
            - cell.overlay.alpine * weights.cover_canopy_alpine_penalty,
    );
    let forest = clamp01(
        canopy * weights.cover_forest_canopy_weight
            + cell.overlay.riverine * weights.cover_forest_river_weight
            + cell.moisture.humid * weights.cover_forest_humid_weight,
    );
    let openness = clamp01(
        weights.cover_openness_base
            + cell.aridity * weights.cover_openness_aridity_weight
            + cell.form.plain * weights.cover_openness_plain_weight
            - canopy * weights.cover_openness_canopy_penalty
            - cell.moisture.wet * weights.cover_openness_wet_penalty,
    );
    let grass = clamp01(
        openness * weights.cover_grass_openness_weight
            + cell.moisture.subhumid * weights.cover_grass_subhumid_weight
            + cell.moisture.humid * weights.cover_grass_humid_weight
            + cell.thermal.temperate * weights.cover_grass_temperate_weight,
    );
    let shrub = clamp01(
        cell.aridity * weights.cover_shrub_aridity_weight
            + cell.moisture.semi_arid * weights.cover_shrub_semi_arid_weight
            + cell.form.hill * weights.cover_shrub_hill_weight
            + weights.cover_shrub_bias,
    );

    CoverPotentials {
        openness,
        grass_potential: grass,
        shrub_potential: shrub,
        canopy_potential: canopy,
        forest_potential: forest,
    }
}

fn ecotone_strength(cell: &AtlasCell, tuning: &AtlasTuning) -> f32 {
    let weights = tuning.weights;
    let thermal_competition = 1.0
        - max5([
            cell.thermal.polar,
            cell.thermal.cold,
            cell.thermal.temperate,
            cell.thermal.warm,
            cell.thermal.hot,
        ]);
    let moisture_competition = 1.0
        - max5([
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

    clamp01(
        thermal_competition * weights.ecotone_thermal_weight
            + moisture_competition * weights.ecotone_moisture_weight
            + overlay_competition * weights.ecotone_overlay_weight,
    )
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

fn compute_distance_to_value(
    mask: &[bool],
    width: u32,
    height: u32,
    source_value: bool,
) -> Vec<u32> {
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
        return vec![NO_SOURCE_DISTANCE; len];
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

fn compute_downhill_targets(
    area: AtlasArea,
    land_mask: &[bool],
    field: &[f32],
) -> Vec<Option<usize>> {
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

fn coast_distance_cells(
    land_mask: &[bool],
    ocean_distance_raw: &[u32],
    land_distance_raw: &[u32],
    index: usize,
) -> f32 {
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
        let tuning = AtlasTuning::default();

        let land_cells = atlas
            .cells()
            .values()
            .iter()
            .filter(|cell| cell.landness >= tuning.normalization.land_threshold)
            .count();

        assert!(land_cells > 0, "reference preview should contain some land");
        assert!(
            land_cells < atlas.cells().values().len(),
            "reference preview should contain some ocean"
        );
    }

    #[test]
    fn distance_without_source_stays_far_instead_of_collapsing_to_zero() {
        let distances = compute_distance_to_value(&[true, true, true, true], 2, 2, false);

        assert!(
            distances
                .iter()
                .all(|distance| *distance == NO_SOURCE_DISTANCE)
        );
    }

    #[test]
    fn reference_seed_area_contains_some_mountain_signal() {
        let meta = WorldMeta::new(42);
        let area = AtlasArea::new(AtlasCoord::new(-64, -64), 128, 128).unwrap();
        let atlas = generate_atlas_fields(&meta, area);

        let strongest = atlas
            .cells()
            .values()
            .iter()
            .map(|cell| cell.mountain_mass)
            .fold(0.0_f32, f32::max);

        assert!(
            strongest > 0.25,
            "reference seed should contain some mountain-bearing atlas cells"
        );
    }
}
