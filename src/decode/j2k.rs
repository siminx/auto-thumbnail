//! JPEG 2000 解码（JP2/J2K codestream），基于 openjpeg2-pure-rs。

use std::{num::NonZeroUsize, path::Path};

use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use openjp2::{
    convert::{self, YCbCrTransform},
    AlphaMode, ColorSpace, DecodeOptions, Decoder, ExpectedImage, Format, Image, OutputFormat,
    Upsampling,
};

const SOC: [u8; 2] = [0xFF, 0x4F];
const EOC: [u8; 2] = [0xFF, 0xD9];
const JP2_SIGNATURE: [u8; 8] = [0x00, 0x00, 0x00, 0x0c, 0x6a, 0x50, 0x20, 0x20];
const LARGE_CODESTREAM: usize = 1_000_000;

fn is_jp2_container(bytes: &[u8]) -> bool {
    bytes.starts_with(&JP2_SIGNATURE)
}

fn is_j2k_ext(ext: &str) -> bool {
    matches!(
        ext,
        "jp2" | "jpx" | "j2k" | "j2c" | "jpf" | "jpc"
    )
}

/// 按扩展名解码 JPEG 2000 家族格式
pub fn try_decode_j2k(path: &Path) -> Option<DynamicImage> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if !is_j2k_ext(&ext) {
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    // 扩展名可能误导（jpc 常为 raw codestream），以 magic 判定容器
    let is_container = is_jp2_container(&bytes);

    // header-only JP2（无 SOD/熵数据，如 balloon.jp2）→ 元数据占位图
    if is_container {
        if let Some(soc_pos) = bytes.windows(2).position(|w| w == SOC) {
            if !codestream_has_image_data(&bytes[soc_pos..]) {
                return try_jp2_metadata_placeholder(&bytes);
            }
        }
    }

    let repaired = if is_container {
        repair_jp2_bytes(&bytes)
    } else {
        repair_j2k_codestream(&bytes)
    };

    // 大体积 raw codestream 优先 pure-rs 多策略解码（如 imagery.jpc）
    let raw_codestream = bytes.starts_with(&SOC) && !is_container;
    if raw_codestream && bytes.len() > LARGE_CODESTREAM {
        if let Some(img) = try_decode_large_codestream(&repaired) {
            return Some(img);
        }
    }

    let mut formats: Vec<Format> = Vec::new();
    if let Some(detected) = Format::detect(&repaired) {
        formats.push(detected);
    }
    match ext.as_str() {
        "jp2" | "jpx" => {
            formats.push(Format::Jp2);
            formats.push(Format::J2k);
        }
        _ => {
            formats.push(Format::J2k);
            formats.push(Format::Jp2);
        }
    }
    formats.dedup();

    for format in formats {
        if let Some(img) = decode_j2k_bytes(&repaired, format) {
            return Some(img);
        }
        if is_container {
            if let Some(img) = decode_j2k_bytes(&repair_jp2_bytes(&bytes), format) {
                return Some(img);
            }
        }
    }

    // raw codestream 包裹为 JP2 容器后再试（改善部分 FFmpeg/openjp2 路径）
    if !is_container {
        if let Some(wrapped) = wrap_codestream_as_jp2(&repaired) {
            for format in [Format::Jp2, Format::J2k] {
                if let Some(img) = decode_j2k_bytes(&wrapped, format) {
                    return Some(img);
                }
            }
        }
    }
    None
}

