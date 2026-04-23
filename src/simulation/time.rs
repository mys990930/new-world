use crate::world::{
    AtlasClimateRuntimeState, AtlasClimateRuntimeUpdate, AtlasCoord, CalendarAdvance,
    ClimateRegime, DeferredSeasonPatch, DeferredSeasonPatchKind, DeferredSeasonPatchTarget,
    ElevationBand, HydrologyContext, LocalWeatherKind, LocalWeatherState, LocalWeatherUpdate,
    MoistureBand, RegionClassSample, SeasonalBiomeStateId, SeasonalPhase, TemperatureBand,
    WorldCalendar,
};

use super::{SimEvent, SimFollowupRequest, SimRegion, SimTick, SimulationResult, SubSystemId};

#[derive(Debug, Clone, PartialEq)]
pub struct TimeSimConfig {
    pub ticks_per_game_minute: u32,
    pub days_per_year: u32,
    pub climate_response: f32,
}

impl TimeSimConfig {
    pub fn for_fixed_rate(ticks_per_second: u32) -> Self {
        Self {
            ticks_per_game_minute: ticks_per_second.max(1),
            days_per_year: 360,
            climate_response: 0.35,
        }
    }
}

impl Default for TimeSimConfig {
    fn default() -> Self {
        Self::for_fixed_rate(20)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSimCellInput {
    pub coord: AtlasCoord,
    pub region: RegionClassSample,
    pub climate_state: AtlasClimateRuntimeState,
    pub current_weather: LocalWeatherState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeSimBundleInput {
    pub world_seed: u64,
    pub calendar: WorldCalendar,
    pub cells: Vec<TimeSimCellInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeSimInput {
    pub tick: SimTick,
    pub region: SimRegion,
    pub world_seed: u64,
    pub calendar: WorldCalendar,
    pub cells: Vec<TimeSimCellInput>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalClimateState {
    pub temperature_signal: f32,
    pub humidity_factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalClimateDisplay {
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
}

pub struct TimeSim {
    config: TimeSimConfig,
}

impl TimeSim {
    pub fn new(config: TimeSimConfig) -> Self {
        Self { config }
    }

    pub fn step(&self, input: TimeSimInput) -> SimulationResult {
        let mut result = SimulationResult::empty(SubSystemId::Time, input.tick);
        result
            .events
            .push(SimEvent::FixedTickAdvanced { tick: input.tick.index });

        let ticks_per_game_minute = u64::from(self.config.ticks_per_game_minute.max(1));
        let minute_boundary = input.tick.index % ticks_per_game_minute == 0;
        let previous_calendar = input.calendar;
        let mut next_calendar = input.calendar;
        next_calendar.absolute_tick = next_calendar.absolute_tick.saturating_add(1);

        if minute_boundary {
            next_calendar = next_calendar.advanced_minute(self.config.days_per_year);
            result.events.push(SimEvent::CalendarMinuteElapsed {
                absolute_minutes: next_calendar.absolute_minutes(),
            });
        }

        let day_boundary = minute_boundary && next_calendar.minute == 0 && next_calendar.hour == 0;
        let season_changed = previous_calendar.season_phase != next_calendar.season_phase;

        let mut climate_updates = Vec::new();
        let mut local_weather_updates = Vec::new();
        let mut deferred_patches = Vec::new();

        if minute_boundary {
            for cell in input.cells {
                let climate_state = update_climate_state(
                    input.world_seed,
                    next_calendar,
                    input.tick.index,
                    cell.coord,
                    cell.region,
                    cell.climate_state,
                    self.config.climate_response,
                );
                climate_updates.push(AtlasClimateRuntimeUpdate {
                    coord: cell.coord,
                    state: climate_state,
                });

                let next_weather = derive_local_weather(
                    input.world_seed,
                    next_calendar,
                    input.tick.index,
                    ticks_per_game_minute,
                    cell.coord,
                    cell.region,
                    climate_state,
                );
                if next_weather != cell.current_weather {
                    result.events.push(SimEvent::WeatherUpdated {
                        coord: cell.coord,
                        kind: next_weather.kind,
                    });
                }
                local_weather_updates.push(LocalWeatherUpdate {
                    coord: cell.coord,
                    state: next_weather,
                });

                if season_changed {
                    deferred_patches.push(DeferredSeasonPatch {
                        target: DeferredSeasonPatchTarget::AtlasCell(cell.coord),
                        kind: DeferredSeasonPatchKind::SetSeasonalState(seasonal_state_for_region(
                            cell.region,
                            next_calendar.season_phase,
                        )),
                        authored_at_tick: input.tick.index,
                        apply_on_realization: true,
                    });
                }
            }
        }

        if day_boundary {
            result
                .events
                .push(SimEvent::DayAdvanced { day: next_calendar.day });
        }

        if !deferred_patches.is_empty() {
            result.events.push(SimEvent::DeferredSeasonPatchesQueued {
                count: deferred_patches.len(),
            });
        }

        result.followups.push(SimFollowupRequest::PersistCalendarState);
        result.calendar_advance = Some(CalendarAdvance {
            calendar: next_calendar,
            climate_updates,
            local_weather_updates,
            deferred_patches,
        });
        result
    }
}

fn update_climate_state(
    world_seed: u64,
    calendar: WorldCalendar,
    tick_index: u64,
    coord: AtlasCoord,
    region: RegionClassSample,
    current: AtlasClimateRuntimeState,
    response: f32,
) -> AtlasClimateRuntimeState {
    let time_of_day = calendar.time_of_day_hours();
    let diurnal = ((time_of_day - 6.0) / 24.0 * std::f32::consts::TAU).sin();
    let seasonality = seasonality_for_regime(region.climate_regime);
    let seasonal_temp = seasonal_temperature_bias(calendar.season_phase, region.climate_regime);
    let seasonal_humidity = seasonal_humidity_bias(calendar.season_phase, region.climate_regime);
    let jitter_temp = hash_signed01(world_seed, coord, calendar.absolute_minutes(), 0xA11C_E001);
    let jitter_humidity = hash_signed01(world_seed, coord, calendar.absolute_minutes(), 0xA11C_E002);

    let target_temp =
        seasonal_temp + diurnal * (0.10 + seasonality * 0.12) + jitter_temp * 0.05;
    let target_humidity = seasonal_humidity + jitter_humidity * 0.07;

    AtlasClimateRuntimeState {
        temperature_offset: lerp(current.temperature_offset, target_temp, response),
        humidity_offset: lerp(current.humidity_offset, target_humidity, response),
        last_updated_tick: tick_index,
    }
}

fn derive_local_weather(
    world_seed: u64,
    calendar: WorldCalendar,
    tick_index: u64,
    ticks_per_game_minute: u64,
    coord: AtlasCoord,
    region: RegionClassSample,
    climate_state: AtlasClimateRuntimeState,
) -> LocalWeatherState {
    let local_climate = evaluate_local_climate(region, climate_state);
    let effective_temperature = local_climate.temperature_signal;
    let effective_humidity = local_climate.humidity_factor;

    let weather_window = calendar.absolute_minutes();
    let cloud_seed = hash01(world_seed, coord, weather_window, 0x0C10_D001);
    let precip_seed = hash01(world_seed, coord, weather_window, 0x0C10_D002);
    let storm_seed = hash01(world_seed, coord, weather_window, 0x0C10_D003);
    let cloudiness = (effective_humidity * 0.74 + cloud_seed * 0.26).clamp(0.0, 1.0);
    let precipitation_signal = (effective_humidity * 0.68 + precip_seed * 0.32).clamp(0.0, 1.0);

    let (kind, intensity) = if storm_seed > 0.90 && cloudiness > 0.72 && effective_humidity > 0.66
    {
        (LocalWeatherKind::Storm, (storm_seed * 0.9).clamp(0.0, 1.0))
    } else if precipitation_signal > 0.62 && effective_humidity > 0.54 {
        if effective_temperature < -0.08 {
            (
                LocalWeatherKind::Snow,
                (precipitation_signal * (1.0 + effective_humidity) * 0.5).clamp(0.0, 1.0),
            )
        } else {
            (
                LocalWeatherKind::Rain,
                (precipitation_signal * (1.0 + effective_humidity) * 0.5).clamp(0.0, 1.0),
            )
        }
    } else if cloudiness > 0.42 {
        (LocalWeatherKind::Overcast, cloudiness)
    } else {
        (LocalWeatherKind::Clear, 0.0)
    };

    LocalWeatherState {
        kind,
        intensity,
        window_start_tick: tick_index,
        window_end_tick: tick_index.saturating_add(ticks_per_game_minute),
        source_atlas: coord,
        climate_regime: region.climate_regime,
    }
}

pub fn evaluate_local_climate(
    region: RegionClassSample,
    climate_state: AtlasClimateRuntimeState,
) -> LocalClimateState {
    let base_temperature = base_temperature_for_band(region.temperature_band);
    let base_humidity = base_humidity_for_band(region.moisture_band);
    let regime_humidity = humidity_bias_for_regime(region.climate_regime);
    let regime_temperature = temperature_bias_for_regime(region.climate_regime);

    LocalClimateState {
        temperature_signal: base_temperature + regime_temperature + climate_state.temperature_offset,
        humidity_factor: (base_humidity + regime_humidity + climate_state.humidity_offset)
            .clamp(0.0, 1.0),
    }
}

pub fn display_local_climate(
    climate: LocalClimateState,
    weather: LocalWeatherState,
) -> LocalClimateDisplay {
    let mut temperature_celsius = (climate.temperature_signal * 21.0) + 13.5;
    temperature_celsius = match weather.kind {
        LocalWeatherKind::Snow => temperature_celsius.min(1.0),
        LocalWeatherKind::Rain if temperature_celsius < 1.0 => 1.0,
        _ => temperature_celsius,
    }
    .clamp(-28.0, 42.0);

    let mut humidity_percent = climate.humidity_factor * 100.0;
    humidity_percent = match weather.kind {
        LocalWeatherKind::Clear => humidity_percent,
        LocalWeatherKind::Overcast => humidity_percent.max(60.0),
        LocalWeatherKind::Rain => humidity_percent.max(82.0),
        LocalWeatherKind::Snow => humidity_percent.max(72.0),
        LocalWeatherKind::Storm => humidity_percent.max(90.0),
    }
    .clamp(0.0, 100.0);

    LocalClimateDisplay {
        temperature_celsius,
        humidity_percent,
    }
}

fn seasonal_state_for_region(
    region: RegionClassSample,
    global_season: SeasonalPhase,
) -> SeasonalBiomeStateId {
    if matches!(region.elevation_band, ElevationBand::Alpine)
        && matches!(global_season, SeasonalPhase::Winter)
    {
        return SeasonalBiomeStateId::AlpineSnowpack;
    }

    if matches!(
        region.hydrology_context,
        HydrologyContext::WetLowland | HydrologyContext::LakeBasin
    ) && matches!(global_season, SeasonalPhase::Winter)
    {
        return SeasonalBiomeStateId::ColdFrozenWetland;
    }

    match region.climate_regime {
        ClimateRegime::TropicalWet => SeasonalBiomeStateId::TropicalWetSeason,
        ClimateRegime::TropicalSeasonal => {
            if matches!(global_season, SeasonalPhase::Spring | SeasonalPhase::Summer) {
                SeasonalBiomeStateId::TropicalWetSeason
            } else {
                SeasonalBiomeStateId::TropicalDrySeason
            }
        }
        ClimateRegime::ColdAlpine if matches!(global_season, SeasonalPhase::Winter) => {
            SeasonalBiomeStateId::AlpineSnowpack
        }
        _ if matches!(global_season, SeasonalPhase::Autumn)
            && !matches!(region.hydrology_context, HydrologyContext::Dryland) =>
        {
            SeasonalBiomeStateId::CoastalStormSeason
        }
        _ if matches!(global_season, SeasonalPhase::Winter) => SeasonalBiomeStateId::TemperateSnowy,
        _ => SeasonalBiomeStateId::TemperateGrowing,
    }
}

fn base_temperature_for_band(band: TemperatureBand) -> f32 {
    match band {
        TemperatureBand::Polar => -0.82,
        TemperatureBand::Cold => -0.46,
        TemperatureBand::Temperate => 0.02,
        TemperatureBand::Warm => 0.34,
        TemperatureBand::Hot => 0.72,
    }
}

fn base_humidity_for_band(band: MoistureBand) -> f32 {
    match band {
        MoistureBand::Arid => 0.10,
        MoistureBand::SemiArid => 0.24,
        MoistureBand::Subhumid => 0.46,
        MoistureBand::Humid => 0.66,
        MoistureBand::Wet => 0.84,
    }
}

fn temperature_bias_for_regime(regime: ClimateRegime) -> f32 {
    match regime {
        ClimateRegime::Polar => -0.18,
        ClimateRegime::ColdAlpine => -0.12,
        ClimateRegime::AridHot => 0.16,
        ClimateRegime::TropicalWet => 0.12,
        ClimateRegime::TropicalSeasonal => 0.10,
        ClimateRegime::Continental => -0.04,
        ClimateRegime::TemperateSeasonal => 0.0,
    }
}

fn humidity_bias_for_regime(regime: ClimateRegime) -> f32 {
    match regime {
        ClimateRegime::Polar => -0.05,
        ClimateRegime::ColdAlpine => -0.04,
        ClimateRegime::AridHot => -0.18,
        ClimateRegime::TropicalWet => 0.18,
        ClimateRegime::TropicalSeasonal => 0.04,
        ClimateRegime::Continental => -0.02,
        ClimateRegime::TemperateSeasonal => 0.0,
    }
}

fn seasonality_for_regime(regime: ClimateRegime) -> f32 {
    match regime {
        ClimateRegime::Polar => 0.75,
        ClimateRegime::ColdAlpine => 0.58,
        ClimateRegime::AridHot => 0.24,
        ClimateRegime::TropicalWet => 0.14,
        ClimateRegime::TropicalSeasonal => 0.36,
        ClimateRegime::Continental => 0.62,
        ClimateRegime::TemperateSeasonal => 0.48,
    }
}

fn seasonal_temperature_bias(phase: SeasonalPhase, regime: ClimateRegime) -> f32 {
    let base = match phase {
        SeasonalPhase::Spring => 0.02,
        SeasonalPhase::Summer => 0.16,
        SeasonalPhase::Autumn => -0.03,
        SeasonalPhase::Winter => -0.20,
        SeasonalPhase::WetSeason => 0.06,
        SeasonalPhase::DrySeason => 0.14,
        SeasonalPhase::Thaw => -0.08,
    };

    base * (0.55 + seasonality_for_regime(regime))
}

fn seasonal_humidity_bias(phase: SeasonalPhase, regime: ClimateRegime) -> f32 {
    match regime {
        ClimateRegime::TropicalSeasonal => match phase {
            SeasonalPhase::Spring | SeasonalPhase::Summer => 0.14,
            SeasonalPhase::Autumn | SeasonalPhase::Winter => -0.10,
            _ => 0.0,
        },
        ClimateRegime::AridHot => match phase {
            SeasonalPhase::Summer => -0.08,
            SeasonalPhase::Winter => 0.04,
            _ => -0.02,
        },
        _ => match phase {
            SeasonalPhase::Spring => 0.04,
            SeasonalPhase::Summer => -0.03,
            SeasonalPhase::Autumn => 0.05,
            SeasonalPhase::Winter => 0.02,
            _ => 0.0,
        },
    }
}

fn lerp(current: f32, target: f32, t: f32) -> f32 {
    current + (target - current) * t.clamp(0.0, 1.0)
}

fn hash_signed01(world_seed: u64, coord: AtlasCoord, time_window: u64, salt: u64) -> f32 {
    hash01(world_seed, coord, time_window, salt) * 2.0 - 1.0
}

fn hash01(world_seed: u64, coord: AtlasCoord, time_window: u64, salt: u64) -> f32 {
    let mut state = world_seed
        ^ ((coord.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ ((coord.z as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        ^ time_window.rotate_left(17)
        ^ salt;
    splitmix64(&mut state) as f32 / u64::MAX as f32
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::world::{
        AtlasArea, AtlasClimateRuntimeState, AtlasCoord, ClimateRegime, CoastalContext,
        ElevationBand, HydrologyContext, MoistureBand, RegionArchetype, RegionClassSample,
        ReliefClass, TemperatureBand, TerrainFormFamily,
    };

    fn sample_region() -> RegionClassSample {
        RegionClassSample {
            temperature_band: TemperatureBand::Temperate,
            moisture_band: MoistureBand::Humid,
            elevation_band: ElevationBand::Low,
            relief_class: ReliefClass::Plain,
            hydrology_context: HydrologyContext::WellDrained,
            coastal_context: CoastalContext::Inland,
            climate_regime: ClimateRegime::TemperateSeasonal,
            biome_family: crate::world::BiomeFamily::TemperateGrassland,
            terrain_form_family: TerrainFormFamily::Plain,
            archetype: RegionArchetype::TemperatePlain,
        }
    }

    #[test]
    fn time_step_advances_calendar_on_minute_boundary() {
        let sim = TimeSim::new(TimeSimConfig {
            ticks_per_game_minute: 2,
            days_per_year: 360,
            climate_response: 0.35,
        });
        let coord = AtlasCoord::new(0, 0);
        let result = sim.step(TimeSimInput {
            tick: SimTick {
                index: 2,
                delta: Duration::from_millis(50),
            },
            region: SimRegion {
                center_atlas: coord,
                atlas_area: AtlasArea::new(coord, 1, 1).unwrap(),
            },
            world_seed: 42,
            calendar: WorldCalendar::default(),
            cells: vec![TimeSimCellInput {
                coord,
                region: sample_region(),
                climate_state: AtlasClimateRuntimeState::default(),
                current_weather: LocalWeatherState::clear(
                    coord,
                    ClimateRegime::TemperateSeasonal,
                    0,
                    2,
                ),
            }],
        });

        let advance = result
            .calendar_advance
            .expect("time step should emit calendar advance");
        assert_eq!(advance.calendar.minute, 22);
        assert_eq!(advance.calendar.absolute_tick, 1);
        assert_eq!(advance.climate_updates.len(), 1);
        assert_eq!(advance.local_weather_updates.len(), 1);
    }

    #[test]
    fn time_step_is_deterministic_for_same_input() {
        let sim = TimeSim::new(TimeSimConfig::default());
        let coord = AtlasCoord::new(-3, 4);
        let input = TimeSimInput {
            tick: SimTick {
                index: 20,
                delta: Duration::from_millis(50),
            },
            region: SimRegion {
                center_atlas: coord,
                atlas_area: AtlasArea::new(coord, 1, 1).unwrap(),
            },
            world_seed: 99,
            calendar: WorldCalendar::default(),
            cells: vec![TimeSimCellInput {
                coord,
                region: sample_region(),
                climate_state: AtlasClimateRuntimeState::default(),
                current_weather: LocalWeatherState::clear(
                    coord,
                    ClimateRegime::TemperateSeasonal,
                    0,
                    20,
                ),
            }],
        };

        let left = sim.step(input.clone());
        let right = sim.step(input);
        assert_eq!(left, right);
    }

    #[test]
    fn display_local_climate_maps_temperate_conditions_into_mild_units() {
        let climate = evaluate_local_climate(
            sample_region(),
            AtlasClimateRuntimeState {
                temperature_offset: 0.0,
                humidity_offset: 0.0,
                last_updated_tick: 0,
            },
        );
        let display = display_local_climate(
            climate,
            LocalWeatherState::clear(
                AtlasCoord::new(0, 0),
                ClimateRegime::TemperateSeasonal,
                0,
                20,
            ),
        );

        assert!((13.0..=15.0).contains(&display.temperature_celsius));
        assert!((64.0..=68.0).contains(&display.humidity_percent));
    }

    #[test]
    fn snowy_weather_caps_display_temperature_near_freezing() {
        let display = display_local_climate(
            LocalClimateState {
                temperature_signal: 0.48,
                humidity_factor: 0.88,
            },
            LocalWeatherState {
                kind: LocalWeatherKind::Snow,
                intensity: 0.6,
                window_start_tick: 0,
                window_end_tick: 20,
                source_atlas: AtlasCoord::new(0, 0),
                climate_regime: ClimateRegime::ColdAlpine,
            },
        );

        assert!(display.temperature_celsius <= 1.0);
        assert!(display.humidity_percent >= 72.0);
    }

    #[test]
    fn storm_display_applies_a_high_humidity_floor() {
        let display = display_local_climate(
            LocalClimateState {
                temperature_signal: 0.12,
                humidity_factor: 0.42,
            },
            LocalWeatherState {
                kind: LocalWeatherKind::Storm,
                intensity: 0.9,
                window_start_tick: 0,
                window_end_tick: 20,
                source_atlas: AtlasCoord::new(1, -1),
                climate_regime: ClimateRegime::TemperateSeasonal,
            },
        );

        assert_eq!(display.humidity_percent, 90.0);
    }
}
