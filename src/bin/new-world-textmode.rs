use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{self, ErrorKind, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use new_world::ecs::{
    EcsRuntime, LocalPlayerEntity, Player, PlayerBody, PlayerInventory, PlayerPhysicsState,
    Transform, Velocity,
};
use new_world::simulation::{
    EcologySimBundleInput, EcologySimChunkInput, SimEcologyEvent, SimEvent, SimInputBundle,
    SimPlantGrowthStage, SimPlantKind, SimRegion, SimSpatialScope, SimSpecies, SimTick,
    SimulationConfig, SimulationCore, SimulationResult, TimeSimBundleInput, TimeSimCellInput,
};
use new_world::world::{
    AtlasArea, AtlasCoord, BlockRegistry, CHUNK_EDGE_I32, ChunkCoord, ChunkData, LocalWeatherKind,
    LocalWeatherState, SeasonalPhase, SurfaceCondition, SurfaceConditionKind, WorldCalendar,
    WorldCore, WorldEdit, WorldMeta, atlas_coord_for_chunk,
    generation::{
        GraphBiomeKind, HydrologyConfig, MacroMapConfig, VoronoiGraphConfig,
        VoronoiGraphPatchRequest, WorldPlanePoint, apply_headwater_source_hydration_to_biomes,
        generate_macro_map, generate_voronoi_graph_patch, solve_hydrology,
    },
};

const DEFAULT_SEED: u64 = 42;
const DEFAULT_TICKS_PER_SECOND: u32 = 20;
const DAYS_PER_YEAR: u32 = 360;
const DAYS_PER_MONTH: u32 = 30;
const GRID_CELL_WIDTH: usize = 56;
const GRID_LABEL_WIDTH: usize = 8;

fn main() -> Result<(), Box<dyn Error>> {
    let config = TextModeConfig::parse(env::args().skip(1).collect())?;
    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );
    let meta = WorldMeta::new(config.seed);
    let mut world = WorldCore::new(meta, block_registry);
    let mut ecs = EcsRuntime::new();
    spawn_textmode_player(&mut ecs, config.center_chunk);

    let sim_config = SimulationConfig::with_fixed_ticks_per_second(config.ticks_per_second);
    let simulation = SimulationCore::new(sim_config);
    let tick_delta = Duration::from_secs_f64(1.0 / f64::from(config.ticks_per_second.max(1)));
    let initial_chunks = ecs.active_chunk_observer_scope().chunks;
    let graph_biomes = TextModeGraphBiomes::build(world.meta(), &initial_chunks);

    println!("new-world-textmode: press Ctrl+C to exit");

    let mut second = 0;
    while config.should_run_second(second) {
        let mut events = Vec::new();
        let mut world_updates = ChunkUpdateLog::default();

        for _ in 0..config.ticks_per_second {
            ecs.run_fixed_update();
            let active_region = ecs.active_sim_region();
            let active_chunks = ecs.active_chunk_observer_scope();
            realize_active_chunks(&mut world, &active_chunks.chunks, &mut world_updates);

            let tick = SimTick {
                index: ecs.sim_clock().tick_index,
                delta: tick_delta,
            };
            let region = SimRegion {
                center_atlas: active_region.center_atlas,
                atlas_area: active_region.area,
            };
            let input = SimInputBundle {
                ecology: Some(build_ecology_input(
                    &world,
                    &graph_biomes,
                    &active_chunks.chunks,
                )),
                time: Some(build_time_input(
                    &world,
                    active_region.center_atlas,
                    active_region.area,
                )),
                weather: None,
            };
            let results = simulation.step_all(tick, region, input);
            apply_simulation_results(
                &mut world,
                &active_chunks.chunks,
                &results,
                &mut world_updates,
            );
            events.extend(results.into_iter().flat_map(|result| result.events));
        }

        let active_chunks = ecs.active_chunk_observer_scope();
        let summary = format_second_summary(
            second + 1,
            config,
            world.calendar(),
            &world,
            &graph_biomes,
            &active_chunks.chunks,
            &events,
            &world_updates,
        );
        redraw_console(&summary);
        second = second.saturating_add(1);
        thread::sleep(Duration::from_secs(1));
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextModeConfig {
    seed: u64,
    seconds: Option<u32>,
    ticks_per_second: u32,
    center_chunk: ChunkCoord,
}

impl TextModeConfig {
    #[cfg(test)]
    fn default_for_test() -> Self {
        Self {
            seed: DEFAULT_SEED,
            seconds: Some(1),
            ticks_per_second: DEFAULT_TICKS_PER_SECOND,
            center_chunk: ChunkCoord(0, 0, 0),
        }
    }

    fn parse(mut args: Vec<String>) -> Result<Self, Box<dyn Error>> {
        let mut config = Self {
            seed: DEFAULT_SEED,
            seconds: None,
            ticks_per_second: DEFAULT_TICKS_PER_SECOND,
            center_chunk: ChunkCoord(0, 0, 0),
        };

        while let Some(flag) = args.first().cloned() {
            args.remove(0);
            match flag.as_str() {
                "--seed" => config.seed = parse_required::<u64>(&mut args, "seed")?,
                "--seconds" => config.seconds = Some(parse_required::<u32>(&mut args, "seconds")?),
                "--ticks-per-second" => {
                    config.ticks_per_second =
                        parse_required::<u32>(&mut args, "ticks-per-second")?.max(1)
                }
                "--center-chunk-x" => {
                    config.center_chunk.0 = parse_required::<i32>(&mut args, "center-chunk-x")?
                }
                "--center-chunk-y" => {
                    config.center_chunk.1 = parse_required::<i32>(&mut args, "center-chunk-y")?
                }
                "--center-chunk-z" => {
                    config.center_chunk.2 = parse_required::<i32>(&mut args, "center-chunk-z")?
                }
                "--help" | "-h" => return Err(cli_error(usage())),
                _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
            }
        }

        Ok(config)
    }

    fn should_run_second(self, elapsed_seconds: u32) -> bool {
        self.seconds
            .map(|seconds| elapsed_seconds < seconds)
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ChunkUpdateLog {
    records: HashMap<ChunkCoord, Vec<String>>,
}

impl ChunkUpdateLog {
    fn push(&mut self, coord: ChunkCoord, record: impl Into<String>) {
        self.records.entry(coord).or_default().push(record.into());
    }

    fn records_for(&self, coord: ChunkCoord) -> &[String] {
        self.records.get(&coord).map(Vec::as_slice).unwrap_or(&[])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextModeGraphBiomes {
    chunks: HashMap<ChunkCoord, GraphBiomeKind>,
}

impl TextModeGraphBiomes {
    fn build(meta: &WorldMeta, chunks: &[ChunkCoord]) -> Self {
        let center = chunks
            .get(chunks.len() / 2)
            .copied()
            .unwrap_or(ChunkCoord(0, 0, 0));
        let center_point = chunk_center_point(center);
        let graph_config = VoronoiGraphConfig::new(meta.seed, meta.generator_version);
        let request = VoronoiGraphPatchRequest::new(
            graph_config,
            center_point.x.round() as i32,
            center_point.z.round() as i32,
        );
        let patch = generate_voronoi_graph_patch(request);
        let mut macro_map = generate_macro_map(
            &patch,
            MacroMapConfig::new(meta.seed, meta.generator_version),
        );
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);

        let chunks = chunks
            .iter()
            .copied()
            .map(|coord| {
                let point = chunk_center_point(coord);
                let biome = macro_map
                    .biomes
                    .iter()
                    .filter_map(|biome| {
                        let site = patch.site(biome.site)?;
                        Some((distance_squared(site.position, point), biome.biome))
                    })
                    .min_by(|left, right| {
                        left.0
                            .partial_cmp(&right.0)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(_, biome)| biome)
                    .unwrap_or(GraphBiomeKind::TemperateGrassland);
                (coord, biome)
            })
            .collect();

        Self { chunks }
    }

    fn biome_for_chunk(&self, coord: ChunkCoord) -> GraphBiomeKind {
        self.chunks
            .get(&coord)
            .copied()
            .unwrap_or(GraphBiomeKind::TemperateGrassland)
    }
}

fn chunk_center_point(coord: ChunkCoord) -> WorldPlanePoint {
    WorldPlanePoint::new(
        (coord.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2) as f32,
        (coord.2 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2) as f32,
    )
}

fn distance_squared(left: WorldPlanePoint, right: WorldPlanePoint) -> f32 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

fn spawn_textmode_player(ecs: &mut EcsRuntime, center_chunk: ChunkCoord) {
    let center_x = center_chunk.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
    let center_y = center_chunk.1 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
    let center_z = center_chunk.2 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
    let entity = ecs
        .world_mut()
        .spawn((
            Player,
            PlayerInventory::default(),
            PlayerBody::default(),
            PlayerPhysicsState::default(),
            Transform {
                translation: [center_x as f32, center_y as f32, center_z as f32],
            },
            Velocity::default(),
        ))
        .id();
    ecs.world_mut().resource_mut::<LocalPlayerEntity>().0 = Some(entity);
}

fn realize_active_chunks(
    world: &mut WorldCore,
    chunks: &[ChunkCoord],
    updates: &mut ChunkUpdateLog,
) {
    for coord in chunks.iter().copied() {
        if world.has_chunk(coord) {
            continue;
        }
        world.insert_chunk(coord, ChunkData::new_empty(coord));
        updates.push(coord, "realized_empty_chunk");
    }
}

fn build_ecology_input(
    world: &WorldCore,
    graph_biomes: &TextModeGraphBiomes,
    chunks: &[ChunkCoord],
) -> EcologySimBundleInput {
    EcologySimBundleInput {
        world_seed: world.meta().seed,
        chunks: chunks
            .iter()
            .copied()
            .map(|coord| EcologySimChunkInput {
                coord,
                biome: graph_biomes.biome_for_chunk(coord),
            })
            .collect(),
    }
}

fn build_time_input(
    world: &WorldCore,
    center_atlas: AtlasCoord,
    area: AtlasArea,
) -> TimeSimBundleInput {
    let classes = world.resolve_region_class_area(area);
    let cells = area
        .coords()
        .map(|coord| {
            let region = classes
                .get(coord)
                .copied()
                .unwrap_or_else(|| world.sample_region_class_atlas(center_atlas));
            let current_weather = world.local_weather(coord).unwrap_or_else(|| {
                LocalWeatherState::clear(
                    coord,
                    region.climate_regime,
                    world.calendar().absolute_tick,
                    world.calendar().absolute_tick,
                )
            });
            TimeSimCellInput {
                coord,
                region,
                climate_state: world.climate_state(coord),
                current_weather,
            }
        })
        .collect();

    TimeSimBundleInput {
        world_seed: world.meta().seed,
        calendar: *world.calendar(),
        cells,
    }
}

fn apply_simulation_results(
    world: &mut WorldCore,
    active_chunks: &[ChunkCoord],
    results: &[SimulationResult],
    updates: &mut ChunkUpdateLog,
) {
    for result in results {
        if let Some(advance) = result.calendar_advance.clone() {
            let apply = world.apply_calendar_advance(advance);
            for coord in active_chunks.iter().copied().filter(|coord| {
                apply
                    .weather_changed_cells
                    .contains(&atlas_coord_for_chunk(*coord))
            }) {
                updates.push(coord, "weather_state_applied");
            }
            for coord in active_chunks.iter().copied().filter(|coord| {
                apply
                    .changed_atlas_cells
                    .contains(&atlas_coord_for_chunk(*coord))
            }) {
                updates.push(coord, "climate_or_calendar_cell_applied");
            }
            if apply.deferred_patch_count > 0 {
                for coord in active_chunks.iter().copied() {
                    updates.push(
                        coord,
                        format!("deferred_season_patches={}", apply.deferred_patch_count),
                    );
                }
            }
        }

        for edit in result.world_edits.iter().cloned() {
            let result = world.apply_edit(edit.clone());
            record_world_edit(updates, edit, &result.changed_chunks, result.applied);
        }

        for event in &result.events {
            match event {
                SimEvent::WorldUpdateRequested { scope, update } => {
                    record_scoped_update(
                        updates,
                        *scope,
                        format!("requested:{}", format_edit(update)),
                    );
                }
                SimEvent::WorldUpdateApplied {
                    scope,
                    update,
                    result,
                } => {
                    record_scoped_update(
                        updates,
                        *scope,
                        format!(
                            "applied:{}:{}",
                            format_edit(update),
                            if result.applied { "ok" } else { "failed" }
                        ),
                    );
                }
                _ => {}
            }
        }
    }
}

fn record_world_edit(
    updates: &mut ChunkUpdateLog,
    edit: WorldEdit,
    changed_chunks: &[ChunkCoord],
    applied: bool,
) {
    for coord in changed_chunks.iter().copied() {
        updates.push(
            coord,
            format!(
                "world_edit:{}:{}",
                format_edit(&edit),
                if applied { "ok" } else { "failed" }
            ),
        );
    }
}

fn record_scoped_update(updates: &mut ChunkUpdateLog, scope: SimSpatialScope, record: String) {
    if let SimSpatialScope::Chunk(coord) = scope {
        updates.push(coord, record);
    }
}

fn format_second_summary(
    second: u32,
    config: TextModeConfig,
    calendar: &WorldCalendar,
    world: &WorldCore,
    graph_biomes: &TextModeGraphBiomes,
    chunks: &[ChunkCoord],
    events: &[SimEvent],
    updates: &ChunkUpdateLog,
) -> String {
    let mut lines = Vec::new();
    lines.push("new-world-textmode  |  Ctrl+C to exit".to_string());
    lines.push(format!(
        "seed={} tick_rate={} second={} time={}",
        config.seed,
        config.ticks_per_second,
        second,
        format_calendar(*calendar)
    ));
    lines.push(String::new());
    lines.extend(format_chunk_grid(
        world,
        graph_biomes,
        chunks,
        events,
        updates,
    ));
    lines.join("\n")
}

fn format_chunk_grid(
    world: &WorldCore,
    graph_biomes: &TextModeGraphBiomes,
    chunks: &[ChunkCoord],
    events: &[SimEvent],
    updates: &ChunkUpdateLog,
) -> Vec<String> {
    let rows: Vec<Vec<Vec<String>>> = chunks
        .chunks(3)
        .map(|row| {
            let mut cells: Vec<Vec<String>> = row
                .iter()
                .copied()
                .map(|coord| format_chunk_cell(world, graph_biomes, coord, events, updates))
                .collect();
            while cells.len() < 3 {
                cells.push(Vec::new());
            }
            cells
        })
        .collect();

    let mut lines = Vec::new();
    lines.push(grid_border('┌', '┬', '┐'));
    for (row_index, row) in rows.iter().enumerate() {
        let row_height = row.iter().map(Vec::len).max().unwrap_or(0);
        for line_index in 0..row_height {
            let mut line = String::from("│");
            for cell in row {
                let text = cell.get(line_index).map(String::as_str).unwrap_or("");
                line.push_str(&pad_grid_text(text, GRID_CELL_WIDTH));
                line.push('│');
            }
            lines.push(line);
        }
        if row_index + 1 == rows.len() {
            lines.push(grid_border('└', '┴', '┘'));
        } else {
            lines.push(grid_border('├', '┼', '┤'));
        }
    }
    lines
}

fn format_chunk_cell(
    world: &WorldCore,
    graph_biomes: &TextModeGraphBiomes,
    coord: ChunkCoord,
    events: &[SimEvent],
    updates: &ChunkUpdateLog,
) -> Vec<String> {
    let observation = world.observe_chunk_surface_condition(coord);
    let biome = graph_biomes.biome_for_chunk(coord);
    let atlas = atlas_coord_for_chunk(coord);
    let weather = weather_for_chunk(world, coord);
    let ecology = join_or_none(ecology_events_for_chunk(events, coord));
    let world_updates = join_or_none(updates.records_for(coord).iter().cloned().collect());

    vec![
        format_cell_line(
            "chunk",
            format!("({:+},{:+},{:+})", coord.0, coord.1, coord.2),
        ),
        format_cell_line("atlas", format!("({:+},{:+})", atlas.x, atlas.z)),
        format_cell_line("biome", format_graph_biome(biome)),
        format_cell_line("weather", format_weather(weather)),
        format_cell_line("surface", format_surface(observation.condition)),
        format_cell_line("ecology", ecology),
        format_cell_line("updates", world_updates),
    ]
}

fn format_cell_line(label: &str, value: impl AsRef<str>) -> String {
    format!(
        "{label:<GRID_LABEL_WIDTH$}: {}",
        value.as_ref(),
        GRID_LABEL_WIDTH = GRID_LABEL_WIDTH
    )
}

fn grid_border(left: char, middle: char, right: char) -> String {
    let mut line = String::new();
    line.push(left);
    for cell_index in 0..3 {
        line.push_str(&"─".repeat(GRID_CELL_WIDTH));
        if cell_index == 2 {
            line.push(right);
        } else {
            line.push(middle);
        }
    }
    line
}

fn pad_grid_text(text: &str, width: usize) -> String {
    let truncated = truncate_for_grid(text, width.saturating_sub(1));
    let visible_len = truncated.chars().count();
    let padding = width.saturating_sub(visible_len);
    format!("{truncated}{}", " ".repeat(padding))
}

fn truncate_for_grid(text: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in text.chars().take(max_chars) {
        output.push(ch);
    }
    if text.chars().count() > max_chars && max_chars > 0 {
        output.pop();
        output.push('…');
    }
    output
}

fn weather_for_chunk(world: &WorldCore, coord: ChunkCoord) -> LocalWeatherState {
    let atlas = atlas_coord_for_chunk(coord);
    world.local_weather(atlas).unwrap_or_else(|| {
        let region = world.sample_region_class_atlas(atlas);
        LocalWeatherState::clear(
            atlas,
            region.climate_regime,
            world.calendar().absolute_tick,
            world.calendar().absolute_tick,
        )
    })
}

fn ecology_events_for_chunk(events: &[SimEvent], coord: ChunkCoord) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            SimEvent::EcologyEventObserved {
                scope: SimSpatialScope::Chunk(event_coord),
                event,
                ..
            } if *event_coord == coord => Some(format_ecology_event(*event)),
            _ => None,
        })
        .collect()
}

fn format_calendar(calendar: WorldCalendar) -> String {
    let year = calendar.day / u64::from(DAYS_PER_YEAR);
    let month = calendar.day_of_year / DAYS_PER_MONTH + 1;
    let day = calendar.day_of_year % DAYS_PER_MONTH + 1;
    format!(
        "{:02}-{:02}-{:02} {:02}:{:02} ({})",
        year % 100,
        month,
        day,
        calendar.hour,
        calendar.minute,
        format_season(calendar.season_phase)
    )
}

fn format_season(season: SeasonalPhase) -> &'static str {
    match season {
        SeasonalPhase::Spring => "spring",
        SeasonalPhase::Summer => "summer",
        SeasonalPhase::Autumn => "autumn",
        SeasonalPhase::Winter => "winter",
        SeasonalPhase::WetSeason => "wet-season",
        SeasonalPhase::DrySeason => "dry-season",
        SeasonalPhase::Thaw => "thaw",
    }
}

fn format_graph_biome(biome: GraphBiomeKind) -> String {
    format!("{biome:?}")
}

fn format_weather(weather: LocalWeatherState) -> String {
    format!(
        "{}({:.2})",
        match weather.kind {
            LocalWeatherKind::Clear => "clear",
            LocalWeatherKind::Overcast => "overcast",
            LocalWeatherKind::Rain => "rain",
            LocalWeatherKind::Snow => "snow",
            LocalWeatherKind::Storm => "storm",
        },
        weather.intensity
    )
}

fn format_surface(condition: SurfaceCondition) -> String {
    format!(
        "{}(wet={:.2},snow={:.2},thaw={:.2})",
        match condition.kind {
            SurfaceConditionKind::Dry => "dry",
            SurfaceConditionKind::Wet => "wet",
            SurfaceConditionKind::SnowCovered => "snow-covered",
            SurfaceConditionKind::HalfThawedSnow => "half-thawed-snow",
            SurfaceConditionKind::Frozen => "frozen",
        },
        condition.wetness,
        condition.snow_depth,
        condition.thaw
    )
}

fn format_ecology_event(event: SimEcologyEvent) -> String {
    match event {
        SimEcologyEvent::AnimalSpawned { species } => {
            format!("animal_spawn:{}", format_species(species))
        }
        SimEcologyEvent::AnimalFight { attacker, defender } => {
            format!(
                "animal_fight:{}>{}",
                format_species(attacker),
                format_species(defender)
            )
        }
        SimEcologyEvent::CarcassCreated { species } => {
            format!("carcass:{}", format_species(species))
        }
        SimEcologyEvent::PlantGrazed { plant, by } => {
            format!(
                "plant_grazed:{}:by_{}",
                format_plant(plant),
                format_species(by)
            )
        }
        SimEcologyEvent::PlantGrowthAdvanced { plant, stage } => {
            format!(
                "plant_growth:{}:{}",
                format_plant(plant),
                format_growth(stage)
            )
        }
    }
}

fn format_species(species: SimSpecies) -> &'static str {
    match species {
        SimSpecies::SmallHerbivore => "small_herbivore",
        SimSpecies::LargeHerbivore => "large_herbivore",
        SimSpecies::SmallPredator => "small_predator",
        SimSpecies::LargePredator => "large_predator",
        SimSpecies::Hare => "hare",
        SimSpecies::Deer => "deer",
        SimSpecies::Boar => "boar",
        SimSpecies::Fox => "fox",
        SimSpecies::Wolf => "wolf",
        SimSpecies::Bear => "bear",
        SimSpecies::WadingBird => "wading_bird",
        SimSpecies::SmallFish => "small_fish",
    }
}

fn format_plant(plant: SimPlantKind) -> &'static str {
    match plant {
        SimPlantKind::Grass => "grass",
        SimPlantKind::Reed => "reed",
        SimPlantKind::Shrub => "shrub",
        SimPlantKind::BerryBush => "berry_bush",
        SimPlantKind::Tree => "tree",
        SimPlantKind::Conifer => "conifer",
        SimPlantKind::MangroveSapling => "mangrove_sapling",
        SimPlantKind::Cactus => "cactus",
        SimPlantKind::Moss => "moss",
        SimPlantKind::Crop => "crop",
    }
}

