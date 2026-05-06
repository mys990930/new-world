use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbaImage;
use rayon::prelude::*;

use new_world::ecs::QUARTER_VIEW_VERTICAL_WORLD_SIZE;
use new_world::renderer::{
    CpuMesh, MeshVertex, OffscreenRenderRequest, RenderBounds, RenderCameraState,
    RenderEnvironment, RenderMaterialKind, RenderProjectionMode, RenderTextureArraySource,
    RenderTextureSource, RenderTextureTile, RenderViewBasis, render_offscreen,
};
use new_world::world::generation::{
    BoundaryCache, BoundaryConfig, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS,
    GraphHydrologyGraph, GraphMacroMap, GraphRegionArea, GraphRegionCoord, HeightfieldColumn,
    HeightfieldConfig, HeightfieldTerrainKind, HeightfieldTile, MacroFieldTile,
    MacroFieldTileConfig, MacroMapConfig, VoronoiGraphConfig, VoronoiGraphPatch,
    VoronoiGraphPatchRequest, generate_heightfield_tile, generate_macro_field_tile,
    generate_macro_map, generate_noisy_boundaries, generate_voronoi_graph_patch,
    graph_region_for_world_block, solve_hydrology,
};
use new_world::world::{BlockFace, WorldMeta};

const DEFAULT_IMAGE_WIDTH: u32 = 1280;
const DEFAULT_IMAGE_HEIGHT: u32 = 720;
const DEFAULT_WORLD_SPAN_BLOCKS: i32 = 8192;
const DEFAULT_COLUMNS_X: u32 = 192;
const DEFAULT_VERTICAL_SCALE: f32 = 6.0;
const BASE_Y_BLOCKS: f32 = -56.0;
const WATER_ALPHA: f32 = 0.72;
const ISO_PREVIEW_CAMERA_DISTANCE: f32 = 520.0;

#[derive(Debug, Clone)]
struct PreviewConfig {
    seed: u64,
    center_x: i32,
    center_z: i32,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    region_size_blocks: i32,
    site_spacing_blocks: i32,
    land_bias: f32,
    columns_x: u32,
    columns_z: Option<u32>,
    quarter_turns: u8,
    vertical_scale: f32,
    output: Option<PathBuf>,
}

impl PreviewConfig {
    fn validate(self) -> Result<Self, Box<dyn Error>> {
        if self.width == 0 || self.height == 0 {
            return Err(cli_error("width and height must be positive"));
        }
        if self.world_span_blocks <= 0 {
            return Err(cli_error("world-span-blocks must be positive"));
        }
        if self.region_size_blocks <= 0 || self.site_spacing_blocks <= 0 {
            return Err(cli_error("region and site spacing must be positive"));
        }
        if self.columns_x == 0 || self.columns_z == Some(0) {
            return Err(cli_error("columns-x and columns-z must be positive"));
        }
        if !self.vertical_scale.is_finite() || self.vertical_scale <= 0.0 {
            return Err(cli_error("vertical-scale must be a positive finite number"));
        }
        Ok(self)
    }

    fn columns_z(&self) -> u32 {
        self.columns_z.unwrap_or_else(|| {
            ((self.columns_x as f32 * self.height as f32 / self.width as f32)
                .round()
                .max(1.0)) as u32
        })
    }

    fn window(&self) -> PreviewWindow {
        let span_x = self.world_span_blocks as f32;
        let span_z = span_x * self.columns_z() as f32 / self.columns_x as f32;
        PreviewWindow {
            center_x: self.center_x as f32,
            center_z: self.center_z as f32,
            columns_x: self.columns_x,
            columns_z: self.columns_z(),
            world_span_x: span_x,
            world_span_z: span_z,
        }
    }

    fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            PathBuf::from(format!(
                "target/heightfield-preview/s{}_x{}_z{}.png",
                self.seed, self.center_x, self.center_z
            ))
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct PreviewWindow {
    center_x: f32,
    center_z: f32,
    columns_x: u32,
    columns_z: u32,
    world_span_x: f32,
    world_span_z: f32,
}

impl PreviewWindow {
    fn min_x(self) -> f32 {
        self.center_x - self.world_span_x * 0.5
    }

    fn max_x(self) -> f32 {
        self.center_x + self.world_span_x * 0.5
    }

    fn min_z(self) -> f32 {
        self.center_z - self.world_span_z * 0.5
    }

    fn max_z(self) -> f32 {
        self.center_z + self.world_span_z * 0.5
    }

    fn sample_spacing(self) -> f32 {
        self.world_span_x / self.columns_x as f32
    }

