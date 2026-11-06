//! Instance badges: the app icon with a coloured dot, so several pinned copies of the same app
//! can be told apart on the taskbar or dock.

use std::path::Path;

use image::{imageops, Rgba, RgbaImage};

pub fn parse_hex(color: &str) -> [u8; 3] {
    let h = color.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x0A84FF);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

fn blend(dst: &mut Rgba<u8>, src: [u8; 3], a: f32) {
    let a = a.clamp(0.0, 1.0);
    let da = dst[3] as f32 / 255.0;
    let out_a = a + da * (1.0 - a);
    for i in 0..3 {
        let c = (src[i] as f32 * a + dst[i] as f32 * da * (1.0 - a)) / out_a.max(1e-6);
        dst[i] = c.round() as u8;
    }
    dst[3] = (out_a * 255.0).round() as u8;
}

/// Anti-aliased filled circle with a white ring.
fn dot(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: [u8; 3]) {
    let ring = (r * 0.22).max(1.0);
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let outer = (r + ring - d + 0.5).clamp(0.0, 1.0);
            if outer <= 0.0 {
                continue;
            }
            let p = img.get_pixel_mut(x, y);
            blend(p, [255, 255, 255], outer);
            let inner = (r - d + 0.5).clamp(0.0, 1.0);
            if inner > 0.0 {
                blend(p, color, inner);
            }
        }
    }
}

/// Rounded square in the instance colour, used when the app icon cannot be read.
pub fn plain_tile(size: u32, color: [u8; 3]) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let r = size as f32 * 0.22;
    let s = size as f32;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let dx = (r - fx).max(fx - (s - r)).max(0.0);
            let dy = (r - fy).max(fy - (s - r)).max(0.0);
            let a = (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            if a > 0.0 {
                img.put_pixel(x, y, Rgba([color[0], color[1], color[2], (a * 255.0) as u8]));
            }
        }
    }
    img
}

/// `base` (any size) scaled to `size` with a badge dot in the bottom-right corner.
pub fn badged(base: &RgbaImage, size: u32, color: [u8; 3]) -> RgbaImage {
    let mut img = imageops::resize(base, size, size, imageops::FilterType::Lanczos3);
    let s = size as f32;
    let r = s * 0.2;
    dot(&mut img, s - r - s * 0.06, s - r - s * 0.06, r, color);
    img
}

/// Writes a multi-resolution `.ico` (16–256 px) of the badged icon.
pub fn write_ico(base: &RgbaImage, color: [u8; 3], out: &Path) -> Result<(), String> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    let frames: Result<Vec<IcoFrame>, _> = [16u32, 24, 32, 48, 64, 256]
        .iter()
        .map(|s| {
            let img = badged(base, *s, color);
            IcoFrame::as_png(img.as_raw(), *s, *s, image::ExtendedColorType::Rgba8)
        })
        .collect();
    let frames = frames.map_err(|e| e.to_string())?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    IcoEncoder::new(file).encode_images(&frames).map_err(|e| e.to_string())
}

pub fn write_png(base: &RgbaImage, color: [u8; 3], size: u32, out: &Path) -> Result<(), String> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    badged(base, size, color).save(out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_paints_corner() {
        let base = plain_tile(64, [10, 10, 10]);
        let b = badged(&base, 64, [255, 0, 0]);
        let p = b.get_pixel(50, 50);
        assert!(p[0] > 200 && p[1] < 80, "corner is the badge colour: {p:?}");
        assert_eq!(parse_hex("#0A84FF"), [0x0A, 0x84, 0xFF]);
    }
}