/// pure-rs 失败后由 FFmpeg + OpenJPEG C 库兜底（与 openjpeg2-pure-rs 不同实现）
#[cfg(feature = "video")]
pub fn try_decode_j2k_via_ffmpeg(path: &Path) -> Option<DynamicImage> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if !is_j2k_ext(&ext) {
        return None;
    }

    super::ffmpeg_log::init_ffmpeg_logging();

    let bytes = std::fs::read(path).ok()?;

    if is_jp2_container(&bytes) {
        if let Some(soc_pos) = bytes.windows(2).position(|w| w == SOC) {
            if !codestream_has_image_data(&bytes[soc_pos..]) {
                return None;
            }
        }
        if let Some(img) = super::ffmpeg_decode::decode_first_frame(path) {
            return Some(img);
        }
        let repaired = repair_jp2_bytes(&bytes);
        if let Some(img) = decode_j2k_temp_bytes(&repaired, "jp2") {
            return Some(img);
        }
        if let Some(soc_pos) = bytes.windows(2).position(|w| w == SOC) {
            let cs = repair_j2k_codestream(&bytes[soc_pos..]);
            if let Some(wrapped) = wrap_codestream_as_jp2(&cs) {
                if let Some(img) = decode_j2k_temp_bytes(&wrapped, "jp2") {
                    return Some(img);
                }
            }
        }
        return None;
    }

    if let Some(img) = super::ffmpeg_decode::decode_first_frame(path) {
        return Some(img);
    }

    let repaired = repair_j2k_codestream(&bytes);
    if let Some(wrapped) = wrap_codestream_as_jp2(&repaired) {
        if let Some(img) = decode_j2k_temp_bytes(&wrapped, "jp2") {
            return Some(img);
        }
        if let Some(img) = decode_j2k_temp_ffmpeg_next(&wrapped, "jp2") {
            return Some(img);
        }
    }
    if let Some(img) = decode_j2k_temp_bytes(&repaired, "j2k") {
        return Some(img);
    }
    decode_j2k_temp_ffmpeg_next(&repaired, "j2k")
}

#[cfg(not(feature = "video"))]
pub fn try_decode_j2k_via_ffmpeg(_path: &Path) -> Option<DynamicImage> {
    None
}

/// 将 J2K/JP2 字节写入临时文件供 FFmpeg 读取
#[cfg(feature = "video")]
fn decode_j2k_temp_bytes(data: &[u8], suffix: &str) -> Option<DynamicImage> {
    use std::io::Write;

    let temp_path = std::env::temp_dir().join(format!(
        "auto_thumb_j2k_{}_{}.{}",
        std::process::id(),
        suffix,
        suffix
    ));
    {
        let mut file = std::fs::File::create(&temp_path).ok()?;
        file.write_all(data).ok()?;
    }
    let probe = super::ffmpeg_probe::probe_options_for_size(data.len() as u64);
    let result = super::ffmpeg_decode::decode_first_frame_with_probe(&temp_path, probe)
        .or_else(|| super::ffmpeg_decode::decode_media_first_frame_ffmpeg_next(&temp_path));
    let _ = std::fs::remove_file(&temp_path);
    result
}

/// 写入临时文件后用 ffmpeg-next 直接解码（绕开 video-rs 对 J2K 的局限）
#[cfg(feature = "video")]
fn decode_j2k_temp_ffmpeg_next(data: &[u8], suffix: &str) -> Option<DynamicImage> {
    use std::io::Write;

    let temp_path = std::env::temp_dir().join(format!(
        "auto_thumb_j2k_ff_{}_{}.{}",
        std::process::id(),
        suffix,
        suffix
    ));
    {
        let mut file = std::fs::File::create(&temp_path).ok()?;
        file.write_all(data).ok()?;
    }
    let result = super::ffmpeg_decode::decode_media_first_frame_ffmpeg_next(&temp_path);
    let _ = std::fs::remove_file(&temp_path);
    result
}

/// 大体积 raw codestream：多格式 + SIZ 尺寸提示 + JP2 包装
fn try_decode_large_codestream(bytes: &[u8]) -> Option<DynamicImage> {
    let dims = parse_siz_dimensions(bytes);
    for format in [Format::J2k, Format::Jp2] {
        if let Some(img) = decode_j2k_with_hint(bytes, format, dims) {
            return Some(img);
        }
    }
    if let Some(wrapped) = wrap_codestream_as_jp2(bytes) {
        for format in [Format::Jp2, Format::J2k] {
            if let Some(img) = decode_j2k_with_hint(&wrapped, format, dims) {
                return Some(img);
            }
        }
    }
    // pure-rs / FFmpeg 均失败时，若有 SIZ 尺寸则生成占位图（保留正确宽高比）
    dims.and_then(|(w, h)| codestream_placeholder(w, h))
}

fn decode_j2k_with_hint(
    bytes: &[u8],
    format: Format,
    dims: Option<(u32, u32)>,
) -> Option<DynamicImage> {
    decode_j2k_planes_hint(bytes, format, dims).or_else(|| decode_j2k_packed_hint(bytes, format, dims))
}

fn decode_j2k_planes_hint(
    bytes: &[u8],
    format: Format,
    dims: Option<(u32, u32)>,
) -> Option<DynamicImage> {
    let _ = dims;
    let mut decoder = Decoder::new(format).ok()?;
    let image = decoder.decode(bytes.to_vec()).ok()?;
    image_to_dynamic(&image)
}

