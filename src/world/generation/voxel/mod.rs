use rayon::prelude::*;

use crate::world::legacy::chunk::{BlockId, ChunkData};
use crate::world::legacy::coord::{
    CHUNK_EDGE, CHUNK_EDGE_I32, CHUNK_VOLUME, ChunkCoord, WorldBlockCoord, world_to_chunk_local,
};
use crate::world::legacy::meta::WorldMeta;
use crate::world::legacy::registry::BlockRegistry;

use super::boundary::{BoundaryConfig, generate_noisy_boundaries};
use super::graph::{
    DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, GraphRegionArea,
    VoronoiGraphConfig, VoronoiGraphPatchRequest, generate_voronoi_graph_patch,
    graph_region_for_world_block,
};
use super::heightfield::{HeightfieldConfig, HeightfieldPerlinConfig, generate_heightfield_tile};
use super::hydrology::solve_hydrology;
use super::macro_field::{MacroFieldTileConfig, generate_macro_field_tile};
use super::macro_map::{MacroMapConfig, generate_macro_map};
use super::pixelize::{
    PixelizeConfig, PixelizedChunkArea, PixelizedColumn, generate_pixelized_chunk_area,
};
use super::river_plan::{RiverPlanConfig, build_river_plan};
use super::surface_plan::{SurfaceColumnPlan, SurfacePlanConfig, generate_surface_plan_area};
use super::{DEFAULT_GRAPH_PADDING_REGIONS, apply_headwater_source_hydration_to_biomes};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphFirstVoxelBuildConfig {
    pub region_size_blocks: i32,
    pub site_spacing_blocks: i32,
    pub land_bias: f32,
    pub pixelize: PixelizeConfig,
    pub fill: GraphFirstVoxelFillConfig,
}

impl GraphFirstVoxelBuildConfig {
    pub fn new(seed: u64, generator_version: u32) -> Self {
        let mut config = Self::default();
        config.land_bias = MacroMapConfig::new(seed, generator_version).land_bias;
        config.pixelize.heightfield.perlin =
            HeightfieldPerlinConfig::preview_enabled(seed, generator_version);
        config
    }
}

