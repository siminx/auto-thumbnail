mod thumbs;

use std::{fs::File, path::Path, str::FromStr};

use ::image::{DynamicImage, ImageFormat, codecs::jpeg::JpegEncoder};
use anyhow::Context;
use strum_macros::{AsRefStr, Display, EnumString};

#[derive(thiserror::Error, Debug)]
pub enum ThumbnailError<'a> {
    #[error("IOError")]
    IOError(#[from] std::io::Error),
    #[error("ImageError")]
    ImageError(#[from] ::image::ImageError),
    #[error("PngError")]
    PngError(#[from] oxipng::PngError),
    #[error("AnyError")]
    AnyError(#[from] anyhow::Error),
    #[error("Unsupported MIME type:`{0}`")]
    UnsupportedError(&'a str),
}

#[derive(Debug, Copy, Clone, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Encoding {
    Jpeg,
    Png,
    Webp,
}

/// Represents fixed sizes of a thumbnail
#[derive(Clone, Copy, Debug)]
pub enum ThumbnailSize {
    Icon,
    Small,
    Medium,
    Large,
    Larger,
    Custom((u32, u32)),
}

impl ThumbnailSize {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ThumbnailSize::Icon => (64, 64),
            ThumbnailSize::Small => (128, 128),
            ThumbnailSize::Medium => (256, 256),
            ThumbnailSize::Large => (512, 512),
            ThumbnailSize::Larger => (1024, 1024),
            ThumbnailSize::Custom(size) => *size,
        }
    }
}

pub struct Thumbnailer {
    /// The maximum output width.
    pub width: u32,
    /// The maximum output height.
    pub height: u32,
    /// Encode the image with the given quality.
    /// Only support Jpeg and Webp.
    /// The image quality must be between 1 and 100 inclusive for minimal and maximal quality respectively.
    pub quality: u8,
}

impl Default for Thumbnailer {
    fn default() -> Self {
        Self::new(ThumbnailSize::Medium, 90)
    }
}

impl Thumbnailer {
    pub fn new(size: ThumbnailSize, quality: u8) -> Self {
        let (width, height) = size.dimensions();
        Self {
            width,
            height,
            quality,
        }
    }

    /// create thumbnail image.
    /// path: source file path.
    /// output: thumbnail image path.
    pub fn create_thumbnail<P>(
        &'_ self,
        path: P,
        output: P,
    ) -> anyhow::Result<(), ThumbnailError<'_>>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let mime = tika_magic::from_filepath(path).context("Failed to find MIME type.")?;
        // println!("mime: {}", mime);

        let encoding = output
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_ascii_uppercase().to_str().map(str::to_string))
            .and_then(|ext| Encoding::from_str(&ext).ok())
            .unwrap_or_else(|| {
                log::debug!("Defaulting encoding to Jpeg");
                Encoding::Jpeg
            });

        #[cfg(feature = "image")]
        if mime.starts_with("image/") {
            use crate::thumbs::image;

            let img = image::create_thumbnail(path, self.width, self.height)?;
            self.encod_and_save(img, encoding, output)?;
            return Ok(());
        }

        #[cfg(feature = "pdf")]
        if mime.eq("application/pdf") {
            use crate::thumbs::pdf;

            let img = pdf::create_thumbnail(path, self.width, self.height)?;
            self.encod_and_save(img, encoding, output)?;
            return Ok(());
        }

        #[cfg(feature = "video")]
        if mime.starts_with("video/") {
            use crate::thumbs::video;

            let img = video::create_thumbnail(path, self.width, self.height)?;
            self.encod_and_save(img, encoding, output)?;
            return Ok(());
        }

        Err(ThumbnailError::UnsupportedError(mime))
    }

    fn encod_and_save<P>(
        &'_ self,
        img: DynamicImage,
        encoding: Encoding,
        output: P,
    ) -> anyhow::Result<(), ThumbnailError<'_>>
    where
        P: AsRef<Path>,
    {
        match encoding {
            Encoding::Jpeg => {
                let output = File::create(output)?;
                let encoder = JpegEncoder::new_with_quality(output, self.quality);
                img.write_with_encoder(encoder)?;
            }
            Encoding::Png => {
                img.save_with_format(&output, ImageFormat::Png)?;

                oxipng::optimize(
                    &oxipng::InFile::Path(output.as_ref().to_path_buf()),
                    &oxipng::OutFile::from_path(output.as_ref().to_path_buf()),
                    &oxipng::Options::max_compression(),
                )?;
            }
            Encoding::Webp => {
                let encoder = webp::Encoder::from_image(&img)
                    .ok()
                    .context("Unimplemented")?;
                let memory = encoder.encode(self.quality.into());
                std::fs::write(output, &*memory)?;
            }
        };

        Ok(())
    }
}