    fn graph_area(self, region_size_blocks: i32) -> Result<GraphRegionArea, Box<dyn Error>> {
        let min = graph_region_for_world_block(
            self.min_x().floor() as i32,
            self.min_z().floor() as i32,
            region_size_blocks,
        );
        let max = graph_region_for_world_block(
            self.max_x().ceil() as i32,
            self.max_z().ceil() as i32,
            region_size_blocks,
        );
        GraphRegionArea::new(min, max).ok_or_else(|| cli_error("invalid heightfield preview area"))
    }
}

#[derive(Debug, Clone)]
struct PreviewHeader {
    seed: u64,
    generator_version: u32,
    center_x: i32,
    center_z: i32,
    width: u32,
    height: u32,
    world_span_blocks: i32,
    columns_x: u32,
    columns_z: u32,
    sample_spacing_blocks: f32,
    vertical_scale: f32,
    graph_site_count: usize,
    macro_sample_count: usize,
    column_count: usize,
    min_surface: f32,
    avg_surface: f32,
    max_surface: f32,
    water_columns: usize,
    ocean_columns: usize,
    lake_columns: usize,
    river_columns: usize,
    dry_basin_columns: usize,
    ridge_columns: usize,
    build_ms: u128,
    macro_field_ms: u128,
    heightfield_ms: u128,
    mesh_ms: u128,
    render_ms: u128,
    total_ms: u128,
}

impl PreviewHeader {
    fn to_metadata_text(&self) -> String {
        [
            "stage=heightfield".to_string(),
            format!("seed={}", self.seed),
            format!("generator_version={}", self.generator_version),
            format!("center={},{}", self.center_x, self.center_z),
            format!("image={}x{}", self.width, self.height),
            format!("world_span_blocks={}", self.world_span_blocks),
            format!("columns={}x{}", self.columns_x, self.columns_z),
            format!("sample_spacing_blocks={:.3}", self.sample_spacing_blocks),
            format!("vertical_scale={:.3}", self.vertical_scale),
            "view=isometric".to_string(),
            "projection=orthographic_true_isometric".to_string(),
            format!("graph_sites={}", self.graph_site_count),
            format!("macro_samples={}", self.macro_sample_count),
            format!("heightfield_columns={}", self.column_count),
            format!(
                "surface_min_avg_max_blocks={:.3},{:.3},{:.3}",
                self.min_surface, self.avg_surface, self.max_surface
            ),
            format!(
                "water_ocean_lake_river_dry_ridge_columns={},{},{},{},{},{}",
                self.water_columns,
                self.ocean_columns,
                self.lake_columns,
                self.river_columns,
                self.dry_basin_columns,
                self.ridge_columns
            ),
            "meso_delta_blocks=0".to_string(),
            "micro_relief_blocks=0".to_string(),
            "height_mapping=combined_macro_height_-0.75_to_1.25_maps_-48_to_160_blocks".to_string(),
            format!(
                "timing_ms=build:{} macro_field:{} heightfield:{} mesh:{} render:{} total:{}",
                self.build_ms,
                self.macro_field_ms,
                self.heightfield_ms,
                self.mesh_ms,
                self.render_ms,
                self.total_ms
            ),
            "meaning=diagnostic_voxelized_heightfield_columns_not_final_chunkdata".to_string(),
        ]
        .join("\n")
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let total_start = Instant::now();
    let config = parse_args()?.validate()?;
    let output = config.output_path();
    let meta = WorldMeta::new(config.seed);
    let window = config.window();
    let graph_area = window.graph_area(config.region_size_blocks)?;

    let build_start = Instant::now();
    let (patch, macro_map, hydrology, boundary) =
        build_generation_inputs(&meta, &config, graph_area)?;
    let build_ms = build_start.elapsed().as_millis();

    let macro_start = Instant::now();
    let macro_tile = build_macro_field_tile(window, &patch, &macro_map, &hydrology, &boundary);
    let macro_field_ms = macro_start.elapsed().as_millis();

    let heightfield_start = Instant::now();
    let heightfield = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
    let heightfield_ms = heightfield_start.elapsed().as_millis();

    let mesh_start = Instant::now();
    let meshes = build_heightfield_meshes(&heightfield, config.vertical_scale);
    let mesh_ms = mesh_start.elapsed().as_millis();
    if meshes.iter().all(|mesh| mesh.vertices.is_empty()) {
        return Err(cli_error(
            "heightfield preview produced no renderable meshes",
        ));
    }

    let render_start = Instant::now();
    let bounds = combined_render_bounds(&meshes).ok_or_else(|| cli_error("missing mesh bounds"))?;
    let camera = build_preview_camera(
        bounds,
        config.width,
        config.height,
        config.quarter_turns % 4,
    );
    let mut image = render_offscreen(OffscreenRenderRequest {
        width: config.width,
        height: config.height,
        camera,
        textures: white_render_textures(),
        environment: preview_environment(),
        chunk_meshes: meshes,
        clear_color_override: Some([0.045, 0.052, 0.060, 1.0]),
    })?;
    let render_ms = render_start.elapsed().as_millis();

    let total_ms = total_start.elapsed().as_millis();
    let header = PreviewHeader {
        seed: config.seed,
        generator_version: meta.generator_version,
        center_x: config.center_x,
        center_z: config.center_z,
        width: config.width,
        height: config.height,
        world_span_blocks: config.world_span_blocks,
        columns_x: window.columns_x,
        columns_z: window.columns_z,
        sample_spacing_blocks: window.sample_spacing(),
        vertical_scale: config.vertical_scale,
        graph_site_count: patch.sites.len(),
        macro_sample_count: macro_tile.samples.len(),
        column_count: heightfield.stats.column_count,
        min_surface: heightfield.stats.min_surface_height_blocks,
        avg_surface: heightfield.stats.average_surface_height_blocks,
        max_surface: heightfield.stats.max_surface_height_blocks,
        water_columns: heightfield.stats.water_column_count,
        ocean_columns: heightfield.stats.ocean_column_count,
        lake_columns: heightfield.stats.lake_column_count,
        river_columns: heightfield.stats.river_hint_column_count,
        dry_basin_columns: heightfield.stats.dry_basin_column_count,
        ridge_columns: heightfield.stats.ridge_column_count,
        build_ms,
        macro_field_ms,
        heightfield_ms,
        mesh_ms,
        render_ms,
        total_ms,
    };
    draw_overlay(&mut image, &header);
    write_rgba_png_with_metadata(&image, &output, &header)?;

    println!("heightfield preview: seed {}", config.seed);
    println!(
        "window: center=({}, {}), span={} blocks, columns={}x{}, spacing={:.2} blocks",
        config.center_x,
        config.center_z,
        config.world_span_blocks,
        window.columns_x,
        window.columns_z,
        window.sample_spacing()
    );
    println!(
        "view: isometric orthographic, quarter turns {}, vertical scale {:.2}",
        config.quarter_turns % 4,
        config.vertical_scale
    );
    println!(
        "surface height min/avg/max {:.2}/{:.2}/{:.2} blocks",
        heightfield.stats.min_surface_height_blocks,
        heightfield.stats.average_surface_height_blocks,
        heightfield.stats.max_surface_height_blocks
    );
    println!(
        "columns: total {}, water {}, ocean {}, lake {}, river {}, dry {}, ridge {}",
        heightfield.stats.column_count,
        heightfield.stats.water_column_count,
        heightfield.stats.ocean_column_count,
        heightfield.stats.lake_column_count,
        heightfield.stats.river_hint_column_count,
        heightfield.stats.dry_basin_column_count,
        heightfield.stats.ridge_column_count
    );
    println!(
        "timing: build {} ms, macro field {} ms, heightfield {} ms, mesh {} ms, render {} ms, total {} ms",
        build_ms, macro_field_ms, heightfield_ms, mesh_ms, render_ms, total_ms
    );
    println!("output: {}", output.display());
    println!("metadata: new-world-preview-header iTXt chunk");

    Ok(())
}

fn build_generation_inputs(
    meta: &WorldMeta,
    config: &PreviewConfig,
    graph_area: GraphRegionArea,
) -> Result<
    (
        VoronoiGraphPatch,
        GraphMacroMap,
        GraphHydrologyGraph,
        BoundaryCache,
    ),
    Box<dyn Error>,
> {
    let center_region =
        graph_region_for_world_block(config.center_x, config.center_z, config.region_size_blocks);
    let padding_regions = required_padding_regions(center_region, graph_area)?;
    let graph_config = VoronoiGraphConfig {
        seed: meta.seed,
        generator_version: meta.generator_version,
        region_size_blocks: config.region_size_blocks,
        site_spacing_blocks: config.site_spacing_blocks,
        padding_regions,
    };
    let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
        graph_config,
        config.center_x,
        config.center_z,
    ));
    let macro_map = generate_macro_map(
        &patch,
        MacroMapConfig {
            land_bias: config.land_bias,
            ..MacroMapConfig::new(meta.seed, meta.generator_version)
        },
    );
    let hydrology = solve_hydrology(&patch, &macro_map, Default::default());
    let boundary = generate_noisy_boundaries(
        &patch,
        &macro_map,
        BoundaryConfig::new(meta.seed, meta.generator_version),
    );

