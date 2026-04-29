use std::collections::{BTreeMap, HashMap};
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;

use image::{Rgb, RgbImage};
use rayon::prelude::*;

use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCoord, BiomeFamily, BlockRegistry, CHUNK_EDGE_I32, ChunkCoord,
    ChunkGenerationInputs, CoastalContext, ElevationBand, HydrologyContext, RealizationSample,
    RegionClassSample, ReliefClass, TerrainFormFamily, TopdownColumnScan, WORLD_FLOOR_Y, WorldCore,
    WorldMeta, build_chunk_generation_voxelization_plan, build_chunk_realization_field_patch,
    chunk_generation_input_area, color_topdown_cell, generate_chunk_from_voxelization_plan,
    prepare_chunk_generation_inputs, sample_chunk_realization_field, sample_region_classes,
    sample_topdown_columns, topdown_outline_strength, topdown_surface_range,
};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 0;
const DEFAULT_MIN_Y_CHUNK: i32 = -2;
const DEFAULT_MAX_Y_CHUNK: i32 = 3;
const DEFAULT_PIXELS_PER_BLOCK: u32 = 2;
const PANEL_MIN_WIDTH: u32 = 132;
const PANEL_GAP: u32 = 12;
const OUTER_MARGIN: u32 = 12;
const TITLE_HEIGHT: u32 = 22;
const LEGEND_GAP: u32 = 8;
const LEGEND_HEIGHT: u32 = 82;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreviewWindow {
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
    pixels_per_block: u32,
}

