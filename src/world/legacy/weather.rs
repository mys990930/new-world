use super::coord::ChunkCoord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkWeatherKind {
    Clear,
    Cloudy,
    Rain,
    Snow,
    Storm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkWeatherState {
    pub temperature: f32,
    pub moisture: f32,
    pub cloud: f32,
    pub rain: f32,
    pub kind: ChunkWeatherKind,
    pub updated_at_tick: u64,
}

impl ChunkWeatherState {
    pub const fn clear(updated_at_tick: u64) -> Self {
        Self {
            temperature: 0.5,
            moisture: 0.0,
            cloud: 0.0,
            rain: 0.0,
            kind: ChunkWeatherKind::Clear,
            updated_at_tick,
        }
    }

    pub fn clamped(self) -> Self {
        Self {
            temperature: self.temperature.clamp(0.0, 1.0),
            moisture: self.moisture.clamp(0.0, 1.0),
            cloud: self.cloud.clamp(0.0, 1.0),
            rain: self.rain.clamp(0.0, 1.0),
            kind: self.kind,
            updated_at_tick: self.updated_at_tick,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkWeatherUpdate {
    pub coord: ChunkCoord,
    pub state: ChunkWeatherState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherApplyResult {
    pub coord: ChunkCoord,
    pub previous: Option<ChunkWeatherState>,
    pub current: ChunkWeatherState,
    pub changed: bool,
}
