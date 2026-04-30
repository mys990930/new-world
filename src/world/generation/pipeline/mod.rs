use super::field::{ContinuousFieldSample, VoronoiBlendSample};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphGenerationStage {
    PaddedVoronoiGraph,
    BaseGraphFields,
    ContinentOceanMacroElevation,
    EdgeGuideSelection,
    HydrologySolve,
    NoisyBoundaryRealization,
    GraphDerivedMacroMap,
    PerlinMicroRelief,
    HeightfieldAndWaterSurface,
    ClimateHydrationBiomeResolve,
    SurfacePlan,
    VegetationPlan,
    VoxelFill,
}

pub const GRAPH_GENERATION_STAGES: [GraphGenerationStage; 13] = [
    GraphGenerationStage::PaddedVoronoiGraph,
    GraphGenerationStage::BaseGraphFields,
    GraphGenerationStage::ContinentOceanMacroElevation,
    GraphGenerationStage::EdgeGuideSelection,
    GraphGenerationStage::HydrologySolve,
    GraphGenerationStage::NoisyBoundaryRealization,
    GraphGenerationStage::GraphDerivedMacroMap,
    GraphGenerationStage::PerlinMicroRelief,
    GraphGenerationStage::HeightfieldAndWaterSurface,
    GraphGenerationStage::ClimateHydrationBiomeResolve,
    GraphGenerationStage::SurfacePlan,
    GraphGenerationStage::VegetationPlan,
    GraphGenerationStage::VoxelFill,
];

pub fn graph_generation_stages() -> &'static [GraphGenerationStage] {
    &GRAPH_GENERATION_STAGES
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphWorldGenerationConfig {
    pub graph_region_size_blocks: i32,
    pub site_spacing_blocks: i32,
    pub boundary_warp_amplitude_blocks: f32,
    pub sea_level_y: i32,
}

impl Default for GraphWorldGenerationConfig {
    fn default() -> Self {
        Self {
            graph_region_size_blocks: super::graph::DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: super::graph::DEFAULT_SITE_SPACING_BLOCKS,
            boundary_warp_amplitude_blocks: 24.0,
            sea_level_y: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSynthesisRequest {
    pub world_x: i32,
    pub world_z: i32,
    pub graph_sample: VoronoiBlendSample,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnSynthesisSample {
    pub surface_y: i32,
    pub water_y: Option<i32>,
    pub fields: ContinuousFieldSample,
}

impl ColumnSynthesisSample {
    pub fn is_submerged(self) -> bool {
        self.water_y
            .is_some_and(|water_y| water_y >= self.surface_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_order_keeps_graph_before_column_synthesis() {
        assert_eq!(
            graph_generation_stages(),
            &[
                GraphGenerationStage::PaddedVoronoiGraph,
                GraphGenerationStage::BaseGraphFields,
                GraphGenerationStage::ContinentOceanMacroElevation,
                GraphGenerationStage::EdgeGuideSelection,
                GraphGenerationStage::HydrologySolve,
                GraphGenerationStage::NoisyBoundaryRealization,
                GraphGenerationStage::GraphDerivedMacroMap,
                GraphGenerationStage::PerlinMicroRelief,
                GraphGenerationStage::HeightfieldAndWaterSurface,
                GraphGenerationStage::ClimateHydrationBiomeResolve,
                GraphGenerationStage::SurfacePlan,
                GraphGenerationStage::VegetationPlan,
                GraphGenerationStage::VoxelFill,
            ]
        );
    }

    #[test]
    fn default_config_keeps_existing_sea_level_contract() {
        let config = GraphWorldGenerationConfig::default();

        assert_eq!(config.sea_level_y, 0);
        assert!(config.site_spacing_blocks < config.graph_region_size_blocks);
    }
}