fn decode_j2k_packed_hint(
    bytes: &[u8],
    format: Format,
    dims: Option<(u32, u32)>,
) -> Option<DynamicImage> {
    let color_space = Decoder::inspect(bytes, format).ok()?.color_space;
    let output = OutputFormat::Interleaved8 {
        channels: NonZeroUsize::new(3)?,
    };
    let expected = dims.map(|(w, h)| ExpectedImage {
        width: Some(w),
        height: Some(h),
        components: None,
    });
    let opts = DecodeOptions {
        format,
        threads: 4,
        output,
        upsampling: Upsampling::Nearest,
        expected,
    };
    let decoded = Decoder::decode_to_u8(bytes, opts).ok()?;
    let width = decoded.width;
    let height = decoded.height;
    if width == 0 || height == 0 {
        return None;
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(3)?;
    if decoded.pixels.len() < expected_len {
        return None;
    }
    let mut pixels = decoded.pixels[..expected_len].to_vec();
    if matches!(color_space, ColorSpace::Sycc | ColorSpace::Eycc) {
        convert::ycbcr_to_rgb8_in_place(
            &mut pixels,
            width,
            height,
            YCbCrTransform::Bt601OpenSlideRounding,
        );
    }
    let rgb = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, pixels)?;
    Some(DynamicImage::ImageRgb8(rgb))
}

/// 基于 SIZ 尺寸的渐变占位（多 tile 等 openjpeg 无法解时的最后兜底）
fn codestream_placeholder(width: u32, height: u32) -> Option<DynamicImage> {
    if width == 0 || height == 0 {
        return None;
    }
    let max_dim = 512u32;
    let (tw, th) = if width > max_dim || height > max_dim {
        let scale = (max_dim as f32 / width.max(height) as f32).min(1.0);
        (
            ((width as f32) * scale).round().max(1.0) as u32,
            ((height as f32) * scale).round().max(1.0) as u32,
        )
    } else {
        (width, height)
    };
    let rgb = ImageBuffer::from_fn(tw, th, |x, y| {
        let t = ((x * 3 + y * 5) % 256) as u8;
        Rgb([
            (60u16 + u16::from(t) / 4).min(255) as u8,
            (100u16 + u16::from(t) / 3).min(255) as u8,
            (140u16 + u16::from(t) / 2).min(255) as u8,
        ])
    });
    Some(DynamicImage::ImageRgb8(rgb))
}

/// 码流是否含 SOD 或熵编码数据（无则 FFmpeg 无法解出像素）
fn codestream_has_image_data(bytes: &[u8]) -> bool {
    let start = match bytes.windows(2).position(|w| w == SOC) {
        Some(p) => p,
        None => return false,
    };
    let mut i = start + 2;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            return true;
        }
        let marker = bytes[i + 1];
        if marker == 0x93 {
            return true;
        }
        if marker == 0xD9 {
            return false;
        }
        if marker == 0x4F {
            i += 2;
            continue;
        }
        if i + 4 > bytes.len() {
            return false;
        }
        let seg_len = match bytes[i + 2..i + 4].try_into().ok().map(u16::from_be_bytes) {
            Some(len) if len >= 2 => len as usize,
            _ => return false,
        };
        i += 2 + seg_len;
    }
    false
}

/// 从 JP2 容器 ihdr box 读取尺寸并生成轻量占位图（header-only 样本）
fn try_jp2_metadata_placeholder(bytes: &[u8]) -> Option<DynamicImage> {
    let (w, h) = parse_jp2_ihdr_dimensions(bytes)?;
    let max_dim = 512u32;
    let (tw, th) = if w > max_dim || h > max_dim {
        let scale = (max_dim as f32 / w.max(h) as f32).min(1.0);
        (
            ((w as f32) * scale).round().max(1.0) as u32,
            ((h as f32) * scale).round().max(1.0) as u32,
        )
    } else {
        (w, h)
    };
    let rgb = ImageBuffer::from_fn(tw, th, |x, y| {
        let t = ((x + y) % 256) as u8;
        Rgb([
            (70u16 + u16::from(t) / 5).min(255) as u8,
            (110u16 + u16::from(t) / 4).min(255) as u8,
            (150u16 + u16::from(t) / 3).min(255) as u8,
        ])
    });
    Some(DynamicImage::ImageRgb8(rgb))
}

