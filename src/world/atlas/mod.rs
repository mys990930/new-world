mod atlas_debug;
mod atlas_fields;
mod meso;
mod region;
mod atlas_resolver;
mod scale;
mod seed;
mod structure;
mod tuning;

pub use atlas_debug::{
    AtlasDebugError, AtlasDebugOptions, write_debug_images, write_debug_images_with_options,
    write_debug_images_with_options_and_tuning,
};
pub use atlas_fields::{
    AtlasCell, AtlasFieldMap, CoverPotentials, MoistureWeights, OverlayWeights,
    TerrainFormWeights, ThermalWeights, generate_atlas_fields, generate_atlas_fields_with_tuning,
};
pub use meso::{
    MESO_GUIDE_CELL_SIZE_IN_CHUNKS, MESO_GUIDE_CELLS_PER_ATLAS_CELL, MESO_REGION_EDGE_CELLS,
    MesoCatalogEntry, MesoCatalogStatus, MesoFeatureDef, MesoGuideCell, MesoGuideMap,
    MesoGuideSample, MesoRegion, MesoRegionCoord, generate_meso_guides, meso_catalog_entries,
    meso_feature_def, meso_feature_defs, meso_region_coord_for_atlas,
    meso_regions_covering_area, sample_meso_guides,
};
pub use region::{
    BiomeFamily, ClimateRegime, CoastalContext, ElevationBand, HydrologyContext, MoistureBand,
    PrototypeArchetypeHint, RAW_CLASSIFICATION_DIMENSIONS, RESOLVED_CLASSIFICATION_DIMENSIONS,
    RawClassificationDimension, RegionArchetype, RegionArchetypeDef, RegionCatalogEntry,
    RegionCatalogStatus, RegionClassCell, RegionClassMap, RegionClassSample, ReliefClass,
    TemperatureBand, TerrainFormFamily, region_archetype_def, region_archetype_defs,
    region_archetype_prototype_hint, region_catalog_entries, resolve_region_classes,
    sample_region_classes,
};
pub use atlas_resolver::{
    AtlasResolvedCell, AtlasResolvedMap, BiomePreview, MoistureClass, OverlayClass,
    TerrainFormClass, ThermalClass, resolve_atlas, resolve_atlas_with_tuning,
};
pub use scale::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_IN_REGIONS, ATLAS_CELL_SIZE_M, AtlasArea,
    AtlasAreaError, AtlasCoord, AtlasGrid,
};
pub use structure::{
    ATLAS_STRUCTURE_REGION_EDGE_CELLS, ATLAS_STRUCTURE_REGION_PADDING_CELLS, AtlasStructureMap,
    AtlasStructureRegion, AtlasStructureRegionCoord, DrainageGraph, DrainageNode,
    DrainageNodeKind, MountainChainGraph, MountainChainId, MountainChainScale,
    MountainSpineSegment, RiverPathId, RiverPathKind, RiverPathSegment,
    atlas_structure_region_coord_for_atlas, atlas_structure_regions_covering_area,
    generate_atlas_structure, generate_atlas_structure_with_tuning,
};
pub use tuning::{
    AtlasClimateTuning, AtlasColorRgb, AtlasContinentTuning, AtlasHydrologyTuning,
    AtlasNormalizationTuning, AtlasPreviewDebugTuning, AtlasResolverTuning, AtlasRidgeTuning,
    AtlasTerrainTuning, AtlasTuning, AtlasWeightTuning,
};
