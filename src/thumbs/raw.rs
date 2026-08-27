//! RAW 相机格式：rawler 解码 + 默认 develop。

use std::path::Path;

use anyhow::{Context, Result};
use image::DynamicImage;
use rawler::decoders::RawDecodeParams;
use rawler::imgop::develop::RawDevelop;
use rawler::rawsource::RawSource;

use super::x3f;

/// RAW 解码后缩略；x3f 优先尝试内嵌 JPEG 预览
pub fn create_thumbnail(path: &Path, max_dim: u32) -> Result<DynamicImage> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "x3f" {
        if let Ok(img) = x3f::create_thumbnail(path, max_dim) {
            return Ok(img);
        }
    }

    let rawfile = RawSource::new(path).context("打开 RAW 文件失败")?;
    let params = RawDecodeParams::default();
    let rawimage = rawler::decode(&rawfile, &params).context("RAW 解码失败")?;
    let dev = RawDevelop::default();
    let developed = dev
        .develop_intermediate(&rawimage)
        .context("RAW develop 失败")?;
    let dynamic = developed
        .to_dynamic_image()
        .context("RAW 转 DynamicImage 失败")?;
    Ok(dynamic.thumbnail(max_dim, max_dim))
}