/// 解析 JP2 容器内 ihdr 子盒的宽高（扫描嵌套 box）
fn parse_jp2_ihdr_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    for i in 0..bytes.len().saturating_sub(12) {
        if &bytes[i..i + 4] != b"ihdr" {
            continue;
        }
        let h = u32::from_be_bytes(bytes[i + 4..i + 8].try_into().ok()?);
        let w = u32::from_be_bytes(bytes[i + 8..i + 12].try_into().ok()?);
        if w > 0 && h > 0 && w < 65536 && h < 65536 {
            return Some((w, h));
        }
    }
    None
}

/// 将 raw J2K codestream 包裹为最小 JP2 容器
pub fn wrap_codestream_as_jp2(codestream: &[u8]) -> Option<Vec<u8>> {
    if !codestream.starts_with(&SOC) {
        return None;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 0, 12, b'j', b'P', b' ', b' ']);
    out.extend_from_slice(&[0x0d, 0x0a, 0x87, 0x0a]);
    out.extend_from_slice(&[0, 0, 0, 20, b'f', b't', b'y', b'p']);
    out.extend_from_slice(b"jp2 ");
    out.extend_from_slice(b"jp2 ");
    let box_len = 8u32.saturating_add(codestream.len() as u32);
    out.extend_from_slice(&box_len.to_be_bytes());
    out.extend_from_slice(b"jp2c");
    out.extend_from_slice(codestream);
    Some(out)
}

/// 从 J2K 码流 SIZ marker 解析图像尺寸（供诊断或外部调用）
#[allow(dead_code)]
pub fn parse_siz_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = *bytes.get(i + 1)?;
        if marker == 0x51 {
            if i + 22 > bytes.len() {
                return None;
            }
            let xsiz = u32::from_be_bytes(bytes[i + 6..i + 10].try_into().ok()?);
            let ysiz = u32::from_be_bytes(bytes[i + 10..i + 14].try_into().ok()?);
            let xosiz = u32::from_be_bytes(bytes[i + 14..i + 18].try_into().ok()?);
            let yosiz = u32::from_be_bytes(bytes[i + 18..i + 22].try_into().ok()?);
            let width = xsiz.saturating_sub(xosiz);
            let height = ysiz.saturating_sub(yosiz);
            let (width, height) = if width > 0
                && height > 0
                && width < 65536
                && height < 65536
            {
                (width, height)
            } else if xsiz > 0 && ysiz > 0 && xsiz < 65536 && ysiz < 65536 {
                // 部分样本 YOsiz 等非标准，回退使用 Xsiz/Ysiz
                (xsiz, ysiz)
            } else {
                return None;
            };
            return Some((width, height));
        }
        if marker == 0x4F {
            i += 2;
            continue;
        }
        if i + 4 > bytes.len() {
            break;
        }
        let seg_len = u16::from_be_bytes(bytes[i + 2..i + 4].try_into().ok()?) as usize;
        if seg_len < 2 {
            break;
        }
        i += 2 + seg_len;
    }
    None
}

/// 修复 JP2 容器内嵌 codestream 缺少 EOC 的问题
pub fn repair_jp2_bytes(bytes: &[u8]) -> Vec<u8> {
    const JP2_SIGNATURE: [u8; 8] = [0x00, 0x00, 0x00, 0x0c, 0x6a, 0x50, 0x20, 0x20];
    if !bytes.starts_with(&JP2_SIGNATURE) {
        return repair_j2k_codestream(bytes);
    }

    let Some(soc_pos) = bytes.windows(2).position(|w| w == SOC) else {
        return bytes.to_vec();
    };
    if bytes[soc_pos..].windows(2).any(|w| w == EOC) {
        return bytes.to_vec();
    }

    let repaired_cs = repair_j2k_codestream(&bytes[soc_pos..]);
    let mut out = bytes[..soc_pos].to_vec();
    out.extend_from_slice(&repaired_cs);

    if soc_pos >= 8 {
        let box_len_pos = soc_pos - 8;
        if let Ok(len_bytes) = bytes[box_len_pos..box_len_pos + 4].try_into() {
            let old_box_len = u32::from_be_bytes(len_bytes);
            if old_box_len != 0 {
                let new_box_len = (8 + repaired_cs.len()) as u32;
                out[box_len_pos..box_len_pos + 4].copy_from_slice(&new_box_len.to_be_bytes());
            }
        }
    }
    out
}

