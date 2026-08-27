//! FFmpeg 探针参数：J2K 用大探针，视频用较小探针 + 损坏容器容错 flags。

use std::collections::HashMap;
use std::path::Path;

/// J2K / 大体积图像 codestream
const MIN_PROBE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_PROBE_BYTES: u64 = 128 * 1024 * 1024;
const MIN_ANALYZE_US: u64 = 10_000_000;
const MAX_ANALYZE_US: u64 = 120_000_000;

/// 视频缩略图：较小探针，避免损坏容器在 analyze 阶段读过多 packet
const VIDEO_PROBE_BYTES: u64 = 2 * 1024 * 1024;
const VIDEO_ANALYZE_US: u64 = 3_000_000;

/// 10MB probesize + 10s analyzeduration
#[allow(dead_code)]
pub fn probe_options() -> HashMap<String, String> {
    probe_options_for_size(MIN_PROBE_BYTES)
}

/// 按文件/字节规模放大探针窗口，便于大体积 J2K codestream
pub fn probe_options_for_size(byte_len: u64) -> HashMap<String, String> {
    let probesize = byte_len.max(1_048_576).min(MAX_PROBE_BYTES);
    let analyzeduration = (byte_len.saturating_mul(2)).clamp(MIN_ANALYZE_US, MAX_ANALYZE_US);
    base_probe_map(probesize, analyzeduration)
}

/// 从路径读取文件大小后生成探针参数（J2K 等图像路径）
pub fn probe_options_for_path(path: &Path) -> HashMap<String, String> {
    let byte_len = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(MIN_PROBE_BYTES);
    probe_options_for_size(byte_len)
}

/// 视频解码：2MB/3s 探针 + 损坏容器容错 flags（与 J2K 大探针分离）
pub fn probe_options_for_video(path: &Path) -> HashMap<String, String> {
    let byte_len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let probesize = byte_len.min(VIDEO_PROBE_BYTES).max(512 * 1024);
    let mut opts = base_probe_map(probesize, VIDEO_ANALYZE_US);
    opts.insert("fflags".to_string(), "+discardcorrupt+genpts".to_string());
    opts.insert("err_detect".to_string(), "ignore_err".to_string());
    opts
}

fn base_probe_map(probesize: u64, analyzeduration: u64) -> HashMap<String, String> {
    HashMap::from([
        ("probesize".to_string(), probesize.to_string()),
        ("analyzeduration".to_string(), analyzeduration.to_string()),
    ])
}
