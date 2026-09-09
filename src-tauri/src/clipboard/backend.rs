//! 系统剪贴板读写（arboard，三平台同一 API）：把剪贴板内容读成 `Captured` 快照、把 `Captured` 写回剪贴板。
//!
//! 边界：不监听变化、不模拟按键（那是各平台文件 `clipboard/{windows,...}.rs` 的事），不碰数据库。
//! `arboard::Clipboard` 非 `Send`，本模块全部是同步函数，调用方放在 `spawn_blocking` 或监听线程里用完即丢。

use std::borrow::Cow;
use std::io::Cursor;
use std::thread;
use std::time::Duration;

use arboard::{Clipboard, ImageData};
use image::{ImageFormat, RgbaImage};

use super::{
    Captured, ClipboardKind, MAX_IMAGE_BYTES, MAX_TEXT_BYTES, THUMB_MAX_EDGE, domain_hash,
};
use crate::error::AppError;

/// 剪贴板被其它进程占用时的重试次数与间隔（Excel 等应用在 `WM_CLIPBOARDUPDATE` 到达时可能仍持有剪贴板）
const OCCUPIED_RETRIES: usize = 3;
const OCCUPIED_RETRY_DELAY: Duration = Duration::from_millis(50);

/// 读取当前剪贴板为快照；优先级 文件列表 > 图片 > 文本。
///
/// 返回 `None` 的情形都不算错误：内容为空 / 不是三类之一 / 超过阈值 / 重试后仍被占用，只记 debug 或 warn 日志。
pub fn read_snapshot() -> Option<Captured> {
    match with_retry(read_once) {
        Ok(captured) => captured,
        Err(e) => {
            log::warn!("读取剪贴板失败: {e}");
            None
        }
    }
}

/// 把快照写回系统剪贴板：文本 → `set().text()`；图片 → PNG 解码为 RGBA8 → `set().image()`；文件 → `set().file_list()`。
pub fn write(captured: &Captured) -> Result<(), AppError> {
    // 解码在打开剪贴板之前做一次，重试时不重复解码；失败与 arboard 错误同归 AppError::Clipboard
    let image = match captured {
        Captured::Image { png, .. } => Some(decode_rgba(png)?),
        _ => None,
    };
    with_retry(|clipboard| match (captured, &image) {
        (Captured::Text(text), _) => clipboard.set().text(text.as_str()),
        (Captured::Image { .. }, Some(rgba)) => clipboard.set().image(ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: Cow::Borrowed(rgba.as_raw()),
        }),
        (Captured::Image { .. }, None) => Err(arboard::Error::ConversionFailure),
        (Captured::Files(paths), _) => clipboard.set().file_list(paths),
    })
}

/// PNG 字节 → RGBA8 像素（写回剪贴板用）
fn decode_rgba(png: &[u8]) -> Result<RgbaImage, AppError> {
    image::load_from_memory_with_format(png, ImageFormat::Png)
        .map(|img| img.into_rgba8())
        .map_err(clipboard_error)
}

