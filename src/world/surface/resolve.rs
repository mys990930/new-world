use crate::world::atlas::{ClimateRegime, CoastalContext, HydrologyContext, RegionArchetype, sample_region_classes};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};
use crate::world::generation::v2::{ChunkGenerationV2Inputs, HydrologyMode, HydrologySolve, SmoothedPrototype};
use crate::world::{SEA_LEVEL_Y, SmoothedColumn};

use super::cover::CoverPhase;
use super::material::{MaterialPolicyDef, MaterialPolicyId, material_policy_def};
use super::seasonal::{SeasonalBiomeStateId, SeasonalPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceRuntimeContext {
    pub seasonal_phase: Option<SeasonalPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceColumnPlan {
    pub owner_archetype: RegionArchetype,
    pub material_policy: MaterialPolicyId,
    pub seasonal_state: Option<SeasonalBiomeStateId>,
    pub cover_phase: CoverPhase,
    pub cover_override_key: Option<&'static str>,
    pub terrain_top_y: i32,
    pub water_top_y: Option<i32>,
    pub top_block_key: &'static str,
    pub filler_block_key: &'static str,
    pub core_block_key: &'static str,
    pub water_block_key: Option<&'static str>,
    pub filler_depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSurfacePlan {
    pub chunk: ChunkCoord,
    pub columns: Vec<SurfaceColumnPlan>,
}

pub fn empty_chunk_surface_plan(chunk: ChunkCoord) -> ChunkSurfacePlan {
    ChunkSurfacePlan {
        chunk,
        columns: Vec::new(),
    }
}

pub fn resolve_material_policy_for_archetype(archetype: RegionArchetype) -> MaterialPolicyId {
    match archetype {
        RegionArchetype::OceanicShelf => MaterialPolicyId::OceanicShelf,
        RegionArchetype::SandyBeachPlain
        | RegionArchetype::BarrierCoast
        | RegionArchetype::LagoonCoast => MaterialPolicyId::SandyBeach,
        RegionArchetype::CoastalCliffland
        | RegionArchetype::RockyShoreCoast
        | RegionArchetype::FjordCoast => MaterialPolicyId::CoastalCliff,
        RegionArchetype::ColdWetLowland
        | RegionArchetype::MarshFloodplain
        | RegionArchetype::SwampLowland
        | RegionArchetype::EstuaryLowland
        | RegionArchetype::CoastalDelta
        | RegionArchetype::MangroveLagoon
        | RegionArchetype::MangroveDelta
        | RegionArchetype::FloodedForestAlluvialLowland
        | RegionArchetype::FloodedForestFloodplain
        | RegionArchetype::MonsoonFloodplain
        | RegionArchetype::MonsoonDelta
        | RegionArchetype::BorealWetLowland => MaterialPolicyId::ColdWetland,
        RegionArchetype::TemperatePlain
        | RegionArchetype::TemperateRollingPlain
        | RegionArchetype::TemperateBasin
        | RegionArchetype::TemperateBroadValley
        | RegionArchetype::TemperateBroadleafPlain
        | RegionArchetype::TemperateMixedHills
        | RegionArchetype::BorealPlain
        | RegionArchetype::BorealHills => MaterialPolicyId::TemperateGrassland,
        RegionArchetype::TemperateHills | RegionArchetype::TemperateEscarpmentUpland => {
            MaterialPolicyId::TemperateGrassland
        }
        RegionArchetype::TemperatePlateau | RegionArchetype::MonsoonPlateau => {
            MaterialPolicyId::TemperatePlateau
        }
        RegionArchetype::SteppePlain | RegionArchetype::SteppeHills => {
            MaterialPolicyId::SteppeGrassland
        }
        RegionArchetype::DesertPlain
        | RegionArchetype::DesertDuneField
        | RegionArchetype::DesertBasin
        | RegionArchetype::DesertMesaCountry
        | RegionArchetype::DesertAlluvialFan
        | RegionArchetype::SemiDesertPediment
        | RegionArchetype::DryShrublandBadlands
        | RegionArchetype::DryShrublandKarst
        | RegionArchetype::MediterraneanShrublandHills => MaterialPolicyId::DesertSurface,
        RegionArchetype::SavannaPlain
        | RegionArchetype::SavannaHills
        | RegionArchetype::TropicalDryForestHills => MaterialPolicyId::SavannaGrassland,
        RegionArchetype::TropicalRainforestLowland => MaterialPolicyId::TropicalLowland,
        RegionArchetype::TropicalRainforestHills => MaterialPolicyId::TropicalHills,
        RegionArchetype::GlaciatedAlpine
        | RegionArchetype::SubalpineWoodedFront
        | RegionArchetype::AlpineMeadowMountain
        | RegionArchetype::GlacialValley
        | RegionArchetype::CrevassedIcefield
        | RegionArchetype::BorealRidgeCountry
        | RegionArchetype::AlpineRavineCountry => MaterialPolicyId::AlpineExposed,
        RegionArchetype::TundraPlain | RegionArchetype::PolarBarrensPlain => {
            MaterialPolicyId::TundraExposure
        }
    }
}

pub fn resolve_chunk_surface_plan(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
) -> ChunkSurfacePlan {
    resolve_chunk_surface_plan_with_runtime(chunk, inputs, smoothed, hydrology, None)
}

pub fn resolve_chunk_surface_plan_with_runtime(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    smoothed: &SmoothedPrototype,
    hydrology: &HydrologySolve,
    runtime: Option<&SurfaceRuntimeContext>,
) -> ChunkSurfacePlan {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(smoothed.chunk, chunk);
    debug_assert_eq!(hydrology.chunk, chunk);
    debug_assert_eq!(smoothed.columns.len(), hydrology.columns.len());

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity(hydrology.columns.len());

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = column_index(local_x, local_z);
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let region = sample_region_classes(&inputs.region_classes, world_x, world_z);
            columns.push(resolve_surface_column_plan(
                region,
                smoothed.columns[index],
                hydrology.columns[index],
                runtime,
            ));
        }
    }

    ChunkSurfacePlan { chunk, columns }
}

fn resolve_surface_column_plan(
    region: crate::world::RegionClassCell,
    smoothed: SmoothedColumn,
    hydrology: crate::world::HydrologyColumn,
    runtime: Option<&SurfaceRuntimeContext>,
) -> SurfaceColumnPlan {
    let material_policy = resolve_material_policy_for_archetype(region.archetype);
    let material = material_policy_def(material_policy)
        .expect("every archetype surface policy should resolve to a definition");
    let seasonal_state = resolve_seasonal_state(region.archetype, region.climate_regime, runtime);
    let terrain_top_y = hydrology.terrain_height.floor() as i32;
    let water_top_y = quantize_water_top_y(region, hydrology, seasonal_state, terrain_top_y);
    let cover_phase = resolve_cover_phase(
        region.archetype,
        region.climate_regime,
        seasonal_state,
        hydrology,
        water_top_y,
    );
    let cover_override_key =
        resolve_cover_override_key(material_policy, seasonal_state, cover_phase, hydrology);
    let filler_depth = filler_depth_for_policy(material_policy, hydrology);
    let water_block_key = water_top_y.map(|_| resolve_water_block_key(region, seasonal_state));
    let (top_block_key, filler_block_key, core_block_key) = resolve_block_stack(
        material,
        region.archetype,
        region.coastal_context,
        region.hydrology_context,
        smoothed,
        hydrology,
        seasonal_state,
        cover_phase,
        cover_override_key,
        water_block_key,
    );

    SurfaceColumnPlan {
        owner_archetype: region.archetype,
        material_policy,
        seasonal_state,
        cover_phase,
        cover_override_key,
        terrain_top_y,
        water_top_y,
        top_block_key,
        filler_block_key,
        core_block_key,
        water_block_key,
        filler_depth,
    }
}

fn resolve_seasonal_state(
    archetype: RegionArchetype,
    climate: ClimateRegime,
    runtime: Option<&SurfaceRuntimeContext>,
) -> Option<SeasonalBiomeStateId> {
    match runtime.and_then(|ctx| ctx.seasonal_phase) {
        Some(SeasonalPhase::Winter) => match archetype {
            RegionArchetype::ColdWetLowland => Some(SeasonalBiomeStateId::ColdFrozenWetland),
            RegionArchetype::TemperatePlain
            | RegionArchetype::TemperateHills
            | RegionArchetype::TemperatePlateau
            | RegionArchetype::SteppePlain => Some(SeasonalBiomeStateId::TemperateSnowy),
            RegionArchetype::GlaciatedAlpine => Some(SeasonalBiomeStateId::AlpineSnowpack),
            RegionArchetype::CoastalCliffland | RegionArchetype::SandyBeachPlain => {
                Some(SeasonalBiomeStateId::CoastalStormSeason)
            }
            _ if matches!(climate, ClimateRegime::Polar | ClimateRegime::ColdAlpine) => {
                Some(SeasonalBiomeStateId::AlpineSnowpack)
            }
            _ => None,
        },
        Some(SeasonalPhase::Spring) | Some(SeasonalPhase::Summer) => match archetype {
            RegionArchetype::TemperatePlain
            | RegionArchetype::TemperateHills
            | RegionArchetype::TemperatePlateau => Some(SeasonalBiomeStateId::TemperateGrowing),
            _ => None,
        },
        Some(SeasonalPhase::Autumn) => match archetype {
            RegionArchetype::CoastalCliffland | RegionArchetype::SandyBeachPlain => {
                Some(SeasonalBiomeStateId::CoastalStormSeason)
            }
            _ => None,
        },
        Some(SeasonalPhase::WetSeason) => match archetype {
            RegionArchetype::SavannaPlain
            | RegionArchetype::TropicalRainforestLowland
            | RegionArchetype::TropicalRainforestHills => {
                Some(SeasonalBiomeStateId::TropicalWetSeason)
            }
            _ => None,
        },
        Some(SeasonalPhase::DrySeason) => match archetype {
            RegionArchetype::SavannaPlain
            | RegionArchetype::TropicalRainforestLowland
            | RegionArchetype::TropicalRainforestHills => {
                Some(SeasonalBiomeStateId::TropicalDrySeason)
            }
            _ => None,
        },
        Some(SeasonalPhase::Thaw) => match archetype {
            RegionArchetype::GlaciatedAlpine => Some(SeasonalBiomeStateId::AlpineSnowpack),
            _ => None,
        },
        None => None,
    }
}

fn quantize_water_top_y(
    region: crate::world::RegionClassCell,
    hydrology: crate::world::HydrologyColumn,
    seasonal_state: Option<SeasonalBiomeStateId>,
    terrain_top_y: i32,
) -> Option<i32> {
    let hydrology_surface = hydrology.water_surface_height.map(|height| height.floor() as i32);
    let marine_surface = if matches!(region.archetype, RegionArchetype::OceanicShelf)
        || (matches!(region.coastal_context, CoastalContext::Marine)
            && hydrology.terrain_height < SEA_LEVEL_Y as f32 - 0.25)
        || (matches!(region.coastal_context, CoastalContext::Coastal)
            && hydrology.terrain_height < SEA_LEVEL_Y as f32 - 0.90)
    {
        Some(SEA_LEVEL_Y)
    } else {
        None
    };

    hydrology_surface
        .or(marine_surface)
        .filter(|water_top_y| *water_top_y > terrain_top_y)
        .map(|water_top_y| {
            if freezes_surface_water(region.archetype, seasonal_state) {
                water_top_y.max(terrain_top_y + 1)
            } else {
                water_top_y
            }
        })
}

fn resolve_cover_phase(
    archetype: RegionArchetype,
    climate: ClimateRegime,
    seasonal_state: Option<SeasonalBiomeStateId>,
    hydrology: crate::world::HydrologyColumn,
    water_top_y: Option<i32>,
) -> CoverPhase {
    if matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::TemperateSnowy | SeasonalBiomeStateId::AlpineSnowpack)
    ) || matches!(archetype, RegionArchetype::GlaciatedAlpine)
    {
        return CoverPhase::SnowCovered;
    }

    if matches!(seasonal_state, Some(SeasonalBiomeStateId::ColdFrozenWetland))
        || freezes_surface_water(archetype, seasonal_state)
    {
        return CoverPhase::Frozen;
    }

    if water_top_y.is_some()
        || matches!(
            hydrology.mode,
            HydrologyMode::Channel | HydrologyMode::Floodplain | HydrologyMode::Lake | HydrologyMode::Wetland
        )
        || hydrology.saturation >= 0.55
    {
        return CoverPhase::Saturated;
    }

    if matches!(seasonal_state, Some(SeasonalBiomeStateId::TropicalDrySeason))
        || matches!(climate, ClimateRegime::AridHot)
    {
        return CoverPhase::Dry;
    }

    if matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::TemperateGrowing | SeasonalBiomeStateId::TropicalWetSeason)
    ) {
        return CoverPhase::Growing;
    }

    CoverPhase::Dormant
}