fn format_growth(stage: SimPlantGrowthStage) -> &'static str {
    match stage {
        SimPlantGrowthStage::Seedling => "seedling",
        SimPlantGrowthStage::Growing => "growing",
        SimPlantGrowthStage::Mature => "mature",
        SimPlantGrowthStage::Dormant => "dormant",
    }
}

fn format_edit(edit: &WorldEdit) -> String {
    match edit {
        WorldEdit::SetBlock { pos, block } => {
            format!("set_block@({},{},{})={:?}", pos.0, pos.1, pos.2, block)
        }
    }
}

fn join_or_none(values: Vec<String>) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(",")
    }
}

fn redraw_console(summary: &str) {
    print!("\x1B[2J\x1B[H{summary}\n");
    let _ = io::stdout().flush();
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing {label}\n\n{}", usage())))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage: cargo run --bin new-world-textmode -- [--seed <u64>] [--seconds <u32>] [--ticks-per-second <u32>] [--center-chunk-x <i32>] [--center-chunk-y <i32>] [--center-chunk-z <i32>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_format_uses_year_month_day_hour_minute_and_season() {
        let calendar = WorldCalendar {
            day: 31,
            day_of_year: 31,
            hour: 4,
            minute: 7,
            season_phase: SeasonalPhase::Spring,
            ..WorldCalendar::default()
        };

        assert_eq!(format_calendar(calendar), "00-02-02 04:07 (spring)");
    }

    #[test]
    fn default_config_runs_until_ctrl_c() {
        let config = TextModeConfig::parse(Vec::new()).expect("default config parses");

        assert_eq!(config.seconds, None);
        assert!(config.should_run_second(0));
        assert!(config.should_run_second(1_000_000));
    }

    #[test]
    fn chunk_summary_line_includes_required_fields() {
        let registry = Arc::new(BlockRegistry::load_default().expect("default registry loads"));
        let mut world = WorldCore::new(WorldMeta::new(7), registry);
        let coord = ChunkCoord(0, 0, 0);
        world.insert_chunk(coord, ChunkData::new_empty(coord));
        let events = vec![SimEvent::EcologyEventObserved {
            scope: SimSpatialScope::Chunk(coord),
            biome: GraphBiomeKind::TemperateGrassland,
            event: SimEcologyEvent::AnimalSpawned {
                species: SimSpecies::SmallHerbivore,
            },
        }];
        let mut updates = ChunkUpdateLog::default();
        updates.push(coord, "realized_empty_chunk");
        let graph_biomes = TextModeGraphBiomes {
            chunks: HashMap::from([(coord, GraphBiomeKind::TemperateGrassland)]),
        };

        let summary = format_second_summary(
            1,
            TextModeConfig::default_for_test(),
            world.calendar(),
            &world,
            &graph_biomes,
            &[coord],
            &events,
            &updates,
        );

        assert!(summary.contains("time=00-01-01 11:00 (spring)"));
        assert!(summary.contains("┌"));
        assert!(summary.contains("biome   : "));
        assert!(summary.contains("weather : "));
        assert!(summary.contains("surface : "));
        assert!(summary.contains("animal_spawn:small_herbivore"));
        assert!(summary.contains("updates : realized_empty_chunk"));
    }
}
