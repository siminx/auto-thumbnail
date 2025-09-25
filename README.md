# auto-thumbnail

## A thumbnailing library.

Converts various file formats into thumbnail image.

Support image, video, PDF.

## Installation

To use `auto-thumbnail` in your Rust project, add it to your `Cargo.toml`.

```toml
[dependencies]
auto-thumbnail = "0.1"
```

## API

Create a thumbnail:

```rust
use auto_thumbnail::Thumbnailer;

let thumbnailer = Thumbnailer::default();
let result = thumbnailer.create_thumbnail("demo/1.webp", "demo/output1.webp");
```

### Quality Control

Set compression quality 1-100, default 90.

### Supported Formats

Image

| extension | MIME type   |
| --------- | ----------- |
| jpg/jpeg  | image/jpeg  |
| png       | image/png   |
| gif       | image/gif   |
| bmp       | image/bmp   |
| tiff      | image/tiff  |
| webp      | image/webp  |
| tga       | image/x-tga |

Video

| extension | MIME type                     |
| --------- | ----------------------------- |
| mp4       | video/mp4                     |
| webm      | video/webm                    |
| vob       | video/mpeg                    |
| mov       | video/quicktime               |
| ogg       | video/theora                  |
| flv       | video/x-flv                   |
| wmv       | video/x-ms-asf                |
| avi       | video/x-msvideo               |
| mkv       | application/x-matroska        |
| swf       | application/x-shockwave-flash |
| 3gp       | video/3gpp                    |
| 3g2       | video/3gpp2                   |

PDF

| extension | MIME type       |
| --------- | --------------- |
| pdf       | application/pdf |

### Output Formats

- **WebP** (.webp) - Modern format, excellent compression
- **JPEG** (.jpeg) - Good compression, lossy
- **PNG** (.png) - Lossless, supports transparency

## Building

Some file types require additional setup and can be disabled via `features` if unneeded.

Video thumbnails depend on `ffmpeg`. See [rust-ffmpeg](https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building)

PDF thumbnails depend on `pdfium`. See [pdfium-render](https://github.com/ajrcarey/pdfium-render?#dynamic-linking)