fn resolve_cover_override_key(
    material_policy: MaterialPolicyId,
    seasonal_state: Option<SeasonalBiomeStateId>,
    cover_phase: CoverPhase,
    hydrology: crate::world::HydrologyColumn,
) -> Option<&'static str> {
    match seasonal_state {
        Some(SeasonalBiomeStateId::TemperateSnowy)
            if matches!(
                material_policy,
                MaterialPolicyId::TemperateGrassland
                    | MaterialPolicyId::TemperatePlateau
                    | MaterialPolicyId::SteppeGrassland
                    | MaterialPolicyId::SavannaGrassland
            ) =>
        {
            Some("snowy_grass")
        }
        Some(SeasonalBiomeStateId::ColdFrozenWetland)
            if matches!(cover_phase, CoverPhase::Frozen)
                && hydrology.saturation >= 0.40 =>
        {
            Some("frozen_mud")
        }
        Some(SeasonalBiomeStateId::TropicalWetSeason)
            if matches!(
                material_policy,
                MaterialPolicyId::SavannaGrassland
                    | MaterialPolicyId::TropicalLowland
                    | MaterialPolicyId::TropicalHills
            ) =>
        {
            Some("wet_season_greening")
        }
        _ => None,
    }
}

fn resolve_block_stack(
    material: &MaterialPolicyDef,
    archetype: RegionArchetype,
    coastal_context: CoastalContext,
    hydrology_context: HydrologyContext,
    smoothed: SmoothedColumn,
    hydrology: crate::world::HydrologyColumn,
    _seasonal_state: Option<SeasonalBiomeStateId>,
    cover_phase: CoverPhase,
    cover_override_key: Option<&'static str>,
    water_block_key: Option<&'static str>,
) -> (&'static str, &'static str, &'static str) {
    let mut top = material.default_top_block;
    let mut filler = material.subsoil_block;
    let mut core = material.deep_block;
    let steepness = smoothed.local_slope;
    let steep_threshold = rock_exposure_threshold(material.id);
    let saturated = matches!(cover_phase, CoverPhase::Saturated)
        || matches!(
            hydrology.mode,
            HydrologyMode::Channel | HydrologyMode::Floodplain | HydrologyMode::Lake | HydrologyMode::Wetland
        );

    if hydrology.gravel_bar_strength >= 0.38 {
        top = if water_block_key.is_some()
            && matches!(coastal_context, CoastalContext::Coastal | CoastalContext::Marine)
        {
            "wet_gravel"
        } else {
            "gravel"
        };
        filler = if matches!(coastal_context, CoastalContext::Coastal | CoastalContext::Marine) {
            "gravel"
        } else {
            material.sediment_block
        };
    } else if saturated {
        let (wet_top, wet_filler, wet_core) = hydrology_stack_override(
            material.id,
            archetype,
            hydrology_context,
            hydrology,
            water_block_key,
        );
        top = wet_top;
        filler = wet_filler;
        core = wet_core;
    } else if steepness >= steep_threshold {
        top = material.exposed_block;
        filler = if steepness >= steep_threshold + 0.55 {
            material.deep_block
        } else {
            material.subsoil_block
        };
    } else if matches!(cover_phase, CoverPhase::Frozen) {
        top = material.frozen_top_block;
    } else if matches!(cover_phase, CoverPhase::Dry) {
        top = material.dry_top_block;
    } else if matches!(cover_phase, CoverPhase::Saturated) {
        top = material.wet_top_block;
    }

    match cover_override_key {
        Some("snowy_grass") => {
            top = "snow";
        }
        Some("frozen_mud") => {
            top = if water_block_key.is_some() { "ice" } else { "snow" };
            filler = "mud";
        }
        Some("wet_season_greening") => {
            top = match material.id {
                MaterialPolicyId::SavannaGrassland => "grass",
                MaterialPolicyId::TropicalLowland => "leaf_litter",
                MaterialPolicyId::TropicalHills => "moss",
                _ => top,
            };
        }
        _ => {}
    }

    (top, filler, core)
}

