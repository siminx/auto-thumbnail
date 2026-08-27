//! FFmpeg 日志：默认静默，避免损坏视频解码刷屏；`AUTO_THUMB_FFMPEG_LOG=debug` 恢复详细日志。

use std::sync::Once;

static INIT: Once = Once::new();

/// 空 log callback，彻底丢弃 FFmpeg stderr 输出
#[cfg(feature = "video")]
unsafe extern "C" fn null_log_callback(
    _avcl: *mut std::ffi::c_void,
    _level: std::ffi::c_int,
    _fmt: *const std::ffi::c_char,
    #[cfg(all(target_arch = "x86_64", target_family = "unix"))] _vl: *mut ffmpeg_next::ffi::__va_list_tag,
    #[cfg(not(all(target_arch = "x86_64", target_family = "unix")))] _vl: ffmpeg_next::ffi::va_list,
) {
}

/// 首次解码前调用；须在 `ffmpeg::init()` 之前
#[cfg(feature = "video")]
pub fn init_ffmpeg_logging() {
    INIT.call_once(|| {
        use ffmpeg_next::ffi::{av_log_default_callback, av_log_set_callback};
        use ffmpeg_next::util::log::{self, Level};

        let debug = std::env::var("AUTO_THUMB_FFMPEG_LOG")
            .ok()
            .is_some_and(|v| v.eq_ignore_ascii_case("debug"));

        if debug {
            unsafe {
                av_log_set_callback(Some(av_log_default_callback));
            }
            log::set_level(Level::Debug);
        } else {
            unsafe {
                av_log_set_callback(Some(null_log_callback));
            }
            log::set_level(Level::Quiet);
        }
    });
}

#[cfg(not(feature = "video"))]
pub fn init_ffmpeg_logging() {}
