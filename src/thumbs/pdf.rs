//! PDF 首页渲染。pdfium-render 的 BINDINGS 只能 set 一次，且 FFI 非线程安全，
//! 必须单次绑定 + 全局互斥，避免并发 bind 断言 panic / 堆损坏。

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::Mutex;

use image::DynamicImage;
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};

/// pdfium 绑定与渲染共用一把锁：绑定只能成功一次，渲染也不能并发
static PDFIUM_LOCK: Mutex<PdfiumBindState> = Mutex::new(PdfiumBindState::Unbound);

enum PdfiumBindState {
    Unbound,
    Ready,
    Failed,
}

pub(crate) fn create_thumbnail<P>(path: P, width: u32, height: u32) -> anyhow::Result<DynamicImage>
where
    P: AsRef<Path>,
{
    let path = path.as_ref().to_path_buf();
    let mut state = PDFIUM_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if matches!(*state, PdfiumBindState::Unbound) {
        *state = if bind_once() {
            PdfiumBindState::Ready
        } else {
            PdfiumBindState::Failed
        };
    }
    if matches!(*state, PdfiumBindState::Failed) {
        anyhow::bail!("pdfium 库绑定失败");
    }

    // 渲染也在锁内：pdfium FFI 非线程安全
    catch_unwind(AssertUnwindSafe(|| render_first_page(&path, width, height)))
        .map_err(|_| anyhow::anyhow!("pdfium 渲染 panic"))?
}

/// 进程内只尝试绑定一次；已初始化时 BINDINGS.set 会断言，需 catch 后复用 default
fn bind_once() -> bool {
    let bind_result = catch_unwind(|| {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
    });
    match bind_result {
        Ok(Ok(_)) => true,
        Ok(Err(_)) => false,
        // 断言失败 = 全局 BINDINGS 已 set，可走 Pdfium::default()
        Err(_) => catch_unwind(|| {
            let _ = Pdfium::default();
        })
        .is_ok(),
    }
}

fn render_first_page(path: &Path, width: u32, height: u32) -> anyhow::Result<DynamicImage> {
    let pdfium = Pdfium::default();
    let document = pdfium.load_pdf_from_file(path, None)?;
    let render_config = PdfRenderConfig::new();
    let first_page = document.pages().first()?;
    let img = first_page.render_with_config(&render_config)?.as_image()?;
    Ok(img.thumbnail(width, height))
}
