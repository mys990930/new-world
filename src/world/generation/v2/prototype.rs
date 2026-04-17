use crate::world::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, CoastalContext,
    HydrologyContext, RegionArchetype, RegionClassCell, TerrainFormFamily, sample_region_classes,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::super::{SEA_LEVEL_Y, WORLD_FLOOR_Y};
use super::{ChunkCorridorWindow, ChunkGenerationV2Inputs, RiverCorridorConstraint};

const ATLAS_CELL_BLOCK_SPAN: i32 = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
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
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let center_world_x = chunk_origin_x + CHUNK_EDGE_I32.div_euclid(2);
    let center_world_z = chunk_origin_z + CHUNK_EDGE_I32.div_euclid(2);
    let center_region = sample_region_classes(&inputs.region_classes, center_world_x, center_world_z);
    let chunk_family = prototype_policy_family(center_region);
    let mut columns = Vec::with_capacity((CHUNK_EDGE_I32 as usize) * (CHUNK_EDGE_I32 as usize));

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let field = sample_atlas_field(inputs, world_x, world_z);
            let region = sample_region_classes(&inputs.region_classes, world_x, world_z);
            let family = resolve_column_family(chunk_family, region);
            let mut base_height =
                family_base_height(family, field, region, world_x as f32, world_z as f32);
            let corridor_adjustment = corridor_adjustment_for_column(
                family,
                region,
                field,
                local_x as f32 + 0.5,
                local_z as f32 + 0.5,
                &corridor_window.corridors,
            );
            base_height += corridor_adjustment.height_delta;
            base_height = base_height.clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y);

            let relief_budget = family_relief_budget(
                family,
                field,
                region,
                corridor_adjustment.strongest_influence,
                corridor_adjustment.relief_budget_penalty,
            );

            columns.push(PrototypeColumn {
                base_height,
                relief_budget,
            });
        }
    }

    BaseHeightfieldPrototype { chunk, columns }
}

fn sample_atlas_field(inputs: &ChunkGenerationV2Inputs, world_x: i32, world_z: i32) -> AtlasCell {
    let coord = clamp_atlas_coord_to_area(
        atlas_coord_for_world_point(world_x, world_z),
        inputs.atlas_fields.area(),
    );
    *inputs
        .atlas_fields
        .get(coord)
        .expect("prototype atlas field sample must exist inside the prepared area")
}

fn resolve_column_family(
    chunk_family: PrototypePolicyFamily,
    local_region: RegionClassCell,
) -> PrototypePolicyFamily {
    let local_family = prototype_policy_family(local_region);

    if matches!(
        local_family,
        PrototypePolicyFamily::MarineCoastalEdge
            | PrototypePolicyFamily::LowlandBasin
            | PrototypePolicyFamily::PlateauEscarpment
            | PrototypePolicyFamily::DuneBody
            | PrototypePolicyFamily::AlpineHighRelief
    ) {
        return local_family;
    }

    chunk_family
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

    (family_height + terrain_bias).clamp(MIN_BASE_HEIGHT_Y, MAX_BASE_HEIGHT_Y)
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

fn corridor_adjustment_for_column(
    family: PrototypePolicyFamily,
    region: RegionClassCell,
    field: AtlasCell,
    local_x: f32,
    local_z: f32,
    corridors: &[RiverCorridorConstraint],
) -> CorridorAdjustment {
    let mode = classify_corridor_mode(region);
    let mut adjustment = CorridorAdjustment::default();

    for corridor in corridors {
        let response =
            single_corridor_adjustment(family, mode, field, local_x, local_z, *corridor);
        adjustment.height_delta += response.height_delta;
        adjustment.relief_budget_penalty += response.relief_budget_penalty;
        adjustment.strongest_influence = adjustment
            .strongest_influence
            .max(response.strongest_influence);
    }

    adjustment.height_delta = adjustment.height_delta.max(-24.0);
    adjustment.relief_budget_penalty = adjustment.relief_budget_penalty.min(18.0);
    adjustment
}

fn single_corridor_adjustment(
    family: PrototypePolicyFamily,
    mode: CorridorMode,
    field: AtlasCell,
    local_x: f32,
    local_z: f32,
    corridor: RiverCorridorConstraint,
) -> CorridorAdjustment {
    let dx = local_x - corridor.center_x;
    let dz = local_z - corridor.center_z;
    let distance = (dx * dx + dz * dz).sqrt();
    let width_multiplier = match mode {
        CorridorMode::ValleySeat => 1.0,
        CorridorMode::FloodplainOpening => 1.35,
        CorridorMode::BasinOutlet => 1.2,
        CorridorMode::CoastalExit => 1.45,
    };
    let effective_half_width = (corridor.half_width_blocks * width_multiplier).max(1.0);
    let normalized = (1.0 - distance / effective_half_width).clamp(0.0, 1.0);
    if normalized <= 0.0 {
        return CorridorAdjustment::default();
    }

    let influence = smoothstep(normalized);
    let base_drop = match mode {
        CorridorMode::ValleySeat => 6.0,
        CorridorMode::FloodplainOpening => 4.5,
        CorridorMode::BasinOutlet => 5.5,
        CorridorMode::CoastalExit => 4.0,
    } + corridor.half_width_blocks * 0.035
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

fn dune_body_wave(world_x: f32, world_z: f32) -> f32 {
    let long_wave = (world_x / 72.0 + world_z / 128.0).sin();
    let cross_wave = (world_x / 148.0 - world_z / 84.0 + 0.9).cos();
    long_wave * 0.65 + cross_wave * 0.35
}

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn atlas_coord_for_world_point(world_x: i32, world_z: i32) -> AtlasCoord {
    AtlasCoord::new(
        world_x.div_euclid(ATLAS_CELL_BLOCK_SPAN),
        world_z.div_euclid(ATLAS_CELL_BLOCK_SPAN),
    )
}

fn clamp_atlas_coord_to_area(coord: AtlasCoord, area: AtlasArea) -> AtlasCoord {
    let origin = area.origin();
    let max_x = origin.x + area.width() as i32 - 1;
    let max_z = origin.z + area.height() as i32 - 1;

    AtlasCoord::new(coord.x.clamp(origin.x, max_x), coord.z.clamp(origin.z, max_z))
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
        let with_corridor =
            build_chunk_base_heightfield_prototype(chunk, &inputs, &corridor_window);
        let without_corridor =
            build_chunk_base_heightfield_prototype(chunk, &inputs, &empty_chunk_corridor_window(chunk));
        let focus = corridor_focus_index(corridor_window.corridors[0]);

        assert!(
            with_corridor.columns[focus].base_height < without_corridor.columns[focus].base_height
        );
        assert!(
            with_corridor.columns[focus].relief_budget
                <= without_corridor.columns[focus].relief_budget
        );
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
}
