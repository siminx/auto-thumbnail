//! 音频内嵌封面提取：无封面时返回 None。

use std::path::Path;

use image::DynamicImage;
use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;

/// 优先 CoverFront，其次 Other 类型内嵌图
pub fn extract_cover(path: &Path, max_dim: u32) -> Option<DynamicImage> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag()?;
    let picture = tag.pictures().iter().find(|p| {
        p.pic_type() == PictureType::CoverFront || p.pic_type() == PictureType::Other
    })?;
    let data = picture.data();
    let img = image::load_from_memory(data).ok()?;
    Some(DynamicImage::from(img).thumbnail(max_dim, max_dim))
}