    Ok((patch, macro_map, hydrology, boundary))
}

fn required_padding_regions(
    center: GraphRegionCoord,
    area: GraphRegionArea,
) -> Result<u32, Box<dyn Error>> {
    let dx = (center.x - area.min.x)
        .abs()
        .max((area.max.x - center.x).abs());
    let dz = (center.z - area.min.z)
        .abs()
        .max((area.max.z - center.z).abs());
    u32::try_from(dx.max(dz).saturating_add(1))
        .map_err(|_| cli_error("heightfield preview padding overflowed"))
}

fn build_macro_field_tile(
    window: PreviewWindow,
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
    boundary: &BoundaryCache,
) -> MacroFieldTile {
    let sample_spacing = window.sample_spacing();
    let config = MacroFieldTileConfig::new(
        window.min_x() + sample_spacing * 0.5,
        window.min_z() + sample_spacing * 0.5,
        window.columns_x,
        window.columns_z,
        sample_spacing,
    );

    generate_macro_field_tile(patch, macro_map, hydrology, boundary, config)
}

fn build_heightfield_meshes(tile: &HeightfieldTile, vertical_scale: f32) -> Vec<CpuMesh> {
    let terrain_mesh = build_terrain_mesh(tile, vertical_scale);
    let water_mesh = build_water_mesh(tile, vertical_scale);
    [terrain_mesh, water_mesh]
        .into_iter()
        .filter(|mesh| !mesh.vertices.is_empty())
        .collect()
}

