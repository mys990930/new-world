#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use image::{Rgb, RgbImage};

use new_world::world::atlas::region_archetype_prototype_hint;
use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCoord, BiomeFamily, CHUNK_EDGE_I32, ChunkCoord,
    ChunkGenerationV2Scaffold, CoastalContext, ElevationBand, HydrologyContext, RegionArchetype,
    RegionClassCell, RegionClassMap, RegionClassSample, RealizationSample, ReliefClass,
    RiverPathKind, TerrainFormFamily, WorldMeta, build_chunk_v2_scaffold,
    sample_chunk_realization_field, sample_region_classes,
};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 8;
const DEFAULT_BLOCKS_PER_PIXEL: u32 = 8;
const DEFAULT_MODE: PreviewMode = PreviewMode::Composite;

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

    fn min_chunk_z(self) -> i32 {
        self.center_z - self.radius
    }

    fn max_chunk_x(self) -> i32 {
        self.center_x + self.radius
    }

    fn max_chunk_z(self) -> i32 {
        self.center_z + self.radius
    }

    fn min_world_x(self) -> i32 {
        self.min_chunk_x() * CHUNK_EDGE_I32
    }

    fn min_world_z(self) -> i32 {
        self.min_chunk_z() * CHUNK_EDGE_I32
    }

    fn sample_world_x(self, pixel_x: u32) -> i32 {
        self.min_world_x()
            + pixel_x as i32 * self.blocks_per_pixel as i32
            + self.blocks_per_pixel as i32 / 2
    }

    fn sample_world_z(self, pixel_z: u32) -> i32 {
        self.min_world_z()
            + pixel_z as i32 * self.blocks_per_pixel as i32
            + self.blocks_per_pixel as i32 / 2
    }

    fn sampled_atlas_bounds(self) -> Result<AtlasBounds, Box<dyn Error>> {
        let (width, height) = self.image_dimensions()?;
        let max_x = width
            .checked_sub(1)
            .ok_or_else(|| cli_error("preview width must be >= 1"))?;
        let max_z = height
            .checked_sub(1)
            .ok_or_else(|| cli_error("preview height must be >= 1"))?;

        let min = atlas_coord_for_world(self.sample_world_x(0), self.sample_world_z(0));
        let max = atlas_coord_for_world(self.sample_world_x(max_x), self.sample_world_z(max_z));
        Ok(AtlasBounds { min, max })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewMode {
    Composite,
    Biome,
    Archetype,
    Flatness,
    Relief,
    Uplift,
    Wetness,
    Ridge,
    Terrace,
    Corridor,
}

impl PreviewMode {
    fn parse(value: &str) -> Option<Self> {
        match canonical_key(value).as_str() {
            "composite" => Some(Self::Composite),
            "biome" => Some(Self::Biome),
            "archetype" => Some(Self::Archetype),
            "flatness" => Some(Self::Flatness),
            "relief" => Some(Self::Relief),
            "uplift" => Some(Self::Uplift),
            "wetness" => Some(Self::Wetness),
            "ridge" => Some(Self::Ridge),
            "terrace" => Some(Self::Terrace),
            "corridor" => Some(Self::Corridor),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Composite => "composite",
            Self::Biome => "biome",
            Self::Archetype => "archetype",
            Self::Flatness => "flatness",
            Self::Relief => "relief",
            Self::Uplift => "uplift",
            Self::Wetness => "wetness",
            Self::Ridge => "ridge",
            Self::Terrace => "terrace",
            Self::Corridor => "corridor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasBounds {
    min: AtlasCoord,
    max: AtlasCoord,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChannelStats {
    min: f32,
    max: f32,
    avg: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreviewSample {
    region: RegionClassSample,
    uplift: f32,
    flatness: f32,
    relief: f32,
    wetness: f32,
    ridge: f32,
    terrace: f32,
    corridor: f32,
}

#[derive(Debug, Clone)]
struct PreviewPatch {
    pixels_per_chunk: u32,
    samples: Vec<PreviewSample>,
}

impl PreviewPatch {
    fn sample(&self, local_pixel_x: u32, local_pixel_z: u32) -> PreviewSample {
        let width = self.pixels_per_chunk as usize;
        self.samples[local_pixel_z as usize * width + local_pixel_x as usize]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedCount {
    key: String,
    count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct PreviewSummary {
    total_pixels: usize,
    sampled_atlas_bounds: AtlasBounds,
    biome_counts: Vec<NamedCount>,
    archetype_counts: Vec<NamedCount>,
    terrain_counts: Vec<NamedCount>,
    hydrology_counts: Vec<NamedCount>,
    uplift: ChannelStats,
    flatness: ChannelStats,
    relief: ChannelStats,
    wetness: ChannelStats,
    ridge: ChannelStats,
    terrace: ChannelStats,
    corridor: ChannelStats,
}

#[derive(Debug, Clone, Copy)]
struct RegionSampleWeight {
    cell: RegionClassCell,
    weight: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct ScalarFieldSample {
    continent_core_factor: f32,
    macro_elevation: f32,
    slope: f32,
    ruggedness: f32,
    ridge_factor: f32,
    mountain_mass: f32,
    basinness: f32,
    pass_potential: f32,
    river_flow_potential: f32,
    lake_potential: f32,
    inlandness: f32,
    aridity: f32,
    wetness: f32,
    alpine_factor: f32,
    coast_factor: f32,
    wetland_factor: f32,
    riverine_factor: f32,
}

#[derive(Debug, Clone, Copy)]
struct ArchetypeHintBlend {
    macro_height_bonus_delta: f32,
    wet_flatten_delta: f32,
    low_freq_amp_scale: f32,
    mid_freq_amp_scale: f32,
    terrace_amp_scale: f32,
    relief_base_scale: f32,
    relief_gain_scale: f32,
    corridor_depth_scale: f32,
    floodplain_width_scale: f32,
    ridge_lift_scale: f32,
    ridge_shoulder_lift_scale: f32,
    ridge_preservation_scale: f32,
}

impl ArchetypeHintBlend {
    fn neutral() -> Self {
        Self {
            macro_height_bonus_delta: 0.0,
            wet_flatten_delta: 0.0,
            low_freq_amp_scale: 1.0,
            mid_freq_amp_scale: 1.0,
            terrace_amp_scale: 1.0,
            relief_base_scale: 1.0,
            relief_gain_scale: 1.0,
            corridor_depth_scale: 1.0,
            floodplain_width_scale: 1.0,
            ridge_lift_scale: 1.0,
            ridge_shoulder_lift_scale: 1.0,
            ridge_preservation_scale: 1.0,
        }
    }

    fn add_weighted(&mut self, other: Self, weight: f32) {
        self.macro_height_bonus_delta += other.macro_height_bonus_delta * weight;
        self.wet_flatten_delta += other.wet_flatten_delta * weight;
        self.low_freq_amp_scale += other.low_freq_amp_scale * weight;
        self.mid_freq_amp_scale += other.mid_freq_amp_scale * weight;
        self.terrace_amp_scale += other.terrace_amp_scale * weight;
        self.relief_base_scale += other.relief_base_scale * weight;
        self.relief_gain_scale += other.relief_gain_scale * weight;
        self.corridor_depth_scale += other.corridor_depth_scale * weight;
        self.floodplain_width_scale += other.floodplain_width_scale * weight;
        self.ridge_lift_scale += other.ridge_lift_scale * weight;
        self.ridge_shoulder_lift_scale += other.ridge_shoulder_lift_scale * weight;
        self.ridge_preservation_scale += other.ridge_preservation_scale * weight;
    }

    fn scale(mut self, factor: f32) -> Self {
        self.macro_height_bonus_delta *= factor;
        self.wet_flatten_delta *= factor;
        self.low_freq_amp_scale *= factor;
        self.mid_freq_amp_scale *= factor;
        self.terrace_amp_scale *= factor;
        self.relief_base_scale *= factor;
        self.relief_gain_scale *= factor;
        self.corridor_depth_scale *= factor;
        self.floodplain_width_scale *= factor;
        self.ridge_lift_scale *= factor;
        self.ridge_shoulder_lift_scale *= factor;
        self.ridge_preservation_scale *= factor;
        self
    }
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
    let mut mode = DEFAULT_MODE;
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
            "--mode" => {
                let value = parse_required::<String>(&mut args, "mode")?;
                mode = PreviewMode::parse(&value).ok_or_else(|| {
                    cli_error(format!(
                        "invalid mode: {value} (expected one of: composite, biome, archetype, flatness, relief, uplift, wetness, ridge, terrace, corridor)"
                    ))
                })?;
            }
            "--output" => output = Some(PathBuf::from(parse_required::<String>(&mut args, "output")?)),
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    let window = PreviewWindow::new(center_x, center_z, radius, blocks_per_pixel)?;
    let sampled_atlas_bounds = window.sampled_atlas_bounds()?;
    let output = output.unwrap_or_else(|| {
        default_output_path(
            seed,
            window.center_x,
            window.center_z,
            window.radius,
            window.blocks_per_pixel,
            mode,
        )
    });

    let meta = WorldMeta::new(seed);
    let (image, summary) = render_preview(&meta, window, sampled_atlas_bounds, mode)?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&output)?;

    println!("seed: {seed}");
    println!("center chunk: ({}, {})", window.center_x, window.center_z);
    println!(
        "footprint: chunks x={}..{}, z={}..{}, radius={}",
        window.min_chunk_x(),
        window.max_chunk_x(),
        window.min_chunk_z(),
        window.max_chunk_z(),
        window.radius
    );
    println!(
        "scale: {} blocks/pixel ({}x{} blocks per sample, {} pixels/chunk)",
        window.blocks_per_pixel,
        window.blocks_per_pixel,
        window.blocks_per_pixel,
        window.pixels_per_chunk()
    );
    println!("mode: {}", mode.as_str());
    println!(
        "sampled atlas cells: x={}..{}, z={}..{}",
        summary.sampled_atlas_bounds.min.x,
        summary.sampled_atlas_bounds.max.x,
        summary.sampled_atlas_bounds.min.z,
        summary.sampled_atlas_bounds.max.z
    );
    println!("sample pixels: {}", summary.total_pixels);
    print_channel_stats("uplift", summary.uplift);
    print_channel_stats("flatness", summary.flatness);
    print_channel_stats("relief", summary.relief);
    print_channel_stats("wetness", summary.wetness);
    print_channel_stats("ridge", summary.ridge);
    print_channel_stats("terrace", summary.terrace);
    print_channel_stats("corridor", summary.corridor);
    print_named_counts("top biomes:", &summary.biome_counts);
    print_named_counts("top archetypes:", &summary.archetype_counts);
    print_named_counts("top terrain forms:", &summary.terrain_counts);
    print_named_counts("top hydrology contexts:", &summary.hydrology_counts);
    println!("output: {}", output.display());
    println!("image: {}x{}", image.width(), image.height());

    Ok(())
}

fn render_preview(
    meta: &WorldMeta,
    window: PreviewWindow,
    sampled_atlas_bounds: AtlasBounds,
    mode: PreviewMode,
) -> Result<(RgbImage, PreviewSummary), Box<dyn Error>> {
    let (width, height) = window.image_dimensions()?;
    let width_usize = usize::try_from(width).map_err(|_| cli_error("preview width overflowed"))?;
    let height_usize =
        usize::try_from(height).map_err(|_| cli_error("preview height overflowed"))?;
    let pixels_per_chunk = window.pixels_per_chunk();
    let mut cache = HashMap::<ChunkCoord, PreviewPatch>::new();
    let mut cells = Vec::with_capacity(width_usize * height_usize);

    for chunk_z in window.min_chunk_z()..=window.max_chunk_z() {
        for local_pixel_z in 0..pixels_per_chunk {
            for chunk_x in window.min_chunk_x()..=window.max_chunk_x() {
                let chunk = ChunkCoord(chunk_x, 0, chunk_z);
                let patch = cache
                    .entry(chunk)
                    .or_insert_with(|| build_preview_patch(chunk, meta, window.blocks_per_pixel));
                for local_pixel_x in 0..pixels_per_chunk {
                    cells.push(patch.sample(local_pixel_x, local_pixel_z));
                }
            }
        }
    }

    let summary = summarize_cells(&cells, sampled_atlas_bounds);
    let normalized_cells = cells
        .iter()
        .copied()
        .map(|sample| normalize_preview_sample(sample, &summary))
        .collect::<Vec<_>>();
    let mut image = RgbImage::new(width, height);
    for pixel_z in 0..height_usize {
        for pixel_x in 0..width_usize {
            let sample = normalized_cells[pixel_z * width_usize + pixel_x];
            let base = color_for_sample(sample, mode);
            let transition = transition_strength(&normalized_cells, width_usize, pixel_x, pixel_z);
            let grid = grid_strength(window, pixel_x, pixel_z);
            let color = darken(base, (transition + grid).clamp(0.0, 0.42));
            image.put_pixel(pixel_x as u32, pixel_z as u32, Rgb(color));
        }
    }

    Ok((image, summary))
}

fn build_preview_patch(chunk: ChunkCoord, meta: &WorldMeta, blocks_per_pixel: u32) -> PreviewPatch {
    let scaffold = build_chunk_v2_scaffold(chunk, meta);
    let pixels_per_chunk = CHUNK_EDGE_I32 as u32 / blocks_per_pixel;
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut samples = Vec::with_capacity((pixels_per_chunk * pixels_per_chunk) as usize);

    for local_pixel_z in 0..pixels_per_chunk {
        for local_pixel_x in 0..pixels_per_chunk {
            let world_x =
                chunk_origin_x + local_pixel_x as i32 * blocks_per_pixel as i32 + blocks_per_pixel as i32 / 2;
            let world_z =
                chunk_origin_z + local_pixel_z as i32 * blocks_per_pixel as i32 + blocks_per_pixel as i32 / 2;
            samples.push(sample_preview_field(&scaffold, world_x as f32, world_z as f32));
        }
    }

    PreviewPatch {
        pixels_per_chunk,
        samples,
    }
}

fn sample_preview_field(
    scaffold: &ChunkGenerationV2Scaffold,
    world_x: f32,
    world_z: f32,
) -> PreviewSample {
    let region = sample_region_classes(
        &scaffold.inputs.region_classes,
        world_x.floor() as i32,
        world_z.floor() as i32,
    );
    let realization =
        sample_chunk_realization_field(&scaffold.realization_field_patch, world_x, world_z);
    let uplift = realization_uplift_signal(realization);
    let flatness = realization_flatness_signal(realization);
    let relief = realization_relief_signal(realization);
    let wetness = realization_wetness_signal(realization);
    let ridge = realization_ridge_signal(realization);
    let terrace = realization_terrace_signal(realization);
    let corridor = realization_corridor_signal(realization);

    PreviewSample {
        region,
        uplift,
        flatness,
        relief,
        wetness,
        ridge,
        terrace,
        corridor,
    }
}

fn normalize_preview_sample(sample: PreviewSample, summary: &PreviewSummary) -> PreviewSample {
    PreviewSample {
        region: sample.region,
        uplift: normalize_stat_value(sample.uplift, summary.uplift),
        flatness: normalize_stat_value(sample.flatness, summary.flatness),
        relief: normalize_stat_value(sample.relief, summary.relief),
        wetness: normalize_stat_value(sample.wetness, summary.wetness),
        ridge: normalize_stat_value(sample.ridge, summary.ridge),
        terrace: normalize_stat_value(sample.terrace, summary.terrace),
        corridor: normalize_stat_value(sample.corridor, summary.corridor),
    }
}

fn normalize_stat_value(value: f32, stats: ChannelStats) -> f32 {
    let span = stats.max - stats.min;
    if span.abs() <= 0.000_1 {
        return 0.5;
    }
    ((value - stats.min) / span).clamp(0.0, 1.0)
}

fn realization_uplift_signal(sample: RealizationSample) -> f32 {
    normalize_range(
        sample.macro_height_bonus
            + sample.inland_lift * 0.9
            + sample.arid_lift * 0.5
            + sample.ridge_lift * 0.4
            + sample.ridge_shoulder_lift * 0.15
            + sample.coastal_cliff_lift * 0.2
            - sample.coastal_shelf_depth * 0.3
            - sample.basin_depth * 0.3,
        -10.0,
        18.0,
    )
}

fn realization_flatness_signal(sample: RealizationSample) -> f32 {
    let wet_flatten = normalize_range(sample.wet_flatten, 0.0, 8.0);
    let relief_mass = normalize_range(sample.relief_base + sample.relief_gain * 2.2, 4.0, 30.0);
    let carrier =
        normalize_range(sample.low_freq_amp + sample.mid_freq_amp + sample.dune_amp, 0.0, 12.0);
    clamp01(wet_flatten * 0.55 + (1.0 - relief_mass) * 0.35 + (1.0 - carrier) * 0.10)
}

fn realization_relief_signal(sample: RealizationSample) -> f32 {
    let broad = normalize_range(sample.relief_base + sample.relief_gain * 2.0, 4.0, 30.0);
    let carrier =
        normalize_range(sample.low_freq_amp + sample.mid_freq_amp + sample.dune_amp, 0.0, 12.0);
    let reserve = normalize_range(sample.meso_relief_reserve, 0.0, 12.0);
    clamp01(broad * 0.55 + carrier * 0.30 + reserve * 0.15)
}

fn realization_wetness_signal(sample: RealizationSample) -> f32 {
    normalize_range(sample.wet_flatten, 0.0, 8.0)
}

fn realization_ridge_signal(sample: RealizationSample) -> f32 {
    let lift = normalize_range(sample.ridge_lift + sample.ridge_shoulder_lift, 0.0, 18.0);
    let preservation = normalize_range(sample.ridge_preservation, 0.0, 1.6);
    clamp01(lift * 0.70 + preservation * 0.30)
}

fn realization_terrace_signal(sample: RealizationSample) -> f32 {
    let terrace = normalize_range(sample.terrace_amp, 0.0, 4.0);
    let coastal = normalize_range(
        sample.coastal_cliff_lift + sample.coastal_shelf_depth,
        0.0,
        12.0,
    );
    clamp01(terrace * 0.80 + coastal * 0.20)
}

fn realization_corridor_signal(sample: RealizationSample) -> f32 {
    let depth = normalize_range(sample.corridor_depth, 0.0, 7.0);
    let corridor_width = normalize_range(sample.corridor_width_scale, 0.75, 1.8);
    let floodplain = normalize_range(sample.floodplain_width_scale, 1.0, 1.8);
    let outlet = normalize_range(sample.outlet_open_scale, 1.0, 1.8);
    let preservation = normalize_range(sample.ridge_preservation, 0.0, 1.6);
    clamp01(
        depth * 0.42
            + corridor_width * 0.14
            + floodplain * 0.22
            + outlet * 0.10
            + preservation * 0.12,
    )
}

fn summarize_cells(cells: &[PreviewSample], sampled_atlas_bounds: AtlasBounds) -> PreviewSummary {
    let mut biome = BTreeMap::<String, usize>::new();
    let mut archetype = BTreeMap::<String, usize>::new();
    let mut terrain = BTreeMap::<String, usize>::new();
    let mut hydrology = BTreeMap::<String, usize>::new();

    for cell in cells {
        increment_count(&mut biome, enum_key(cell.region.biome_family));
        increment_count(&mut archetype, enum_key(cell.region.archetype));
        increment_count(&mut terrain, enum_key(cell.region.terrain_form_family));
        increment_count(&mut hydrology, enum_key(cell.region.hydrology_context));
    }

    PreviewSummary {
        total_pixels: cells.len(),
        sampled_atlas_bounds,
        biome_counts: sorted_counts(biome),
        archetype_counts: sorted_counts(archetype),
        terrain_counts: sorted_counts(terrain),
        hydrology_counts: sorted_counts(hydrology),
        uplift: channel_stats(cells, |sample| sample.uplift),
        flatness: channel_stats(cells, |sample| sample.flatness),
        relief: channel_stats(cells, |sample| sample.relief),
        wetness: channel_stats(cells, |sample| sample.wetness),
        ridge: channel_stats(cells, |sample| sample.ridge),
        terrace: channel_stats(cells, |sample| sample.terrace),
        corridor: channel_stats(cells, |sample| sample.corridor),
    }
}

fn channel_stats<F>(cells: &[PreviewSample], sample: F) -> ChannelStats
where
    F: Fn(PreviewSample) -> f32,
{
    if cells.is_empty() {
        return ChannelStats {
            min: 0.0,
            max: 0.0,
            avg: 0.0,
        };
    }

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0_f32;
    for cell in cells {
        let value = sample(*cell);
        min = min.min(value);
        max = max.max(value);
        sum += value;
    }

    ChannelStats {
        min,
        max,
        avg: sum / cells.len() as f32,
    }
}

fn color_for_sample(sample: PreviewSample, mode: PreviewMode) -> [u8; 3] {
    match mode {
        PreviewMode::Composite => composite_color(sample),
        PreviewMode::Biome => biome_semantic_color(sample.region),
        PreviewMode::Archetype => archetype_semantic_color(sample.region),
        PreviewMode::Flatness => ramp_color(
            sample.flatness,
            [70, 74, 86],
            [146, 132, 100],
            [222, 224, 176],
        ),
        PreviewMode::Relief => ramp_color(
            sample.relief,
            [60, 78, 82],
            [134, 128, 118],
            [236, 232, 224],
        ),
        PreviewMode::Uplift => ramp_color(
            sample.uplift,
            [58, 92, 126],
            [158, 150, 112],
            [226, 206, 190],
        ),
        PreviewMode::Wetness => ramp_color(
            sample.wetness,
            [178, 154, 108],
            [110, 150, 148],
            [80, 126, 196],
        ),
        PreviewMode::Ridge => ramp_color(
            sample.ridge,
            [76, 84, 90],
            [146, 126, 116],
            [220, 96, 86],
        ),
        PreviewMode::Terrace => ramp_color(
            sample.terrace,
            [76, 82, 94],
            [148, 138, 108],
            [236, 196, 112],
        ),
        PreviewMode::Corridor => ramp_color(
            sample.corridor,
            [80, 84, 92],
            [108, 152, 138],
            [88, 170, 220],
        ),
    }
}

fn composite_color(sample: PreviewSample) -> [u8; 3] {
    let warm = clamp01(sample.uplift * 0.52 + sample.ridge * 0.28 + sample.terrace * 0.20);
    let lowland = clamp01(
        sample.flatness * 0.50
            + (1.0 - sample.uplift) * 0.18
            + (1.0 - sample.relief) * 0.12
            + sample.terrace * 0.08,
    );
    let water = clamp01(sample.wetness * 0.54 + sample.corridor * 0.26 + sample.terrace * 0.06);
    let base = [
        mix_channel(54, 226, warm),
        mix_channel(58, 214, lowland),
        mix_channel(62, 220, water),
    ];
    let brightness = 0.74 + sample.relief * 0.14 + sample.uplift * 0.08;
    let highlighted = blend(
        scale(base, brightness),
        [240, 236, 228],
        (sample.ridge * 0.08 + sample.relief * 0.06).clamp(0.0, 0.16),
    );
    darken(highlighted, sample.corridor * 0.04)
}

fn ramp_color(value: f32, low: [u8; 3], mid: [u8; 3], high: [u8; 3]) -> [u8; 3] {
    let value = clamp01(value);
    if value <= 0.5 {
        blend(low, mid, value * 2.0)
    } else {
        blend(mid, high, (value - 0.5) * 2.0)
    }
}

fn biome_semantic_color(region: RegionClassSample) -> [u8; 3] {
    let mut color = biome_base_color(region.biome_family);
    color = blend(
        color,
        coastal_tint(region.coastal_context),
        coastal_tint_strength(region.coastal_context),
    );
    color = blend(
        color,
        hydrology_tint(region.hydrology_context),
        hydrology_tint_strength(region.hydrology_context),
    );

    if let Some((accent, strength)) = terrain_accent(region.terrain_form_family) {
        color = blend(color, accent, strength);
    }

    let relief_factor = relief_brightness(region.relief_class);
    let elevation_factor = elevation_brightness(region.elevation_band);
    scale(color, relief_factor * elevation_factor)
}

fn archetype_semantic_color(region: RegionClassSample) -> [u8; 3] {
    let base = biome_semantic_color(region);
    let (accent, strength) = archetype_tint(region.archetype);
    blend(base, accent, strength)
}

fn biome_base_color(biome: BiomeFamily) -> [u8; 3] {
    match biome {
        BiomeFamily::Oceanic => [44, 92, 160],
        BiomeFamily::RockyCoast => [98, 110, 126],
        BiomeFamily::SandyCoast => [216, 196, 132],
        BiomeFamily::EstuarineCoast => [112, 152, 140],
        BiomeFamily::LagoonCoast => [116, 182, 170],
        BiomeFamily::Mangrove => [62, 116, 86],
        BiomeFamily::Marsh => [124, 136, 72],
        BiomeFamily::Swamp => [76, 102, 70],
        BiomeFamily::FloodedForest => [60, 112, 96],
        BiomeFamily::Desert => [216, 176, 104],
        BiomeFamily::SemiDesert => [194, 160, 104],
        BiomeFamily::Steppe => [172, 154, 94],
        BiomeFamily::DryShrubland => [140, 136, 92],
        BiomeFamily::MediterraneanShrubland => [124, 146, 82],
        BiomeFamily::TemperateBroadleafForest => [88, 148, 82],
        BiomeFamily::TemperateMixedForest => [78, 132, 86],
        BiomeFamily::TemperateRainforest => [72, 132, 94],
        BiomeFamily::BorealForest => [70, 116, 98],
        BiomeFamily::Savanna => [166, 172, 82],
        BiomeFamily::TropicalDryForest => [120, 126, 74],
        BiomeFamily::TropicalRainforest => [58, 150, 82],
        BiomeFamily::MonsoonForest => [58, 140, 92],
        BiomeFamily::SubalpineWoodland => [84, 120, 98],
        BiomeFamily::AlpineMeadow => [114, 156, 112],
        BiomeFamily::Tundra => [148, 152, 118],
        BiomeFamily::PolarBarrens => [168, 170, 176],
        BiomeFamily::PolarIce => [232, 240, 246],
        BiomeFamily::TemperateGrassland => [120, 170, 92],
    }
}

fn coastal_tint(context: CoastalContext) -> [u8; 3] {
    match context {
        CoastalContext::Marine => [90, 146, 200],
        CoastalContext::Coastal => [138, 182, 196],
        CoastalContext::NearCoast => [152, 180, 170],
        CoastalContext::Inland => [255, 255, 255],
    }
}

fn coastal_tint_strength(context: CoastalContext) -> f32 {
    match context {
        CoastalContext::Marine => 0.32,
        CoastalContext::Coastal => 0.14,
        CoastalContext::NearCoast => 0.07,
        CoastalContext::Inland => 0.0,
    }
}

fn hydrology_tint(context: HydrologyContext) -> [u8; 3] {
    match context {
        HydrologyContext::Dryland => [255, 255, 255],
        HydrologyContext::WellDrained => [238, 236, 220],
        HydrologyContext::RiverCorridor => [92, 134, 196],
        HydrologyContext::LakeBasin => [102, 138, 178],
        HydrologyContext::WetLowland => [104, 142, 122],
    }
}

fn hydrology_tint_strength(context: HydrologyContext) -> f32 {
    match context {
        HydrologyContext::Dryland => 0.0,
        HydrologyContext::WellDrained => 0.04,
        HydrologyContext::RiverCorridor => 0.18,
        HydrologyContext::LakeBasin => 0.22,
        HydrologyContext::WetLowland => 0.15,
    }
}

fn terrain_accent(form: TerrainFormFamily) -> Option<([u8; 3], f32)> {
    match form {
        TerrainFormFamily::MarineShelf
        | TerrainFormFamily::LagoonCoast
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::EstuaryLowland
        | TerrainFormFamily::Delta => Some(([124, 176, 188], 0.12)),
        TerrainFormFamily::BeachPlain
        | TerrainFormFamily::AlluvialFan
        | TerrainFormFamily::DuneField
        | TerrainFormFamily::MesaCountry
        | TerrainFormFamily::Badlands
        | TerrainFormFamily::Pediment
        | TerrainFormFamily::Canyon => Some(([194, 136, 88], 0.14)),
        TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::Basin
        | TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley => Some(([106, 136, 106], 0.10)),
        TerrainFormFamily::SeaCliff
        | TerrainFormFamily::FjordCoast
        | TerrainFormFamily::MountainFront
        | TerrainFormFamily::Escarpment
        | TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::GlacialValley
        | TerrainFormFamily::RavineCountry => Some(([164, 172, 184], 0.12)),
        TerrainFormFamily::Icefield | TerrainFormFamily::CrevassedIcefield => {
            Some(([226, 236, 244], 0.18))
        }
        TerrainFormFamily::Karst => Some(([168, 180, 166], 0.08)),
        TerrainFormFamily::RockyShore
        | TerrainFormFamily::Plain
        | TerrainFormFamily::RollingPlain
        | TerrainFormFamily::HillCountry
        | TerrainFormFamily::HillCluster
        | TerrainFormFamily::Plateau => None,
    }
}

fn relief_brightness(relief: ReliefClass) -> f32 {
    match relief {
        ReliefClass::Plain => 1.06,
        ReliefClass::Rolling => 1.0,
        ReliefClass::Hill => 0.94,
        ReliefClass::Mountain => 0.86,
    }
}

fn elevation_brightness(elevation: ElevationBand) -> f32 {
    match elevation {
        ElevationBand::Low => 1.03,
        ElevationBand::Upland => 0.99,
        ElevationBand::Highland => 0.94,
        ElevationBand::Alpine => 0.89,
    }
}

fn archetype_tint(archetype: RegionArchetype) -> ([u8; 3], f32) {
    match archetype {
        RegionArchetype::OceanicShelf
        | RegionArchetype::SandyBeachPlain
        | RegionArchetype::CoastalCliffland
        | RegionArchetype::RockyShoreCoast
        | RegionArchetype::BarrierCoast
        | RegionArchetype::LagoonCoast
        | RegionArchetype::EstuaryLowland
        | RegionArchetype::CoastalDelta
        | RegionArchetype::MangroveLagoon
        | RegionArchetype::MangroveDelta
        | RegionArchetype::MonsoonDelta
        | RegionArchetype::FjordCoast => ([128, 190, 204], 0.18),
        RegionArchetype::ColdWetLowland
        | RegionArchetype::TundraPlain
        | RegionArchetype::TropicalRainforestLowland
        | RegionArchetype::MarshFloodplain
        | RegionArchetype::SwampLowland
        | RegionArchetype::FloodedForestAlluvialLowland
        | RegionArchetype::FloodedForestFloodplain
        | RegionArchetype::TemperateBasin
        | RegionArchetype::TemperateBroadValley
        | RegionArchetype::BorealWetLowland
        | RegionArchetype::DesertBasin
        | RegionArchetype::MonsoonFloodplain
        | RegionArchetype::GlacialValley => ([112, 154, 126], 0.18),
        RegionArchetype::TemperatePlain
        | RegionArchetype::SavannaPlain
        | RegionArchetype::TemperateRollingPlain
        | RegionArchetype::TemperateBroadleafPlain
        | RegionArchetype::BorealPlain
        | RegionArchetype::PolarBarrensPlain => ([186, 188, 122], 0.12),
        RegionArchetype::TemperateHills
        | RegionArchetype::TropicalRainforestHills
        | RegionArchetype::SteppeHills
        | RegionArchetype::TemperateMixedHills
        | RegionArchetype::BorealHills
        | RegionArchetype::MediterraneanShrublandHills
        | RegionArchetype::SavannaHills
        | RegionArchetype::TropicalDryForestHills
        | RegionArchetype::SubalpineWoodedFront
        | RegionArchetype::DesertAlluvialFan
        | RegionArchetype::BorealRidgeCountry
        | RegionArchetype::AlpineRavineCountry => ([154, 126, 94], 0.18),
        RegionArchetype::TemperatePlateau
        | RegionArchetype::TemperateEscarpmentUpland
        | RegionArchetype::MonsoonPlateau
        | RegionArchetype::DesertMesaCountry => ([198, 132, 92], 0.20),
        RegionArchetype::SteppePlain
        | RegionArchetype::DesertPlain
        | RegionArchetype::SemiDesertPediment
        | RegionArchetype::DryShrublandBadlands
        | RegionArchetype::DryShrublandKarst => ([212, 168, 104], 0.16),
        RegionArchetype::DesertDuneField => ([236, 204, 124], 0.28),
        RegionArchetype::GlaciatedAlpine
        | RegionArchetype::AlpineMeadowMountain
        | RegionArchetype::CrevassedIcefield => ([222, 230, 240], 0.22),
    }
}

fn transition_strength(cells: &[PreviewSample], width: usize, x: usize, z: usize) -> f32 {
    let index = z * width + x;
    let current = cells[index];
    let mut strength = 0.0_f32;

    if x > 0 {
        strength = strength.max(sample_difference_strength(current, cells[index - 1]));
    }
    if z > 0 {
        strength = strength.max(sample_difference_strength(current, cells[index - width]));
    }

    strength
}

fn sample_difference_strength(left: PreviewSample, right: PreviewSample) -> f32 {
    let scalar = (left.uplift - right.uplift).abs() * 0.12
        + (left.flatness - right.flatness).abs() * 0.12
        + (left.relief - right.relief).abs() * 0.12
        + (left.wetness - right.wetness).abs() * 0.12
        + (left.ridge - right.ridge).abs() * 0.10
        + (left.terrace - right.terrace).abs() * 0.08
        + (left.corridor - right.corridor).abs() * 0.08;
    let semantic = if left.region.archetype != right.region.archetype {
        0.18
    } else if left.region.terrain_form_family != right.region.terrain_form_family {
        0.12
    } else if left.region.hydrology_context != right.region.hydrology_context {
        0.08
    } else {
        0.0
    };

    scalar.max(semantic).clamp(0.0, 0.22)
}

fn grid_strength(window: PreviewWindow, pixel_x: usize, pixel_z: usize) -> f32 {
    let mut strength = 0.0_f32;
    let block_x = window.min_world_x() + pixel_x as i32 * window.blocks_per_pixel as i32;
    let block_z = window.min_world_z() + pixel_z as i32 * window.blocks_per_pixel as i32;
    let atlas_span_blocks = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;

    if block_x.rem_euclid(CHUNK_EDGE_I32) == 0 || block_z.rem_euclid(CHUNK_EDGE_I32) == 0 {
        strength = strength.max(0.06);
    }
    if block_x.rem_euclid(atlas_span_blocks) == 0 || block_z.rem_euclid(atlas_span_blocks) == 0 {
        strength = strength.max(0.16);
    }

    strength
}

fn sample_region_weights(
    classes: &RegionClassMap,
    world_x: f32,
    world_z: f32,
) -> [RegionSampleWeight; 4] {
    let (atlas_x, atlas_z) = atlas_sample_position(world_x, world_z);
    let (base_x, base_z, east_x, south_z, frac_x, frac_z) =
        fractional_sample_window(classes.area().origin().x, classes.area().origin().z, classes.area().width(), classes.area().height(), atlas_x, atlas_z);
    let c00 = *classes
        .get(AtlasCoord::new(base_x, base_z))
        .expect("fractional region sample must exist");
    let c10 = *classes
        .get(AtlasCoord::new(east_x, base_z))
        .expect("fractional region east sample must exist");
    let c01 = *classes
        .get(AtlasCoord::new(base_x, south_z))
        .expect("fractional region south sample must exist");
    let c11 = *classes
        .get(AtlasCoord::new(east_x, south_z))
        .expect("fractional region southeast sample must exist");
    let (w00, w10, w01, w11) = bilerp_weights(frac_x, frac_z);

    [
        RegionSampleWeight {
            cell: c00,
            weight: w00,
        },
        RegionSampleWeight {
            cell: c10,
            weight: w10,
        },
        RegionSampleWeight {
            cell: c01,
            weight: w01,
        },
        RegionSampleWeight {
            cell: c11,
            weight: w11,
        },
    ]
}

fn sample_scalar_fields(fields: &new_world::world::AtlasFieldMap, world_x: f32, world_z: f32) -> ScalarFieldSample {
    let (atlas_x, atlas_z) = atlas_sample_position(world_x, world_z);
    let area = fields.area();
    let (base_x, base_z, east_x, south_z, frac_x, frac_z) = fractional_sample_window(
        area.origin().x,
        area.origin().z,
        area.width(),
        area.height(),
        atlas_x,
        atlas_z,
    );
    let c00 = fields
        .get(AtlasCoord::new(base_x, base_z))
        .expect("fractional atlas sample must exist");
    let c10 = fields
        .get(AtlasCoord::new(east_x, base_z))
        .expect("fractional atlas east sample must exist");
    let c01 = fields
        .get(AtlasCoord::new(base_x, south_z))
        .expect("fractional atlas south sample must exist");
    let c11 = fields
        .get(AtlasCoord::new(east_x, south_z))
        .expect("fractional atlas southeast sample must exist");

    ScalarFieldSample {
        continent_core_factor: bilerp(
            c00.continent_core_factor,
            c10.continent_core_factor,
            c01.continent_core_factor,
            c11.continent_core_factor,
            frac_x,
            frac_z,
        ),
        macro_elevation: bilerp(
            c00.macro_elevation,
            c10.macro_elevation,
            c01.macro_elevation,
            c11.macro_elevation,
            frac_x,
            frac_z,
        ),
        slope: bilerp(c00.slope, c10.slope, c01.slope, c11.slope, frac_x, frac_z),
        ruggedness: bilerp(
            c00.ruggedness,
            c10.ruggedness,
            c01.ruggedness,
            c11.ruggedness,
            frac_x,
            frac_z,
        ),
        ridge_factor: bilerp(
            c00.ridge_factor,
            c10.ridge_factor,
            c01.ridge_factor,
            c11.ridge_factor,
            frac_x,
            frac_z,
        ),
        mountain_mass: bilerp(
            c00.mountain_mass,
            c10.mountain_mass,
            c01.mountain_mass,
            c11.mountain_mass,
            frac_x,
            frac_z,
        ),
        basinness: bilerp(
            c00.basinness,
            c10.basinness,
            c01.basinness,
            c11.basinness,
            frac_x,
            frac_z,
        ),
        pass_potential: bilerp(
            c00.pass_potential,
            c10.pass_potential,
            c01.pass_potential,
            c11.pass_potential,
            frac_x,
            frac_z,
        ),
        river_flow_potential: bilerp(
            c00.river_flow_potential,
            c10.river_flow_potential,
            c01.river_flow_potential,
            c11.river_flow_potential,
            frac_x,
            frac_z,
        ),
        lake_potential: bilerp(
            c00.lake_potential,
            c10.lake_potential,
            c01.lake_potential,
            c11.lake_potential,
            frac_x,
            frac_z,
        ),
        inlandness: bilerp(
            c00.inlandness,
            c10.inlandness,
            c01.inlandness,
            c11.inlandness,
            frac_x,
            frac_z,
        ),
        aridity: bilerp(c00.aridity, c10.aridity, c01.aridity, c11.aridity, frac_x, frac_z),
        wetness: bilerp(c00.wetness, c10.wetness, c01.wetness, c11.wetness, frac_x, frac_z),
        alpine_factor: bilerp(
            c00.alpine_factor,
            c10.alpine_factor,
            c01.alpine_factor,
            c11.alpine_factor,
            frac_x,
            frac_z,
        ),
        coast_factor: bilerp(
            c00.coast_factor,
            c10.coast_factor,
            c01.coast_factor,
            c11.coast_factor,
            frac_x,
            frac_z,
        ),
        wetland_factor: bilerp(
            c00.wetland_factor,
            c10.wetland_factor,
            c01.wetland_factor,
            c11.wetland_factor,
            frac_x,
            frac_z,
        ),
        riverine_factor: bilerp(
            c00.riverine_factor,
            c10.riverine_factor,
            c01.riverine_factor,
            c11.riverine_factor,
            frac_x,
            frac_z,
        ),
    }
}

fn blend_archetype_hints(region_samples: &[RegionSampleWeight; 4]) -> ArchetypeHintBlend {
    let mut total_weight = 0.0_f32;
    let mut blended = ArchetypeHintBlend::neutral().scale(0.0);

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }
        blended.add_weighted(hint_for_region(sample.cell), sample.weight);
        total_weight += sample.weight;
    }

    if total_weight <= f32::EPSILON {
        return ArchetypeHintBlend::neutral();
    }

    blended.scale(total_weight.recip())
}

fn hint_for_region(cell: RegionClassCell) -> ArchetypeHintBlend {
    region_archetype_prototype_hint(cell.archetype)
        .map(|hint| ArchetypeHintBlend {
            macro_height_bonus_delta: hint.macro_height_bonus_delta,
            wet_flatten_delta: hint.wet_flatten_delta,
            low_freq_amp_scale: hint.low_freq_amp_scale,
            mid_freq_amp_scale: hint.mid_freq_amp_scale,
            terrace_amp_scale: hint.terrace_amp_scale,
            relief_base_scale: hint.relief_base_scale,
            relief_gain_scale: hint.relief_gain_scale,
            corridor_depth_scale: hint.corridor_depth_scale,
            floodplain_width_scale: hint.floodplain_width_scale,
            ridge_lift_scale: hint.ridge_lift_scale,
            ridge_shoulder_lift_scale: hint.ridge_shoulder_lift_scale,
            ridge_preservation_scale: hint.ridge_preservation_scale,
        })
        .unwrap_or_else(ArchetypeHintBlend::neutral)
}

fn weighted_region_signal<F>(samples: &[RegionSampleWeight; 4], signal: F) -> f32
where
    F: Fn(RegionClassCell) -> f32,
{
    let mut total = 0.0_f32;
    let mut weight_sum = 0.0_f32;

    for sample in samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }
        total += signal(sample.cell) * sample.weight;
        weight_sum += sample.weight;
    }

    if weight_sum <= f32::EPSILON {
        0.0
    } else {
        total / weight_sum
    }
}

fn region_uplift_signal(region: RegionClassCell) -> f32 {
    let elevation = match region.elevation_band {
        ElevationBand::Low => 0.12,
        ElevationBand::Upland => 0.38,
        ElevationBand::Highland => 0.70,
        ElevationBand::Alpine => 0.92,
    };
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::MarineShelf => 0.02,
        TerrainFormFamily::LagoonCoast
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::EstuaryLowland
        | TerrainFormFamily::Delta
        | TerrainFormFamily::BeachPlain
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland => 0.12,
        TerrainFormFamily::Plain
        | TerrainFormFamily::RollingPlain
        | TerrainFormFamily::Karst
        | TerrainFormFamily::DuneField => 0.22,
        TerrainFormFamily::Basin
        | TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley
        | TerrainFormFamily::AlluvialFan => 0.28,
        TerrainFormFamily::HillCountry | TerrainFormFamily::HillCluster => 0.46,
        TerrainFormFamily::Plateau
        | TerrainFormFamily::Pediment
        | TerrainFormFamily::MesaCountry
        | TerrainFormFamily::Badlands
        | TerrainFormFamily::Escarpment
        | TerrainFormFamily::MountainFront
        | TerrainFormFamily::SeaCliff
        | TerrainFormFamily::RockyShore => 0.64,
        TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::GlacialValley
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield
        | TerrainFormFamily::FjordCoast
        | TerrainFormFamily::Canyon => 0.82,
    };
    let coastal = match region.coastal_context {
        CoastalContext::Marine => -0.14,
        CoastalContext::Coastal => -0.08,
        CoastalContext::NearCoast => -0.02,
        CoastalContext::Inland => 0.0,
    };
    let hydrology = match region.hydrology_context {
        HydrologyContext::LakeBasin | HydrologyContext::WetLowland => -0.08,
        HydrologyContext::RiverCorridor => -0.04,
        HydrologyContext::WellDrained => 0.0,
        HydrologyContext::Dryland => 0.04,
    };

    clamp01(elevation * 0.55 + terrain * 0.35 + hydrology * 0.10 + coastal)
}

fn region_flatness_signal(region: RegionClassCell) -> f32 {
    let relief = match region.relief_class {
        ReliefClass::Plain => 0.92,
        ReliefClass::Rolling => 0.68,
        ReliefClass::Hill => 0.34,
        ReliefClass::Mountain => 0.10,
    };
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::MarineShelf
        | TerrainFormFamily::LagoonCoast
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::EstuaryLowland
        | TerrainFormFamily::Delta
        | TerrainFormFamily::BeachPlain
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland => 0.90,
        TerrainFormFamily::Plain
        | TerrainFormFamily::RollingPlain
        | TerrainFormFamily::Basin
        | TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley => 0.72,
        TerrainFormFamily::Plateau | TerrainFormFamily::Pediment | TerrainFormFamily::Karst => 0.58,
        TerrainFormFamily::AlluvialFan
        | TerrainFormFamily::DuneField
        | TerrainFormFamily::MesaCountry
        | TerrainFormFamily::Badlands => 0.44,
        TerrainFormFamily::HillCountry | TerrainFormFamily::HillCluster => 0.32,
        TerrainFormFamily::Escarpment
        | TerrainFormFamily::MountainFront
        | TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::SeaCliff
        | TerrainFormFamily::FjordCoast
        | TerrainFormFamily::Canyon
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield
        | TerrainFormFamily::GlacialValley
        | TerrainFormFamily::RockyShore => 0.16,
    };
    let hydrology = match region.hydrology_context {
        HydrologyContext::WetLowland | HydrologyContext::LakeBasin => 0.90,
        HydrologyContext::RiverCorridor => 0.68,
        HydrologyContext::WellDrained => 0.48,
        HydrologyContext::Dryland => 0.42,
    };

    clamp01(relief * 0.48 + terrain * 0.34 + hydrology * 0.18)
}

fn region_relief_signal(region: RegionClassCell) -> f32 {
    let relief = match region.relief_class {
        ReliefClass::Plain => 0.10,
        ReliefClass::Rolling => 0.34,
        ReliefClass::Hill => 0.66,
        ReliefClass::Mountain => 0.92,
    };
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::MarineShelf
        | TerrainFormFamily::LagoonCoast
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::EstuaryLowland
        | TerrainFormFamily::Delta
        | TerrainFormFamily::BeachPlain
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland => 0.14,
        TerrainFormFamily::Plain | TerrainFormFamily::RollingPlain | TerrainFormFamily::Basin => 0.24,
        TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley
        | TerrainFormFamily::AlluvialFan
        | TerrainFormFamily::Plateau
        | TerrainFormFamily::Pediment
        | TerrainFormFamily::Karst => 0.42,
        TerrainFormFamily::DuneField | TerrainFormFamily::MesaCountry | TerrainFormFamily::HillCountry => {
            0.58
        }
        TerrainFormFamily::HillCluster
        | TerrainFormFamily::Escarpment
        | TerrainFormFamily::MountainFront
        | TerrainFormFamily::SeaCliff
        | TerrainFormFamily::GlacialValley
        | TerrainFormFamily::RockyShore
        | TerrainFormFamily::Badlands => 0.72,
        TerrainFormFamily::Mountain
        | TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::FjordCoast
        | TerrainFormFamily::Canyon
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield => 0.90,
    };

    clamp01(relief * 0.60 + terrain * 0.40)
}

fn region_wetness_signal(region: RegionClassCell) -> f32 {
    let hydrology = match region.hydrology_context {
        HydrologyContext::Dryland => 0.08,
        HydrologyContext::WellDrained => 0.28,
        HydrologyContext::RiverCorridor => 0.72,
        HydrologyContext::LakeBasin => 0.88,
        HydrologyContext::WetLowland => 0.94,
    };
    let coastal = match region.coastal_context {
        CoastalContext::Marine => 0.66,
        CoastalContext::Coastal => 0.56,
        CoastalContext::NearCoast => 0.42,
        CoastalContext::Inland => 0.22,
    };
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::LagoonCoast
        | TerrainFormFamily::BarrierCoast
        | TerrainFormFamily::EstuaryLowland
        | TerrainFormFamily::Delta
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::WetLowland
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::Basin => 0.82,
        TerrainFormFamily::MarineShelf | TerrainFormFamily::BeachPlain => 0.60,
        TerrainFormFamily::BroadValley | TerrainFormFamily::NarrowValley => 0.56,
        TerrainFormFamily::Plain | TerrainFormFamily::RollingPlain | TerrainFormFamily::HillCountry => 0.34,
        _ => 0.20,
    };

    clamp01(hydrology * 0.54 + coastal * 0.20 + terrain * 0.26)
}