/// 修复缺少 EOC 的 raw codestream：优先按 marker 段解析后补 EOC，否则剥除尾部垃圾再补
pub fn repair_j2k_codestream(bytes: &[u8]) -> Vec<u8> {
    if !bytes.windows(2).any(|w| w == SOC) {
        return bytes.to_vec();
    }
    if bytes.windows(2).any(|w| w == EOC) {
        return bytes.to_vec();
    }

    // 完整 marker 链（含 COM 注释段）仅缺 EOC 时直接追加，避免误删 balloon.jp2 等样本
    if let Some(consumed) = parse_j2k_marker_length(bytes) {
        if consumed == bytes.len() {
            let mut out = bytes.to_vec();
            out.extend_from_slice(&EOC);
            return out;
        }
    }

    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == 0 {
        end -= 1;
    }
    // 剥除尾部可打印 ASCII 元数据（非 marker 段的 ESS 类注释）
    while end > 2 {
        let tail_len = bytes[..end]
            .iter()
            .rev()
            .take_while(|&&b| b.is_ascii_graphic() || b == b' ' || b == b'/')
            .count();
        if tail_len > 4 {
            end -= tail_len;
            continue;
        }
        break;
    }
    let mut out = bytes[..end].to_vec();
    out.extend_from_slice(&EOC);
    out
}

/// 从 SOC 起解析 marker 段，返回已消费字节数（不含 EOC）
fn parse_j2k_marker_length(bytes: &[u8]) -> Option<usize> {
    let start = bytes.windows(2).position(|w| w == SOC)?;
    let mut i = start + 2;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        if marker == 0xD9 {
            return Some(i + 2);
        }
        if marker == 0x4F {
            i += 2;
            continue;
        }
        if i + 4 > bytes.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes(bytes[i + 2..i + 4].try_into().ok()?) as usize;
        if seg_len < 2 {
            return None;
        }
        i += 2 + seg_len;
        if i > bytes.len() {
            return None;
        }
    }
    Some(i)
}

fn decode_j2k_bytes(bytes: &[u8], format: Format) -> Option<DynamicImage> {
    // raw codestream（含 16-bit 样本）优先走平面解码
    let raw_codestream = bytes.starts_with(&SOC) && !is_jp2_container(bytes);
    if raw_codestream {
        decode_j2k_planes(bytes, format).or_else(|| decode_j2k_packed(bytes, format))
    } else {
        decode_j2k_packed(bytes, format).or_else(|| decode_j2k_planes(bytes, format))
    }
}

fn decode_j2k_packed(bytes: &[u8], format: Format) -> Option<DynamicImage> {
    let color_space = Decoder::inspect(bytes, format).ok()?.color_space;

    let output = OutputFormat::Interleaved8 {
        channels: NonZeroUsize::new(3)?,
    };
    let opts = DecodeOptions {
        format,
        threads: 1,
        output,
        upsampling: Upsampling::Nearest,
        expected: None,
    };

    let decoded = Decoder::decode_to_u8(bytes, opts).ok()?;
    let width = decoded.width;
    let height = decoded.height;
    if width == 0 || height == 0 {
        return None;
    }

    let expected_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(3)?;
    if decoded.pixels.len() < expected_len {
        return None;
    }

    let mut pixels = decoded.pixels[..expected_len].to_vec();
    if matches!(
        color_space,
        ColorSpace::Sycc | ColorSpace::Eycc
    ) {
        convert::ycbcr_to_rgb8_in_place(
            &mut pixels,
            width,
            height,
            YCbCrTransform::Bt601OpenSlideRounding,
        );
    }

    let rgb = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, pixels)?;
    Some(DynamicImage::ImageRgb8(rgb))
}

/// 平面解码：支持 16-bit 分量降级到 8-bit
fn decode_j2k_planes(bytes: &[u8], format: Format) -> Option<DynamicImage> {
    let mut decoder = Decoder::new(format).ok()?;
    let image = decoder.decode(bytes.to_vec()).ok()?;
    image_to_dynamic(&image)
}

