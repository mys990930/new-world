use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};

use super::atlas_fields::{AtlasCell, AtlasFieldMap};
use super::atlas_resolver::{
    AtlasResolvedMap, BiomePreview, MoistureClass, OverlayClass, ThermalClass,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasDebugOptions {
    pub pixels_per_cell: u32,
}

impl Default for AtlasDebugOptions {
    fn default() -> Self {
        Self { pixels_per_cell: 4 }
    }
}

#[derive(Debug)]
pub enum AtlasDebugError {
    Io(std::io::Error),
    Image(image::ImageError),
}

impl Display for AtlasDebugError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "atlas debug IO failed: {error}"),
            Self::Image(error) => write!(f, "atlas debug image encode failed: {error}"),
        }
    }
}

impl Error for AtlasDebugError {}

impl From<std::io::Error> for AtlasDebugError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<image::ImageError> for AtlasDebugError {
    fn from(error: image::ImageError) -> Self {
        Self::Image(error)
    }
}

pub fn write_debug_images(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    output_dir: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, AtlasDebugError> {
    write_debug_images_with_options(fields, resolved, output_dir, AtlasDebugOptions::default())
}

pub fn write_debug_images_with_options(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    output_dir: impl AsRef<Path>,
    options: AtlasDebugOptions,
) -> Result<Vec<PathBuf>, AtlasDebugError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;

    let images = [
        ("00_landness.png", render_landness(fields, options)),
        ("01_elevation.png", render_elevation(fields, options)),
        ("02_ridge.png", render_ridge(fields, options)),
        ("03_hydrology.png", render_hydrology(fields, options)),
        ("04_temperature.png", render_temperature(fields, resolved, options)),
        ("05_humidity.png", render_humidity(fields, resolved, options)),
        ("06_overlay.png", render_overlay(fields, resolved, options)),
        ("07_biome_preview.png", render_biome_preview(fields, resolved, options)),
        ("08_ecotone.png", render_ecotone(fields, options)),
    ];

    let mut paths = Vec::with_capacity(images.len());
    for (name, image) in images {
        let path = output_dir.join(name);
        image.save(&path)?;
        paths.push(path);
    }

    Ok(paths)
}

fn render_landness(fields: &AtlasFieldMap, options: AtlasDebugOptions) -> RgbImage {
    render_map(fields, options, |cell, _, _| {
        if cell.overlay.ocean > 0.5 {
            gradient3(cell.landness, [6, 32, 88], [20, 84, 168], [116, 192, 220])
        } else {
            let elevation = cell.macro_elevation;
            gradient4(
                elevation,
                [210, 198, 148],
                [92, 148, 82],
                [122, 104, 72],
                [244, 246, 244],
            )
        }
    })
}

fn render_elevation(fields: &AtlasFieldMap, options: AtlasDebugOptions) -> RgbImage {
    render_map(fields, options, |cell, _, _| {
        if cell.overlay.ocean > 0.5 {
            gradient3(cell.landness, [8, 24, 60], [18, 64, 128], [72, 142, 212])
        } else {
            gradient5(
                cell.macro_elevation,
                [42, 94, 54],
                [92, 148, 82],
                [164, 150, 92],
                [132, 104, 78],
                [242, 242, 242],
            )
        }
    })
}

fn render_ridge(fields: &AtlasFieldMap, options: AtlasDebugOptions) -> RgbImage {
    render_map(fields, options, |cell, _, _| {
        if cell.overlay.ocean > 0.5 {
            [18, 40, 84]
        } else {
            let base = grayscale(cell.ridge_factor);
            mix(base, [240, 170, 92], cell.mountain_mass * 0.65)
        }
    })
}

fn render_hydrology(fields: &AtlasFieldMap, options: AtlasDebugOptions) -> RgbImage {
    render_map(fields, options, |cell, _, _| {
        if cell.overlay.ocean > 0.5 {
            return [10, 46, 108];
        }

        let mut color = gradient3(cell.macro_elevation, [84, 126, 74], [146, 162, 102], [182, 170, 136]);
        if cell.wetland_factor > 0.52 {
            color = mix(color, [72, 122, 88], 0.55);
        }
        if cell.lake_potential > 0.62 {
            color = [54, 120, 178];
        } else if cell.river_flow_potential > 0.045 {
            color = mix(color, [56, 122, 208], (cell.riverine_factor * 0.85).clamp(0.0, 0.85));
        }
        color
    })
}

