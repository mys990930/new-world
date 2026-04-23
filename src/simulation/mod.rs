mod time;

use std::time::Duration;

use crate::world::{
    AtlasArea, AtlasCoord, CalendarAdvance, ChunkCoord, LocalWeatherKind, WorldEdit,
};

pub use time::{
    LocalClimateDisplay, LocalClimateState, TimeSim, TimeSimBundleInput, TimeSimCellInput,
    TimeSimConfig, TimeSimInput, display_local_climate, evaluate_local_climate,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedStepConfig {
    pub ticks_per_second: u32,
}

impl Default for FixedStepConfig {
    fn default() -> Self {
        Self { ticks_per_second: 20 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationConfig {
    pub fixed: FixedStepConfig,
    pub time: TimeSimConfig,
}

impl SimulationConfig {
    pub fn with_fixed_ticks_per_second(ticks_per_second: u32) -> Self {
        let fixed = FixedStepConfig {
            ticks_per_second: ticks_per_second.max(1),
        };
        Self {
            time: TimeSimConfig::for_fixed_rate(fixed.ticks_per_second),
            fixed,
        }
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self::with_fixed_ticks_per_second(FixedStepConfig::default().ticks_per_second)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimTick {
    pub index: u64,
    pub delta: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubSystemId {
    Ecology,
    Power,
    Fluid,
    Fire,
    Farming,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimRegion {
    pub center_atlas: AtlasCoord,
    pub atlas_area: AtlasArea,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimInput {
    Time(TimeSimInput),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimInputBundle {
    pub time: Option<TimeSimBundleInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationResult {
    pub subsystem: SubSystemId,
    pub tick: SimTick,
    pub calendar_advance: Option<CalendarAdvance>,
    pub world_edits: Vec<WorldEdit>,
    pub dirty_chunks: Vec<ChunkCoord>,
    pub events: Vec<SimEvent>,
    pub followups: Vec<SimFollowupRequest>,
}

impl SimulationResult {
    pub fn empty(subsystem: SubSystemId, tick: SimTick) -> Self {
        Self {
            subsystem,
            tick,
            calendar_advance: None,
            world_edits: Vec::new(),
            dirty_chunks: Vec::new(),
            events: Vec::new(),
            followups: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimEvent {
    FixedTickAdvanced { tick: u64 },
    CalendarMinuteElapsed { absolute_minutes: u64 },
    DayAdvanced { day: u64 },
    WeatherUpdated { coord: AtlasCoord, kind: LocalWeatherKind },
    DeferredSeasonPatchesQueued { count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimFollowupRequest {
    PersistCalendarState,
}

pub struct SimulationCore {
    config: SimulationConfig,
    time: TimeSim,
}

impl SimulationCore {
    pub fn new(config: SimulationConfig) -> Self {
        let time = TimeSim::new(config.time.clone());
        Self { config, time }
    }

    pub fn config(&self) -> &SimulationConfig {
        &self.config
    }

    pub fn step(&self, subsystem: SubSystemId, input: SimInput) -> SimulationResult {
        match (subsystem, input) {
            (SubSystemId::Time, SimInput::Time(input)) => self.time.step(input),
            (expected, received) => panic!(
                "simulation input/subsystem mismatch: expected {:?}, received {:?}",
                expected, received
            ),
        }
    }

    pub fn step_all(
        &self,
        tick: SimTick,
        region: SimRegion,
        input: SimInputBundle,
    ) -> Vec<SimulationResult> {
        let mut results = Vec::new();

        if let Some(time) = input.time {
            results.push(self.step(
                SubSystemId::Time,
                SimInput::Time(TimeSimInput {
                    tick,
                    region,
                    world_seed: time.world_seed,
                    calendar: time.calendar,
                    cells: time.cells,
                }),
            ));
        }

        results
    }
}
