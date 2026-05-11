use crate::renderer::RenderEnvironment;
use crate::simulation::{
    EcologySimBundleInput, EcologySimChunkInput, SimInputBundle, SimRegion, SimTick,
    SimulationResult, TimeSimBundleInput, TimeSimCellInput, WeatherSimBundleInput,
    WeatherSimChunkInput,
};
use crate::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasClimateRuntimeState, AtlasCoord, BiomeFamily,
    CHUNK_EDGE_I32, ChunkCoord, ChunkWeatherState, CoastalContext, ElevationBand, HydrologyContext,
    LocalWeatherState, MoistureBand, RegionClassSample, TemperatureBand, WorldCalendar, WorldCore,
    atlas_coord_for_chunk,
    generation::{GraphBiomeContext, GraphBiomeKind, GraphBiomeWaterRole},
};

use super::GameApp;

impl GameApp {
    pub(crate) fn run_fixed_updates(&mut self) {
        let fixed_dt = self.config.timing.fixed_step_interval();
        self.timing.fixed_accumulator = self
            .timing
            .fixed_accumulator
            .saturating_add(self.timing.frame_dt);

        let max_steps = self.config.timing.max_fixed_steps_per_frame.max(1);
        let mut executed_steps = 0;
        while self.timing.fixed_accumulator >= fixed_dt && executed_steps < max_steps {
            self.ecs.run_fixed_update();
            let active_region = self.ecs.active_sim_region();
            let active_chunk_scope = self.ecs.active_chunk_observer_scope();
            let tick = SimTick {
                index: self.ecs.sim_clock().tick_index,
                delta: fixed_dt,
            };
            let sim_region = SimRegion {
                center_atlas: active_region.center_atlas,
                atlas_area: active_region.area,
            };
            let input = SimInputBundle {
                ecology: Some(self.build_ecology_sim_input_bundle(&active_chunk_scope.chunks)),
                time: Some(self.build_time_sim_input_bundle(
                    active_region.center_atlas,
                    active_region.area,
                    tick,
                )),
                weather: Some(
                    self.build_weather_sim_input_bundle(&active_chunk_scope.chunks, tick),
                ),
            };
            let results = self.simulation.step_all(tick, sim_region, input);
            self.ecs.enqueue_simulation_results(results);
            for result in self.ecs.drain_pending_simulation_results() {
                self.apply_simulation_result(&result);
            }

            self.timing.fixed_accumulator = self.timing.fixed_accumulator.saturating_sub(fixed_dt);
            executed_steps += 1;
        }

        if executed_steps >= max_steps && self.timing.fixed_accumulator > fixed_dt {
            self.timing.fixed_accumulator = fixed_dt;
        }
    }

    pub(crate) fn sync_renderer_environment_from_world(&mut self) {
        let focus_atlas = self
            .ecs
            .local_player_transform()
            .map(|transform| atlas_coord_for_translation(transform.translation))
            .unwrap_or_else(|| self.ecs.active_sim_region().center_atlas);
        let region = self
            .world
            .sample_cached_region_class_atlas(focus_atlas)
            .unwrap_or_else(RegionClassSample::default);
        let climate = self.world.climate_state(focus_atlas);
        let weather = self.world.local_weather(focus_atlas).unwrap_or_else(|| {
            LocalWeatherState::clear(
                focus_atlas,
                region.climate_regime,
                self.world.calendar().absolute_tick,
                self.world.calendar().absolute_tick,
            )
        });
        self.renderer.set_environment(render_environment_from_world(
            *self.world.calendar(),
            climate,
            weather,
        ));
    }

