//! PSD/PSB Image Data Section（composite image）解码。
//!
//! 读取 Maximize Compatibility 保存的完整拼合图，可得到远超 160px IRB 的预览尺寸。

use std::path::Path;

use ag_psd::{get_composite_image_data, read_psd};
use ag_psd::psd::ReadOptions;
use image::{DynamicImage, RgbaImage};

/// composite 解码内存上限
const COMPOSITE_MEMORY_LIMIT: usize = 256 * 1024 * 1024;

/// 超过此尺寸或文件过大时跳过 composite，避免 OOM
const MAX_COMPOSITE_DIMENSION: u32 = 12_000;

const MAX_COMPOSITE_FILE_BYTES: u64 = 500 * 1024 * 1024;

/// 从 PSD/PSB 读取 composite image 并缩放到 max_dim 以内
pub fn extract_psd_composite(path: &Path, max_dim: u32) -> Option<DynamicImage> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_COMPOSITE_FILE_BYTES {
        log::debug!(
            "PSD composite 跳过：文件过大 {} bytes: {:?}",
            meta.len(),
            path
        );
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    let opts = ReadOptions {
        skip_layer_image_data: Some(true),
        skip_thumbnail: Some(true),
        total_memory_limit: Some(COMPOSITE_MEMORY_LIMIT),
        ..Default::default()
    };

    let psd = read_psd(&bytes, &opts).ok()?;
    let doc_w = psd.width as u32;
    let doc_h = psd.height as u32;
    if doc_w == 0 || doc_h == 0 {
        return None;
    }
    if doc_w > MAX_COMPOSITE_DIMENSION || doc_h > MAX_COMPOSITE_DIMENSION {
        log::debug!(
            "PSD composite 跳过：文档尺寸过大 {}x{}: {:?}",
            doc_w,
            doc_h,
            path
        );
        return None;
    }

    // 常规读取路径：composite 已在 canvas/image_data
    if let Some(canvas) = &psd.canvas {
        return pixel_data_to_image(canvas).map(|img| scale_to_max_dim(img, max_dim));
    }
    if let Some(image_data) = &psd.image_data {
        return pixel_data_to_image(image_data).map(|img| scale_to_max_dim(img, max_dim));
    }

    // use_raw_data 延迟解码路径
    let pixels = get_composite_image_data(&psd).ok()??;
    pixel_data_to_image(&pixels).map(|img| scale_to_max_dim(img, max_dim))
}

/// PixelData(RGBA8) 转 DynamicImage
fn pixel_data_to_image(pixels: &ag_psd::psd::PixelData) -> Option<DynamicImage> {
    let expected = (pixels.width as usize)
        .checked_mul(pixels.height as usize)?
        .checked_mul(4)?;
    if pixels.data.len() < expected {
        return None;
    }
    RgbaImage::from_raw(pixels.width, pixels.height, pixels.data.clone())
        .map(DynamicImage::ImageRgba8)
}

fn scale_to_max_dim(img: DynamicImage, max_dim: u32) -> DynamicImage {
    if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img
    }
}
