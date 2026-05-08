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
    Coast,
    Lake,
    Wetland,
    DryBasin,
    PolarIce,
    Tundra,
    BorealForest,
    TemperateGrassland,
    TemperateForest,
    TemperateRainforest,
    HotDesert,
    Savanna,
    TropicalSeasonalForest,
    TropicalRainforest,
    Alpine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphBiomeCell {
    pub site: super::graph::VoronoiSiteId,
    pub context: GraphBiomeContext,
    pub biome: GraphBiomeKind,
}

pub fn classify_graph_biome(context: GraphBiomeContext) -> GraphBiomeKind {
    let context = context.clamped();
    match context.water_role {
        GraphBiomeWaterRole::DeepOcean => return GraphBiomeKind::DeepOcean,
        GraphBiomeWaterRole::ShallowOcean => return GraphBiomeKind::ShallowOcean,
        GraphBiomeWaterRole::Coast => return GraphBiomeKind::Coast,
        GraphBiomeWaterRole::Lake => return GraphBiomeKind::Lake,
        GraphBiomeWaterRole::Wetland => return GraphBiomeKind::Wetland,
        GraphBiomeWaterRole::DryBasin => return GraphBiomeKind::DryBasin,
        GraphBiomeWaterRole::Land => {}
    }

    let effective_temperature =
        (context.temperature - context.mountainness * 0.18 - context.elevation.max(0.0) * 0.10)
            .clamp(0.0, 1.0);
    let hydration = context.hydration;

    if context.mountainness >= 0.84 && context.elevation >= 0.52 {
        return GraphBiomeKind::Alpine;
    }
    if effective_temperature <= 0.08 {
        return GraphBiomeKind::PolarIce;
    }
    if effective_temperature <= 0.20 {
        return GraphBiomeKind::Tundra;
    }
    if effective_temperature <= 0.34 {
        return if hydration >= 0.48 {
            GraphBiomeKind::BorealForest
        } else {
            GraphBiomeKind::Tundra
        };
    }
    if effective_temperature <= 0.68 {
        return if hydration < 0.24 {
            GraphBiomeKind::TemperateGrassland
        } else if hydration < 0.58 {
            GraphBiomeKind::TemperateForest
        } else if hydration < 0.82 {
            GraphBiomeKind::TemperateRainforest
        } else {
            GraphBiomeKind::Wetland
        };
    }

    if hydration < 0.18 {
        GraphBiomeKind::HotDesert
    } else if hydration < 0.44 {
        GraphBiomeKind::Savanna
    } else if hydration < 0.72 {
        GraphBiomeKind::TropicalSeasonalForest
    } else {
        GraphBiomeKind::TropicalRainforest
    }
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
    fn water_and_coast_roles_take_priority_over_climate() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Coast, 0.90, 0.90, 0.04)),
            GraphBiomeKind::Coast
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Wetland, 0.90, 0.90, 0.04)),
            GraphBiomeKind::Wetland
        );
    }

    #[test]
    fn climate_classification_distinguishes_hot_dry_and_hot_wet_land() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.88, 0.08, 0.20)),
            GraphBiomeKind::HotDesert
        );
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.88, 0.84, 0.20)),
            GraphBiomeKind::TropicalRainforest
        );
    }

    #[test]
    fn cool_wet_land_classifies_as_boreal_forest() {
        assert_eq!(
            classify_graph_biome(context(GraphBiomeWaterRole::Land, 0.30, 0.66, 0.16)),
            GraphBiomeKind::BorealForest
        );
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
