pub const IMAGE_MIME_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/bmp",
    "image/tiff",
    "image/webp",
    "image/x-tga",
    "image/avif",
    "image/x-icon",
    "image/svg+xml",
    "image/vnd.adobe.photoshop",
    "image/heic",
    "image/heif",
    "image/x-exr",
    "image/x-portable-anymap",
    "image/x-portable-graymap",
    "image/x-portable-pixmap",
    "image/x-pcx",
    "image/x-icns",
    "image/jp2",
    "image/jpx",
    "image/vnd.radiance",
    "image/x-mng",
    "image/jxl",
    "image/vnd-ms.dds",
    "image/x-portable-bitmap",
];

pub const VIDEO_MIME_TYPES: &[&str] = &[
    "video/mp4",
    "video/webm",
    "video/mpeg",
    "video/quicktime",
    "video/theora",
    "video/x-flv",
    "video/x-ms-asf",
    "video/x-msvideo",
    "video/x-ms-wmv",
    "video/x-matroska",
    "application/x-matroska",
    "application/x-shockwave-flash",
    "video/3gpp",
    "video/3gpp2",
    "video/ogg",
    "video/x-m4v",
    "video/x-f4v",
    "video/vnd.dlna.mpeg-tts",
    "video/x-ms-vob",
    "video/mp2t",
    "video/x-mng",
];

pub const PDF_MIME_TYPES: &[&str] = &["application/pdf"];

pub const AUDIO_MIME_TYPES: &[&str] = &[
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/flac",
    "audio/aac",
    "audio/mp4",
    "audio/x-aiff",
    "audio/amr",
    "audio/ape",
];

/// OOXML / ODF 等 ZIP-based Office MIME（OLE 老格式 doc/xls/ppt 由应用层 Shell 兜底）
pub const OFFICE_MIME_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
];

/// RAW 相机格式扩展名
pub const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "arw", "cr2", "cr3", "crw", "dng", "erf", "mrw", "nef", "nrw", "orf", "pef", "raf",
    "raw", "rw2", "sr2", "srw", "x3f",
];

/// ZIP-based Office 文档扩展名（OLE 老格式 doc/xls/ppt 由应用层 Shell 兜底）
pub const OFFICE_EXTENSIONS: &[&str] = &[
    "docx", "xlsx", "pptx", "potx", "odt", "ods", "odp",
];

/// 音频扩展名
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "aac", "aiff", "amr", "ape", "flac", "m4a", "mp3", "ogg", "wav",
];
