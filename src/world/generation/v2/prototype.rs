use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::world::atlas::{AtlasCell, CoastalContext, HydrologyContext, RegionArchetype, RegionClassCell, TerrainFormFamily};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};
use crate::world::meta::WorldMeta;

use super::super::{SEA_LEVEL_Y, WORLD_FLOOR_Y};
use super::continuity::{BorderAnchorPoint, GenerationTileBounds};
use super::{
    ChunkCorridorWindow, ChunkGenerationV2Inputs, RegionSampleWeight, RiverCorridorConstraint,
    sample_atlas_fields_fractional, sample_region_weights,
};

const MIN_BASE_HEIGHT_Y: f32 = WORLD_FLOOR_Y as f32 + 8.0;
const MAX_BASE_HEIGHT_Y: f32 = SEA_LEVEL_Y as f32 + 192.0;
const MIN_RELIEF_BUDGET: f32 = 4.0;
const MAX_RELIEF_BUDGET: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrototypePolicyFamily {
    MarineCoastalEdge,
    LowlandBasin,
    OpenPlain,
    HillCountry,
    PlateauEscarpment,
    AridPlain,
    DuneBody,
    AlpineHighRelief,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorridorMode {
    ValleySeat,
    FloodplainOpening,
    BasinOutlet,
    CoastalExit,
}

#[derive(Debug, Clone, Copy, Default)]
struct CorridorAdjustment {
    height_delta: f32,
    relief_budget_penalty: f32,
    strongest_influence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CorridorBranchKey {
    river_id: u32,
    kind: crate::world::atlas::RiverPathKind,
    order: u8,
}

#[derive(Debug, Clone, Copy)]
struct CorridorBranchResponse {
    key: CorridorBranchKey,
    accumulator: CorridorBranchAccumulator,
}

#[derive(Debug, Clone, Copy, Default)]
struct CorridorBranchAccumulator {
    weighted_height_delta: f32,
    weighted_relief_budget_penalty: f32,
    total_weight: f32,
    strongest_influence: f32,
    max_relief_budget_penalty: f32,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedCorridorPoint {
    distance_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WorldCorridorSegmentKey {
    river_id: u32,
    kind_rank: u8,
    order: u8,
    start_x_bits: u32,
    start_z_bits: u32,
    end_x_bits: u32,
    end_z_bits: u32,
}

#[derive(Debug, Clone, Copy)]
struct WorldCorridorConstraint {
    river_id: u32,
    kind: crate::world::atlas::RiverPathKind,
    order: u8,
    start_x: f32,
    start_z: f32,
    end_x: f32,
    end_z: f32,
    half_width_blocks: f32,
    downstream_grade_per_block: f32,
}

#[derive(Debug, Clone)]
struct CachedChunkSolveState {
    inputs: ChunkGenerationV2Inputs,
    corridor_window: ChunkCorridorWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CorridorNeighborhoodKey {
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius_chunks: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PrototypeChunkStateCacheKey {
    seed: u64,
    world_version: u32,
    generator_version: u32,
    save_format_version: u32,
    chunk_x: i32,
    chunk_y: i32,
    chunk_z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PrototypeTileCorridorCacheKey {
    meta: PrototypeChunkStateCacheKey,
    tile_x: i32,
    tile_z: i32,
}

struct PrototypeSolveCache {
    meta: WorldMeta,
    chunk_states: HashMap<ChunkCoord, CachedChunkSolveState>,
    neighborhood_corridors: HashMap<CorridorNeighborhoodKey, Vec<WorldCorridorConstraint>>,
}

fn global_chunk_state_cache(
) -> &'static Mutex<HashMap<PrototypeChunkStateCacheKey, CachedChunkSolveState>> {
    static CACHE: OnceLock<Mutex<HashMap<PrototypeChunkStateCacheKey, CachedChunkSolveState>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn global_tile_corridor_cache(
) -> &'static Mutex<HashMap<PrototypeTileCorridorCacheKey, Vec<WorldCorridorConstraint>>> {
    static CACHE: OnceLock<Mutex<HashMap<PrototypeTileCorridorCacheKey, Vec<WorldCorridorConstraint>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrototypeColumn {
    pub base_height: f32,
    pub relief_budget: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseHeightfieldPrototype {
    pub chunk: ChunkCoord,
    pub columns: Vec<PrototypeColumn>,
}

pub fn empty_base_heightfield_prototype(chunk: ChunkCoord) -> BaseHeightfieldPrototype {
    BaseHeightfieldPrototype {
        chunk,
        columns: Vec::new(),
    }
}

pub fn build_chunk_base_heightfield_prototype(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
) -> BaseHeightfieldPrototype {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    let tile_bounds = GenerationTileBounds::for_chunk(chunk);
    let mut solve_cache = PrototypeSolveCache::new(inputs, corridor_window);
    let tile_corridors = solve_cache.collect_tile_corridors(tile_bounds);
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity((CHUNK_EDGE_I32 as usize) * (CHUNK_EDGE_I32 as usize));

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let sample_world_x = world_x as f32 + 0.5;
            let sample_world_z = world_z as f32 + 0.5;
            let field = sample_atlas_fields_fractional(&inputs.atlas_fields, sample_world_x, sample_world_z);
            let region_samples = sample_region_weights(&inputs.region_classes, sample_world_x, sample_world_z);
            let (mut base_height, corridor_adjustment) = raw_prototype_height_for_column(
                field,
                &region_samples,
                sample_world_x,
                sample_world_z,
                &tile_corridors,
            );
            base_height = apply_chunk_edge_continuity_blend(
                base_height,
                world_x,
                world_z,
                tile_bounds,
                &tile_corridors,
                &mut solve_cache,
            );
            base_height = apply_border_anchor_blend(
                base_height,
                world_x,
                world_z,
                tile_bounds,
                &mut solve_cache,
            );
            base_height = base_height.clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y);

            let mut relief_budget = blended_relief_budget(
                &region_samples,
                field,
                corridor_adjustment.strongest_influence,
                corridor_adjustment.relief_budget_penalty,
            );
            let anchor_strength = border_anchor_strength(world_x, world_z, tile_bounds);
            if anchor_strength > 0.0 {
                relief_budget = (relief_budget - anchor_strength * 2.0)
                    .clamp(MIN_RELIEF_BUDGET, MAX_RELIEF_BUDGET);
            }

            columns.push(PrototypeColumn {
                base_height,
                relief_budget,
            });
        }
    }

    BaseHeightfieldPrototype { chunk, columns }
}

impl PrototypeSolveCache {
    fn new(inputs: &ChunkGenerationV2Inputs, corridor_window: &ChunkCorridorWindow) -> Self {
        let mut chunk_states = HashMap::new();
        chunk_states.insert(
            inputs.chunk,
            CachedChunkSolveState {
                inputs: inputs.clone(),
                corridor_window: corridor_window.clone(),
            },
        );

        Self {
            meta: inputs.meta,
            chunk_states,
            neighborhood_corridors: HashMap::new(),
        }
    }

    fn chunk_state(&mut self, chunk: ChunkCoord) -> &CachedChunkSolveState {
        if !self.chunk_states.contains_key(&chunk) {
            let cache_key = prototype_chunk_state_cache_key(self.meta, chunk);
            let cached = global_chunk_state_cache()
                .lock()
                .expect("prototype chunk-state cache mutex must not be poisoned")
                .get(&cache_key)
                .cloned();
            let state = if let Some(cached) = cached {
                cached
            } else {
                let inputs = super::prepare_chunk_v2_inputs(chunk, &self.meta);
                let corridor_window = super::build_chunk_corridor_window(chunk, &inputs);
                let state = CachedChunkSolveState {
                    inputs,
                    corridor_window,
                };
                global_chunk_state_cache()
                    .lock()
                    .expect("prototype chunk-state cache mutex must not be poisoned")
                    .insert(cache_key, state.clone());
                state
            };
            self.chunk_states.insert(chunk, state);
        }

        self.chunk_states
            .get(&chunk)
            .expect("prototype chunk-state cache entry must exist")
    }

    fn collect_tile_corridors(&mut self, bounds: GenerationTileBounds) -> Vec<WorldCorridorConstraint> {
        let tile_cache_key = prototype_tile_corridor_cache_key(self.meta, bounds);
        if let Some(cached) = global_tile_corridor_cache()
            .lock()
            .expect("prototype tile-corridor cache mutex must not be poisoned")
            .get(&tile_cache_key)
            .cloned()
        {
            return cached;
        }

        let mut corridors = HashMap::<WorldCorridorSegmentKey, WorldCorridorConstraint>::new();

        for chunk in bounds.solve_chunks() {
            let state = self.chunk_state(chunk);
            for corridor in &state.corridor_window.corridors {
                let world_corridor = world_corridor_from_chunk(chunk, *corridor);
                corridors
                    .entry(world_corridor.segment_key())
                    .or_insert(world_corridor);
            }
        }

        let mut values = corridors.into_values().collect::<Vec<_>>();
        values.sort_by(|left, right| {
            left.river_id
                .cmp(&right.river_id)
                .then_with(|| left.order.cmp(&right.order))
                .then_with(|| left.start_x.total_cmp(&right.start_x))
                .then_with(|| left.start_z.total_cmp(&right.start_z))
                .then_with(|| left.end_x.total_cmp(&right.end_x))
                .then_with(|| left.end_z.total_cmp(&right.end_z))
        });
        global_tile_corridor_cache()
            .lock()
            .expect("prototype tile-corridor cache mutex must not be poisoned")
            .insert(tile_cache_key, values.clone());
        values
    }

    fn neighborhood_corridors(
        &mut self,
        center_chunk: ChunkCoord,
        radius_chunks: i32,
    ) -> &[WorldCorridorConstraint] {
        let key = CorridorNeighborhoodKey {
            center_chunk_x: center_chunk.0,
            center_chunk_z: center_chunk.2,
            radius_chunks,
        };

        if !self.neighborhood_corridors.contains_key(&key) {
            let mut corridors = HashMap::<WorldCorridorSegmentKey, WorldCorridorConstraint>::new();

            for chunk_z in center_chunk.2 - radius_chunks..=center_chunk.2 + radius_chunks {
                for chunk_x in center_chunk.0 - radius_chunks..=center_chunk.0 + radius_chunks {
                    let chunk = ChunkCoord(chunk_x, center_chunk.1, chunk_z);
                    let state = self.chunk_state(chunk);
                    for corridor in &state.corridor_window.corridors {
                        let world_corridor = world_corridor_from_chunk(chunk, *corridor);
                        corridors
                            .entry(world_corridor.segment_key())
                            .or_insert(world_corridor);
                    }
                }
            }

            let mut values = corridors.into_values().collect::<Vec<_>>();
            values.sort_by(|left, right| {
                left.river_id
                    .cmp(&right.river_id)
                    .then_with(|| left.order.cmp(&right.order))
                    .then_with(|| left.start_x.total_cmp(&right.start_x))
                    .then_with(|| left.start_z.total_cmp(&right.start_z))
                    .then_with(|| left.end_x.total_cmp(&right.end_x))
                    .then_with(|| left.end_z.total_cmp(&right.end_z))
            });
            self.neighborhood_corridors.insert(key, values);
        }

        self.neighborhood_corridors
            .get(&key)
            .expect("corridor neighborhood cache entry must exist")
    }

    fn sample_field_and_regions(
        &mut self,
        sample_world_x: f32,
        sample_world_z: f32,
    ) -> (AtlasCell, [RegionSampleWeight; 4]) {
        let owner_chunk = owner_chunk_for_world_sample(sample_world_x, sample_world_z);
        let state = self.chunk_state(owner_chunk);
        let field = sample_atlas_fields_fractional(&state.inputs.atlas_fields, sample_world_x, sample_world_z);
        let region_samples = sample_region_weights(&state.inputs.region_classes, sample_world_x, sample_world_z);

        (field, region_samples)
    }
}

impl WorldCorridorConstraint {
    fn segment_key(self) -> WorldCorridorSegmentKey {
        WorldCorridorSegmentKey {
            river_id: self.river_id,
            kind_rank: corridor_kind_rank(self.kind),
            order: self.order,
            start_x_bits: self.start_x.to_bits(),
            start_z_bits: self.start_z.to_bits(),
            end_x_bits: self.end_x.to_bits(),
            end_z_bits: self.end_z.to_bits(),
        }
    }
}

fn corridor_kind_rank(kind: crate::world::atlas::RiverPathKind) -> u8 {
    match kind {
        crate::world::atlas::RiverPathKind::Trunk => 0,
        crate::world::atlas::RiverPathKind::Tributary => 1,
    }
}

fn world_corridor_from_chunk(
    chunk: ChunkCoord,
    corridor: RiverCorridorConstraint,
) -> WorldCorridorConstraint {
    let origin_x = chunk.0 as f32 * CHUNK_EDGE_I32 as f32;
    let origin_z = chunk.2 as f32 * CHUNK_EDGE_I32 as f32;

    WorldCorridorConstraint {
        river_id: corridor.river_id,
        kind: corridor.kind,
        order: corridor.order,
        start_x: origin_x + corridor.start_x,
        start_z: origin_z + corridor.start_z,
        end_x: origin_x + corridor.end_x,
        end_z: origin_z + corridor.end_z,
        half_width_blocks: corridor.half_width_blocks,
        downstream_grade_per_block: corridor.downstream_grade_per_block,
    }
}

fn owner_chunk_for_world_sample(sample_world_x: f32, sample_world_z: f32) -> ChunkCoord {
    ChunkCoord(
        (sample_world_x.floor() as i32).div_euclid(CHUNK_EDGE_I32),
        0,
        (sample_world_z.floor() as i32).div_euclid(CHUNK_EDGE_I32),
    )
}

fn prototype_chunk_state_cache_key(meta: WorldMeta, chunk: ChunkCoord) -> PrototypeChunkStateCacheKey {
    PrototypeChunkStateCacheKey {
        seed: meta.seed,
        world_version: meta.world_version,
        generator_version: meta.generator_version,
        save_format_version: meta.save_format_version,
        chunk_x: chunk.0,
        chunk_y: chunk.1,
        chunk_z: chunk.2,
    }
}

fn prototype_tile_corridor_cache_key(
    meta: WorldMeta,
    bounds: GenerationTileBounds,
) -> PrototypeTileCorridorCacheKey {
    PrototypeTileCorridorCacheKey {
        meta: PrototypeChunkStateCacheKey {
            seed: meta.seed,
            world_version: meta.world_version,
            generator_version: meta.generator_version,
            save_format_version: meta.save_format_version,
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
        },
        tile_x: bounds.coord.x,
        tile_z: bounds.coord.z,
    }
}

fn border_anchor_strength(world_x: i32, world_z: i32, tile_bounds: GenerationTileBounds) -> f32 {
    tile_bounds
        .anchors_for_world_column(world_x, world_z)
        .iter()
        .map(|anchor| anchor.strength)
        .sum::<f32>()
        .clamp(0.0, 1.0)
}

fn apply_border_anchor_blend(
    base_height: f32,
    world_x: i32,
    world_z: i32,
    tile_bounds: GenerationTileBounds,
    solve_cache: &mut PrototypeSolveCache,
) -> f32 {
    let anchors = tile_bounds.anchors_for_world_column(world_x, world_z);
    let mut weighted_height = 0.0;
    let mut total_weight = 0.0;

    for anchor in anchors.iter() {
        let target_height = sample_border_anchor_height(anchor, solve_cache);
        weighted_height += target_height * anchor.strength;
        total_weight += anchor.strength;
    }

    if total_weight <= f32::EPSILON {
        return base_height;
    }

    let anchor_target = weighted_height / total_weight;
    let local_x = world_x.rem_euclid(CHUNK_EDGE_I32);
    let local_z = world_z.rem_euclid(CHUNK_EDGE_I32);
    let on_chunk_x_edge = local_x == 0 || local_x == CHUNK_EDGE_I32 - 1;
    let on_chunk_z_edge = local_z == 0 || local_z == CHUNK_EDGE_I32 - 1;
    let edge_scale = if on_chunk_x_edge && on_chunk_z_edge {
        0.0
    } else if on_chunk_x_edge || on_chunk_z_edge {
        0.45
    } else {
        1.0
    };
    let blend = total_weight.clamp(0.0, 1.0) * edge_scale;

    if blend <= f32::EPSILON {
        return base_height;
    }

    base_height + (anchor_target - base_height) * blend
}

fn sample_border_anchor_height(
    anchor: BorderAnchorPoint,
    solve_cache: &mut PrototypeSolveCache,
) -> f32 {
    let sample_world_x = anchor.sample_world_x;
    let sample_world_z = anchor.sample_world_z;
    let (field, region_samples) = solve_cache.sample_field_and_regions(sample_world_x, sample_world_z);
    let owner_chunk = owner_chunk_for_world_sample(sample_world_x, sample_world_z);
    let corridors = solve_cache.neighborhood_corridors(owner_chunk, 0);
    let base_height = blended_base_height(&region_samples, field, sample_world_x, sample_world_z);
    let corridor_adjustment = corridor_adjustment_for_world_column(
        field,
        sample_world_x,
        sample_world_z,
        &region_samples,
        corridors,
    );

    (base_height + corridor_adjustment.height_delta).clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn raw_prototype_height_for_column(
    field: AtlasCell,
    region_samples: &[RegionSampleWeight; 4],
    sample_world_x: f32,
    sample_world_z: f32,
    tile_corridors: &[WorldCorridorConstraint],
) -> (f32, CorridorAdjustment) {
    let mut base_height = blended_base_height(region_samples, field, sample_world_x, sample_world_z);
    let corridor_adjustment = corridor_adjustment_for_world_column(
        field,
        sample_world_x,
        sample_world_z,
        region_samples,
        tile_corridors,
    );
    base_height += corridor_adjustment.height_delta;
    (base_height, corridor_adjustment)
}

fn apply_chunk_edge_continuity_blend(
    base_height: f32,
    world_x: i32,
    world_z: i32,
    tile_bounds: GenerationTileBounds,
    tile_corridors: &[WorldCorridorConstraint],
    solve_cache: &mut PrototypeSolveCache,
) -> f32 {
    let local_x = world_x.rem_euclid(CHUNK_EDGE_I32);
    let local_z = world_z.rem_euclid(CHUNK_EDGE_I32);
    let mut target_sum = 0.0;
    let mut total_weight = 0.0;

    if local_x == CHUNK_EDGE_I32 - 1 && world_x + 1 < tile_bounds.core_max_world_x_exclusive() {
        let boundary_target =
            seam_pair_average(world_x, world_z, world_x + 1, world_z, tile_corridors, solve_cache);
        target_sum += boundary_target;
        total_weight += 1.0;
    } else if local_x == CHUNK_EDGE_I32 - 2 && world_x + 2 < tile_bounds.core_max_world_x_exclusive() {
        let boundary_target =
            seam_pair_average(world_x + 1, world_z, world_x + 2, world_z, tile_corridors, solve_cache);
        target_sum += boundary_target * 0.7;
        total_weight += 0.7;
    } else if local_x == 0 && world_x > tile_bounds.core_min_world_x() {
        let boundary_target =
            seam_pair_average(world_x - 1, world_z, world_x, world_z, tile_corridors, solve_cache);
        target_sum += boundary_target;
        total_weight += 1.0;
    } else if local_x == 1 && world_x - 1 > tile_bounds.core_min_world_x() {
        let boundary_target =
            seam_pair_average(world_x - 2, world_z, world_x - 1, world_z, tile_corridors, solve_cache);
        target_sum += boundary_target * 0.7;
        total_weight += 0.7;
    }

    if local_z == CHUNK_EDGE_I32 - 1 && world_z + 1 < tile_bounds.core_max_world_z_exclusive() {
        let boundary_target =
            seam_pair_average(world_x, world_z, world_x, world_z + 1, tile_corridors, solve_cache);
        target_sum += boundary_target;
        total_weight += 1.0;
    } else if local_z == CHUNK_EDGE_I32 - 2 && world_z + 2 < tile_bounds.core_max_world_z_exclusive() {
        let boundary_target =
            seam_pair_average(world_x, world_z + 1, world_x, world_z + 2, tile_corridors, solve_cache);
        target_sum += boundary_target * 0.7;
        total_weight += 0.7;
    } else if local_z == 0 && world_z > tile_bounds.core_min_world_z() {
        let boundary_target =
            seam_pair_average(world_x, world_z - 1, world_x, world_z, tile_corridors, solve_cache);
        target_sum += boundary_target;
        total_weight += 1.0;
    } else if local_z == 1 && world_z - 1 > tile_bounds.core_min_world_z() {
        let boundary_target =
            seam_pair_average(world_x, world_z - 2, world_x, world_z - 1, tile_corridors, solve_cache);
        target_sum += boundary_target * 0.7;
        total_weight += 0.7;
    }

    if total_weight <= f32::EPSILON {
        return base_height;
    }

    let target = target_sum / total_weight;
    base_height + (target - base_height) * total_weight.clamp(0.0, 1.0)
}

fn sample_raw_height_at_world_column(
    world_x: i32,
    world_z: i32,
    tile_corridors: &[WorldCorridorConstraint],
    solve_cache: &mut PrototypeSolveCache,
) -> f32 {
    let sample_world_x = world_x as f32 + 0.5;
    let sample_world_z = world_z as f32 + 0.5;
    let (field, region_samples) = solve_cache.sample_field_and_regions(sample_world_x, sample_world_z);
    let (raw_height, _) = raw_prototype_height_for_column(
        field,
        &region_samples,
        sample_world_x,
        sample_world_z,
        tile_corridors,
    );
    raw_height.clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn seam_pair_average(
    ax: i32,
    az: i32,
    bx: i32,
    bz: i32,
    tile_corridors: &[WorldCorridorConstraint],
    solve_cache: &mut PrototypeSolveCache,
) -> f32 {
    let a = sample_raw_height_at_world_column(ax, az, tile_corridors, solve_cache);
    let b = sample_raw_height_at_world_column(bx, bz, tile_corridors, solve_cache);
    0.5 * (a + b)
}

fn blended_base_height(
    region_samples: &[RegionSampleWeight; 4],
    field: AtlasCell,
    world_x: f32,
    world_z: f32,
) -> f32 {
    let mut base_height = 0.0;

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }
        base_height += family_base_height(
            prototype_policy_family(sample.cell),
            field,
            sample.cell,
            world_x,
            world_z,
        ) * sample.weight;
    }

    base_height.clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn blended_relief_budget(
    region_samples: &[RegionSampleWeight; 4],
    field: AtlasCell,
    strongest_corridor_influence: f32,
    corridor_penalty: f32,
) -> f32 {
    let mut relief_budget = 0.0;

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }
        relief_budget += family_relief_budget(
            prototype_policy_family(sample.cell),
            field,
            sample.cell,
            strongest_corridor_influence,
            corridor_penalty,
        ) * sample.weight;
    }

    relief_budget.clamp(MIN_RELIEF_BUDGET, MAX_RELIEF_BUDGET)
}

fn prototype_policy_family(region: RegionClassCell) -> PrototypePolicyFamily {
    match region.archetype {
        RegionArchetype::OceanicShelf
        | RegionArchetype::SandyBeachPlain
        | RegionArchetype::CoastalCliffland
        | RegionArchetype::RockyShoreCoast
        | RegionArchetype::BarrierCoast
        | RegionArchetype::LagoonCoast
        | RegionArchetype::EstuaryLowland
        | RegionArchetype::CoastalDelta
        | RegionArchetype::MangroveLagoon
        | RegionArchetype::MangroveDelta
        | RegionArchetype::MonsoonDelta
        | RegionArchetype::FjordCoast => PrototypePolicyFamily::MarineCoastalEdge,
        RegionArchetype::ColdWetLowland
        | RegionArchetype::TundraPlain
        | RegionArchetype::TropicalRainforestLowland
        | RegionArchetype::MarshFloodplain
        | RegionArchetype::SwampLowland
        | RegionArchetype::FloodedForestAlluvialLowland
        | RegionArchetype::FloodedForestFloodplain
        | RegionArchetype::TemperateBasin
        | RegionArchetype::TemperateBroadValley
        | RegionArchetype::BorealWetLowland
        | RegionArchetype::DesertBasin
        | RegionArchetype::MonsoonFloodplain
        | RegionArchetype::GlacialValley => PrototypePolicyFamily::LowlandBasin,
        RegionArchetype::TemperatePlain
        | RegionArchetype::SavannaPlain
        | RegionArchetype::TemperateRollingPlain
        | RegionArchetype::TemperateBroadleafPlain
        | RegionArchetype::BorealPlain
        | RegionArchetype::PolarBarrensPlain => PrototypePolicyFamily::OpenPlain,
        RegionArchetype::TemperateHills
        | RegionArchetype::TropicalRainforestHills
        | RegionArchetype::SteppeHills
        | RegionArchetype::TemperateMixedHills
        | RegionArchetype::BorealHills
        | RegionArchetype::MediterraneanShrublandHills
        | RegionArchetype::SavannaHills
        | RegionArchetype::TropicalDryForestHills
        | RegionArchetype::SubalpineWoodedFront
        | RegionArchetype::DesertAlluvialFan
        | RegionArchetype::BorealRidgeCountry
        | RegionArchetype::AlpineRavineCountry => PrototypePolicyFamily::HillCountry,
        RegionArchetype::TemperatePlateau
        | RegionArchetype::TemperateEscarpmentUpland
        | RegionArchetype::MonsoonPlateau
        | RegionArchetype::DesertMesaCountry => PrototypePolicyFamily::PlateauEscarpment,
        RegionArchetype::SteppePlain
        | RegionArchetype::DesertPlain
        | RegionArchetype::SemiDesertPediment
        | RegionArchetype::DryShrublandBadlands
        | RegionArchetype::DryShrublandKarst => PrototypePolicyFamily::AridPlain,
        RegionArchetype::DesertDuneField => PrototypePolicyFamily::DuneBody,
        RegionArchetype::GlaciatedAlpine
        | RegionArchetype::AlpineMeadowMountain
        | RegionArchetype::CrevassedIcefield => PrototypePolicyFamily::AlpineHighRelief,
    }
}

fn family_base_height(
    family: PrototypePolicyFamily,
    field: AtlasCell,
    region: RegionClassCell,
    world_x: f32,
    world_z: f32,
) -> f32 {
    let macro_base = macro_elevation_to_world_y(field.macro_elevation);
    let terrain_bias = terrain_form_height_bias(region.terrain_form_family, field);

    let family_height = match family {
        PrototypePolicyFamily::MarineCoastalEdge => {
            let shelf_wave = low_frequency_wave(world_x, world_z, 160.0, 144.0, 0.35) * 4.0;
            let edge_bias = match region.terrain_form_family {
                TerrainFormFamily::SeaCliff
                | TerrainFormFamily::RockyShore
                | TerrainFormFamily::FjordCoast => {
                    12.0 + field.ruggedness * 20.0 + field.slope * 16.0
                }
                TerrainFormFamily::BeachPlain
                | TerrainFormFamily::BarrierCoast
                | TerrainFormFamily::LagoonCoast
                | TerrainFormFamily::MarineShelf => -5.0 - field.slope * 6.0,
                _ => 0.0,
            };
            SEA_LEVEL_Y as f32 - 8.0 + field.macro_elevation * 62.0 + shelf_wave + edge_bias
        }
        PrototypePolicyFamily::LowlandBasin => {
            macro_base - 8.0 - field.basinness * 15.0 + field.wetness * 6.0
                + low_frequency_wave(world_x, world_z, 112.0, 104.0, 0.9) * 3.0
                - field.slope * 5.0
        }
        PrototypePolicyFamily::OpenPlain => {
            macro_base
                + low_frequency_wave(world_x, world_z, 128.0, 120.0, 0.6) * 6.0
                + field.continent_core_factor * 5.0
                + field.ruggedness * 5.0
                - field.basinness * 3.0
        }
        PrototypePolicyFamily::HillCountry => {
            macro_base
                + low_frequency_wave(world_x, world_z, 96.0, 104.0, 0.4) * 10.0
                + field.ruggedness * 18.0
                + field.slope * 11.0
                + field.mountain_mass * 14.0
        }
        PrototypePolicyFamily::PlateauEscarpment => {
            macro_base
                + 12.0
                + low_frequency_wave(world_x, world_z, 144.0, 112.0, 1.2) * 5.0
                + field.continent_core_factor * 9.0
                + field.mountain_mass * 6.0
                - field.basinness * 4.0
        }
        PrototypePolicyFamily::AridPlain => {
            macro_base
                + low_frequency_wave(world_x, world_z, 136.0, 124.0, 0.75) * 8.0
                + field.aridity * 10.0
                + field.inlandness * 4.0
                - field.wetness * 4.0
                + field.slope * 3.0
        }
        PrototypePolicyFamily::DuneBody => {
            macro_base
                + dune_body_wave(world_x, world_z) * 14.0
                + low_frequency_wave(world_x, world_z, 168.0, 152.0, 0.15) * 4.0
                + field.aridity * 12.0
                - field.wetness * 4.0
                - field.basinness * 2.0
        }
        PrototypePolicyFamily::AlpineHighRelief => {
            macro_base
                + low_frequency_wave(world_x, world_z, 88.0, 92.0, 0.55) * 8.0
                + field.mountain_mass * 22.0
                + field.alpine_factor * 28.0
                + field.ruggedness * 20.0
                + field.slope * 10.0
                + field.polar_factor * 6.0
        }
    };
    let micro_relief = family_micro_relief(family, field, region, world_x, world_z);

    (family_height + terrain_bias + micro_relief).clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn terrain_form_height_bias(terrain_form: TerrainFormFamily, field: AtlasCell) -> f32 {
    match terrain_form {
        TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::EstuaryLowland => -4.0 - field.basinness * 4.0,
        TerrainFormFamily::Basin => -9.0 - field.basinness * 6.0,
        TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley
        | TerrainFormFamily::GlacialValley => -6.0,
        TerrainFormFamily::Plateau | TerrainFormFamily::MesaCountry | TerrainFormFamily::Escarpment => 7.0,
        TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::Canyon
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield => 10.0,
        TerrainFormFamily::AlluvialFan => 4.0,
        TerrainFormFamily::DuneField => 5.0,
        TerrainFormFamily::SeaCliff | TerrainFormFamily::RockyShore | TerrainFormFamily::FjordCoast => 8.0,
        TerrainFormFamily::MarineShelf
        | TerrainFormFamily::BeachPlain
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::LagoonCoast => -2.0,
        _ => 0.0,
    }
}

fn family_micro_relief(
    family: PrototypePolicyFamily,
    field: AtlasCell,
    region: RegionClassCell,
    world_x: f32,
    world_z: f32,
) -> f32 {
    let amplitude = match family {
        PrototypePolicyFamily::MarineCoastalEdge => 2.2,
        PrototypePolicyFamily::LowlandBasin => 3.8,
        PrototypePolicyFamily::OpenPlain => 5.2,
        PrototypePolicyFamily::HillCountry => 6.0,
        PrototypePolicyFamily::PlateauEscarpment => 5.2,
        PrototypePolicyFamily::AridPlain => 5.0,
        PrototypePolicyFamily::DuneBody => 3.6,
        PrototypePolicyFamily::AlpineHighRelief => 6.4,
    };
    let terrain_scale = (0.52
        + field.ruggedness * 0.58
        + field.slope * 0.34
        + field.mountain_mass * 0.18
        + field.aridity * 0.18
        - field.wetness * 0.10
        - field.riverine_factor * 0.08)
        .clamp(0.42, 1.45);
    let terrace_scale = terrain_form_micro_relief_scale(family, region.terrain_form_family);
    let ripple_primary = mid_frequency_wave(world_x, world_z, 20.0, 16.0, 0.45);
    let ripple_secondary = mid_frequency_wave(world_x, world_z, 11.0, 9.0, 1.35);
    let terrace = terraced_wave(world_x, world_z, 28.0, 24.0, 0.25, 6.0);
    let banding = terraced_wave(world_x, world_z, 14.0, 12.0, 1.10, 5.0);

    amplitude * terrain_scale * (ripple_primary * 0.58 + ripple_secondary * 0.24)
        + amplitude * terrace_scale * (terrace * 0.46 + banding * 0.22)
}

fn terrain_form_micro_relief_scale(
    family: PrototypePolicyFamily,
    terrain_form: TerrainFormFamily,
) -> f32 {
    match terrain_form {
        TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::EstuaryLowland => 0.24,
        TerrainFormFamily::Basin => 0.20,
        TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley
        | TerrainFormFamily::GlacialValley => 0.36,
        TerrainFormFamily::Plateau
        | TerrainFormFamily::MesaCountry
        | TerrainFormFamily::Escarpment => 0.54,
        TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::Canyon
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield => 0.62,
        TerrainFormFamily::DuneField => 0.38,
        TerrainFormFamily::MarineShelf
        | TerrainFormFamily::BeachPlain
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::LagoonCoast => 0.20,
        _ => match family {
            PrototypePolicyFamily::MarineCoastalEdge => 0.22,
            PrototypePolicyFamily::LowlandBasin => 0.26,
            PrototypePolicyFamily::OpenPlain => 0.38,
            PrototypePolicyFamily::HillCountry => 0.54,
            PrototypePolicyFamily::PlateauEscarpment => 0.56,
            PrototypePolicyFamily::AridPlain => 0.46,
            PrototypePolicyFamily::DuneBody => 0.34,
            PrototypePolicyFamily::AlpineHighRelief => 0.58,
        },
    }
}

fn corridor_adjustment_for_world_column(
    field: AtlasCell,
    sample_world_x: f32,
    sample_world_z: f32,
    region_samples: &[RegionSampleWeight; 4],
    corridors: &[WorldCorridorConstraint],
) -> CorridorAdjustment {
    let mut adjustment = CorridorAdjustment::default();
    let mut strongest_influence = 0.0;

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }

        let family = prototype_policy_family(sample.cell);
        let mode = classify_corridor_mode(sample.cell);
        let mut local_adjustment = CorridorAdjustment::default();
        let mut branch_responses = Vec::<CorridorBranchResponse>::new();

        for corridor in corridors {
            let response =
                single_corridor_adjustment(family, mode, field, sample_world_x, sample_world_z, *corridor);
            if response.strongest_influence <= f32::EPSILON {
                continue;
            }
            let key = CorridorBranchKey {
                river_id: corridor.river_id,
                kind: corridor.kind,
                order: corridor.order,
            };

            if let Some(existing) = branch_responses.iter_mut().find(|entry| entry.key == key) {
                existing.accumulator.push(response);
            } else {
                let mut accumulator = CorridorBranchAccumulator::default();
                accumulator.push(response);
                branch_responses.push(CorridorBranchResponse { key, accumulator });
            }
        }

        for branch in branch_responses {
            let response = branch.accumulator.finish();
            local_adjustment.height_delta += response.height_delta;
            local_adjustment.relief_budget_penalty += response.relief_budget_penalty;
            local_adjustment.strongest_influence = local_adjustment
                .strongest_influence
                .max(response.strongest_influence);
        }

        adjustment.height_delta += local_adjustment.height_delta * sample.weight;
        adjustment.relief_budget_penalty += local_adjustment.relief_budget_penalty * sample.weight;
        strongest_influence += local_adjustment.strongest_influence * sample.weight;
    }

    adjustment.height_delta = adjustment.height_delta.max(-24.0);
    adjustment.relief_budget_penalty = adjustment.relief_budget_penalty.min(18.0);
    adjustment.strongest_influence = strongest_influence.clamp(0.0, 1.0);
    adjustment
}

impl CorridorBranchAccumulator {
    fn push(&mut self, response: CorridorAdjustment) {
        let weight = response.strongest_influence.max(0.0001);
        self.weighted_height_delta += response.height_delta * weight;
        self.weighted_relief_budget_penalty += response.relief_budget_penalty * weight;
        self.total_weight += weight;
        self.strongest_influence = self.strongest_influence.max(response.strongest_influence);
        self.max_relief_budget_penalty = self
            .max_relief_budget_penalty
            .max(response.relief_budget_penalty);
    }

    fn finish(self) -> CorridorAdjustment {
        if self.total_weight <= f32::EPSILON {
            return CorridorAdjustment::default();
        }

        CorridorAdjustment {
            height_delta: self.weighted_height_delta / self.total_weight,
            relief_budget_penalty: (self.weighted_relief_budget_penalty / self.total_weight)
                .max(self.max_relief_budget_penalty * 0.6),
            strongest_influence: self.strongest_influence,
        }
    }
}

fn single_corridor_adjustment(
    family: PrototypePolicyFamily,
    mode: CorridorMode,
    field: AtlasCell,
    sample_world_x: f32,
    sample_world_z: f32,
    corridor: WorldCorridorConstraint,
) -> CorridorAdjustment {
    let projection =
        project_point_onto_corridor_segment((sample_world_x, sample_world_z), corridor);
    let width_multiplier = match mode {
        CorridorMode::ValleySeat => 1.0,
        CorridorMode::FloodplainOpening => 1.35,
        CorridorMode::BasinOutlet => 1.2,
        CorridorMode::CoastalExit => 1.45,
    };
    let effective_half_width = (corridor.half_width_blocks * width_multiplier).max(1.0);
    let normalized = (1.0 - projection.distance_blocks / effective_half_width).clamp(0.0, 1.0);
    if normalized <= 0.0 {
        return CorridorAdjustment::default();
    }

    let influence = smoothstep(normalized);
    let base_drop = match mode {
        CorridorMode::ValleySeat => 6.0,
        CorridorMode::FloodplainOpening => 4.5,
        CorridorMode::BasinOutlet => 5.5,
        CorridorMode::CoastalExit => 4.0,
    } + corridor_depth_from_width(corridor.half_width_blocks)
        + corridor.downstream_grade_per_block * 320.0;
    let depth_scale = family_corridor_depth_scale(family);
    let drop = base_drop * influence * depth_scale * (0.92 + field.river_flow_potential * 0.16);
    let shoulder_gain = if matches!(
        family,
        PrototypePolicyFamily::HillCountry
            | PrototypePolicyFamily::PlateauEscarpment
            | PrototypePolicyFamily::AlpineHighRelief
    ) {
        influence * (1.0 - influence) * (4.0 + corridor.downstream_grade_per_block * 220.0)
    } else {
        0.0
    };
    let relief_budget_penalty = match mode {
        CorridorMode::ValleySeat => 6.0,
        CorridorMode::FloodplainOpening => 9.0,
        CorridorMode::BasinOutlet => 8.0,
        CorridorMode::CoastalExit => 7.0,
    } * influence;

    CorridorAdjustment {
        height_delta: shoulder_gain - drop,
        relief_budget_penalty,
        strongest_influence: influence,
    }
}

fn classify_corridor_mode(region: RegionClassCell) -> CorridorMode {
    if region.coastal_context != CoastalContext::Inland
        || matches!(
            region.terrain_form_family,
            TerrainFormFamily::MarineShelf
                | TerrainFormFamily::BeachPlain
                | TerrainFormFamily::BarrierCoast
                | TerrainFormFamily::LagoonCoast
                | TerrainFormFamily::EstuaryLowland
                | TerrainFormFamily::Delta
        )
    {
        CorridorMode::CoastalExit
    } else if region.hydrology_context == HydrologyContext::LakeBasin
        || matches!(region.terrain_form_family, TerrainFormFamily::Basin)
        || matches!(
            region.archetype,
            RegionArchetype::TemperateBasin | RegionArchetype::DesertBasin
        )
    {
        CorridorMode::BasinOutlet
    } else if matches!(
        region.hydrology_context,
        HydrologyContext::RiverCorridor | HydrologyContext::WetLowland
    ) || matches!(
        region.terrain_form_family,
        TerrainFormFamily::Floodplain
            | TerrainFormFamily::WetLowland
            | TerrainFormFamily::AlluvialLowland
            | TerrainFormFamily::BroadValley
            | TerrainFormFamily::GlacialValley
    ) {
        CorridorMode::FloodplainOpening
    } else {
        CorridorMode::ValleySeat
    }
}

fn family_corridor_depth_scale(family: PrototypePolicyFamily) -> f32 {
    match family {
        PrototypePolicyFamily::MarineCoastalEdge => 0.85,
        PrototypePolicyFamily::LowlandBasin => 0.75,
        PrototypePolicyFamily::OpenPlain => 0.92,
        PrototypePolicyFamily::HillCountry => 1.08,
        PrototypePolicyFamily::PlateauEscarpment => 1.12,
        PrototypePolicyFamily::AridPlain => 0.82,
        PrototypePolicyFamily::DuneBody => 0.52,
        PrototypePolicyFamily::AlpineHighRelief => 1.16,
    }
}

fn corridor_depth_from_width(half_width_blocks: f32) -> f32 {
    2.2 + half_width_blocks.max(1.0).sqrt() * 0.55
}

fn family_relief_budget(
    family: PrototypePolicyFamily,
    field: AtlasCell,
    region: RegionClassCell,
    strongest_corridor_influence: f32,
    corridor_penalty: f32,
) -> f32 {
    let base_budget = match family {
        PrototypePolicyFamily::MarineCoastalEdge => 12.0,
        PrototypePolicyFamily::LowlandBasin => 14.0,
        PrototypePolicyFamily::OpenPlain => 19.0,
        PrototypePolicyFamily::HillCountry => 24.0,
        PrototypePolicyFamily::PlateauEscarpment => 18.0,
        PrototypePolicyFamily::AridPlain => 20.0,
        PrototypePolicyFamily::DuneBody => 22.0,
        PrototypePolicyFamily::AlpineHighRelief => 16.0,
    };
    let terrain_bonus = match region.terrain_form_family {
        TerrainFormFamily::Plateau
        | TerrainFormFamily::MesaCountry
        | TerrainFormFamily::Escarpment
        | TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::Canyon
        | TerrainFormFamily::RavineCountry => 4.0,
        TerrainFormFamily::DuneField => 3.0,
        TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::Delta
        | TerrainFormFamily::Basin => -3.0,
        _ => 0.0,
    };
    let dynamic_budget = base_budget
        + field.ruggedness * 9.0
        + field.slope * 6.0
        + field.aridity * 4.0
        - field.wetness * 4.0
        - field.riverine_factor * 3.0
        + terrain_bonus
        - strongest_corridor_influence * 4.0
        - corridor_penalty;

    dynamic_budget.clamp(MIN_RELIEF_BUDGET, MAX_RELIEF_BUDGET)
}

fn macro_elevation_to_world_y(macro_elevation: f32) -> f32 {
    (SEA_LEVEL_Y as f32 - 8.0 + macro_elevation * 140.0).clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
}

fn low_frequency_wave(world_x: f32, world_z: f32, scale_x: f32, scale_z: f32, phase: f32) -> f32 {
    ((world_x / scale_x + phase).sin() + (world_z / scale_z + phase * 1.7).cos()) * 0.5
}

fn mid_frequency_wave(world_x: f32, world_z: f32, scale_x: f32, scale_z: f32, phase: f32) -> f32 {
    let primary = ((world_x / scale_x + phase).sin() + (world_z / scale_z + phase * 1.9).cos()) * 0.5;
    let cross = ((world_x / (scale_x * 0.58) - phase * 0.7).cos()
        + (world_z / (scale_z * 0.74) + phase * 1.3).sin())
        * 0.25;
    primary + cross
}

fn dune_body_wave(world_x: f32, world_z: f32) -> f32 {
    let long_wave = (world_x / 72.0 + world_z / 128.0).sin();
    let cross_wave = (world_x / 148.0 - world_z / 84.0 + 0.9).cos();
    long_wave * 0.65 + cross_wave * 0.35
}

fn terraced_wave(
    world_x: f32,
    world_z: f32,
    scale_x: f32,
    scale_z: f32,
    phase: f32,
    steps: f32,
) -> f32 {
    let raw = mid_frequency_wave(world_x, world_z, scale_x, scale_z, phase).clamp(-1.0, 1.0);
    let normalized = raw * 0.5 + 0.5;
    let terraced = (normalized * steps).floor() / steps;
    (terraced * 2.0 - 1.0) * 0.68 + raw * 0.32
}

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn project_point_onto_corridor_segment(
    point: (f32, f32),
    corridor: WorldCorridorConstraint,
) -> ProjectedCorridorPoint {
    let seg_x = corridor.end_x - corridor.start_x;
    let seg_z = corridor.end_z - corridor.start_z;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return ProjectedCorridorPoint {
            distance_blocks: distance_between_points(point, (corridor.start_x, corridor.start_z)),
        };
    }

    let t = (((point.0 - corridor.start_x) * seg_x + (point.1 - corridor.start_z) * seg_z) / length_sq)
        .clamp(0.0, 1.0);
    let projected = (
        corridor.start_x + seg_x * t,
        corridor.start_z + seg_z * t,
    );

    ProjectedCorridorPoint {
        distance_blocks: distance_between_points(point, projected),
    }
}

fn distance_between_points(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dz = a.1 - b.1;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::v2::{
        build_chunk_corridor_window, empty_chunk_corridor_window, prepare_chunk_v2_inputs,
    };
    use crate::world::meta::WorldMeta;

    #[test]
    fn base_heightfield_is_deterministic_and_emits_a_full_grid() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let a = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
        let b = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert_eq!(a.columns.len(), (CHUNK_EDGE_I32 as usize) * (CHUNK_EDGE_I32 as usize));
        assert!(a
            .columns
            .iter()
            .all(|column| column.base_height.is_finite() && column.relief_budget.is_finite()));
        assert!(a
            .columns
            .iter()
            .all(|column| column.relief_budget >= MIN_RELIEF_BUDGET));
    }

    #[test]
    fn corridor_influence_lowers_the_prototype_near_a_corridor() {
        let meta = WorldMeta::new(42);
        let (chunk, inputs, corridor_window) = chunk_with_corridor_window(&meta);
        let tile_bounds = GenerationTileBounds::for_chunk(chunk);
        let mut solve_cache = PrototypeSolveCache::new(&inputs, &corridor_window);
        let tile_corridors = solve_cache.collect_tile_corridors(tile_bounds);
        let focus = corridor_focus_index(corridor_window.corridors[0]);
        let local_x = (focus % CHUNK_EDGE_I32 as usize) as i32;
        let local_z = (focus / CHUNK_EDGE_I32 as usize) as i32;
        let sample_world_x = (chunk.0 * CHUNK_EDGE_I32 + local_x) as f32 + 0.5;
        let sample_world_z = (chunk.2 * CHUNK_EDGE_I32 + local_z) as f32 + 0.5;
        let field = sample_atlas_fields_fractional(&inputs.atlas_fields, sample_world_x, sample_world_z);
        let region_samples = sample_region_weights(&inputs.region_classes, sample_world_x, sample_world_z);
        let with_corridor = corridor_adjustment_for_world_column(
            field,
            sample_world_x,
            sample_world_z,
            &region_samples,
            &tile_corridors,
        );
        let without_corridor = corridor_adjustment_for_world_column(
            field,
            sample_world_x,
            sample_world_z,
            &region_samples,
            &[],
        );

        assert!(with_corridor.height_delta < without_corridor.height_delta);
        assert!(with_corridor.relief_budget_penalty >= without_corridor.relief_budget_penalty);
    }

    #[test]
    fn prototype_output_varies_across_the_chunk_surface() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(4, 0, -3);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let prototype = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
        let min_height = prototype
            .columns
            .iter()
            .map(|column| column.base_height)
            .fold(f32::INFINITY, f32::min);
        let max_height = prototype
            .columns
            .iter()
            .map(|column| column.base_height)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(max_height > min_height);
    }

    #[test]
    fn shared_edge_transition_stays_close_to_neighboring_local_slopes() {
        let meta = WorldMeta::new(42);
        let left = build_prototype(ChunkCoord(4, 0, -3), &meta);
        let right = build_prototype(ChunkCoord(5, 0, -3), &meta);

        assert_shared_edge_is_gradual(&left, &right);
    }

    #[test]
    fn atlas_cell_edge_transition_stays_close_to_neighboring_local_slopes() {
        let meta = WorldMeta::new(42);
        let left = build_prototype(ChunkCoord(15, 0, 15), &meta);
        let right = build_prototype(ChunkCoord(16, 0, 15), &meta);

        assert_shared_edge_is_gradual(&left, &right);
    }

    #[test]
    fn canonical_region_edge_transition_stays_close_to_neighboring_local_slopes() {
        let meta = WorldMeta::new(42);
        let left = build_prototype(ChunkCoord(127, 0, 0), &meta);
        let right = build_prototype(ChunkCoord(128, 0, 0), &meta);

        assert_shared_edge_is_gradual(&left, &right);
    }

    #[test]
    #[ignore = "diagnostic output for the reported wall strip"]
    fn dump_reported_wall_strip() {
        let meta = WorldMeta::new(42);
        let scan = scan_reported_wall_area(&meta);

        println!(
            "reported wall scan: strongest {} delta {:.3} at world ({}, {}) between {:.3} and {:.3}",
            scan.axis,
            scan.delta,
            scan.world_x,
            scan.world_z,
            scan.left_or_back_height,
            scan.right_or_front_height
        );

        let min_world_x = scan.world_x.saturating_sub(4);
        let max_world_x = scan.world_x.saturating_add(4);
        let min_world_z = scan.world_z.saturating_sub(2);
        let max_world_z = scan.world_z.saturating_add(2);
        let mut cache = std::collections::HashMap::new();
        for world_z in min_world_z..=max_world_z {
            print!("z={world_z:>5}:");
            for world_x in min_world_x..=max_world_x {
                print!(
                    " {:>7.2}",
                    sampled_world_height(world_x, world_z, &meta, &mut cache)
                );
            }
            println!();
        }

        dump_world_sample_details(scan.world_x, scan.world_z - 1, &meta);
        dump_world_sample_details(scan.world_x, scan.world_z, &meta);
    }

    #[test]
    fn reported_wall_strip_stays_below_large_vertical_step_threshold() {
        let meta = WorldMeta::new(42);
        let scan = scan_reported_wall_area(&meta);

        assert!(
            scan.delta <= 1.35,
            "reported wall strip still has a large {} delta of {:.3} at world ({}, {})",
            scan.axis,
            scan.delta,
            scan.world_x,
            scan.world_z
        );
    }

    #[test]
    fn low_relief_chunks_still_show_subchunk_layer_variation() {
        let meta = WorldMeta::new(42);
        let prototype = low_relief_prototype(&meta);
        let mut max_adjacent_delta = 0.0_f32;

        for row in 0..CHUNK_EDGE_I32 as usize {
            for col in 1..CHUNK_EDGE_I32 as usize {
                let current = prototype.columns[row * CHUNK_EDGE_I32 as usize + col].base_height;
                let prev = prototype.columns[row * CHUNK_EDGE_I32 as usize + col - 1].base_height;
                max_adjacent_delta = max_adjacent_delta.max((current - prev).abs());
            }
        }

        assert!(
            max_adjacent_delta >= 0.25,
            "expected visible subchunk layer variation, found max adjacent delta {max_adjacent_delta}"
        );
    }

    fn build_prototype(chunk: ChunkCoord, meta: &WorldMeta) -> BaseHeightfieldPrototype {
        let inputs = prepare_chunk_v2_inputs(chunk, meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window)
    }

    fn chunk_with_corridor_window(
        meta: &WorldMeta,
    ) -> (ChunkCoord, ChunkGenerationV2Inputs, ChunkCorridorWindow) {
        let candidates = [
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
        ];

        for chunk in candidates {
            let inputs = prepare_chunk_v2_inputs(chunk, meta);
            let corridor_window = build_chunk_corridor_window(chunk, &inputs);
            if !corridor_window.corridors.is_empty() {
                return (chunk, inputs, corridor_window);
            }
        }

        panic!("expected at least one sampled chunk to carry corridor influence");
    }

    fn low_relief_prototype(meta: &WorldMeta) -> BaseHeightfieldPrototype {
        let candidates = [
            ChunkCoord(30, 0, -20),
            ChunkCoord(0, 0, 0),
            ChunkCoord(8, 0, 8),
            ChunkCoord(24, 0, -8),
            ChunkCoord(-8, 0, 12),
        ];

        for chunk in candidates {
            let inputs = prepare_chunk_v2_inputs(chunk, meta);
            let corridor_window = empty_chunk_corridor_window(chunk);
            let prototype = build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
            let max_adjacent_delta = prototype
                .columns
                .chunks(CHUNK_EDGE_I32 as usize)
                .flat_map(|row| row.windows(2))
                .map(|pair| (pair[1].base_height - pair[0].base_height).abs())
                .fold(0.0_f32, f32::max);
            if max_adjacent_delta > 0.0 {
                return prototype;
            }
        }

        panic!("expected at least one low-relief prototype candidate");
    }

    fn sampled_world_height(
        world_x: i32,
        world_z: i32,
        meta: &WorldMeta,
        cache: &mut std::collections::HashMap<ChunkCoord, BaseHeightfieldPrototype>,
    ) -> f32 {
        let chunk = ChunkCoord(
            world_x.div_euclid(CHUNK_EDGE_I32),
            0,
            world_z.div_euclid(CHUNK_EDGE_I32),
        );
        let local_x = world_x.rem_euclid(CHUNK_EDGE_I32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_EDGE_I32) as usize;
        let prototype = cache.entry(chunk).or_insert_with(|| build_prototype(chunk, meta));
        prototype.columns[local_z * CHUNK_EDGE_I32 as usize + local_x].base_height
    }

    fn scan_reported_wall_area(meta: &WorldMeta) -> ReportedWallScan {
        let min_chunk_x = 20;
        let max_chunk_x = 30;
        let min_chunk_z = -22;
        let max_chunk_z = -21;
        let min_world_x = min_chunk_x * CHUNK_EDGE_I32;
        let max_world_x = (max_chunk_x + 1) * CHUNK_EDGE_I32 - 1;
        let min_world_z = min_chunk_z * CHUNK_EDGE_I32;
        let max_world_z = (max_chunk_z + 1) * CHUNK_EDGE_I32 - 1;
        let width = usize::try_from(max_world_x - min_world_x + 1).expect("valid wall scan width");
        let depth = usize::try_from(max_world_z - min_world_z + 1).expect("valid wall scan depth");
        let mut heights = vec![0.0_f32; width * depth];
        let mut cache = std::collections::HashMap::new();

        for world_z in min_world_z..=max_world_z {
            for world_x in min_world_x..=max_world_x {
                let dz = usize::try_from(world_z - min_world_z).expect("valid wall scan row");
                let dx = usize::try_from(world_x - min_world_x).expect("valid wall scan column");
                heights[dz * width + dx] =
                    sampled_world_height(world_x, world_z, meta, &mut cache);
            }
        }

        let mut strongest = ReportedWallScan {
            axis: "horizontal",
            delta: 0.0,
            world_x: min_world_x,
            world_z: min_world_z,
            left_or_back_height: heights[0],
            right_or_front_height: heights[0],
        };

        for dz in 0..depth {
            let world_z = min_world_z + dz as i32;
            for dx in 1..width {
                let world_x = min_world_x + dx as i32;
                let left = heights[dz * width + dx - 1];
                let right = heights[dz * width + dx];
                let delta = (right - left).abs();
                if delta > strongest.delta {
                    strongest = ReportedWallScan {
                        axis: "horizontal",
                        delta,
                        world_x,
                        world_z,
                        left_or_back_height: left,
                        right_or_front_height: right,
                    };
                }
            }
        }

        for dz in 1..depth {
            let world_z = min_world_z + dz as i32;
            for dx in 0..width {
                let world_x = min_world_x + dx as i32;
                let back = heights[(dz - 1) * width + dx];
                let front = heights[dz * width + dx];
                let delta = (front - back).abs();
                if delta > strongest.delta {
                    strongest = ReportedWallScan {
                        axis: "vertical",
                        delta,
                        world_x,
                        world_z,
                        left_or_back_height: back,
                        right_or_front_height: front,
                    };
                }
            }
        }

        strongest
    }

    fn corridor_focus_index(corridor: RiverCorridorConstraint) -> usize {
        let focus_x = corridor
            .center_x
            .floor()
            .clamp(0.0, CHUNK_EDGE_I32 as f32 - 1.0) as usize;
        let focus_z = corridor
            .center_z
            .floor()
            .clamp(0.0, CHUNK_EDGE_I32 as f32 - 1.0) as usize;

        focus_z * CHUNK_EDGE_I32 as usize + focus_x
    }

    fn assert_shared_edge_is_gradual(
        left: &BaseHeightfieldPrototype,
        right: &BaseHeightfieldPrototype,
    ) {
        for row in 0..CHUNK_EDGE_I32 as usize {
            let left_last = left.columns[row * CHUNK_EDGE_I32 as usize + (CHUNK_EDGE_I32 as usize - 1)]
                .base_height;
            let left_prev = left.columns[row * CHUNK_EDGE_I32 as usize + (CHUNK_EDGE_I32 as usize - 2)]
                .base_height;
            let right_first = right.columns[row * CHUNK_EDGE_I32 as usize].base_height;
            let right_next = right.columns[row * CHUNK_EDGE_I32 as usize + 1].base_height;
            let seam_delta = (right_first - left_last).abs();
            let local_delta = (left_last - left_prev)
                .abs()
                .max((right_next - right_first).abs());

            assert!(
                seam_delta <= local_delta + 10.0,
                "shared edge delta {seam_delta} should stay close to neighboring local slope {local_delta} at row {row}"
            );
        }
    }

    fn dump_world_sample_details(world_x: i32, world_z: i32, meta: &WorldMeta) {
        let chunk = ChunkCoord(
            world_x.div_euclid(CHUNK_EDGE_I32),
            0,
            world_z.div_euclid(CHUNK_EDGE_I32),
        );
        let local_x = world_x.rem_euclid(CHUNK_EDGE_I32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_EDGE_I32) as usize;
        let sample_world_x = world_x as f32 + 0.5;
        let sample_world_z = world_z as f32 + 0.5;
        let inputs = prepare_chunk_v2_inputs(chunk, meta);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let field = sample_atlas_fields_fractional(&inputs.atlas_fields, sample_world_x, sample_world_z);
        let region_samples = sample_region_weights(&inputs.region_classes, sample_world_x, sample_world_z);
        let base_before_corridor =
            blended_base_height(&region_samples, field, sample_world_x, sample_world_z);
        let world_corridors = corridor_window
            .corridors
            .iter()
            .copied()
            .map(|corridor| world_corridor_from_chunk(chunk, corridor))
            .collect::<Vec<_>>();
        let corridor_adjustment = corridor_adjustment_for_world_column(
            field,
            sample_world_x,
            sample_world_z,
            &region_samples,
            &world_corridors,
        );

        println!(
            "sample world=({}, {}) chunk=({}, {}) local=({}, {}) base_before_corridor={:.3} corridor_delta={:.3} final={:.3} corridors={}",
            world_x,
            world_z,
            chunk.0,
            chunk.2,
            local_x,
            local_z,
            base_before_corridor,
            corridor_adjustment.height_delta,
            base_before_corridor + corridor_adjustment.height_delta,
            corridor_window.corridors.len()
        );
        println!(
            "  field macro={:.3} slope={:.3} rugged={:.3} mountain={:.3} basin={:.3} river={:.3} wetness={:.3}",
            field.macro_elevation,
            field.slope,
            field.ruggedness,
            field.mountain_mass,
            field.basinness,
            field.river_flow_potential,
            field.wetness
        );

        for sample in region_samples {
            if sample.weight <= f32::EPSILON {
                continue;
            }
            println!(
                "  region weight={:.3} archetype={:?} terrain={:?} hydro={:?} coastal={:?} family={:?}",
                sample.weight,
                sample.cell.archetype,
                sample.cell.terrain_form_family,
                sample.cell.hydrology_context,
                sample.cell.coastal_context,
                prototype_policy_family(sample.cell)
            );
        }

        for sample in region_samples {
            if sample.weight <= f32::EPSILON {
                continue;
            }
            let family = prototype_policy_family(sample.cell);
            let mode = classify_corridor_mode(sample.cell);
            for corridor in &corridor_window.corridors {
                let world_corridor = world_corridor_from_chunk(chunk, *corridor);
                let response = single_corridor_adjustment(
                    family,
                    mode,
                    field,
                    sample_world_x,
                    sample_world_z,
                    world_corridor,
                );
                if response.strongest_influence <= 0.0 {
                    continue;
                }
                println!(
                    "  corridor river={} kind={:?} order={} mode={:?} width={:.2} grade={:.5} influence={:.3} delta={:.3}",
                    corridor.river_id,
                    corridor.kind,
                    corridor.order,
                    mode,
                    corridor.half_width_blocks,
                    corridor.downstream_grade_per_block,
                    response.strongest_influence,
                    response.height_delta
                );
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct ReportedWallScan {
        axis: &'static str,
        delta: f32,
        world_x: i32,
        world_z: i32,
        left_or_back_height: f32,
        right_or_front_height: f32,
    }
}
