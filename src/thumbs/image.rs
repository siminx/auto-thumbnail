use std::path::Path;

use crate::decode;

pub(crate) fn create_thumbnail<P>(path: P, width: u32, height: u32) -> anyhow::Result<::image::DynamicImage>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let max_dim = width.max(height);
    let img = decode::decode_and_thumbnail(path, max_dim)?;
    Ok(img.thumbnail(width, height))
}
