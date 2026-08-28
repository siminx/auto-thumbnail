//! 设计源文件内嵌预览提取：PSD IRB / AI(PDF|EPS) / ZIP 包预览 / 通用 JPEG 扫描。
//!
//! 仅抽文件里已经存好的预览图，不做宿主级渲染。失败返回 None，交应用层 Shell 兜底。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use image::DynamicImage;

/// 25 种源文件扩展名
pub const SOURCE_EXTENSIONS: &[&str] = &[
    "aep", "af", "afdesign", "afphoto", "afpub", "ai", "c4d", "cdr", "clip", "dwg", "graffle",
    "idml", "indd", "indt", "mindnode", "principle", "psb", "psd", "psdt", "pxd", "sketch", "skp",
    "skt", "xd", "xmind",
];

pub fn is_source_ext(ext: &str) -> bool {
    SOURCE_EXTENSIONS.contains(&ext)
}

/// 仅扫描前 16MB，避免对几百 MB 的 PSD/INDD 做全文件 JPEG 扫描
const SCAN_LIMIT: usize = 16 * 1024 * 1024;

/// 主入口：按扩展名分发，提取内嵌预览并缩放到 max_dim
pub fn create_thumbnail(path: &Path, max_dim: u32) -> Option<DynamicImage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let img = match ext.as_str() {
        "psd" | "psb" | "psdt" => extract_psd_preview(path, max_dim),
        "ai" => extract_ai_preview(path, max_dim),
        "sketch" | "xd" | "xmind" | "graffle" | "pxd" | "idml" => extract_zip_preview(path),
        _ => extract_embedded_jpeg(path),
    }?;

    // 源文件预览常为深色/浅色大屏，不能套视频黑帧阈值
    if img.width() == 0 || img.height() == 0 {
        return None;
    }
    Some(if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img
    })
}

fn read_limited(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(SCAN_LIMIT);
    let n = (&mut file)
        .take(SCAN_LIMIT as u64)
        .read_to_end(&mut buf)
        .ok()?;
    buf.truncate(n);
    Some(buf)
}

/// PSD/PSB/PSDT：列表档优先 IRB，避免 composite 全文件读与 PS 保存争用
const PSD_COMPOSITE_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// PSD/PSB/PSDT：优先 composite image，失败再读 IRB 1036/1033 内嵌 JPEG。
fn extract_psd_preview(path: &Path, max_dim: u32) -> Option<DynamicImage> {
    // Icon 档（512px）只用 IRB 局部读，不整文件读入
    if max_dim <= 512 {
        return extract_psd_irb_jpeg(path);
    }
    let size = std::fs::metadata(path).ok()?.len();
    if size <= PSD_COMPOSITE_MAX_BYTES {
        if let Some(img) = crate::thumbs::psd_composite::extract_psd_composite(path, max_dim) {
            return Some(img);
        }
    }
    extract_psd_irb_jpeg(path)
}

/// 按 Image Resource 规范 seek 遍历，跳过巨型 XMP(1060)，只读 1036/1033 JPEG。
pub fn extract_psd_irb_jpeg_from<R: Read + Seek>(reader: &mut R) -> Option<DynamicImage> {
    let mut header = [0u8; 26];
    reader.read_exact(&mut header).ok()?;
    if &header[0..4] != b"8BPS" {
        return None;
    }

    let cm_len = read_u32_from(reader)? as u64;
    reader.seek(SeekFrom::Current(cm_len as i64)).ok()?;
    let ir_len = read_u32_from(reader)? as u64;
    let ir_end = reader.stream_position().ok()?.checked_add(ir_len)?;

    while reader.stream_position().ok()? + 12 <= ir_end {
        let mut sig = [0u8; 4];
        reader.read_exact(&mut sig).ok()?;
        if &sig != b"8BIM" && &sig != b"MeSa" {
            break;
        }
        let mut id_buf = [0u8; 2];
        reader.read_exact(&mut id_buf).ok()?;
        let id = u16::from_be_bytes(id_buf);

        let mut nlen = [0u8; 1];
        reader.read_exact(&mut nlen).ok()?;
        let name_len = nlen[0] as i64;
        if name_len > 0 {
            reader.seek(SeekFrom::Current(name_len)).ok()?;
        }
        if (name_len + 1) % 2 == 1 {
            reader.seek(SeekFrom::Current(1)).ok()?;
        }

        let size = read_u32_from(reader)? as u64;
        if id == 0x040C || id == 0x0409 {
            let read_len = size.min(4 * 1024 * 1024) as usize;
            let mut payload = vec![0u8; read_len];
            reader.read_exact(&mut payload).ok()?;
            if size > read_len as u64 {
                reader
                    .seek(SeekFrom::Current((size - read_len as u64) as i64))
                    .ok()?;
            }
            if size % 2 == 1 {
                reader.seek(SeekFrom::Current(1)).ok()?;
            }
            if let Some(img) = decode_photoshop_thumbnail(&payload) {
                return Some(img);
            }
        } else {
            let skip = size + if size % 2 == 1 { 1 } else { 0 };
            reader.seek(SeekFrom::Current(skip as i64)).ok()?;
        }
    }
    None
}