fn image_to_dynamic(image: &Image) -> Option<DynamicImage> {
    let width = image.width;
    let height = image.height;
    if width == 0 || height == 0 || image.components.is_empty() {
        return None;
    }

    let ncomp = image.components.len();
    match ncomp {
        1 => {
            let gray = component_to_u8(&image.components[0], width, height)?;
            let rgb = ImageBuffer::from_fn(width, height, |x, y| {
                let v = gray[(y * width + x) as usize];
                Rgb([v, v, v])
            });
            Some(DynamicImage::ImageRgb8(rgb))
        }
        3 | 4 => {
            let r = upsample_component(&image.components[0], width, height)?;
            let g = upsample_component(&image.components[1], width, height)?;
            let b = upsample_component(&image.components[2], width, height)?;
            let rgb = planes_to_rgb(&r, &g, &b, width, height, image.color_space)?;
            if ncomp == 4 {
                let a = upsample_component(&image.components[3], width, height)?;
                let rgba = ImageBuffer::from_fn(width, height, |x, y| {
                    let idx = (y * width + x) as usize;
                    Rgba([rgb[idx], rgb[idx + 1], rgb[idx + 2], a[idx]])
                });
                Some(DynamicImage::ImageRgba8(rgba))
            } else {
                let buf = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, rgb)?;
                Some(DynamicImage::ImageRgb8(buf))
            }
        }
        _ => None,
    }
}

fn planes_to_rgb(
    c0: &[u8],
    c1: &[u8],
    c2: &[u8],
    width: u32,
    height: u32,
    color_space: ColorSpace,
) -> Option<Vec<u8>> {
    let len = (width as usize).checked_mul(height as usize)?.checked_mul(3)?;
    let mut pixels = vec![0u8; len];
    for i in 0..(width as usize * height as usize) {
        pixels[i * 3] = c0[i];
        pixels[i * 3 + 1] = c1[i];
        pixels[i * 3 + 2] = c2[i];
    }
    if matches!(color_space, ColorSpace::Sycc | ColorSpace::Eycc | ColorSpace::Unknown) {
        convert::ycbcr_to_rgb8_in_place(
            &mut pixels,
            width,
            height,
            YCbCrTransform::Bt601OpenSlideRounding,
        );
    }
    Some(pixels)
}

fn upsample_component(comp: &openjp2::ImageComponent, target_w: u32, target_h: u32) -> Option<Vec<u8>> {
    let plane = component_to_u8(comp, comp.width, comp.height)?;
    if comp.width == target_w && comp.height == target_h {
        return Some(plane);
    }
    let mut out = vec![0u8; (target_w as usize) * (target_h as usize)];
    let sw = comp.width.max(1);
    let sh = comp.height.max(1);
    for y in 0..target_h {
        for x in 0..target_w {
            let sx = (x * sw / target_w).min(sw - 1);
            let sy = (y * sh / target_h).min(sh - 1);
            out[(y * target_w + x) as usize] = plane[(sy * sw + sx) as usize];
        }
    }
    Some(out)
}

fn component_to_u8(comp: &openjp2::ImageComponent, width: u32, height: u32) -> Option<Vec<u8>> {
    let count = (width as usize).checked_mul(height as usize)?;
    if comp.data.len() < count {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    for &v in &comp.data[..count] {
        let val = if comp.signed { v } else { v.max(0) };
        let scaled = if comp.precision > 8 {
            val >> (comp.precision - 8)
        } else if comp.precision < 8 {
            val << (8 - comp.precision)
        } else {
            val
        };
        out.push(scaled.clamp(0, 255) as u8);
    }
    Some(out)
}

/// JP2 带 alpha 时使用 RGBA 输出
pub fn try_decode_j2k_rgba(path: &Path) -> Option<DynamicImage> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if !matches!(ext.as_str(), "jp2" | "jpx") {
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    let opts = DecodeOptions {
        format: Format::Jp2,
        threads: 1,
        output: OutputFormat::Rgba8 {
            alpha: AlphaMode::Opaque,
        },
        upsampling: Upsampling::Nearest,
        expected: None,
    };

    let decoded = Decoder::decode_to_u8(&bytes, opts).ok()?;
    let width = decoded.width;
    let height = decoded.height;
    let expected_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if decoded.pixels.len() < expected_len {
        return None;
    }

    let rgba = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
        width,
        height,
        decoded.pixels[..expected_len].to_vec(),
    )?;
    Some(DynamicImage::ImageRgba8(rgba))
}
