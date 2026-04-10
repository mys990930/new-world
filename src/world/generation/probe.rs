use super::context::ColumnAtlasSample;
use super::profile::{TerrainProfile, resolve_profile};
use super::profiles::surface_y_for_profile;
use super::sampler::{generate_chunk_atlas_fields, sample_column_atlas};
use super::super::atlas::{ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCoord, AtlasFieldMap, AtlasTuning};
use super::super::coord::{CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, chunk_local_to_world};
use super::super::meta::WorldMeta;

const ATLAS_CELL_SPAN_BLOCKS_I32: i32 = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainProfileCounts {
    pub deep_ocean: u32,
    pub shelf: u32,
    pub coast: u32,
    pub plain: u32,
    pub upland: u32,
    pub ridge: u32,
}

impl TerrainProfileCounts {
    fn record(&mut self, profile: TerrainProfile) {
        match profile {
            TerrainProfile::DeepOcean => self.deep_ocean += 1,
            TerrainProfile::Shelf => self.shelf += 1,
            TerrainProfile::Coast => self.coast += 1,
            TerrainProfile::Plain => self.plain += 1,
            TerrainProfile::Upland => self.upland += 1,
            TerrainProfile::Ridge => self.ridge += 1,
        }
    }

    pub fn total(self) -> u32 {
        self.deep_ocean + self.shelf + self.coast + self.plain + self.upland + self.ridge
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnGenerationProbe {
    pub chunk: ChunkCoord,
    pub local_x: u8,
    pub local_z: u8,
    pub world_x: i32,
    pub world_z: i32,
    pub atlas_coord: AtlasCoord,
    pub atlas_frac_x: f32,
    pub atlas_frac_z: f32,
    pub atlas_sample: ColumnAtlasSample,
    pub profile: TerrainProfile,
    pub surface_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkGenerationProbe {
    pub chunk: ChunkCoord,
    pub surface_min_y: i32,
    pub surface_max_y: i32,
    pub surface_mean_y: f32,
    pub dominant_profile: TerrainProfile,
    pub profile_counts: TerrainProfileCounts,
}

pub fn probe_column(
    coord: ChunkCoord,
    local_x: u8,
    local_z: u8,
    meta: &WorldMeta,
) -> ColumnGenerationProbe {
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    probe_column_with_fields(&atlas_fields, coord, local_x, local_z, meta.seed)
}

pub fn probe_chunk(coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationProbe {
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let mut surface_min_y = i32::MAX;
    let mut surface_max_y = i32::MIN;
    let mut surface_sum = 0_i64;
    let mut counts = TerrainProfileCounts::default();

    for local_z in 0..CHUNK_EDGE_I32 as u8 {
        for local_x in 0..CHUNK_EDGE_I32 as u8 {
            let column = probe_column_with_fields(&atlas_fields, coord, local_x, local_z, meta.seed);
            surface_min_y = surface_min_y.min(column.surface_y);
            surface_max_y = surface_max_y.max(column.surface_y);
            surface_sum += i64::from(column.surface_y);
            counts.record(column.profile);
        }
    }

    let total = counts.total().max(1);

    ChunkGenerationProbe {
        chunk: coord,
        surface_min_y,
        surface_max_y,
        surface_mean_y: surface_sum as f32 / total as f32,
        dominant_profile: dominant_profile(counts),
        profile_counts: counts,
    }
}

fn probe_column_with_fields(
    atlas_fields: &AtlasFieldMap,
    coord: ChunkCoord,
    local_x: u8,
    local_z: u8,
    seed: u64,
) -> ColumnGenerationProbe {
    let local = LocalBlockCoord::new(local_x, 0, local_z).expect("probe local coordinates must be in bounds");
    let column_origin = chunk_local_to_world(coord, local);
    let world_x = column_origin.0;
    let world_z = column_origin.2;
    let atlas_coord = AtlasCoord::new(
        world_x.div_euclid(ATLAS_CELL_SPAN_BLOCKS_I32),
        world_z.div_euclid(ATLAS_CELL_SPAN_BLOCKS_I32),
    );
    let atlas_frac_x =
        (world_x.rem_euclid(ATLAS_CELL_SPAN_BLOCKS_I32) as f32 + 0.5) / ATLAS_CELL_SPAN_BLOCKS_I32 as f32;
    let atlas_frac_z =
        (world_z.rem_euclid(ATLAS_CELL_SPAN_BLOCKS_I32) as f32 + 0.5) / ATLAS_CELL_SPAN_BLOCKS_I32 as f32;
    let atlas_sample = sample_column_atlas(atlas_fields, world_x, world_z);
    let land_threshold = AtlasTuning::default().normalization.land_threshold;
    let profile = resolve_profile(atlas_sample, land_threshold);
    let surface_y = surface_y_for_profile(seed, world_x, world_z, atlas_sample, profile);

    ColumnGenerationProbe {
        chunk: coord,
        local_x,
        local_z,
        world_x,
        world_z,
        atlas_coord,
        atlas_frac_x,
        atlas_frac_z,
        atlas_sample,
        profile,
        surface_y,
    }
}

fn dominant_profile(counts: TerrainProfileCounts) -> TerrainProfile {
    [
        (counts.deep_ocean, TerrainProfile::DeepOcean),
        (counts.shelf, TerrainProfile::Shelf),
        (counts.coast, TerrainProfile::Coast),
        (counts.plain, TerrainProfile::Plain),
        (counts.upland, TerrainProfile::Upland),
        (counts.ridge, TerrainProfile::Ridge),
    ]
    .into_iter()
    .max_by_key(|(count, _)| *count)
    .map(|(_, profile)| profile)
    .unwrap_or(TerrainProfile::Plain)
}
