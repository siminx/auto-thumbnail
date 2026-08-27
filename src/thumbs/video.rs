//! 视频首帧提取：ffmpeg-next + packet 预算，多时间点 seek，损坏容器早停。

use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use image::DynamicImage;

use crate::decode::ffmpeg_decode::{self, VideoFrameError};
use crate::decode::{ffmpeg_log, ffmpeg_probe};
use crate::frame_valid::is_effectively_blank;

/// 解码第一帧；失败时依次 seek 到多个时间点
pub(crate) fn create_thumbnail<P>(path: P, width: u32, height: u32) -> anyhow::Result<DynamicImage>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    ffmpeg_log::init_ffmpeg_logging();

    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("swf"))
    {
        if let Some(img) = ffmpeg_decode::decode_swf_first_frame(path) {
            return Ok(img.thumbnail(width, height));
        }
        anyhow::bail!("无法解码 SWF 首帧: {:?}", path);
    }

    let probe = ffmpeg_probe::probe_options_for_video(path);
    let duration = ffmpeg_decode::probe_duration_secs(path, probe.clone());
    let seek_points = seek_points_for(path, duration);
    let mut open_failed = false;

    for &seconds in &seek_points {
        if open_failed {
            break;
        }
        match decode_frame_at_offset(path, seconds, &probe) {
            Ok(img) => {
                if !is_effectively_blank(&img) {
                    return Ok(img.thumbnail(width, height));
                }
            }
            Err(err) => {
                if seconds == 0.0 && is_corrupt_container_error(&err) {
                    open_failed = true;
                }
            }
        }
    }

    Err(anyhow::anyhow!("无法找到有效视频帧: {:?}", path))
}

/// 合并固定时间点与时长比例 seek 点，去重排序
fn seek_points_for(_path: &Path, duration_secs: Option<f64>) -> Vec<f64> {
    let mut points = vec![0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0];

    if let Some(duration) = duration_secs.filter(|&d| d > 1.0) {
        for ratio in [0.10, 0.25, 0.50] {
            let secs = duration * ratio;
            let clamped = secs.clamp(1.0, 120.0);
            if clamped < duration - 0.5 {
                points.push(clamped);
            }
        }
    }

    points.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    points.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    points
}

/// 损坏容器/流特征：首帧失败时跳过后续 seek，避免重复劳动
fn is_corrupt_container_error(err: &anyhow::Error) -> bool {
    if let Some(vfe) = err.downcast_ref::<VideoFrameError>() {
        return matches!(
            vfe,
            VideoFrameError::PacketBudgetExhausted
                | VideoFrameError::ConsecutiveDecodeFailures
                | VideoFrameError::NoFrame
                | VideoFrameError::InvalidSize
        );
    }
    let msg = err.to_string();
    msg.contains("Invalid mvhd")
        || msg.contains("无法打开视频")
        || msg.contains("StreamNotFound")
        || msg.contains("packet budget exhausted")
        || msg.contains("consecutive decode failures")
        || msg.contains("no frame decoded")
}

fn decode_frame_at_offset(
    path: &Path,
    seconds: f64,
    probe: &HashMap<String, String>,
) -> anyhow::Result<DynamicImage> {
    ffmpeg_decode::decode_video_first_frame(path, seconds, probe.clone())
        .map_err(|e| anyhow::Error::new(e))
        .with_context(|| format!("seek={seconds}s {:?}", path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seek_points_include_duration_ratios() {
        let path = Path::new("test.mp4");
        let points = seek_points_for(path, Some(100.0));
        assert!(points.contains(&10.0));
        assert!(points.contains(&25.0));
        assert!(points.contains(&50.0));
    }

    #[test]
    fn seek_points_skip_ratio_near_end() {
        let path = Path::new("test.mp4");
        // 过短视频不追加比例 seek 点
        let with_duration = seek_points_for(path, Some(1.5));
        let fixed_only = seek_points_for(path, None);
        assert_eq!(with_duration, fixed_only);
    }
}
