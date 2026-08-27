//! 通用图像解码：ImageReader → ICO 宽松 → 色调映射。
//!
//! 不含平台 Shell/GDI；应用层（cherry-box）负责最终兜底。

pub(crate) mod ffmpeg_decode;
mod ffmpeg_image;
pub(crate) mod ffmpeg_log;
pub(crate) mod ffmpeg_probe;
mod ico;
mod j2k;
mod jxl;
mod mng;
mod reader;
pub mod tone_map;

use std::path::Path;
use std::sync::Once;

use image::DynamicImage;
use thiserror::Error;

static REGISTER_EXTRAS: Once = Once::new();

fn ensure_extras_registered() {
    REGISTER_EXTRAS.call_once(|| {
        image_extras::register();
    });
}

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("无法解码图像")]
    Unsupported,
}

/// 尽力解码任意支持的图像文件
pub fn decode_image(path: &Path) -> Result<DynamicImage, DecodeError> {
    ensure_extras_registered();
    #[cfg(feature = "video")]
    ffmpeg_log::init_ffmpeg_logging();
    reader::try_decode_reader(path)
        .or_else(|| jxl::try_decode_jxl(path))
        .or_else(|| j2k::try_decode_j2k(path))
        .or_else(|| j2k::try_decode_j2k_rgba(path))
        .or_else(|| j2k::try_decode_j2k_via_ffmpeg(path))
        .or_else(|| ico::try_decode_ico(path))
        .or_else(|| mng::try_decode_mng(path))
        .or_else(|| ffmpeg_image::try_decode_via_ffmpeg(path))
        .ok_or(DecodeError::Unsupported)
}

/// 解码并缩放到 max_dim 边长内
pub fn decode_and_thumbnail(path: &Path, max_dim: u32) -> Result<DynamicImage, DecodeError> {
    let img = decode_image(path)?;
    Ok(if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn decode_reported_hdr_samples() {
        let samples = [
            r"D:\xsmspace\data\1\file\图像\photo_hdr.hdr",
            r"D:\xsmspace\data\2\file\图像\sample.hdr",
        ];
        for sample in samples {
            let path = Path::new(sample);
            if !path.exists() {
                eprintln!("跳过不存在样本: {sample}");
                continue;
            }
            let img = decode_image(path).unwrap_or_else(|_| panic!("无法解码 HDR: {sample}"));
            assert!(img.width() > 0 && img.height() > 0);
        }
    }

    /// ravif 暂不支持 HDR AVIF（10-bit）；此类样本由应用层 Shell 兜底
    #[test]
    fn decode_reported_avif_best_effort() {
        let samples = [
            r"D:\xsmspace\data\2\_clone_sample-files\images\sample.avif",
            r"D:\xsmspace\data\1\file\图像\hdr_cosmos.avif",
            r"D:\xsmspace\data\2\file\图像\sample.avif",
        ];
        for sample in samples {
            let path = Path::new(sample);
            if !path.exists() {
                continue;
            }
            match decode_image(path) {
                Ok(img) => assert!(img.width() > 0 && img.height() > 0),
                Err(_) => eprintln!("AVIF 纯 Rust 解码跳过（可能为 HDR AVIF）: {sample}"),
            }
        }
    }

    /// header-only JP2 与 raw codestream FFmpeg 兜底
    #[test]
    fn decode_j2k_ffmpeg_fallback_samples() {
        let samples = [
            r"D:\xsmspace\data\1\file\图像\d2_colr.j2c",
            r"D:\xsmspace\data\1\file\图像\imagery.jpc",
            r"D:\xsmspace\data\1\file\图像\balloon.jp2",
        ];
        for sample in samples {
            let path = Path::new(sample);
            if !path.exists() {
                eprintln!("跳过不存在: {sample}");
                continue;
            }
            let img = decode_image(path).unwrap_or_else(|_| panic!("必须能解码: {sample}"));
            assert!(img.width() > 0 && img.height() > 0, "{sample}");
            eprintln!("OK: {sample} {}x{}", img.width(), img.height());
        }
    }

    #[test]
    fn decode_logged_failure_samples_best_effort() {
        let required = [r"D:\xsmspace\data\1\file\图像\animated.mng"];
        for sample in required {
            let path = Path::new(sample);
            if !path.exists() {
                eprintln!("跳过不存在样本: {sample}");
                continue;
            }
            let img = decode_image(path).unwrap_or_else(|_| panic!("必须能解码: {sample}"));
            assert!(img.width() > 0 && img.height() > 0, "{sample}");
        }

        let samples = [
            r"D:\xsmspace\data\1\file\图像\BLOOD02.pcx",
            r"D:\xsmspace\data\1\file\图像\balloon.jp2",
            r"D:\xsmspace\data\1\file\图像\imagery.jpc",
            r"D:\xsmspace\data\1\file\图像\cropped_16bit.j2k",
            r"D:\xsmspace\data\1\file\图像\d2_colr.j2c",
            r"D:\xsmspace\data\1\file\图像\balloon.jpf",
            r"D:\xsmspace\data\1\file\图像\deerstalker.cur",
            r"D:\xsmspace\data\1\file\图像\hdr_cosmos.avif",
            r"D:\xsmspace\data\1\file\图像\g4-multi.tiff",
            r"D:\xsmspace\data\1\file\图像\dscf0013.tif",
        ];
        let mut ok_count = 0;
        for sample in samples {
            let path = Path::new(sample);
            if !path.exists() {
                continue;
            }
            if decode_image(path).is_ok() {
                ok_count += 1;
            }
        }
        assert!(
            ok_count >= 8,
            "auto-thumbnail 解码样本成功率过低: {ok_count}/{}",
            samples.len()
        );
    }
}
