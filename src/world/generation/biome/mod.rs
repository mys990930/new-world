#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphBiomeContext {
    pub temperature: f32,
    pub hydration: f32,
    pub elevation: f32,
    pub continentality: f32,
    pub coastness: f32,
    pub mountainness: f32,
    pub basinness: f32,
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
            basinness: clamp_unit(self.basinness),
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

    if context.elevation >= 0.68 && context.mountainness >= 0.56 {
        return classify_alpine(context, effective_temperature);
    }
    if effective_temperature <= 0.08 {
        return GraphBiomeKind::PolarIce;
    }
    if effective_temperature <= 0.16 {
        return if hydration < 0.28 {
            GraphBiomeKind::PolarBarrens
        } else {
            GraphBiomeKind::Tundra
        };
    }
    if effective_temperature <= 0.28 {
        return GraphBiomeKind::Tundra;
    }
    if hydration < 0.38 {
        return classify_dry_arid(context, effective_temperature);
    }
    if effective_temperature <= 0.40 {
        return if hydration >= 0.42 {
            GraphBiomeKind::BorealForest
        } else {
            GraphBiomeKind::Steppe
        };
    }
    if effective_temperature >= 0.68 {
        return if hydration >= 0.76 {
            GraphBiomeKind::TropicalRainforest
        } else if hydration >= 0.58 {
            GraphBiomeKind::MonsoonForest
        } else if hydration >= 0.42 {
            GraphBiomeKind::TropicalDryForest
        } else {
            GraphBiomeKind::Savanna
        };
    }

    if hydration >= 0.82 {
        GraphBiomeKind::TemperateRainforest
    } else if hydration >= 0.58 {
        GraphBiomeKind::TemperateMixedForest
    } else if hydration >= 0.42 {
        GraphBiomeKind::TemperateBroadleafForest
    } else {
        GraphBiomeKind::TemperateGrassland
    }
}

fn classify_coast(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    if context.mountainness >= 0.58 || context.elevation >= 0.42 {
        GraphBiomeKind::RockyCoast
    } else if effective_temperature >= 0.68
        && context.hydration >= 0.72
        && context.basinness >= 0.48
    {
        GraphBiomeKind::Mangrove
    } else if context.hydration >= 0.68 && context.basinness >= 0.58 {
        GraphBiomeKind::EstuarineCoast
    } else if context.coastness >= 0.72 && context.elevation <= 0.08 && context.basinness >= 0.40 {
        GraphBiomeKind::LagoonCoast
    } else {
        GraphBiomeKind::SandyCoast
    }
}

fn classify_wetland(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    if context.hydration >= 0.78 && effective_temperature >= 0.38 && context.mountainness < 0.48 {
        GraphBiomeKind::FloodedForest
    } else if (effective_temperature >= 0.52 && context.hydration >= 0.62)
        || context.basinness >= 0.58
    {
        GraphBiomeKind::Swamp
    } else {
        GraphBiomeKind::Marsh
    }
}

fn classify_alpine(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    if context.elevation >= 0.78 && effective_temperature <= 0.12 {
        GraphBiomeKind::PolarIce
    } else if effective_temperature <= 0.18 {
        if context.hydration < 0.26 {
            GraphBiomeKind::PolarBarrens
        } else {
            GraphBiomeKind::Tundra
        }
    } else if effective_temperature <= 0.42 && context.hydration >= 0.52 {
        GraphBiomeKind::SubalpineWoodland
    } else {
        GraphBiomeKind::AlpineMeadow
    }
}

fn classify_dry_arid(context: GraphBiomeContext, effective_temperature: f32) -> GraphBiomeKind {
    let hydration = context.hydration;

    if effective_temperature <= 0.20 {
        return GraphBiomeKind::PolarBarrens;
    }
    if hydration < 0.16 && effective_temperature >= 0.52 {
        return GraphBiomeKind::Desert;
    }
    if hydration < 0.24 {
        return GraphBiomeKind::SemiDesert;
    }
    if effective_temperature <= 0.46 {
        return GraphBiomeKind::Steppe;
    }
    if (0.46..=0.72).contains(&effective_temperature)
        && hydration >= 0.28
        && (context.coastness >= 0.12 || context.basinness < 0.55)
    {
        return GraphBiomeKind::MediterraneanShrubland;
    }
    if hydration < 0.34 {
        GraphBiomeKind::DryShrubland
    } else if effective_temperature >= 0.68 {
        GraphBiomeKind::Savanna
    } else {
        GraphBiomeKind::TemperateGrassland
    }
}