    pub(crate) fn queue_environment_region_resolve_for_focus(&mut self) {
        let focus_atlas = self
            .ecs
            .local_player_transform()
            .map(|transform| atlas_coord_for_translation(transform.translation))
            .unwrap_or_else(|| self.ecs.active_sim_region().center_atlas);
        let area =
            AtlasArea::new(focus_atlas, 1, 1).expect("single focus atlas area must be valid");
        if self.world.cached_region_class_area(area).is_some() {
            return;
        }

        if let Err(error) = self
            .jobs
            .submit(crate::jobs::JobRequest::ResolveRegionClassArea {
                meta: *self.world.meta(),
                area,
            })
        {
            eprintln!(
                "[app] failed to queue region class resolve for atlas ({}, {}): {:?}",
                focus_atlas.x, focus_atlas.z, error
            );
        }
    }

    fn build_ecology_sim_input_bundle(
        &self,
        active_chunks: &[ChunkCoord],
    ) -> EcologySimBundleInput {
        EcologySimBundleInput {
            world_seed: self.world.meta().seed,
            chunks: ecology_chunk_inputs_from_world(&self.world, active_chunks),
        }
    }

    fn build_time_sim_input_bundle(
        &self,
        center_atlas: AtlasCoord,
        area: crate::world::AtlasArea,
        tick: SimTick,
    ) -> TimeSimBundleInput {
        let ticks_per_game_minute =
            u64::from(self.simulation.config().time.ticks_per_game_minute.max(1));
        if tick.index % ticks_per_game_minute != 0 {
            return TimeSimBundleInput {
                world_seed: self.world.meta().seed,
                calendar: *self.world.calendar(),
                cells: Vec::new(),
            };
        }

        let Some(classes) = self.world.cached_region_class_area(area) else {
            return TimeSimBundleInput {
                world_seed: self.world.meta().seed,
                calendar: *self.world.calendar(),
                cells: Vec::new(),
            };
        };
        let weather_window_end = tick.index.saturating_add(ticks_per_game_minute);
        let mut cells = Vec::with_capacity(area.len());
        for coord in area.coords() {
            let region = classes
                .get(coord)
                .copied()
                .unwrap_or_else(|| self.world.sample_region_class_atlas(center_atlas));
            let climate_state = self.world.climate_state(coord);
            let current_weather = self.world.local_weather(coord).unwrap_or_else(|| {
                LocalWeatherState::clear(
                    coord,
                    region.climate_regime,
                    tick.index,
                    weather_window_end,
                )
            });
            cells.push(TimeSimCellInput {
                coord,
                region,
                climate_state,
                current_weather,
            });
        }

        TimeSimBundleInput {
            world_seed: self.world.meta().seed,
            calendar: *self.world.calendar(),
            cells,
        }
    }

    fn build_weather_sim_input_bundle(
        &self,
        active_chunks: &[ChunkCoord],
        tick: SimTick,
    ) -> WeatherSimBundleInput {
        let ticks_per_game_hour =
            u64::from(self.simulation.config().weather.ticks_per_game_hour.max(1));
        if tick.index % ticks_per_game_hour != 0 {
            return WeatherSimBundleInput {
                world_seed: self.world.meta().seed,
                calendar: *self.world.calendar(),
                chunks: Vec::new(),
            };
        }

        WeatherSimBundleInput {
            world_seed: self.world.meta().seed,
            calendar: *self.world.calendar(),
            chunks: weather_chunk_inputs_from_world(&self.world, active_chunks, tick.index),
        }
    }

    fn apply_simulation_result(&mut self, result: &SimulationResult) {
        if let Some(advance) = result.calendar_advance.clone() {
            self.world.apply_calendar_advance(advance);
        }

        for update in result.chunk_weather_updates.iter().copied() {
            self.world.apply_chunk_weather_update(update);
        }

        for edit in result.world_edits.iter().cloned() {
            let _ = self.world.apply_edit(edit);
        }

        self.sync_renderer_environment_from_world();
    }
}

fn ecology_chunk_inputs_from_world(
    world: &WorldCore,
    active_chunks: &[ChunkCoord],
) -> Vec<EcologySimChunkInput> {
    active_chunks
        .iter()
        .copied()
        .map(|coord| {
            let observation = world.observe_chunk_surface_condition(coord);
            EcologySimChunkInput {
                coord,
                biome: graph_biome_for_runtime_compat(observation.cell_biome),
            }
        })
        .collect()
}

