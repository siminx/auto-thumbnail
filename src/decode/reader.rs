//! ImageReader magic 嗅探解码；HDR 含 `#?RGBE` 非严格路径。

use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use image::{
    codecs::hdr::HdrDecoder, DynamicImage, ImageFormat, ImageReader,
};

use super::tone_map::apply_tone_map_if_needed;

/// magic 嗅探 + 扩展名显式格式 + 解码；HDR/EXR 自动色调映射
pub fn try_decode_reader(path: &Path) -> Option<DynamicImage> {
    if let Some(img) = try_decode_hdr(path) {
        return Some(img);
    }
    if let Some(img) = decode_with_guessed(path) {
        return Some(img);
    }
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pcx"))
    {
        if let Ok(img) = image::open(path) {
            return Some(apply_tone_map_if_needed(img));
        }
    }
    if let Some(fmt) = format_from_extension(path) {
        if let Some(img) = decode_with_format(path, fmt) {
            return Some(img);
        }
    }
    None
}

/// `#?RGBE` 等 image crate 严格模式不接受的 Radiance HDR
fn try_decode_hdr(path: &Path) -> Option<DynamicImage> {
    let magic = read_magic_prefix(path, 10)?;
    let is_hdr = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("hdr"))
        || magic.starts_with(b"#?RGBE")
        || magic.starts_with(b"#?RADIANCE");
    if !is_hdr {
        return None;
    }

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let decoder = if magic.starts_with(b"#?RADIANCE") {
        HdrDecoder::new(reader).ok()?
    } else {
        HdrDecoder::new_nonstrict(reader).ok()?
    };
    let img = DynamicImage::from_decoder(decoder).ok()?;
    Some(apply_tone_map_if_needed(img))
}

fn read_magic_prefix(path: &Path, len: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; len];
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);
    Some(buf)
}

fn decode_with_guessed(path: &Path) -> Option<DynamicImage> {
    let img = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    Some(apply_tone_map_if_needed(img))
}

fn decode_with_format(path: &Path, format: ImageFormat) -> Option<DynamicImage> {
    let mut reader = ImageReader::open(path).ok()?;
    reader.set_format(format);
    let img = reader.decode().ok()?;
    Some(apply_tone_map_if_needed(img))
}

fn format_from_extension(path: &Path) -> Option<ImageFormat> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "avif" => Some(ImageFormat::Avif),
        "hdr" => Some(ImageFormat::Hdr),
        "exr" => Some(ImageFormat::OpenExr),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::WebP),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        "tga" => Some(ImageFormat::Tga),
        _ => None,
    }
}
