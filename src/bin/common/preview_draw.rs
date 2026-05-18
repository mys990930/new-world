#![allow(dead_code)]

use image::{RgbImage, RgbaImage};

pub fn blend_pixel(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3], amount: f32) {
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

pub fn blend_pixel_i32(image: &mut RgbImage, x: i32, y: i32, color: [u8; 3], amount: f32) {
    if x < 0 || y < 0 {
        return;
    }
    blend_pixel(image, x as u32, y as u32, color, amount);
}

pub fn blend_rgba_pixel(image: &mut RgbaImage, x: u32, y: u32, color: [u8; 4], amount: f32) {
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

pub fn blend_rgba_pixel_i32(image: &mut RgbaImage, x: i32, y: i32, color: [u8; 4], amount: f32) {
    if x < 0 || y < 0 {
        return;
    }
    blend_rgba_pixel(image, x as u32, y as u32, color, amount);
}

pub fn set_pixel(image: &mut RgbImage, x: u32, y: u32, color: [u8; 3]) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, image::Rgb(color));
    }
}

pub fn blend_rect(
    image: &mut RgbImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 3],
    amount: f32,
) {
    let max_x = (x + width).min(image.width());
    let max_y = (y + height).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            blend_pixel(image, px, py, color, amount);
        }
    }
}

pub fn draw_scale_bar_line(
    image: &mut RgbImage,
    x: u32,
    y: u32,
    width: u32,
    color: [u8; 3],
    amount: f32,
) {
    for px in x..=(x + width).min(image.width().saturating_sub(1)) {
        blend_pixel(image, px, y, color, amount);
        blend_pixel(image, px, y + 1, color, amount);
    }
    for tick_x in [x, (x + width).min(image.width().saturating_sub(1))] {
        for py in y.saturating_sub(3)..=(y + 4).min(image.height().saturating_sub(1)) {
            blend_pixel(image, tick_x, py, color, amount);
            if tick_x + 1 < image.width() {
                blend_pixel(image, tick_x + 1, py, color, amount);
            }
        }
    }
}

pub fn draw_pixel_line(
    image: &mut RgbImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 3],
    amount: f32,
) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        blend_pixel_i32(image, x, y, color, amount);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x += sx;
        }
        if e2 <= dx {
            error += dx;
            y += sy;
        }
    }
}

pub fn draw_pixel_line_with_radius(
    image: &mut RgbImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 3],
    amount: f32,
    radius: i32,
) {
    if radius <= 0 {
        draw_pixel_line(image, x0, y0, x1, y1, color, amount);
        return;
    }

    let radius_sq = radius * radius;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy > radius_sq {
                continue;
            }
            let offset_amount = if dx == 0 && dy == 0 {
                amount
            } else {
                amount * 0.62
            };
            draw_pixel_line(
                image,
                x0 + dx,
                y0 + dy,
                x1 + dx,
                y1 + dy,
                color,
                offset_amount,
            );
        }
    }
}

pub fn draw_disc(
    image: &mut RgbImage,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: [u8; 3],
    amount: f32,
) {
    let radius_sq = radius * radius;
    for y in center_y - radius..=center_y + radius {
        for x in center_x - radius..=center_x + radius {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= radius_sq {
                blend_pixel_i32(image, x, y, color, amount);
            }
        }
    }
}

pub fn draw_circle_ring(
    image: &mut RgbImage,
    center_x: i32,
    center_y: i32,
    radius: i32,
    ring_width: i32,
    color: [u8; 3],
    amount: f32,
) {
    let outer = radius.max(1);
    let inner = (outer - ring_width.max(1)).max(0);
    let outer_sq = outer * outer;
    let inner_sq = inner * inner;
    for y in center_y - outer..=center_y + outer {
        for x in center_x - outer..=center_x + outer {
            let dx = x - center_x;
            let dy = y - center_y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= outer_sq && dist_sq >= inner_sq {
                blend_pixel_i32(image, x, y, color, amount);
            }
        }
    }
}

pub fn draw_text(image: &mut RgbImage, x: u32, y: u32, text: &str, color: [u8; 3], scale: u32) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_char(image, cursor_x, y, ch, color, scale);
        cursor_x = cursor_x.saturating_add(4 * scale);
    }
}

pub fn draw_rgba_text(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    text: &str,
    color: [u8; 4],
    scale: u32,
) {
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_rgba_char(image, cursor_x, y, ch, color, scale);
        cursor_x = cursor_x.saturating_add(4 * scale);
    }
}

pub fn text_width(text: &str, scale: u32) -> u32 {
    text.chars().count() as u32 * 4 * scale
}

fn draw_char(image: &mut RgbImage, x: u32, y: u32, ch: char, color: [u8; 3], scale: u32) {
    let glyph = glyph_3x5(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..3 {
            if (bits >> (2 - col)) & 1 == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    blend_pixel(
                        image,
                        x + col * scale + sx,
                        y + row as u32 * scale + sy,
                        color,
                        0.95,
                    );
                }
            }
        }
    }
}

fn draw_rgba_char(image: &mut RgbaImage, x: u32, y: u32, ch: char, color: [u8; 4], scale: u32) {
    let glyph = glyph_3x5(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..3 {
            if (bits >> (2 - col)) & 1 == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    blend_rgba_pixel(
                        image,
                        x + col * scale + sx,
                        y + row as u32 * scale + sy,
                        color,
                        0.95,
                    );
                }
            }
        }
    }
}

pub fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
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
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        _ => [0b000, 0b000, 0b000, 0b000, 0b000],
    }
}

fn mix(base: u8, tint: u8, amount: f32) -> u8 {
    ((base as f32 * (1.0 - amount)) + (tint as f32 * amount))
        .round()
        .clamp(0.0, 255.0) as u8
}
