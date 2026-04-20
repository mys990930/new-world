use crate::world::atlas::{
    AtlasFieldMap, AtlasStructureMap, MesoGuideMap, RegionClassMap, RegionClassSample,
    sample_region_classes,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};
use crate::world::meta::WorldMeta;

use super::corridors::{ChunkCorridorWindow, build_chunk_corridor_window};
use super::realization_field::{ChunkRealizationFieldPatch, build_chunk_realization_field_patch};
use super::super::sampler::{
    generate_chunk_atlas_fields, generate_chunk_atlas_structure, generate_chunk_meso_guides,
    generate_chunk_region_classes,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V2ScaffoldStage {
    AtlasInputsReady,
    RegionClassificationReady,
    RealizationFieldReady,
    CorridorWindowReady,
    PrototypeReady,
    MesoReady,
    SmoothingReady,
    HydrologyReady,
    MaterialReady,
    SeasonalReady,
}

#[derive(Debug, Clone)]
pub struct ChunkGenerationV2Inputs {
    pub chunk: ChunkCoord,
    pub atlas_fields: AtlasFieldMap,
    pub atlas_structure: AtlasStructureMap,
    pub region_classes: RegionClassMap,
    pub meso_guides: MesoGuideMap,
}

impl PartialEq for ChunkGenerationV2Inputs {
    fn eq(&self, other: &Self) -> bool {
        self.chunk == other.chunk
            && self.atlas_fields.area() == other.atlas_fields.area()
            && self.atlas_fields.cells().values() == other.atlas_fields.cells().values()
            && self.atlas_structure == other.atlas_structure
            && self.region_classes == other.region_classes
            && self.meso_guides == other.meso_guides
    }
}

#[derive(Debug, Clone)]
pub struct ChunkGenerationV2Scaffold {
    pub chunk: ChunkCoord,
    pub stage: V2ScaffoldStage,
    pub inputs: ChunkGenerationV2Inputs,
    pub center_region: RegionClassSample,
    pub realization_field_patch: ChunkRealizationFieldPatch,
    pub corridor_window: ChunkCorridorWindow,
}

impl PartialEq for ChunkGenerationV2Scaffold {
    fn eq(&self, other: &Self) -> bool {
        self.chunk == other.chunk
            && self.stage == other.stage
            && self.inputs == other.inputs
            && self.center_region == other.center_region
            && self.realization_field_patch == other.realization_field_patch
            && self.corridor_window == other.corridor_window
    }
}

pub fn prepare_chunk_v2_inputs(coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationV2Inputs {
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let atlas_structure = generate_chunk_atlas_structure(coord, meta);
    let region_classes = generate_chunk_region_classes(coord, meta, &atlas_fields, &atlas_structure);
    let meso_guides = generate_chunk_meso_guides(coord, meta, &atlas_fields, &atlas_structure);

    ChunkGenerationV2Inputs {
        chunk: coord,
        atlas_fields,
        atlas_structure,
        region_classes,
        meso_guides,
    }
}

pub fn build_chunk_v2_scaffold(coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationV2Scaffold {
    let inputs = prepare_chunk_v2_inputs(coord, meta);
    let center_world_x = coord.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2);
    let center_world_z = coord.2 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2);
    let center_region = sample_region_classes(&inputs.region_classes, center_world_x, center_world_z);
    let realization_field_patch = build_chunk_realization_field_patch(coord, &inputs);
    let corridor_window = build_chunk_corridor_window(coord, &inputs);

    ChunkGenerationV2Scaffold {
        chunk: coord,
        stage: V2ScaffoldStage::CorridorWindowReady,
        inputs,
        center_region,
        realization_field_patch,
        corridor_window,
    }
}