impl Default for GraphFirstVoxelBuildConfig {
    fn default() -> Self {
        Self {
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: MacroMapConfig::new(0, 0).land_bias,
            pixelize: PixelizeConfig {
                heightfield: HeightfieldConfig {
                    perlin: HeightfieldPerlinConfig::preview_enabled(0, 0),
                    ..HeightfieldConfig::default()
                },
            },
            fill: GraphFirstVoxelFillConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphFirstVoxelFillConfig {
    pub terrain_block_key: &'static str,
    pub water_block_key: &'static str,
}

impl Default for GraphFirstVoxelFillConfig {
    fn default() -> Self {
        Self {
            terrain_block_key: "grass",
            water_block_key: "water",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphFirstVoxelColumnPlan {
    pub world_x: i32,
    pub world_z: i32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub local_x: u8,
    pub local_z: u8,
    pub terrain_top_y: i32,
    pub water_top_y: Option<i32>,
    pub top_block_key: &'static str,
    pub subsurface_block_key: &'static str,
    pub base_block_key: &'static str,
    pub underwater_top_block_key: &'static str,
    pub water_block_key: &'static str,
    pub soil_depth_blocks: u8,
}

impl GraphFirstVoxelColumnPlan {
    pub fn top_non_air_y(self) -> i32 {
        self.terrain_top_y.max(self.water_top_y.unwrap_or(i32::MIN))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphFirstVoxelPlan {
    pub min_chunk_x: i32,
    pub max_chunk_x: i32,
    pub min_chunk_z: i32,
    pub max_chunk_z: i32,
    pub width: u32,
    pub height: u32,
    pub columns: Vec<GraphFirstVoxelColumnPlan>,
    pub fill: GraphFirstVoxelFillConfig,
}

impl GraphFirstVoxelPlan {
    pub fn column(&self, x: u32, z: u32) -> Option<&GraphFirstVoxelColumnPlan> {
        if x >= self.width || z >= self.height {
            return None;
        }
        self.columns
            .get(z as usize * self.width as usize + x as usize)
    }

    pub fn column_for_chunk_local(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        local_x: u8,
        local_z: u8,
    ) -> Option<&GraphFirstVoxelColumnPlan> {
        if chunk_x < self.min_chunk_x
            || chunk_x > self.max_chunk_x
            || chunk_z < self.min_chunk_z
            || chunk_z > self.max_chunk_z
        {
            return None;
        }

        let x = (chunk_x - self.min_chunk_x) as u32 * CHUNK_EDGE as u32 + u32::from(local_x);
        let z = (chunk_z - self.min_chunk_z) as u32 * CHUNK_EDGE as u32 + u32::from(local_z);
        self.column(x, z)
    }

    pub fn columns_for_chunk_xz(
        &self,
        chunk_x: i32,
        chunk_z: i32,
    ) -> impl Iterator<Item = &GraphFirstVoxelColumnPlan> {
        (0..CHUNK_EDGE as u8).flat_map(move |local_z| {
            (0..CHUNK_EDGE as u8).filter_map(move |local_x| {
                self.column_for_chunk_local(chunk_x, chunk_z, local_x, local_z)
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphFirstVoxelError {
    InvalidChunkRange,
    MissingBlockKey(&'static str),
    MismatchedSurfacePlan,
}

impl std::fmt::Display for GraphFirstVoxelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChunkRange => f.write_str("invalid graph-first voxel chunk range"),
            Self::MissingBlockKey(key) => {
                write!(f, "graph-first voxel fill requires block key `{key}`")
            }
            Self::MismatchedSurfacePlan => {
                f.write_str("graph-first voxel fill requires matching heightfield/surface columns")
            }
        }
    }
}

impl std::error::Error for GraphFirstVoxelError {}

pub fn build_graph_first_voxel_plan(
    meta: &WorldMeta,
    min_chunk_x: i32,
    max_chunk_x: i32,
    min_chunk_z: i32,
    max_chunk_z: i32,
    config: GraphFirstVoxelBuildConfig,
) -> Result<GraphFirstVoxelPlan, GraphFirstVoxelError> {
    if min_chunk_x > max_chunk_x
        || min_chunk_z > max_chunk_z
        || config.region_size_blocks <= 0
        || config.site_spacing_blocks <= 0
    {
        return Err(GraphFirstVoxelError::InvalidChunkRange);
    }

    let min_world_x = min_chunk_x
        .checked_mul(CHUNK_EDGE_I32)
        .ok_or(GraphFirstVoxelError::InvalidChunkRange)?;
    let min_world_z = min_chunk_z
        .checked_mul(CHUNK_EDGE_I32)
        .ok_or(GraphFirstVoxelError::InvalidChunkRange)?;
    let max_world_x_exclusive = max_chunk_x
        .checked_add(1)
        .and_then(|value| value.checked_mul(CHUNK_EDGE_I32))
        .ok_or(GraphFirstVoxelError::InvalidChunkRange)?;
    let max_world_z_exclusive = max_chunk_z
        .checked_add(1)
        .and_then(|value| value.checked_mul(CHUNK_EDGE_I32))
        .ok_or(GraphFirstVoxelError::InvalidChunkRange)?;
    let width = u32::try_from(max_world_x_exclusive - min_world_x)
        .map_err(|_| GraphFirstVoxelError::InvalidChunkRange)?;
    let height = u32::try_from(max_world_z_exclusive - min_world_z)
        .map_err(|_| GraphFirstVoxelError::InvalidChunkRange)?;
    let center_world_x = min_world_x + (max_world_x_exclusive - min_world_x) / 2;
    let center_world_z = min_world_z + (max_world_z_exclusive - min_world_z) / 2;
    let max_world_x = max_world_x_exclusive - 1;
    let max_world_z = max_world_z_exclusive - 1;
    let graph_area = GraphRegionArea::new(
        graph_region_for_world_block(min_world_x, min_world_z, config.region_size_blocks),
        graph_region_for_world_block(max_world_x, max_world_z, config.region_size_blocks),
    )
    .ok_or(GraphFirstVoxelError::InvalidChunkRange)?;
    let center_region =
        graph_region_for_world_block(center_world_x, center_world_z, config.region_size_blocks);
    let padding_regions = required_padding_regions(center_region, graph_area);

    let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
        VoronoiGraphConfig {
            seed: meta.seed,
            generator_version: meta.generator_version,
            region_size_blocks: config.region_size_blocks,
            site_spacing_blocks: config.site_spacing_blocks,
            padding_regions,
        },
        center_world_x,
        center_world_z,
    ));
    let mut macro_map = generate_macro_map(
        &patch,
        MacroMapConfig {
            land_bias: config.land_bias,
            ..MacroMapConfig::new(meta.seed, meta.generator_version)
        },
    );
    let hydrology = solve_hydrology(&patch, &macro_map, Default::default());
    apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);
    let river_plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
    let boundary = generate_noisy_boundaries(
        &patch,
        &macro_map,
        BoundaryConfig::new(meta.seed, meta.generator_version),
    );
    let macro_tile = generate_macro_field_tile(
        &patch,
        &macro_map,
        &river_plan,
        &boundary,
        MacroFieldTileConfig::new(min_world_x as f32, min_world_z as f32, width, height, 1.0),
    );
    let heightfield = generate_heightfield_tile(&macro_tile, config.pixelize.heightfield);
    let surface = generate_surface_plan_area(
        &heightfield,
        Some(&macro_tile),
        SurfacePlanConfig::new(meta.seed, meta.generator_version),
    );
    let pixelized = generate_pixelized_chunk_area(&macro_tile, config.pixelize);

    build_graph_first_voxel_plan_from_pixelized_area_and_surface(&pixelized, &surface, config.fill)
}

pub fn build_graph_first_voxel_plan_from_pixelized_area(
    area: &PixelizedChunkArea,
    fill: GraphFirstVoxelFillConfig,
) -> GraphFirstVoxelPlan {
    let columns = area
        .columns
        .par_iter()
        .map(|column| graph_first_voxel_column_from_pixelized_column(*column, fill))
        .collect::<Vec<_>>();

    GraphFirstVoxelPlan {
        min_chunk_x: area.min_chunk_x,
        max_chunk_x: area.max_chunk_x,
        min_chunk_z: area.min_chunk_z,
        max_chunk_z: area.max_chunk_z,
        width: area.width,
        height: area.height,
        columns,
        fill,
    }
}

pub fn build_graph_first_voxel_plan_from_pixelized_area_and_surface(
    area: &PixelizedChunkArea,
    surface: &super::surface_plan::SurfacePlanArea,
    fill: GraphFirstVoxelFillConfig,
) -> Result<GraphFirstVoxelPlan, GraphFirstVoxelError> {
    if area.width != surface.width
        || area.height != surface.height
        || area.columns.len() != surface.columns.len()
    {
        return Err(GraphFirstVoxelError::MismatchedSurfacePlan);
    }

    let columns = area
        .columns
        .par_iter()
        .zip(surface.columns.par_iter())
        .map(|(pixel, surface)| graph_first_voxel_column_from_surface(*pixel, *surface, fill))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GraphFirstVoxelPlan {
        min_chunk_x: area.min_chunk_x,
        max_chunk_x: area.max_chunk_x,
        min_chunk_z: area.min_chunk_z,
        max_chunk_z: area.max_chunk_z,
        width: area.width,
        height: area.height,
        columns,
        fill,
    })
}

pub fn graph_first_voxel_column_from_pixelized_column(
    column: PixelizedColumn,
    fill: GraphFirstVoxelFillConfig,
) -> GraphFirstVoxelColumnPlan {
    let terrain_top_y = match column.water_y {
        Some(water_y) if water_y >= column.surface_y => column.surface_y.min(water_y - 1),
        _ => column.surface_y,
    };

    GraphFirstVoxelColumnPlan {
        world_x: column.world_x,
        world_z: column.world_z,
        chunk_x: column.chunk_x,
        chunk_z: column.chunk_z,
        local_x: column.local_x,
        local_z: column.local_z,
        terrain_top_y,
        water_top_y: column.water_y,
        top_block_key: fill.terrain_block_key,
        subsurface_block_key: fill.terrain_block_key,
        base_block_key: fill.terrain_block_key,
        underwater_top_block_key: fill.terrain_block_key,
        water_block_key: fill.water_block_key,
        soil_depth_blocks: 0,
    }
}

fn graph_first_voxel_column_from_surface(
    column: PixelizedColumn,
    surface: SurfaceColumnPlan,
    fill: GraphFirstVoxelFillConfig,
) -> Result<GraphFirstVoxelColumnPlan, GraphFirstVoxelError> {
    if column.world_x != surface.world_x
        || column.world_z != surface.world_z
        || column.surface_y != surface.surface_y
        || column.water_y != surface.water_y
    {
        return Err(GraphFirstVoxelError::MismatchedSurfacePlan);
    }

    let terrain_top_y = match surface.water_y {
        Some(water_y) if water_y >= surface.surface_y => surface.surface_y.min(water_y - 1),
        _ => surface.surface_y,
    };
    let (chunk, local) = world_to_chunk_local(WorldBlockCoord(column.world_x, 0, column.world_z));

    Ok(GraphFirstVoxelColumnPlan {
        world_x: column.world_x,
        world_z: column.world_z,
        chunk_x: chunk.0,
        chunk_z: chunk.2,
        local_x: local.x,
        local_z: local.z,
        terrain_top_y,
        water_top_y: surface.water_y,
        top_block_key: surface.top_block,
        subsurface_block_key: surface.subsurface_block,
        base_block_key: surface.base_block,
        underwater_top_block_key: surface.underwater_top_block,
        water_block_key: fill.water_block_key,
        soil_depth_blocks: surface.soil_depth_blocks,
    })
}

pub fn voxelize_graph_first_chunk(
    coord: ChunkCoord,
    plan: &GraphFirstVoxelPlan,
    registry: &BlockRegistry,
) -> Result<ChunkData, GraphFirstVoxelError> {
    let chunk_min_y = coord.1 * CHUNK_EDGE_I32;
    let mut blocks = vec![BlockId::AIR; CHUNK_VOLUME];

    for local_z in 0..CHUNK_EDGE as u8 {
        for local_x in 0..CHUNK_EDGE as u8 {
            let Some(column) = plan.column_for_chunk_local(coord.0, coord.2, local_x, local_z)
            else {
                continue;
            };
            let palette = VoxelColumnBlockIds::resolve(*column, registry)?;
            for local_y in 0..CHUNK_EDGE as u8 {
                let world_y = chunk_min_y + i32::from(local_y);
                blocks[linear_index(local_x, local_y, local_z)] =
                    block_for_world_y(world_y, *column, palette);
            }
        }
    }

    Ok(ChunkData::from_blocks(coord, blocks))
}

#[derive(Debug, Clone, Copy)]
struct VoxelColumnBlockIds {
    top: BlockId,
    subsurface: BlockId,
    base: BlockId,
    underwater_top: BlockId,
    water: BlockId,
}

impl VoxelColumnBlockIds {
    fn resolve(
        column: GraphFirstVoxelColumnPlan,
        registry: &BlockRegistry,
    ) -> Result<Self, GraphFirstVoxelError> {
        Ok(Self {
            top: block_id(registry, column.top_block_key)?,
            subsurface: block_id(registry, column.subsurface_block_key)?,
            base: block_id(registry, column.base_block_key)?,
            underwater_top: block_id(registry, column.underwater_top_block_key)?,
            water: block_id(registry, column.water_block_key)?,
        })
    }
}

fn block_id(registry: &BlockRegistry, key: &'static str) -> Result<BlockId, GraphFirstVoxelError> {
    registry
        .block_id(key)
        .ok_or(GraphFirstVoxelError::MissingBlockKey(key))
}

fn block_for_world_y(
    world_y: i32,
    column: GraphFirstVoxelColumnPlan,
    palette: VoxelColumnBlockIds,
) -> BlockId {
    if world_y <= column.terrain_top_y {
        if world_y == column.terrain_top_y {
            if column
                .water_top_y
                .is_some_and(|water_top_y| water_top_y > column.terrain_top_y)
            {
                palette.underwater_top
            } else {
                palette.top
            }
        } else if world_y >= column.terrain_top_y - i32::from(column.soil_depth_blocks) {
            palette.subsurface
        } else {
            palette.base
        }
    } else if column
        .water_top_y
        .is_some_and(|water_top_y| world_y <= water_top_y)
    {
        palette.water
    } else {
        BlockId::AIR
    }
}

fn linear_index(x: u8, y: u8, z: u8) -> usize {
    usize::from(x) + usize::from(z) * CHUNK_EDGE + usize::from(y) * CHUNK_EDGE * CHUNK_EDGE
}

fn required_padding_regions(center: super::graph::GraphRegionCoord, area: GraphRegionArea) -> u32 {
    let dx = (center.x - area.min.x)
        .abs()
        .max((area.max.x - center.x).abs());
    let dz = (center.z - area.min.z)
        .abs()
        .max((area.max.z - center.z).abs());
    u32::try_from(dx.max(dz).saturating_add(1))
        .unwrap_or(DEFAULT_GRAPH_PADDING_REGIONS)
        .max(DEFAULT_GRAPH_PADDING_REGIONS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::pixelize::PixelizedTerrainKind;

    #[test]
    fn graph_first_voxel_plan_from_pixelized_area_is_deterministic() {
        let area = synthetic_pixelized_area();
        let fill = GraphFirstVoxelFillConfig::default();

        let first = build_graph_first_voxel_plan_from_pixelized_area(&area, fill);
        let second = build_graph_first_voxel_plan_from_pixelized_area(&area, fill);

        assert_eq!(first, second);
    }

    #[test]
    fn graph_first_voxel_fill_reuses_columns_across_vertical_stack() {
        let registry = BlockRegistry::load_default().expect("default registry");
        let area = synthetic_pixelized_area();
        let plan = build_graph_first_voxel_plan_from_pixelized_area(
            &area,
            GraphFirstVoxelFillConfig::default(),
        );

        let lower =
            voxelize_graph_first_chunk(ChunkCoord(0, 0, 0), &plan, &registry).expect("lower chunk");
        let upper =
            voxelize_graph_first_chunk(ChunkCoord(0, 1, 0), &plan, &registry).expect("upper chunk");

        assert_eq!(
            lower.get_block(crate::world::legacy::coord::LocalBlockCoord::new(0, 10, 0).unwrap()),
            registry.block_id("grass")
        );
        assert_eq!(
            upper.get_block(crate::world::legacy::coord::LocalBlockCoord::new(0, 0, 0).unwrap()),
            Some(BlockId::AIR)
        );
    }

    #[test]
    fn graph_first_voxel_fill_places_water_grass_and_air() {
        let registry = BlockRegistry::load_default().expect("default registry");
        let area = synthetic_pixelized_area();
        let plan = build_graph_first_voxel_plan_from_pixelized_area(
            &area,
            GraphFirstVoxelFillConfig::default(),
        );
        let chunk =
            voxelize_graph_first_chunk(ChunkCoord(0, 0, 0), &plan, &registry).expect("chunk");
        let grass = registry.block_id("grass");
        let water = registry.block_id("water");

        assert_eq!(
            chunk.get_block(crate::world::legacy::coord::LocalBlockCoord::new(0, 10, 0).unwrap()),
            grass
        );
        assert_eq!(
            chunk.get_block(crate::world::legacy::coord::LocalBlockCoord::new(1, 0, 0).unwrap()),
            water
        );
        assert_eq!(
            chunk.get_block(crate::world::legacy::coord::LocalBlockCoord::new(0, 31, 0).unwrap()),
            Some(BlockId::AIR)
        );
    }

    #[test]
    fn voxel_column_plan_preserves_negative_ocean_bed_under_sea_level_water() {
        let mut column = synthetic_pixelized_area().columns[1];
        column.surface_y = -8;
        column.water_y = Some(0);

        let plan = graph_first_voxel_column_from_pixelized_column(
            column,
            GraphFirstVoxelFillConfig::default(),
        );

        assert_eq!(plan.terrain_top_y, -8);
        assert_eq!(plan.water_top_y, Some(0));
        assert_eq!(plan.top_non_air_y(), 0);
    }

    fn synthetic_pixelized_area() -> PixelizedChunkArea {
        let columns = (0..CHUNK_EDGE)
            .flat_map(|z| {
                (0..CHUNK_EDGE).map(move |x| {
                    let water = x == 1 && z == 0;
                    PixelizedColumn {
                        world_x: x as i32,
                        world_z: z as i32,
                        chunk_x: 0,
                        chunk_z: 0,
                        local_x: x as u8,
                        local_z: z as u8,
                        surface_y: if water { 0 } else { 10 },
                        water_y: water.then_some(0),
                        terrain_kind: if water {
                            PixelizedTerrainKind::Ocean
                        } else {
                            PixelizedTerrainKind::Land
                        },
                        source_macro_elevation: 0.0,
                        source_combined_macro_height: 0.0,
                        source_ocean_mask: if water { 1.0 } else { 0.0 },
                        source_lake_mask: 0.0,
                        source_coast_mask: 0.0,
                        source_dry_basin_mask: 0.0,
                        source_ridge_influence: 0.0,
                        source_terrain_ruggedness: 0.0,
                        source_river_valley_strength: 0.0,
                        source_river_flow_hint: 0.0,
                    }
                })
            })
            .collect::<Vec<_>>();

        PixelizedChunkArea {
            origin_world_x: 0,
            origin_world_z: 0,
            width: CHUNK_EDGE as u32,
            height: CHUNK_EDGE as u32,
            min_chunk_x: 0,
            max_chunk_x: 0,
            min_chunk_z: 0,
            max_chunk_z: 0,
            columns,
            stats: Default::default(),
            config: PixelizeConfig::default(),
        }
    }
}
