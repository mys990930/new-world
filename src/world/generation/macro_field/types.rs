use crate::world::generation::biome::{GraphBiomeContext, GraphBiomeKind};
use crate::world::generation::graph::{VoronoiSiteId, WorldPlanePoint};
use crate::world::generation::macro_map::MacroSurfaceKind;
pub const DEFAULT_MACRO_FIELD_SAMPLE_SPACING_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS: f32 = 256.0;
pub const DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS: f32 = 640.0;
pub const DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS: f32 = 384.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE: f32 = 0.0;
pub const DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE: f32 = 0.012;
pub const DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH: f32 = 0.96;
pub const DEFAULT_MACRO_FIELD_BOUNDARY_BLEND_RADIUS_BLOCKS: f32 = 96.0;
pub const DEFAULT_MACRO_FIELD_BOUNDARY_ROUGHNESS_BLOCKS: f32 = 96.0;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldTileConfig {
    pub origin: WorldPlanePoint,
    pub width: u32,
    pub height: u32,
    pub sample_spacing_blocks: f32,
    pub ridge_radius_blocks: f32,
    pub river_radius_blocks: f32,
    pub coast_radius_blocks: f32,
    pub boundary_blend_radius_blocks: f32,
    pub boundary_roughness_blocks: f32,
    pub ridge_height_scale: f32,
    pub river_carve_scale: f32,
    pub lake_flatten_strength: f32,
}

impl MacroFieldTileConfig {
    pub const fn new(
        origin_x: f32,
        origin_z: f32,
        width: u32,
        height: u32,
        sample_spacing_blocks: f32,
    ) -> Self {
        Self {
            origin: WorldPlanePoint::new(origin_x, origin_z),
            width,
            height,
            sample_spacing_blocks,
            ridge_radius_blocks: DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS,
            river_radius_blocks: DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS,
            coast_radius_blocks: DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS,
            boundary_blend_radius_blocks: DEFAULT_MACRO_FIELD_BOUNDARY_BLEND_RADIUS_BLOCKS,
            boundary_roughness_blocks: DEFAULT_MACRO_FIELD_BOUNDARY_ROUGHNESS_BLOCKS,
            ridge_height_scale: DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE,
            river_carve_scale: DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE,
            lake_flatten_strength: DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH,
        }
    }

    pub fn sample_count(self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn sample_position(self, index: usize) -> WorldPlanePoint {
        let x = index % self.width as usize;
        let z = index / self.width as usize;
        WorldPlanePoint::new(
            self.origin.x + x as f32 * self.sample_spacing_blocks,
            self.origin.z + z as f32 * self.sample_spacing_blocks,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldSample {
    pub position: WorldPlanePoint,
    pub raw_nearest_site: Option<VoronoiSiteId>,
    pub raw_biome_context: Option<GraphBiomeContext>,
    pub raw_biome: Option<GraphBiomeKind>,
    pub nearest_site: Option<VoronoiSiteId>,
    pub surface_kind: Option<MacroSurfaceKind>,
    pub biome_context: Option<GraphBiomeContext>,
    pub biome: Option<GraphBiomeKind>,
    pub macro_elevation: f32,
    pub ocean_mask: f32,
    pub coast_mask: f32,
    pub lake_mask: f32,
    pub dry_basin_mask: f32,
    pub ridge_influence: f32,
    pub river_core_strength: f32,
    pub river_shoulder_strength: f32,
    pub river_valley_strength: f32,
    pub river_distance_blocks: f32,
    pub river_flow_hint: f32,
    pub river_longitudinal_blocks: f32,
    pub river_bed_depth_hint: f32,
    pub river_bank_roughness_hint: f32,
    pub river_gravel_hint: f32,
    pub river_cutbank_hint: f32,
    pub combined_macro_height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MacroFieldTileStats {
    pub sample_count: usize,
    pub min_combined_macro_height: f32,
    pub max_combined_macro_height: f32,
    pub max_ridge_influence: f32,
    pub average_ridge_influence: f32,
    pub ridge_active_sample_count: usize,
    pub max_river_core_strength: f32,
    pub max_river_shoulder_strength: f32,
    pub max_river_valley_strength: f32,
    pub ocean_sample_count: usize,
    pub lake_sample_count: usize,
    pub dry_basin_sample_count: usize,
    pub min_dry_basin_height: f32,
    pub max_dry_basin_height: f32,
    pub average_dry_basin_height: f32,
    pub ridge_source_curve_count: usize,
    pub river_source_curve_count: usize,
    pub coast_source_curve_count: usize,
    pub ridge_source_pixel_count: usize,
    pub river_source_pixel_count: usize,
    pub coast_source_pixel_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroFieldTile {
    pub config: MacroFieldTileConfig,
    pub samples: Vec<MacroFieldSample>,
    pub stats: MacroFieldTileStats,
}

impl MacroFieldTile {
    pub fn sample(&self, x: u32, z: u32) -> Option<&MacroFieldSample> {
        if x >= self.config.width || z >= self.config.height {
            return None;
        }
        let index = z as usize * self.config.width as usize + x as usize;
        self.samples.get(index)
    }
}
pub(super) fn validate_macro_field_config(config: MacroFieldTileConfig) {
    assert!(config.width > 0, "macro field tile width must be > 0");
    assert!(config.height > 0, "macro field tile height must be > 0");
    for (name, value) in [
        ("sample_spacing_blocks", config.sample_spacing_blocks),
        ("ridge_radius_blocks", config.ridge_radius_blocks),
        ("river_radius_blocks", config.river_radius_blocks),
        ("coast_radius_blocks", config.coast_radius_blocks),
        (
            "boundary_blend_radius_blocks",
            config.boundary_blend_radius_blocks,
        ),
        (
            "boundary_roughness_blocks",
            config.boundary_roughness_blocks,
        ),
        ("ridge_height_scale", config.ridge_height_scale),
        ("river_carve_scale", config.river_carve_scale),
        ("lake_flatten_strength", config.lake_flatten_strength),
    ] {
        assert!(value.is_finite(), "{name} must be finite");
    }
    assert!(
        config.sample_spacing_blocks > 0.0,
        "sample spacing must be positive"
    );
    assert!(config.ridge_radius_blocks > 0.0, "ridge radius must be > 0");
    assert!(config.river_radius_blocks > 0.0, "river radius must be > 0");
    assert!(config.coast_radius_blocks > 0.0, "coast radius must be > 0");
    assert!(
        config.boundary_blend_radius_blocks > 0.0,
        "boundary blend radius must be > 0"
    );
    assert!(
        config.boundary_roughness_blocks >= 0.0,
        "boundary roughness must be >= 0"
    );
}