/// 按 Image Resource 规范 seek 遍历，跳过巨型 XMP(1060)，只读 1036/1033 JPEG。
fn extract_psd_irb_jpeg(path: &Path) -> Option<DynamicImage> {
    let mut file = File::open(path).ok()?;
    extract_psd_irb_jpeg_from(&mut file)
}

fn read_u32_from<R: Read>(reader: &mut R) -> Option<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).ok()?;
    Some(u32::from_be_bytes(buf))
}

/// 资源 1036/1033：format=1 时 28 字节头后为 JPEG
fn decode_photoshop_thumbnail(payload: &[u8]) -> Option<DynamicImage> {
    if payload.len() > 28 {
        let format = u32::from_be_bytes(payload[0..4].try_into().ok()?);
        if format == 1 {
            if let Ok(img) = image::load_from_memory(&payload[28..]) {
                return Some(DynamicImage::from(img));
            }
        }
    }
    if let Some(jpeg) = extract_jpeg_in_window(payload) {
        return image::load_from_memory(&jpeg).ok().map(DynamicImage::from);
    }
    None
}

/// AI：现代 .ai 多为 PDF 容器，复用 pdfium；失败再扫内嵌 JPEG；老式 EPS 抽 TIFF
fn extract_ai_preview(path: &Path, max_dim: u32) -> Option<DynamicImage> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 20];
    let n = file.read(&mut head).ok()?;
    let head = &head[..n];

    if head.starts_with(b"%PDF") {
        #[cfg(feature = "pdf")]
        {
            if let Ok(img) = crate::thumbs::pdf::create_thumbnail(path, max_dim, max_dim) {
                if img.width() > 0 && img.height() > 0 {
                    return Some(img);
                }
            }
        }
        return extract_embedded_jpeg(path);
    }

    if head.starts_with(b"\xC5\xD0\xD3\xC6") {
        return extract_eps_tiff_preview(path);
    }

    extract_embedded_jpeg(path)
}

/// EPS 二进制：读头中的 TIFF 偏移/长度，取出 TIFF 解码
fn extract_eps_tiff_preview(path: &Path) -> Option<DynamicImage> {
    let data = read_limited(path)?;
    if data.len() < 20 {
        return None;
    }
    let tiff_off = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
    let tiff_len = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
    if tiff_off == 0 || tiff_len == 0 || tiff_off + tiff_len > data.len() {
        return None;
    }
    let tiff = &data[tiff_off..tiff_off + tiff_len];
    image::load_from_memory(tiff).ok().map(DynamicImage::from)
}

