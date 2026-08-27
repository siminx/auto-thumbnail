//! ICO/CUR 宽松解析：绕过 image crate 对 hotspot / 非 RGBA PNG 的严格校验。

use std::{
    fs::File,
    io::Read,
    path::Path,
};

use image::{DynamicImage, RgbaImage};

/// 宽松 ICO 解码，取最大可用条目
pub fn try_decode_ico(path: &Path) -> Option<DynamicImage> {
    if !is_ico_file(path) {
        return None;
    }

    let file = File::open(path).ok()?;
    let icon_dir = ico::IconDir::read(file).ok()?;

    let mut indices: Vec<usize> = (0..icon_dir.entries().len()).collect();
    indices.sort_by_key(|&i| {
        let e = &icon_dir.entries()[i];
        u32::from(e.width()) * u32::from(e.height())
    });
    indices.reverse();

    for &i in &indices {
        let entry = &icon_dir.entries()[i];
        if let Ok(icon_image) = entry.decode() {
            if let Some(img) = icon_image_to_dynamic(&icon_image) {
                return Some(img);
            }
        }
    }

    try_ico_png_fallback(path)
}

fn try_ico_png_fallback(path: &Path) -> Option<DynamicImage> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    let dir_end = 6 + count * 16;
    if data.len() < dir_end {
        return None;
    }

    let mut entries: Vec<(u32, u32, u32)> = Vec::with_capacity(count);
    for i in 0..count {
        let base = 6 + i * 16;
        let w = if data[base] == 0 {
            256u32
        } else {
            u32::from(data[base])
        };
        let h = if data[base + 1] == 0 {
            256u32
        } else {
            u32::from(data[base + 1])
        };
        let size = u32::from_le_bytes([
            data[base + 8],
            data[base + 9],
            data[base + 10],
            data[base + 11],
        ]);
        let offset = u32::from_le_bytes([
            data[base + 12],
            data[base + 13],
            data[base + 14],
            data[base + 15],
        ]);
        entries.push((w * h, offset, size));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0));

    for &(_, offset, size) in &entries {
        let start = offset as usize;
        let end = start.checked_add(size as usize)?;
        if end > data.len() {
            continue;
        }
        let chunk = &data[start..end];
        if !chunk.starts_with(b"\x89PNG\r\n\x1a\n") {
            continue;
        }
        if let Ok(img) = image::load_from_memory(chunk) {
            return Some(img);
        }
    }
    None
}

fn icon_image_to_dynamic(img: &ico::IconImage) -> Option<DynamicImage> {
    RgbaImage::from_raw(
        u32::from(img.width()),
        u32::from(img.height()),
        img.rgba_data().to_vec(),
    )
    .map(DynamicImage::ImageRgba8)
}

fn is_ico_file(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "ico" | "cur"))
    {
        return true;
    }
    let mut buf = [0u8; 4];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let Ok(n) = file.read(&mut buf) else {
        return false;
    };
    n >= 4 && &buf[..4] == b"\x00\x00\x01\x00"
}