fn weather_chunk_inputs_from_world(
    world: &WorldCore,
    active_chunks: &[ChunkCoord],
    tick_index: u64,
) -> Vec<WeatherSimChunkInput> {
    active_chunks
        .iter()
        .copied()
        .map(|coord| {
            let observation = world.observe_chunk_surface_condition(coord);
            let atlas = atlas_coord_for_chunk(coord);
            let region = world
                .sample_cached_region_class_atlas(atlas)
                .unwrap_or_else(|| world.sample_region_class_atlas(atlas));
            WeatherSimChunkInput {
                coord,
                biome: graph_biome_for_runtime_compat(observation.cell_biome),
                context: graph_biome_context_for_runtime_compat(region),
                previous_weather: world
                    .chunk_weather(coord)
                    .unwrap_or_else(|| ChunkWeatherState::clear(tick_index)),
                neighbor_weather: chunk_weather_neighbors(world, coord),
            }
        })
        .collect()
}

fn chunk_weather_neighbors(world: &WorldCore, coord: ChunkCoord) -> Vec<ChunkWeatherState> {
    [
        coord.offset(-1, 0, 0),
        coord.offset(1, 0, 0),
        coord.offset(0, 0, -1),
        coord.offset(0, 0, 1),
    ]
    .into_iter()
    .filter_map(|neighbor| world.chunk_weather(neighbor))
    .collect()
}

fn graph_biome_for_runtime_compat(biome: BiomeFamily) -> GraphBiomeKind {
    match biome {
        BiomeFamily::Oceanic => GraphBiomeKind::ShallowOcean,
        BiomeFamily::Mangrove => GraphBiomeKind::Mangrove,
        BiomeFamily::EstuarineCoast => GraphBiomeKind::EstuarineCoast,
        BiomeFamily::LagoonCoast => GraphBiomeKind::LagoonCoast,
        BiomeFamily::RockyCoast => GraphBiomeKind::RockyCoast,
        BiomeFamily::SandyCoast => GraphBiomeKind::SandyCoast,
        BiomeFamily::Marsh => GraphBiomeKind::Marsh,
        BiomeFamily::Swamp => GraphBiomeKind::Swamp,
        BiomeFamily::FloodedForest => GraphBiomeKind::FloodedForest,
        BiomeFamily::Desert => GraphBiomeKind::Desert,
        BiomeFamily::SemiDesert => GraphBiomeKind::SemiDesert,
        BiomeFamily::Steppe => GraphBiomeKind::Steppe,
        BiomeFamily::DryShrubland => GraphBiomeKind::DryShrubland,
        BiomeFamily::MediterraneanShrubland => GraphBiomeKind::MediterraneanShrubland,
        BiomeFamily::PolarIce => GraphBiomeKind::PolarIce,
        BiomeFamily::PolarBarrens => GraphBiomeKind::PolarBarrens,
        BiomeFamily::Tundra => GraphBiomeKind::Tundra,
        BiomeFamily::SubalpineWoodland => GraphBiomeKind::SubalpineWoodland,
        BiomeFamily::AlpineMeadow => GraphBiomeKind::AlpineMeadow,
        BiomeFamily::BorealForest => GraphBiomeKind::BorealForest,
        BiomeFamily::TropicalRainforest => GraphBiomeKind::TropicalRainforest,
        BiomeFamily::MonsoonForest => GraphBiomeKind::MonsoonForest,
        BiomeFamily::TropicalDryForest => GraphBiomeKind::TropicalDryForest,
        BiomeFamily::Savanna => GraphBiomeKind::Savanna,
        BiomeFamily::TemperateRainforest => GraphBiomeKind::TemperateRainforest,
        BiomeFamily::TemperateMixedForest => GraphBiomeKind::TemperateMixedForest,
        BiomeFamily::TemperateBroadleafForest => GraphBiomeKind::TemperateBroadleafForest,
        BiomeFamily::TemperateGrassland => GraphBiomeKind::TemperateGrassland,
    }
}