/// ZIP 类源文件：遍历包内条目，命中常见预览路径则解码
fn extract_zip_preview(path: &Path) -> Option<DynamicImage> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    const PREFERRED: &[&str] = &[
        "previews/preview.png",
        "preview.png",
        "preview.jpg",
        "preview.jpeg",
        "Thumbnails/thumbnail.png",
        "thumbnail.png",
        "QuickLook/Preview.png",
        "QuickLook/Thumbnail.png",
    ];

    for name in PREFERRED {
        if let Ok(mut entry) = archive.by_name(name) {
            if let Some(img) = decode_zip_entry(&mut entry) {
                return Some(img);
            }
        }
    }

    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else {
            continue;
        };
        let name = entry.name().to_ascii_lowercase();
        let is_image = name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg");
        if is_image && (name.contains("preview") || name.contains("thumbnail")) {
            if let Some(img) = decode_zip_entry(&mut entry) {
                return Some(img);
            }
        }
    }
    None
}

fn decode_zip_entry<R: Read>(entry: &mut zip::read::ZipFile<'_, R>) -> Option<DynamicImage> {
    let mut buf = Vec::new();
    if entry.read_to_end(&mut buf).is_err() {
        return None;
    }
    image::load_from_memory(&buf).ok().map(DynamicImage::from)
}

/// 通用兜底：在前 16MB 内找最大的 JPEG 片段解码
fn extract_embedded_jpeg(path: &Path) -> Option<DynamicImage> {
    let data = read_limited(path)?;
    let mut best: Option<Vec<u8>> = None;
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            if let Some(end) = find_jpeg_eoi(&data[i..]) {
                let seg = &data[i..i + end];
                if seg.len() > 8 * 1024 {
                    if best.as_ref().is_none_or(|b| b.len() < seg.len()) {
                        best = Some(seg.to_vec());
                    }
                }
                i += end;
                continue;
            }
        }
        i += 1;
    }
    let jpeg = best?;
    image::load_from_memory(&jpeg).ok().map(DynamicImage::from)
}

/// 在窗口内找 JPEG SOI(FF D8)..EOI(FF D9)，返回结束偏移
fn extract_jpeg_in_window(data: &[u8]) -> Option<Vec<u8>> {
    let start = find_subslice(data, b"\xFF\xD8")?;
    let end = find_jpeg_eoi(&data[start..])?;
    Some(data[start..start + end].to_vec())
}