fn hydrology_stack_override(
    material_policy: MaterialPolicyId,
    archetype: RegionArchetype,
    hydrology_context: HydrologyContext,
    hydrology: crate::world::HydrologyColumn,
    water_block_key: Option<&'static str>,
) -> (&'static str, &'static str, &'static str) {
    if hydrology.gravel_bar_strength >= 0.52 {
        return (
            if water_block_key.is_some() { "wet_gravel" } else { "gravel" },
            "gravel",
            "stone",
        );
    }

    match material_policy {
        MaterialPolicyId::OceanicShelf => ("silt", "clay", "stone"),
        MaterialPolicyId::SandyBeach => {
            if matches!(hydrology.mode, HydrologyMode::Lake | HydrologyMode::Floodplain) {
                ("silt", "sand", "stone")
            } else {
                ("wet_sand", "sand", "stone")
            }
        }
        MaterialPolicyId::CoastalCliff => ("wet_gravel", "gravel", "stone"),
        MaterialPolicyId::TemperateGrassland | MaterialPolicyId::TemperatePlateau => {
            if matches!(hydrology.mode, HydrologyMode::Wetland | HydrologyMode::Lake) {
                ("mud", "clay", "stone")
            } else {
                ("silt", "dirt", "stone")
            }
        }
        MaterialPolicyId::SteppeGrassland | MaterialPolicyId::SavannaGrassland => {
            if matches!(hydrology.mode, HydrologyMode::Lake | HydrologyMode::Wetland) {
                ("mud", "clay", "stone")
            } else {
                ("silt", "dirt", "stone")
            }
        }
        MaterialPolicyId::DesertSurface => match hydrology.mode {
            HydrologyMode::Lake => ("clay", "silt", "sandstone"),
            HydrologyMode::Floodplain => ("wet_sand", "sand", "sandstone"),
            _ => ("gravel", "sand", "sandstone"),
        },
        MaterialPolicyId::TropicalLowland | MaterialPolicyId::TropicalHills => {
            if matches!(hydrology.mode, HydrologyMode::Wetland) {
                ("peat", "mud", if matches!(archetype, RegionArchetype::TropicalRainforestHills) { "stone" } else { "dirt" })
            } else {
                ("mud", "clay", if matches!(archetype, RegionArchetype::TropicalRainforestHills) { "stone" } else { "dirt" })
            }
        }
        MaterialPolicyId::ColdWetland => {
            if matches!(hydrology.mode, HydrologyMode::Wetland | HydrologyMode::Lake)
                || matches!(hydrology_context, HydrologyContext::WetLowland | HydrologyContext::LakeBasin)
            {
                ("peat", "mud", "dirt")
            } else {
                ("mud", "clay", "dirt")
            }
        }
        MaterialPolicyId::AlpineExposed | MaterialPolicyId::TundraExposure => {
            if water_block_key == Some("ice") {
                ("ice", "gravel", "stone")
            } else {
                ("gravel", "moraine", "stone")
            }
        }
    }
}