fn build_terrain_mesh(tile: &HeightfieldTile, vertical_scale: f32) -> CpuMesh {
    let width = tile.width as usize;
    let height = tile.height as usize;
    let spacing = tile.sample_spacing_blocks;
    let faces = (0..tile.columns.len())
        .into_par_iter()
        .map(|index| {
            let x = index % width;
            let z = index / width;
            let column = tile.columns[index];
            let min_x = column.position.x - spacing * 0.5;
            let max_x = column.position.x + spacing * 0.5;
            let min_z = column.position.z - spacing * 0.5;
            let max_z = column.position.z + spacing * 0.5;
            let top_y = column.surface_height_blocks.max(BASE_Y_BLOCKS + 1.0);
            let color = terrain_color(column);
            let mut local = Vec::with_capacity(5);
            local.push(BoxFace {
                min: [min_x, BASE_Y_BLOCKS, min_z],
                max: [max_x, top_y, max_z],
                face: BlockFace::PosY,
                color,
                material: RenderMaterialKind::GenericOpaque,
            });
            if x == 0 || tile.columns[index - 1].surface_height_blocks < top_y {
                let neighbor = if x == 0 {
                    BASE_Y_BLOCKS
                } else {
                    tile.columns[index - 1]
                        .surface_height_blocks
                        .max(BASE_Y_BLOCKS)
                };
                local.push(BoxFace {
                    min: [min_x, neighbor, min_z],
                    max: [max_x, top_y, max_z],
                    face: BlockFace::NegX,
                    color,
                    material: RenderMaterialKind::GenericOpaque,
                });
            }
            if x + 1 >= width || tile.columns[index + 1].surface_height_blocks < top_y {
                let neighbor = if x + 1 >= width {
                    BASE_Y_BLOCKS
                } else {
                    tile.columns[index + 1]
                        .surface_height_blocks
                        .max(BASE_Y_BLOCKS)
                };
                local.push(BoxFace {
                    min: [min_x, neighbor, min_z],
                    max: [max_x, top_y, max_z],
                    face: BlockFace::PosX,
                    color,
                    material: RenderMaterialKind::GenericOpaque,
                });
            }
            if z == 0 || tile.columns[index - width].surface_height_blocks < top_y {
                let neighbor = if z == 0 {
                    BASE_Y_BLOCKS
                } else {
                    tile.columns[index - width]
                        .surface_height_blocks
                        .max(BASE_Y_BLOCKS)
                };
                local.push(BoxFace {
                    min: [min_x, neighbor, min_z],
                    max: [max_x, top_y, max_z],
                    face: BlockFace::NegZ,
                    color,
                    material: RenderMaterialKind::GenericOpaque,
                });
            }
            if z + 1 >= height || tile.columns[index + width].surface_height_blocks < top_y {
                let neighbor = if z + 1 >= height {
                    BASE_Y_BLOCKS
                } else {
                    tile.columns[index + width]
                        .surface_height_blocks
                        .max(BASE_Y_BLOCKS)
                };
                local.push(BoxFace {
                    min: [min_x, neighbor, min_z],
                    max: [max_x, top_y, max_z],
                    face: BlockFace::PosZ,
                    color,
                    material: RenderMaterialKind::GenericOpaque,
                });
            }
            local
        })
        .reduce(Vec::new, |mut left, right| {
            left.extend(right);
            left
        });
    mesh_from_faces(&faces, vertical_scale)
}

fn build_water_mesh(tile: &HeightfieldTile, vertical_scale: f32) -> CpuMesh {
    let spacing = tile.sample_spacing_blocks;
    let faces = tile
        .columns
        .par_iter()
        .filter_map(|column| {
            let water = column.water_level_blocks?;
            if water <= column.surface_height_blocks {
                return None;
            }
            let min_x = column.position.x - spacing * 0.5;
            let max_x = column.position.x + spacing * 0.5;
            let min_z = column.position.z - spacing * 0.5;
            let max_z = column.position.z + spacing * 0.5;
            Some(BoxFace {
                min: [min_x, column.surface_height_blocks + 0.05, min_z],
                max: [max_x, water + 0.12, max_z],
                face: BlockFace::PosY,
                color: water_color(*column),
                material: RenderMaterialKind::Water,
            })
        })
        .collect::<Vec<_>>();
    mesh_from_faces(&faces, vertical_scale)
}

#[derive(Debug, Clone, Copy)]
struct BoxFace {
    min: [f32; 3],
    max: [f32; 3],
    face: BlockFace,
    color: [f32; 4],
    material: RenderMaterialKind,
}

fn mesh_from_faces(faces: &[BoxFace], vertical_scale: f32) -> CpuMesh {
    let mut mesh = CpuMesh::default();
    for face in faces {
        append_box_face(&mut mesh, *face, vertical_scale);
    }
    mesh
}

