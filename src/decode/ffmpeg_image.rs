//! FFmpeg 图像兜底：Rust 解码失败时由 FFmpeg demuxer 取首帧（HDR AVIF、G4 TIFF 等）。
//!
//! JPEG2000 由 `j2k::try_decode_j2k_via_ffmpeg` 单独兜底（OpenJPEG C 库，与 pure-rs 不同实现）。

use std::path::Path;

use image::DynamicImage;

use super::ffmpeg_decode;

/// 扩展名白名单：仅在 Rust 解码失败后尝试 FFmpeg
pub fn should_try_ffmpeg_fallback(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "avif" | "tif" | "tiff"
            )
        })
}

/// 通过 video-rs/FFmpeg 解码第一帧为 RGB 图像
#[cfg(feature = "video")]
pub fn try_decode_via_ffmpeg(path: &Path) -> Option<DynamicImage> {
    if !should_try_ffmpeg_fallback(path) {
        return None;
    }
    ffmpeg_decode::decode_first_frame(path)
}

#[cfg(not(feature = "video"))]
pub fn try_decode_via_ffmpeg(_path: &Path) -> Option<DynamicImage> {
    None
}
