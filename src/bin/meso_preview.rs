use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use image::{Rgb, RgbImage};
use rayon::prelude::*;

use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCell, AtlasCoord, BaseHeightfieldPrototype, CHUNK_EDGE,
    CHUNK_EDGE_I32, ChunkCoord, MesoGuideCell, MesoGuideMap, MesoGuideSample, PrototypeColumn,
    RegionClassSample, WorldMeta, build_chunk_meso_applied_prototype, build_chunk_v2_scaffold,
    empty_chunk_corridor_window, region_archetype_def, sample_meso_guides,
};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 4;
const DEFAULT_BLOCKS_PER_PIXEL: u32 = 1;
const DEFAULT_BASE_HEIGHT: f32 = 48.0;
const DEFAULT_RELIEF_BUDGET: f32 = 24.0;
const DEFAULT_CONTOUR_STEP: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreviewWindow {
    center_x: i32,
    center_z: i32,
    radius: i32,
    blocks_per_pixel: u32,
}

impl PreviewWindow {
    fn new(
        center_x: i32,
        center_z: i32,
        radius: i32,
        blocks_per_pixel: u32,
    ) -> Result<Self, Box<dyn Error>> {
        if radius < 0 {
            return Err(cli_error("radius must be non-negative"));
        }
        if blocks_per_pixel == 0 {
            return Err(cli_error("blocks-per-pixel must be >= 1"));
        }
        if blocks_per_pixel > CHUNK_EDGE_I32 as u32 {
            return Err(cli_error(format!(
                "blocks-per-pixel must be <= CHUNK_EDGE ({CHUNK_EDGE_I32})"
            )));
        }
        if CHUNK_EDGE_I32 % blocks_per_pixel as i32 != 0 {
            return Err(cli_error(format!(
                "blocks-per-pixel must evenly divide CHUNK_EDGE ({CHUNK_EDGE_I32})"
            )));
        }

        Ok(Self {
            center_x,
            center_z,
            radius,
            blocks_per_pixel,
        })
    }