fn append_box_face(mesh: &mut CpuMesh, face: BoxFace, vertical_scale: f32) {
    if face.min[0] >= face.max[0] || face.min[1] >= face.max[1] || face.min[2] >= face.max[2] {
        return;
    }

    let positions = scaled_face_positions(face.min, face.max, face.face, vertical_scale);
    let base_index = mesh.vertices.len() as u32;
    let normal = face_normal(face.face);
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    extend_render_bounds(&mut mesh.bounds, &positions);

    for (position, uv) in positions.into_iter().zip(uv) {
        mesh.vertices.push(MeshVertex {
            position,
            color: face.color,
            normal,
            uv,
            texture_layer: 0,
            material_kind: face.material.as_u32(),
            contour_edges: 0,
        });
    }

    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn scaled_face_positions(
    min: [f32; 3],
    max: [f32; 3],
    face: BlockFace,
    vertical_scale: f32,
) -> [[f32; 3]; 4] {
    let ty = |y: f32| BASE_Y_BLOCKS + (y - BASE_Y_BLOCKS) * vertical_scale;
    let min = [min[0], ty(min[1]), min[2]];
    let max = [max[0], ty(max[1]), max[2]];

    match face {
        BlockFace::NegX => [
            [min[0], min[1], min[2]],
            [min[0], min[1], max[2]],
            [min[0], max[1], max[2]],
            [min[0], max[1], min[2]],
        ],
        BlockFace::PosX => [
            [max[0], min[1], max[2]],
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
        ],
        BlockFace::NegY => [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], min[1], min[2]],
            [min[0], min[1], min[2]],
        ],
        BlockFace::PosY => [
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
        BlockFace::NegZ => [
            [max[0], min[1], min[2]],
            [min[0], min[1], min[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
        ],
        BlockFace::PosZ => [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
    }
}

fn face_normal(face: BlockFace) -> [f32; 3] {
    match face {
        BlockFace::NegX => [-1.0, 0.0, 0.0],
        BlockFace::PosX => [1.0, 0.0, 0.0],
        BlockFace::NegY => [0.0, -1.0, 0.0],
        BlockFace::PosY => [0.0, 1.0, 0.0],
        BlockFace::NegZ => [0.0, 0.0, -1.0],
        BlockFace::PosZ => [0.0, 0.0, 1.0],
    }
}

fn terrain_color(column: HeightfieldColumn) -> [f32; 4] {
    let t = ((column.combined_macro_height + 0.75) / 2.0).clamp(0.0, 1.0);
    let mut color = match column.terrain_kind {
        HeightfieldTerrainKind::Ocean => rgb8([45, 78, 102]),
        HeightfieldTerrainKind::Lake => rgb8([55, 100, 124]),
        HeightfieldTerrainKind::River => rgb8([63, 109, 122]),
        HeightfieldTerrainKind::DryBasin => rgb8([118, 111, 119]),
        HeightfieldTerrainKind::Ridge => rgb8([190, 190, 181]),
        HeightfieldTerrainKind::Coast => rgb8([138, 148, 118]),
        HeightfieldTerrainKind::Land => combined_terrain_ramp(t),
    };
    let altitude = (column.surface_height_blocks / 160.0).clamp(-0.2, 0.6);
    for channel in color.iter_mut().take(3) {
        *channel = (*channel * (0.86 + altitude * 0.20)).clamp(0.0, 1.0);
    }
    encode_vertex_tint(color, 0.16)
}

fn water_color(column: HeightfieldColumn) -> [f32; 4] {
    let base = if matches!(column.terrain_kind, HeightfieldTerrainKind::Lake) {
        [54, 118, 150]
    } else {
        [46, 92, 130]
    };
    let mut color = rgb8(base);
    color[3] = WATER_ALPHA;
    encode_vertex_tint(color, 0.16)
}

fn encode_vertex_tint(target: [f32; 4], tint_strength: f32) -> [f32; 4] {
    let keep = 1.0 - tint_strength;
    [
        ((target[0] - keep) / tint_strength).clamp(-8.0, 1.0),
        ((target[1] - keep) / tint_strength).clamp(-8.0, 1.0),
        ((target[2] - keep) / tint_strength).clamp(-8.0, 1.0),
        target[3],
    ]
}

fn combined_terrain_ramp(value: f32) -> [f32; 4] {
    let c = gradient_color(
        value,
        &[
            (0.00, [45, 78, 102]),
            (0.28, [67, 111, 123]),
            (0.38, [101, 130, 117]),
            (0.52, [112, 140, 102]),
            (0.68, [143, 145, 116]),
            (0.84, [168, 166, 151]),
            (1.00, [226, 228, 218]),
        ],
    );
    rgb8(c)
}

fn gradient_color(value: f32, stops: &[(f32, [u8; 3])]) -> [u8; 3] {
    let value = value.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let (left_t, left_color) = pair[0];
        let (right_t, right_color) = pair[1];
        if value <= right_t {
            let local = if (right_t - left_t).abs() <= f32::EPSILON {
                0.0
            } else {
                (value - left_t) / (right_t - left_t)
            };
            return [
                lerp_channel(left_color[0], right_color[0], local),
                lerp_channel(left_color[1], right_color[1], local),
                lerp_channel(left_color[2], right_color[2], local),
            ];
        }
    }
    stops.last().map_or([0, 0, 0], |(_, color)| *color)
}

fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn rgb8(color: [u8; 3]) -> [f32; 4] {
    [
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        1.0,
    ]
}

fn extend_render_bounds(bounds: &mut Option<RenderBounds>, positions: &[[f32; 3]; 4]) {
    for position in positions {
        match bounds {
            Some(bounds) => {
                bounds.min[0] = bounds.min[0].min(position[0]);
                bounds.min[1] = bounds.min[1].min(position[1]);
                bounds.min[2] = bounds.min[2].min(position[2]);
                bounds.max[0] = bounds.max[0].max(position[0]);
                bounds.max[1] = bounds.max[1].max(position[1]);
                bounds.max[2] = bounds.max[2].max(position[2]);
            }
            None => {
                *bounds = Some(RenderBounds {
                    min: *position,
                    max: *position,
                });
            }
        }
    }
}

fn combined_render_bounds(meshes: &[CpuMesh]) -> Option<RenderBounds> {
    let mut combined: Option<RenderBounds> = None;
    for mesh in meshes {
        let Some(bounds) = mesh.bounds else {
            continue;
        };
        combined = Some(match combined {
            Some(current) => RenderBounds {
                min: [
                    current.min[0].min(bounds.min[0]),
                    current.min[1].min(bounds.min[1]),
                    current.min[2].min(bounds.min[2]),
                ],
                max: [
                    current.max[0].max(bounds.max[0]),
                    current.max[1].max(bounds.max[1]),
                    current.max[2].max(bounds.max[2]),
                ],
            },
            None => bounds,
        });
    }
    combined
}

fn build_preview_camera(
    bounds: RenderBounds,
    width: u32,
    height: u32,
    quarter_turns: u8,
) -> RenderCameraState {
    let aspect = if height == 0 {
        1.0
    } else {
        width as f32 / height as f32
    };
    let basis = isometric_preview_basis(quarter_turns);
    let target = [
        (bounds.min[0] + bounds.max[0]) * 0.5,
        bounds.min[1] + (bounds.max[1] - bounds.min[1]) * 0.45,
        (bounds.min[2] + bounds.max[2]) * 0.5,
    ];
    let mut right_extent = 0.0_f32;
    let mut up_extent = 0.0_f32;
    for corner in bounds_corners(bounds) {
        let delta = [
            corner[0] - target[0],
            corner[1] - target[1],
            corner[2] - target[2],
        ];
        right_extent = right_extent.max(dot3(delta, basis.right).abs());
        up_extent = up_extent.max(dot3(delta, basis.up).abs());
    }
    let half_height =
        (up_extent.max(right_extent / aspect) * 1.12).max(QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.5);

    RenderCameraState {
        eye: add3(target, scale3(basis.forward, -ISO_PREVIEW_CAMERA_DISTANCE)),
        target,
        up: basis.up,
        aspect_override: Some(aspect),
        projection_mode: RenderProjectionMode::Orthographic {
            vertical_world_size: half_height * 2.0,
        },
        basis_override: Some(RenderViewBasis {
            right: basis.right,
            up: basis.up,
            forward: basis.forward,
        }),
    }
}

fn isometric_preview_basis(quarter_turns: u8) -> RenderViewBasis {
    let inv_sqrt_2 = std::f32::consts::FRAC_1_SQRT_2;
    let inv_sqrt_6 = 1.0 / 6.0_f32.sqrt();
    let right = rotate_y_quarter_turns([inv_sqrt_2, 0.0, -inv_sqrt_2], quarter_turns);
    let up = rotate_y_quarter_turns([inv_sqrt_6, 2.0 * inv_sqrt_6, inv_sqrt_6], quarter_turns);
    let forward = normalize3(cross3(right, up));

    RenderViewBasis { right, up, forward }
}

fn bounds_corners(bounds: RenderBounds) -> [[f32; 3]; 8] {
    [
        [bounds.min[0], bounds.min[1], bounds.min[2]],
        [bounds.max[0], bounds.min[1], bounds.min[2]],
        [bounds.min[0], bounds.max[1], bounds.min[2]],
        [bounds.max[0], bounds.max[1], bounds.min[2]],
        [bounds.min[0], bounds.min[1], bounds.max[2]],
        [bounds.max[0], bounds.min[1], bounds.max[2]],
        [bounds.min[0], bounds.max[1], bounds.max[2]],
        [bounds.max[0], bounds.max[1], bounds.max[2]],
    ]
}

fn white_render_textures() -> RenderTextureArraySource {
    RenderTextureArraySource {
        tile_size: 16,
        tiles: vec![RenderTextureTile {
            layer: 0,
            key: "diagnostic_white".to_string(),
            source: RenderTextureSource::BuiltinWhite,
        }],
    }
}

fn draw_overlay(image: &mut new_world::renderer::OffscreenRenderOutput, header: &PreviewHeader) {
    let Some(mut rgba) =
        RgbaImage::from_raw(image.width, image.height, std::mem::take(&mut image.rgba))
    else {
        return;
    };
    let scale = if image.width >= 1000 { 2 } else { 1 };
    draw_panel(&mut rgba, 8, 8, 218 * scale, 48 * scale);
    draw_text(&mut rgba, 16, 16, "ISO HEIGHT", [230, 235, 226, 255], scale);
    draw_text(
        &mut rgba,
        16,
        16 + 12 * scale,
        &format!("{}x{} COLS", header.columns_x, header.columns_z),
        [204, 214, 203, 255],
        scale,
    );
    draw_text(
        &mut rgba,
        16,
        16 + 24 * scale,
        &format!(
            "H {:.0}/{:.0}/{:.0}",
            header.min_surface, header.avg_surface, header.max_surface
        ),
        [204, 214, 203, 255],
        scale,
    );
    draw_legend_keys(&mut rgba, 16, 16 + 36 * scale, scale);
    image.rgba = rgba.into_raw();
}

fn draw_panel(image: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    let max_x = (x + w).min(image.width());
    let max_y = (y + h).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            blend_rgba(image, px, py, [7, 10, 13, 210], 0.72);
        }
    }
}