fn graph_biome_context_for_runtime_compat(region: RegionClassSample) -> GraphBiomeContext {
    GraphBiomeContext {
        temperature: temperature_band_signal(region.temperature_band),
        hydration: moisture_band_signal(region.moisture_band),
        elevation: elevation_band_signal(region.elevation_band),
        continentality: match region.coastal_context {
            CoastalContext::Marine | CoastalContext::Coastal => -0.10,
            CoastalContext::NearCoast => 0.05,
            CoastalContext::Inland => 0.30,
        },
        coastness: match region.coastal_context {
            CoastalContext::Marine => 1.0,
            CoastalContext::Coastal => 0.75,
            CoastalContext::NearCoast => 0.35,
            CoastalContext::Inland => 0.0,
        },
        mountainness: if matches!(region.elevation_band, ElevationBand::Alpine) {
            1.0
        } else {
            0.0
        },
        ruggedness: 0.0,
        water_role: match region.hydrology_context {
            HydrologyContext::LakeBasin => GraphBiomeWaterRole::Lake,
            HydrologyContext::WetLowland => GraphBiomeWaterRole::Wetland,
            HydrologyContext::Dryland => GraphBiomeWaterRole::DryBasin,
            _ => GraphBiomeWaterRole::Land,
        },
    }
}

fn temperature_band_signal(band: TemperatureBand) -> f32 {
    match band {
        TemperatureBand::Polar => 0.08,
        TemperatureBand::Cold => 0.26,
        TemperatureBand::Temperate => 0.50,
        TemperatureBand::Warm => 0.68,
        TemperatureBand::Hot => 0.86,
    }
}

fn moisture_band_signal(band: MoistureBand) -> f32 {
    match band {
        MoistureBand::Arid => 0.10,
        MoistureBand::SemiArid => 0.24,
        MoistureBand::Subhumid => 0.46,
        MoistureBand::Humid => 0.66,
        MoistureBand::Wet => 0.84,
    }
}

fn elevation_band_signal(band: ElevationBand) -> f32 {
    match band {
        ElevationBand::Low => 0.08,
        ElevationBand::Upland => 0.32,
        ElevationBand::Highland => 0.54,
        ElevationBand::Alpine => 0.78,
    }
}

fn render_environment_from_world(
    calendar: WorldCalendar,
    climate: AtlasClimateRuntimeState,
    weather: LocalWeatherState,
) -> RenderEnvironment {
    let hours = calendar.time_of_day_hours();
    let day_angle = ((hours - 6.0) / 24.0) * std::f32::consts::TAU;
    let solar = day_angle.sin().clamp(-1.0, 1.0);
    let daylight = ((solar + 0.12) / 1.12).clamp(0.0, 1.0);
    let twilight = (1.0 - ((hours - 18.0).abs() / 6.0)).clamp(0.0, 1.0);
    let overcast = weather.overcast_factor();
    let wetness = weather.wetness_factor();
    let weather_strength = weather.weather_strength();
    let temperature_bias = climate.temperature_offset.clamp(-1.0, 1.0);
    let humidity = (0.46 + climate.humidity_offset + overcast * 0.18).clamp(0.0, 1.0);

    let sky_day = [0.42, 0.69, 0.98];
    let sky_dusk = [0.61, 0.44, 0.56];
    let sky_night = [0.08, 0.10, 0.16];
    let horizon_day = [0.80, 0.91, 0.99];
    let horizon_dusk = [0.98, 0.63, 0.41];
    let horizon_night = [0.11, 0.13, 0.20];
    let sun_color_day = [1.02, 1.0, 0.95];
    let sun_color_dusk = [1.10, 0.76, 0.49];

    let mut sky_color = lerp3(lerp3(sky_night, sky_dusk, twilight), sky_day, daylight);
    let mut horizon_color = lerp3(
        lerp3(horizon_night, horizon_dusk, twilight),
        horizon_day,
        daylight,
    );
    sky_color = lerp3(sky_color, [0.55, 0.58, 0.64], overcast * 0.45);
    horizon_color = lerp3(horizon_color, [0.58, 0.61, 0.66], overcast * 0.38);

    RenderEnvironment {
        time_of_day_hours: hours,
        sun_direction: normalize3([0.42, 0.12 + daylight * 0.88, -0.24]),
        sun_color: lerp3(sun_color_dusk, sun_color_day, daylight),
        sun_intensity: (0.10 + daylight * 1.18) * (1.0 - overcast * 0.28),
        ambient_color: lerp3([0.13, 0.15, 0.22], [0.56, 0.64, 0.74], daylight),
        ambient_intensity: 0.22 + daylight * 0.84,
        fog_color: lerp3(horizon_color, sky_color, 0.35),
        fog_density: 0.0038 + overcast * 0.0042 + (1.0 - daylight) * 0.0032,
        fog_height_falloff: 0.032 + overcast * 0.010,
        sky_color,
        horizon_color,
        overcast,
        weather_strength,
        wetness,
        climate_tint: [
            (1.0 + temperature_bias * 0.10).clamp(0.82, 1.18),
            (1.0 + humidity * 0.04).clamp(0.86, 1.14),
            (1.0 - temperature_bias * 0.08).clamp(0.82, 1.18),
        ],
        climate_humidity: humidity,
        climate_temperature_bias: temperature_bias,
        top_face_boost: 0.18 + daylight * 0.16,
        side_shadow_strength: 0.28 + (1.0 - daylight) * 0.18 + overcast * 0.10,
        silhouette_boost: 0.16 + (1.0 - daylight) * 0.14,
        saturation_boost: 0.02 + daylight * 0.04 - overcast * 0.03,
    }
}

