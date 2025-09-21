use anyhow::Context;
use image::{DynamicImage, ImageBuffer};
use std::path::Path;
use video_rs::decode::Decoder;

pub(crate) fn create_thumbnail<P>(path: P, width: u32, height: u32) -> anyhow::Result<DynamicImage>
where
    P: AsRef<Path>,
{
    let mut decoder = Decoder::new(path.as_ref())?;

    let (w, h) = decoder.size();
    let frame = decoder.decode()?.1;

    let buf = frame
        .as_slice()
        .context("Failed to turn frame into slice.")?;

    let img =
        ImageBuffer::from_raw(w, h, buf.to_vec()).context("Failed to construct image buffer.")?;

    let resized = DynamicImage::ImageRgb8(img).thumbnail(width, height);

    Ok(resized)
}