fn region_ridge_signal(region: RegionClassCell) -> f32 {
    let relief = match region.relief_class {
        ReliefClass::Plain => 0.08,
        ReliefClass::Rolling => 0.28,
        ReliefClass::Hill => 0.60,
        ReliefClass::Mountain => 0.92,
    };
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::RidgeCountry
        | TerrainFormFamily::Mountain
        | TerrainFormFamily::MountainFront
        | TerrainFormFamily::Escarpment
        | TerrainFormFamily::FjordCoast
        | TerrainFormFamily::SeaCliff
        | TerrainFormFamily::RavineCountry
        | TerrainFormFamily::GlacialValley
        | TerrainFormFamily::Icefield
        | TerrainFormFamily::CrevassedIcefield => 0.90,
        TerrainFormFamily::Plateau | TerrainFormFamily::HillCountry | TerrainFormFamily::HillCluster => 0.54,
        TerrainFormFamily::MesaCountry | TerrainFormFamily::Pediment | TerrainFormFamily::Canyon => 0.68,
        _ => 0.14,
    };

    clamp01(relief * 0.58 + terrain * 0.42)
}

fn region_terrace_signal(region: RegionClassCell) -> f32 {
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::Plateau
        | TerrainFormFamily::Pediment
        | TerrainFormFamily::MesaCountry
        | TerrainFormFamily::Escarpment => 0.84,
        TerrainFormFamily::AlluvialFan
        | TerrainFormFamily::Floodplain
        | TerrainFormFamily::Delta
        | TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley => 0.62,
        TerrainFormFamily::Basin | TerrainFormFamily::Karst | TerrainFormFamily::Canyon => 0.54,
        TerrainFormFamily::DuneField | TerrainFormFamily::Plain | TerrainFormFamily::RollingPlain => 0.24,
        _ => 0.14,
    };
    let elevation = match region.elevation_band {
        ElevationBand::Low => 0.26,
        ElevationBand::Upland => 0.46,
        ElevationBand::Highland => 0.68,
        ElevationBand::Alpine => 0.58,
    };

    clamp01(terrain * 0.72 + elevation * 0.28)
}

