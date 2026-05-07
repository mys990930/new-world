#![allow(dead_code)]

use image::{RgbImage, RgbaImage};
use new_world::renderer::OffscreenRenderOutput;

const PANEL_RGB: [u8; 3] = [7, 10, 13];
const PANEL_RGBA: [u8; 4] = [7, 10, 13, 210];
const LINE_RGB: [u8; 3] = [232, 238, 226];
const LINE_RGBA: [u8; 4] = [232, 238, 226, 255];
const EAST_RGB: [u8; 3] = [122, 196, 238];
const EAST_RGBA: [u8; 4] = [122, 196, 238, 255];
const SOUTH_WEST_RGB: [u8; 3] = [182, 193, 184];
const SOUTH_WEST_RGBA: [u8; 4] = [182, 193, 184, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompassLayout {
    x: u32,
    y: u32,
    panel: u32,
    center_x: i32,
    center_y: i32,
    arm: i32,
    scale: u32,
}

impl CompassLayout {
    fn new(width: u32, height: u32) -> Option<Self> {
        let min_axis = width.min(height);
        if min_axis < 96 {
            return None;
        }

        let arm = (min_axis / 22).clamp(18, 72) as i32;
        let label_pad = (arm / 2).max(10) as u32;
        let panel = (arm as u32)
            .saturating_mul(2)
            .saturating_add(label_pad)
            .clamp(52, min_axis.saturating_sub(8));
        let margin = (min_axis / 80).clamp(6, 24);
        let x = width.saturating_sub(panel + margin);
        let y = margin;
        let center_x = x as i32 + panel as i32 / 2;
        let center_y = y as i32 + panel as i32 / 2;
        let arm = ((panel as i32 - label_pad as i32) / 2).max(14);
        let scale = (arm as u32 / 16).clamp(1, 4);

        Some(Self {
            x,
            y,
            panel,
            center_x,
            center_y,
            arm,
            scale,
        })
    }
}

pub fn draw_compass_rgb(image: &mut RgbImage) {
    let Some(layout) = CompassLayout::new(image.width(), image.height()) else {
        return;
    };

    fill_panel_rgb(image, layout.x, layout.y, layout.panel, layout.panel);
    draw_line_rgb(
        image,
        layout.center_x,
        layout.center_y + layout.arm,
        layout.center_x,
        layout.center_y - layout.arm,
        LINE_RGB,
        0.92,
        2,
    );
    draw_line_rgb(
        image,
        layout.center_x - layout.arm,
        layout.center_y,
        layout.center_x + layout.arm,
        layout.center_y,
        EAST_RGB,
        0.92,
        2,
    );
    draw_arrow_head_rgb(
        image,
        layout.center_x,
        layout.center_y - layout.arm,
        0,
        -1,
        LINE_RGB,
    );
    draw_arrow_head_rgb(
        image,
        layout.center_x + layout.arm,
        layout.center_y,
        1,
        0,
        EAST_RGB,
    );
    draw_text_rgb(
        image,
        label_x(layout.center_x, layout.scale),
        (layout.center_y - layout.arm - 8 * layout.scale as i32).max(0) as u32,
        "N",
        LINE_RGB,
        layout.scale,
    );
    draw_text_rgb(
        image,
        label_x(layout.center_x, layout.scale),
        (layout.center_y + layout.arm + 4).max(0) as u32,
        "S",
        SOUTH_WEST_RGB,
        layout.scale,
    );
    draw_text_rgb(
        image,
        (layout.center_x + layout.arm + 5).max(0) as u32,
        label_y(layout.center_y, layout.scale),
        "E",
        EAST_RGB,
        layout.scale,
    );
    draw_text_rgb(
        image,
        (layout.center_x - layout.arm - (8 * layout.scale as i32)).max(0) as u32,
        label_y(layout.center_y, layout.scale),
        "W",
        SOUTH_WEST_RGB,
        layout.scale,
    );
}

pub fn draw_compass_rgba(image: &mut RgbaImage) {
    let Some(layout) = CompassLayout::new(image.width(), image.height()) else {
        return;
    };

    fill_panel_rgba(image, layout.x, layout.y, layout.panel, layout.panel);
    draw_line_rgba(
        image,
        layout.center_x,
        layout.center_y + layout.arm,
        layout.center_x,
        layout.center_y - layout.arm,
        LINE_RGBA,
        0.92,
        2,
    );
    draw_line_rgba(
        image,
        layout.center_x - layout.arm,
        layout.center_y,
        layout.center_x + layout.arm,
        layout.center_y,
        EAST_RGBA,
        0.92,
        2,
    );
    draw_arrow_head_rgba(
        image,
        layout.center_x,
        layout.center_y - layout.arm,
        0,
        -1,
        LINE_RGBA,
    );
    draw_arrow_head_rgba(
        image,
        layout.center_x + layout.arm,
        layout.center_y,
        1,
        0,
        EAST_RGBA,
    );
    draw_text_rgba(
        image,
        label_x(layout.center_x, layout.scale),
        (layout.center_y - layout.arm - 8 * layout.scale as i32).max(0) as u32,
        "N",
        LINE_RGBA,
        layout.scale,
    );
    draw_text_rgba(
        image,
        label_x(layout.center_x, layout.scale),
        (layout.center_y + layout.arm + 4).max(0) as u32,
        "S",
        SOUTH_WEST_RGBA,
        layout.scale,
    );
    draw_text_rgba(
        image,
        (layout.center_x + layout.arm + 5).max(0) as u32,
        label_y(layout.center_y, layout.scale),
        "E",
        EAST_RGBA,
        layout.scale,
    );
    draw_text_rgba(
        image,
        (layout.center_x - layout.arm - (8 * layout.scale as i32)).max(0) as u32,
        label_y(layout.center_y, layout.scale),
        "W",
        SOUTH_WEST_RGBA,
        layout.scale,
    );
}

#[allow(dead_code)]
pub fn draw_compass_offscreen(image: &mut OffscreenRenderOutput) {
    let Some(mut rgba) =
        RgbaImage::from_raw(image.width, image.height, std::mem::take(&mut image.rgba))
    else {
        return;
    };
    draw_compass_rgba(&mut rgba);
    image.rgba = rgba.into_raw();
}

fn label_x(center_x: i32, scale: u32) -> u32 {
    (center_x - (3 * scale as i32) / 2).max(0) as u32
}

fn label_y(center_y: i32, scale: u32) -> u32 {
    (center_y - (5 * scale as i32) / 2).max(0) as u32
}

fn draw_arrow_head_rgb(
    image: &mut RgbImage,
    tip_x: i32,
    tip_y: i32,
    dir_x: i32,
    dir_y: i32,
    color: [u8; 3],
) {
    let len = (image.width().min(image.height()) as i32 / 70).clamp(6, 18);
    let wing = (len / 2).max(3);
    let base_x = tip_x - dir_x * len;
    let base_y = tip_y - dir_y * len;
    let perp_x = -dir_y;
    let perp_y = dir_x;
    draw_line_rgb(
        image,
        tip_x,
        tip_y,
        base_x + perp_x * wing,
        base_y + perp_y * wing,
        color,
        0.94,
        2,
    );
    draw_line_rgb(
        image,
        tip_x,
        tip_y,
        base_x - perp_x * wing,
        base_y - perp_y * wing,
        color,
        0.94,
        2,
    );
}

fn draw_arrow_head_rgba(
    image: &mut RgbaImage,
    tip_x: i32,
    tip_y: i32,
    dir_x: i32,
    dir_y: i32,
    color: [u8; 4],
) {
    let len = (image.width().min(image.height()) as i32 / 70).clamp(6, 18);
    let wing = (len / 2).max(3);
    let base_x = tip_x - dir_x * len;
    let base_y = tip_y - dir_y * len;
    let perp_x = -dir_y;
    let perp_y = dir_x;
    draw_line_rgba(
        image,
        tip_x,
        tip_y,
        base_x + perp_x * wing,
        base_y + perp_y * wing,
        color,
        0.94,
        2,
    );
    draw_line_rgba(
        image,
        tip_x,
        tip_y,
        base_x - perp_x * wing,
        base_y - perp_y * wing,
        color,
        0.94,
        2,
    );
}

fn fill_panel_rgb(image: &mut RgbImage, x: u32, y: u32, w: u32, h: u32) {
    for py in y..(y + h).min(image.height()) {
        for px in x..(x + w).min(image.width()) {
            blend_rgb(image, px, py, PANEL_RGB, 0.58);
        }
    }
}

fn fill_panel_rgba(image: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    for py in y..(y + h).min(image.height()) {
        for px in x..(x + w).min(image.width()) {
            blend_rgba(image, px, py, PANEL_RGBA, 0.58);
        }
    }
}

fn draw_line_rgb(
    image: &mut RgbImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 3],
    amount: f32,
    thickness: i32,
) {
    for (x, y) in line_points(x0, y0, x1, y1) {
        for oy in -thickness / 2..=thickness / 2 {
            for ox in -thickness / 2..=thickness / 2 {
                blend_rgb_i32(image, x + ox, y + oy, color, amount);
            }
        }
    }
}

