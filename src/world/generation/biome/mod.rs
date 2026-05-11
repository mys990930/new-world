#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphBiomeContext {
    pub temperature: f32,
    pub hydration: f32,
    pub elevation: f32,
    pub continentality: f32,
    pub coastness: f32,
    pub mountainness: f32,
    pub ruggedness: f32,
    pub water_role: GraphBiomeWaterRole,
}

impl GraphBiomeContext {
    pub fn clamped(self) -> Self {
        Self {
            temperature: clamp_unit(self.temperature),
            hydration: clamp_unit(self.hydration),
            elevation: self.elevation,
            continentality: self.continentality.clamp(-1.0, 1.0),
            coastness: clamp_unit(self.coastness),
            mountainness: clamp_unit(self.mountainness),
            ruggedness: clamp_unit(self.ruggedness),
            water_role: self.water_role,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphBiomeWaterRole {
    Land,
    Coast,
    ShallowOcean,
    DeepOcean,
    Lake,
    Wetland,
    DryBasin,
}

impl Default for GraphBiomeWaterRole {
    fn default() -> Self {
        Self::Land
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphBiomeKind {
    ShallowOcean,
    DeepOcean,
    Mangrove,
    EstuarineCoast,
    LagoonCoast,
    RockyCoast,
    SandyCoast,
    Lake,
    Marsh,
    Swamp,
    FloodedForest,
    Desert,
    SemiDesert,
    Steppe,
    DryShrubland,
    MediterraneanShrubland,
    PolarIce,
    PolarBarrens,
    Tundra,
    SubalpineWoodland,
    AlpineMeadow,
    BorealForest,
    TropicalRainforest,
    MonsoonForest,
    TropicalDryForest,
    Savanna,
    TemperateRainforest,
    TemperateMixedForest,
    TemperateBroadleafForest,
    TemperateGrassland,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphBiomeCell {
    pub site: super::graph::VoronoiSiteId,
    pub context: GraphBiomeContext,
    pub biome: GraphBiomeKind,
}

pub fn classify_graph_biome(context: GraphBiomeContext) -> GraphBiomeKind {
    let context = context.clamped();
    let effective_temperature = effective_temperature(context);
    match context.water_role {
        GraphBiomeWaterRole::DeepOcean => return GraphBiomeKind::DeepOcean,
        GraphBiomeWaterRole::ShallowOcean => return GraphBiomeKind::ShallowOcean,
        GraphBiomeWaterRole::Coast => return classify_coast(context, effective_temperature),
        GraphBiomeWaterRole::Lake => return GraphBiomeKind::Lake,
        GraphBiomeWaterRole::Wetland => return classify_wetland(context, effective_temperature),
        GraphBiomeWaterRole::DryBasin => {
            return classify_dry_arid(context, effective_temperature);
        }
        GraphBiomeWaterRole::Land => {}
    }

    let hydration = context.hydration;

    if is_high_alpine(context) {
        return classify_alpine(context, effective_temperature);
    }
    if effective_temperature <= 0.24 {
        return GraphBiomeKind::PolarIce;
    }
    if effective_temperature <= 0.34 {
        return if hydration < 0.36 {
            GraphBiomeKind::PolarBarrens
        } else {
            GraphBiomeKind::Tundra
        };
    }
    if effective_temperature <= 0.36 {
        return GraphBiomeKind::Tundra;
    }
    if is_cold_steppe_context(context, effective_temperature) {
        return GraphBiomeKind::Steppe;
    }
    if hydration < 0.40 {
        return classify_dry_arid(context, effective_temperature);
    }
    if is_boreal_forest_context(context, effective_temperature) {
        return GraphBiomeKind::BorealForest;
    }
    if effective_temperature >= 0.62 {
        return if hydration >= 0.68 {
            GraphBiomeKind::TropicalRainforest
        } else if hydration >= 0.60 {
            GraphBiomeKind::MonsoonForest
        } else if is_savanna_context(context, effective_temperature) {
            GraphBiomeKind::Savanna
        } else if hydration >= 0.44 {
            GraphBiomeKind::TropicalDryForest
        } else {
            GraphBiomeKind::TropicalDryForest
        };
    }

    if hydration >= 0.68 {
        GraphBiomeKind::TemperateRainforest
    } else if hydration >= 0.56 {
        GraphBiomeKind::TemperateMixedForest
    } else if hydration >= 0.46 {
        GraphBiomeKind::TemperateBroadleafForest
    } else {
        GraphBiomeKind::TemperateGrassland
    }
}

fn classify_coast(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    if context.ruggedness > 0.0 || context.mountainness > 0.0 || context.elevation >= 0.08 {
        GraphBiomeKind::RockyCoast
    } else if effective_temperature >= 0.62
        && context.hydration >= 0.66
        && context.coastness >= 0.98
        && context.ruggedness < 0.004
        && context.continentality <= 0.06
    {
        GraphBiomeKind::Mangrove
    } else if context.coastness >= 0.98
        && context.elevation <= 0.06
        && context.hydration >= 0.68
        && context.ruggedness <= 0.002
        && context.mountainness <= 0.005
        && (0.035..=0.080).contains(&context.continentality)
    {
        GraphBiomeKind::LagoonCoast
    } else if context.hydration >= 0.67
        && context.coastness >= 0.98
        && context.ruggedness < 0.008
        && context.continentality <= 0.07
    {
        GraphBiomeKind::EstuarineCoast
    } else {
        GraphBiomeKind::SandyCoast
    }
}

fn classify_wetland(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    if context.hydration >= 0.68
        && effective_temperature >= 0.50
        && context.mountainness < 0.48
        && context.ruggedness < 0.36
        && context.continentality >= -0.20
    {
        GraphBiomeKind::FloodedForest
    } else if effective_temperature >= 0.50
        && context.hydration >= 0.58
        && context.ruggedness < 0.40
    {
        GraphBiomeKind::Swamp
    } else {
        GraphBiomeKind::Marsh
    }
}

fn classify_alpine(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    if context.elevation >= 0.68 && effective_temperature <= 0.28 {
        GraphBiomeKind::PolarIce
    } else if effective_temperature <= 0.32
        || (context.ruggedness >= 0.28 && effective_temperature <= 0.40 && context.hydration < 0.40)
    {
        if context.hydration < 0.36 {
            GraphBiomeKind::PolarBarrens
        } else {
            GraphBiomeKind::Tundra
        }
    } else if effective_temperature <= 0.50 && context.hydration >= 0.50 {
        GraphBiomeKind::SubalpineWoodland
    } else {
        GraphBiomeKind::AlpineMeadow
    }
}

fn classify_dry_arid(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    let hydration = context.hydration;

    if effective_temperature <= 0.34 {
        return GraphBiomeKind::PolarBarrens;
    }
    if hydration < 0.30
        && effective_temperature >= 0.60
        && is_inland_dry_context(context, 0.24)
        && context.coastness <= 0.45
    {
        return GraphBiomeKind::Desert;
    }
    if context.ruggedness >= 0.12 && hydration < 0.40 && is_inland_dry_context(context, 0.10) {
        return GraphBiomeKind::DryShrubland;
    }
    if hydration < 0.36 && effective_temperature >= 0.52 && is_inland_dry_context(context, 0.18) {
        return GraphBiomeKind::SemiDesert;
    }
    if effective_temperature <= 0.54 && hydration < 0.40 && is_inland_dry_context(context, 0.24) {
        return GraphBiomeKind::Steppe;
    }
    if (0.50..=0.64).contains(&effective_temperature)
        && (0.36..=0.48).contains(&hydration)
        && context.coastness >= 0.18
        && context.continentality <= 0.30
        && context.ruggedness < 0.12
    {
        return GraphBiomeKind::MediterraneanShrubland;
    }
    if is_savanna_context(context, effective_temperature) {
        GraphBiomeKind::Savanna
    } else if hydration < 0.38 {
        GraphBiomeKind::TemperateGrassland
    } else {
        GraphBiomeKind::TemperateGrassland
    }
}

fn effective_temperature(context: GraphBiomeContext) -> f32 {
    (context.temperature
        - context.mountainness * 0.14
        - context.ruggedness * 0.04
        - context.elevation.max(0.0) * 0.08)
        .clamp(0.0, 1.0)
}

fn is_high_alpine(context: GraphBiomeContext) -> bool {
    context.elevation >= 0.56 && context.mountainness >= 0.34 && context.ruggedness >= 0.10
}

fn is_savanna_context(context: GraphBiomeContext, effective_temperature: f32) -> bool {
    effective_temperature >= 0.62
        && (0.38..=0.52).contains(&context.hydration)
        && context.continentality >= 0.24
        && context.coastness <= 0.35
        && context.ruggedness < 0.18
}

fn is_boreal_forest_context(context: GraphBiomeContext, effective_temperature: f32) -> bool {
    effective_temperature <= 0.44
        && context.hydration >= 0.48
        && context.continentality >= 0.10
        && context.mountainness < 0.34
}

fn is_cold_steppe_context(context: GraphBiomeContext, effective_temperature: f32) -> bool {
    effective_temperature <= 0.44
        && context.hydration < 0.40
        && context.continentality >= 0.18
        && context.coastness <= 0.45
        && context.ruggedness < 0.12
}

fn is_inland_dry_context(context: GraphBiomeContext, minimum_continentality: f32) -> bool {
    context.water_role == GraphBiomeWaterRole::DryBasin
        || context.continentality >= minimum_continentality
        || context.coastness <= 0.35
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocean_biome_is_split_into_shallow_and_deep() {
        assert_eq!(
            classify_graph_biome(context(
                GraphBiomeWaterRole::ShallowOcean,
                0.55,
                0.50,
                -0.08
            )),
            GraphBiomeKind::ShallowOcean
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::DeepOcean, 0.55, 0.50, -0.72)),
            GraphBiomeKind::DeepOcean
        );
    }

    #[test]
    fn context_clamps_explicit_ruggedness_input() {
        let mut rough = context(GraphBiomeWaterRole::Land, 0.50, 0.50, 0.10);
        rough.ruggedness = 2.0;
        rough.continentality = 2.0;

        let clamped = rough.clamped();

        assert_eq!(clamped.ruggedness, 1.0);
        assert_eq!(clamped.continentality, 1.0);
    }

    #[test]
    fn coast_roles_resolve_detailed_variants_before_land_climate() {
        let mut mangrove = context(GraphBiomeWaterRole::Coast, 0.82, 0.78, 0.04);
        mangrove.coastness = 1.0;
        mangrove.continentality = 0.03;
        assert_eq!(classify_graph_biome(mangrove), GraphBiomeKind::Mangrove);

        let mut estuary = context(GraphBiomeWaterRole::Coast, 0.58, 0.74, 0.04);
        estuary.coastness = 1.0;
        estuary.continentality = 0.02;
        assert_eq!(
            classify_graph_biome(estuary),
            GraphBiomeKind::EstuarineCoast
        );

        let mut lagoon = context(GraphBiomeWaterRole::Coast, 0.60, 0.68, 0.03);
        lagoon.coastness = 1.0;
        lagoon.continentality = 0.05;
        lagoon.ruggedness = 0.0;
        assert_eq!(classify_graph_biome(lagoon), GraphBiomeKind::LagoonCoast);

        let mut rocky = context(GraphBiomeWaterRole::Coast, 0.60, 0.50, 0.46);
        rocky.ruggedness = 0.60;
        assert_eq!(classify_graph_biome(rocky), GraphBiomeKind::RockyCoast);

        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Coast, 0.54, 0.38, 0.05)),
            GraphBiomeKind::SandyCoast
        );
    }

    #[test]
    fn coast_default_favors_sandy_then_rugged_rocky_and_lagoon_is_rare() {
        let mut common_coast = context(GraphBiomeWaterRole::Coast, 0.64, 0.58, 0.04);
        common_coast.coastness = 0.76;
        common_coast.continentality = 0.12;
        assert_eq!(
            classify_graph_biome(common_coast),
            GraphBiomeKind::SandyCoast
        );

        common_coast.ruggedness = 0.54;
        assert_eq!(
            classify_graph_biome(common_coast),
            GraphBiomeKind::RockyCoast
        );

        let mut shoreline = context(GraphBiomeWaterRole::Coast, 0.60, 0.68, 0.03);
        shoreline.coastness = 0.92;
        shoreline.continentality = 0.18;
        assert_eq!(classify_graph_biome(shoreline), GraphBiomeKind::SandyCoast);

        let mut inland_pocket = shoreline;
        inland_pocket.coastness = 1.0;
        inland_pocket.continentality = 0.05;
        inland_pocket.ruggedness = 0.0;
        assert_eq!(
            classify_graph_biome(inland_pocket),
            GraphBiomeKind::LagoonCoast
        );
    }

    #[test]
    fn wetland_roles_resolve_marsh_swamp_and_flooded_forest() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Wetland, 0.34, 0.56, 0.04)),
            GraphBiomeKind::Marsh
        );

        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Wetland, 0.58, 0.66, 0.04)),
            GraphBiomeKind::Swamp
        );

        let flooded = context(GraphBiomeWaterRole::Wetland, 0.54, 0.82, 0.05);
        assert_eq!(classify_graph_biome(flooded), GraphBiomeKind::FloodedForest);
    }

    #[test]
    fn dry_arid_branch_resolves_all_requested_variants() {
        let mut desert = context(GraphBiomeWaterRole::Land, 0.86, 0.10, 0.10);
        desert.continentality = 0.48;
        assert_eq!(classify_graph_biome(desert), GraphBiomeKind::Desert);

        let mut semi_desert = context(GraphBiomeWaterRole::Land, 0.58, 0.20, 0.10);
        semi_desert.continentality = 0.32;
        assert_eq!(
            classify_graph_biome(semi_desert),
            GraphBiomeKind::SemiDesert
        );

        let mut steppe = context(GraphBiomeWaterRole::Land, 0.42, 0.30, 0.10);
        steppe.continentality = 0.38;
        assert_eq!(classify_graph_biome(steppe), GraphBiomeKind::Steppe);

        let mut shrub = context(GraphBiomeWaterRole::Land, 0.74, 0.30, 0.10);
        shrub.continentality = 0.32;
        shrub.ruggedness = 0.48;
        assert_eq!(classify_graph_biome(shrub), GraphBiomeKind::DryShrubland);

        let mut mediterranean = context(GraphBiomeWaterRole::Land, 0.62, 0.38, 0.10);
        mediterranean.coastness = 0.18;
        assert_eq!(
            classify_graph_biome(mediterranean),
            GraphBiomeKind::MediterraneanShrubland
        );
    }

    #[test]
    fn continentality_gates_inland_dry_biomes() {
        let mut coastal_hot_dry = context(GraphBiomeWaterRole::Land, 0.86, 0.10, 0.10);
        coastal_hot_dry.coastness = 0.72;
        coastal_hot_dry.continentality = 0.04;
        assert_ne!(
            classify_graph_biome(coastal_hot_dry),
            GraphBiomeKind::Desert
        );

        let mut savanna = context(GraphBiomeWaterRole::Land, 0.78, 0.42, 0.08);
        savanna.continentality = 0.42;
        assert_eq!(classify_graph_biome(savanna), GraphBiomeKind::Savanna);

        savanna.continentality = 0.12;
        savanna.coastness = 0.62;
        assert_eq!(
            classify_graph_biome(savanna),
            GraphBiomeKind::TropicalDryForest
        );
    }

    #[test]
    fn cold_and_alpine_branch_resolves_detailed_variants() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.06, 0.54, 0.10)),
            GraphBiomeKind::PolarIce
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.30, 0.18, 0.10)),
            GraphBiomeKind::PolarBarrens
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.36, 0.44, 0.10)),
            GraphBiomeKind::Tundra
        );

        let mut subalpine = context(GraphBiomeWaterRole::Land, 0.60, 0.60, 0.60);
        subalpine.mountainness = 0.38;
        subalpine.ruggedness = 0.12;
        assert_eq!(
            classify_graph_biome(subalpine),
            GraphBiomeKind::SubalpineWoodland
        );

        let mut meadow = context(GraphBiomeWaterRole::Land, 0.68, 0.42, 0.60);
        meadow.mountainness = 0.38;
        meadow.ruggedness = 0.12;
        assert_eq!(classify_graph_biome(meadow), GraphBiomeKind::AlpineMeadow);
    }

    #[test]
    fn boreal_requires_inland_cold_humid_low_mountain_context() {
        let mut boreal = context(GraphBiomeWaterRole::Land, 0.44, 0.58, 0.08);
        boreal.continentality = 0.18;
        assert_eq!(classify_graph_biome(boreal), GraphBiomeKind::BorealForest);

        let mut oceanic_edge = boreal;
        oceanic_edge.continentality = 0.08;
        assert_eq!(
            classify_graph_biome(oceanic_edge),
            GraphBiomeKind::TemperateMixedForest
        );

        let mut mountain_edge = boreal;
        mountain_edge.temperature = 0.49;
        mountain_edge.mountainness = 0.34;
        assert_eq!(
            classify_graph_biome(mountain_edge),
            GraphBiomeKind::TemperateMixedForest
        );
    }

    #[test]
    fn cold_humid_edge_falls_back_to_temperate_instead_of_boreal() {
        let mut warm_edge = context(GraphBiomeWaterRole::Land, 0.45, 0.50, 0.08);
        warm_edge.continentality = 0.20;
        assert_eq!(
            classify_graph_biome(warm_edge),
            GraphBiomeKind::TemperateBroadleafForest
        );

        let mut not_humid_enough = context(GraphBiomeWaterRole::Land, 0.44, 0.46, 0.08);
        not_humid_enough.continentality = 0.20;
        assert_eq!(
            classify_graph_biome(not_humid_enough),
            GraphBiomeKind::TemperateBroadleafForest
        );
    }

    #[test]
    fn steppe_requires_dry_and_inland_context() {
        let mut cold_steppe = context(GraphBiomeWaterRole::Land, 0.44, 0.38, 0.08);
        cold_steppe.continentality = 0.24;
        cold_steppe.coastness = 0.35;
        assert_eq!(classify_graph_biome(cold_steppe), GraphBiomeKind::Steppe);

        let mut rugged_cold_dry = cold_steppe;
        rugged_cold_dry.ruggedness = 0.12;
        assert_eq!(
            classify_graph_biome(rugged_cold_dry),
            GraphBiomeKind::DryShrubland
        );

        let mut weakly_inland = cold_steppe;
        weakly_inland.continentality = 0.12;
        weakly_inland.coastness = 0.50;
        assert_eq!(
            classify_graph_biome(weakly_inland),
            GraphBiomeKind::TemperateGrassland
        );

        let mut not_dry = cold_steppe;
        not_dry.hydration = 0.40;
        assert_eq!(
            classify_graph_biome(not_dry),
            GraphBiomeKind::TemperateGrassland
        );
    }

    #[test]
    fn alpine_tundra_is_strict_and_milder_mountains_fall_back() {
        let mut harsh_alpine = context(GraphBiomeWaterRole::Land, 0.51, 0.39, 0.60);
        harsh_alpine.mountainness = 0.38;
        harsh_alpine.ruggedness = 0.28;
        assert_eq!(classify_graph_biome(harsh_alpine), GraphBiomeKind::Tundra);

        let mut milder_alpine = harsh_alpine;
        milder_alpine.temperature = 0.52;
        assert_eq!(
            classify_graph_biome(milder_alpine),
            GraphBiomeKind::AlpineMeadow
        );

        let mut humid_alpine = milder_alpine;
        humid_alpine.hydration = 0.54;
        assert_eq!(
            classify_graph_biome(humid_alpine),
            GraphBiomeKind::SubalpineWoodland
        );
    }

    #[test]
    fn ruggedness_gates_rocky_dry_shrub_and_alpine_behavior() {
        let mut coast = context(GraphBiomeWaterRole::Coast, 0.62, 0.48, 0.05);
        coast.ruggedness = 0.50;
        assert_eq!(classify_graph_biome(coast), GraphBiomeKind::RockyCoast);

        let mut dry = context(GraphBiomeWaterRole::Land, 0.72, 0.30, 0.12);
        dry.continentality = 0.36;
        assert_ne!(classify_graph_biome(dry), GraphBiomeKind::DryShrubland);
        dry.ruggedness = 0.48;
        assert_eq!(classify_graph_biome(dry), GraphBiomeKind::DryShrubland);

        let mut high_but_smooth = context(GraphBiomeWaterRole::Land, 0.58, 0.42, 0.72);
        high_but_smooth.mountainness = 0.66;
        assert_ne!(
            classify_graph_biome(high_but_smooth),
            GraphBiomeKind::AlpineMeadow
        );
        high_but_smooth.temperature = 0.68;
        high_but_smooth.ruggedness = 0.12;
        assert_eq!(
            classify_graph_biome(high_but_smooth),
            GraphBiomeKind::AlpineMeadow
        );
    }

    #[test]
    fn boreal_tropical_and_temperate_branches_resolve_forest_gradients() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.42, 0.62, 0.08)),
            GraphBiomeKind::BorealForest
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.82, 0.82, 0.08)),
            GraphBiomeKind::TropicalRainforest
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.78, 0.62, 0.08)),
            GraphBiomeKind::MonsoonForest
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.74, 0.46, 0.08)),
            GraphBiomeKind::TropicalDryForest
        );
        assert_eq!(
            {
                let mut savanna = context(GraphBiomeWaterRole::Land, 0.72, 0.40, 0.08);
                savanna.continentality = 0.36;
                classify_graph_biome(savanna)
            },
            GraphBiomeKind::Savanna
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.58, 0.86, 0.08)),
            GraphBiomeKind::TemperateRainforest
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.56, 0.64, 0.08)),
            GraphBiomeKind::TemperateMixedForest
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.56, 0.48, 0.08)),
            GraphBiomeKind::TemperateBroadleafForest
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.58, 0.40, 0.08)),
            GraphBiomeKind::TemperateGrassland
        );
    }

    #[test]
    fn alpine_meadow_requires_high_elevation_not_just_cold_mountains() {
        let mut mountain = context(GraphBiomeWaterRole::Land, 0.68, 0.42, 0.55);
        mountain.mountainness = 0.38;
        mountain.ruggedness = 0.12;
        assert_ne!(classify_graph_biome(mountain), GraphBiomeKind::AlpineMeadow);

        mountain.elevation = 0.60;
        assert_eq!(classify_graph_biome(mountain), GraphBiomeKind::AlpineMeadow);
    }

    fn context(
        water_role: GraphBiomeWaterRole,
        temperature: f32,
        hydration: f32,
        elevation: f32,
    ) -> GraphBiomeContext {
        GraphBiomeContext {
            temperature,
            hydration,
            elevation,
            continentality: 0.2,
            coastness: 0.0,
            mountainness: 0.0,
            ruggedness: 0.0,
            water_role,
        }
    }
}