/// 打开剪贴板并执行 `op`；`ClipboardOccupied` 时按 [`OCCUPIED_RETRIES`] 重试，其它错误直接返回。
fn with_retry<T>(
    mut op: impl FnMut(&mut Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, AppError> {
    let mut attempt = 0;
    loop {
        let result = Clipboard::new().and_then(|mut clipboard| op(&mut clipboard));
        match result {
            Ok(value) => return Ok(value),
            Err(arboard::Error::ClipboardOccupied) if attempt < OCCUPIED_RETRIES => {
                attempt += 1;
                log::debug!("剪贴板被占用，第 {attempt} 次重试");
                thread::sleep(OCCUPIED_RETRY_DELAY);
            }
            Err(e) => return Err(clipboard_error(e)),
        }
    }
}

/// 单次读取：某格式不可用就试下一种；只有「被占用」向上抛出触发重试。
fn read_once(clipboard: &mut Clipboard) -> Result<Option<Captured>, arboard::Error> {
    match clipboard.get().file_list() {
        Ok(paths) if !paths.is_empty() => {
            return Ok(Some(Captured::Files(
                paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
            )));
        }
        Ok(_) => {}
        Err(arboard::Error::ClipboardOccupied) => return Err(arboard::Error::ClipboardOccupied),
        Err(e) => log::trace!("剪贴板无文件列表: {e}"),
    }
    match clipboard.get().image() {
        Ok(image) => return Ok(image_snapshot(&image)),
        Err(arboard::Error::ClipboardOccupied) => return Err(arboard::Error::ClipboardOccupied),
        Err(e) => log::trace!("剪贴板无图片: {e}"),
    }
    match clipboard.get().text() {
        Ok(text) => Ok(text_snapshot(text)),
        Err(arboard::Error::ClipboardOccupied) => Err(arboard::Error::ClipboardOccupied),
        Err(e) => {
            log::trace!("剪贴板无文本: {e}");
            Ok(None)
        }
    }
}

/// 文本入库前的门槛：trim 后为空或超过 `MAX_TEXT_BYTES` 不记录（原文保持不 trim）
fn text_snapshot(text: String) -> Option<Captured> {
    if text.trim().is_empty() {
        log::debug!("剪贴板文本为空白，跳过");
        return None;
    }
    if text.len() > MAX_TEXT_BYTES {
        log::debug!("剪贴板文本 {} 字节超过上限，跳过", text.len());
        return None;
    }
    Some(Captured::Text(text))
}

/// 图片：校验尺寸与字节数 → 像素哈希 → 编码 PNG 原图 + 缩略图
fn image_snapshot(image: &ImageData<'_>) -> Option<Captured> {
    let (Ok(width), Ok(height)) = (u32::try_from(image.width), u32::try_from(image.height)) else {
        log::warn!("剪贴板图片尺寸异常，跳过");
        return None;
    };
    if image_too_large(width, height) {
        log::debug!("剪贴板图片 {width}×{height} 超过上限，跳过");
        return None;
    }
    let Some(rgba) = RgbaImage::from_raw(width, height, image.bytes.to_vec()) else {
        log::warn!("剪贴板图片字节数与尺寸不匹配，跳过");
        return None;
    };
    let rgba_hash = pixel_hash(width, height, rgba.as_raw());
    let (thumb_w, thumb_h) = thumb_size(width, height);
    let thumb = image::imageops::thumbnail(&rgba, thumb_w, thumb_h);
    match (encode_png(&rgba), encode_png(&thumb)) {
        (Ok(png), Ok(thumb_png)) => Some(Captured::Image {
            png,
            thumb_png,
            width,
            height,
            rgba_hash,
        }),
        (Err(e), _) | (_, Err(e)) => {
            log::warn!("剪贴板图片编码 PNG 失败: {e}");
            None
        }
    }
}

/// 解码后 RGBA8 字节数（宽 × 高 × 4）是否超过 `MAX_IMAGE_BYTES`；乘法溢出也视为过大
fn image_too_large(width: u32, height: u32) -> bool {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|px| px.checked_mul(4))
        .is_none_or(|bytes| bytes > MAX_IMAGE_BYTES)
}

/// 像素哈希：blake3("image\0", 宽 LE, 高 LE, RGBA8)。基于像素而非 PNG 字节，写回后格式往返仍命中同一条
fn pixel_hash(width: u32, height: u32, rgba: &[u8]) -> String {
    domain_hash(
        ClipboardKind::Image,
        &[&width.to_le_bytes(), &height.to_le_bytes(), rgba],
    )
}

/// 缩略图尺寸：最长边缩到 `THUMB_MAX_EDGE`、等比、不放大、至少 1 像素
fn thumb_size(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= THUMB_MAX_EDGE || longest == 0 {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(THUMB_MAX_EDGE) / f64::from(longest);
    let scaled = |edge: u32| ((f64::from(edge) * scale).round() as u32).max(1);
    (scaled(width), scaled(height))
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut buf = Cursor::new(Vec::new());
    image.write_to(&mut buf, ImageFormat::Png)?;
    Ok(buf.into_inner())
}

/// arboard / image 的错误都归为 `AppError::Clipboard`，文案取底层 `Display`
fn clipboard_error(e: impl std::fmt::Display) -> AppError {
    AppError::Clipboard(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_threshold_skips_blank_and_oversized() {
        assert_eq!(text_snapshot("   \n\t".into()), None);
        assert_eq!(
            text_snapshot("  a  ".into()),
            Some(Captured::Text("  a  ".into())),
            "入库保留原文，不 trim"
        );
        assert!(text_snapshot("x".repeat(MAX_TEXT_BYTES)).is_some());
        assert_eq!(text_snapshot("x".repeat(MAX_TEXT_BYTES + 1)), None);
    }

    #[test]
    fn image_size_threshold_is_rgba_bytes() {
        // 20 MiB / 4 = 5_242_880 像素
        assert!(!image_too_large(2290, 2289));
        assert!(image_too_large(2300, 2300));
        assert!(image_too_large(u32::MAX, u32::MAX), "溢出视为过大");
        assert!(!image_too_large(0, 0));
    }

    #[test]
    fn thumb_size_fits_longest_edge_without_upscaling() {
        assert_eq!(thumb_size(100, 50), (100, 50));
        assert_eq!(thumb_size(256, 256), (256, 256));
        assert_eq!(thumb_size(512, 256), (256, 128));
        assert_eq!(thumb_size(256, 1024), (64, 256));
        assert_eq!(thumb_size(10_000, 1), (256, 1), "短边至少 1 像素");
        assert_eq!(thumb_size(0, 0), (1, 1));
    }

    #[test]
    fn pixel_hash_depends_on_dimensions_and_pixels() {
        let px = vec![0u8; 16];
        let a = pixel_hash(2, 2, &px);
        assert_eq!(a, pixel_hash(2, 2, &px));
        assert_ne!(a, pixel_hash(4, 1, &px), "同字节不同尺寸不是同一张图");
        let mut other = px.clone();
        other[0] = 1;
        assert_ne!(a, pixel_hash(2, 2, &other));
        assert_eq!(a.len(), 64, "blake3 hex 长度");
    }

    #[test]
    fn image_snapshot_encodes_png_and_thumbnail() {
        let (w, h) = (300u32, 100u32);
        let bytes = vec![200u8; (w * h * 4) as usize];
        let data = ImageData {
            width: w as usize,
            height: h as usize,
            bytes: Cow::Owned(bytes),
        };
        let Some(Captured::Image {
            png,
            thumb_png,
            width,
            height,
            rgba_hash,
        }) = image_snapshot(&data)
        else {
            panic!("应得到图片快照");
        };
        assert_eq!((width, height), (w, h));
        assert_eq!(
            rgba_hash,
            pixel_hash(w, h, &vec![200u8; (w * h * 4) as usize])
        );
        let decoded =
            image::load_from_memory_with_format(&png, ImageFormat::Png).expect("原图应是合法 PNG");
        assert_eq!((decoded.width(), decoded.height()), (w, h));
        let thumb = image::load_from_memory_with_format(&thumb_png, ImageFormat::Png)
            .expect("缩略图应是合法 PNG");
        assert_eq!((thumb.width(), thumb.height()), (256, 85));
    }

    #[test]
    fn image_snapshot_rejects_mismatched_bytes() {
        let data = ImageData {
            width: 2,
            height: 2,
            bytes: Cow::Owned(vec![0u8; 3]),
        };
        assert_eq!(image_snapshot(&data), None);
    }
}
