use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};

use super::atlas_fields::{AtlasCell, AtlasFieldMap};
use super::atlas_resolver::{
    AtlasResolvedMap, BiomePreview, MoistureClass, OverlayClass, ThermalClass,
};
use super::tuning::AtlasTuning;

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
    write_debug_images_with_options_and_tuning(
        fields,
        resolved,
        output_dir,
        AtlasDebugOptions::default(),
        &AtlasTuning::default(),
    )
}

pub fn write_debug_images_with_options(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    output_dir: impl AsRef<Path>,
    options: AtlasDebugOptions,
) -> Result<Vec<PathBuf>, AtlasDebugError> {
    write_debug_images_with_options_and_tuning(
        fields,
        resolved,
        output_dir,
        options,
        &AtlasTuning::default(),
    )
}

pub fn write_debug_images_with_options_and_tuning(
    fields: &AtlasFieldMap,
    resolved: &AtlasResolvedMap,
    output_dir: impl AsRef<Path>,
    options: AtlasDebugOptions,
    tuning: &AtlasTuning,
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
        (
            "07_biome_preview.png",
            render_biome_preview(fields, resolved, options, tuning),
        ),
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
    tuning: &AtlasTuning,
) -> RgbImage {
    render_map_with_resolved(fields, resolved, options, |cell, resolved, _, _| {
        let signed_height = preview_signed_height(cell, tuning);
        biome_preview_color(cell, resolved.biome, signed_height, tuning)
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

fn biome_preview_color(
    cell: &AtlasCell,
    biome: BiomePreview,
    signed_height: f32,
    tuning: &AtlasTuning,
) -> [u8; 3] {
    let preview = tuning.preview;
    let abs_height = signed_height.abs().clamp(0.0, 1.0);
    let base_color = match biome {
        BiomePreview::Ocean => shade_from_height(
            preview.ocean_light,
            preview.ocean_dark,
            abs_height,
            preview.ocean_shade_strength,
        ),
        BiomePreview::Coast => shade_from_height(
            preview.coast_light,
            preview.coast_dark,
            abs_height,
            preview.coast_shade_strength,
        ),
        BiomePreview::PolarTundra => shade_from_height(
            preview.polar_light,
            preview.polar_dark,
            abs_height,
            preview.polar_shade_strength,
        ),
        BiomePreview::Desert => shade_from_height(
            preview.desert_light,
            preview.desert_dark,
            abs_height,
            preview.desert_shade_strength,
        ),
        BiomePreview::Wetland => shade_from_height(
            preview.wetland_light,
            preview.wetland_dark,
            abs_height,
            preview.wetland_shade_strength,
        ),
        BiomePreview::Riverplain => shade_from_height(
            preview.riverplain_light,
            preview.riverplain_dark,
            abs_height,
            preview.riverplain_shade_strength,
        ),
        BiomePreview::Alpine => shade_from_height(
            preview.alpine_light,
            preview.alpine_dark,
            abs_height,
            preview.alpine_shade_strength,
        ),
        BiomePreview::Mountain => shade_from_height(
            preview.mountain_light,
            preview.mountain_dark,
            abs_height,
            preview.mountain_shade_strength,
        ),
        BiomePreview::Steppe => shade_from_height(
            preview.steppe_light,
            preview.steppe_dark,
            abs_height,
            preview.steppe_shade_strength,
        ),
        BiomePreview::Grassland => shade_from_height(
            preview.grassland_light,
            preview.grassland_dark,
            abs_height,
            preview.grassland_shade_strength,
        ),
        BiomePreview::TemperateForest => shade_from_height(
            preview.temperate_forest_light,
            preview.temperate_forest_dark,
            abs_height,
            preview.temperate_forest_shade_strength,
        ),
        BiomePreview::BorealForest => shade_from_height(
            preview.boreal_forest_light,
            preview.boreal_forest_dark,
            abs_height,
            preview.boreal_forest_shade_strength,
        ),
        BiomePreview::TropicalForest => shade_from_height(
            preview.tropical_forest_light,
            preview.tropical_forest_dark,
            abs_height,
            preview.tropical_forest_shade_strength,
        ),
    };

    let color = apply_mountain_range_shading(base_color, cell, biome, signed_height, preview);

    if matches!(biome, BiomePreview::Ocean | BiomePreview::Coast | BiomePreview::Desert | BiomePreview::PolarTundra) {
        color
    } else if cell.overlay.riverine > preview.river_tint_threshold {
        mix(
            color,
            preview.river_tint,
            (cell.overlay.riverine * preview.river_tint_strength)
                .clamp(0.0, preview.river_tint_strength),
        )
    } else {
        color
    }
}

fn apply_mountain_range_shading(
    color: [u8; 3],
    cell: &AtlasCell,
    biome: BiomePreview,
    signed_height: f32,
    preview: super::tuning::AtlasPreviewDebugTuning,
) -> [u8; 3] {
    if matches!(biome, BiomePreview::Ocean | BiomePreview::Coast | BiomePreview::PolarTundra) {
        return color;
    }

    let range_signal = clamp01(
        cell.ridge_factor * preview.mountain_range_ridge_weight
            + cell.mountain_mass * preview.mountain_range_mass_weight
            + cell.form.mountain * preview.mountain_range_form_weight,
    );
    let positive_height = signed_height.max(0.0);
    let biome_bonus = match biome {
        BiomePreview::Mountain => 1.00,
        BiomePreview::Alpine => 0.92,
        _ => 0.72,
    };
    let shade = smoothstep(preview.mountain_range_threshold, 1.0, range_signal)
        * positive_height
        * preview.mountain_range_strength
        * biome_bonus;

    mix(color, preview.mountain_dark, shade.clamp(0.0, 0.90))
}

fn preview_signed_height(cell: &AtlasCell, tuning: &AtlasTuning) -> f32 {
    let preview = tuning.preview;
    if cell.overlay.ocean > 0.5 {
        let depth = (cell.coast_distance * preview.ocean_depth_coast_weight
            + (1.0 - cell.landness) * preview.ocean_depth_landness_weight)
            .clamp(0.0, 1.0);
        -depth
    } else {
        let height = (
            cell.macro_elevation * preview.land_height_macro_weight
                + cell.mountain_mass * preview.land_height_mountain_weight
                + cell.ruggedness * preview.land_height_ruggedness_weight
                + cell.alpine_factor * preview.land_height_alpine_weight
                - cell.coast_factor * preview.land_height_coast_penalty
        )
        .clamp(0.0, 1.0);
        height
    }
}

fn shade_from_height(light: [u8; 3], dark: [u8; 3], height: f32, strength: f32) -> [u8; 3] {
    mix(light, dark, (height.clamp(0.0, 1.0) * strength).clamp(0.0, 1.0))
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if edge0 >= edge1 {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = clamp01((value - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
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

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::atlas_fields::TerrainFormWeights;

    fn brightness(color: [u8; 3]) -> u32 {
        color[0] as u32 + color[1] as u32 + color[2] as u32
    }

    #[test]
    fn biome_preview_darkens_mountainous_land_more_than_flat_land() {
        let tuning = AtlasTuning::default();
        let flat = AtlasCell {
            ridge_factor: 0.08,
            mountain_mass: 0.06,
            form: TerrainFormWeights {
                plain: 0.92,
                hill: 0.08,
                mountain: 0.0,
            },
            ..AtlasCell::default()
        };
        let mountainous = AtlasCell {
            ridge_factor: 0.84,
            mountain_mass: 0.88,
            form: TerrainFormWeights {
                plain: 0.0,
                hill: 0.10,
                mountain: 0.90,
            },
            ..AtlasCell::default()
        };

        let flat_color = biome_preview_color(
            &flat,
            BiomePreview::TemperateForest,
            0.76,
            &tuning,
        );
        let mountainous_color = biome_preview_color(
            &mountainous,
            BiomePreview::TemperateForest,
            0.76,
            &tuning,
        );

        assert!(brightness(mountainous_color) < brightness(flat_color));
    }
}
