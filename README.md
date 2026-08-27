# auto-thumbnail

## A thumbnailing library.

Converts various file formats into thumbnail image.

Support image, video, PDF, SVG, RAW, audio cover, and Office embedded thumbnails via optional features.

## Installation

To use `auto-thumbnail` in your Rust project, add it to your `Cargo.toml`.

```toml
[dependencies]
auto-thumbnail = { version = "0.2", features = ["full"] }
```

### Feature flags

| Feature | Enables |
| ------- | ------- |
| `image` | Raster images via enhanced decode path |
| `video` | FFmpeg video first-frame extraction |
| `pdf` | pdfium first-page render |
| `svg` | resvg vector rasterization |
| `raw` | rawler RAW develop (+ X3F embedded JPEG) |
| `audio` | lofty embedded cover art |
| `office` | ZIP embedded thumbnail from OOXML/ODF |
| `full` | All of the above (default) |

## API

Create a thumbnail:

```rust
use auto_thumbnail::Thumbnailer;

let thumbnailer = Thumbnailer::default();
let result = thumbnailer.create_thumbnail("demo/1.webp", "demo/output1.webp");
```

Decode an image without writing a thumbnail (shared by thumbnail generation and downstream callers):

```rust
use auto_thumbnail::{decode_image, decode_and_thumbnail, decode_for_thumbnail};

let img = decode_image("photo.hdr")?;
let thumb = decode_and_thumbnail("logo.ico", 256)?;
let svg = decode_for_thumbnail("icon.svg", 512)?; // routes svg/raw/audio/office
```

Blank-frame detection (video skip / Office placeholder filter):

```rust
use auto_thumbnail::{is_effectively_blank, is_rgba_blank};
```

### Quality Control

Set compression quality 1-100, default 90.

### Supported Formats

Image

| extension | MIME type | Notes |
| --------- | --------- | ----- |
| jpg/jpeg | image/jpeg | |
| png | image/png | |
| gif | image/gif | |
| bmp | image/bmp | |
| tif/tiff | image/tiff | |
| webp | image/webp | |
| tga | image/x-tga | |
| avif | image/avif | 8-bit AVIF via ravif; HDR/10-bit may fail |
| hdr | image/vnd.radiance | `#?RGBE` and `#?RADIANCE`; tone-mapped LDR |
| exr | image/x-exr | OpenEXR; Reinhard tone map to 8-bit RGB |
| ico/cur | image/x-icon | Lenient ICO parser + PNG entry fallback |
| pcx | image/x-pcx | via image-extras |
| icns | image/x-icns | via image-extras |
| dds | image/vnd-ms.dds | via image-extras |
| jp2/jpx/j2k/j2c/jpf/jpc | image/jp2 | openjpeg2-pure-rs; FFmpeg fallback with `video` feature |
| jxl | image/jxl | via jxl-oxide |
| mng | image/x-mng | first frame from embedded PNG |
| ppm/pgm/pbm/pam/pnm | image/x-portable-* | PNM family via image crate |

`decode_image` / image thumbnails use the enhanced decode path (magic sniff, ICO fallback, HDR/EXR tone mapping). Platform Shell/GDI is **not** included—callers should add OS-specific fallbacks for HEIC, HDR AVIF, etc.

Video

| extension | MIME type |
| --------- | --------- |
| mp4 | video/mp4 |
| m4v | video/mp4 |
| webm | video/webm |
| vob | video/x-ms-vob |
| mov | video/quicktime |
| ogv | video/ogg |
| flv | video/x-flv |
| f4v | video/x-flv |
| wmv | video/x-ms-wmv |
| avi | video/x-msvideo |
| mkv | video/x-matroska |
| swf | application/x-shockwave-flash |
| 3gp | video/3gpp |
| 3g2 | video/3gpp2 |
| mpg/mpeg | video/mpeg |
| ts/trp | video/mp2t |
| mts/m2ts | video/vnd.dlna.mpeg-tts |

PDF

| extension | MIME type |
| --------- | --------- |
| pdf | application/pdf |

SVG (`feature = "svg"`)

| extension | MIME type |
| --------- | --------- |
| svg | image/svg+xml |

RAW (`feature = "raw"`)

| extension | Notes |
| --------- | ----- |
| 3fr, arw, cr2, cr3, crw, dng, erf, mrw, nef, nrw, orf, pef, raf, rw2, sr2, srw | rawler develop |
| x3f | embedded JPEG preview first |

Audio cover (`feature = "audio"`)

| extension | MIME type |
| --------- | --------- |
| aac | audio/aac |
| aiff | audio/x-aiff |
| amr | audio/amr |
| ape | audio/ape |
| flac | audio/flac |
| m4a | audio/mp4 |
| mp3 | audio/mpeg |
| ogg | audio/ogg |
| wav | audio/wav |

Office embedded (`feature = "office"`)

| extension | Notes |
| --------- | ----- |
| docx, xlsx, pptx, potx, odt, ods, odp | reads `docProps/thumbnail.*` from ZIP; skips blank placeholders |

Platform Shell/GDI and EMF rasterization are **not** included—callers (e.g. cherry-box) add OS-specific fallbacks.

### Output Formats

- **WebP** (.webp) - Modern format, excellent compression
- **JPEG** (.jpeg) - Good compression, lossy
- **PNG** (.png) - Lossless, supports transparency

## Building

Some file types require additional setup and can be disabled via `features` if unneeded.

Video thumbnails depend on `ffmpeg`. See [rust-ffmpeg](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)

PDF thumbnails depend on `pdfium`. See [pdfium-render](https://github.com/ajrcarey/pdfium-render?#dynamic-linking)
