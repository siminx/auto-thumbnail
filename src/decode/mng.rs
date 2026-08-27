//! MNG 动画图像首帧提取：解析 chunk 重组内嵌 PNG，不依赖 FFmpeg MNG demuxer。

use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageReader};

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const MNG_SIGNATURE: [u8; 8] = [0x8A, b'M', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// 解码 MNG 文件首帧
pub fn try_decode_mng(path: &Path) -> Option<DynamicImage> {
    if path.extension()?.to_str()?.to_ascii_lowercase() != "mng" {
        return None;
    }

    let data = std::fs::read(path).ok()?;

    if data.starts_with(&PNG_SIGNATURE) {
        return decode_png_bytes(&data);
    }

    if !data.starts_with(&MNG_SIGNATURE) {
        return None;
    }

    let png = extract_first_png_from_mng(&data)?;
    decode_png_bytes(&png)
}

fn decode_png_bytes(bytes: &[u8]) -> Option<DynamicImage> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
}

/// 从 MNG 数据流中提取首帧 PNG 字节（含 PNG signature）
fn extract_first_png_from_mng(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = MNG_SIGNATURE.len();
    let mut png = Vec::new();
    let mut in_png = false;

    while pos + 12 <= data.len() {
        let length = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        let chunk_type: [u8; 4] = data[pos + 4..pos + 8].try_into().ok()?;
        let chunk_end = pos + 8 + length + 4;
        if chunk_end > data.len() {
            break;
        }
        let chunk_bytes = &data[pos..chunk_end];

        if chunk_type == *b"IHDR" {
            in_png = true;
            png.clear();
            png.extend_from_slice(&PNG_SIGNATURE);
            png.extend_from_slice(chunk_bytes);
        } else if in_png {
            if is_mng_control_chunk(&chunk_type) {
                if chunk_type == *b"IEND" {
                    // 不应出现：IEND 是 PNG chunk
                }
                break;
            }
            if is_png_chunk(&chunk_type) {
                png.extend_from_slice(chunk_bytes);
                if chunk_type == *b"IEND" {
                    return Some(png);
                }
            }
        } else if is_mng_control_chunk(&chunk_type) && chunk_type == *b"MEND" {
            break;
        }

        pos = chunk_end;
    }

    if png.len() > PNG_SIGNATURE.len() {
        // 有 IHDR/IDAT 但缺少 IEND 时仍尝试解码
        return Some(png);
    }
    None
}

fn is_mng_control_chunk(chunk_type: &[u8; 4]) -> bool {
    matches!(
        chunk_type,
        b"MHDR"
            | b"FRAM"
            | b"MOVE"
            | b"CLON"
            | b"LOOP"
            | b"ENDL"
            | b"DEFI"
            | b"BASE"
            | b"BASI"
            | b"DHDR"
            | b"DROP"
            | b"PAST"
            | b"MAGN"
            | b"MEND"
            | b"SEEK"
            | b"SHOW"
            | b"BACK"
            | b"TERM"
            | b"SAVE"
            | b"DISC"
            | b"CBRD"
    )
}

fn is_png_chunk(chunk_type: &[u8; 4]) -> bool {
    if is_mng_control_chunk(chunk_type) {
        return false;
    }
    // PNG chunk：末字节可小写；MNG 控制 chunk 通常全大写
    chunk_type[0].is_ascii_alphabetic()
        && chunk_type[1].is_ascii_alphabetic()
        && chunk_type[2].is_ascii_alphabetic()
        && chunk_type[3].is_ascii_alphabetic()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn decode_reported_mng_sample() {
        let sample = r"D:\xsmspace\data\1\file\图像\animated.mng";
        let path = Path::new(sample);
        if !path.exists() {
            return;
        }
        let img = try_decode_mng(path).expect("应能解码 animated.mng 首帧");
        assert!(img.width() > 0 && img.height() > 0);
    }
}
