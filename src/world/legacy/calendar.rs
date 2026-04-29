use super::atlas::{AtlasCoord, ClimateRegime};
use super::surface::{SeasonalBiomeStateId, SeasonalPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCalendar {
    pub absolute_tick: u64,
    pub minute: u32,
    pub hour: u32,
    pub day: u64,
    pub day_of_year: u32,
    pub season_index: u8,
    pub season_phase: SeasonalPhase,
}

impl WorldCalendar {
    pub const MINUTES_PER_HOUR: u32 = 60;
    pub const HOURS_PER_DAY: u32 = 24;
    pub const MINUTES_PER_DAY: u32 = Self::MINUTES_PER_HOUR * Self::HOURS_PER_DAY;

    pub const fn new(
        absolute_tick: u64,
        minute: u32,
        hour: u32,
        day: u64,
        day_of_year: u32,
        season_index: u8,
        season_phase: SeasonalPhase,
    ) -> Self {
        Self {
            absolute_tick,
            minute,
            hour,
            day,
            day_of_year,
            season_index,
            season_phase,
        }
    }

    pub fn time_of_day_hours(self) -> f32 {
        self.hour as f32 + self.minute as f32 / Self::MINUTES_PER_HOUR as f32
    }

    pub fn absolute_minutes(self) -> u64 {
        self.day
            .saturating_mul(Self::MINUTES_PER_DAY as u64)
            .saturating_add((self.hour * Self::MINUTES_PER_HOUR + self.minute) as u64)
    }

    pub fn advanced_minute(self, days_per_year: u32) -> Self {
        let safe_days_per_year = days_per_year.max(4);
        let mut next = self;
        next.minute += 1;

        if next.minute >= Self::MINUTES_PER_HOUR {
            next.minute = 0;
            next.hour += 1;
        }

        if next.hour >= Self::HOURS_PER_DAY {
            next.hour = 0;
            next.day = next.day.saturating_add(1);
            next.day_of_year = (next.day_of_year + 1) % safe_days_per_year;
        }

        next.season_index = season_index_for_day(next.day_of_year, safe_days_per_year);
        next.season_phase = season_phase_for_index(next.season_index);
        next
    }
}

impl Default for WorldCalendar {
    fn default() -> Self {
        Self {
            absolute_tick: 0,
            minute: 0,
            hour: 11,
            day: 0,
            day_of_year: 0,
            season_index: 0,
            season_phase: SeasonalPhase::Spring,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasClimateRuntimeState {
    pub temperature_offset: f32,
    pub humidity_offset: f32,
    pub last_updated_tick: u64,
}

impl AtlasClimateRuntimeState {
    pub const fn new(last_updated_tick: u64) -> Self {
        Self {
            temperature_offset: 0.0,
            humidity_offset: 0.0,
            last_updated_tick,
        }
    }
}

impl Default for AtlasClimateRuntimeState {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalWeatherKind {
    Clear,
    Overcast,
    Rain,
    Snow,
    Storm,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalWeatherState {
    pub kind: LocalWeatherKind,
    pub intensity: f32,
    pub window_start_tick: u64,
    pub window_end_tick: u64,
    pub source_atlas: AtlasCoord,
    pub climate_regime: ClimateRegime,
}

impl LocalWeatherState {
    pub fn clear(
        source_atlas: AtlasCoord,
        climate_regime: ClimateRegime,
        window_start_tick: u64,
        window_end_tick: u64,
    ) -> Self {
        Self {
            kind: LocalWeatherKind::Clear,
            intensity: 0.0,
            window_start_tick,
            window_end_tick,
            source_atlas,
            climate_regime,
        }
    }

    pub fn overcast_factor(self) -> f32 {
        match self.kind {
            LocalWeatherKind::Clear => 0.06,
            LocalWeatherKind::Overcast => 0.48 + self.intensity * 0.28,
            LocalWeatherKind::Rain => 0.62 + self.intensity * 0.24,
            LocalWeatherKind::Snow => 0.68 + self.intensity * 0.20,
            LocalWeatherKind::Storm => 0.82 + self.intensity * 0.16,
        }
        .clamp(0.0, 1.0)
    }

    pub fn weather_strength(self) -> f32 {
        match self.kind {
            LocalWeatherKind::Clear => 0.0,
            LocalWeatherKind::Overcast => 0.15 + self.intensity * 0.10,
            LocalWeatherKind::Rain => 0.50 + self.intensity * 0.35,
            LocalWeatherKind::Snow => 0.44 + self.intensity * 0.30,
            LocalWeatherKind::Storm => 0.76 + self.intensity * 0.22,
        }
        .clamp(0.0, 1.0)
    }

    pub fn wetness_factor(self) -> f32 {
        match self.kind {
            LocalWeatherKind::Clear => 0.0,
            LocalWeatherKind::Overcast => 0.10 + self.intensity * 0.08,
            LocalWeatherKind::Rain => 0.42 + self.intensity * 0.34,
            LocalWeatherKind::Snow => 0.24 + self.intensity * 0.18,
            LocalWeatherKind::Storm => 0.64 + self.intensity * 0.26,
        }
        .clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredSeasonPatchTarget {
    AtlasCell(AtlasCoord),
    ChunkColumn { chunk_x: i32, chunk_z: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredSeasonPatchKind {
    SetSeasonalState(SeasonalBiomeStateId),
    SnowCover,
    Thaw,
    Bloom,
    LeafTintShift,
    BareBranchConversion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredSeasonPatch {
    pub target: DeferredSeasonPatchTarget,
    pub kind: DeferredSeasonPatchKind,
    pub authored_at_tick: u64,
    pub apply_on_realization: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasClimateRuntimeUpdate {
    pub coord: AtlasCoord,
    pub state: AtlasClimateRuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalWeatherUpdate {
    pub coord: AtlasCoord,
    pub state: LocalWeatherState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarAdvance {
    pub calendar: WorldCalendar,
    pub climate_updates: Vec<AtlasClimateRuntimeUpdate>,
    pub local_weather_updates: Vec<LocalWeatherUpdate>,
    pub deferred_patches: Vec<DeferredSeasonPatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CalendarApplyResult {
    pub changed_atlas_cells: Vec<AtlasCoord>,
    pub weather_changed_cells: Vec<AtlasCoord>,
    pub deferred_patch_count: usize,
}

pub fn season_index_for_day(day_of_year: u32, days_per_year: u32) -> u8 {
    let safe_days_per_year = days_per_year.max(4);
    ((day_of_year.saturating_mul(4)) / safe_days_per_year)
        .min(3)
        .try_into()
        .expect("season index must stay in 0..=3")
}

pub fn season_phase_for_index(index: u8) -> SeasonalPhase {
    match index {
        0 => SeasonalPhase::Spring,
        1 => SeasonalPhase::Summer,
        2 => SeasonalPhase::Autumn,
        _ => SeasonalPhase::Winter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_calendar_starts_at_eleven_am() {
        let calendar = WorldCalendar::default();

        assert_eq!(calendar.hour, 11);
        assert_eq!(calendar.minute, 0);
        assert_eq!(calendar.time_of_day_hours(), 11.0);
    }
}
