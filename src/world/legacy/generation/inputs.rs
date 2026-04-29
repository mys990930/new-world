use std::collections::HashMap;

use crate::world::atlas::{
    AtlasArea, AtlasFieldMap, AtlasStructureMap, MesoGuideMap, RegionClassMap, RegionClassSample,
    sample_region_classes,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};
use crate::world::meta::WorldMeta;

use super::corridors::{ChunkCorridorWindow, build_chunk_corridor_window};
use super::realization_field::{ChunkRealizationFieldPatch, build_chunk_realization_field_patch};
use super::sampler::{
    chunk_generation_atlas_area, generate_chunk_atlas_fields, generate_chunk_atlas_structure,
    generate_chunk_meso_guides, generate_chunk_region_classes,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationScaffoldStage {
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
pub struct ChunkGenerationInputs {
    pub chunk: ChunkCoord,
    pub seed: u64,
    pub atlas_fields: AtlasFieldMap,
    pub atlas_structure: AtlasStructureMap,
    pub region_classes: RegionClassMap,
    pub meso_guides: MesoGuideMap,
}

#[derive(Debug, Clone)]
struct SharedChunkGenerationInputs {
    seed: u64,
    atlas_fields: AtlasFieldMap,
    atlas_structure: AtlasStructureMap,
    region_classes: RegionClassMap,
    meso_guides: MesoGuideMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ChunkGenerationInputAreaKey {
    origin_x: i32,
    origin_z: i32,
    width: u32,
    height: u32,
}

impl ChunkGenerationInputAreaKey {
    fn from_area(area: AtlasArea) -> Self {
        Self {
            origin_x: area.origin().x,
            origin_z: area.origin().z,
            width: area.width(),
            height: area.height(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChunkGenerationInputCache {
    entries: HashMap<ChunkGenerationInputAreaKey, SharedChunkGenerationInputs>,
}

impl ChunkGenerationInputCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare_inputs(&mut self, coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationInputs {
        let area = chunk_generation_atlas_area(coord);
        let key = ChunkGenerationInputAreaKey::from_area(area);
        let shared = self
            .entries
            .entry(key)
            .or_insert_with(|| build_shared_chunk_generation_inputs(coord, meta));

        shared.inputs_for_chunk(coord)
    }

    pub fn cached_inputs(&self, coord: ChunkCoord) -> Option<ChunkGenerationInputs> {
        let area = chunk_generation_atlas_area(coord);
        let key = ChunkGenerationInputAreaKey::from_area(area);

        self.entries
            .get(&key)
            .map(|shared| shared.inputs_for_chunk(coord))
    }
}

impl SharedChunkGenerationInputs {
    fn inputs_for_chunk(&self, coord: ChunkCoord) -> ChunkGenerationInputs {
        ChunkGenerationInputs {
            chunk: coord,
            seed: self.seed,
            atlas_fields: self.atlas_fields.clone(),
            atlas_structure: self.atlas_structure.clone(),
            region_classes: self.region_classes.clone(),
            meso_guides: self.meso_guides.clone(),
        }
    }
}

impl PartialEq for ChunkGenerationInputs {
    fn eq(&self, other: &Self) -> bool {
        self.chunk == other.chunk
            && self.seed == other.seed
            && self.atlas_fields.area() == other.atlas_fields.area()
            && self.atlas_fields.cells().values() == other.atlas_fields.cells().values()
            && self.atlas_structure == other.atlas_structure
            && self.region_classes == other.region_classes
            && self.meso_guides == other.meso_guides
    }
}

#[derive(Debug, Clone)]
pub struct ChunkGenerationScaffold {
    pub chunk: ChunkCoord,
    pub stage: GenerationScaffoldStage,
    pub inputs: ChunkGenerationInputs,
    pub center_region: RegionClassSample,
    pub realization_field_patch: ChunkRealizationFieldPatch,
    pub corridor_window: ChunkCorridorWindow,
}

impl PartialEq for ChunkGenerationScaffold {
    fn eq(&self, other: &Self) -> bool {
        self.chunk == other.chunk
            && self.stage == other.stage
            && self.inputs == other.inputs
            && self.center_region == other.center_region
            && self.realization_field_patch == other.realization_field_patch
            && self.corridor_window == other.corridor_window
    }
}

pub fn prepare_chunk_generation_inputs(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkGenerationInputs {
    build_shared_chunk_generation_inputs(coord, meta).inputs_for_chunk(coord)
}

pub fn chunk_generation_input_area(coord: ChunkCoord) -> AtlasArea {
    chunk_generation_atlas_area(coord)
}

fn build_shared_chunk_generation_inputs(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> SharedChunkGenerationInputs {
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let atlas_structure = generate_chunk_atlas_structure(coord, meta);
    let region_classes =
        generate_chunk_region_classes(coord, meta, &atlas_fields, &atlas_structure);
    let meso_guides = generate_chunk_meso_guides(coord, meta, &atlas_fields, &atlas_structure);

    SharedChunkGenerationInputs {
        seed: meta.seed,
        atlas_fields,
        atlas_structure,
        region_classes,
        meso_guides,
    }
}

pub fn build_chunk_generation_scaffold(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> ChunkGenerationScaffold {
    let inputs = prepare_chunk_generation_inputs(coord, meta);
    let center_world_x = coord.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2);
    let center_world_z = coord.2 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2);
    let center_region =
        sample_region_classes(&inputs.region_classes, center_world_x, center_world_z);
    let realization_field_patch = build_chunk_realization_field_patch(coord, &inputs);
    let corridor_window = build_chunk_corridor_window(coord, &inputs);

    ChunkGenerationScaffold {
        chunk: coord,
        stage: GenerationScaffoldStage::CorridorWindowReady,
        inputs,
        center_region,
        realization_field_patch,
        corridor_window,
    }
}
