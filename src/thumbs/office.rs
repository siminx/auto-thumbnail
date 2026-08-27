//! Office Open XML 内嵌缩略图：读 ZIP 内 docProps/thumbnail.*。

use std::io::{Cursor, Read};
use std::path::Path;

use image::DynamicImage;
use zip::ZipArchive;

use crate::frame_valid::is_rgba_blank;

/// ZIP 内常见的内嵌缩略图路径
const EMBEDDED_IMAGE_PATHS: &[&str] = &[
    "docProps/thumbnail.jpeg",
    "docProps/thumbnail.png",
    "Thumbnails/thumbnail.png",
];

const EMBEDDED_EMF_PATH: &str = "docProps/thumbnail.emf";

/// 读取 ZIP 内 EMF 缩略图原始字节（供平台层 GDI 栅格化）
pub fn extract_emf_bytes(path: &Path) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).ok()?;
    let mut file = archive.by_name(EMBEDDED_EMF_PATH).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// 从 Office 包内提取预生成缩略图（跳过空白占位图）
pub fn create_thumbnail(path: &Path, max_dim: u32) -> Option<DynamicImage> {
    let data = std::fs::read(path).ok()?;
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).ok()?;

    for thumb_path in EMBEDDED_IMAGE_PATHS {
        if let Some(img) = read_zip_image(&mut archive, thumb_path, max_dim) {
            return Some(img);
        }
    }

    None
}

fn read_zip_image(
    archive: &mut ZipArchive<Cursor<Vec<u8>>>,
    name: &str,
    max_dim: u32,
) -> Option<DynamicImage> {
    let mut file = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    let img = image::load_from_memory(&buf).ok()?;
    let thumb = DynamicImage::from(img).thumbnail(max_dim, max_dim);
    let rgba = thumb.to_rgba8();
    if is_rgba_blank(&rgba) {
        return None;
    }
    Some(thumb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// sample.docx 内嵌 JPEG 为空白占位图，应跳过
    #[test]
    fn skips_blank_embedded_thumbnail() {
        let path = Path::new(r"D:\xsmspace\data\2\_clone_sample-files\documents\sample.docx");
        if !path.exists() {
            return;
        }
        assert!(create_thumbnail(path, 512).is_none());
    }
}
