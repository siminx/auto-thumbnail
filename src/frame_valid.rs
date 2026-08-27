//! 帧/图像有效性检测：过滤全黑/全白及低方差无效帧。

use image::{DynamicImage, RgbaImage};

/// TV 级黑场像素上限（Y=16 对应 RGB 约 16）
const TV_BLACK_LEVEL: u8 = 16;
/// 暗场像素上限（max(R,G,B)）
const DARK_LEVEL: u8 = 32;
/// 近黑/近暗像素占比阈值
const DARK_RATIO_PERCENT: u32 = 90;

/// 采样判断 RGBA 图像是否实质空白（跳过全透明像素）
pub fn is_rgba_blank(img: &RgbaImage) -> bool {
    if img.width() == 0 || img.height() == 0 {
        return true;
    }

    let step = (img.width().max(img.height()) / 32).max(1) as usize;
    let mut total = 0u32;
    let mut near_white = 0u32;
    let mut near_black = 0u32;
    let mut near_dark = 0u32;
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;

    let mut y = 0u32;
    while y < img.height() {
        let mut x = 0u32;
        while x < img.width() {
            let pixel = img.get_pixel(x, y);
            if pixel[3] == 0 {
                x += step as u32;
                continue;
            }
            total += 1;
            sum_r += pixel[0] as u64;
            sum_g += pixel[1] as u64;
            sum_b += pixel[2] as u64;
            if pixel[0] >= 250 && pixel[1] >= 250 && pixel[2] >= 250 {
                near_white += 1;
            }
            if pixel[0] <= TV_BLACK_LEVEL
                && pixel[1] <= TV_BLACK_LEVEL
                && pixel[2] <= TV_BLACK_LEVEL
            {
                near_black += 1;
            }
            if pixel[0].max(pixel[1]).max(pixel[2]) <= DARK_LEVEL {
                near_dark += 1;
            }
            x += step as u32;
        }
        y += step as u32;
    }

    if total == 0 {
        return true;
    }

    if near_white * 100 / total > 98
        || near_black * 100 / total > DARK_RATIO_PERCENT
        || near_dark * 100 / total > DARK_RATIO_PERCENT
    {
        return true;
    }

    let n = total as f64;
    let mean_r = sum_r as f64 / n;
    let mean_g = sum_g as f64 / n;
    let mean_b = sum_b as f64 / n;

    let mut variance = 0.0f64;
    let mut y = 0u32;
    while y < img.height() {
        let mut x = 0u32;
        while x < img.width() {
            let pixel = img.get_pixel(x, y);
            if pixel[3] != 0 {
                let dr = pixel[0] as f64 - mean_r;
                let dg = pixel[1] as f64 - mean_g;
                let db = pixel[2] as f64 - mean_b;
                variance += dr * dr + dg * dg + db * db;
            }
            x += step as u32;
        }
        y += step as u32;
    }

    let mean = (mean_r + mean_g + mean_b) / 3.0;
    variance / (n * 3.0) < 4.0 && (mean > 200.0 || mean < 35.0)
}

/// 采样判断帧是否实质空白（全黑、全白或近零方差纯色），不适合作为缩略图
pub fn is_effectively_blank(img: &DynamicImage) -> bool {
    is_rgba_blank(&img.to_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb, Rgba, RgbaImage};

    #[test]
    fn detects_white_rgba_as_blank() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        assert!(is_rgba_blank(&img));
    }

    #[test]
    fn detects_black_rgba_as_blank() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        assert!(is_rgba_blank(&img));
    }

    #[test]
    fn detects_tv_black_rgba_as_blank() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([16, 16, 16, 255]));
        assert!(is_rgba_blank(&img));
    }

    #[test]
    fn detects_dark_uniform_rgba_as_blank() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([30, 30, 30, 255]));
        assert!(is_rgba_blank(&img));
    }

    #[test]
    fn detects_colored_rgba_as_not_blank() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([120, 80, 40, 255]));
        assert!(!is_rgba_blank(&img));
    }

    #[test]
    fn detects_scene_rgba_as_not_blank() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([80, 60, 40, 255]));
        assert!(!is_rgba_blank(&img));
    }

    #[test]
    fn detects_white_dynamic_as_blank() {
        let rgb = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(100, 100, Rgb([255, 255, 255]));
        assert!(is_effectively_blank(&DynamicImage::ImageRgb8(rgb)));
    }

    #[test]
    fn detects_colored_dynamic_as_not_blank() {
        let rgb = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_pixel(100, 100, Rgb([120, 80, 40]));
        assert!(!is_effectively_blank(&DynamicImage::ImageRgb8(rgb)));
    }
}