impl PreviewWindow {
    fn new(
        center_x: i32,
        center_z: i32,
        radius: i32,
        min_y_chunk: i32,
        max_y_chunk: i32,
        pixels_per_block: u32,
    ) -> Result<Self, Box<dyn Error>> {
        if radius < 0 {
            return Err(cli_error("radius must be non-negative"));
        }
        if min_y_chunk > max_y_chunk {
            return Err(cli_error("min-y-chunk must be <= max-y-chunk"));
        }
        if pixels_per_block == 0 {
            return Err(cli_error("pixels-per-block must be >= 1"));
        }

        Ok(Self {
            center_x,
            center_z,
            radius,
            min_y_chunk,
            max_y_chunk,
            pixels_per_block,
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

    fn blocks_per_axis(self) -> Result<u32, Box<dyn Error>> {
        self.chunk_span()?
            .checked_mul(CHUNK_EDGE_I32 as u32)
            .ok_or_else(|| cli_error("preview block width overflowed"))
    }

    fn panel_image_dimensions(self) -> Result<(u32, u32), Box<dyn Error>> {
        let blocks = self.blocks_per_axis()?;
        let width = blocks
            .checked_mul(self.pixels_per_block)
            .ok_or_else(|| cli_error("preview image width overflowed"))?;
        let height = blocks
            .checked_mul(self.pixels_per_block)
            .ok_or_else(|| cli_error("preview image height overflowed"))?;
        Ok((width, height))
    }

    fn panel_area_width(self) -> Result<u32, Box<dyn Error>> {
        let (panel_width, _) = self.panel_image_dimensions()?;
        Ok(panel_width.max(PANEL_MIN_WIDTH))
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

    fn min_world_x(self) -> i32 {
        self.min_chunk_x() * CHUNK_EDGE_I32
    }

    fn min_world_z(self) -> i32 {
        self.min_chunk_z() * CHUNK_EDGE_I32
    }

    fn min_world_y(self) -> i32 {
        (self.min_y_chunk * CHUNK_EDGE_I32).max(WORLD_FLOOR_Y)
    }

    fn max_world_y(self) -> i32 {
        (self.max_y_chunk + 1) * CHUNK_EDGE_I32 - 1
    }

    fn sampled_atlas_bounds(self) -> Result<AtlasBounds, Box<dyn Error>> {
        let blocks = self.blocks_per_axis()?;
        let max_offset = blocks
            .checked_sub(1)
            .ok_or_else(|| cli_error("preview block width must be >= 1"))?;
        let min = atlas_coord_for_world(self.min_world_x(), self.min_world_z());
        let max = atlas_coord_for_world(
            self.min_world_x() + max_offset as i32,
            self.min_world_z() + max_offset as i32,
        );
        Ok(AtlasBounds { min, max })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasBounds {
    min: AtlasCoord,
    max: AtlasCoord,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RealizationPreviewSample {
    region: RegionClassSample,
    uplift: f32,
    flatness: f32,
    relief: f32,
    wetness: f32,
    ridge: f32,
    terrace: f32,
    corridor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChannelStats {
    min: f32,
    max: f32,
    avg: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedColorCount {
    key: String,
    count: usize,
    color: [u8; 3],
}

#[derive(Debug, Clone, PartialEq)]
struct PreviewSummary {
    atlas_bounds: AtlasBounds,
    atlas_biomes: Vec<NamedColorCount>,
    atlas_archetypes: Vec<NamedColorCount>,
    realization_uplift: ChannelStats,
    realization_flatness: ChannelStats,
    realization_relief: ChannelStats,
    realization_wetness: ChannelStats,
    realization_ridge: ChannelStats,
    realization_terrace: ChannelStats,
    realization_corridor: ChannelStats,
    final_blocks: Vec<NamedColorCount>,
    final_surface: Option<new_world::world::TopdownSurfaceRange>,
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
    let mut min_y_chunk = DEFAULT_MIN_Y_CHUNK;
    let mut max_y_chunk = DEFAULT_MAX_Y_CHUNK;
    let mut pixels_per_block = DEFAULT_PIXELS_PER_BLOCK;
    let mut output: Option<PathBuf> = None;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-x" | "--chunk-x" => center_x = parse_required::<i32>(&mut args, "center-x")?,
            "--center-z" | "--chunk-z" => center_z = parse_required::<i32>(&mut args, "center-z")?,
            "--radius" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--min-y-chunk" => min_y_chunk = parse_required::<i32>(&mut args, "min-y-chunk")?,
            "--max-y-chunk" => max_y_chunk = parse_required::<i32>(&mut args, "max-y-chunk")?,
            "--pixels-per-block" => {
                pixels_per_block = parse_required::<u32>(&mut args, "pixels-per-block")?
            }
            "--output" => {
                output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    let window = PreviewWindow::new(
        center_x,
        center_z,
        radius,
        min_y_chunk,
        max_y_chunk,
        pixels_per_block,
    )?;
    if window.max_world_y() < window.min_world_y() {
        return Err(cli_error(format!(
            "requested vertical window resolves to an empty world-y range: {}..{}",
            window.min_world_y(),
            window.max_world_y()
        )));
    }

    let output = output
        .unwrap_or_else(|| default_output_path(seed, center_x, center_z, radius, pixels_per_block));
    let meta = WorldMeta::new(seed);
    let registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );
    let input_cache = PreviewInputCache::for_preview_window(&meta, center_x, center_z, radius);
    let chunk_inputs = build_chunk_inputs(&input_cache, window);
    let realization_patches = build_realization_patches(&chunk_inputs);
    let mut world = WorldCore::new(meta, Arc::clone(&registry));
    generate_final_chunks(&mut world, registry.as_ref(), &input_cache, window);

    let atlas_panel = render_atlas_panel(&chunk_inputs, window)?;
    let realization_panel = render_realization_panel(&chunk_inputs, &realization_patches, window)?;
    let chunk_panel = render_chunk_panel(&world, registry.as_ref(), window)?;
    let summary = PreviewSummary {
        atlas_bounds: window.sampled_atlas_bounds()?,
        atlas_biomes: atlas_panel.biome_counts.clone(),
        atlas_archetypes: atlas_panel.archetype_counts.clone(),
        realization_uplift: realization_panel.summary.uplift,
        realization_flatness: realization_panel.summary.flatness,
        realization_relief: realization_panel.summary.relief,
        realization_wetness: realization_panel.summary.wetness,
        realization_ridge: realization_panel.summary.ridge,
        realization_terrace: realization_panel.summary.terrace,
        realization_corridor: realization_panel.summary.corridor,
        final_blocks: chunk_panel.block_counts.clone(),
        final_surface: chunk_panel.surface_range,
    };
    let image = compose_image(&atlas_panel, &realization_panel, &chunk_panel, window)?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&output)?;

    println!("seed: {seed}");
    println!("center chunk: ({center_x}, {center_z})");
    println!(
        "footprint: chunks x={}..{}, z={}..{}, radius={}",
        window.min_chunk_x(),
        window.max_chunk_x(),
        window.min_chunk_z(),
        window.max_chunk_z(),
        radius
    );
    println!(
        "scale: {} pixels/block, panel={}x{}",
        pixels_per_block,
        atlas_panel.image.width(),
        atlas_panel.image.height()
    );
    println!(
        "sampled atlas cells: x={}..{}, z={}..{}",
        summary.atlas_bounds.min.x,
        summary.atlas_bounds.max.x,
        summary.atlas_bounds.min.z,
        summary.atlas_bounds.max.z
    );
    print_channel_stats("realization uplift", summary.realization_uplift);
    print_channel_stats("realization flatness", summary.realization_flatness);
    print_channel_stats("realization relief", summary.realization_relief);
    print_channel_stats("realization wetness", summary.realization_wetness);
    print_channel_stats("realization ridge", summary.realization_ridge);
    print_channel_stats("realization terrace", summary.realization_terrace);
    print_channel_stats("realization corridor", summary.realization_corridor);
    if let Some(surface) = summary.final_surface {
        println!("final surface relief: {}..{}", surface.min_y, surface.max_y);
    }
    print_named_counts("top atlas biomes:", &summary.atlas_biomes);
    print_named_counts("top atlas archetypes:", &summary.atlas_archetypes);
    print_named_counts("top final blocks:", &summary.final_blocks);
    println!("output: {}", output.display());
    println!("image: {}x{}", image.width(), image.height());

    Ok(())
}

#[derive(Debug, Clone)]
struct AtlasPanel {
    image: RgbImage,
    biome_counts: Vec<NamedColorCount>,
    archetype_counts: Vec<NamedColorCount>,
}

#[derive(Debug, Clone)]
struct RealizationPanel {
    image: RgbImage,
    summary: RealizationSummary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RealizationSummary {
    uplift: ChannelStats,
    flatness: ChannelStats,
    relief: ChannelStats,
    wetness: ChannelStats,
    ridge: ChannelStats,
    terrace: ChannelStats,
    corridor: ChannelStats,
}

#[derive(Debug, Clone)]
struct ChunkPanel {
    image: RgbImage,
    block_counts: Vec<NamedColorCount>,
    surface_range: Option<new_world::world::TopdownSurfaceRange>,
}

fn render_atlas_panel(
    inputs_by_chunk: &HashMap<(i32, i32), ChunkGenerationInputs>,
    window: PreviewWindow,
) -> Result<AtlasPanel, Box<dyn Error>> {
    let blocks = window.blocks_per_axis()?;
    let blocks_usize = usize::try_from(blocks).map_err(|_| cli_error("block width overflowed"))?;
    let mut samples = Vec::with_capacity(blocks_usize * blocks_usize);
    let mut biome_counts = BTreeMap::<String, (usize, [u8; 3])>::new();
    let mut archetype_counts = BTreeMap::<String, (usize, [u8; 3])>::new();

    for block_z in 0..blocks_usize {
        let world_z = window.min_world_z() + block_z as i32;
        for block_x in 0..blocks_usize {
            let world_x = window.min_world_x() + block_x as i32;
            let inputs = inputs_for_world_xz(inputs_by_chunk, world_x, world_z);
            let region = sample_region_classes(&inputs.region_classes, world_x, world_z);
            let color = biome_semantic_color(region);
            increment_color_count(
                &mut biome_counts,
                enum_key(region.biome_family),
                biome_base_color(region.biome_family),
            );
            increment_color_count(&mut archetype_counts, enum_key(region.archetype), color);
            samples.push(region);
        }
    }

    let mut image = blank_panel_image(window)?;
    for block_z in 0..blocks_usize {
        for block_x in 0..blocks_usize {
            let index = block_z * blocks_usize + block_x;
            let region = samples[index];
            let base = biome_semantic_color(region);
            let transition = atlas_transition_strength(&samples, blocks_usize, block_x, block_z);
            let color = draw_grid_adjusted_block_color(base, window, block_x, block_z);
            draw_scaled_block(
                &mut image,
                block_x as u32,
                block_z as u32,
                window.pixels_per_block,
                darken(color, transition),
            );
        }
    }

    Ok(AtlasPanel {
        image,
        biome_counts: sorted_color_counts(biome_counts),
        archetype_counts: sorted_color_counts(archetype_counts),
    })
}

fn render_realization_panel(
    inputs_by_chunk: &HashMap<(i32, i32), ChunkGenerationInputs>,
    patches: &HashMap<(i32, i32), new_world::world::ChunkRealizationFieldPatch>,
    window: PreviewWindow,
) -> Result<RealizationPanel, Box<dyn Error>> {
    let blocks = window.blocks_per_axis()?;
    let blocks_usize = usize::try_from(blocks).map_err(|_| cli_error("block width overflowed"))?;
    let mut samples = Vec::with_capacity(blocks_usize * blocks_usize);

    for block_z in 0..blocks_usize {
        let world_z = window.min_world_z() + block_z as i32;
        for block_x in 0..blocks_usize {
            let world_x = window.min_world_x() + block_x as i32;
            let inputs = inputs_for_world_xz(inputs_by_chunk, world_x, world_z);
            let patch = patch_for_world_xz(patches, world_x, world_z);
            let region = sample_region_classes(&inputs.region_classes, world_x, world_z);
            let realization =
                sample_chunk_realization_field(patch, world_x as f32 + 0.5, world_z as f32 + 0.5);
            samples.push(realization_preview_sample(region, realization));
        }
    }

    let summary = summarize_realization(&samples);
    let normalized = samples
        .iter()
        .copied()
        .map(|sample| normalize_realization_sample(sample, summary))
        .collect::<Vec<_>>();
    let mut image = blank_panel_image(window)?;
    for block_z in 0..blocks_usize {
        for block_x in 0..blocks_usize {
            let index = block_z * blocks_usize + block_x;
            let sample = normalized[index];
            let base = realization_composite_color(sample);
            let transition =
                realization_transition_strength(&normalized, blocks_usize, block_x, block_z);
            let color = draw_grid_adjusted_block_color(base, window, block_x, block_z);
            draw_scaled_block(
                &mut image,
                block_x as u32,
                block_z as u32,
                window.pixels_per_block,
                darken(color, transition),
            );
        }
    }

    Ok(RealizationPanel { image, summary })
}

fn render_chunk_panel(
    world: &WorldCore,
    registry: &BlockRegistry,
    window: PreviewWindow,
) -> Result<ChunkPanel, Box<dyn Error>> {
    let blocks = window.blocks_per_axis()?;
    let blocks_usize = usize::try_from(blocks).map_err(|_| cli_error("block width overflowed"))?;
    let columns = sample_topdown_columns(
        world,
        registry,
        window.min_world_x(),
        window.min_world_z(),
        blocks,
        blocks,
        window.min_world_y(),
        window.max_world_y(),
    );
    let surface_range = topdown_surface_range(&columns);
    let range = surface_range.ok_or_else(|| {
        cli_error("no non-air blocks were found inside the requested final chunk scan window")
    })?;
    let mut block_counts = BTreeMap::<String, (usize, [u8; 3])>::new();
    let mut image = blank_panel_image(window)?;

    for block_z in 0..blocks_usize {
        for block_x in 0..blocks_usize {
            let index = block_z * blocks_usize + block_x;
            let column = columns[index];
            let base = color_topdown_cell(column.visible, registry, range);
            if let Some(key) = block_key_for_column(column, registry) {
                increment_color_count(&mut block_counts, key, base);
            }
            let pixel_origin_x = block_x as u32 * window.pixels_per_block;
            let pixel_origin_y = block_z as u32 * window.pixels_per_block;
            for local_y in 0..window.pixels_per_block {
                for local_x in 0..window.pixels_per_block {
                    let outline = topdown_outline_strength(
                        &columns,
                        blocks_usize,
                        blocks_usize,
                        block_x,
                        block_z,
                        local_x,
                        local_y,
                        window.pixels_per_block,
                    );
                    image.put_pixel(
                        pixel_origin_x + local_x,
                        pixel_origin_y + local_y,
                        Rgb(darken(base, outline)),
                    );
                }
            }
        }
    }

    Ok(ChunkPanel {
        image,
        block_counts: sorted_color_counts(block_counts),
        surface_range,
    })
}

fn compose_image(
    atlas: &AtlasPanel,
    realization: &RealizationPanel,
    chunk: &ChunkPanel,
    window: PreviewWindow,
) -> Result<RgbImage, Box<dyn Error>> {
    let (panel_w, panel_h) = window.panel_image_dimensions()?;
    let panel_area_w = window.panel_area_width()?;
    let width = OUTER_MARGIN * 2 + panel_area_w * 3 + PANEL_GAP * 2;
    let height = OUTER_MARGIN * 2 + TITLE_HEIGHT + panel_h + LEGEND_GAP + LEGEND_HEIGHT;
    let mut image = RgbImage::from_pixel(width, height, Rgb([20, 24, 30]));
    let panel_y = OUTER_MARGIN + TITLE_HEIGHT;

    draw_panel(
        &mut image,
        atlas.image.clone(),
        "ATLAS BIOME",
        0,
        panel_area_w,
        panel_w,
        panel_y,
    );
    draw_panel(
        &mut image,
        realization.image.clone(),
        "REALIZATION",
        1,
        panel_area_w,
        panel_w,
        panel_y,
    );
    draw_panel(
        &mut image,
        chunk.image.clone(),
        "FINAL CHUNK",
        2,
        panel_area_w,
        panel_w,
        panel_y,
    );

    let legend_y = panel_y + panel_h + LEGEND_GAP;
    draw_atlas_legend(&mut image, atlas, panel_origin_x(0, panel_area_w), legend_y);
    draw_realization_legend(
        &mut image,
        realization,
        panel_origin_x(1, panel_area_w),
        legend_y,
    );
    draw_chunk_legend(&mut image, chunk, panel_origin_x(2, panel_area_w), legend_y);

    Ok(image)
}

fn draw_panel(
    target: &mut RgbImage,
    panel: RgbImage,
    title: &str,
    panel_index: u32,
    panel_area_w: u32,
    panel_w: u32,
    panel_y: u32,
) {
    let area_x = panel_origin_x(panel_index, panel_area_w);
    let panel_x = area_x + (panel_area_w - panel_w) / 2;
    draw_text(target, area_x, OUTER_MARGIN, title, [236, 240, 232], 2);
    blit(target, &panel, panel_x, panel_y);
    draw_rect_outline(
        target,
        panel_x.saturating_sub(1),
        panel_y.saturating_sub(1),
        panel.width() + 2,
        panel.height() + 2,
        [78, 88, 102],
    );
}

fn draw_atlas_legend(target: &mut RgbImage, panel: &AtlasPanel, x: u32, y: u32) {
    let mut cursor_y = y;
    draw_text(target, x, cursor_y, "BIOME COLOR", [214, 222, 214], 1);
    cursor_y += 12;
    draw_text(
        target,
        x,
        cursor_y,
        "TINT: HYDRO/TERRAIN",
        [178, 190, 184],
        1,
    );
    cursor_y += 12;
    draw_text(target, x, cursor_y, "LINES: BOUNDS", [178, 190, 184], 1);
    cursor_y += 18;
    draw_text(target, x, cursor_y, "TOP BIOMES", [236, 224, 170], 1);
    cursor_y += 12;
    draw_count_entries(target, x, cursor_y, &panel.biome_counts, 2);
}

fn draw_realization_legend(target: &mut RgbImage, panel: &RealizationPanel, x: u32, y: u32) {
    let mut cursor_y = y;
    draw_text(target, x, cursor_y, "R UPLIFT/RIDGE", [236, 182, 160], 1);
    cursor_y += 12;
    draw_text(target, x, cursor_y, "G FLAT/LOWLAND", [178, 224, 168], 1);
    cursor_y += 12;
    draw_text(target, x, cursor_y, "B WET/CORRIDOR", [160, 198, 236], 1);
    cursor_y += 12;
    draw_text(target, x, cursor_y, "BRIGHT RELIEF", [216, 218, 210], 1);
    cursor_y += 14;
    draw_stat_line(target, x, cursor_y, "REL", panel.summary.relief);
    cursor_y += 11;
    draw_stat_line(target, x, cursor_y, "WET", panel.summary.wetness);
    cursor_y += 11;
    draw_stat_line(target, x, cursor_y, "COR", panel.summary.corridor);
}

fn draw_chunk_legend(target: &mut RgbImage, panel: &ChunkPanel, x: u32, y: u32) {
    let mut cursor_y = y;
    draw_text(target, x, cursor_y, "TOP BLOCK COLOR", [214, 222, 214], 1);
    cursor_y += 12;
    draw_text(target, x, cursor_y, "BRIGHT: SURFACE Y", [178, 190, 184], 1);
    cursor_y += 12;
    draw_text(target, x, cursor_y, "LINES: EDGES", [178, 190, 184], 1);
    cursor_y += 18;
    if let Some(range) = panel.surface_range {
        draw_text(
            target,
            x,
            cursor_y,
            &format!("SURFACE Y {}..{}", range.min_y, range.max_y),
            [236, 224, 170],
            1,
        );
    } else {
        draw_text(target, x, cursor_y, "SURFACE Y NONE", [236, 224, 170], 1);
    }
    cursor_y += 14;
    draw_count_entries(target, x, cursor_y, &panel.block_counts, 2);
}

fn draw_count_entries(
    target: &mut RgbImage,
    x: u32,
    mut y: u32,
    counts: &[NamedColorCount],
    limit: usize,
) {
    for count in counts.iter().take(limit) {
        fill_rect(target, x, y + 1, 10, 8, count.color);
        draw_rect_outline(target, x, y + 1, 10, 8, [20, 24, 30]);
        draw_text(
            target,
            x + 14,
            y,
            &format!("{} {}", count.key, count.count),
            [214, 222, 214],
            1,
        );
        y += 12;
    }
}

fn draw_stat_line(target: &mut RgbImage, x: u32, y: u32, label: &str, stats: ChannelStats) {
    draw_text(
        target,
        x,
        y,
        &format!("{} AVG {:.2}", label, stats.avg),
        [208, 216, 208],
        1,
    );
}

fn generate_final_chunks(
    world: &mut WorldCore,
    registry: &BlockRegistry,
    input_cache: &PreviewInputCache,
    window: PreviewWindow,
) {
    let generated = preview_chunk_xz_coords(window.center_x, window.center_z, window.radius)
        .into_par_iter()
        .flat_map(|(chunk_x, chunk_z)| {
            let plan_coord = ChunkCoord(chunk_x, 0, chunk_z);
            let inputs = input_cache.inputs_for_chunk(plan_coord);
            let plan = build_chunk_generation_voxelization_plan(&inputs);
            (window.min_y_chunk..=window.max_y_chunk)
                .map(|chunk_y| {
                    let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                    let chunk = generate_chunk_from_voxelization_plan(coord, &plan, registry);
                    (coord, chunk)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (coord, chunk) in generated {
        world.insert_chunk(coord, chunk);
    }
}

fn build_chunk_inputs(
    input_cache: &PreviewInputCache,
    window: PreviewWindow,
) -> HashMap<(i32, i32), ChunkGenerationInputs> {
    preview_chunk_xz_coords(window.center_x, window.center_z, window.radius)
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            let coord = ChunkCoord(chunk_x, 0, chunk_z);
            ((chunk_x, chunk_z), input_cache.inputs_for_chunk(coord))
        })
        .collect()
}

fn build_realization_patches(
    inputs_by_chunk: &HashMap<(i32, i32), ChunkGenerationInputs>,
) -> HashMap<(i32, i32), new_world::world::ChunkRealizationFieldPatch> {
    inputs_by_chunk
        .par_iter()
        .map(|(&(chunk_x, chunk_z), inputs)| {
            let coord = ChunkCoord(chunk_x, 0, chunk_z);
            (
                (chunk_x, chunk_z),
                build_chunk_realization_field_patch(coord, inputs),
            )
        })
        .collect()
}

fn inputs_for_world_xz(
    inputs_by_chunk: &HashMap<(i32, i32), ChunkGenerationInputs>,
    world_x: i32,
    world_z: i32,
) -> &ChunkGenerationInputs {
    let chunk_x = world_x.div_euclid(CHUNK_EDGE_I32);
    let chunk_z = world_z.div_euclid(CHUNK_EDGE_I32);
    inputs_by_chunk
        .get(&(chunk_x, chunk_z))
        .expect("preview input map should cover every sampled world block")
}

fn patch_for_world_xz(
    patches: &HashMap<(i32, i32), new_world::world::ChunkRealizationFieldPatch>,
    world_x: i32,
    world_z: i32,
) -> &new_world::world::ChunkRealizationFieldPatch {
    let chunk_x = world_x.div_euclid(CHUNK_EDGE_I32);
    let chunk_z = world_z.div_euclid(CHUNK_EDGE_I32);
    patches
        .get(&(chunk_x, chunk_z))
        .expect("realization patch map should cover every sampled world block")
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

#[derive(Debug, Clone)]
struct PreviewInputCache {
    entries: HashMap<PreviewInputCacheKey, ChunkGenerationInputs>,
}

impl PreviewInputCache {
    fn for_preview_window(meta: &WorldMeta, center_x: i32, center_z: i32, radius: i32) -> Self {
        let mut representatives = HashMap::<PreviewInputCacheKey, ChunkCoord>::new();
        for (chunk_x, chunk_z) in preview_chunk_xz_coords(center_x, center_z, radius) {
            let coord = ChunkCoord(chunk_x, 0, chunk_z);
            representatives
                .entry(PreviewInputCacheKey::for_chunk(coord))
                .or_insert(coord);
        }

        let entries = representatives
            .into_par_iter()
            .map(|(key, coord)| (key, prepare_chunk_generation_inputs(coord, meta)))
            .collect::<HashMap<_, _>>();

        Self { entries }
    }

    fn inputs_for_chunk(&self, coord: ChunkCoord) -> ChunkGenerationInputs {
        let key = PreviewInputCacheKey::for_chunk(coord);
        let mut inputs = self
            .entries
            .get(&key)
            .expect("preview input cache should cover every requested chunk")
            .clone();
        inputs.chunk = coord;
        inputs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PreviewInputCacheKey {
    origin_x: i32,
    origin_z: i32,
    width: u32,
    height: u32,
}

impl PreviewInputCacheKey {
    fn for_chunk(coord: ChunkCoord) -> Self {
        let area = chunk_generation_input_area(coord);
        Self {
            origin_x: area.origin().x,
            origin_z: area.origin().z,
            width: area.width(),
            height: area.height(),
        }
    }
}

fn realization_preview_sample(
    region: RegionClassSample,
    realization: RealizationSample,
) -> RealizationPreviewSample {
    RealizationPreviewSample {
        region,
        uplift: realization_uplift_signal(realization),
        flatness: realization_flatness_signal(realization),
        relief: realization_relief_signal(realization),
        wetness: realization_wetness_signal(realization),
        ridge: realization_ridge_signal(realization),
        terrace: realization_terrace_signal(realization),
        corridor: realization_corridor_signal(realization),
    }
}

fn summarize_realization(samples: &[RealizationPreviewSample]) -> RealizationSummary {
    RealizationSummary {
        uplift: channel_stats(samples, |sample| sample.uplift),
        flatness: channel_stats(samples, |sample| sample.flatness),
        relief: channel_stats(samples, |sample| sample.relief),
        wetness: channel_stats(samples, |sample| sample.wetness),
        ridge: channel_stats(samples, |sample| sample.ridge),
        terrace: channel_stats(samples, |sample| sample.terrace),
        corridor: channel_stats(samples, |sample| sample.corridor),
    }
}

fn channel_stats<F>(samples: &[RealizationPreviewSample], sample_value: F) -> ChannelStats
where
    F: Fn(RealizationPreviewSample) -> f32,
{
    if samples.is_empty() {
        return ChannelStats {
            min: 0.0,
            max: 0.0,
            avg: 0.0,
        };
    }

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0_f32;
    for sample in samples {
        let value = sample_value(*sample);
        min = min.min(value);
        max = max.max(value);
        sum += value;
    }

    ChannelStats {
        min,
        max,
        avg: sum / samples.len() as f32,
    }
}

fn normalize_realization_sample(
    sample: RealizationPreviewSample,
    summary: RealizationSummary,
) -> RealizationPreviewSample {
    RealizationPreviewSample {
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
    let carrier = normalize_range(
        sample.low_freq_amp + sample.mid_freq_amp + sample.dune_amp,
        0.0,
        12.0,
    );
    clamp01(wet_flatten * 0.55 + (1.0 - relief_mass) * 0.35 + (1.0 - carrier) * 0.10)
}

fn realization_relief_signal(sample: RealizationSample) -> f32 {
    let broad = normalize_range(sample.relief_base + sample.relief_gain * 2.0, 4.0, 30.0);
    let carrier = normalize_range(
        sample.low_freq_amp + sample.mid_freq_amp + sample.dune_amp,
        0.0,
        12.0,
    );
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

fn realization_composite_color(sample: RealizationPreviewSample) -> [u8; 3] {
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

fn atlas_transition_strength(
    samples: &[RegionClassSample],
    width: usize,
    x: usize,
    z: usize,
) -> f32 {
    let index = z * width + x;
    let current = samples[index];
    let mut strength = 0.0_f32;
    if x > 0 {
        strength = strength.max(region_difference_strength(current, samples[index - 1]));
    }
    if z > 0 {
        strength = strength.max(region_difference_strength(current, samples[index - width]));
    }
    strength
}

fn region_difference_strength(left: RegionClassSample, right: RegionClassSample) -> f32 {
    if left.archetype != right.archetype {
        0.22
    } else if left.terrain_form_family != right.terrain_form_family {
        0.16
    } else if left.biome_family != right.biome_family {
        0.12
    } else if left.hydrology_context != right.hydrology_context {
        0.08
    } else {
        0.0
    }
}

fn realization_transition_strength(
    samples: &[RealizationPreviewSample],
    width: usize,
    x: usize,
    z: usize,
) -> f32 {
    let index = z * width + x;
    let current = samples[index];
    let mut strength = 0.0_f32;
    if x > 0 {
        strength = strength.max(realization_difference_strength(current, samples[index - 1]));
    }
    if z > 0 {
        strength = strength.max(realization_difference_strength(
            current,
            samples[index - width],
        ));
    }
    strength
}

fn realization_difference_strength(
    left: RealizationPreviewSample,
    right: RealizationPreviewSample,
) -> f32 {
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

fn draw_grid_adjusted_block_color(
    color: [u8; 3],
    window: PreviewWindow,
    block_x: usize,
    block_z: usize,
) -> [u8; 3] {
    let world_x = window.min_world_x() + block_x as i32;
    let world_z = window.min_world_z() + block_z as i32;
    let atlas_span_blocks = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
    let mut strength = 0.0_f32;
    if world_x.rem_euclid(CHUNK_EDGE_I32) == 0 || world_z.rem_euclid(CHUNK_EDGE_I32) == 0 {
        strength = strength.max(0.05);
    }
    if world_x.rem_euclid(atlas_span_blocks) == 0 || world_z.rem_euclid(atlas_span_blocks) == 0 {
        strength = strength.max(0.16);
    }
    darken(color, strength)
}

fn block_key_for_column(column: TopdownColumnScan, registry: &BlockRegistry) -> Option<String> {
    column
        .visible
        .top_y
        .map(|_| registry.block_or_missing(column.visible.block).key.clone())
}

fn blank_panel_image(window: PreviewWindow) -> Result<RgbImage, Box<dyn Error>> {
    let (width, height) = window.panel_image_dimensions()?;
    Ok(RgbImage::from_pixel(width, height, Rgb([18, 22, 28])))
}

fn draw_scaled_block(
    image: &mut RgbImage,
    block_x: u32,
    block_z: u32,
    scale_px: u32,
    color: [u8; 3],
) {
    let origin_x = block_x * scale_px;
    let origin_y = block_z * scale_px;
    for local_y in 0..scale_px {
        for local_x in 0..scale_px {
            image.put_pixel(origin_x + local_x, origin_y + local_y, Rgb(color));
        }
    }
}

fn panel_origin_x(index: u32, panel_area_w: u32) -> u32 {
    OUTER_MARGIN + index * (panel_area_w + PANEL_GAP)
}

fn blit(target: &mut RgbImage, source: &RgbImage, origin_x: u32, origin_y: u32) {
    for y in 0..source.height() {
        for x in 0..source.width() {
            let pixel = *source.get_pixel(x, y);
            put_pixel_safe(target, origin_x + x, origin_y + y, pixel.0);
        }
    }
}

fn fill_rect(target: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: [u8; 3]) {
    for dy in 0..height {
        for dx in 0..width {
            put_pixel_safe(target, x + dx, y + dy, color);
        }
    }
}

fn draw_rect_outline(
    target: &mut RgbImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 3],
) {
    if width == 0 || height == 0 {
        return;
    }
    for dx in 0..width {
        put_pixel_safe(target, x + dx, y, color);
        put_pixel_safe(target, x + dx, y + height - 1, color);
    }
    for dy in 0..height {
        put_pixel_safe(target, x, y + dy, color);
        put_pixel_safe(target, x + width - 1, y + dy, color);
    }
}

fn put_pixel_safe(target: &mut RgbImage, x: u32, y: u32, color: [u8; 3]) {
    if x < target.width() && y < target.height() {
        target.put_pixel(x, y, Rgb(color));
    }
}

fn draw_text(target: &mut RgbImage, x: u32, y: u32, text: &str, color: [u8; 3], scale_px: u32) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_char(target, cursor_x, y, ch, color, scale_px);
        cursor_x += 6 * scale_px;
    }
}

fn draw_char(target: &mut RgbImage, x: u32, y: u32, ch: char, color: [u8; 3], scale_px: u32) {
    let glyph = glyph(ch.to_ascii_uppercase());
    for (row, mask) in glyph.iter().copied().enumerate() {
        for col in 0..5 {
            if (mask & (1 << (4 - col))) == 0 {
                continue;
            }
            fill_rect(
                target,
                x + col * scale_px,
                y + row as u32 * scale_px,
                scale_px,
                scale_px,
                color,
            );
        }
    }
}

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '=' => [
            0b00000, 0b11111, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        '>' => [
            0b10000, 0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0b10000,
        ],
        '<' => [
            0b00001, 0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0b00001,
        ],
        ' ' => [0; 7],
        _ => [
            0b11111, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn increment_color_count(
    counts: &mut BTreeMap<String, (usize, [u8; 3])>,
    key: String,
    color: [u8; 3],
) {
    let entry = counts.entry(key).or_insert((0, color));
    entry.0 += 1;
}

fn sorted_color_counts(counts: BTreeMap<String, (usize, [u8; 3])>) -> Vec<NamedColorCount> {
    let mut counts = counts
        .into_iter()
        .map(|(key, (count, color))| NamedColorCount { key, count, color })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    counts
}

fn print_named_counts(label: &str, counts: &[NamedColorCount]) {
    println!("{label}");
    for count in counts.iter().take(8) {
        println!("  {}: {}", count.key, count.count);
    }
}

fn print_channel_stats(label: &str, stats: ChannelStats) {
    println!(
        "{label}: min {:.3}, max {:.3}, avg {:.3}",
        stats.min, stats.max, stats.avg
    );
}

fn atlas_coord_for_world(world_x: i32, world_z: i32) -> AtlasCoord {
    let atlas_span_blocks = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
    AtlasCoord::new(
        world_x.div_euclid(atlas_span_blocks),
        world_z.div_euclid(atlas_span_blocks),
    )
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

fn default_output_path(
    seed: u64,
    center_x: i32,
    center_z: i32,
    radius: i32,
    pixels_per_block: u32,
) -> PathBuf {
    PathBuf::from(format!(
        "target/atlas-realization-chunk-topdown-preview/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}_ppb{pixels_per_block}.png"
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
                            && next
                                .map(|value| value.is_ascii_lowercase())
                                .unwrap_or(false)))
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
    "usage: cargo run --bin atlas_realization_chunk_topdown_preview -- <seed> [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--pixels-per-block <u32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_uses_same_scale_for_all_panels() {
        let window = PreviewWindow::new(0, 0, 0, -2, 3, 2).unwrap();
        assert_eq!(window.blocks_per_axis().unwrap(), CHUNK_EDGE_I32 as u32);
        assert_eq!(window.panel_image_dimensions().unwrap(), (64, 64));
    }

    #[test]
    fn realization_composite_pushes_blue_for_wet_samples() {
        let dry = realization_composite_color(RealizationPreviewSample {
            region: RegionClassSample::default(),
            uplift: 0.8,
            flatness: 0.2,
            relief: 0.5,
            wetness: 0.1,
            ridge: 0.3,
            terrace: 0.1,
            corridor: 0.0,
        });
        let wet = realization_composite_color(RealizationPreviewSample {
            region: RegionClassSample::default(),
            uplift: 0.2,
            flatness: 0.8,
            relief: 0.5,
            wetness: 0.9,
            ridge: 0.1,
            terrace: 0.1,
            corridor: 0.6,
        });

        assert!(wet[2] > dry[2]);
        assert!(dry[0] > wet[0]);
    }

    #[test]
    fn atlas_palette_keeps_polar_ice_lighter_than_grassland() {
        let polar = biome_base_color(BiomeFamily::PolarIce);
        let grass = biome_base_color(BiomeFamily::TemperateGrassland);
        assert!(polar[0] > grass[0]);
        assert!(polar[1] > grass[1]);
        assert!(polar[2] > grass[2]);
    }
}