fn atlas_coord_for_translation(translation: [f32; 3]) -> AtlasCoord {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32).max(1);
    AtlasCoord::new(
        (translation[0].floor() as i32).div_euclid(atlas_span_blocks),
        (translation[2].floor() as i32).div_euclid(atlas_span_blocks),
    )
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
    if length_sq <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        let inv_length = length_sq.sqrt().recip();
        [
            vector[0] * inv_length,
            vector[1] * inv_length,
            vector[2] * inv_length,
        ]
    }
}

fn lerp3(start: [f32; 3], end: [f32; 3], t: f32) -> [f32; 3] {
    [
        start[0] + (end[0] - start[0]) * t,
        start[1] + (end[1] - start[1]) * t,
        start[2] + (end[2] - start[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::world::{
        AtlasCoord, BlockRegistry, ChunkCoord, ClimateRegime, LocalWeatherState, WorldCalendar,
        WorldMeta,
    };

    #[test]
    fn clear_default_evening_environment_keeps_atmosphere_subtle() {
        let environment = render_environment_from_world(
            WorldCalendar::default(),
            AtlasClimateRuntimeState::default(),
            LocalWeatherState::clear(
                AtlasCoord::new(0, 0),
                ClimateRegime::TemperateSeasonal,
                0,
                0,
            ),
        );

        assert!(environment.fog_density <= 0.008);
        assert!(environment.fog_height_falloff <= 0.04);
        assert!(environment.validate().is_ok());
    }

    #[test]
    fn ecology_chunk_inputs_use_runtime_compat_graph_biome() {
        let registry = Arc::new(BlockRegistry::load_default().expect("default registry loads"));
        let world = WorldCore::new(WorldMeta::new(7), registry);
        let chunks = vec![
            ChunkCoord(-1, 0, -1),
            ChunkCoord(0, 0, 0),
            ChunkCoord(1, 0, 1),
        ];

        let inputs = ecology_chunk_inputs_from_world(&world, &chunks);

        assert_eq!(inputs.len(), chunks.len());
        for (input, coord) in inputs.iter().zip(chunks) {
            assert_eq!(input.coord, coord);
            assert_eq!(
                input.biome,
                graph_biome_for_runtime_compat(
                    world.observe_chunk_surface_condition(coord).cell_biome
                )
            );
        }
    }
}