fn draw_legend_keys(image: &mut RgbaImage, x: u32, y: u32, scale: u32) {
    let keys = [
        ("WTR", [58, 120, 154, 255]),
        ("LOW", [101, 130, 117, 255]),
        ("HI", [190, 190, 181, 255]),
        ("DRY", [118, 111, 119, 255]),
    ];
    let mut cursor = x;
    for (label, color) in keys {
        let swatch = 5 * scale;
        for sy in 0..swatch {
            for sx in 0..swatch {
                set_rgba(image, cursor + sx, y + sy, color);
            }
        }
        draw_text(
            image,
            cursor + swatch + 2 * scale,
            y,
            label,
            [218, 224, 212, 255],
            scale,
        );
        cursor += swatch + 21 * scale;
    }
}

fn draw_text(image: &mut RgbaImage, x: u32, y: u32, text: &str, color: [u8; 4], scale: u32) {
    let mut cursor = x;
    for ch in text.chars() {
        draw_char(image, cursor, y, ch, color, scale);
        cursor = cursor.saturating_add(4 * scale);
    }
}

fn draw_char(image: &mut RgbaImage, x: u32, y: u32, ch: char, color: [u8; 4], scale: u32) {
    let glyph = glyph_3x5(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    set_rgba(
                        image,
                        x + col * scale + sx,
                        y + row as u32 * scale + sy,
                        color,
                    );
                }
            }
        }
    }
}

fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        ' ' => [0, 0, 0, 0, 0],
        _ => [0b111, 0b001, 0b010, 0b000, 0b010],
    }
}

fn set_rgba(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4]) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, image::Rgba(color));
    }
}

fn blend_rgba(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let base = image.get_pixel(x, y).0;
    let amount = amount.clamp(0.0, 1.0);
    let blended = [
        ((base[0] as f32 * (1.0 - amount)) + color[0] as f32 * amount) as u8,
        ((base[1] as f32 * (1.0 - amount)) + color[1] as f32 * amount) as u8,
        ((base[2] as f32 * (1.0 - amount)) + color[2] as f32 * amount) as u8,
        255,
    ];
    image.put_pixel(x, y, image::Rgba(blended));
}

fn write_rgba_png_with_metadata(
    image: &new_world::renderer::OffscreenRenderOutput,
    output: &Path,
    header: &PreviewHeader,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(output)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_itxt_chunk(
        "new-world-preview-header".to_string(),
        header.to_metadata_text(),
    )?;
    encoder.add_text_chunk(
        "Software".to_string(),
        "new-world heightfield_preview".to_string(),
    )?;
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.rgba)?;
    Ok(())
}

fn preview_environment() -> RenderEnvironment {
    let mut environment = RenderEnvironment::sunset_quarter_view();
    environment.time_of_day_hours = 15.0;
    environment.sun_direction = normalize3([0.42, 0.72, -0.55]);
    environment.sun_color = [0.92, 0.90, 0.84];
    environment.sun_intensity = 0.78;
    environment.ambient_color = [0.34, 0.37, 0.40];
    environment.ambient_intensity = 0.72;
    environment.fog_density = 0.0;
    environment.top_face_boost = 0.18;
    environment.side_shadow_strength = 0.55;
    environment.saturation_boost = 0.10;
    environment
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2])
        .sqrt()
        .max(f32::EPSILON);
    [value[0] / length, value[1] / length, value[2] / length]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn rotate_y_quarter_turns(value: [f32; 3], quarter_turns: u8) -> [f32; 3] {
    match quarter_turns % 4 {
        0 => value,
        1 => [value[2], value[1], -value[0]],
        2 => [-value[0], value[1], -value[2]],
        3 => [-value[2], value[1], value[0]],
        _ => unreachable!(),
    }
}