fn render_temperature(fields: &AtlasFieldMap, resolved: &AtlasResolvedMap, options: AtlasDebugOptions) -> RgbImage {
    render_map_with_resolved(fields, resolved, options, |cell, resolved, _, _| {
        let base = gradient5(
            cell.temperature,
            [226, 244, 252],
            [132, 186, 224],
            [138, 196, 112],
            [232, 198, 86],
            [210, 92, 56],
        );
        if matches!(resolved.thermal, ThermalClass::Polar) {
            mix(base, [245, 248, 250], 0.45)
        } else {
            base
        }
    })
}

fn render_humidity(fields: &AtlasFieldMap, resolved: &AtlasResolvedMap, options: AtlasDebugOptions) -> RgbImage {
    render_map_with_resolved(fields, resolved, options, |cell, resolved, _, _| {
        let base = gradient5(
            cell.humidity,
            [194, 164, 108],
            [200, 186, 116],
            [148, 176, 104],
            [82, 152, 118],
            [44, 126, 126],
        );
        if matches!(resolved.moisture, MoistureClass::Wet) {
            mix(base, [52, 116, 142], 0.30)
        } else {
            base
        }
    })
}

fn render_overlay(fields: &AtlasFieldMap, resolved: &AtlasResolvedMap, options: AtlasDebugOptions) -> RgbImage {
    render_map_with_resolved(fields, resolved, options, |cell, resolved, _, _| match resolved.overlay {
        OverlayClass::Ocean => [16, 70, 152],
        OverlayClass::Coast => [220, 204, 138],
        OverlayClass::Riverine => mix([116, 154, 102], [52, 114, 192], cell.riverine_factor),
        OverlayClass::Wetland => [74, 118, 84],
        OverlayClass::Alpine => [188, 190, 194],
        OverlayClass::None => {
            if cell.form.mountain > 0.50 {
                [118, 96, 86]
            } else {
                [116, 156, 98]
            }
        }
    })
}

fn render_biome_preview(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    options: AtlasDebugOptions,
) -> RgbImage {
    render_map_with_resolved(fields, resolved, options, |cell, resolved, _, _| {
        let signed_height = preview_signed_height(cell);
        biome_preview_color(cell, resolved.biome, signed_height)
    })
}

fn render_ecotone(fields: &AtlasFieldMap, options: AtlasDebugOptions) -> RgbImage {
    render_map(fields, options, |cell, _, _| gradient3(cell.ecotone_strength, [22, 26, 30], [112, 126, 94], [246, 236, 170]))
}

fn render_map(
    fields: &AtlasFieldMap,
    options: AtlasDebugOptions,
    mut color_for: impl FnMut(&super::atlas_fields::AtlasCell, u32, u32) -> [u8; 3],
) -> RgbImage {
    let area = fields.area();
    let scale = options.pixels_per_cell.max(1);
    let mut image = RgbImage::new(area.width() * scale, area.height() * scale);

    for (index, cell) in fields.cells().values().iter().enumerate() {
        let x = index as u32 % area.width();
        let z = index as u32 / area.width();
        fill_cell(&mut image, area.width(), scale, index, color_for(cell, x, z));
    }

    image
}

fn render_map_with_resolved(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    options: AtlasDebugOptions,
    mut color_for: impl FnMut(
        &super::atlas_fields::AtlasCell,
        &super::atlas_resolver::AtlasResolvedCell,
        u32,
        u32,
    ) -> [u8; 3],
) -> RgbImage {
    let area = fields.area();
    let scale = options.pixels_per_cell.max(1);
    let mut image = RgbImage::new(area.width() * scale, area.height() * scale);

    for (index, (cell, resolved_cell)) in fields
        .cells()
        .values()
        .iter()
        .zip(resolved.cells().values().iter())
        .enumerate()
    {
        let x = index as u32 % area.width();
        let z = index as u32 / area.width();
        fill_cell(&mut image, area.width(), scale, index, color_for(cell, resolved_cell, x, z));
    }

    image
}

