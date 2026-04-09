mod atlas_debug;
mod atlas_fields;
mod atlas_resolver;
mod scale;
mod seed;

pub use atlas_debug::{
    AtlasDebugError, AtlasDebugOptions, write_debug_images, write_debug_images_with_options,
};
pub use atlas_fields::{
    AtlasCell, AtlasFieldMap, CoverPotentials, MoistureWeights, OverlayWeights,
    TerrainFormWeights, ThermalWeights, generate_atlas_fields,
};
pub use atlas_resolver::{
    AtlasResolvedCell, AtlasResolvedMap, BiomePreview, MoistureClass, OverlayClass,
    TerrainFormClass, ThermalClass, resolve_atlas,
};
pub use scale::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_IN_REGIONS, ATLAS_CELL_SIZE_M, AtlasArea,
    AtlasAreaError, AtlasCoord, AtlasGrid,
};
