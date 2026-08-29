//! Sigma X3F RAW：提取容器内嵌 JPEG 预览（rawler 不支持 Foveon 传感器）。

use std::path::Path;

use anyhow::Result;
use image::DynamicImage;

/// 从 X3F 文件中扫描并解码内嵌 JPEG 预览
pub fn create_thumbnail(path: &Path, max_dim: u32) -> Result<DynamicImage> {
    let data = std::fs::read(path)?;
    for jpeg in extract_jpeg_candidates(&data) {
        if let Ok(img) = image::load_from_memory(&jpeg) {
            return Ok(DynamicImage::from(img).thumbnail(max_dim, max_dim));
        }
    }
    anyhow::bail!("X3F 无有效内嵌 JPEG 预览: {:?}", path)
}

/// 收集 FF D8 … FF D9 片段，按体积降序尝试解码
fn extract_jpeg_candidates(data: &[u8]) -> Vec<Vec<u8>> {
    let mut segments: Vec<Vec<u8>> = Vec::new();
    let mut i = 0;
    while i + 2 < data.len() {
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            if let Some(rel) = data[i + 2..]
                .windows(2)
                .position(|w| w[0] == 0xFF && w[1] == 0xD9)
            {
                let end = i + 2 + rel + 2;
                segments.push(data[i..end].to_vec());
                i = end;
                continue;
            }
        }
        i += 1;
    }
    segments.sort_by_key(|b| std::cmp::Reverse(b.len()));
    segments
}