    fn chunk_span(self) -> Result<u32, Box<dyn Error>> {
        let span = self
            .radius
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| cli_error("radius produced an invalid chunk span"))?;
        u32::try_from(span).map_err(|_| cli_error("radius produced an invalid chunk span"))
    }

    fn pixels_per_chunk(self) -> u32 {
        CHUNK_EDGE_I32 as u32 / self.blocks_per_pixel
    }

    fn image_dimensions(self) -> Result<(u32, u32), Box<dyn Error>> {
        let chunk_span = self.chunk_span()?;
        let pixels_per_chunk = self.pixels_per_chunk();
        let width = chunk_span
            .checked_mul(pixels_per_chunk)
            .ok_or_else(|| cli_error("preview width overflowed"))?;
        let height = chunk_span
            .checked_mul(pixels_per_chunk)
            .ok_or_else(|| cli_error("preview height overflowed"))?;
        Ok((width, height))
    }

    fn min_chunk_x(self) -> i32 {
        self.center_x - self.radius
    }

    fn max_chunk_x(self) -> i32 {
        self.center_x + self.radius
    }

    fn min_chunk_z(self) -> i32 {
        self.center_z - self.radius
    }

    fn max_chunk_z(self) -> i32 {
        self.center_z + self.radius
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewFeature {
    All,
    HillCluster,
    ShallowBasin,
    EscarpmentBand,
    UplandTerrace,
}

impl PreviewFeature {
    fn parse(value: &str) -> Option<Self> {
        match canonical_key(value).as_str() {
            "all" => Some(Self::All),
            "hillcluster" => Some(Self::HillCluster),
            "shallowbasin" => Some(Self::ShallowBasin),
            "escarpmentband" => Some(Self::EscarpmentBand),
            "uplandterrace" => Some(Self::UplandTerrace),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::HillCluster => "hill_cluster",
            Self::ShallowBasin => "shallow_basin",
            Self::EscarpmentBand => "escarpment_band",
            Self::UplandTerrace => "upland_terrace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorridorMode {
    None,
    Live,
}

impl CorridorMode {
    fn parse(value: &str) -> Option<Self> {
        match canonical_key(value).as_str() {
            "none" | "off" => Some(Self::None),
            "live" | "on" => Some(Self::Live),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Live => "live",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PreviewConfig {
    feature: PreviewFeature,
    corridor_mode: CorridorMode,
    base_height: f32,
    relief_budget: f32,
    contour_step: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewPixel {
    surface_height: f32,
    delta: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DeltaSummary {
    min_delta: f32,
    max_delta: f32,
    mean_delta: f32,
    nonzero_count: usize,
    positive_count: usize,
    negative_count: usize,
    sample_count: usize,
}

#[derive(Debug, Clone)]
struct ChunkPreviewPatch {
    coord: ChunkCoord,
    pixels_per_chunk: u32,
    cells: Vec<PreviewPixel>,
    summary: DeltaSummary,
    applied_feature_keys: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct CenterChunkDiagnostics {
    atlas_coord: AtlasCoord,
    atlas_cell: AtlasCell,
    region: RegionClassSample,
    source_meso: MesoGuideSample,
    filtered_meso: MesoGuideSample,
    archetype_summary: Option<&'static str>,
    allowed_meso_keys: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewSummary {
    delta: DeltaSummary,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut center_x = DEFAULT_CENTER_X;
    let mut center_z = DEFAULT_CENTER_Z;
    let mut radius = DEFAULT_RADIUS;
    let mut blocks_per_pixel = DEFAULT_BLOCKS_PER_PIXEL;
    let mut feature = PreviewFeature::HillCluster;
    let mut corridor_mode = CorridorMode::None;
    let mut base_height = DEFAULT_BASE_HEIGHT;
    let mut relief_budget = DEFAULT_RELIEF_BUDGET;
    let mut contour_step = DEFAULT_CONTOUR_STEP;
    let mut output: Option<PathBuf> = None;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-x" | "--chunk-x" => {
                center_x = parse_required::<i32>(&mut args, "center-x")?;
            }
            "--center-z" | "--chunk-z" => {
                center_z = parse_required::<i32>(&mut args, "center-z")?;
            }
            "--radius" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--blocks-per-pixel" => {
                blocks_per_pixel = parse_required::<u32>(&mut args, "blocks-per-pixel")?
            }
            "--feature" => {
                let value = parse_required::<String>(&mut args, "feature")?;
                feature = PreviewFeature::parse(&value).ok_or_else(|| {
                    cli_error(format!(
                        "unknown feature '{value}'; expected all, hill_cluster, shallow_basin, escarpment_band, or upland_terrace"
                    ))
                })?;
            }
            "--corridors" => {
                let value = parse_required::<String>(&mut args, "corridors")?;
                corridor_mode = CorridorMode::parse(&value).ok_or_else(|| {
                    cli_error(format!(
                        "unknown corridor mode '{value}'; expected none or live"
                    ))
                })?;
            }
            "--base-height" => base_height = parse_required::<f32>(&mut args, "base-height")?,
            "--relief-budget" => {
                relief_budget = parse_required::<f32>(&mut args, "relief-budget")?
            }
            "--contour-step" => contour_step = parse_required::<f32>(&mut args, "contour-step")?,
            "--output" => output = Some(PathBuf::from(parse_required::<String>(&mut args, "output")?)),
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if relief_budget <= 0.0 {
        return Err(cli_error("relief-budget must be > 0"));
    }
    if contour_step <= 0.0 {
        return Err(cli_error("contour-step must be > 0"));
    }

    let window = PreviewWindow::new(center_x, center_z, radius, blocks_per_pixel)?;
    let config = PreviewConfig {
        feature,
        corridor_mode,
        base_height,
        relief_budget,
        contour_step,
    };
    let output = output.unwrap_or_else(|| {
        default_output_path(
            seed,
            window.center_x,
            window.center_z,
            window.radius,
            window.blocks_per_pixel,
            config.feature,
            config.corridor_mode,
        )
    });

    let meta = WorldMeta::new(seed);
    let diagnostics = collect_center_chunk_diagnostics(
        ChunkCoord(window.center_x, 0, window.center_z),
        &meta,
        config.feature,
    )?;
    let patches = build_preview_patches(&meta, window, config);
    let center_patch = patches
        .iter()
        .find(|patch| patch.coord.0 == window.center_x && patch.coord.2 == window.center_z)
        .ok_or_else(|| cli_error("center chunk patch was not generated"))?;
    let summary = summarize_preview(&patches);
    let image = render_preview(window, config, &patches, summary)?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&output)?;

    println!("seed: {seed}");
    println!("center chunk: ({}, {})", window.center_x, window.center_z);
    println!(
        "mode: feature={}, corridors={}, base_height={:.1}, relief_budget={:.1}, contour_step={:.1}",
        config.feature.key(),
        config.corridor_mode.key(),
        config.base_height,
        config.relief_budget,
        config.contour_step,
    );
    println!(
        "footprint: chunks x={}..{}, z={}..{}, radius={}",
        window.min_chunk_x(),
        window.max_chunk_x(),
        window.min_chunk_z(),
        window.max_chunk_z(),
        window.radius
    );
    println!(
        "scale: {} blocks/pixel ({} pixels/chunk)",
        window.blocks_per_pixel,
        window.pixels_per_chunk()
    );
    println!(
        "preview delta: min={:.2}, max={:.2}, mean={:.2}, nonzero_pixels={}, positive_pixels={}, negative_pixels={}",
        summary.delta.min_delta,
        summary.delta.max_delta,
        summary.delta.mean_delta,
        summary.delta.nonzero_count,
        summary.delta.positive_count,
        summary.delta.negative_count,
    );
    println!(
        "center chunk delta: min={:.2}, max={:.2}, mean={:.2}, applied_features={}",
        center_patch.summary.min_delta,
        center_patch.summary.max_delta,
        center_patch.summary.mean_delta,
        if center_patch.applied_feature_keys.is_empty() {
            String::from("(none)")
        } else {
            center_patch.applied_feature_keys.join(", ")
        }
    );
    print_center_chunk_diagnostics(&diagnostics);
    println!("output: {}", output.display());
    println!("image: {}x{}", image.width(), image.height());

    Ok(())
}

fn build_preview_patches(
    meta: &WorldMeta,
    window: PreviewWindow,
    config: PreviewConfig,
) -> Vec<ChunkPreviewPatch> {
    preview_chunk_xz_coords(window.center_x, window.center_z, window.radius)
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            build_chunk_preview_patch(
                ChunkCoord(chunk_x, 0, chunk_z),
                meta,
                window.blocks_per_pixel,
                config,
            )
        })
        .collect()
}

fn build_chunk_preview_patch(
    chunk: ChunkCoord,
    meta: &WorldMeta,
    blocks_per_pixel: u32,
    config: PreviewConfig,
) -> ChunkPreviewPatch {
    let scaffold = build_chunk_v2_scaffold(chunk, meta);
    let mut inputs = scaffold.inputs.clone();
    filter_meso_guides(&mut inputs.meso_guides, config.feature);
    let corridor_window = match config.corridor_mode {
        CorridorMode::None => empty_chunk_corridor_window(chunk),
        CorridorMode::Live => scaffold.corridor_window.clone(),
    };
    let prototype = flat_base_prototype(chunk, config.base_height, config.relief_budget);
    let meso = build_chunk_meso_applied_prototype(chunk, &inputs, &corridor_window, &prototype);
    let pixels_per_chunk = CHUNK_EDGE_I32 as u32 / blocks_per_pixel;
    let mut cells = Vec::with_capacity((pixels_per_chunk * pixels_per_chunk) as usize);

    for pixel_z in 0..pixels_per_chunk {
        let block_z_start = pixel_z * blocks_per_pixel;
        for pixel_x in 0..pixels_per_chunk {
            let block_x_start = pixel_x * blocks_per_pixel;
            let mut height_sum = 0.0_f32;
            let mut delta_sum = 0.0_f32;
            let mut sample_count = 0_u32;

            for block_offset_z in 0..blocks_per_pixel {
                let local_z = block_z_start + block_offset_z;
                for block_offset_x in 0..blocks_per_pixel {
                    let local_x = block_x_start + block_offset_x;
                    let index = local_z as usize * CHUNK_EDGE + local_x as usize;
                    let surface_height = meso.columns[index].height;
                    height_sum += surface_height;
                    delta_sum += surface_height - config.base_height;
                    sample_count += 1;
                }
            }

            let divisor = sample_count.max(1) as f32;
            cells.push(PreviewPixel {
                surface_height: height_sum / divisor,
                delta: delta_sum / divisor,
            });
        }
    }

    ChunkPreviewPatch {
        coord: chunk,
        pixels_per_chunk,
        summary: summarize_patch(&cells),
        cells,
        applied_feature_keys: meso.applied_feature_keys,
    }
}

fn summarize_patch(cells: &[PreviewPixel]) -> DeltaSummary {
    let mut min_delta = f32::INFINITY;
    let mut max_delta = f32::NEG_INFINITY;
    let mut sum_delta = 0.0_f32;
    let mut nonzero_count = 0_usize;
    let mut positive_count = 0_usize;
    let mut negative_count = 0_usize;

    for cell in cells {
        min_delta = min_delta.min(cell.delta);
        max_delta = max_delta.max(cell.delta);
        sum_delta += cell.delta;
        if cell.delta.abs() >= 0.05 {
            nonzero_count += 1;
        }
        if cell.delta > 0.05 {
            positive_count += 1;
        }
        if cell.delta < -0.05 {
            negative_count += 1;
        }
    }

    let sample_count = cells.len();
    let mean_delta = if sample_count == 0 {
        0.0
    } else {
        sum_delta / sample_count as f32
    };

    DeltaSummary {
        min_delta: if sample_count == 0 { 0.0 } else { min_delta },
        max_delta: if sample_count == 0 { 0.0 } else { max_delta },
        mean_delta,
        nonzero_count,
        positive_count,
        negative_count,
        sample_count,
    }
}

fn summarize_preview(patches: &[ChunkPreviewPatch]) -> PreviewSummary {
    let mut min_delta = f32::INFINITY;
    let mut max_delta = f32::NEG_INFINITY;
    let mut sum_delta = 0.0_f32;
    let mut nonzero_count = 0_usize;
    let mut positive_count = 0_usize;
    let mut negative_count = 0_usize;
    let mut sample_count = 0_usize;

    for patch in patches {
        for cell in &patch.cells {
            min_delta = min_delta.min(cell.delta);
            max_delta = max_delta.max(cell.delta);
            sum_delta += cell.delta;
            if cell.delta.abs() >= 0.05 {
                nonzero_count += 1;
            }
            if cell.delta > 0.05 {
                positive_count += 1;
            }
            if cell.delta < -0.05 {
                negative_count += 1;
            }
            sample_count += 1;
        }
    }

    PreviewSummary {
        delta: DeltaSummary {
            min_delta: if sample_count == 0 { 0.0 } else { min_delta },
            max_delta: if sample_count == 0 { 0.0 } else { max_delta },
            mean_delta: if sample_count == 0 {
                0.0
            } else {
                sum_delta / sample_count as f32
            },
            nonzero_count,
            positive_count,
            negative_count,
            sample_count,
        },
    }
}

fn render_preview(
    window: PreviewWindow,
    config: PreviewConfig,
    patches: &[ChunkPreviewPatch],
    summary: PreviewSummary,
) -> Result<RgbImage, Box<dyn Error>> {
    let (width, height) = window.image_dimensions()?;
    let mut image = RgbImage::new(width, height);
    let mut raster = vec![
        PreviewPixel {
            surface_height: config.base_height,
            delta: 0.0,
        };
        width as usize * height as usize
    ];
    let pixels_per_chunk = window.pixels_per_chunk();
    let min_chunk_x = window.min_chunk_x();
    let min_chunk_z = window.min_chunk_z();

    for patch in patches {
        debug_assert_eq!(patch.pixels_per_chunk, pixels_per_chunk);
        let chunk_offset_x = u32::try_from(patch.coord.0 - min_chunk_x)
            .map_err(|_| cli_error("preview chunk x offset overflowed"))?;
        let chunk_offset_z = u32::try_from(patch.coord.2 - min_chunk_z)
            .map_err(|_| cli_error("preview chunk z offset overflowed"))?;
        let pixel_origin_x = chunk_offset_x * pixels_per_chunk;
        let pixel_origin_z = chunk_offset_z * pixels_per_chunk;

        for local_z in 0..pixels_per_chunk {
            let row_offset = (pixel_origin_z + local_z) * width;
            for local_x in 0..pixels_per_chunk {
                let source_index = (local_z * pixels_per_chunk + local_x) as usize;
                let target_index = (row_offset + pixel_origin_x + local_x) as usize;
                raster[target_index] = patch.cells[source_index];
            }
        }
    }

    let delta_peak = summary
        .delta
        .min_delta
        .abs()
        .max(summary.delta.max_delta.abs())
        .max(1.0);
    let chunk_stride = pixels_per_chunk.max(1);

    for pixel_z in 0..height {
        for pixel_x in 0..width {
            let index = (pixel_z * width + pixel_x) as usize;
            let cell = raster[index];
            let shade = hillshade(&raster, width, height, pixel_x, pixel_z);
            let contour = contour_strength(&raster, width, height, pixel_x, pixel_z, config.contour_step);
            let grid = chunk_grid_strength(pixel_x, pixel_z, chunk_stride);
            let rgb = colorize_delta(cell.delta, delta_peak, shade, contour, grid);
            image.put_pixel(pixel_x, pixel_z, Rgb(rgb));
        }
    }

    Ok(image)
}

fn hillshade(
    cells: &[PreviewPixel],
    width: u32,
    height: u32,
    pixel_x: u32,
    pixel_z: u32,
) -> f32 {
    let sample = |x: i32, z: i32| -> f32 {
        let x = x.clamp(0, width.saturating_sub(1) as i32) as u32;
        let z = z.clamp(0, height.saturating_sub(1) as i32) as u32;
        cells[(z * width + x) as usize].surface_height
    };

    let x = pixel_x as i32;
    let z = pixel_z as i32;
    let dx = sample(x + 1, z) - sample(x - 1, z);
    let dz = sample(x, z + 1) - sample(x, z - 1);
    let normal = normalize3([-dx * 0.72, 2.2, -dz * 0.72]);
    let light = normalize3([-0.55, 0.80, -0.24]);
    let lambert = dot3(normal, light).clamp(-0.25, 1.0);
    (0.80 + lambert * 0.24).clamp(0.58, 1.18)
}

fn contour_strength(
    cells: &[PreviewPixel],
    width: u32,
    height: u32,
    pixel_x: u32,
    pixel_z: u32,
    contour_step: f32,
) -> f32 {
    let index = (pixel_z * width + pixel_x) as usize;
    let current_band = contour_band(cells[index].delta, contour_step);
    let mut edge = false;

    if pixel_x > 0 {
        edge |= contour_band(cells[(pixel_z * width + (pixel_x - 1)) as usize].delta, contour_step)
            != current_band;
    }
    if pixel_x + 1 < width {
        edge |= contour_band(cells[(pixel_z * width + (pixel_x + 1)) as usize].delta, contour_step)
            != current_band;
    }
    if pixel_z > 0 {
        edge |= contour_band(cells[((pixel_z - 1) * width + pixel_x) as usize].delta, contour_step)
            != current_band;
    }
    if pixel_z + 1 < height {
        edge |= contour_band(cells[((pixel_z + 1) * width + pixel_x) as usize].delta, contour_step)
            != current_band;
    }

    if edge { 0.26 } else { 0.0 }
}

fn contour_band(value: f32, contour_step: f32) -> i32 {
    (value / contour_step).floor() as i32
}

fn chunk_grid_strength(pixel_x: u32, pixel_z: u32, pixels_per_chunk: u32) -> f32 {
    if pixels_per_chunk == 0 {
        return 0.0;
    }

    let on_vertical = pixel_x % pixels_per_chunk == 0;
    let on_horizontal = pixel_z % pixels_per_chunk == 0;
    if on_vertical || on_horizontal { 0.14 } else { 0.0 }
}

fn colorize_delta(
    delta: f32,
    delta_peak: f32,
    shade: f32,
    contour_strength: f32,
    grid_strength: f32,
) -> [u8; 3] {
    let base = [216.0_f32, 209.0, 193.0];
    let positive_mid = [143.0_f32, 171.0, 118.0];
    let positive_peak = [225.0_f32, 186.0, 128.0];
    let negative_mid = [123.0_f32, 147.0, 181.0];
    let negative_peak = [66.0_f32, 104.0, 164.0];

    let magnitude = (delta.abs() / delta_peak).clamp(0.0, 1.0);
    let mut color = if delta >= 0.0 {
        let first = lerp_rgb(base, positive_mid, magnitude.sqrt());
        lerp_rgb(first, positive_peak, magnitude.powf(1.35))
    } else {
        let first = lerp_rgb(base, negative_mid, magnitude.sqrt());
        lerp_rgb(first, negative_peak, magnitude.powf(1.35))
    };

    let darken = (contour_strength + grid_strength).clamp(0.0, 0.45);
    let brighten = if delta.abs() >= 0.05 { 0.06 } else { 0.0 };
    let shade = (shade + brighten).clamp(0.0, 1.25);

    for channel in &mut color {
        *channel = (*channel * shade * (1.0 - darken)).clamp(0.0, 255.0);
    }

    [color[0] as u8, color[1] as u8, color[2] as u8]
}

fn collect_center_chunk_diagnostics(
    chunk: ChunkCoord,
    meta: &WorldMeta,
    feature: PreviewFeature,
) -> Result<CenterChunkDiagnostics, Box<dyn Error>> {
    let scaffold = build_chunk_v2_scaffold(chunk, meta);
    let (center_world_x, center_world_z) = center_chunk_sample_world_xz(chunk);
    let atlas_coord = atlas_coord_for_world_xz(center_world_x, center_world_z);
    let atlas_cell = scaffold
        .inputs
        .atlas_fields
        .get(atlas_coord)
        .copied()
        .ok_or_else(|| {
            cli_error(format!(
                "center atlas coord ({}, {}) fell outside the scaffold atlas field map",
                atlas_coord.x, atlas_coord.z
            ))
        })?;
    let source_meso = sample_meso_guides(&scaffold.inputs.meso_guides, center_world_x, center_world_z);
    let mut filtered_guides = scaffold.inputs.meso_guides.clone();
    filter_meso_guides(&mut filtered_guides, feature);
    let filtered_meso = sample_meso_guides(&filtered_guides, center_world_x, center_world_z);
    let archetype_def = region_archetype_def(scaffold.center_region.archetype);

    Ok(CenterChunkDiagnostics {
        atlas_coord,
        atlas_cell,
        region: scaffold.center_region,
        source_meso,
        filtered_meso,
        archetype_summary: archetype_def.map(|def| def.summary),
        allowed_meso_keys: archetype_def.map(|def| def.allowed_meso_keys).unwrap_or(&[]),
    })
}

fn print_center_chunk_diagnostics(diagnostics: &CenterChunkDiagnostics) {
    println!("center chunk diagnostics:");
    println!(
        "  region: archetype={:?}, biome={:?}, terrain={:?}, climate={:?}, hydrology={:?}, coastal={:?}",
        diagnostics.region.archetype,
        diagnostics.region.biome_family,
        diagnostics.region.terrain_form_family,
        diagnostics.region.climate_regime,
        diagnostics.region.hydrology_context,
        diagnostics.region.coastal_context,
    );
    println!(
        "  region axes: temperature={:?}, moisture={:?}, elevation={:?}, relief={:?}",
        diagnostics.region.temperature_band,
        diagnostics.region.moisture_band,
        diagnostics.region.elevation_band,
        diagnostics.region.relief_class,
    );
    if let Some(summary) = diagnostics.archetype_summary {
        println!("  archetype summary: {summary}");
    }
    if !diagnostics.allowed_meso_keys.is_empty() {
        println!("  allowed meso: {}", diagnostics.allowed_meso_keys.join(", "));
    }
    println!(
        "  atlas cell: ({}, {}), landness={:.3}, macro={:.3}, coast_distance={:.3}, ridge={:.3}, mountain={:.3}, basin={:.3}, river_flow={:.3}",
        diagnostics.atlas_coord.x,
        diagnostics.atlas_coord.z,
        diagnostics.atlas_cell.landness,
        diagnostics.atlas_cell.macro_elevation,
        diagnostics.atlas_cell.coast_distance,
        diagnostics.atlas_cell.ridge_factor,
        diagnostics.atlas_cell.mountain_mass,
        diagnostics.atlas_cell.basinness,
        diagnostics.atlas_cell.river_flow_potential,
    );
    println!(
        "  source meso: hilliness={:.3}, hill_height={:.2}, basin_weight={:.3}, basin_depth={:.2}, escarpment_weight={:.3}, escarpment_height={:.2}, terrace_weight={:.3}, terrace_step_height={:.2}",
        diagnostics.source_meso.hilliness,
        diagnostics.source_meso.hill_height,
        diagnostics.source_meso.basin_weight,
        diagnostics.source_meso.basin_depth,
        diagnostics.source_meso.escarpment_weight,
        diagnostics.source_meso.escarpment_height,
        diagnostics.source_meso.terrace_weight,
        diagnostics.source_meso.terrace_step_height,
    );
    println!(
        "  filtered meso: hilliness={:.3}, hill_height={:.2}, basin_weight={:.3}, basin_depth={:.2}, escarpment_weight={:.3}, escarpment_height={:.2}, terrace_weight={:.3}, terrace_step_height={:.2}",
        diagnostics.filtered_meso.hilliness,
        diagnostics.filtered_meso.hill_height,
        diagnostics.filtered_meso.basin_weight,
        diagnostics.filtered_meso.basin_depth,
        diagnostics.filtered_meso.escarpment_weight,
        diagnostics.filtered_meso.escarpment_height,
        diagnostics.filtered_meso.terrace_weight,
        diagnostics.filtered_meso.terrace_step_height,
    );
}

fn flat_base_prototype(
    chunk: ChunkCoord,
    base_height: f32,
    relief_budget: f32,
) -> BaseHeightfieldPrototype {
    BaseHeightfieldPrototype {
        chunk,
        columns: vec![
            PrototypeColumn {
                base_height,
                relief_budget,
            };
            CHUNK_EDGE * CHUNK_EDGE
        ],
    }
}

fn filter_meso_guides(guides: &mut MesoGuideMap, feature: PreviewFeature) {
    if matches!(feature, PreviewFeature::All) {
        return;
    }

    for cell in guides.cells_mut().values_mut() {
        *cell = retain_feature_channel(*cell, feature);
    }
}

fn retain_feature_channel(cell: MesoGuideCell, feature: PreviewFeature) -> MesoGuideCell {
    match feature {
        PreviewFeature::All => cell,
        PreviewFeature::HillCluster => MesoGuideCell {
            hilliness: cell.hilliness,
            hill_height: cell.hill_height,
            ..MesoGuideCell::default()
        },
        PreviewFeature::ShallowBasin => MesoGuideCell {
            basin_weight: cell.basin_weight,
            basin_depth: cell.basin_depth,
            ..MesoGuideCell::default()
        },
        PreviewFeature::EscarpmentBand => MesoGuideCell {
            escarpment_weight: cell.escarpment_weight,
            escarpment_height: cell.escarpment_height,
            escarpment_heading_x: cell.escarpment_heading_x,
            escarpment_heading_z: cell.escarpment_heading_z,
            escarpment_signed_distance_cells: cell.escarpment_signed_distance_cells,
            ..MesoGuideCell::default()
        },
        PreviewFeature::UplandTerrace => MesoGuideCell {
            terrace_weight: cell.terrace_weight,
            terrace_step_height: cell.terrace_step_height,
            terrace_spacing_cells: cell.terrace_spacing_cells,
            terrace_heading_x: cell.terrace_heading_x,
            terrace_heading_z: cell.terrace_heading_z,
            terrace_signed_distance_cells: cell.terrace_signed_distance_cells,
            ..MesoGuideCell::default()
        },
    }
}

fn preview_chunk_xz_coords(center_x: i32, center_z: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut coords = Vec::new();
    for chunk_z in (center_z - radius)..=(center_z + radius) {
        for chunk_x in (center_x - radius)..=(center_x + radius) {
            coords.push((chunk_x, chunk_z));
        }
    }
    coords
}

fn center_chunk_sample_world_xz(chunk: ChunkCoord) -> (i32, i32) {
    (
        chunk.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2),
        chunk.2 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2),
    )
}

fn atlas_coord_for_world_xz(world_x: i32, world_z: i32) -> AtlasCoord {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32).max(1);
    AtlasCoord::new(
        world_x.div_euclid(atlas_span_blocks),
        world_z.div_euclid(atlas_span_blocks),
    )
}

fn default_output_path(
    seed: u64,
    center_x: i32,
    center_z: i32,
    radius: i32,
    blocks_per_pixel: u32,
    feature: PreviewFeature,
    corridor_mode: CorridorMode,
) -> PathBuf {
    PathBuf::from(format!(
        "target/meso-preview/seed_{seed}_{}_{}_cx{center_x}_cz{center_z}_r{radius}_bpp{blocks_per_pixel}.png",
        feature.key(),
        corridor_mode.key(),
    ))
}

fn canonical_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn lerp_rgb(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        from[0] + (to[0] - from[0]) * t,
        from[1] + (to[1] - from[1]) * t,
        from[2] + (to[2] - from[2]) * t,
    ]
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = dot3(vector, vector);
    if length_sq <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        let inv_length = length_sq.sqrt().recip();
        [
            vector[0] * inv_length,
            vector[1] * inv_length,
            vector[2] * inv_length,
        ]
    }
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing {label}\n\n{}", usage())))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage: cargo run --bin meso_preview -- <seed> [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--blocks-per-pixel <u32>] [--feature <all|hill_cluster|shallow_basin|escarpment_band|upland_terrace>] [--corridors <none|live>] [--base-height <f32>] [--relief-budget <f32>] [--contour-step <f32>] [--output <path>]\n\nThis preview isolates the post-prototype meso surface by replacing the normal base heightfield with a flat plain baseline and rendering the resulting meso delta field as a top-down heatmap."
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_feature_parser_accepts_runtime_keys() {
        assert_eq!(
            PreviewFeature::parse("hill_cluster"),
            Some(PreviewFeature::HillCluster)
        );
        assert_eq!(
            PreviewFeature::parse("shallow-basin"),
            Some(PreviewFeature::ShallowBasin)
        );
        assert_eq!(
            PreviewFeature::parse("escarpment band"),
            Some(PreviewFeature::EscarpmentBand)
        );
        assert_eq!(
            PreviewFeature::parse("uplandTerrace"),
            Some(PreviewFeature::UplandTerrace)
        );
        assert_eq!(PreviewFeature::parse("all"), Some(PreviewFeature::All));
        assert_eq!(PreviewFeature::parse("ravine"), None);
    }

    #[test]
    fn default_output_path_marks_feature_and_corridor_mode() {
        let output = default_output_path(
            42,
            -57,
            93,
            10,
            1,
            PreviewFeature::HillCluster,
            CorridorMode::None,
        );
        let output = output.to_string_lossy();
        assert!(output.contains("seed_42_hill_cluster_none_cx-57_cz93_r10_bpp1.png"));
    }

    #[test]
    fn preview_patch_builds_for_a_seed_chunk() {
        let meta = WorldMeta::new(42);
        let patch = build_chunk_preview_patch(
            ChunkCoord(0, 0, 0),
            &meta,
            2,
            PreviewConfig {
                feature: PreviewFeature::HillCluster,
                corridor_mode: CorridorMode::None,
                base_height: DEFAULT_BASE_HEIGHT,
                relief_budget: DEFAULT_RELIEF_BUDGET,
                contour_step: DEFAULT_CONTOUR_STEP,
            },
        );

        assert_eq!(patch.pixels_per_chunk, CHUNK_EDGE_I32 as u32 / 2);
        assert_eq!(
            patch.cells.len(),
            (patch.pixels_per_chunk * patch.pixels_per_chunk) as usize
        );
        assert!(patch.summary.sample_count > 0);
        assert!(patch.summary.min_delta.is_finite());
        assert!(patch.summary.max_delta.is_finite());
    }
}
