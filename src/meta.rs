//! 媒体元数据探测：只读文件头/容器元信息，不解码像素。
//!
//! 供索引等批量场景低成本采集"分辨率"与"时长"：图片/RAW/PSD 头部毫秒级，
//! 视频走 ffmpeg 容器探测（数十毫秒）。读不到的格式返回 None，调用方按
//! "该文件无此属性"处理，不应重试或回退到全量解码。

use std::path::Path;

/// 媒体元数据；字段为 None 表示该文件类型无此属性或读取失败
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MediaMeta {
    /// 像素宽度（视频为编码分辨率，未处理旋转元数据）
    pub width: Option<u32>,
    /// 像素高度
    pub height: Option<u32>,
    /// 时长（秒）；视频取容器 duration，音频取流属性
    pub duration_secs: Option<f64>,
}

impl MediaMeta {
    /// 宽高是否构成有效的"分辨率"展示值
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
            _ => None,
        }
    }
}

/// 按扩展名分派探测媒体元数据；无法识别的扩展名返回 None
pub fn probe_media_meta(path: &Path) -> Option<MediaMeta> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())?;

    #[cfg(feature = "video")]
    if crate::mime_resolve::is_video_ext(&ext) {
        return probe_video(path);
    }

    #[cfg(feature = "audio")]
    if crate::mime_resolve::is_audio_ext(&ext) {
        return probe_audio(path);
    }

    // PSD/PSB 头部结构相同（仅 version 不同），直接解析文件头
    if matches!(ext.as_str(), "psd" | "psb") {
        return probe_psd_header(path);
    }

    #[cfg(feature = "pdf")]
    if ext == "ai" {
        return probe_ai_page_size(path);
    }

    // RAW 与普通图片统一走头部尺寸读取：TIFF 系 RAW（DNG/CR2/NEF 等）可读，
    // 私有容器（CRW/X3F 等）读不到即 None；SVG 无固定像素尺寸也会在此返回 None
    probe_image_dimensions(path)
}

/// 视频：ffmpeg 容器探测宽高与时长；宽高拿不到时整体视为不可解析
#[cfg(feature = "video")]
fn probe_video(path: &Path) -> Option<MediaMeta> {
    let probe = crate::decode::ffmpeg_probe::probe_options_for_video(path);
    let (dimensions, duration_secs) = crate::decode::ffmpeg_decode::probe_video_meta(path, probe);
    let (width, height) = dimensions?;
    Some(MediaMeta {
        width: Some(width),
        height: Some(height),
        duration_secs,
    })
}

/// 音频：lofty 只读标签与流属性即可拿到时长
#[cfg(feature = "audio")]
fn probe_audio(path: &Path) -> Option<MediaMeta> {
    use lofty::file::AudioFile;

    let tagged = lofty::read_from_path(path).ok()?;
    let duration = tagged.properties().duration();
    if duration.is_zero() {
        return None;
    }
    Some(MediaMeta {
        width: None,
        height: None,
        duration_secs: Some(duration.as_secs_f64()),
    })
}

/// PSD/PSB 文件头：签名 "8BPS"，通道数之后紧跟大端 height/width，
/// 只读 22 字节，比库全量解析便宜几个量级
fn probe_psd_header(path: &Path) -> Option<MediaMeta> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 22];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"8BPS" {
        return None;
    }
    // 头部布局：signature(0-3) version(4-5) reserved(6-11) channels(12-13)
    // height(14-17) width(18-21)，均为大端
    let height = u32::from_be_bytes(header[14..18].try_into().ok()?);
    let width = u32::from_be_bytes(header[18..22].try_into().ok()?);
    Some(MediaMeta {
        width: Some(width),
        height: Some(height),
        duration_secs: None,
    })
}

/// AI 文件多为 PDF 兼容封装，经 pdfium 读首页画板尺寸；私有二进制打不开即 None
#[cfg(feature = "pdf")]
fn probe_ai_page_size(path: &Path) -> Option<MediaMeta> {
    let (width, height) = crate::thumbs::pdf::probe_page_size(path)?;
    Some(MediaMeta {
        width: Some(width),
        height: Some(height),
        duration_secs: None,
    })
}

/// 只读头部拿像素尺寸；image crate 不认识的格式（含各 RAW 私有容器）返回 None
fn probe_image_dimensions(path: &Path) -> Option<MediaMeta> {
    let (width, height) = image::ImageReader::open(path)
        .ok()?
        .into_dimensions()
        .ok()?;
    Some(MediaMeta {
        width: Some(width),
        height: Some(height),
        duration_secs: None,
    })
}