fn find_jpeg_eoi(data: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            return Some((i + 2).min(data.len()));
        }
        i += 1;
    }
    None
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn source_ext_check() {
        assert!(is_source_ext("psd"));
        assert!(is_source_ext("ai"));
        assert!(is_source_ext("sketch"));
        assert!(is_source_ext("principle"));
        assert!(!is_source_ext("heic"));
    }

    /// 真实样本：001.ai / sample.ai（PDF 容器，需 pdf 特征）
    #[test]
    fn ai_samples_pdf() {
        let samples = [
            r"D:\xsmspace\data\1\file\源文件\001.ai",
            r"D:\xsmspace\data\2\file\源文件\sample.ai",
            r"D:\Pixcall\设计素材\自定义下载-大屏\a-9.ai",
        ];
        let mut ok = 0;
        let mut total = 0;
        for s in samples {
            let path = Path::new(s);
            if !path.exists() {
                continue;
            }
            total += 1;
            if create_thumbnail(path, 512).is_some() {
                ok += 1;
            } else {
                eprintln!("AI 解码失败: {s}");
            }
        }
        if total == 0 {
            return;
        }
        assert!(ok > 0, "至少一个 AI 样本应能提取预览");
    }

    /// graffle 样本（ZIP 或失败）
    #[test]
    fn graffle_sample() {
        let path = Path::new(r"D:\xsmspace\data\1\file\源文件\User_journey.graffle");
        if !path.exists() {
            return;
        }
        // 不强制要求成功，但不应 panic
        let _ = create_thumbnail(path, 512);
    }

    /// 用户报告漏抽的真实样本：文件存在则必须抽出非空预览
    #[test]
    fn reported_missing_source_samples() {
        let samples = [
            r"D:\Pixcall\设计素材\自定义下载-大屏\可视化大屏模版20个大屏合集-Psd  A20\4\三维可视化大屏.ai",
            r"D:\Pixcall\设计素材\自定义下载-大屏\37款大屏样机\可视化大屏样机\07.psd",
            r"D:\Pixcall\设计素材\自定义下载-大屏\可视化大屏模版20个大屏合集-Psd  A20\19\个别卡片源文件01.psb",
            r"D:\Pixcall\设计素材\电力海报\06-海报.psd",
            r"D:\Pixcall\设计素材\电力海报\0865 电力展板.psd",
            r"D:\Pixcall\设计素材\自定义下载-大屏\001-20240314\3.psd",
        ];
        for s in samples {
            let path = Path::new(s);
            if !path.exists() {
                eprintln!("跳过不存在: {s}");
                continue;
            }
            let img = create_thumbnail(path, 512);
            assert!(
                img.as_ref().is_some_and(|i| i.width() > 0 && i.height() > 0),
                "应能提取内容预览: {s}"
            );
        }
    }

    /// 最小 PSD fixture：手写含 Image Resource 1036（JPEG 缩略图）的 PSD，验证提取链路
    #[test]
    fn psd_fixture_irb_jpeg() {
        use image::{DynamicImage, Rgba, RgbaImage};
        use std::io::Cursor;

        // 4x4 红色像素 JPEG
        let img = RgbaImage::from_pixel(4, 4, Rgba([200, 30, 30, 255]));
        let mut jpeg = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .expect("encode jpeg");

        // 缩略图资源数据：28 字节头 + JPEG
        let mut thumb = Vec::new();
        thumb.extend_from_slice(&1u32.to_be_bytes()); // format
        thumb.extend_from_slice(&4u32.to_be_bytes()); // width
        thumb.extend_from_slice(&4u32.to_be_bytes()); // height
        thumb.extend_from_slice(&4u32.to_be_bytes()); // widthbytes
        thumb.extend_from_slice(&4u32.to_be_bytes()); // heightbytes
        thumb.extend_from_slice(&0u32.to_be_bytes()); // depends
        thumb.extend_from_slice(&(jpeg.len() as u32).to_be_bytes()); // size
        thumb.extend_from_slice(&0u16.to_be_bytes()); // compressed
        thumb.extend_from_slice(&jpeg);

        // Image Resource 块：8BIM + id(0x040c) + pascal name(0)+pad + data_len + data
        let mut resources = Vec::new();
        resources.extend_from_slice(b"8BIM");
        resources.extend_from_slice(&[0x04, 0x0c]);
        resources.push(0); // pascal name 长度 0
        resources.push(0); // pad 到偶数
        resources.extend_from_slice(&(thumb.len() as u32).to_be_bytes());
        resources.extend_from_slice(&thumb);

        // PSD 头 + 颜色模式数据 + 图像资源段
        let mut buf = Vec::new();
        buf.extend_from_slice(b"8BPS");
        buf.extend_from_slice(&1u16.to_be_bytes()); // version
        buf.extend_from_slice(&[0u8; 6]); // reserved
        buf.extend_from_slice(&1u16.to_be_bytes()); // channels
        buf.extend_from_slice(&4u32.to_be_bytes()); // height
        buf.extend_from_slice(&4u32.to_be_bytes()); // width
        buf.extend_from_slice(&8u16.to_be_bytes()); // depth
        buf.extend_from_slice(&0u16.to_be_bytes()); // color mode
        buf.extend_from_slice(&0u32.to_be_bytes()); // color mode data len
        buf.extend_from_slice(&(resources.len() as u32).to_be_bytes()); // image resources len
        buf.extend_from_slice(&resources);

        let path = std::env::temp_dir().join(format!(
            "cherry_psd_test_{}.psd",
            std::process::id()
        ));
        std::fs::write(&path, &buf).expect("write temp psd");
        let result = create_thumbnail(&path, 512);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_some(), "PSD IRB 1036 应能提取 JPEG 预览");
        let img = result.unwrap();
        assert!(img.width() > 0 && img.height() > 0);
    }

    /// 含 composite Image Data 的 PSD：应优先读出完整拼合图而非 160px IRB
    #[test]
    fn psd_fixture_composite_image() {
        use ag_psd::{read_psd, write_psd};
        use ag_psd::psd::{ColorMode, PixelData, Psd, ReadOptions, WriteOptions};

        let w = 300u32;
        let h = 200u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        for px in data.chunks_exact_mut(4) {
            px[0] = 50;
            px[1] = 120;
            px[2] = 200;
            px[3] = 255;
        }

        let psd = Psd {
            width: w as f64,
            height: h as f64,
            channels: Some(3.0),
            bits_per_channel: Some(8.0),
            color_mode: Some(ColorMode::Rgb),
            canvas: Some(PixelData {
                width: w,
                height: h,
                data,
            }),
            ..Default::default()
        };

        let bytes = write_psd(&psd, &WriteOptions::default());
        let roundtrip = read_psd(&bytes, &ReadOptions::default()).expect("roundtrip psd");
        assert!(roundtrip.canvas.is_some(), "fixture 应含 composite");

        let path = std::env::temp_dir().join(format!(
            "cherry_psd_composite_test_{}.psd",
            std::process::id()
        ));
        std::fs::write(&path, &bytes).expect("write temp psd");
        let result = create_thumbnail(&path, 512);
        let _ = std::fs::remove_file(&path);

        let img = result.expect("composite PSD 应能提取预览");
        assert_eq!(img.width(), 300, "应保留 composite 原始宽度");
        assert_eq!(img.height(), 200);
        assert!(img.width() > 160, "应大于 IRB 160px 上限");
    }

    /// 真实 PSD 样本：至少一个应通过 composite 得到远超 160px 的预览
    #[test]
    fn psd_real_samples_exceed_irb_size() {
        let samples = [
            r"D:\Pixcall\设计素材\自定义下载-大屏\37款大屏样机\可视化大屏样机\07.psd",
            r"D:\Pixcall\设计素材\自定义下载-大屏\可视化大屏模版20个大屏合集-Psd  A20\19\个别卡片源文件01.psb",
            r"D:\Pixcall\设计素材\电力海报\06-海报.psd",
        ];
        let mut checked = 0;
        let mut high_res = 0;
        for s in samples {
            let path = Path::new(s);
            if !path.exists() {
                continue;
            }
            checked += 1;
            let icon = create_thumbnail(path, 512).expect("Icon 档应成功");
            let preview = create_thumbnail(path, 2048).expect("Preview 档应成功");
            let icon_max = icon.width().max(icon.height());
            let preview_max = preview.width().max(preview.height());
            if icon_max > 200 {
                high_res += 1;
            }
            assert!(
                preview_max >= icon_max,
                "Preview 档不应小于 Icon 档: {s}"
            );
            eprintln!(
                "{s}: icon={}x{} (max={icon_max}), preview={}x{} (max={preview_max})",
                icon.width(),
                icon.height(),
                preview.width(),
                preview.height(),
            );
        }
        if checked == 0 {
            eprintln!("跳过：无本地 PSD 样本");
            return;
        }
        assert!(
            high_res >= 1,
            "至少一个含 composite 的大尺寸 PSD 应输出 >200px 预览"
        );
    }

    /// 诊断用户报告提取失败的样本
    #[test]
    fn psd_reported_failures() {
        let samples = [
            r"D:\Pixcall\设计素材\电力海报\06-海报.psd",
            r"D:\Pixcall\设计素材\自定义下载-大屏\50+主视觉\12.psd",
            r"D:\Pixcall\设计素材\自定义下载-大屏\50+主视觉\13.psd",
            r"D:\Pixcall\设计素材\自定义下载-大屏\可视化大屏模版20个大屏合集-Psd  A20\19\个别卡片源文件01.psb",
        ];
        for s in samples {
            let path = Path::new(s);
            if !path.exists() {
                eprintln!("SKIP (missing): {s}");
                continue;
            }
            let composite = crate::thumbs::psd_composite::extract_psd_composite(path, 512);
            let irb = extract_psd_irb_jpeg(path);
            let full = create_thumbnail(path, 512);
            eprintln!(
                "{s}: composite={:?}, irb={:?}, full={:?}",
                composite.as_ref().map(|i| (i.width(), i.height())),
                irb.as_ref().map(|i| (i.width(), i.height())),
                full.as_ref().map(|i| (i.width(), i.height())),
            );
            assert!(full.is_some(), "create_thumbnail 应成功: {s}");
        }
    }
}