fn region_corridor_signal(region: RegionClassCell) -> f32 {
    let hydrology = match region.hydrology_context {
        HydrologyContext::RiverCorridor => 0.92,
        HydrologyContext::WetLowland => 0.74,
        HydrologyContext::LakeBasin => 0.66,
        HydrologyContext::WellDrained => 0.18,
        HydrologyContext::Dryland => 0.08,
    };
    let terrain = match region.terrain_form_family {
        TerrainFormFamily::Floodplain
        | TerrainFormFamily::Delta
        | TerrainFormFamily::AlluvialLowland
        | TerrainFormFamily::BroadValley
        | TerrainFormFamily::NarrowValley
        | TerrainFormFamily::Basin => 0.74,
        TerrainFormFamily::WetLowland | TerrainFormFamily::EstuaryLowland => 0.68,
        TerrainFormFamily::Plain | TerrainFormFamily::RollingPlain => 0.30,
        _ => 0.16,
    };

    clamp01(hydrology * 0.68 + terrain * 0.32)
}

fn corridor_influence(scaffold: &ChunkGenerationV2Scaffold, local_x: f32, local_z: f32) -> f32 {
    let mut strongest = 0.0_f32;

    for corridor in &scaffold.corridor_window.corridors {
        let distance = point_segment_distance(
            local_x,
            local_z,
            corridor.start_x,
            corridor.start_z,
            corridor.end_x,
            corridor.end_z,
        );
        let width = corridor.half_width_blocks.max(1.0);
        let radial = 1.0 - (distance / (width * 2.2)).clamp(0.0, 1.0);
        if radial <= 0.0 {
            continue;
        }

        let width_bias = 0.85 + (width / 96.0).clamp(0.0, 1.0) * 0.15;
        let grade_bias =
            0.80 + (1.0 - (corridor.downstream_grade_per_block / 0.012).clamp(0.0, 1.0)) * 0.20;
        let kind_bias = match corridor.kind {
            RiverPathKind::Trunk => 1.0,
            RiverPathKind::Tributary => 0.82,
        };
        strongest = strongest.max(radial * radial * width_bias * grade_bias * kind_bias);
    }

    strongest.clamp(0.0, 1.0)
}

