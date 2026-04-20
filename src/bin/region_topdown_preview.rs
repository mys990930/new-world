use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use image::{Rgb, RgbImage};

use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCoord, BiomeFamily, CHUNK_EDGE_I32,
    CoastalContext, ElevationBand, HydrologyContext, RegionClassMap, RegionClassSample,
    ReliefClass, TerrainFormFamily, WorldMeta, generate_atlas_fields, generate_atlas_structure,
    resolve_region_classes, sample_region_classes,
};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 8;
const DEFAULT_BLOCKS_PER_PIXEL: u32 = 8;

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

    fn generation_area(self) -> Result<AtlasArea, Box<dyn Error>> {
        let sampled = self.sampled_atlas_bounds()?;
        let origin = AtlasCoord::new(sampled.min.x - 1, sampled.min.z - 1);
        let width = u32::try_from(sampled.max.x - sampled.min.x + 3)
            .map_err(|_| cli_error("atlas generation width overflowed"))?;
        let height = u32::try_from(sampled.max.z - sampled.min.z + 3)
            .map_err(|_| cli_error("atlas generation height overflowed"))?;
        AtlasArea::new(origin, width, height)
            .map_err(|error| cli_error(format!("invalid atlas generation area: {error}")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasBounds {
    min: AtlasCoord,
    max: AtlasCoord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreviewCell {
    region: RegionClassSample,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedCount {
    key: String,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewSummary {
    total_pixels: usize,
    sampled_atlas_bounds: AtlasBounds,
    biome_counts: Vec<NamedCount>,
    archetype_counts: Vec<NamedCount>,
    terrain_counts: Vec<NamedCount>,
    hydrology_counts: Vec<NamedCount>,
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
            "--output" => output = Some(PathBuf::from(parse_required::<String>(&mut args, "output")?)),
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    let window = PreviewWindow::new(center_x, center_z, radius, blocks_per_pixel)?;
    let sampled_atlas_bounds = window.sampled_atlas_bounds()?;
    let output = output.unwrap_or_else(|| {
        default_output_path(seed, window.center_x, window.center_z, window.radius, blocks_per_pixel)
    });

    let meta = WorldMeta::new(seed);
    let generation_area = window.generation_area()?;
    let fields = generate_atlas_fields(&meta, generation_area);
    let structure = generate_atlas_structure(&meta, generation_area);
    let regions = resolve_region_classes(&meta, generation_area, &fields, &structure);
    let (image, summary) = render_preview(&regions, window, sampled_atlas_bounds)?;

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
    println!(
        "sampled atlas cells: x={}..{}, z={}..{}",
        summary.sampled_atlas_bounds.min.x,
        summary.sampled_atlas_bounds.max.x,
        summary.sampled_atlas_bounds.min.z,
        summary.sampled_atlas_bounds.max.z
    );
    println!("sample pixels: {}", summary.total_pixels);
    print_named_counts("top biomes:", &summary.biome_counts);
    print_named_counts("top archetypes:", &summary.archetype_counts);
    print_named_counts("top terrain forms:", &summary.terrain_counts);
    print_named_counts("top hydrology contexts:", &summary.hydrology_counts);
    println!("output: {}", output.display());
    println!("image: {}x{}", image.width(), image.height());

    Ok(())
}

fn render_preview(
    regions: &RegionClassMap,
    window: PreviewWindow,
    sampled_atlas_bounds: AtlasBounds,
) -> Result<(RgbImage, PreviewSummary), Box<dyn Error>> {
    let (width, height) = window.image_dimensions()?;
    let width_usize = usize::try_from(width).map_err(|_| cli_error("preview width overflowed"))?;
    let height_usize =
        usize::try_from(height).map_err(|_| cli_error("preview height overflowed"))?;
    let mut cells = Vec::with_capacity(width_usize * height_usize);

    for pixel_z in 0..height {
        for pixel_x in 0..width {
            let world_x = window.sample_world_x(pixel_x);
            let world_z = window.sample_world_z(pixel_z);
            cells.push(PreviewCell {
                region: sample_region_classes(regions, world_x, world_z),
            });
        }
    }

    let summary = summarize_cells(&cells, sampled_atlas_bounds);
    let mut image = RgbImage::new(width, height);
    for pixel_z in 0..height_usize {
        for pixel_x in 0..width_usize {
            let cell = cells[pixel_z * width_usize + pixel_x];
            let base = color_for_region(cell.region);
            let transition = transition_strength(&cells, width_usize, pixel_x, pixel_z);
            let grid = grid_strength(window, pixel_x, pixel_z);
            let color = darken(base, (transition + grid).clamp(0.0, 0.45));
            image.put_pixel(pixel_x as u32, pixel_z as u32, Rgb(color));
        }
    }

    Ok((image, summary))
}

fn summarize_cells(cells: &[PreviewCell], sampled_atlas_bounds: AtlasBounds) -> PreviewSummary {
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
    }
}

fn color_for_region(region: RegionClassSample) -> [u8; 3] {
    let mut color = biome_base_color(region.biome_family);
    color = blend(color, coastal_tint(region.coastal_context), coastal_tint_strength(region.coastal_context));
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

fn transition_strength(cells: &[PreviewCell], width: usize, x: usize, z: usize) -> f32 {
    let index = z * width + x;
    let region = cells[index].region;
    let mut strength = 0.0_f32;

    if x > 0 {
        strength = strength.max(region_difference_strength(region, cells[index - 1].region));
    }
    if z > 0 {
        strength = strength.max(region_difference_strength(region, cells[index - width].region));
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

fn grid_strength(window: PreviewWindow, pixel_x: usize, pixel_z: usize) -> f32 {
    let mut strength = 0.0_f32;
    let block_x = window.min_world_x() + pixel_x as i32 * window.blocks_per_pixel as i32;
    let block_z = window.min_world_z() + pixel_z as i32 * window.blocks_per_pixel as i32;
    let atlas_span_blocks = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;

    if block_x.rem_euclid(CHUNK_EDGE_I32) == 0 || block_z.rem_euclid(CHUNK_EDGE_I32) == 0 {
        strength = strength.max(0.05);
    }
    if block_x.rem_euclid(atlas_span_blocks) == 0 || block_z.rem_euclid(atlas_span_blocks) == 0 {
        strength = strength.max(0.16);
    }

    strength
}

fn atlas_coord_for_world(world_x: i32, world_z: i32) -> AtlasCoord {
    let atlas_span_blocks = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;
    AtlasCoord::new(
        world_x.div_euclid(atlas_span_blocks),
        world_z.div_euclid(atlas_span_blocks),
    )
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

fn print_named_counts(label: &str, counts: &[NamedCount]) {
    println!("{label}");
    for count in counts.iter().take(8) {
        println!("  {}: {}", count.key, count.count);
    }
}

fn blend(base: [u8; 3], tint: [u8; 3], amount: f32) -> [u8; 3] {
    if amount <= 0.0 {
        return base;
    }

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
) -> PathBuf {
    PathBuf::from(format!(
        "target/region-topdown-preview/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}_bpp{blocks_per_pixel}.png"
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
    "usage: cargo run --bin region_topdown_preview -- <seed> [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--blocks-per-pixel <u32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn preview_generation_area_covers_sampled_atlas_bounds() {
        let window =
            PreviewWindow::new(0, 0, DEFAULT_RADIUS, DEFAULT_BLOCKS_PER_PIXEL).unwrap();
        let sampled = window.sampled_atlas_bounds().unwrap();
        let area = window.generation_area().unwrap();

        assert!(area.contains(sampled.min));
        assert!(area.contains(sampled.max));
    }

    #[test]
    fn polar_ice_palette_stays_lighter_than_temperate_grassland() {
        let polar = biome_base_color(BiomeFamily::PolarIce);
        let grass = biome_base_color(BiomeFamily::TemperateGrassland);

        assert!(polar[0] > grass[0]);
        assert!(polar[1] > grass[1]);
        assert!(polar[2] > grass[2]);
    }
}
