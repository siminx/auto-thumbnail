use image::DynamicImage;
use std::path::Path;

pub(crate) fn create_thumbnail<P>(path: P, width: u32, height: u32) -> anyhow::Result<DynamicImage>
where
    P: AsRef<Path>,
{
    let img = image::open(path)?;
    let resized = img.thumbnail(width, height);

    Ok(resized)
}