fn point_segment_distance(px: f32, pz: f32, ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let abx = bx - ax;
    let abz = bz - az;
    let apx = px - ax;
    let apz = pz - az;
    let length_sq = abx * abx + abz * abz;
    if length_sq <= f32::EPSILON {
        return ((px - ax).powi(2) + (pz - az).powi(2)).sqrt();
    }

    let t = ((apx * abx + apz * abz) / length_sq).clamp(0.0, 1.0);
    let nearest_x = ax + abx * t;
    let nearest_z = az + abz * t;
    ((px - nearest_x).powi(2) + (pz - nearest_z).powi(2)).sqrt()
}

fn atlas_sample_position(world_x: f32, world_z: f32) -> (f32, f32) {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32) as f32;
    (world_x / atlas_span_blocks, world_z / atlas_span_blocks)
}

fn fractional_sample_window(
    min_x: i32,
    min_z: i32,
    width: u32,
    height: u32,
    sample_x: f32,
    sample_z: f32,
) -> (i32, i32, i32, i32, f32, f32) {
    let max_x = min_x + width as i32 - 1;
    let max_z = min_z + height as i32 - 1;
    let base_x = sample_x.floor() as i32;
    let base_z = sample_z.floor() as i32;
    let clamped_base_x = base_x.clamp(min_x, max_x);
    let clamped_base_z = base_z.clamp(min_z, max_z);
    let east_x = (clamped_base_x + 1).min(max_x);
    let south_z = (clamped_base_z + 1).min(max_z);
    let frac_x = (sample_x - clamped_base_x as f32).clamp(0.0, 1.0);
    let frac_z = (sample_z - clamped_base_z as f32).clamp(0.0, 1.0);

    (clamped_base_x, clamped_base_z, east_x, south_z, frac_x, frac_z)
}