fn resolve_water_block_key(
    region: crate::world::RegionClassCell,
    seasonal_state: Option<SeasonalBiomeStateId>,
) -> &'static str {
    if freezes_surface_water(region.archetype, seasonal_state)
        || (matches!(region.climate_regime, ClimateRegime::Polar | ClimateRegime::ColdAlpine)
            && !matches!(region.archetype, RegionArchetype::OceanicShelf))
    {
        "ice"
    } else {
        "water"
    }
}

fn freezes_surface_water(
    archetype: RegionArchetype,
    seasonal_state: Option<SeasonalBiomeStateId>,
) -> bool {
    matches!(
        seasonal_state,
        Some(SeasonalBiomeStateId::ColdFrozenWetland | SeasonalBiomeStateId::AlpineSnowpack)
    ) || matches!(
        archetype,
        RegionArchetype::GlaciatedAlpine
            | RegionArchetype::GlacialValley
            | RegionArchetype::CrevassedIcefield
            | RegionArchetype::AlpineRavineCountry
    )
}

fn filler_depth_for_policy(
    material_policy: MaterialPolicyId,
    hydrology: crate::world::HydrologyColumn,
) -> u8 {
    let base: u8 = match material_policy {
        MaterialPolicyId::OceanicShelf => 3,
        MaterialPolicyId::SandyBeach => 3,
        MaterialPolicyId::CoastalCliff => 2,
        MaterialPolicyId::TemperateGrassland => 4,
        MaterialPolicyId::TemperatePlateau => 3,
        MaterialPolicyId::SteppeGrassland => 3,
        MaterialPolicyId::DesertSurface => 3,
        MaterialPolicyId::SavannaGrassland => 3,
        MaterialPolicyId::TropicalLowland => 5,
        MaterialPolicyId::TropicalHills => 4,
        MaterialPolicyId::ColdWetland => 4,
        MaterialPolicyId::AlpineExposed => 2,
        MaterialPolicyId::TundraExposure => 3,
    };

    if matches!(hydrology.mode, HydrologyMode::Wetland | HydrologyMode::Lake) {
        base.saturating_add(1)
    } else {
        base
    }
}

