//! SVG 矢量图缩略：resvg 栅格化。

use std::path::Path;

use anyhow::{Context, Result};
use image::DynamicImage;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{Options, Tree};

/// 等比缩放至 max_dim 边长内栅格化 SVG
pub fn create_thumbnail(path: &Path, max_dim: u32) -> Result<DynamicImage> {
    let data = std::fs::read(path).with_context(|| format!("读取 SVG 失败: {:?}", path))?;
    let opt = Options::default();
    let tree = Tree::from_data(&data, &opt).context("解析 SVG 失败")?;
    let size_f = tree.size();
    // 大图缩小，小图不放大以免模糊
    let scale = (max_dim as f32 / size_f.width().max(size_f.height())).min(1.0);
    let w = (size_f.width() * scale).ceil() as u32;
    let h = (size_f.height() * scale).ceil() as u32;
    let mut pixmap = Pixmap::new(w, h).context("创建 pixmap 失败")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let img = DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(w, h, pixmap.data().to_vec()).context("构建 RGBA 失败")?,
    );
    Ok(img.thumbnail(max_dim, max_dim))
}