fn bilerp(a00: f32, a10: f32, a01: f32, a11: f32, tx: f32, tz: f32) -> f32 {
    let smooth_x = smootherstep(tx);
    let smooth_z = smootherstep(tz);
    let top = a00 + (a10 - a00) * smooth_x;
    let bottom = a01 + (a11 - a01) * smooth_x;
    top + (bottom - top) * smooth_z
}

fn bilerp_weights(tx: f32, tz: f32) -> (f32, f32, f32, f32) {
    let smooth_x = smootherstep(tx);
    let smooth_z = smootherstep(tz);
    let inv_x = 1.0 - smooth_x;
    let inv_z = 1.0 - smooth_z;
    (
        inv_x * inv_z,
        smooth_x * inv_z,
        inv_x * smooth_z,
        smooth_x * smooth_z,
    )
}

fn smootherstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn atlas_coord_for_world(world_x: i32, world_z: i32) -> AtlasCoord {
    let atlas_span_blocks = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
    AtlasCoord::new(
        world_x.div_euclid(atlas_span_blocks),
        world_z.div_euclid(atlas_span_blocks),
    )
}

fn positive_scale_delta(scale: f32) -> f32 {
    ((scale - 1.0) / 0.6).clamp(0.0, 1.0)
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn normalize_range(value: f32, min: f32, max: f32) -> f32 {
    let span = max - min;
    if span.abs() <= f32::EPSILON {
        return 0.0;
    }
    ((value - min) / span).clamp(0.0, 1.0)
}

fn print_channel_stats(label: &str, stats: ChannelStats) {
    println!(
        "{}: min {:.3}, max {:.3}, avg {:.3}",
        label, stats.min, stats.max, stats.avg
    );
}

fn print_named_counts(label: &str, counts: &[NamedCount]) {
    println!("{label}");
    for count in counts.iter().take(8) {
        println!("  {}: {}", count.key, count.count);
    }
}

fn increment_count(counts: &mut BTreeMap<String, usize>, key: String) {
    *counts.entry(key).or_insert(0) += 1;
}

fn sorted_counts(counts: BTreeMap<String, usize>) -> Vec<NamedCount> {
    let mut counts = counts
        .into_iter()
        .map(|(key, count)| NamedCount { key, count })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    counts
}

fn blend(base: [u8; 3], tint: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        mix_channel(base[0], tint[0], amount),
        mix_channel(base[1], tint[1], amount),
        mix_channel(base[2], tint[2], amount),
    ]
}