fn biome_preview_color(cell: &AtlasCell, biome: BiomePreview, signed_height: f32) -> [u8; 3] {
    let abs_height = signed_height.abs().clamp(0.0, 1.0);
    let color = match biome {
        BiomePreview::Ocean => depth_shaded_blue(abs_height),
        BiomePreview::Coast => shade_from_height([244, 232, 176], [214, 196, 126], abs_height, 0.52),
        BiomePreview::PolarTundra => shade_from_height([250, 250, 246], [214, 222, 228], abs_height, 0.40),
        BiomePreview::Desert => shade_from_height([238, 150, 64], [166, 88, 26], abs_height, 0.62),
        BiomePreview::Wetland => shade_from_height([82, 138, 98], [40, 86, 66], abs_height, 0.70),
        BiomePreview::Riverplain => shade_from_height([98, 164, 114], [48, 98, 78], abs_height, 0.60),
        BiomePreview::Alpine => shade_from_height([126, 156, 134], [62, 94, 78], abs_height, 0.78),
        BiomePreview::Mountain => shade_from_height([92, 138, 82], [28, 62, 32], abs_height, 0.86),
        BiomePreview::Steppe => shade_from_height([142, 156, 92], [78, 96, 40], abs_height, 0.66),
        BiomePreview::Grassland => shade_from_height([112, 172, 92], [44, 96, 34], abs_height, 0.68),
        BiomePreview::TemperateForest => shade_from_height([74, 136, 72], [20, 58, 24], abs_height, 0.76),
        BiomePreview::BorealForest => shade_from_height([72, 118, 94], [26, 56, 42], abs_height, 0.74),
        BiomePreview::TropicalForest => shade_from_height([46, 154, 82], [12, 68, 30], abs_height, 0.72),
    };

    if matches!(biome, BiomePreview::Ocean | BiomePreview::Coast | BiomePreview::Desert | BiomePreview::PolarTundra) {
        color
    } else if cell.overlay.riverine > 0.56 {
        mix(color, [70, 142, 186], (cell.overlay.riverine * 0.18).clamp(0.0, 0.18))
    } else {
        color
    }
}

fn preview_signed_height(cell: &AtlasCell) -> f32 {
    if cell.overlay.ocean > 0.5 {
        let depth = (cell.coast_distance * 0.72 + (1.0 - cell.landness) * 0.28).clamp(0.0, 1.0);
        -depth
    } else {
        let height = (
            cell.macro_elevation * 0.56
                + cell.mountain_mass * 0.26
                + cell.ruggedness * 0.12
                + cell.alpine_factor * 0.06
                - cell.coast_factor * 0.22
        )
        .clamp(0.0, 1.0);
        height
    }
}

fn depth_shaded_blue(depth: f32) -> [u8; 3] {
    shade_from_height([106, 176, 232], [8, 38, 102], depth, 0.88)
}

fn shade_from_height(light: [u8; 3], dark: [u8; 3], height: f32, strength: f32) -> [u8; 3] {
    mix(light, dark, (height.clamp(0.0, 1.0) * strength).clamp(0.0, 1.0))
}

fn fill_cell(image: &mut RgbImage, map_width: u32, scale: u32, index: usize, color: [u8; 3]) {
    let pixel_x = (index as u32 % map_width) * scale;
    let pixel_y = (index as u32 / map_width) * scale;

    for local_y in 0..scale {
        for local_x in 0..scale {
            image.put_pixel(pixel_x + local_x, pixel_y + local_y, Rgb(color));
        }
    }
}

fn grayscale(value: f32) -> [u8; 3] {
    let channel = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    [channel, channel, channel]
}

fn gradient3(value: f32, a: [u8; 3], b: [u8; 3], c: [u8; 3]) -> [u8; 3] {
    let value = value.clamp(0.0, 1.0);
    if value < 0.5 {
        mix(a, b, value * 2.0)
    } else {
        mix(b, c, (value - 0.5) * 2.0)
    }
}

fn gradient4(value: f32, a: [u8; 3], b: [u8; 3], c: [u8; 3], d: [u8; 3]) -> [u8; 3] {
    let value = value.clamp(0.0, 1.0);
    if value < 0.333_333_34 {
        mix(a, b, value / 0.333_333_34)
    } else if value < 0.666_666_7 {
        mix(b, c, (value - 0.333_333_34) / 0.333_333_34)
    } else {
        mix(c, d, (value - 0.666_666_7) / 0.333_333_34)
    }
}

fn gradient5(value: f32, a: [u8; 3], b: [u8; 3], c: [u8; 3], d: [u8; 3], e: [u8; 3]) -> [u8; 3] {
    let value = value.clamp(0.0, 1.0);
    if value < 0.25 {
        mix(a, b, value / 0.25)
    } else if value < 0.50 {
        mix(b, c, (value - 0.25) / 0.25)
    } else if value < 0.75 {
        mix(c, d, (value - 0.50) / 0.25)
    } else {
        mix(d, e, (value - 0.75) / 0.25)
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        lerp_channel(a[0], b[0], t),
        lerp_channel(a[1], b[1], t),
        lerp_channel(a[2], b[2], t),
    ]
}

fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}
