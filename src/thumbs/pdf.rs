use std::path::Path;

use image::DynamicImage;
use pdfium_render::prelude::{PdfRenderConfig, Pdfium, PdfiumError};

pub(crate) fn create_thumbnail<P>(path: P, width: u32, height: u32) -> anyhow::Result<DynamicImage>
where
    P: AsRef<Path>,
{
    let pdfium = get_pdfium()?;
    let document = pdfium.load_pdf_from_file(&path, None)?;
    let render_config = PdfRenderConfig::new();
    let first_page = document.pages().first()?;
    let img = first_page.render_with_config(&render_config)?.as_image()?;

    let resized = img.thumbnail(width, height);

    Ok(resized)
}

fn get_pdfium() -> Result<Pdfium, PdfiumError> {
    match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")) {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        // 进程内已成功绑定过 pdfium，复用全局单例，避免重复绑定报错
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium::default()),
        Err(_) => Pdfium::bind_to_system_library().map(Pdfium::new),
    }
}
