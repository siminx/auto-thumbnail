//! JPEG XL 解码（jxl-oxide）

use std::io::Cursor;
use std::path::Path;

use image::DynamicImage;
use jxl_oxide::integration::JxlDecoder;

/// 解码 .jxl 文件
pub fn try_decode_jxl(path: &Path) -> Option<DynamicImage> {
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jxl"))
    {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    let decoder = JxlDecoder::new(Cursor::new(data)).ok()?;
    DynamicImage::from_decoder(decoder).ok()
}