fn darken(color: [u8; 3], amount: f32) -> [u8; 3] {
    scale(color, (1.0 - amount).clamp(0.0, 1.0))
}

fn scale(color: [u8; 3], factor: f32) -> [u8; 3] {
    [
        scale_channel(color[0], factor),
        scale_channel(color[1], factor),
        scale_channel(color[2], factor),
    ]
}

fn mix_channel(base: u8, tint: u8, amount: f32) -> u8 {
    let amount = amount.clamp(0.0, 1.0);
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
}

fn scale_channel(channel: u8, factor: f32) -> u8 {
    (channel as f32 * factor).round().clamp(0.0, 255.0) as u8
}

fn default_output_path(
    seed: u64,
    center_x: i32,
    center_z: i32,
    radius: i32,
    blocks_per_pixel: u32,
    mode: PreviewMode,
) -> PathBuf {
    PathBuf::from(format!(
        "target/realization-field-preview/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}_bpp{blocks_per_pixel}_{}.png",
        mode.as_str()
    ))
}

fn enum_key<T>(value: T) -> String
where
    T: Debug,
{
    canonical_key(&format!("{value:?}"))
}

fn canonical_key(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut out = String::new();

    for (index, ch) in chars.iter().copied().enumerate() {
        if matches!(ch, '-' | '_' | ' ' | '/') {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            continue;
        }

        if ch.is_ascii_uppercase() {
            if index > 0 {
                let previous = chars[index - 1];
                let next = chars.get(index + 1).copied();
                if !out.ends_with('_')
                    && ((previous.is_ascii_lowercase() || previous.is_ascii_digit())
                        || (previous.is_ascii_uppercase()
                            && next.map(|value| value.is_ascii_lowercase()).unwrap_or(false)))
                {
                    out.push('_');
                }
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch.to_ascii_lowercase());
        }
    }

    out.trim_matches('_').to_string()
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
    "usage: cargo run --bin realization_field_preview -- <seed> [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--blocks-per-pixel <u32>] [--mode <composite|biome|archetype|flatness|relief|uplift|wetness|ridge|terrace|corridor>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_with_region(region: RegionClassCell) -> PreviewSample {
        PreviewSample {
            region,
            uplift: 0.5,
            flatness: 0.5,
            relief: 0.5,
            wetness: 0.5,
            ridge: 0.5,
            terrace: 0.5,
            corridor: 0.5,
        }
    }

    fn sample_with_channels(uplift: f32, flatness: f32, wetness: f32) -> PreviewSample {
        PreviewSample {
            region: RegionClassCell::default(),
            uplift,
            flatness,
            relief: uplift,
            wetness,
            ridge: uplift,
            terrace: flatness,
            corridor: wetness,
        }
    }

    #[test]
    fn default_quarter_chunk_scale_uses_four_pixels_per_chunk() {
        let window = PreviewWindow::new(0, 0, 2, DEFAULT_BLOCKS_PER_PIXEL)
            .expect("default preview window should be valid");

        assert_eq!(window.pixels_per_chunk(), 4);
        assert_eq!(
            window.image_dimensions().expect("image dimensions should resolve"),
            (20, 20)
        );
    }

    #[test]
    fn mode_parser_accepts_named_channels() {
        assert_eq!(PreviewMode::parse("biome"), Some(PreviewMode::Biome));
        assert_eq!(PreviewMode::parse("archetype"), Some(PreviewMode::Archetype));
        assert_eq!(PreviewMode::parse("flatness"), Some(PreviewMode::Flatness));
        assert_eq!(PreviewMode::parse("wetness"), Some(PreviewMode::Wetness));
        assert_eq!(PreviewMode::parse("ridge"), Some(PreviewMode::Ridge));
        assert_eq!(PreviewMode::parse("unknown"), None);
    }

    #[test]
    fn composite_palette_pushes_blue_for_wet_samples() {
        let dry = composite_color(sample_with_channels(0.8, 0.2, 0.1));
        let wet = composite_color(sample_with_channels(0.2, 0.8, 0.9));

        assert!(wet[2] > dry[2]);
        assert!(dry[0] > wet[0]);
    }

    #[test]
    fn biome_palette_keeps_polar_ice_lighter_than_temperate_grassland() {
        let polar = color_for_sample(
            sample_with_region(RegionClassCell {
                biome_family: BiomeFamily::PolarIce,
                ..RegionClassCell::default()
            }),
            PreviewMode::Biome,
        );
        let grass = color_for_sample(
            sample_with_region(RegionClassCell {
                biome_family: BiomeFamily::TemperateGrassland,
                ..RegionClassCell::default()
            }),
            PreviewMode::Biome,
        );

        assert!(polar[0] > grass[0]);
        assert!(polar[1] > grass[1]);
        assert!(polar[2] > grass[2]);
    }

    #[test]
    fn archetype_mode_adds_family_accent_over_biome_mode() {
        let region = RegionClassCell {
            biome_family: BiomeFamily::Desert,
            archetype: RegionArchetype::DesertDuneField,
            terrain_form_family: TerrainFormFamily::DuneField,
            ..RegionClassCell::default()
        };

        let biome = color_for_sample(sample_with_region(region), PreviewMode::Biome);
        let archetype = color_for_sample(sample_with_region(region), PreviewMode::Archetype);

        assert_ne!(biome, archetype);
        assert!(archetype[1] > biome[1]);
    }
}