fn draw_line_rgba(
    image: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 4],
    amount: f32,
    thickness: i32,
) {
    for (x, y) in line_points(x0, y0, x1, y1) {
        for oy in -thickness / 2..=thickness / 2 {
            for ox in -thickness / 2..=thickness / 2 {
                blend_rgba_i32(image, x + ox, y + oy, color, amount);
            }
        }
    }
}

fn line_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    let mut points = Vec::with_capacity((dx.max(-dy) + 1).max(1) as usize);

    loop {
        points.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }

    points
}

fn draw_text_rgb(image: &mut RgbImage, x: u32, y: u32, text: &str, color: [u8; 3], scale: u32) {
    let mut cursor = x;
    for ch in text.chars() {
        let glyph = glyph_3x5(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3 {
                if (bits >> (2 - col)) & 1 == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        blend_rgb(
                            image,
                            cursor + col * scale + sx,
                            y + row as u32 * scale + sy,
                            color,
                            0.96,
                        );
                    }
                }
            }
        }
        cursor += 4 * scale;
    }
}

fn draw_text_rgba(image: &mut RgbaImage, x: u32, y: u32, text: &str, color: [u8; 4], scale: u32) {
    let mut cursor = x;
    for ch in text.chars() {
        let glyph = glyph_3x5(ch);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3 {
                if (bits >> (2 - col)) & 1 == 0 {
                    continue;
                }
                for sy in 0..scale {
                    for sx in 0..scale {
                        blend_rgba(
                            image,
                            cursor + col * scale + sx,
                            y + row as u32 * scale + sy,
                            color,
                            0.96,
                        );
                    }
                }
            }
        }
        cursor += 4 * scale;
    }
}

fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch {
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        _ => [0, 0, 0, 0, 0],
    }
}

fn blend_rgb_i32(image: &mut RgbImage, x: i32, y: i32, color: [u8; 3], amount: f32) {
    if x < 0 || y < 0 {
        return;
    }
    blend_rgb(image, x as u32, y as u32, color, amount);
}

fn blend_rgba_i32(image: &mut RgbaImage, x: i32, y: i32, color: [u8; 4], amount: f32) {
    if x < 0 || y < 0 {
        return;
    }
    blend_rgba(image, x as u32, y as u32, color, amount);
}

fn blend_rgb(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let amount = amount.clamp(0.0, 1.0);
    let base = image.get_pixel(x, y).0;
    image.put_pixel(
        x,
        y,
        image::Rgb([
            mix(base[0], color[0], amount),
            mix(base[1], color[1], amount),
            mix(base[2], color[2], amount),
        ]),
    );
}

fn blend_rgba(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4], amount: f32) {
    if x >= image.width() || y >= image.height() {
        return;
    }
    let amount = (amount * (color[3] as f32 / 255.0)).clamp(0.0, 1.0);
    let base = image.get_pixel(x, y).0;
    image.put_pixel(
        x,
        y,
        image::Rgba([
            mix(base[0], color[0], amount),
            mix(base[1], color[1], amount),
            mix(base[2], color[2], amount),
            255,
        ]),
    );
}

fn mix(base: u8, tint: u8, amount: f32) -> u8 {
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_overlay_changes_rgb_pixels() {
        let mut image = RgbImage::from_pixel(320, 180, image::Rgb([12, 18, 24]));
        let before = image.clone();
        draw_compass_rgb(&mut image);
        assert_ne!(image.as_raw(), before.as_raw());
    }

    #[test]
    fn compass_overlay_changes_rgba_pixels() {
        let mut image = RgbaImage::from_pixel(320, 180, image::Rgba([12, 18, 24, 255]));
        let before = image.clone();
        draw_compass_rgba(&mut image);
        assert_ne!(image.as_raw(), before.as_raw());
    }
}
