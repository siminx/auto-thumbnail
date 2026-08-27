//! MIME 解析：tika_magic 结果 + 扩展名兜底，修正 ogv/m4v/mng 等误判。

use std::path::Path;

use crate::types::{
    AUDIO_EXTENSIONS, AUDIO_MIME_TYPES, IMAGE_MIME_TYPES, OFFICE_EXTENSIONS, OFFICE_MIME_TYPES,
    PDF_MIME_TYPES, RAW_EXTENSIONS, VIDEO_MIME_TYPES,
};

/// 扩展名 → MIME，与 cherry-box formats 对齐
fn mime_from_extension(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" | "dib" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        "ico" | "cur" => Some("image/x-icon"),
        "pcx" => Some("image/x-pcx"),
        "jp2" | "jpx" | "j2k" | "j2c" | "jpf" | "jpc" => Some("image/jp2"),
        "tga" => Some("image/x-tga"),
        "avif" => Some("image/avif"),
        "exr" => Some("image/x-exr"),
        "hdr" => Some("image/vnd.radiance"),
        "svg" => Some("image/svg+xml"),
        "mng" => Some("image/x-mng"),
        "ppm" => Some("image/x-portable-pixmap"),
        "pgm" => Some("image/x-portable-graymap"),
        "pbm" => Some("image/x-portable-bitmap"),
        "pam" | "pnm" => Some("image/x-portable-anymap"),
        "icns" => Some("image/x-icns"),
        "dds" => Some("image/vnd-ms.dds"),
        "jxl" => Some("image/jxl"),
        "mp4" | "m4v" => Some("video/mp4"),
        "webm" => Some("video/webm"),
        "ogv" => Some("video/ogg"),
        "swf" => Some("application/x-shockwave-flash"),
        "trp" | "ts" => Some("video/mp2t"),
        "mov" => Some("video/quicktime"),
        "3gp" => Some("video/3gpp"),
        "3g2" => Some("video/3gpp2"),
        "mts" | "m2ts" => Some("video/vnd.dlna.mpeg-tts"),
        "avi" => Some("video/x-msvideo"),
        "mkv" => Some("video/x-matroska"),
        "wmv" => Some("video/x-ms-wmv"),
        "flv" | "f4v" => Some("video/x-flv"),
        "mpg" | "mpeg" => Some("video/mpeg"),
        "vob" => Some("video/x-ms-vob"),
        "pdf" => Some("application/pdf"),
        "doc" => Some("application/msword"),
        "docx" => Some(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        "xls" => Some("application/vnd.ms-excel"),
        "xlsx" => Some(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        "ppt" => Some("application/vnd.ms-powerpoint"),
        "pptx" => Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        "aac" => Some("audio/aac"),
        "aiff" => Some("audio/x-aiff"),
        "amr" => Some("audio/amr"),
        "ape" => Some("audio/ape"),
        "flac" => Some("audio/flac"),
        "m4a" => Some("audio/mp4"),
        "mp3" => Some("audio/mpeg"),
        "wav" => Some("audio/wav"),
        _ => RAW_EXTENSIONS
            .iter()
            .find(|&&raw_ext| raw_ext == ext.as_str())
            .map(|_| "image/x-raw"),
    }
}

fn is_video_ext(ext: &str) -> bool {
    matches!(
        ext,
        "3g2" | "3gp" | "avi" | "f4v" | "flv" | "m2ts" | "m4v" | "mkv" | "mov" | "mp4" | "mpeg"
            | "mpg" | "mts" | "ogv" | "swf" | "trp" | "ts" | "vob" | "webm" | "wmv"
    )
}

fn is_image_ext(ext: &str) -> bool {
    mime_from_extension(Path::new(&format!("sample.{ext}"))).is_some_and(|m| {
        IMAGE_MIME_TYPES.contains(&m)
            || m == "image/jp2"
            || m == "image/vnd.radiance"
            || m == "image/x-mng"
            || m == "image/jxl"
            || m == "image/vnd-ms.dds"
    })
}

/// 解析文件 MIME，扩展名优先修正 tika 误判
pub fn resolve_mime(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if let Some(mime) = mime_from_extension(path) {
        return mime.to_string();
    }

    let tika = tika_magic::from_filepath(path).unwrap_or_default();

    // tika 误判修正
    if tika == "audio/ogg" && ext == "ogv" {
        return "video/ogg".into();
    }
    if tika == "image/x-icon" && is_video_ext(&ext) {
        return mime_from_extension(path).unwrap_or("video/mp4").into();
    }
    // MNG 是动画图像而非视频容器
    if tika == "video/x-mng" || (tika.is_empty() && ext == "mng") {
        return "image/x-mng".into();
    }

    if !tika.is_empty() {
        return tika.to_string();
    }

    if is_video_ext(&ext) {
        return "video/mp4".into();
    }
    if is_image_ext(&ext) {
        return "image/x-portable-anymap".into();
    }

    tika.to_string()
}

/// 判定 MIME 是否应由 image 分支处理
pub fn is_image_mime(mime: &str) -> bool {
    IMAGE_MIME_TYPES.contains(&mime)
        || mime == "image/jp2"
        || mime == "image/vnd.radiance"
        || mime == "image/x-mng"
        || mime == "image/jxl"
        || mime == "image/vnd-ms.dds"
        || mime == "image/x-portable-bitmap"
}

/// 判定 MIME 是否应由 video 分支处理
pub fn is_video_mime(mime: &str) -> bool {
    VIDEO_MIME_TYPES.contains(&mime) && mime != "video/x-mng"
}

/// 判定 MIME 是否应由 pdf 分支处理
pub fn is_pdf_mime(mime: &str) -> bool {
    PDF_MIME_TYPES.contains(&mime)
}

/// 判定 MIME 是否应由 svg 分支处理
pub fn is_svg_mime(mime: &str) -> bool {
    mime == "image/svg+xml"
}

/// 判定扩展名是否为 RAW 格式
pub fn is_raw_ext(ext: &str) -> bool {
    RAW_EXTENSIONS.contains(&ext)
}

/// 判定 MIME 是否应由 audio 分支处理
pub fn is_audio_mime(mime: &str) -> bool {
    AUDIO_MIME_TYPES.contains(&mime)
}

/// 判定扩展名是否为音频格式
pub fn is_audio_ext(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext)
}

/// 判定 MIME 是否应由 office 分支处理
pub fn is_office_mime(mime: &str) -> bool {
    OFFICE_MIME_TYPES.contains(&mime)
}

/// 判定扩展名是否为 Office 文档
pub fn is_office_ext(ext: &str) -> bool {
    OFFICE_EXTENSIONS.contains(&ext)
}
