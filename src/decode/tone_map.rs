//! HDR/EXR 等高动态范围图像的 Reinhard 色调映射，供主色与缩略图复用。

use image::{DynamicImage, Rgb, RgbImage};

/// 将 f32 HDR/EXR 压缩到 8-bit LDR；普通 8-bit 图像原样返回
pub fn apply_tone_map_if_needed(img: DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgb32F(f32_img) => DynamicImage::ImageRgb8(reinhard_rgb32f(&f32_img)),
        other => other,
    }
}

/// Reinhard 全局色调映射：L_out = L / (1 + L)
fn reinhard_rgb32f(img: &image::Rgb32FImage) -> RgbImage {
    let (w, h) = img.dimensions();
    RgbImage::from_fn(w, h, |x, y| {
        let px = img.get_pixel(x, y);
        Rgb([
            tone_map_channel(px[0]),
            tone_map_channel(px[1]),
            tone_map_channel(px[2]),
        ])
    })
}

fn tone_map_channel(v: f32) -> u8 {
    let l = v.max(0.0);
    let mapped = l / (1.0 + l);
    (mapped * 255.0).clamp(0.0, 255.0) as u8
}
