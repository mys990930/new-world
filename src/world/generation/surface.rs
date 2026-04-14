use super::context::ColumnAtlasSample;
use super::profile::{TerrainProfile, resolve_profile};
use super::profiles::surface_height_for_sample;
use super::sampler::sample_column_atlas;
use super::super::atlas::{AtlasFieldMap, AtlasTuning};
use super::super::coord::{CHUNK_EDGE_I32, ChunkCoord};
use super::super::meta::WorldMeta;

const SURFACE_SMOOTH_RADIUS: i32 = 2;
const SURFACE_CONCAVITY_RADIUS: i32 = 1;
const SURFACE_FIELD_PADDING: i32 = SURFACE_SMOOTH_RADIUS + SURFACE_CONCAVITY_RADIUS;
const LOCAL_CONCAVITY_NORMALIZER: f32 = 3.0;
const SURFACE_SMOOTH_KERNEL: [[f32; 5]; 5] = [
    [1.0, 2.0, 3.0, 2.0, 1.0],
    [2.0, 4.0, 6.0, 4.0, 2.0],
    [3.0, 6.0, 9.0, 6.0, 3.0],
    [2.0, 4.0, 6.0, 4.0, 2.0],
    [1.0, 2.0, 3.0, 2.0, 1.0],
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreparedSurfaceColumn {
    pub atlas_sample: ColumnAtlasSample,
    pub profile: TerrainProfile,
    pub raw_surface_y: f32,
    pub surface_y: f32,
    pub local_concavity: f32,
}

impl Default for PreparedSurfaceColumn {
    fn default() -> Self {
        Self {
            atlas_sample: ColumnAtlasSample::default(),
            profile: TerrainProfile::Plain,
            raw_surface_y: 0.0,
            surface_y: 0.0,
            local_concavity: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ChunkSurfaceField {
    span: i32,
    padding: i32,
    columns: Vec<PreparedSurfaceColumn>,
}

impl ChunkSurfaceField {
    pub(super) fn column(&self, local_x: u8, local_z: u8) -> PreparedSurfaceColumn {
        self.column_i32(i32::from(local_x), i32::from(local_z))
    }

    fn column_i32(&self, local_x: i32, local_z: i32) -> PreparedSurfaceColumn {
        let grid_x = local_x + self.padding;
        let grid_z = local_z + self.padding;
        self.columns[self.index(grid_x, grid_z)]
    }

    pub(super) fn central_columns(&self) -> impl Iterator<Item = PreparedSurfaceColumn> + '_ {
        let field = self;
        (0..CHUNK_EDGE_I32).flat_map(move |local_z| {
            (0..CHUNK_EDGE_I32).map(move |local_x| field.column_i32(local_x, local_z))
        })
    }

    fn index(&self, grid_x: i32, grid_z: i32) -> usize {
        debug_assert!(grid_x >= 0 && grid_x < self.span);
        debug_assert!(grid_z >= 0 && grid_z < self.span);
        (grid_z as usize) * self.span as usize + grid_x as usize
    }
}

pub(super) fn build_chunk_surface_field(
    coord: ChunkCoord,
    meta: &WorldMeta,
    atlas_fields: &AtlasFieldMap,
) -> ChunkSurfaceField {
    let land_threshold = AtlasTuning::default().normalization.land_threshold;
    let span = CHUNK_EDGE_I32 + SURFACE_FIELD_PADDING * 2;
    let chunk_origin_x = coord.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = coord.2 * CHUNK_EDGE_I32;
    let mut columns = vec![PreparedSurfaceColumn::default(); (span * span) as usize];

    for grid_z in 0..span {
        let world_z = chunk_origin_z + grid_z - SURFACE_FIELD_PADDING;
        for grid_x in 0..span {
            let world_x = chunk_origin_x + grid_x - SURFACE_FIELD_PADDING;
            let atlas_sample = sample_column_atlas(atlas_fields, world_x, world_z);
            let profile = resolve_profile(atlas_sample, land_threshold);
            let raw_surface_y =
                surface_height_for_sample(meta.seed, world_x, world_z, atlas_sample, land_threshold);
            let index = (grid_z as usize) * span as usize + grid_x as usize;
            columns[index] = PreparedSurfaceColumn {
                atlas_sample,
                profile,
                raw_surface_y,
                surface_y: raw_surface_y,
                local_concavity: 0.0,
            };
        }
    }

    let raw_surface_y = columns.iter().map(|column| column.raw_surface_y).collect::<Vec<_>>();
    for grid_z in SURFACE_SMOOTH_RADIUS..(span - SURFACE_SMOOTH_RADIUS) {
        for grid_x in SURFACE_SMOOTH_RADIUS..(span - SURFACE_SMOOTH_RADIUS) {
            let mut weighted_sum = 0.0_f32;
            let mut total_weight = 0.0_f32;

            for kernel_z in -SURFACE_SMOOTH_RADIUS..=SURFACE_SMOOTH_RADIUS {
                for kernel_x in -SURFACE_SMOOTH_RADIUS..=SURFACE_SMOOTH_RADIUS {
                    let weight = SURFACE_SMOOTH_KERNEL
                        [(kernel_z + SURFACE_SMOOTH_RADIUS) as usize]
                        [(kernel_x + SURFACE_SMOOTH_RADIUS) as usize];
                    let sample_index = ((grid_z + kernel_z) as usize) * span as usize
                        + (grid_x + kernel_x) as usize;
                    weighted_sum += raw_surface_y[sample_index] * weight;
                    total_weight += weight;
                }
            }

            let index = (grid_z as usize) * span as usize + grid_x as usize;
            columns[index].surface_y = weighted_sum / total_weight.max(f32::EPSILON);
        }
    }

    for grid_z in SURFACE_CONCAVITY_RADIUS..(span - SURFACE_CONCAVITY_RADIUS) {
        for grid_x in SURFACE_CONCAVITY_RADIUS..(span - SURFACE_CONCAVITY_RADIUS) {
            let index = (grid_z as usize) * span as usize + grid_x as usize;
            let center = columns[index].surface_y;
            let mut neighbor_sum = 0.0_f32;
            let mut neighbor_count = 0_u32;

            for neighbor_z in -SURFACE_CONCAVITY_RADIUS..=SURFACE_CONCAVITY_RADIUS {
                for neighbor_x in -SURFACE_CONCAVITY_RADIUS..=SURFACE_CONCAVITY_RADIUS {
                    if neighbor_x == 0 && neighbor_z == 0 {
                        continue;
                    }

                    let neighbor_index = ((grid_z + neighbor_z) as usize) * span as usize
                        + (grid_x + neighbor_x) as usize;
                    neighbor_sum += columns[neighbor_index].surface_y;
                    neighbor_count += 1;
                }
            }

            let neighbor_mean = neighbor_sum / neighbor_count.max(1) as f32;
            columns[index].local_concavity =
                ((neighbor_mean - center) / LOCAL_CONCAVITY_NORMALIZER).clamp(0.0, 1.0);
        }
    }

    ChunkSurfaceField {
        span,
        padding: SURFACE_FIELD_PADDING,
        columns,
    }
}