fn rock_exposure_threshold(material_policy: MaterialPolicyId) -> f32 {
    match material_policy {
        MaterialPolicyId::CoastalCliff | MaterialPolicyId::AlpineExposed => 0.75,
        MaterialPolicyId::TemperatePlateau | MaterialPolicyId::TropicalHills => 0.90,
        MaterialPolicyId::TundraExposure => 0.95,
        MaterialPolicyId::DesertSurface => 1.05,
        MaterialPolicyId::TemperateGrassland
        | MaterialPolicyId::SteppeGrassland
        | MaterialPolicyId::SavannaGrassland
        | MaterialPolicyId::TropicalLowland
        | MaterialPolicyId::ColdWetland
        | MaterialPolicyId::OceanicShelf
        | MaterialPolicyId::SandyBeach => 1.15,
    }
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::{ClimateRegime, CoastalContext, ElevationBand, HydrologyContext, MoistureBand, RegionClassCell, ReliefClass, TemperatureBand, TerrainFormFamily};
    use crate::world::generation::v2::{
        build_chunk_base_heightfield_prototype, build_chunk_corridor_window,
        build_chunk_hydrology_solve, build_chunk_meso_applied_prototype,
        build_chunk_realization_field_patch, build_chunk_smoothed_prototype, prepare_chunk_v2_inputs,
    };
    use crate::world::meta::WorldMeta;

    #[test]
    fn launch_archetypes_resolve_to_defined_material_policies() {
        let launch = [
            RegionArchetype::OceanicShelf,
            RegionArchetype::SandyBeachPlain,
            RegionArchetype::CoastalCliffland,
            RegionArchetype::ColdWetLowland,
            RegionArchetype::TemperatePlain,
            RegionArchetype::TemperateHills,
            RegionArchetype::TemperatePlateau,
            RegionArchetype::SteppePlain,
            RegionArchetype::DesertPlain,
            RegionArchetype::DesertDuneField,
            RegionArchetype::SavannaPlain,
            RegionArchetype::TropicalRainforestLowland,
            RegionArchetype::TropicalRainforestHills,
            RegionArchetype::GlaciatedAlpine,
            RegionArchetype::TundraPlain,
        ];

        for archetype in launch {
            let policy = resolve_material_policy_for_archetype(archetype);
            assert!(material_policy_def(policy).is_some(), "missing policy for {archetype:?}");
        }
    }

    #[test]
    fn runtime_winter_turns_temperate_grass_cover_snowy() {
        let region = RegionClassCell {
            temperature_band: TemperatureBand::Temperate,
            moisture_band: MoistureBand::Subhumid,
            elevation_band: ElevationBand::Low,
            relief_class: ReliefClass::Plain,
            hydrology_context: HydrologyContext::WellDrained,
            coastal_context: CoastalContext::Inland,
            climate_regime: ClimateRegime::TemperateSeasonal,
            biome_family: crate::world::BiomeFamily::TemperateGrassland,
            terrain_form_family: TerrainFormFamily::Plain,
            archetype: RegionArchetype::TemperatePlain,
        };
        let smoothed = SmoothedColumn {
            height: 8.4,
            remaining_relief_budget: 4.0,
            local_slope: 0.25,
            concavity: 0.0,
        };
        let hydrology = crate::world::HydrologyColumn {
            terrain_height: 8.4,
            water_surface_height: None,
            channel_floor_height: None,
            saturation: 0.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Dry,
        };
        let runtime = SurfaceRuntimeContext {
            seasonal_phase: Some(SeasonalPhase::Winter),
        };

        let plan = resolve_surface_column_plan(region, smoothed, hydrology, Some(&runtime));

        assert_eq!(plan.seasonal_state, Some(SeasonalBiomeStateId::TemperateSnowy));
        assert_eq!(plan.cover_override_key, Some("snowy_grass"));
        assert_eq!(plan.top_block_key, "snow");
    }

    #[test]
    fn surface_plan_is_deterministic_for_same_chunk_inputs() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(15, 0, 15);
        let inputs = prepare_chunk_v2_inputs(chunk, &meta);
        let realization = build_chunk_realization_field_patch(chunk, &inputs);
        let corridors = build_chunk_corridor_window(chunk, &inputs);
        let prototype =
            build_chunk_base_heightfield_prototype(chunk, &inputs, &realization, &corridors);
        let meso = build_chunk_meso_applied_prototype(chunk, &inputs, &corridors, &prototype);
        let smoothed = build_chunk_smoothed_prototype(chunk, &corridors, &meso);
        let hydrology = build_chunk_hydrology_solve(chunk, &inputs, &corridors, &smoothed);

        let a = resolve_chunk_surface_plan(chunk, &inputs, &smoothed, &hydrology);
        let b = resolve_chunk_surface_plan(chunk, &inputs, &smoothed, &hydrology);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert_eq!(a.columns.len(), hydrology.columns.len());
    }
}
