use crate::world::{
    ChunkCoord, ChunkWeatherKind, ChunkWeatherState, ChunkWeatherUpdate, SeasonalPhase,
    WorldCalendar,
    generation::{GraphBiomeContext, GraphBiomeKind},
};

use super::{SimEvent, SimRegion, SimSpatialScope, SimTick, SimulationResult, SubSystemId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherSimConfig {
    pub ticks_per_game_hour: u32,
}

impl WeatherSimConfig {
    pub fn for_fixed_rate(ticks_per_second: u32) -> Self {
        Self {
            ticks_per_game_hour: ticks_per_second.max(1) * 60,
        }
    }
}

impl Default for WeatherSimConfig {
    fn default() -> Self {
        Self::for_fixed_rate(20)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSimChunkInput {
    pub coord: ChunkCoord,
    pub biome: GraphBiomeKind,
    pub context: GraphBiomeContext,
    pub previous_weather: ChunkWeatherState,
    pub neighbor_weather: Vec<ChunkWeatherState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSimBundleInput {
    pub world_seed: u64,
    pub calendar: WorldCalendar,
    pub chunks: Vec<WeatherSimChunkInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSimInput {
    pub tick: SimTick,
    pub region: SimRegion,
    pub world_seed: u64,
    pub calendar: WorldCalendar,
    pub chunks: Vec<WeatherSimChunkInput>,
}

pub struct WeatherSim {
    config: WeatherSimConfig,
}

impl WeatherSim {
    pub fn new(config: WeatherSimConfig) -> Self {
        Self { config }
    }

    pub fn step(&self, input: WeatherSimInput) -> SimulationResult {
        let mut result = SimulationResult::empty(SubSystemId::Weather, input.tick);
        let hour_period = u64::from(self.config.ticks_per_game_hour.max(1));

        if input.tick.index % hour_period != 0 {
            return result;
        }

        let weather_window = input.tick.index / hour_period;
        let mut chunks = input.chunks;
        chunks.sort_by_key(|chunk| chunk.coord);

        for chunk in chunks {
            let previous = chunk.previous_weather.clamped();
            let state = update_chunk_weather(
                input.world_seed,
                weather_window,
                input.calendar.season_phase,
                chunk.coord,
                chunk.biome,
                chunk.context,
                previous,
                &chunk.neighbor_weather,
                input.tick.index,
            );
            result.chunk_weather_updates.push(ChunkWeatherUpdate {
                coord: chunk.coord,
                state,
            });
            result.events.push(SimEvent::ChunkWeatherUpdated {
                scope: SimSpatialScope::Chunk(chunk.coord),
                biome: chunk.biome,
                previous,
                state,
            });
        }

        result
    }
}

fn update_chunk_weather(
    world_seed: u64,
    weather_window: u64,
    season: SeasonalPhase,
    coord: ChunkCoord,
    biome: GraphBiomeKind,
    context: GraphBiomeContext,
    previous: ChunkWeatherState,
    neighbors: &[ChunkWeatherState],
    updated_at_tick: u64,
) -> ChunkWeatherState {
    let profile = profile_for_biome(biome);
    let context = context.clamped();
    let midpoint = profile.midpoint();
    let mut target = WeatherScalars {
        temperature: lerp(midpoint.temperature, context.temperature, 0.45),
        moisture: lerp(midpoint.moisture, context.hydration, 0.55),
        cloud: midpoint.cloud + (context.hydration - 0.5) * 0.22
            - (context.temperature - 0.5) * 0.05,
        rain: midpoint.rain + (context.hydration - 0.5) * 0.25,
    };
    target = target.add(seasonal_coefficients(biome, season));

    let jitter = WeatherScalars {
        temperature: hash_signed01(world_seed, weather_window, coord, 0xA771_0001) * 0.025,
        moisture: hash_signed01(world_seed, weather_window, coord, 0xA771_0002) * 0.030,
        cloud: hash_signed01(world_seed, weather_window, coord, 0xA771_0003) * 0.040,
        rain: hash_signed01(world_seed, weather_window, coord, 0xA771_0004) * 0.035,
    };
    target = profile.clamp(target.add(jitter));

    let neighbor = average_weather(neighbors).unwrap_or(target);
    let blended_target = target.mix(profile.clamp(neighbor), 0.20);
    let previous = WeatherScalars::from_state(previous);
    let next = profile.clamp(previous.mix(blended_target, 0.25));
    let kind = derive_chunk_weather_kind(next.temperature, next.cloud, next.rain);

    ChunkWeatherState {
        temperature: next.temperature,
        moisture: next.moisture,
        cloud: next.cloud,
        rain: next.rain,
        kind,
        updated_at_tick,
    }
}

pub fn derive_chunk_weather_kind(temperature: f32, cloud: f32, rain: f32) -> ChunkWeatherKind {
    if cloud >= 0.82 && rain >= 0.72 {
        ChunkWeatherKind::Storm
    } else if rain >= 0.35 && temperature <= 0.18 {
        ChunkWeatherKind::Snow
    } else if rain >= 0.40 {
        ChunkWeatherKind::Rain
    } else if cloud >= 0.38 {
        ChunkWeatherKind::Cloudy
    } else {
        ChunkWeatherKind::Clear
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WeatherScalars {
    temperature: f32,
    moisture: f32,
    cloud: f32,
    rain: f32,
}

impl WeatherScalars {
    fn from_state(state: ChunkWeatherState) -> Self {
        Self {
            temperature: state.temperature,
            moisture: state.moisture,
            cloud: state.cloud,
            rain: state.rain,
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            temperature: self.temperature + other.temperature,
            moisture: self.moisture + other.moisture,
            cloud: self.cloud + other.cloud,
            rain: self.rain + other.rain,
        }
    }

    fn mix(self, other: Self, t: f32) -> Self {
        Self {
            temperature: lerp(self.temperature, other.temperature, t),
            moisture: lerp(self.moisture, other.moisture, t),
            cloud: lerp(self.cloud, other.cloud, t),
            rain: lerp(self.rain, other.rain, t),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WeatherProfile {
    temperature: ScalarRange,
    moisture: ScalarRange,
    cloud: ScalarRange,
    rain: ScalarRange,
}

impl WeatherProfile {
    fn midpoint(self) -> WeatherScalars {
        WeatherScalars {
            temperature: self.temperature.midpoint(),
            moisture: self.moisture.midpoint(),
            cloud: self.cloud.midpoint(),
            rain: self.rain.midpoint(),
        }
    }

    fn clamp(self, scalars: WeatherScalars) -> WeatherScalars {
        WeatherScalars {
            temperature: self.temperature.clamp(scalars.temperature),
            moisture: self.moisture.clamp(scalars.moisture),
            cloud: self.cloud.clamp(scalars.cloud),
            rain: self.rain.clamp(scalars.rain),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScalarRange {
    min: f32,
    max: f32,
}

impl ScalarRange {
    const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    fn midpoint(self) -> f32 {
        (self.min + self.max) * 0.5
    }

    fn clamp(self, value: f32) -> f32 {
        if value.is_nan() {
            self.min
        } else {
            value.clamp(0.0, 1.0).clamp(self.min, self.max)
        }
    }
}

fn profile_for_biome(biome: GraphBiomeKind) -> WeatherProfile {
    use GraphBiomeKind::*;

    match biome {
        ShallowOcean => profile(0.36, 0.72, 0.70, 1.00, 0.35, 0.80, 0.20, 0.70),
        DeepOcean => profile(0.32, 0.68, 0.75, 1.00, 0.35, 0.85, 0.25, 0.75),
        Lake => profile(0.30, 0.70, 0.65, 1.00, 0.30, 0.78, 0.18, 0.68),
        Marsh => profile(0.28, 0.68, 0.70, 1.00, 0.38, 0.85, 0.25, 0.75),
        Swamp => profile(0.42, 0.78, 0.72, 1.00, 0.40, 0.88, 0.25, 0.78),
        FloodedForest => profile(0.38, 0.74, 0.72, 1.00, 0.38, 0.85, 0.24, 0.76),
        Mangrove => profile(0.66, 0.94, 0.75, 1.00, 0.35, 0.82, 0.25, 0.78),
        EstuarineCoast => profile(0.46, 0.78, 0.68, 1.00, 0.38, 0.86, 0.24, 0.76),
        LagoonCoast => profile(0.58, 0.88, 0.68, 1.00, 0.30, 0.78, 0.18, 0.68),
        RockyCoast => profile(0.28, 0.68, 0.45, 0.85, 0.35, 0.82, 0.18, 0.68),
        SandyCoast => profile(0.46, 0.82, 0.38, 0.78, 0.22, 0.68, 0.08, 0.52),
        Desert => profile(0.78, 1.00, 0.00, 0.08, 0.00, 0.12, 0.00, 0.02),
        SemiDesert => profile(0.62, 0.92, 0.05, 0.22, 0.02, 0.25, 0.00, 0.10),
        Steppe => profile(0.32, 0.72, 0.12, 0.38, 0.10, 0.45, 0.02, 0.25),
        DryShrubland => profile(0.48, 0.82, 0.10, 0.35, 0.08, 0.42, 0.02, 0.22),
        MediterraneanShrubland => profile(0.46, 0.80, 0.18, 0.48, 0.12, 0.50, 0.04, 0.32),
        PolarIce => profile(0.00, 0.18, 0.05, 0.28, 0.12, 0.55, 0.00, 0.25),
        PolarBarrens => profile(0.02, 0.26, 0.08, 0.35, 0.12, 0.55, 0.00, 0.28),
        Tundra => profile(0.08, 0.36, 0.20, 0.55, 0.18, 0.65, 0.04, 0.38),
        SubalpineWoodland => profile(0.18, 0.52, 0.35, 0.70, 0.22, 0.72, 0.08, 0.50),
        AlpineMeadow => profile(0.12, 0.46, 0.28, 0.62, 0.18, 0.68, 0.06, 0.45),
        BorealForest => profile(0.12, 0.48, 0.38, 0.78, 0.25, 0.78, 0.10, 0.58),
        TropicalRainforest => profile(0.70, 0.96, 0.78, 1.00, 0.45, 0.92, 0.35, 0.90),
        MonsoonForest => profile(0.68, 0.94, 0.55, 0.95, 0.32, 0.88, 0.18, 0.82),
        TropicalDryForest => profile(0.66, 0.92, 0.28, 0.62, 0.18, 0.62, 0.06, 0.42),
        Savanna => profile(0.66, 0.94, 0.18, 0.55, 0.12, 0.58, 0.04, 0.38),
        TemperateRainforest => profile(0.32, 0.68, 0.72, 1.00, 0.42, 0.90, 0.28, 0.82),
        TemperateMixedForest => profile(0.28, 0.70, 0.42, 0.78, 0.24, 0.72, 0.10, 0.56),
        TemperateBroadleafForest => profile(0.34, 0.74, 0.35, 0.72, 0.22, 0.70, 0.08, 0.52),
        TemperateGrassland => profile(0.30, 0.76, 0.18, 0.48, 0.12, 0.55, 0.04, 0.34),
    }
}

fn profile(
    temp_min: f32,
    temp_max: f32,
    moisture_min: f32,
    moisture_max: f32,
    cloud_min: f32,
    cloud_max: f32,
    rain_min: f32,
    rain_max: f32,
) -> WeatherProfile {
    WeatherProfile {
        temperature: ScalarRange::new(temp_min, temp_max),
        moisture: ScalarRange::new(moisture_min, moisture_max),
        cloud: ScalarRange::new(cloud_min, cloud_max),
        rain: ScalarRange::new(rain_min, rain_max),
    }
}

fn seasonal_coefficients(biome: GraphBiomeKind, season: SeasonalPhase) -> WeatherScalars {
    use GraphBiomeKind::*;

    let group = match biome {
        Desert | SemiDesert | Steppe | DryShrubland | MediterraneanShrubland => 0,
        TemperateRainforest
        | TemperateMixedForest
        | TemperateBroadleafForest
        | TemperateGrassland => 1,
        TropicalRainforest | MonsoonForest | TropicalDryForest | Savanna | Mangrove => 2,
        PolarIce | PolarBarrens | Tundra | SubalpineWoodland | AlpineMeadow | BorealForest => 3,
        _ => 4,
    };

    match (group, season) {
        (0, SeasonalPhase::Spring | SeasonalPhase::WetSeason | SeasonalPhase::Thaw) => {
            coeff(0.02, 0.02, 0.0, 0.0)
        }
        (0, SeasonalPhase::Summer | SeasonalPhase::DrySeason) => coeff(0.05, -0.01, 0.0, 0.0),
        (0, SeasonalPhase::Autumn) => coeff(-0.01, 0.01, 0.0, 0.0),
        (0, SeasonalPhase::Winter) => coeff(-0.04, 0.0, 0.01, 0.0),
        (1, SeasonalPhase::Spring | SeasonalPhase::Thaw) => coeff(0.0, 0.06, 0.0, 0.04),
        (1, SeasonalPhase::Summer | SeasonalPhase::DrySeason) => coeff(0.06, 0.0, 0.0, -0.02),
        (1, SeasonalPhase::Autumn | SeasonalPhase::WetSeason) => coeff(0.0, 0.04, 0.05, 0.0),
        (1, SeasonalPhase::Winter) => coeff(-0.12, 0.0, 0.0, 0.03),
        (2, SeasonalPhase::Spring | SeasonalPhase::WetSeason | SeasonalPhase::Thaw) => {
            coeff(0.0, 0.0, 0.0, 0.08)
        }
        (2, SeasonalPhase::Summer) => coeff(0.03, 0.0, 0.0, 0.10),
        (2, SeasonalPhase::Autumn | SeasonalPhase::DrySeason) => coeff(0.0, 0.0, 0.0, -0.04),
        (2, SeasonalPhase::Winter) => coeff(-0.02, 0.0, 0.0, -0.06),
        (3, SeasonalPhase::Spring | SeasonalPhase::Thaw) => coeff(0.0, 0.04, 0.0, 0.0),
        (3, SeasonalPhase::Summer | SeasonalPhase::WetSeason) => coeff(0.08, 0.0, 0.0, 0.02),
        (3, SeasonalPhase::Autumn) => coeff(0.0, 0.0, 0.04, 0.0),
        (3, SeasonalPhase::Winter | SeasonalPhase::DrySeason) => coeff(-0.16, 0.0, 0.05, 0.0),
        (_, SeasonalPhase::Spring | SeasonalPhase::WetSeason | SeasonalPhase::Thaw) => {
            coeff(0.0, 0.03, 0.0, 0.0)
        }
        (_, SeasonalPhase::Summer | SeasonalPhase::DrySeason) => coeff(0.03, 0.0, 0.0, 0.0),
        (_, SeasonalPhase::Autumn) => coeff(0.0, 0.0, 0.04, 0.0),
        (_, SeasonalPhase::Winter) => coeff(-0.06, 0.0, 0.03, 0.0),
    }
}

fn coeff(temperature: f32, moisture: f32, cloud: f32, rain: f32) -> WeatherScalars {
    WeatherScalars {
        temperature,
        moisture,
        cloud,
        rain,
    }
}

fn average_weather(neighbors: &[ChunkWeatherState]) -> Option<WeatherScalars> {
    if neighbors.is_empty() {
        return None;
    }

    let mut total = WeatherScalars {
        temperature: 0.0,
        moisture: 0.0,
        cloud: 0.0,
        rain: 0.0,
    };
    for neighbor in neighbors {
        total = total.add(WeatherScalars::from_state(neighbor.clamped()));
    }
    let count = neighbors.len() as f32;
    Some(WeatherScalars {
        temperature: total.temperature / count,
        moisture: total.moisture / count,
        cloud: total.cloud / count,
        rain: total.rain / count,
    })
}

fn lerp(current: f32, target: f32, t: f32) -> f32 {
    current + (target - current) * t.clamp(0.0, 1.0)
}

fn hash_signed01(world_seed: u64, weather_window: u64, coord: ChunkCoord, salt: u64) -> f32 {
    hash01(world_seed, weather_window, coord, salt) * 2.0 - 1.0
}

fn hash01(world_seed: u64, weather_window: u64, coord: ChunkCoord, salt: u64) -> f32 {
    let mut state = world_seed
        ^ weather_window.rotate_left(17)
        ^ (coord.0 as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (coord.1 as i64 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ (coord.2 as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
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
    use crate::world::{AtlasArea, AtlasCoord, generation::GraphBiomeWaterRole};

    fn context(temperature: f32, hydration: f32) -> GraphBiomeContext {
        GraphBiomeContext {
            temperature,
            hydration,
            elevation: 0.0,
            continentality: 0.2,
            coastness: 0.0,
            mountainness: 0.0,
            ruggedness: 0.0,
            water_role: GraphBiomeWaterRole::Land,
        }
    }

    fn state(
        temperature: f32,
        moisture: f32,
        cloud: f32,
        rain: f32,
        updated_at_tick: u64,
    ) -> ChunkWeatherState {
        ChunkWeatherState {
            temperature,
            moisture,
            cloud,
            rain,
            kind: derive_chunk_weather_kind(temperature, cloud, rain),
            updated_at_tick,
        }
    }

    fn input(tick_index: u64, chunks: Vec<WeatherSimChunkInput>) -> WeatherSimInput {
        let center = AtlasCoord::new(0, 0);
        WeatherSimInput {
            tick: SimTick {
                index: tick_index,
                delta: Duration::from_millis(50),
            },
            region: SimRegion {
                center_atlas: center,
                atlas_area: AtlasArea::new(center, 1, 1).unwrap(),
            },
            world_seed: 11,
            calendar: WorldCalendar::default(),
            chunks,
        }
    }

    #[test]
    fn weather_kind_thresholds_match_contract_order() {
        assert_eq!(
            derive_chunk_weather_kind(0.70, 0.83, 0.73),
            ChunkWeatherKind::Storm
        );
        assert_eq!(
            derive_chunk_weather_kind(0.18, 0.40, 0.35),
            ChunkWeatherKind::Snow
        );
        assert_eq!(
            derive_chunk_weather_kind(0.50, 0.20, 0.40),
            ChunkWeatherKind::Rain
        );
        assert_eq!(
            derive_chunk_weather_kind(0.50, 0.38, 0.10),
            ChunkWeatherKind::Cloudy
        );
        assert_eq!(
            derive_chunk_weather_kind(0.50, 0.37, 0.39),
            ChunkWeatherKind::Clear
        );
    }

    #[test]
    fn desert_clamps_rain_and_moisture_near_zero() {
        let sim = WeatherSim::new(WeatherSimConfig {
            ticks_per_game_hour: 10,
        });
        let coord = ChunkCoord(0, 0, 0);
        let wet_neighbor = state(0.70, 1.0, 1.0, 1.0, 0);
        let result = sim.step(input(
            10,
            vec![WeatherSimChunkInput {
                coord,
                biome: GraphBiomeKind::Desert,
                context: context(1.0, 1.0),
                previous_weather: state(0.90, 0.90, 0.90, 0.90, 0),
                neighbor_weather: vec![wet_neighbor; 4],
            }],
        ));

        let update = result.chunk_weather_updates[0].state;
        assert!(update.moisture <= 0.08);
        assert!(update.rain <= 0.02);
        assert!(update.cloud <= 0.12);
    }

    #[test]
    fn neighbor_blending_pulls_chunk_toward_adjacent_weather() {
        let sim = WeatherSim::new(WeatherSimConfig {
            ticks_per_game_hour: 10,
        });
        let coord = ChunkCoord(0, 0, 0);
        let dry = WeatherSimChunkInput {
            coord,
            biome: GraphBiomeKind::TemperateRainforest,
            context: context(0.56, 0.72),
            previous_weather: state(0.50, 0.72, 0.42, 0.28, 0),
            neighbor_weather: Vec::new(),
        };
        let wet_neighbors = WeatherSimChunkInput {
            neighbor_weather: vec![state(0.55, 1.0, 0.90, 0.82, 0); 4],
            ..dry.clone()
        };

        let without_neighbors = sim.step(input(10, vec![dry])).chunk_weather_updates[0].state;
        let with_neighbors = sim
            .step(input(10, vec![wet_neighbors]))
            .chunk_weather_updates[0]
            .state;

        assert!(with_neighbors.cloud > without_neighbors.cloud);
        assert!(with_neighbors.rain > without_neighbors.rain);
        assert!(with_neighbors.moisture > without_neighbors.moisture);
    }

    #[test]
    fn weather_updates_only_on_hour_boundary() {
        let sim = WeatherSim::new(WeatherSimConfig {
            ticks_per_game_hour: 10,
        });
        let coord = ChunkCoord(0, 0, 0);
        let chunk = WeatherSimChunkInput {
            coord,
            biome: GraphBiomeKind::TemperateGrassland,
            context: context(0.50, 0.50),
            previous_weather: ChunkWeatherState::clear(0),
            neighbor_weather: Vec::new(),
        };

        let skipped = sim.step(input(9, vec![chunk.clone()]));
        let updated = sim.step(input(10, vec![chunk]));

        assert!(skipped.chunk_weather_updates.is_empty());
        assert!(skipped.events.is_empty());
        assert_eq!(updated.chunk_weather_updates.len(), 1);
        assert!(matches!(
            updated.events.as_slice(),
            [SimEvent::ChunkWeatherUpdated { .. }]
        ));
    }
}