fn effective_temperature(context: GraphBiomeContext) -> f32 {
    (context.temperature - context.mountainness * 0.14 - context.elevation.max(0.0) * 0.08)
        .clamp(0.0, 1.0)
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
    fn coast_roles_resolve_detailed_variants_before_land_climate() {
        let mut mangrove = context(GraphBiomeWaterRole::Coast, 0.82, 0.78, 0.04);
        mangrove.basinness = 0.52;
        assert_eq!(classify_graph_biome(mangrove), GraphBiomeKind::Mangrove);

        let mut estuary = context(GraphBiomeWaterRole::Coast, 0.58, 0.74, 0.04);
        estuary.basinness = 0.62;
        assert_eq!(
            classify_graph_biome(estuary),
            GraphBiomeKind::EstuarineCoast
        );

        let mut lagoon = context(GraphBiomeWaterRole::Coast, 0.60, 0.56, 0.03);
        lagoon.coastness = 0.80;
        lagoon.basinness = 0.45;
        assert_eq!(classify_graph_biome(lagoon), GraphBiomeKind::LagoonCoast);

        let mut rocky = context(GraphBiomeWaterRole::Coast, 0.60, 0.50, 0.46);
        rocky.mountainness = 0.60;
        assert_eq!(classify_graph_biome(rocky), GraphBiomeKind::RockyCoast);

        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Coast, 0.54, 0.38, 0.05)),
            GraphBiomeKind::SandyCoast
        );
    }

    #[test]
    fn wetland_roles_resolve_marsh_swamp_and_flooded_forest() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Wetland, 0.34, 0.56, 0.04)),
            GraphBiomeKind::Marsh
        );

        let mut swamp = context(GraphBiomeWaterRole::Wetland, 0.58, 0.66, 0.04);
        swamp.basinness = 0.48;
        assert_eq!(classify_graph_biome(swamp), GraphBiomeKind::Swamp);

        let flooded = context(GraphBiomeWaterRole::Wetland, 0.54, 0.82, 0.05);
        assert_eq!(classify_graph_biome(flooded), GraphBiomeKind::FloodedForest);
    }

    #[test]
    fn dry_arid_branch_resolves_all_requested_variants() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.86, 0.10, 0.10)),
            GraphBiomeKind::Desert
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.58, 0.20, 0.10)),
            GraphBiomeKind::SemiDesert
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.42, 0.30, 0.10)),
            GraphBiomeKind::Steppe
        );

        let mut shrub = context(GraphBiomeWaterRole::Land, 0.74, 0.30, 0.10);
        shrub.basinness = 0.64;
        assert_eq!(classify_graph_biome(shrub), GraphBiomeKind::DryShrubland);

        let mut mediterranean = context(GraphBiomeWaterRole::Land, 0.62, 0.34, 0.10);
        mediterranean.coastness = 0.18;
        assert_eq!(
            classify_graph_biome(mediterranean),
            GraphBiomeKind::MediterraneanShrubland
        );
    }

    #[test]
    fn cold_and_alpine_branch_resolves_detailed_variants() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.06, 0.54, 0.10)),
            GraphBiomeKind::PolarIce
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.14, 0.18, 0.10)),
            GraphBiomeKind::PolarBarrens
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.22, 0.44, 0.10)),
            GraphBiomeKind::Tundra
        );

        let mut subalpine = context(GraphBiomeWaterRole::Land, 0.48, 0.60, 0.70);
        subalpine.mountainness = 0.62;
        assert_eq!(
            classify_graph_biome(subalpine),
            GraphBiomeKind::SubalpineWoodland
        );

        let mut meadow = context(GraphBiomeWaterRole::Land, 0.58, 0.42, 0.72);
        meadow.mountainness = 0.66;
        assert_eq!(classify_graph_biome(meadow), GraphBiomeKind::AlpineMeadow);
    }

    #[test]
    fn boreal_tropical_and_temperate_branches_resolve_forest_gradients() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.36, 0.62, 0.08)),
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
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.72, 0.40, 0.08)),
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
        let mut mountain = context(GraphBiomeWaterRole::Land, 0.54, 0.42, 0.55);
        mountain.mountainness = 0.72;
        assert_ne!(classify_graph_biome(mountain), GraphBiomeKind::AlpineMeadow);

        mountain.elevation = 0.72;
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
            basinness: 0.0,
            water_role,
        }
    }
}