fn parse_args() -> Result<PreviewConfig, Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 3 {
        return Err(cli_error(usage()));
    }
    let seed = parse_required::<u64>(&mut args, "seed")?;
    let center_x = parse_required::<i32>(&mut args, "center-x")?;
    let center_z = parse_required::<i32>(&mut args, "center-z")?;
    let mut config = PreviewConfig {
        seed,
        center_x,
        center_z,
        width: DEFAULT_IMAGE_WIDTH,
        height: DEFAULT_IMAGE_HEIGHT,
        world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
        region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
        land_bias: MacroMapConfig::new(seed, WorldMeta::new(seed).generator_version).land_bias,
        columns_x: DEFAULT_COLUMNS_X,
        columns_z: None,
        quarter_turns: 0,
        vertical_scale: DEFAULT_VERTICAL_SCALE,
        output: None,
    };

    while !args.is_empty() {
        let flag = args.remove(0);
        match flag.as_str() {
            "--width" => config.width = parse_required::<u32>(&mut args, "width")?,
            "--height" => config.height = parse_required::<u32>(&mut args, "height")?,
            "--world-span-blocks" => {
                config.world_span_blocks = parse_required::<i32>(&mut args, "world-span-blocks")?
            }
            "--region-size-blocks" => {
                config.region_size_blocks = parse_required::<i32>(&mut args, "region-size-blocks")?
            }
            "--site-spacing-blocks" => {
                config.site_spacing_blocks =
                    parse_required::<i32>(&mut args, "site-spacing-blocks")?
            }
            "--land-bias" => config.land_bias = parse_required::<f32>(&mut args, "land-bias")?,
            "--columns-x" => config.columns_x = parse_required::<u32>(&mut args, "columns-x")?,
            "--columns-z" => {
                config.columns_z = Some(parse_required::<u32>(&mut args, "columns-z")?)
            }
            "--quarter-turns" => {
                config.quarter_turns = parse_required::<u8>(&mut args, "quarter-turns")?
            }
            "--vertical-scale" => {
                config.vertical_scale = parse_required::<f32>(&mut args, "vertical-scale")?
            }
            "--output" => {
                config.output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            "--stage" => {
                let stage = parse_required::<String>(&mut args, "stage")?;
                if stage != "heightfield" {
                    return Err(cli_error(
                        "heightfield_preview only supports --stage heightfield",
                    ));
                }
            }
            other => {
                return Err(cli_error(format!(
                    "unknown argument '{other}'\n{}",
                    usage()
                )));
            }
        }
    }

    Ok(config)
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing value for {label}")))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage: cargo run --bin heightfield_preview -- <seed> <center-x> <center-z> [--width <u32>] [--height <u32>] [--world-span-blocks <i32>] [--columns-x <u32>] [--columns-z <u32>] [--quarter-turns <u8>] [--vertical-scale <f32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(CliError(message.into()))
}

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_output_path_is_short() {
        let config = PreviewConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            width: DEFAULT_IMAGE_WIDTH,
            height: DEFAULT_IMAGE_HEIGHT,
            world_span_blocks: DEFAULT_WORLD_SPAN_BLOCKS,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            land_bias: 0.14,
            columns_x: DEFAULT_COLUMNS_X,
            columns_z: None,
            quarter_turns: 0,
            vertical_scale: DEFAULT_VERTICAL_SCALE,
            output: None,
        };

        assert_eq!(
            config.output_path(),
            PathBuf::from("target/heightfield-preview/s42_x0_z0.png")
        );
    }

    #[test]
    fn terrain_ramp_is_not_blank() {
        let low = combined_terrain_ramp(0.15);
        let high = combined_terrain_ramp(0.9);

        assert_ne!(low, high);
        assert!(low[2] > low[0]);
        assert!(high[0] > low[0]);
    }

    #[test]
    fn mesh_builder_emits_vertices_for_simple_column_tile() {
        let column = HeightfieldColumn {
            position: new_world::world::WorldPlanePoint::new(0.0, 0.0),
            surface_height_blocks: 12.0,
            surface_y: 12,
            water_level_blocks: None,
            water_y: None,
            terrain_kind: HeightfieldTerrainKind::Land,
            macro_elevation: 0.2,
            combined_macro_height: 0.2,
            ocean_mask: 0.0,
            lake_mask: 0.0,
            dry_basin_mask: 0.0,
            coast_mask: 0.0,
            ridge_influence: 0.0,
            river_valley_strength: 0.0,
            river_flow_hint: 0.0,
            meso_delta_blocks: 0.0,
            micro_relief_blocks: 0.0,
        };
        let tile = HeightfieldTile {
            width: 1,
            height: 1,
            sample_spacing_blocks: 32.0,
            columns: vec![column],
            stats: Default::default(),
            config: HeightfieldConfig::default(),
        };

        let mesh = build_terrain_mesh(&tile, 2.0);

        assert!(!mesh.vertices.is_empty());
        assert!(mesh.bounds.is_some());
    }

    #[test]
    fn isometric_basis_projects_world_axes_to_equal_lengths() {
        let basis = isometric_preview_basis(0);
        let x = projected_axis_length([1.0, 0.0, 0.0], basis);
        let y = projected_axis_length([0.0, 1.0, 0.0], basis);
        let z = projected_axis_length([0.0, 0.0, 1.0], basis);

        assert!((x - y).abs() < 1e-5);
        assert!((z - y).abs() < 1e-5);
        assert!(dot3([1.0, 0.0, 0.0], basis.right) > 0.0);
        assert!(dot3([0.0, 1.0, 0.0], basis.up) > 0.0);
    }

    #[test]
    fn isometric_camera_bounds_contain_all_corners() {
        let bounds = RenderBounds {
            min: [-64.0, -16.0, -48.0],
            max: [72.0, 128.0, 96.0],
        };
        let width = 1280;
        let height = 720;
        let camera = build_preview_camera(bounds, width, height, 0);
        let basis = camera.basis_override.expect("preview basis");
        let half_height = match camera.projection_mode {
            RenderProjectionMode::Orthographic {
                vertical_world_size,
            } => vertical_world_size * 0.5,
            RenderProjectionMode::Perspective => unreachable!(),
        };
        let half_width = half_height * width as f32 / height as f32;

        for corner in bounds_corners(bounds) {
            let delta = [
                corner[0] - camera.target[0],
                corner[1] - camera.target[1],
                corner[2] - camera.target[2],
            ];
            assert!(dot3(delta, basis.right).abs() <= half_width + 0.001);
            assert!(dot3(delta, basis.up).abs() <= half_height + 0.001);
        }
    }

    fn projected_axis_length(axis: [f32; 3], basis: RenderViewBasis) -> f32 {
        let right = dot3(axis, basis.right);
        let up = dot3(axis, basis.up);
        (right * right + up * up).sqrt()
    }
}
