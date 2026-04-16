pub mod archetypes;
pub mod axes;
pub mod catalog;

pub use archetypes::{RegionArchetypeDef, region_archetype_def, region_archetype_defs};
pub use axes::{
    RAW_CLASSIFICATION_DIMENSIONS, RESOLVED_CLASSIFICATION_DIMENSIONS,
    RawClassificationDimension,
};
pub use catalog::{RegionCatalogEntry, RegionCatalogStatus, region_catalog_entries};

use crate::world::WorldMeta;

use super::atlas_fields::AtlasFieldMap;
use super::scale::{ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCoord, AtlasGrid};
use super::structure::AtlasStructureMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemperatureBand {
    #[default]
    Temperate,
    Polar,
    Cold,
    Warm,
    Hot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoistureBand {
    Arid,
    SemiArid,
    #[default]
    Subhumid,
    Humid,
    Wet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElevationBand {
    #[default]
    Low,
    Upland,
    Highland,
    Alpine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReliefClass {
    #[default]
    Plain,
    Rolling,
    Hill,
    Mountain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HydrologyContext {
    #[default]
    Dryland,
    WellDrained,
    RiverCorridor,
    LakeBasin,
    WetLowland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoastalContext {
    Marine,
    Coastal,
    NearCoast,
    #[default]
    Inland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClimateRegime {
    Polar,
    ColdAlpine,
    AridHot,
    TropicalWet,
    TropicalSeasonal,
    Continental,
    #[default]
    TemperateSeasonal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BiomeFamily {
    Oceanic,
    RockyCoast,
    SandyCoast,
    EstuarineCoast,
    LagoonCoast,
    Mangrove,
    Marsh,
    Swamp,
    FloodedForest,
    Desert,
    SemiDesert,
    Steppe,
    DryShrubland,
    MediterraneanShrubland,
    TemperateBroadleafForest,
    TemperateMixedForest,
    TemperateRainforest,
    BorealForest,
    Savanna,
    TropicalDryForest,
    TropicalRainforest,
    MonsoonForest,
    SubalpineWoodland,
    AlpineMeadow,
    Tundra,
    PolarBarrens,
    PolarIce,
    #[default]
    TemperateGrassland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerrainFormFamily {
    MarineShelf,
    BeachPlain,
    BarrierCoast,
    LagoonCoast,
    RockyShore,
    SeaCliff,
    EstuaryLowland,
    FjordCoast,
    Delta,
    Floodplain,
    WetLowland,
    AlluvialLowland,
    #[default]
    Plain,
    RollingPlain,
    HillCountry,
    Pediment,
    MountainFront,
    HillCluster,
    Mountain,
    AlluvialFan,
    Plateau,
    DuneField,
    MesaCountry,
    Escarpment,
    Badlands,
    Karst,
    Basin,
    NarrowValley,
    BroadValley,
    GlacialValley,
    Canyon,
    RavineCountry,
    RidgeCountry,
    Icefield,
    CrevassedIcefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegionArchetype {
    OceanicShelf,
    SandyBeachPlain,
    CoastalCliffland,
    ColdWetLowland,
    #[default]
    TemperatePlain,
    TemperateHills,
    TemperatePlateau,
    SteppePlain,
    DesertPlain,
    DesertDuneField,
    SavannaPlain,
    TropicalRainforestLowland,
    TropicalRainforestHills,
    GlaciatedAlpine,
    TundraPlain,
    RockyShoreCoast,
    BarrierCoast,
    LagoonCoast,
    EstuaryLowland,
    CoastalDelta,
    MangroveLagoon,
    MangroveDelta,
    MarshFloodplain,
    SwampLowland,
    FloodedForestAlluvialLowland,
    FloodedForestFloodplain,
    TemperateRollingPlain,
    TemperateBasin,
    TemperateBroadValley,
    TemperateEscarpmentUpland,
    TemperateBroadleafPlain,
    TemperateMixedHills,
    BorealPlain,
    BorealHills,
    BorealWetLowland,
    SteppeHills,
    SemiDesertPediment,
    DryShrublandBadlands,
    DryShrublandKarst,
    MediterraneanShrublandHills,
    DesertBasin,
    DesertMesaCountry,
    SavannaHills,
    TropicalDryForestHills,
    MonsoonFloodplain,
    SubalpineWoodedFront,
    AlpineMeadowMountain,
    PolarBarrensPlain,
    MonsoonDelta,
    CrevassedIcefield,
    GlacialValley,
    DesertAlluvialFan,
    FjordCoast,
    BorealRidgeCountry,
    MonsoonPlateau,
    AlpineRavineCountry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionClassCell {
    pub temperature_band: TemperatureBand,
    pub moisture_band: MoistureBand,
    pub elevation_band: ElevationBand,
    pub relief_class: ReliefClass,
    pub hydrology_context: HydrologyContext,
    pub coastal_context: CoastalContext,
    pub climate_regime: ClimateRegime,
    pub biome_family: BiomeFamily,
    pub terrain_form_family: TerrainFormFamily,
    pub archetype: RegionArchetype,
}

impl Default for RegionClassCell {
    fn default() -> Self {
        Self {
            temperature_band: TemperatureBand::default(),
            moisture_band: MoistureBand::default(),
            elevation_band: ElevationBand::default(),
            relief_class: ReliefClass::default(),
            hydrology_context: HydrologyContext::default(),
            coastal_context: CoastalContext::default(),
            climate_regime: ClimateRegime::default(),
            biome_family: BiomeFamily::default(),
            terrain_form_family: TerrainFormFamily::default(),
            archetype: RegionArchetype::default(),
        }
    }
}

pub type RegionClassSample = RegionClassCell;

#[derive(Debug, Clone)]
pub struct RegionClassMap {
    area: AtlasArea,
    cells: AtlasGrid<RegionClassCell>,
}

impl RegionClassMap {
    pub fn area(&self) -> AtlasArea {
        self.area
    }

    pub fn cells(&self) -> &AtlasGrid<RegionClassCell> {
        &self.cells
    }

    pub fn get(&self, coord: AtlasCoord) -> Option<&RegionClassCell> {
        self.cells.get(coord)
    }
}

impl PartialEq for RegionClassMap {
    fn eq(&self, other: &Self) -> bool {
        self.area == other.area && self.cells.values() == other.cells.values()
    }
}

pub fn resolve_region_classes(
    _meta: &WorldMeta,
    area: AtlasArea,
    fields: &AtlasFieldMap,
    structure: &AtlasStructureMap,
) -> RegionClassMap {
    let mut cells = AtlasGrid::defaulted(area);

    for coord in area.coords() {
        let atlas_cell = fields
            .get(coord)
            .expect("region classification field sample must exist");
        let touches_ridge = area_has_ridge(coord, structure);
        let touches_river = area_has_river(coord, structure);
        let cell = classify_region_cell(*atlas_cell, touches_ridge, touches_river);
        *cells
            .get_mut(coord)
            .expect("region classification output cell must exist") = cell;
    }

    RegionClassMap { area, cells }
}

pub fn sample_region_classes(
    classes: &RegionClassMap,
    world_x: i32,
    world_z: i32,
) -> RegionClassSample {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * crate::world::CHUNK_EDGE_I32).max(1);
    let coord = AtlasCoord::new(
        world_x.div_euclid(atlas_span_blocks),
        world_z.div_euclid(atlas_span_blocks),
    );
    *classes
        .get(coord)
        .expect("region classification sample must exist")
}

fn classify_region_cell(
    cell: super::atlas_fields::AtlasCell,
    touches_ridge: bool,
    touches_river: bool,
) -> RegionClassCell {
    let coastal_context = classify_coastal_context(cell);
    let hydrology_context = classify_hydrology_context(cell, touches_river);
    let temperature_band = classify_temperature_band(cell);
    let moisture_band = classify_moisture_band(cell);
    let elevation_band = classify_elevation_band(cell, touches_ridge);
    let relief_class = classify_relief_class(cell, touches_ridge);
    let climate_regime = classify_climate_regime(cell, temperature_band, moisture_band);
    let terrain_form_family =
        classify_terrain_form_family(cell, coastal_context, elevation_band, relief_class, hydrology_context);
    let biome_family = classify_biome_family(
        cell,
        temperature_band,
        moisture_band,
        coastal_context,
        hydrology_context,
        climate_regime,
    );
    let archetype =
        classify_region_archetype(biome_family, terrain_form_family, elevation_band, relief_class);

    RegionClassCell {
        temperature_band,
        moisture_band,
        elevation_band,
        relief_class,
        hydrology_context,
        coastal_context,
        climate_regime,
        biome_family,
        terrain_form_family,
        archetype,
    }
}

fn classify_temperature_band(cell: super::atlas_fields::AtlasCell) -> TemperatureBand {
    if cell.polar_factor > 0.68 || cell.temperature < 0.16 {
        TemperatureBand::Polar
    } else if cell.temperature < 0.34 {
        TemperatureBand::Cold
    } else if cell.temperature < 0.62 {
        TemperatureBand::Temperate
    } else if cell.temperature < 0.82 {
        TemperatureBand::Warm
    } else {
        TemperatureBand::Hot
    }
}

fn classify_moisture_band(cell: super::atlas_fields::AtlasCell) -> MoistureBand {
    let moisture_balance = (cell.humidity * 0.62 + cell.wetness * 0.38) - cell.aridity * 0.56;
    if moisture_balance < -0.24 {
        MoistureBand::Arid
    } else if moisture_balance < -0.04 {
        MoistureBand::SemiArid
    } else if moisture_balance < 0.18 {
        MoistureBand::Subhumid
    } else if moisture_balance < 0.42 {
        MoistureBand::Humid
    } else {
        MoistureBand::Wet
    }
}

fn classify_elevation_band(
    cell: super::atlas_fields::AtlasCell,
    touches_ridge: bool,
) -> ElevationBand {
    let lifted = cell.macro_elevation + cell.mountain_mass * 0.18 + if touches_ridge { 0.08 } else { 0.0 };
    if cell.alpine_factor > 0.72 || lifted > 0.82 {
        ElevationBand::Alpine
    } else if lifted > 0.60 {
        ElevationBand::Highland
    } else if lifted > 0.34 {
        ElevationBand::Upland
    } else {
        ElevationBand::Low
    }
}

fn classify_relief_class(
    cell: super::atlas_fields::AtlasCell,
    touches_ridge: bool,
) -> ReliefClass {
    let relief = cell.ruggedness * 0.58
        + cell.mountain_mass * 0.28
        + cell.ridge_factor * 0.14
        + if touches_ridge { 0.14 } else { 0.0 };
    if relief > 0.72 {
        ReliefClass::Mountain
    } else if relief > 0.42 {
        ReliefClass::Hill
    } else if relief > 0.20 {
        ReliefClass::Rolling
    } else {
        ReliefClass::Plain
    }
}

fn classify_hydrology_context(
    cell: super::atlas_fields::AtlasCell,
    touches_river: bool,
) -> HydrologyContext {
    if cell.lake_potential > 0.62 {
        HydrologyContext::LakeBasin
    } else if touches_river || cell.river_flow_potential > 0.42 || cell.riverine_factor > 0.46 {
        HydrologyContext::RiverCorridor
    } else if cell.wetness > 0.62 || cell.basinness > 0.56 {
        HydrologyContext::WetLowland
    } else if cell.aridity > 0.56 && cell.wetness < 0.24 {
        HydrologyContext::Dryland
    } else {
        HydrologyContext::WellDrained
    }
}

fn classify_coastal_context(cell: super::atlas_fields::AtlasCell) -> CoastalContext {
    if cell.landness < 0.53 {
        CoastalContext::Marine
    } else if cell.coast_factor > 0.54 {
        CoastalContext::Coastal
    } else if cell.ocean_distance < 0.18 {
        CoastalContext::NearCoast
    } else {
        CoastalContext::Inland
    }
}

fn classify_climate_regime(
    cell: super::atlas_fields::AtlasCell,
    temperature_band: TemperatureBand,
    moisture_band: MoistureBand,
) -> ClimateRegime {
    match temperature_band {
        TemperatureBand::Polar => ClimateRegime::Polar,
        TemperatureBand::Cold if cell.alpine_factor > 0.62 => ClimateRegime::ColdAlpine,
        TemperatureBand::Hot if matches!(moisture_band, MoistureBand::Wet | MoistureBand::Humid) => {
            ClimateRegime::TropicalWet
        }
        TemperatureBand::Hot => ClimateRegime::TropicalSeasonal,
        TemperatureBand::Warm | TemperatureBand::Temperate
            if matches!(moisture_band, MoistureBand::Arid | MoistureBand::SemiArid)
                && cell.aridity > 0.58 =>
        {
            ClimateRegime::AridHot
        }
        TemperatureBand::Cold => ClimateRegime::Continental,
        _ => ClimateRegime::TemperateSeasonal,
    }
}

fn classify_biome_family(
    cell: super::atlas_fields::AtlasCell,
    temperature_band: TemperatureBand,
    moisture_band: MoistureBand,
    coastal_context: CoastalContext,
    hydrology_context: HydrologyContext,
    climate_regime: ClimateRegime,
) -> BiomeFamily {
    if matches!(coastal_context, CoastalContext::Marine) {
        return BiomeFamily::Oceanic;
    }
    if matches!(coastal_context, CoastalContext::Coastal) {
        if matches!(hydrology_context, HydrologyContext::RiverCorridor | HydrologyContext::LakeBasin) {
            return BiomeFamily::EstuarineCoast;
        }
        if cell.lake_potential > 0.46 && cell.wetness > 0.40 {
            return BiomeFamily::LagoonCoast;
        }
        if matches!(temperature_band, TemperatureBand::Hot)
            && matches!(moisture_band, MoistureBand::Humid | MoistureBand::Wet)
        {
            return BiomeFamily::Mangrove;
        }
        if cell.ruggedness > 0.34 || cell.ridge_factor > 0.28 {
            return BiomeFamily::RockyCoast;
        }
        return BiomeFamily::SandyCoast;
    }
    if matches!(
        hydrology_context,
        HydrologyContext::WetLowland | HydrologyContext::LakeBasin | HydrologyContext::RiverCorridor
    ) && cell.wetness > 0.34
    {
        if matches!(temperature_band, TemperatureBand::Hot | TemperatureBand::Warm)
            && matches!(moisture_band, MoistureBand::Humid | MoistureBand::Wet)
        {
            if cell.riverine_factor > 0.42 || climate_regime == ClimateRegime::TropicalWet {
                return BiomeFamily::FloodedForest;
            }
            return BiomeFamily::Swamp;
        }
        return BiomeFamily::Marsh;
    }
    if matches!(temperature_band, TemperatureBand::Polar) {
        if cell.polar_factor > 0.78 || cell.alpine_factor > 0.82 {
            return BiomeFamily::PolarIce;
        }
        if matches!(moisture_band, MoistureBand::Arid | MoistureBand::SemiArid) {
            return BiomeFamily::PolarBarrens;
        }
        return BiomeFamily::Tundra;
    }
    if matches!(climate_regime, ClimateRegime::ColdAlpine) || cell.alpine_factor > 0.72 {
        if matches!(moisture_band, MoistureBand::Humid | MoistureBand::Wet)
            && !matches!(temperature_band, TemperatureBand::Polar)
        {
            return BiomeFamily::SubalpineWoodland;
        }
        return BiomeFamily::AlpineMeadow;
    }
    if matches!(moisture_band, MoistureBand::Arid) {
        return BiomeFamily::Desert;
    }
    if matches!(moisture_band, MoistureBand::SemiArid) {
        if matches!(temperature_band, TemperatureBand::Hot) {
            return BiomeFamily::SemiDesert;
        }
        if matches!(temperature_band, TemperatureBand::Warm | TemperatureBand::Temperate)
            && matches!(coastal_context, CoastalContext::NearCoast)
        {
            return BiomeFamily::MediterraneanShrubland;
        }
        if cell.aridity > 0.46 {
            return BiomeFamily::DryShrubland;
        }
        return BiomeFamily::Steppe;
    }
    match climate_regime {
        ClimateRegime::TropicalWet => BiomeFamily::TropicalRainforest,
        ClimateRegime::TropicalSeasonal => {
            if matches!(moisture_band, MoistureBand::Humid | MoistureBand::Wet) {
                BiomeFamily::MonsoonForest
            } else if matches!(moisture_band, MoistureBand::Subhumid) {
                BiomeFamily::TropicalDryForest
            } else {
                BiomeFamily::Savanna
            }
        }
        ClimateRegime::Continental if matches!(temperature_band, TemperatureBand::Cold) => {
            BiomeFamily::BorealForest
        }
        _ if matches!(moisture_band, MoistureBand::Humid | MoistureBand::Wet) => {
            if matches!(coastal_context, CoastalContext::NearCoast) && cell.wetness > 0.54 {
                BiomeFamily::TemperateRainforest
            } else if matches!(moisture_band, MoistureBand::Wet) {
                BiomeFamily::TemperateMixedForest
            } else {
                BiomeFamily::TemperateBroadleafForest
            }
        }
        _ if matches!(temperature_band, TemperatureBand::Warm | TemperatureBand::Hot)
            && matches!(moisture_band, MoistureBand::Subhumid) =>
        {
            BiomeFamily::Savanna
        }
        _ => BiomeFamily::TemperateGrassland,
    }
}

fn classify_terrain_form_family(
    cell: super::atlas_fields::AtlasCell,
    coastal_context: CoastalContext,
    elevation_band: ElevationBand,
    relief_class: ReliefClass,
    hydrology_context: HydrologyContext,
) -> TerrainFormFamily {
    if matches!(coastal_context, CoastalContext::Marine) {
        return TerrainFormFamily::MarineShelf;
    }
    if matches!(coastal_context, CoastalContext::Coastal) {
        if matches!(hydrology_context, HydrologyContext::RiverCorridor)
            && cell.river_flow_potential > 0.44
        {
            return TerrainFormFamily::EstuaryLowland;
        }
        if cell.lake_potential > 0.48 && cell.wetness > 0.42 {
            return TerrainFormFamily::LagoonCoast;
        }
        if matches!(elevation_band, ElevationBand::Alpine) && cell.temperature < 0.34 {
            return TerrainFormFamily::FjordCoast;
        }
        if cell.ruggedness > 0.42 || cell.ridge_factor > 0.36 {
            return TerrainFormFamily::SeaCliff;
        }
        if cell.ruggedness > 0.24 {
            return TerrainFormFamily::RockyShore;
        }
        if cell.coast_factor > 0.70 && cell.basinness > 0.38 {
            return TerrainFormFamily::BarrierCoast;
        }
        return TerrainFormFamily::BeachPlain;
    }
    if matches!(hydrology_context, HydrologyContext::RiverCorridor) {
        if cell.ocean_distance < 0.10 && cell.river_flow_potential > 0.54 {
            return TerrainFormFamily::Delta;
        }
        if matches!(elevation_band, ElevationBand::Highland | ElevationBand::Alpine) {
            if cell.alpine_factor > 0.68 {
                return TerrainFormFamily::GlacialValley;
            }
            if cell.ruggedness > 0.52 {
                return TerrainFormFamily::NarrowValley;
            }
            return TerrainFormFamily::BroadValley;
        }
        if cell.river_flow_potential > 0.52 {
            return TerrainFormFamily::AlluvialLowland;
        }
        return TerrainFormFamily::Floodplain;
    }
    if matches!(hydrology_context, HydrologyContext::WetLowland | HydrologyContext::LakeBasin) {
        return TerrainFormFamily::WetLowland;
    }
    if cell.basinness > 0.60 && !matches!(relief_class, ReliefClass::Mountain) {
        return TerrainFormFamily::Basin;
    }
    if cell.aridity > 0.64 && cell.ruggedness < 0.18 {
        return TerrainFormFamily::DuneField;
    }
    if cell.aridity > 0.58 && cell.ruggedness > 0.42 {
        return TerrainFormFamily::Badlands;
    }
    match (elevation_band, relief_class) {
        (ElevationBand::Alpine, _) if cell.alpine_factor > 0.80 && cell.polar_factor > 0.42 => {
            if cell.ruggedness > 0.46 {
                TerrainFormFamily::CrevassedIcefield
            } else {
                TerrainFormFamily::Icefield
            }
        }
        (ElevationBand::Alpine, _) | (_, ReliefClass::Mountain) => TerrainFormFamily::Mountain,
        (ElevationBand::Highland, ReliefClass::Plain | ReliefClass::Rolling) => {
            TerrainFormFamily::Plateau
        }
        (_, ReliefClass::Hill) => TerrainFormFamily::HillCountry,
        (_, ReliefClass::Rolling) => TerrainFormFamily::RollingPlain,
        _ if cell.aridity > 0.42 && matches!(elevation_band, ElevationBand::Low | ElevationBand::Upland) => {
            TerrainFormFamily::Pediment
        }
        _ => TerrainFormFamily::Plain,
    }
}

fn classify_region_archetype(
    biome_family: BiomeFamily,
    terrain_form_family: TerrainFormFamily,
    elevation_band: ElevationBand,
    relief_class: ReliefClass,
) -> RegionArchetype {
    match (biome_family, terrain_form_family) {
        (BiomeFamily::Oceanic, _) => RegionArchetype::OceanicShelf,
        (BiomeFamily::Mangrove, TerrainFormFamily::Delta) => RegionArchetype::MangroveDelta,
        (BiomeFamily::Mangrove, _) => RegionArchetype::MangroveLagoon,
        (BiomeFamily::EstuarineCoast, TerrainFormFamily::Delta) => RegionArchetype::CoastalDelta,
        (BiomeFamily::EstuarineCoast, _) => RegionArchetype::EstuaryLowland,
        (BiomeFamily::LagoonCoast, _) => RegionArchetype::LagoonCoast,
        (BiomeFamily::RockyCoast, TerrainFormFamily::FjordCoast) => RegionArchetype::FjordCoast,
        (BiomeFamily::RockyCoast, TerrainFormFamily::SeaCliff) => RegionArchetype::CoastalCliffland,
        (BiomeFamily::RockyCoast, _) => RegionArchetype::RockyShoreCoast,
        (BiomeFamily::SandyCoast, TerrainFormFamily::BarrierCoast) => RegionArchetype::BarrierCoast,
        (BiomeFamily::SandyCoast, _) => RegionArchetype::SandyBeachPlain,
        (BiomeFamily::Desert, TerrainFormFamily::DuneField) => RegionArchetype::DesertDuneField,
        (BiomeFamily::Desert, TerrainFormFamily::Basin) => RegionArchetype::DesertBasin,
        (BiomeFamily::Desert, TerrainFormFamily::MesaCountry) => RegionArchetype::DesertMesaCountry,
        (BiomeFamily::Desert, TerrainFormFamily::AlluvialFan) => RegionArchetype::DesertAlluvialFan,
        (BiomeFamily::Desert, _) => RegionArchetype::DesertPlain,
        (BiomeFamily::Steppe, TerrainFormFamily::HillCountry) => RegionArchetype::SteppeHills,
        (BiomeFamily::Steppe, _) => {
            RegionArchetype::SteppePlain
        }
        (BiomeFamily::SemiDesert, _) => RegionArchetype::SemiDesertPediment,
        (BiomeFamily::DryShrubland, TerrainFormFamily::Karst) => RegionArchetype::DryShrublandKarst,
        (BiomeFamily::DryShrubland, _) => RegionArchetype::DryShrublandBadlands,
        (BiomeFamily::MediterraneanShrubland, _) => RegionArchetype::MediterraneanShrublandHills,
        (BiomeFamily::Marsh, TerrainFormFamily::Floodplain) => RegionArchetype::MarshFloodplain,
        (BiomeFamily::Marsh, _) => RegionArchetype::ColdWetLowland,
        (BiomeFamily::Swamp, _) => RegionArchetype::SwampLowland,
        (BiomeFamily::FloodedForest, TerrainFormFamily::AlluvialLowland) => {
            RegionArchetype::FloodedForestAlluvialLowland
        }
        (BiomeFamily::FloodedForest, _) => RegionArchetype::FloodedForestFloodplain,
        (
            BiomeFamily::TropicalRainforest,
            TerrainFormFamily::HillCountry | TerrainFormFamily::Mountain,
        ) => {
            RegionArchetype::TropicalRainforestHills
        }
        (BiomeFamily::TropicalRainforest, _) => RegionArchetype::TropicalRainforestLowland,
        (BiomeFamily::Savanna, TerrainFormFamily::HillCountry) => RegionArchetype::SavannaHills,
        (BiomeFamily::Savanna, _) => RegionArchetype::SavannaPlain,
        (BiomeFamily::TropicalDryForest, _) => RegionArchetype::TropicalDryForestHills,
        (BiomeFamily::MonsoonForest, TerrainFormFamily::Delta) => RegionArchetype::MonsoonDelta,
        (BiomeFamily::MonsoonForest, TerrainFormFamily::Plateau) => RegionArchetype::MonsoonPlateau,
        (BiomeFamily::MonsoonForest, _) => RegionArchetype::MonsoonFloodplain,
        (BiomeFamily::BorealForest, TerrainFormFamily::WetLowland) => RegionArchetype::BorealWetLowland,
        (BiomeFamily::BorealForest, TerrainFormFamily::RidgeCountry) => RegionArchetype::BorealRidgeCountry,
        (BiomeFamily::BorealForest, TerrainFormFamily::HillCountry | TerrainFormFamily::Mountain) => {
            RegionArchetype::BorealHills
        }
        (BiomeFamily::BorealForest, _) => RegionArchetype::BorealPlain,
        (BiomeFamily::PolarIce, TerrainFormFamily::CrevassedIcefield) => {
            RegionArchetype::CrevassedIcefield
        }
        (BiomeFamily::PolarIce, TerrainFormFamily::GlacialValley) => RegionArchetype::GlacialValley,
        (BiomeFamily::PolarIce, _) => RegionArchetype::GlaciatedAlpine,
        (BiomeFamily::Tundra, TerrainFormFamily::WetLowland) => RegionArchetype::ColdWetLowland,
        (BiomeFamily::Tundra, _) => RegionArchetype::TundraPlain,
        (BiomeFamily::PolarBarrens, _) => RegionArchetype::PolarBarrensPlain,
        (BiomeFamily::SubalpineWoodland, _) => RegionArchetype::SubalpineWoodedFront,
        (BiomeFamily::AlpineMeadow, TerrainFormFamily::RavineCountry) => RegionArchetype::AlpineRavineCountry,
        (BiomeFamily::AlpineMeadow, _) => RegionArchetype::AlpineMeadowMountain,
        (_, TerrainFormFamily::Plateau) => RegionArchetype::TemperatePlateau,
        (_, TerrainFormFamily::Escarpment) => RegionArchetype::TemperateEscarpmentUpland,
        (_, TerrainFormFamily::BroadValley) => RegionArchetype::TemperateBroadValley,
        (_, TerrainFormFamily::Basin) => RegionArchetype::TemperateBasin,
        (_, TerrainFormFamily::RollingPlain) => RegionArchetype::TemperateRollingPlain,
        (
            BiomeFamily::TemperateBroadleafForest,
            TerrainFormFamily::HillCountry | TerrainFormFamily::Mountain,
        ) => RegionArchetype::TemperateMixedHills,
        (BiomeFamily::TemperateBroadleafForest, _) => RegionArchetype::TemperateBroadleafPlain,
        (BiomeFamily::TemperateMixedForest, _) => RegionArchetype::TemperateMixedHills,
        (BiomeFamily::TemperateRainforest, TerrainFormFamily::MountainFront) => {
            RegionArchetype::TemperateEscarpmentUpland
        }
        (BiomeFamily::TemperateRainforest, _) => RegionArchetype::TemperateBroadleafPlain,
        (_, TerrainFormFamily::HillCountry | TerrainFormFamily::Mountain)
            if matches!(elevation_band, ElevationBand::Highland | ElevationBand::Alpine)
                || matches!(relief_class, ReliefClass::Hill | ReliefClass::Mountain) =>
        {
            RegionArchetype::TemperateHills
        }
        _ => RegionArchetype::TemperatePlain,
    }
}

fn area_has_ridge(coord: AtlasCoord, structure: &AtlasStructureMap) -> bool {
    let cell_area = AtlasArea::new(coord, 1, 1).expect("single atlas cell area must be valid");
    structure
        .mountain_chains()
        .segments()
        .iter()
        .copied()
        .any(|segment| segment.touches_area(cell_area))
}

fn area_has_river(coord: AtlasCoord, structure: &AtlasStructureMap) -> bool {
    let cell_area = AtlasArea::new(coord, 1, 1).expect("single atlas cell area must be valid");
    structure
        .drainage()
        .segments()
        .iter()
        .copied()
        .any(|segment| segment.touches_area(cell_area))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{AtlasArea, AtlasCoord, WorldMeta, generate_atlas_fields, generate_atlas_structure};

    #[test]
    fn region_classification_is_deterministic() {
        let meta = WorldMeta::new(42);
        let area = AtlasArea::new(AtlasCoord::new(-2, -2), 6, 6).unwrap();
        let fields = generate_atlas_fields(&meta, area);
        let structure = generate_atlas_structure(&meta, area);

        let a = resolve_region_classes(&meta, area, &fields, &structure);
        let b = resolve_region_classes(&meta, area, &fields, &structure);

        assert_eq!(a, b);
    }

    #[test]
    fn region_classification_produces_multiple_archetypes_for_large_area() {
        let meta = WorldMeta::new(42);
        let area = AtlasArea::new(AtlasCoord::new(-8, -8), 16, 16).unwrap();
        let fields = generate_atlas_fields(&meta, area);
        let structure = generate_atlas_structure(&meta, area);
        let classes = resolve_region_classes(&meta, area, &fields, &structure);

        let mut unique = Vec::new();
        for cell in classes.cells().values() {
            if !unique.contains(&cell.archetype) {
                unique.push(cell.archetype);
            }
        }

        assert!(unique.len() >= 3);
    }
}
