/**
 * Image processing utilities.
 * project: Lap
 * author:  julyx10
 * date:    2024-08-08
 */
use arboard::Clipboard;
use exif::{In, Reader, Tag};
use fast_image_resize as fir;
use image::{DynamicImage, GenericImageView, ImageReader, RgbImage, Rgba, RgbaImage};
use little_exif::exif_tag::ExifTag;
use little_exif::filetype::FileExtension;
use little_exif::ifd::ExifTagGroup;
use little_exif::metadata::Metadata as LittleExifMetadata;
use once_cell::sync::Lazy;

use rusqlite::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{t_jxl, t_libraw, t_utils};

#[derive(Default)]
pub struct CaptureSettings {
    pub exposure_time: Option<String>,
    pub f_number: Option<String>,
    pub focal_length: Option<String>,
    pub iso_speed: Option<String>,
}

/// Reads the core capture settings with the same EXIF reader used when image
/// edits preserve metadata. Covers JPEGs that little_exif accepts but
/// kamadak-exif cannot fully decode.
pub fn read_capture_settings_with_little_exif(file_path: &str) -> CaptureSettings {
    if !is_jpeg_path(file_path) {
        return CaptureSettings::default();
    }

    panic::catch_unwind(AssertUnwindSafe(|| {
        let metadata = match LittleExifMetadata::new_from_path(Path::new(file_path)) {
            Ok(metadata) => metadata,
            Err(_) => return CaptureSettings::default(),
        };

        CaptureSettings {
            exposure_time: metadata
                .get_tag_by_hex(0x829a, Some(ExifTagGroup::EXIF))
                .next()
                .and_then(little_exif_rational_value)
                .and_then(format_little_exif_shutter_speed),
            f_number: metadata
                .get_tag_by_hex(0x829d, Some(ExifTagGroup::EXIF))
                .next()
                .and_then(little_exif_rational_value)
                .map(|value| format!("f/{value}")),
            focal_length: metadata
                .get_tag_by_hex(0x920a, Some(ExifTagGroup::EXIF))
                .next()
                .and_then(little_exif_rational_value)
                .map(|value| format!("{value} mm")),
            iso_speed: metadata
                .get_tag_by_hex(0x8827, Some(ExifTagGroup::EXIF))
                .next()
                .and_then(little_exif_iso_value)
                .filter(|value| value != "0")
                .or_else(|| {
                    metadata
                        .get_tag_by_hex(0x8833, Some(ExifTagGroup::EXIF))
                        .next()
                        .and_then(little_exif_iso_value)
                        .filter(|value| value != "0")
                }),
        }
    }))
    .unwrap_or_default()
}

fn little_exif_rational_value(tag: &ExifTag) -> Option<f64> {
    let values = match tag {
        ExifTag::ExposureTime(values) | ExifTag::FNumber(values) | ExifTag::FocalLength(values) => {
            values
        }
        _ => return None,
    };
    let value = values.first()?;
    (value.denominator != 0).then(|| value.nominator as f64 / value.denominator as f64)
}

fn little_exif_iso_value(tag: &ExifTag) -> Option<String> {
    match tag {
        ExifTag::ISO(values) => values.first().map(ToString::to_string),
        ExifTag::ISOSpeed(values) => values.first().map(ToString::to_string),
        _ => None,
    }
}

fn format_little_exif_shutter_speed(value: f64) -> Option<String> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    if value >= 1.0 {
        Some(format!("{value} s"))
    } else {
        Some(format!("1/{} s", (1.0 / value).round()))
    }
}

/// Quick probing of image dimensions without loading the entire file
pub fn get_image_dimensions(file_path: &str) -> Result<(u32, u32), String> {
    if t_jxl::is_jxl_path(file_path) {
        return t_jxl::get_jxl_dimensions(file_path);
    }

    if is_ffmpeg_backed_image_path(file_path) {
        let metadata = crate::t_video::get_video_metadata(file_path)?;
        return Ok((metadata.width, metadata.height));
    }

    // Catch potential panics in the third-party imagesize crate
    let result = panic::catch_unwind(|| imagesize::size(file_path));

    match result {
        Ok(Ok(dimensions)) => {
            let width = dimensions.width as u32;
            let height = dimensions.height as u32;

            if crate::t_libraw::is_tiff_path(file_path) {
                if let Ok((raw_width, raw_height)) = crate::t_libraw::get_raw_dimensions(file_path)
                {
                    if raw_width > width || raw_height > height {
                        return Ok((raw_width, raw_height));
                    }
                }
            }

            Ok((width, height))
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => {
            eprintln!("Panic caught while getting dimensions for: {}", file_path);
            Err(
                "Failed to parse image dimensions due to panic (corrupt or invalid file)"
                    .to_string(),
            )
        }
    }
}

fn get_raw_dimensions_from_exif(file_path: &str) -> Result<Option<(u32, u32)>, String> {
    let exif = match read_exif_permissive(file_path) {
        Some(exif) => exif,
        None => return Ok(None),
    };

    let dimension_tag_pairs = [
        (Tag::PixelXDimension, Tag::PixelYDimension),
        (Tag::ImageWidth, Tag::ImageLength),
    ];

    for (width_tag, height_tag) in dimension_tag_pairs {
        let width = exif
            .get_field(width_tag, In::PRIMARY)
            .and_then(|field| field.value.get_uint(0));
        let height = exif
            .get_field(height_tag, In::PRIMARY)
            .and_then(|field| field.value.get_uint(0));

        if let (Some(width), Some(height)) = (width, height) {
            if width > 0 && height > 0 {
                return Ok(Some((width, height)));
            }
        }
    }

    Ok(None)
}

pub fn read_exif_from_bytes_permissive(data: &[u8]) -> Option<exif::Exif> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut cursor = Cursor::new(data);
        if let Some(exif) = read_exif_container_with_recovery(&mut cursor) {
            return Some(exif);
        }
        if let Some(pos) = data.windows(6).position(|w| w == b"Exif\0\0") {
            let exif_start = pos + 6;
            if exif_start < data.len() {
                if let Some(exif) = read_exif_raw_with_recovery(data[exif_start..].to_vec()) {
                    return Some(exif);
                }
            }
        }
        for sig in [b"II\x2a\x00", b"MM\x00\x2a"] {
            if let Some(pos) = data.windows(4).position(|w| w == sig) {
                if let Some(exif) = read_exif_raw_with_recovery(data[pos..].to_vec()) {
                    return Some(exif);
                }
            }
        }
        None
    }))
    .unwrap_or_else(|_| None)
}

fn read_exif_container_with_recovery(cursor: &mut Cursor<&[u8]>) -> Option<exif::Exif> {
    let mut reader = Reader::new();
    reader.continue_on_error(true);
    read_exif_result_with_recovery(reader.read_from_container(cursor))
}

fn read_exif_raw_with_recovery(data: Vec<u8>) -> Option<exif::Exif> {
    let mut reader = Reader::new();
    reader.continue_on_error(true);
    read_exif_result_with_recovery(reader.read_raw(data))
}

fn read_exif_result_with_recovery(result: Result<exif::Exif, exif::Error>) -> Option<exif::Exif> {
    result
        .or_else(|error| error.distill_partial_result(|_| {}))
        .ok()
}

/// A very aggressive binary scanner that looks for the EXIF Orientation tag (0x0112)
/// directly in the byte stream. This is used as a final fallback for non-standard devices.
pub fn scan_orientation_binary(data: &[u8]) -> Option<i32> {
    // Orientation tag is 0x0112. In TIFF, it's a Short (3) with Count 1.
    // Little Endian: 12 01 03 00 01 00 00 00 [Value] 00 00 00
    // Big Endian:    01 12 00 03 00 00 00 01 00 [Value] 00 00

    // Little Endian search
    if let Some(pos) = data
        .windows(12)
        .position(|w| w[0..8] == [0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00])
    {
        let val = data[pos + 8] as i32;
        if (1..=8).contains(&val) {
            return Some(val);
        }
    }

    // Big Endian search
    if let Some(pos) = data
        .windows(12)
        .position(|w| w[0..8] == [0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01])
    {
        let val = data[pos + 9] as i32;
        if (1..=8).contains(&val) {
            return Some(val);
        }
    }

    None
}

pub fn read_exif_permissive(file_path: &str) -> Option<exif::Exif> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        use std::io::{Read, Seek};
        let mut file = File::open(file_path).ok()?;
        let mut header = [0u8; 2];
        file.read_exact(&mut header).ok()?;

        if header != [0xFF, 0xD8] {
            return None;
        }

        loop {
            let mut marker = [0u8; 2];
            if file.read_exact(&mut marker).is_err() {
                break;
            }
            if marker[0] != 0xFF {
                break;
            }
            if marker[1] == 0xD9 || marker[1] == 0xDA {
                break;
            }

            let mut len_bytes = [0u8; 2];
            if file.read_exact(&mut len_bytes).is_err() {
                break;
            }
            let len = u16::from_be_bytes(len_bytes) as usize;
            if len < 2 {
                break;
            }
            let segment_len = len - 2;

            if marker[1] == 0xE1 {
                // APP1
                let mut segment_data = vec![0u8; segment_len];
                if file.read_exact(&mut segment_data).is_err() {
                    break;
                }
                if segment_data.starts_with(b"Exif\0\0") {
                    if let Some(exif) = read_exif_raw_with_recovery(segment_data[6..].to_vec()) {
                        return Some(exif);
                    }
                }
            } else {
                if file
                    .seek(std::io::SeekFrom::Current(segment_len as i64))
                    .is_err()
                {
                    break;
                }
            }
        }
        None
    }))
    .unwrap_or_else(|_| None)
    .or_else(|| {
        let mut file = File::open(file_path).ok()?;
        let mut buffer = vec![0u8; 128 * 1024];
        let n = file.read(&mut buffer).unwrap_or(0);
        read_exif_from_bytes_permissive(&buffer[..n])
    })
}

pub fn get_image_orientation(file_path: &str) -> i32 {
    let data = match File::open(file_path) {
        Ok(mut f) => {
            let mut buf = vec![0u8; 128 * 1024];
            let n = f.read(&mut buf).unwrap_or(0);
            buf.truncate(n);
            buf
        }
        Err(_) => return 1,
    };

    // 1. Try modern logic
    if let Some(exif) = read_exif_from_bytes_permissive(&data) {
        let orient = exif
            .get_field(Tag::Orientation, In::PRIMARY)
            .or_else(|| exif.fields().find(|f| f.tag == Tag::Orientation))
            .and_then(|field| field.value.get_uint(0))
            .map(|value| value as i32);

        if let Some(o) = orient {
            return o;
        }
    }

    // 2. Industry Fallback: Binary Scan
    // This handles K800i and other phones with broken IFD chains
    scan_orientation_binary(&data).unwrap_or(1)
}

fn apply_orientation(img: DynamicImage, orientation: i32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn compute_thumbnail_dimensions(width: u32, height: u32, thumbnail_size: u32) -> (u32, u32) {
    if width == 0 || height == 0 || thumbnail_size == 0 {
        return (1, 1);
    }

    if width <= thumbnail_size && height <= thumbnail_size {
        return (width.max(1), height.max(1));
    }

    let max_edge = width.max(height) as f32;
    let scale = thumbnail_size as f32 / max_edge;
    let dst_w = ((width as f32) * scale).round().max(1.0) as u32;
    let dst_h = ((height as f32) * scale).round().max(1.0) as u32;
    (dst_w, dst_h)
}

fn encode_jpeg_rgb8(rgb: &image::RgbImage) -> Result<Vec<u8>, String> {
    crate::t_jpeg::encode_rgb8(rgb, 85)
        .map_err(|e| format!("Failed to encode JPEG thumbnail: {}", e))
}

fn resize_rgb_image_to_jpeg(rgb: image::RgbImage, thumbnail_size: u32) -> Result<Vec<u8>, String> {
    let (src_w, src_h) = rgb.dimensions();
    let (dst_w, dst_h) = compute_thumbnail_dimensions(src_w, src_h, thumbnail_size);

    if src_w == dst_w && src_h == dst_h {
        return encode_jpeg_rgb8(&rgb);
    }

    let src_image =
        fir::images::Image::from_vec_u8(src_w, src_h, rgb.into_raw(), fir::PixelType::U8x3)
            .map_err(|e| format!("Failed to prepare RGB source image for resize: {}", e))?;
    let mut dst_image = fir::images::Image::new(dst_w, dst_h, fir::PixelType::U8x3);
    let mut resizer = fir::Resizer::new();
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear));

    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| format!("Failed to resize RGB thumbnail: {}", e))?;

    let resized = image::RgbImage::from_raw(dst_w, dst_h, dst_image.into_vec())
        .ok_or_else(|| "Failed to build resized RGB image".to_string())?;
    encode_jpeg_rgb8(&resized)
}

pub fn is_jpeg_path(file_path: &str) -> bool {
    Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "jpe"))
        .unwrap_or(false)
}

fn decode_scaled_jpeg_image(
    file_path: &str,
    _orientation: i32,
    thumbnail_size: u32,
) -> Result<Option<DynamicImage>, String> {
    if !is_jpeg_path(file_path) || thumbnail_size == 0 {
        return Ok(None);
    }

    // Logic: We pass the thumbnail size to libjpeg-turbo, which picks the best 1/8, 1/4, 1/2 scale.
    match crate::t_jpeg::decode_rgb8_scaled(file_path, thumbnail_size, thumbnail_size) {
        Ok((pixels, w, h)) => {
            let img = RgbImage::from_raw(w, h, pixels)
                .ok_or_else(|| "Failed to build RGB image from turbo pixels".to_string())?;
            Ok(Some(DynamicImage::ImageRgb8(img)))
        }
        Err(e) => {
            eprintln!(
                "libjpeg-turbo scaled decode failed for {}: {}",
                file_path, e
            );
            Ok(None) // Fallback to standard decode
        }
    }
}

pub(crate) fn resize_dynamic_image_to_jpeg(
    img: DynamicImage,
    orientation: i32,
    thumbnail_size: u32,
) -> Result<Vec<u8>, String> {
    let adjusted = apply_orientation(img, orientation);

    if !adjusted.color().has_alpha() {
        return resize_rgb_image_to_jpeg(adjusted.to_rgb8(), thumbnail_size);
    }

    let rgba = adjusted.to_rgba8();
    let (src_w, src_h) = rgba.dimensions();
    let (dst_w, dst_h) = compute_thumbnail_dimensions(src_w, src_h, thumbnail_size);

    if src_w == dst_w && src_h == dst_h {
        return encode_jpeg_rgb8(&DynamicImage::ImageRgba8(rgba).to_rgb8());
    }

    let src_image =
        fir::images::Image::from_vec_u8(src_w, src_h, rgba.into_raw(), fir::PixelType::U8x4)
            .map_err(|e| format!("Failed to prepare source image for resize: {}", e))?;
    let mut dst_image = fir::images::Image::new(dst_w, dst_h, fir::PixelType::U8x4);
    let mut resizer = fir::Resizer::new();
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear));

    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| format!("Failed to resize thumbnail: {}", e))?;

    let resized = image::RgbaImage::from_raw(dst_w, dst_h, dst_image.into_vec())
        .ok_or_else(|| "Failed to build resized RGBA image".to_string())?;
    encode_jpeg_rgb8(&DynamicImage::ImageRgba8(resized).to_rgb8())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchThumbnailStats {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
}

pub fn generate_directory_thumbnails(
    dir_path: &str,
    output_dir: &str,
    thumbnail_size: u32,
) -> Result<BatchThumbnailStats, String> {
    let dir_root = Path::new(dir_path);
    let files: Vec<PathBuf> = WalkDir::new(dir_path)
        .into_iter()
        .filter_entry(|e| !crate::t_utils::is_hidden(e))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect();

    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create thumbnail output directory: {}", e))?;

    let processed = files.len();
    let results: Vec<bool> = files
        .iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();
            let Some(file_type) = t_utils::get_file_type(&path_str) else {
                return false;
            };
            if file_type != 1 && file_type != 3 {
                return false;
            }

            let orientation = get_image_orientation(&path_str);
            let thumb = if file_type == 3 {
                get_raw_thumbnail(&path_str, orientation, thumbnail_size)
            } else {
                get_image_thumbnail(&path_str, orientation, thumbnail_size)
            };

            let Ok(Some(data)) = thumb else {
                return false;
            };

            let relative_path = path.strip_prefix(dir_root).ok().unwrap_or(path.as_path());
            let output_path = Path::new(output_dir)
                .join(relative_path)
                .with_extension("jpg");
            if let Some(parent) = output_path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    return false;
                }
            }
            fs::write(output_path, data).is_ok()
        })
        .collect();

    let succeeded = results.iter().filter(|ok| **ok).count();
    Ok(BatchThumbnailStats {
        processed,
        succeeded,
        failed: processed.saturating_sub(succeeded),
    })
}

/// Get a thumbnail from an image file path
pub fn get_image_thumbnail(
    file_path: &str,
    orientation: i32,
    thumbnail_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    if t_jxl::is_jxl_path(file_path) {
        return t_jxl::get_jxl_thumbnail(file_path, thumbnail_size);
    }

    if is_ffmpeg_backed_image_path(file_path) {
        return crate::t_video::get_video_thumbnail_sync(file_path, thumbnail_size, None, None);
    }

    if crate::t_libraw::is_tiff_path(file_path) {
        if let Ok(Some(data)) = crate::t_libraw::get_raw_thumbnail(file_path, thumbnail_size) {
            return Ok(Some(data));
        }
    }

    let result = panic::catch_unwind(|| {
        let img =
            if let Some(img) = decode_scaled_jpeg_image(file_path, orientation, thumbnail_size)? {
                img
            } else {
                let img_reader = ImageReader::open(file_path)
                    .map_err(|e| format!("Failed to open image: {}", e))?;

                match img_reader.decode() {
                    Ok(img) => img,
                    Err(e) => {
                        // Some formats/variants (notably AVIF) may fail to decode via `image` depending on
                        // the underlying codec support. On macOS, fall back to `sips` which supports
                        // more system formats and returns a JPEG directly.
                        #[cfg(target_os = "macos")]
                        if let Ok(Some(data)) = get_thumbnail_with_sips(file_path, thumbnail_size) {
                            return Ok(Some(data));
                        }
                        // On other platforms, fall back to the bundled FFmpeg sidecar when available.
                        // This is already used for HEIC/HEIF on non-macOS and tends to support more
                        // real-world AVIF variants than the pure-Rust decode path.
                        #[cfg(not(target_os = "macos"))]
                        {
                            if let Ok(Some(data)) = crate::t_video::get_video_thumbnail_sync(
                                file_path,
                                thumbnail_size,
                                None,
                                None,
                            ) {
                                return Ok(Some(data));
                            }
                        }
                        return Err(format!("Failed to decode image: {}", e));
                    }
                }
            };
        resize_dynamic_image_to_jpeg(img, orientation, thumbnail_size).map(Some)
    });

    match result {
        Ok(v) => v,
        Err(_) => {
            eprintln!(
                "Panic caught while creating image thumbnail for: {}",
                file_path
            );
            Ok(None)
        }
    }
}

#[derive(Debug)]
struct EmbeddedJpegCandidate {
    data: Vec<u8>,
    width: u32,
    height: u32,
    max_edge: u32,
}

fn collect_embedded_jpeg_candidates(file_path: &str) -> Result<Vec<EmbeddedJpegCandidate>, String> {
    let exif = match read_exif_permissive(file_path) {
        Some(exif) => exif,
        None => return Ok(Vec::new()),
    };

    let buf = exif.buf();
    let mut candidates: Vec<EmbeddedJpegCandidate> = Vec::new();

    // The parser caps IFD count at 8. Scan all possible IFDs for embedded JPEGs.
    for ifd_index in 0u16..8u16 {
        let ifd = In(ifd_index);
        let offset = exif
            .get_field(Tag::JPEGInterchangeFormat, ifd)
            .and_then(|field| field.value.get_uint(0))
            .map(|value| value as usize);
        let len = exif
            .get_field(Tag::JPEGInterchangeFormatLength, ifd)
            .and_then(|field| field.value.get_uint(0))
            .map(|value| value as usize);

        let (offset, len) = match (offset, len) {
            (Some(offset), Some(len)) if len > 4 => (offset, len),
            _ => continue,
        };

        let end = offset.saturating_add(len);
        if end > buf.len() {
            continue;
        }

        let candidate = &buf[offset..end];
        // Basic JPEG signature check to avoid selecting non-JPEG payloads.
        if !(candidate.starts_with(&[0xFF, 0xD8])) {
            continue;
        }

        let data = candidate.to_vec();
        let (width, height, max_edge) = match image::load_from_memory(&data) {
            Ok(image) => {
                let (width, height) = image.dimensions();
                (width, height, width.max(height))
            }
            Err(_) => continue,
        };

        if max_edge == 0 {
            continue;
        }

        candidates.push(EmbeddedJpegCandidate {
            data,
            width,
            height,
            max_edge,
        });
    }

    Ok(candidates)
}

fn select_embedded_jpeg_for_preview(file_path: &str) -> Result<Option<Vec<u8>>, String> {
    let candidates = collect_embedded_jpeg_candidates(file_path)?;
    let (raw_width, raw_height) = t_libraw::get_raw_dimensions(file_path)?;
    let mut selected: Option<EmbeddedJpegCandidate> = None;

    for candidate in candidates {
        let width_delta = candidate.width.abs_diff(raw_width);
        let height_delta = candidate.height.abs_diff(raw_height);
        let is_fullsize = width_delta.saturating_mul(100) <= raw_width.max(1)
            && height_delta.saturating_mul(100) <= raw_height.max(1);

        if !is_fullsize {
            continue;
        }

        match &selected {
            Some(best) if candidate.max_edge <= best.max_edge => {}
            _ => selected = Some(candidate),
        }
    }

    Ok(selected.map(|item| item.data))
}

fn select_embedded_jpeg_for_thumbnail(
    file_path: &str,
    thumbnail_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    let candidates = collect_embedded_jpeg_candidates(file_path)?;
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut best_not_smaller: Option<EmbeddedJpegCandidate> = None;
    let mut best_smaller: Option<EmbeddedJpegCandidate> = None;

    for candidate in candidates {
        if candidate.max_edge >= thumbnail_size {
            match &best_not_smaller {
                Some(best) if candidate.max_edge >= best.max_edge => {}
                _ => best_not_smaller = Some(candidate),
            }
        } else {
            match &best_smaller {
                Some(best) if candidate.max_edge <= best.max_edge => {}
                _ => best_smaller = Some(candidate),
            }
        }
    }

    Ok(best_not_smaller.or(best_smaller).map(|item| item.data))
}

fn get_jpeg_orientation_from_bytes(data: &[u8]) -> i32 {
    let exif = match read_exif_from_bytes_permissive(data) {
        Some(exif) => exif,
        None => return 1,
    };

    exif.get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .map(|value| value as i32)
        .unwrap_or(1)
}

pub fn get_raw_preview_image(file_path: &str) -> Result<Option<Vec<u8>>, String> {
    // Primary: LibRaw handles extraction and rotation
    if let Ok(Some(data)) = t_libraw::get_raw_preview_image(file_path) {
        return Ok(Some(data));
    }

    // Fallback: EXIF-based embedded JPEG extraction
    if let Ok(Some(preview)) = select_embedded_jpeg_for_preview(file_path) {
        let image = image::load_from_memory(&preview)
            .map_err(|e| format!("Failed to decode embedded RAW preview: {}", e))?;
        let image = apply_orientation(image, get_jpeg_orientation_from_bytes(&preview));
        let buf = crate::t_jpeg::encode_rgb8(&image.to_rgb8(), 85)
            .map_err(|e| format!("Failed to encode embedded RAW preview: {}", e))?;
        return Ok(Some(buf));
    }

    #[cfg(target_os = "macos")]
    if let Ok(Some(data)) = get_thumbnail_with_sips(file_path, 4096) {
        return Ok(Some(data));
    }

    let orientation = get_image_orientation(file_path);

    // Final fallback for formats that can be decoded directly by `image`.
    if let Ok(Some(data)) = get_image_thumbnail(file_path, orientation, 4096) {
        return Ok(Some(data));
    }

    Ok(None)
}

pub fn get_raw_dimensions(file_path: &str) -> Result<(u32, u32), String> {
    if let Ok((width, height, _raw_flip)) = t_libraw::get_raw_dimensions_with_flip(file_path) {
        if width > 0 && height > 0 {
            return Ok((width, height));
        }
    }

    if let Ok((width, height)) = get_image_dimensions(file_path) {
        if width > 0 && height > 0 {
            return Ok((width, height));
        }
    }

    if let Ok(Some((width, height))) = get_raw_dimensions_from_exif(file_path) {
        return Ok((width, height));
    }

    #[cfg(target_os = "macos")]
    if let Ok(Some((width, height))) = get_dimensions_with_sips(file_path) {
        return Ok((width, height));
    }

    if let Ok(Some(preview)) = select_embedded_jpeg_for_preview(file_path) {
        if let Ok(image) = image::load_from_memory(&preview) {
            return Ok(image.dimensions());
        }
    }

    Err("Failed to resolve RAW dimensions".to_string())
}

pub fn get_raw_thumbnail(
    file_path: &str,
    orientation: i32,
    thumbnail_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    // Primary: LibRaw handles extraction and rotation
    if let Ok(Some(data)) = t_libraw::get_raw_thumbnail(file_path, thumbnail_size) {
        return Ok(Some(data));
    }

    // Fallback: EXIF-based embedded JPEG extraction
    if let Ok(Some(preview)) = select_embedded_jpeg_for_thumbnail(file_path, thumbnail_size) {
        let img = image::load_from_memory(&preview)
            .map_err(|e| format!("Failed to decode RAW preview image: {}", e))?;
        return resize_dynamic_image_to_jpeg(
            img,
            get_jpeg_orientation_from_bytes(&preview),
            thumbnail_size,
        )
        .map(Some);
    }

    #[cfg(target_os = "macos")]
    if let Ok(Some(data)) = get_thumbnail_with_sips(file_path, thumbnail_size) {
        return Ok(Some(data));
    }

    // Fallback for formats that can be decoded directly by `image`.
    get_image_thumbnail(file_path, orientation, thumbnail_size)
}

/// edit image impl

/// crop data
#[derive(Debug, Deserialize, Serialize)]
pub struct CropData {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// resize data
#[derive(Debug, Deserialize, Serialize)]
pub struct ResizeData {
    width: Option<u32>,
    height: Option<u32>,
}

/// edit params
#[derive(Debug, Deserialize, Serialize)]
pub struct EditParams {
    #[serde(rename = "sourceFilePath")]
    source_file_path: String,
    #[serde(rename = "destFilePath")]
    dest_file_path: String,
    #[serde(rename = "outputFormat")]
    output_format: String,
    orientation: i32, // exif orientation value
    #[serde(rename = "flipHorizontal")]
    flip_horizontal: bool,
    #[serde(rename = "flipVertical")]
    flip_vertical: bool,
    rotate: i32,
    crop: CropData,
    resize: ResizeData,
    quality: Option<u8>,
    // New adjustments
    filter: Option<String>,  // "grayscale", "sepia", "invert"
    brightness: Option<i32>, // -100 to 100
    contrast: Option<f32>,   // -100.0 to 100.0
    blur: Option<f32>,       // sigma > 0
    hue_rotate: Option<i32>, // degrees
    saturation: Option<f32>, // multiplier, 1.0 is normal
}

/// edit an image and save to dest file
pub async fn edit_image(params: EditParams) -> bool {
    if let Ok(img) = get_edited_image(&params).await {
        let path = Path::new(&params.dest_file_path);
        let format = match params.output_format.as_str() {
            "png" => image::ImageFormat::Png,
            "webp" => image::ImageFormat::WebP,
            _ => image::ImageFormat::Jpeg,
        };

        // Snapshot original metadata before overwriting the file.
        // For overwrite (source == dest) we must copy the original to a
        // temp location first — once we File::create the destination the
        // original EXIF is gone. For save-as-new the source is untouched.
        let metadata_backup_path = if format == image::ImageFormat::Jpeg
            || format == image::ImageFormat::WebP
        {
            match prepare_metadata_backup_path(&params.source_file_path, &params.dest_file_path) {
                Ok(path) => path,
                Err(_) => return false,
            }
        } else {
            None
        };

        let metadata_source = metadata_backup_path
            .as_ref()
            .map(|p| p.as_path())
            .unwrap_or_else(|| Path::new(&params.source_file_path));

        let quality = params.quality.unwrap_or(80);
        let save_ok = if format == image::ImageFormat::Jpeg {
            if let Ok(file) = std::fs::File::create(path) {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
                encoder.encode_image(&img).is_ok()
            } else {
                false
            }
        } else {
            img.save_with_format(path, format).is_ok()
        };

        if !save_ok {
            cleanup_metadata_backup(&metadata_backup_path);
            return false;
        }

        if format == image::ImageFormat::Jpeg || format == image::ImageFormat::WebP {
            if let Err(_) = copy_metadata_to_output(metadata_source, path) {
                if metadata_backup_path.is_some() {
                    let _ = fs::copy(metadata_source, path);
                } else {
                    let _ = fs::remove_file(path);
                }
                cleanup_metadata_backup(&metadata_backup_path);
                return false;
            }

            cleanup_metadata_backup(&metadata_backup_path);
        }

        return true;
    }
    false
}

fn prepare_metadata_backup_path(
    source_file_path: &str,
    dest_file_path: &str,
) -> Result<Option<PathBuf>, String> {
    if source_file_path != dest_file_path {
        return Ok(None);
    }

    let source_path = Path::new(source_file_path);
    let extension = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("tmp");
    let backup_path = std::env::temp_dir().join(format!(
        "lap-edit-metadata-{}.{}",
        Uuid::new_v4(),
        extension
    ));

    fs::copy(source_path, &backup_path)
        .map_err(|e| format!("Failed to create metadata backup: {}", e))?;

    Ok(Some(backup_path))
}

fn cleanup_metadata_backup(path: &Option<PathBuf>) {
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

/// Copies metadata from source to destination.
/// Prefers little_exif for JPEGs and falls back to kamadak-exif for RAW formats.
fn copy_metadata_to_output(source_path: &Path, dest_path: &Path) -> Result<(), String> {
    let source_path_buf = source_path.to_path_buf();

    // Check file type to detect RAW formats
    let file_type =
        crate::t_utils::get_file_type(source_path.to_str().unwrap_or_default()).unwrap_or(0);
    let is_raw = file_type == 3;

    let mut little_exif_worked = false;
    let mut little_exif_error = String::new();

    if !is_raw {
        // Use little_exif for standard formats (JPEG/WebP).
        // Wrapped in catch_unwind as little_exif can panic on malformed data.
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            LittleExifMetadata::new_from_path(&source_path_buf)
        }));

        match result {
            Ok(Ok(mut metadata)) => {
                sanitize_edit_output_metadata(&mut metadata);
                if let Err(e) = metadata.write_to_file(dest_path) {
                    little_exif_error = format!("little_exif write failed: {}", e);
                } else {
                    little_exif_worked = true;
                }
            }
            Ok(Err(e)) => {
                little_exif_error = format!("little_exif read failed: {}", e);
            }
            Err(_) => {
                little_exif_error = "little_exif panicked".to_string();
            }
        }
    }

    if little_exif_worked {
        return Ok(());
    }

    // Fallback: use kamadak-exif which has broader RAW support
    match copy_metadata_from_raw_to_jpeg(source_path, dest_path) {
        Ok(()) => Ok(()),
        Err(raw_error) => {
            if is_raw {
                Err(format!("RAW metadata extraction failed: {}", raw_error))
            } else {
                Err(format!(
                    "Metadata copy failed. little_exif: {}; kamadak: {}",
                    little_exif_error, raw_error
                ))
            }
        }
    }
}

/// Removes tags that shouldn't be copied to the edited output (like original orientation and dimensions).
fn sanitize_edit_output_metadata(metadata: &mut LittleExifMetadata) {
    metadata.remove_tag_by_hex_group(0x0112, ExifTagGroup::GENERIC); // Orientation
    metadata.remove_tag_by_hex_group(0x0100, ExifTagGroup::GENERIC); // ImageWidth
    metadata.remove_tag_by_hex_group(0x0101, ExifTagGroup::GENERIC); // ImageLength
    metadata.remove_tag_by_hex_group(0xA002, ExifTagGroup::EXIF); // PixelXDimension
    metadata.remove_tag_by_hex_group(0xA003, ExifTagGroup::EXIF); // PixelYDimension
    metadata.remove_tag_by_hex_group(0x0201, ExifTagGroup::GENERIC); // JPEGInterchangeFormat
    metadata.remove_tag_by_hex_group(0x0202, ExifTagGroup::GENERIC); // JPEGInterchangeFormatLength
}

/// Filter for EXIF fields to copy.
/// We mainly copy PRIMARY IFD and exclude pointers or hardware-specific tags that
/// might be invalidated by the image edit (like StripOffsets or Orientation).
fn should_copy_exif_field(field: &exif::Field) -> bool {
    if field.ifd_num != In::PRIMARY {
        return false;
    }

    !matches!(
        field.tag,
        Tag::ExifIFDPointer
            | Tag::GPSInfoIFDPointer
            | Tag::InteropIFDPointer
            | Tag::StripOffsets
            | Tag::StripByteCounts
            | Tag::TileOffsets
            | Tag::TileByteCounts
            | Tag::JPEGInterchangeFormat
            | Tag::JPEGInterchangeFormatLength
            | Tag::Orientation
            | Tag::ImageWidth
            | Tag::ImageLength
            | Tag::PixelXDimension
            | Tag::PixelYDimension
    )
}

/// Extracts EXIF from a (potentially RAW) source and injects it into a JPEG destination.
/// Includes a three-pass reduction logic to ensure metadata fits within the 64KB JPEG segment limit.
fn copy_metadata_from_raw_to_jpeg(source_path: &Path, dest_path: &Path) -> Result<(), String> {
    let file = File::open(source_path)
        .map_err(|e| format!("Failed to open source metadata file: {}", e))?;
    let mut reader = BufReader::new(file);

    // Try read_from_container first (handles JPEG/TIFF/RAW-TIFF)
    let exif = match Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif,
        Err(_) => {
            // Fallback: try raw TIFF read (some RAWs are just TIFF structures)
            let data = fs::read(source_path).map_err(|e| e.to_string())?;
            match Reader::new().read_raw(data) {
                Ok(exif) => exif,
                Err(_) => return Ok(()), // Truly no metadata found or unreadable, skip
            }
        }
    };

    // Helper to attempt encoding a set of fields and check if it fits in 64KB
    let encode_and_check = |fields: Vec<&exif::Field>| -> Option<Vec<u8>> {
        let mut writer = exif::experimental::Writer::new();
        for field in fields {
            writer.push_field(field);
        }
        let mut tiff_cursor = Cursor::new(Vec::new());
        if writer.write(&mut tiff_cursor, exif.little_endian()).is_ok() {
            let data = tiff_cursor.into_inner();
            if data.len() <= 65527 {
                // JPEG APP1 max is 65535, minus 8 bytes header
                return Some(data);
            }
        }
        None
    };

    // Pass 1: Attempt to copy all standard fields
    let initial_fields: Vec<&exif::Field> = exif
        .fields()
        .filter(|f| should_copy_exif_field(f))
        .collect();
    let mut exif_data = encode_and_check(initial_fields);

    // Pass 2: If too large, strip typically large vendor blocks (MakerNote, UserComment)
    if exif_data.is_none() {
        let reduced_fields: Vec<&exif::Field> = exif
            .fields()
            .filter(|f| should_copy_exif_field(f))
            .filter(|f| !matches!(f.tag, Tag::MakerNote | Tag::UserComment))
            .collect();
        exif_data = encode_and_check(reduced_fields);
    }

    // Pass 3: If still too large, keep only the most essential photography and GPS tags
    if exif_data.is_none() {
        let essential_fields: Vec<&exif::Field> = exif
            .fields()
            .filter(|f| should_copy_exif_field(f))
            .filter(|f| {
                matches!(
                    f.tag,
                    Tag::Make
                        | Tag::Model
                        | Tag::DateTimeOriginal
                        | Tag::DateTimeDigitized
                        | Tag::ExposureTime
                        | Tag::FNumber
                        | Tag::PhotographicSensitivity
                        | Tag::FocalLength
                        | Tag::LensMake
                        | Tag::LensModel
                        | Tag::ExposureBiasValue
                        | Tag::GPSLatitudeRef
                        | Tag::GPSLatitude
                        | Tag::GPSLongitudeRef
                        | Tag::GPSLongitude
                        | Tag::GPSAltitudeRef
                        | Tag::GPSAltitude
                )
            })
            .collect();
        exif_data = encode_and_check(essential_fields);
    }

    if let Some(data) = exif_data {
        write_jpeg_exif_block(dest_path, &data)
    } else {
        eprintln!("EXIF metadata still too large even after stripping, skipping");
        Ok(())
    }
}

/// Manually injects a TIFF-formatted EXIF block into a JPEG file's APP1 segment.
/// This is used when high-level libraries (like little_exif) fail or are not applicable.
fn write_jpeg_exif_block(dest_path: &Path, exif_tiff_data: &[u8]) -> Result<(), String> {
    let mut file_buffer =
        fs::read(dest_path).map_err(|e| format!("Failed to read destination JPEG: {}", e))?;

    // Clear existing metadata using little_exif if possible
    let _ = LittleExifMetadata::clear_metadata(&mut file_buffer, FileExtension::JPEG);

    if file_buffer.len() < 2 || file_buffer[0] != 0xFF || file_buffer[1] != 0xD8 {
        return Err("Destination file is not a valid JPEG".to_string());
    }

    // Prepare APP1 segment: FF E1 + Length + "Exif\0\0" + TIFF data
    let app1_length = (2 + 6 + exif_tiff_data.len()) as u16;
    let mut app1_segment = Vec::with_capacity(2 + 2 + 6 + exif_tiff_data.len());
    app1_segment.extend_from_slice(&[0xFF, 0xE1]);
    app1_segment.extend_from_slice(&app1_length.to_be_bytes());
    app1_segment.extend_from_slice(b"Exif\0\0");
    app1_segment.extend_from_slice(exif_tiff_data);

    // Reconstruct file: FF D8 + New APP1 + Remainder of file
    let mut output = Vec::with_capacity(file_buffer.len() + app1_segment.len());
    output.extend_from_slice(&file_buffer[..2]);
    output.extend_from_slice(&app1_segment);
    output.extend_from_slice(&file_buffer[2..]);

    fs::write(dest_path, output).map_err(|e| format!("Failed to write destination JPEG: {}", e))
}

/// copy an image to clipboard
pub fn copy_image_to_clipboard(img: DynamicImage) -> bool {
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();
    let bytes = rgba.into_raw();

    if let Ok(mut clipboard) = Clipboard::new() {
        let image_data = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(bytes),
        };
        return clipboard.set_image(image_data).is_ok();
    }
    false
}

pub(crate) fn is_heic_path(file_path: &str) -> bool {
    matches!(
        t_utils::get_file_extension(file_path)
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "heic" | "heif" | "hif"
    )
}

fn is_avif_path(file_path: &str) -> bool {
    matches!(
        t_utils::get_file_extension(file_path)
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "avif"
    )
}

pub(crate) fn is_ffmpeg_backed_image_path(file_path: &str) -> bool {
    t_utils::get_file_extension(file_path).is_some_and(|extension| {
        crate::t_common::FFMPEG_BACKED_IMGS
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&extension))
    })
}

fn should_generate_preview_for_file(file_path: &str, file_type: i64) -> bool {
    file_type == 3
        || crate::t_libraw::is_tiff_path(file_path)
        || t_jxl::is_jxl_path(file_path)
        || is_heic_path(file_path)
        || is_ffmpeg_backed_image_path(file_path)
        || is_avif_path(file_path)
}

async fn get_generated_preview_bytes(file_path: &str) -> Result<Option<Vec<u8>>, String> {
    let file_type = t_utils::get_file_type(file_path).unwrap_or(0);

    if file_type == 3 {
        return get_raw_preview_image(file_path);
    }

    if t_jxl::is_jxl_path(file_path) {
        return t_jxl::get_jxl_preview_image(file_path, 4096);
    }

    if crate::t_libraw::is_tiff_path(file_path) {
        return match get_raw_preview_image(file_path) {
            Ok(Some(data)) => Ok(Some(data)),
            _ => {
                #[cfg(target_os = "macos")]
                {
                    get_thumbnail_with_sips(file_path, 4096)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok(None)
                }
            }
        };
    }

    if is_heic_path(file_path) {
        #[cfg(target_os = "macos")]
        {
            return get_thumbnail_with_sips(file_path, 4096);
        }
        #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
        {
            return crate::t_heif::get_heif_preview(
                file_path,
                get_image_orientation(file_path),
                4096,
            );
        }
        #[cfg(all(not(target_os = "macos"), not(lap_has_libheif)))]
        {
            return crate::t_video::get_video_thumbnail(file_path, 4096, None, None).await;
        }
    }

    if is_ffmpeg_backed_image_path(file_path) {
        return crate::t_video::get_video_thumbnail(file_path, 4096, None, None).await;
    }

    if is_avif_path(file_path) {
        return get_image_thumbnail(file_path, get_image_orientation(file_path), 4096);
    }

    Ok(None)
}

async fn clipboard_preview_png(file_paths: &[String]) -> Option<Vec<u8>> {
    for file_path in file_paths {
        let file_type = t_utils::get_file_type(file_path).unwrap_or(0);
        if file_type != 1 && file_type != 3 {
            continue;
        }
        let img = if should_generate_preview_for_file(file_path, file_type) {
            let Some(preview) = get_generated_preview_bytes(file_path).await.ok().flatten() else {
                continue;
            };
            let Ok(img) = image::load_from_memory(&preview) else {
                continue;
            };
            img
        } else {
            let Ok(img) = image::open(Path::new(file_path)) else {
                continue;
            };
            img
        };
        let mut png = std::io::Cursor::new(Vec::new());
        if img.write_to(&mut png, image::ImageFormat::Png).is_ok() {
            return Some(png.into_inner());
        }
    }
    None
}

pub async fn copy_files_to_clipboard(
    app_handle: &tauri::AppHandle,
    file_paths: Vec<String>,
) -> Result<usize, String> {
    let file_paths = file_paths
        .into_iter()
        .filter(|path| Path::new(path).is_file())
        .take(10)
        .collect::<Vec<_>>();
    if file_paths.is_empty() {
        return Err("No valid files to copy".to_string());
    }
    let preview = clipboard_preview_png(&file_paths).await;
    crate::t_pasteboard::copy_files_and_image(app_handle, &file_paths, preview.as_deref()).await?;
    Ok(file_paths.len())
}

/// collage export params (template / strip / free composite)
#[derive(Debug, Deserialize, Serialize)]
pub struct CollageExportParams {
    #[serde(rename = "sourceFilePaths")]
    source_file_paths: Vec<String>,
    #[serde(rename = "destFilePath")]
    dest_file_path: String,
    #[serde(rename = "outputFormat")]
    output_format: String,
    quality: Option<u8>,
    /// "2" | "3" | "4" | "6" | "9" | "strip-h" | "strip-v" | "free"
    template: String,
    #[serde(rename = "outputWidth")]
    output_width: u32,
    #[serde(rename = "outputHeight")]
    output_height: u32,
    gap: u32,
    margin: u32,
    /// "#RRGGBB" / "RRGGBB" / "rgb(r,g,b)"
    background: String,
    /// "cover" | "contain" (default cover); free mode always cover-fit into item box
    #[serde(rename = "fillMode", default)]
    fill_mode: Option<String>,
    /// cell corner radius in px (export canvas space)
    #[serde(default)]
    radius: Option<u32>,
    #[serde(rename = "strokeWidth", default)]
    stroke_width: Option<u32>,
    #[serde(rename = "strokeColor", default)]
    stroke_color: Option<String>,
    /// Free-canvas items (normalized 0–1 geometry). Used when template == "free".
    #[serde(default)]
    items: Option<Vec<FreeCollageItemParams>>,
    /// Magazine / freeform cells (normalized 0–1). Used when template == "cells"
    /// or when provided for any non-free template.
    #[serde(default)]
    cells: Option<Vec<CollageCellRectParams>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FreeCollageItemParams {
    #[serde(rename = "filePath")]
    file_path: String,
    /// Normalized 0–1 relative to output canvas (top-left of unrotated box)
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    /// Degrees clockwise
    #[serde(default)]
    rotate: f32,
    #[serde(default)]
    z: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CollageCellRectParams {
    /// Normalized 0–1 of canvas
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// Export a template/strip/free collage to dest_file_path. Returns true on success.
pub async fn export_collage(params: CollageExportParams) -> Result<bool, String> {
    if params.template.trim().eq_ignore_ascii_case("free") {
        return export_collage_free(params).await;
    }

    let cells = params
        .cells
        .as_ref()
        .map(|c| c.as_slice())
        .filter(|c| !c.is_empty());
    if cells.is_some() || params.template.trim().eq_ignore_ascii_case("cells") {
        return export_collage_cells(params).await;
    }

    let image_count = params
        .source_file_paths
        .iter()
        .filter(|p| !p.trim().is_empty())
        .count();
    let (cols, rows) = collage_grid_dims(&params.template, image_count)?;
    let cell_count = (cols as usize).saturating_mul(rows as usize);
    if cell_count == 0 {
        return Err("Invalid collage template".to_string());
    }
    // Fixed grids have a hard cell budget; refuse silent truncation of extra sources.
    // Strip templates expand with image_count (see collage_grid_dims), so they won't hit this.
    if image_count > cell_count {
        return Err(format!(
            "Collage template has {} cells but {} images were provided; remove extras or pick a larger template",
            cell_count, image_count
        ));
    }

    let out_w = params.output_width.max(64).min(8192);
    let out_h = params.output_height.max(64).min(8192);
    let margin = params.margin.min(out_w.min(out_h) / 4);
    let gap = params.gap.min(out_w.min(out_h) / 4);

    let inner_w = out_w.saturating_sub(margin.saturating_mul(2));
    let inner_h = out_h.saturating_sub(margin.saturating_mul(2));
    let total_gap_w = gap.saturating_mul(cols.saturating_sub(1));
    let total_gap_h = gap.saturating_mul(rows.saturating_sub(1));
    if inner_w <= total_gap_w || inner_h <= total_gap_h {
        return Err("Collage output size too small for margin/gap".to_string());
    }
    let cell_w = (inner_w - total_gap_w) / cols;
    let cell_h = (inner_h - total_gap_h) / rows;
    if cell_w < 8 || cell_h < 8 {
        return Err("Collage cell size too small".to_string());
    }

    let bg = parse_hex_color(&params.background).unwrap_or([255, 255, 255]);
    let fill = parse_fill_mode(params.fill_mode.as_deref());
    let radius = params.radius.unwrap_or(0).min(cell_w.min(cell_h) / 2);
    let stroke_w = params.stroke_width.unwrap_or(0).min(32);
    let stroke =
        parse_hex_color(params.stroke_color.as_deref().unwrap_or("#000000")).unwrap_or([0, 0, 0]);

    let mut canvas = RgbaImage::from_pixel(out_w, out_h, Rgba([bg[0], bg[1], bg[2], 255]));

    // Equal grid: every cell is cell_w×cell_h — only keep source pixels that can fill that.
    let max_edge = collage_source_max_edge(cell_w, cell_h);
    let mut sources: Vec<Option<DynamicImage>> = Vec::with_capacity(cell_count);
    for i in 0..cell_count {
        if let Some(path) = params.source_file_paths.get(i) {
            if path.trim().is_empty() {
                sources.push(None);
                continue;
            }
            match load_image_for_layout(path, max_edge).await {
                Ok(img) => {
                    let fitted = downscale_image_for_fit_cells(img, cell_w, cell_h, fill);
                    sources.push(Some(fitted));
                }
                Err(err) => {
                    eprintln!("collage skip {}: {}", path, err);
                    sources.push(None);
                }
            }
        } else {
            sources.push(None);
        }
    }

    for row in 0..rows {
        for col in 0..cols {
            let idx = (row * cols + col) as usize;
            let Some(Some(img)) = sources.get(idx) else {
                continue;
            };
            let x = margin + col * (cell_w + gap);
            let y = margin + row * (cell_h + gap);
            draw_fitted_cell(
                &mut canvas,
                img,
                x,
                y,
                cell_w,
                cell_h,
                fill,
                radius,
                stroke_w,
                stroke,
            );
        }
    }

    save_collage_image(&params, DynamicImage::ImageRgba8(canvas))
}

/// Magazine / freeform cells: each cell is a normalized rect (0–1 of canvas).
async fn export_collage_cells(params: CollageExportParams) -> Result<bool, String> {
    let cells = params.cells.clone().unwrap_or_default();
    if cells.is_empty() {
        return Err("No collage cells".to_string());
    }
    let image_count = params
        .source_file_paths
        .iter()
        .filter(|p| !p.trim().is_empty())
        .count();
    if image_count > cells.len() {
        return Err(format!(
            "Collage has {} cells but {} images were provided; remove extras or pick a larger template",
            cells.len(),
            image_count
        ));
    }

    let out_w = params.output_width.max(64).min(8192);
    let out_h = params.output_height.max(64).min(8192);
    let bg = parse_hex_color(&params.background).unwrap_or([255, 255, 255]);
    let fill = parse_fill_mode(params.fill_mode.as_deref());
    let stroke =
        parse_hex_color(params.stroke_color.as_deref().unwrap_or("#000000")).unwrap_or([0, 0, 0]);
    let stroke_w_param = params.stroke_width.unwrap_or(0).min(32);
    let radius_param = params.radius.unwrap_or(0);

    let mut canvas = RgbaImage::from_pixel(out_w, out_h, Rgba([bg[0], bg[1], bg[2], 255]));

    // Resolve pixel rects first, then load each source scaled to its cell.
    let mut resolved: Vec<(u32, u32, u32, u32, String)> = Vec::with_capacity(cells.len());
    for (idx, cell) in cells.iter().enumerate() {
        let path = params
            .source_file_paths
            .get(idx)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        let x_norm = cell.x.clamp(-0.1, 1.0);
        let y_norm = cell.y.clamp(-0.1, 1.0);
        let w_norm = cell.w.clamp(0.01, 1.2);
        let h_norm = cell.h.clamp(0.01, 1.2);
        let x = ((x_norm * out_w as f32).round() as i64).max(0) as u32;
        let y = ((y_norm * out_h as f32).round() as i64).max(0) as u32;
        let cw = ((w_norm * out_w as f32).round() as u32)
            .max(4)
            .min(out_w.saturating_sub(x).max(4));
        let ch = ((h_norm * out_h as f32).round() as u32)
            .max(4)
            .min(out_h.saturating_sub(y).max(4));
        resolved.push((x, y, cw, ch, path));
    }

    // Unique path → max cell it occupies (same photo may appear once per slot).
    use std::collections::HashMap;
    let mut path_max: HashMap<String, (u32, u32)> = HashMap::new();
    for (_, _, cw, ch, path) in &resolved {
        let e = path_max.entry(path.clone()).or_insert((0, 0));
        e.0 = e.0.max(*cw);
        e.1 = e.1.max(*ch);
    }

    let mut cache: HashMap<String, DynamicImage> = HashMap::new();
    if !path_max.is_empty() {
        let mut set = tokio::task::JoinSet::new();
        for (path, (mw, mh)) in path_max {
            let max_edge = collage_source_max_edge(mw, mh);
            set.spawn(async move {
                let result = load_image_for_layout(&path, max_edge).await;
                (path, mw, mh, result)
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((path, mw, mh, Ok(img))) => {
                    cache.insert(path, downscale_image_for_fit_cells(img, mw, mh, fill));
                }
                Ok((path, _, _, Err(err))) => {
                    eprintln!("collage cells skip {}: {}", path, err);
                }
                Err(err) => eprintln!("collage cells load task failed: {}", err),
            }
        }
    }

    for (x, y, cw, ch, path) in resolved {
        let Some(img) = cache.get(&path) else {
            continue;
        };
        let radius = radius_param.min(cw.min(ch) / 2);
        draw_fitted_cell(
            &mut canvas,
            img,
            x,
            y,
            cw,
            ch,
            fill,
            radius,
            stroke_w_param,
            stroke,
        );
    }

    save_collage_image(&params, DynamicImage::ImageRgba8(canvas))
}

async fn export_collage_free(params: CollageExportParams) -> Result<bool, String> {
    let mut items = params.items.clone().unwrap_or_default();
    if items.is_empty() {
        // Fallback: cascade from source paths if items omitted.
        for (i, path) in params.source_file_paths.iter().enumerate().take(20) {
            if path.trim().is_empty() {
                continue;
            }
            let step = 0.06_f32;
            items.push(FreeCollageItemParams {
                file_path: path.clone(),
                x: (0.1 + i as f32 * step).min(0.55),
                y: (0.1 + i as f32 * step * 0.85).min(0.55),
                w: 0.35,
                h: 0.35,
                rotate: 0.0,
                z: i as i32 + 1,
            });
        }
    }
    if items.is_empty() {
        return Err("No free collage items".to_string());
    }

    items.sort_by(|a, b| a.z.cmp(&b.z).then_with(|| a.file_path.cmp(&b.file_path)));

    let out_w = params.output_width.max(64).min(8192);
    let out_h = params.output_height.max(64).min(8192);
    let bg = parse_hex_color(&params.background).unwrap_or([255, 255, 255]);
    let fill = parse_fill_mode(params.fill_mode.as_deref());
    let stroke =
        parse_hex_color(params.stroke_color.as_deref().unwrap_or("#000000")).unwrap_or([0, 0, 0]);
    let stroke_w_param = params.stroke_width.unwrap_or(0).min(32);
    let radius_param = params.radius.unwrap_or(0);

    let mut canvas = RgbaImage::from_pixel(out_w, out_h, Rgba([bg[0], bg[1], bg[2], 255]));

    // Precompute each item's on-canvas box, then load unique paths scaled to max box.
    use std::collections::HashMap;
    let mut path_max: HashMap<String, (u32, u32)> = HashMap::new();
    let mut prepared: Vec<(FreeCollageItemParams, u32, u32)> = Vec::with_capacity(items.len());
    for item in items {
        if item.file_path.trim().is_empty() {
            continue;
        }
        let w_norm = item.w.clamp(0.02, 1.0);
        let h_norm = item.h.clamp(0.02, 1.0);
        let box_w = ((w_norm * out_w as f32).round() as u32).max(8).min(out_w);
        let box_h = ((h_norm * out_h as f32).round() as u32).max(8).min(out_h);
        // Rotation expands the axis-aligned bounds of the unrotated box.
        // Fixed ~15% headroom undersamples near 45° (1/cos45° ≈ 1.414).
        let (need_w, need_h) = rotated_box_source_need(box_w, box_h, item.rotate);
        let e = path_max
            .entry(item.file_path.trim().to_string())
            .or_insert((0, 0));
        e.0 = e.0.max(need_w);
        e.1 = e.1.max(need_h);
        prepared.push((item, box_w, box_h));
    }

    let mut cache: HashMap<String, DynamicImage> = HashMap::new();
    if !path_max.is_empty() {
        let mut set = tokio::task::JoinSet::new();
        for (path, (mw, mh)) in path_max {
            let max_edge = collage_source_max_edge(mw, mh);
            set.spawn(async move {
                let result = load_image_for_layout(&path, max_edge).await;
                (path, mw, mh, result)
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((path, mw, mh, Ok(img))) => {
                    cache.insert(path, downscale_image_for_fit_cells(img, mw, mh, fill));
                }
                Ok((path, _, _, Err(err))) => {
                    eprintln!("collage free skip {}: {}", path, err);
                }
                Err(err) => eprintln!("collage free load task failed: {}", err),
            }
        }
    }

    for (item, box_w, box_h) in prepared {
        let Some(img) = cache.get(item.file_path.trim()) else {
            continue;
        };

        let w_norm = item.w.clamp(0.02, 1.0);
        let h_norm = item.h.clamp(0.02, 1.0);
        let x_norm = item.x.clamp(-0.5, 1.0);
        let y_norm = item.y.clamp(-0.5, 1.0);
        let radius = radius_param.min(box_w.min(box_h) / 2);
        let stroke_w = stroke_w_param;

        // Build unrotated cell, then rotate and place by center.
        let mut cell = RgbaImage::from_pixel(box_w, box_h, Rgba([0, 0, 0, 0]));
        draw_fitted_cell(
            &mut cell, &img, 0, 0, box_w, box_h, fill, radius, stroke_w, stroke,
        );

        let rotated = if item.rotate.abs() > 0.05 {
            rotate_rgba_image(&cell, item.rotate)
        } else {
            cell
        };

        let center_x = (x_norm + w_norm * 0.5) * out_w as f32;
        let center_y = (y_norm + h_norm * 0.5) * out_h as f32;
        let place_x = (center_x - rotated.width() as f32 * 0.5).round() as i64;
        let place_y = (center_y - rotated.height() as f32 * 0.5).round() as i64;
        image::imageops::overlay(&mut canvas, &rotated, place_x, place_y);
    }

    save_collage_image(&params, DynamicImage::ImageRgba8(canvas))
}

fn save_collage_image(params: &CollageExportParams, img: DynamicImage) -> Result<bool, String> {
    let path = Path::new(&params.dest_file_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output folder: {}", e))?;
        }
    }

    let format = match params.output_format.to_ascii_lowercase().as_str() {
        "png" => image::ImageFormat::Png,
        "webp" => image::ImageFormat::WebP,
        _ => image::ImageFormat::Jpeg,
    };
    let quality = params.quality.unwrap_or(90).clamp(1, 100);

    let save_ok = if format == image::ImageFormat::Jpeg {
        let rgb = img.to_rgb8();
        match std::fs::File::create(path) {
            Ok(file) => {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
                encoder.encode_image(&rgb).is_ok()
            }
            Err(_) => false,
        }
    } else {
        img.save_with_format(path, format).is_ok()
    };

    if !save_ok {
        return Err("Failed to write collage image".to_string());
    }
    Ok(true)
}

/// Source decode/downscale budget for a free-canvas item that may be rotated.
/// Returns the AABB size of the unrotated box after rotation so cover-fit keeps
/// enough pixels near 45° (≈ +41%), not a fixed 15% pad.
fn rotated_box_source_need(box_w: u32, box_h: u32, degrees: f32) -> (u32, u32) {
    if degrees.abs() <= 0.05 {
        return (box_w, box_h);
    }
    let rad = degrees.to_radians();
    let (c, s) = (rad.cos().abs(), rad.sin().abs());
    let bw = box_w as f32;
    let bh = box_h as f32;
    // AABB of rectangle rotated about center: |w·cos| + |h·sin|, |w·sin| + |h·cos|.
    let need_w = (bw * c + bh * s).ceil().max(bw).round() as u32;
    let need_h = (bw * s + bh * c).ceil().max(bh).round() as u32;
    (
        need_w.max(box_w).max(1).min(8192),
        need_h.max(box_h).max(1).min(8192),
    )
}

/// Rotate RGBA image around center; expands canvas to fit. Degrees clockwise.
fn rotate_rgba_image(src: &RgbaImage, degrees: f32) -> RgbaImage {
    let rad = degrees.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());
    let sw = src.width() as f32;
    let sh = src.height() as f32;
    let cx = sw * 0.5;
    let cy = sh * 0.5;

    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for (x, y) in corners {
        let dx = x - cx;
        let dy = y - cy;
        let rx = dx * cos - dy * sin;
        let ry = dx * sin + dy * cos;
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let out_w = ((max_x - min_x).ceil() as u32).max(1).min(8192);
    let out_h = ((max_y - min_y).ceil() as u32).max(1).min(8192);
    let mut out = RgbaImage::from_pixel(out_w, out_h, Rgba([0, 0, 0, 0]));
    let ocx = out_w as f32 * 0.5;
    let ocy = out_h as f32 * 0.5;
    // Inverse rotation sampling
    let inv_cos = cos;
    let inv_sin = -sin;

    for oy in 0..out_h {
        for ox in 0..out_w {
            let dx = ox as f32 + 0.5 - ocx;
            let dy = oy as f32 + 0.5 - ocy;
            let sx = dx * inv_cos - dy * inv_sin + cx;
            let sy = dx * inv_sin + dy * inv_cos + cy;
            if sx < 0.0 || sy < 0.0 || sx >= sw || sy >= sh {
                continue;
            }
            let ix = sx.floor() as u32;
            let iy = sy.floor() as u32;
            if ix < src.width() && iy < src.height() {
                *out.get_pixel_mut(ox, oy) = *src.get_pixel(ix, iy);
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CollageFillMode {
    Cover,
    Contain,
}

fn parse_fill_mode(raw: Option<&str>) -> CollageFillMode {
    match raw.unwrap_or("cover").trim().to_ascii_lowercase().as_str() {
        "contain" => CollageFillMode::Contain,
        _ => CollageFillMode::Cover,
    }
}

fn collage_grid_dims(template: &str, image_count: usize) -> Result<(u32, u32), String> {
    let n = image_count.max(1).min(12) as u32;
    match template.trim() {
        "2" | "2h" => Ok((2, 1)),
        "2v" => Ok((1, 2)),
        "3" | "3a" | "3b" => Ok((3, 1)),
        "4" | "4m" => Ok((2, 2)),
        "6" | "6m" => Ok((3, 2)),
        "9" => Ok((3, 3)),
        "strip-h" | "strip_h" | "strip-horizontal" => Ok((n, 1)),
        "strip-v" | "strip_v" | "strip-vertical" => Ok((1, n)),
        "cells" => Err("cells template requires cells payload".to_string()),
        other => Err(format!("Unsupported collage template: {}", other)),
    }
}

fn parse_hex_color(input: &str) -> Option<[u8; 3]> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<_> = rest.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some([r, g, b]);
        }
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some([r, g, b]);
    }
    if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
        return Some([r, g, b]);
    }
    None
}

async fn load_image_for_edit(file_path: &str) -> Result<DynamicImage, String> {
    load_image_for_layout(file_path, 4096).await
}

/// Load/orient an image and downscale so max(width,height) <= max_edge.
/// Used by print layout where each photo only needs cell-sized pixels on the sheet.
async fn load_image_for_layout(file_path: &str, max_edge: u32) -> Result<DynamicImage, String> {
    let file_type = t_utils::get_file_type(file_path).unwrap_or(0);
    if file_type != 1 && file_type != 3 {
        return Err("Not an image file".to_string());
    }

    let mut img = if should_generate_preview_for_file(file_path, file_type) {
        let preview = get_generated_preview_bytes(file_path)
            .await?
            .ok_or_else(|| "Failed to resolve editable preview image".to_string())?;
        let img = image::load_from_memory(&preview)
            .map_err(|e| format!("Failed to decode preview image: {}", e))?;

        #[cfg(target_os = "macos")]
        {
            if is_heic_path(file_path) {
                apply_orientation(img, get_image_orientation(file_path))
            } else {
                img
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            img
        }
    } else {
        let path = Path::new(file_path);
        let img = image::open(path).map_err(|e| e.to_string())?;
        apply_orientation(img, get_image_orientation(file_path))
    };

    let max_edge = max_edge.clamp(64, 8192);
    let (w, h) = img.dimensions();
    let longest = w.max(h).max(1);
    if longest > max_edge {
        let scale = max_edge as f32 / longest as f32;
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        img = img.resize(nw, nh, image::imageops::FilterType::Triangle);
    }
    Ok(img)
}

/// Decode edge budget for a target cell: ~1.25× longest side, clamped.
fn collage_source_max_edge(max_cw: u32, max_ch: u32) -> u32 {
    let edge = max_cw.max(max_ch).saturating_mul(5).saturating_add(4) / 4;
    edge.clamp(64, 4096)
}

/// Shrink a decoded image so Cover-fit into the largest cell only needs ~1:1 pixels.
/// Upscaling is never done here (small sources stay as-is; draw_fitted_cell may enlarge).
fn downscale_image_for_cover_cells(img: DynamicImage, max_cw: u32, max_ch: u32) -> DynamicImage {
    downscale_image_for_fit_cells(img, max_cw, max_ch, CollageFillMode::Cover)
}

/// Downscale for cover or contain into max cell size (never upscale).
fn downscale_image_for_fit_cells(
    img: DynamicImage,
    max_cw: u32,
    max_ch: u32,
    fill: CollageFillMode,
) -> DynamicImage {
    let max_cw = max_cw.max(1);
    let max_ch = max_ch.max(1);
    let (iw, ih) = img.dimensions();
    if iw == 0 || ih == 0 {
        return img;
    }
    // Cover: s = max(cw/iw, ch/ih); Contain: s = min(cw/iw, ch/ih).
    // If s < 1, source is larger than needed; resize by s so intermediate ≈ cell size.
    let s = match fill {
        CollageFillMode::Cover => (max_cw as f32 / iw as f32).max(max_ch as f32 / ih as f32),
        CollageFillMode::Contain => (max_cw as f32 / iw as f32).min(max_ch as f32 / ih as f32),
    };
    if s >= 0.999 {
        return img;
    }
    let nw = ((iw as f32) * s).round().max(1.0) as u32;
    let nh = ((ih as f32) * s).round().max(1.0) as u32;
    // For cover, keep at least cell dimensions on the covering axes.
    let (nw, nh) = match fill {
        CollageFillMode::Cover => (
            nw.max(max_cw).min(iw),
            nh.max(max_ch).min(ih),
        ),
        CollageFillMode::Contain => (nw.min(iw), nh.min(ih)),
    };
    if nw >= iw && nh >= ih {
        return img;
    }
    img.resize(nw.max(1), nh.max(1), image::imageops::FilterType::Triangle)
}

fn draw_fitted_cell(
    canvas: &mut RgbaImage,
    img: &DynamicImage,
    x: u32,
    y: u32,
    cw: u32,
    ch: u32,
    fill: CollageFillMode,
    radius: u32,
    stroke_w: u32,
    stroke: [u8; 3],
) {
    if cw == 0 || ch == 0 {
        return;
    }

    let iw = img.width().max(1) as f32;
    let ih = img.height().max(1) as f32;
    let scale = match fill {
        CollageFillMode::Cover => (cw as f32 / iw).max(ch as f32 / ih),
        CollageFillMode::Contain => (cw as f32 / iw).min(ch as f32 / ih),
    };
    let nw = (iw * scale).round().max(1.0) as u32;
    let nh = (ih * scale).round().max(1.0) as u32;
    let resized = img.resize_exact(nw, nh, image::imageops::FilterType::Triangle);

    let mut cell = RgbaImage::from_pixel(cw, ch, Rgba([0, 0, 0, 0]));
    match fill {
        CollageFillMode::Cover => {
            let ox = nw.saturating_sub(cw) / 2;
            let oy = nh.saturating_sub(ch) / 2;
            let take_w = cw.min(nw);
            let take_h = ch.min(nh);
            let cropped = resized.crop_imm(ox, oy, take_w, take_h).to_rgba8();
            image::imageops::overlay(&mut cell, &cropped, 0, 0);
        }
        CollageFillMode::Contain => {
            let ox = cw.saturating_sub(nw) / 2;
            let oy = ch.saturating_sub(nh) / 2;
            let rgba = resized.to_rgba8();
            image::imageops::overlay(&mut cell, &rgba, ox as i64, oy as i64);
        }
    }

    if radius > 0 || stroke_w > 0 {
        apply_rounded_mask_and_stroke(&mut cell, radius, stroke_w, stroke);
    }

    image::imageops::overlay(canvas, &cell, x as i64, y as i64);
}

/// Soft rounded-rect mask + optional inset stroke (export canvas pixels).
fn apply_rounded_mask_and_stroke(
    cell: &mut RgbaImage,
    radius: u32,
    stroke_w: u32,
    stroke: [u8; 3],
) {
    let w = cell.width() as f32;
    let h = cell.height() as f32;
    let r = (radius as f32).min(w.min(h) * 0.5).max(0.0);
    let stroke_f = stroke_w as f32;

    for py in 0..cell.height() {
        for px in 0..cell.width() {
            let d = rounded_rect_sdf(px as f32 + 0.5, py as f32 + 0.5, w, h, r);
            let pixel = cell.get_pixel_mut(px, py);

            // Outside rounded rect → transparent
            if d > 0.5 {
                pixel[3] = 0;
                continue;
            }

            // Inset stroke band
            if stroke_f > 0.0 && d > -stroke_f {
                let t = ((d + stroke_f) / stroke_f).clamp(0.0, 1.0);
                // Blend stroke over content near the edge
                let a = pixel[3] as f32 / 255.0;
                let sr = stroke[0] as f32;
                let sg = stroke[1] as f32;
                let sb = stroke[2] as f32;
                pixel[0] = (sr * t + pixel[0] as f32 * (1.0 - t) * a).clamp(0.0, 255.0) as u8;
                pixel[1] = (sg * t + pixel[1] as f32 * (1.0 - t) * a).clamp(0.0, 255.0) as u8;
                pixel[2] = (sb * t + pixel[2] as f32 * (1.0 - t) * a).clamp(0.0, 255.0) as u8;
                pixel[3] = 255;
            }
        }
    }
}

/// Signed distance to rounded rectangle centered in [0,w]x[0,h]. Negative = inside.
fn rounded_rect_sdf(px: f32, py: f32, w: f32, h: f32, radius: f32) -> f32 {
    let half_w = w * 0.5;
    let half_h = h * 0.5;
    let cx = px - half_w;
    let cy = py - half_h;
    let r = radius.min(half_w).min(half_h);
    let qx = cx.abs() - (half_w - r);
    let qy = cy.abs() - (half_h - r);
    let outside = (qx.max(0.0).hypot(qy.max(0.0))).max(0.0);
    let inside = qx.max(qy).min(0.0);
    outside + inside - r
}

/// copy an edited image to clipboard
pub async fn copy_edited_image_to_clipboard(params: EditParams) -> bool {
    if let Ok(img) = get_edited_image(&params).await {
        return copy_image_to_clipboard(img);
    }
    false
}

/// get an edited image
async fn get_edited_image(params: &EditParams) -> Result<DynamicImage, String> {
    let file_type = t_utils::get_file_type(&params.source_file_path).unwrap_or(0);
    let mut img = if should_generate_preview_for_file(&params.source_file_path, file_type) {
        let preview = get_generated_preview_bytes(&params.source_file_path)
            .await?
            .ok_or_else(|| "Failed to resolve editable preview image".to_string())?;
        let img = image::load_from_memory(&preview)
            .map_err(|e| format!("Failed to decode editable preview image: {}", e))?;

        #[cfg(target_os = "macos")]
        {
            if is_heic_path(&params.source_file_path) {
                apply_orientation(img, params.orientation)
            } else {
                img
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            img
        }
    } else {
        let path = Path::new(&params.source_file_path);
        let mut img = image::open(path).map_err(|e| e.to_string())?;
        // orientation adjustment based on exif orientation value
        img = apply_orientation(img, params.orientation);
        img
    };

    // 1. Flip
    if params.flip_horizontal {
        img = img.fliph();
    }
    if params.flip_vertical {
        img = img.flipv();
    }

    // 2. Rotate
    match params.rotate {
        90 => img = img.rotate90(),
        180 => img = img.rotate180(),
        270 => img = img.rotate270(),
        -90 => img = img.rotate270(),
        -180 => img = img.rotate180(),
        -270 => img = img.rotate90(),
        _ => {}
    }

    // 3. Crop
    if params.crop.width > 0 && params.crop.height > 0 {
        img = img.crop_imm(
            params.crop.x,
            params.crop.y,
            params.crop.width,
            params.crop.height,
        );
    }

    // 4. Resize — pick filter by scale factor (Lanczos3 is very expensive for large photos).
    if let (Some(w), Some(h)) = (params.resize.width, params.resize.height) {
        if w > 0 && h > 0 && (w != img.width() || h != img.height()) {
            let filter = pick_edit_resize_filter(img.width(), img.height(), w, h);
            img = img.resize_exact(w, h, filter);
        }
    }

    // 5. Adjustments & Filters
    // NOTE: Implementations match CSS filter semantics so preview == saved result.
    // Brightness/contrast (and saturation when no blur/hue) are fused into one pass.
    img = apply_edit_color_adjustments(
        img,
        params.brightness,
        params.contrast,
        params.blur,
        params.hue_rotate,
        params.saturation,
        params.filter.as_deref(),
    );

    Ok(img)
}

/// Choose a resize filter that balances quality vs cost for the edit/export path.
/// Heavy downscales (common for photo-size exports) use Triangle; mild changes use CatmullRom.
fn pick_edit_resize_filter(
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> image::imageops::FilterType {
    let src_area = (src_w as u64).saturating_mul(src_h as u64);
    let dst_area = (dst_w as u64).saturating_mul(dst_h as u64).max(1);
    if src_area > dst_area.saturating_mul(4) {
        // e.g. 24MP → ID/print size: Triangle is much faster and adequate for heavy downscales.
        image::imageops::FilterType::Triangle
    } else {
        // Mild downscale / upscale: CatmullRom is close to Lanczos quality at lower cost.
        image::imageops::FilterType::CatmullRom
    }
}

/// Apply brightness/contrast/blur/hue/saturation/filter with fewer full-image passes.
fn apply_edit_color_adjustments(
    mut img: DynamicImage,
    brightness: Option<i32>,
    contrast: Option<f32>,
    blur: Option<f32>,
    hue_rotate: Option<i32>,
    saturation: Option<f32>,
    filter: Option<&str>,
) -> DynamicImage {
    let brightness_factor = brightness
        .filter(|&b| b != 0)
        .map(|b| (100 + b) as f32 / 100.0);
    let contrast_factor = contrast.filter(|&c| c != 0.0).map(|c| (100.0 + c) / 100.0);
    let blur_sigma = blur.filter(|&s| s > 0.0);
    let hue = hue_rotate.filter(|&h| h != 0);
    let sat = saturation.filter(|&s| (s - 1.0).abs() > f32::EPSILON);
    let filter_name = filter.filter(|name| !name.is_empty() && *name != "none");

    // Fuse B/C/(optional sat) when later spatial ops are absent — one RGBA conversion + scan.
    let can_fuse_sat = blur_sigma.is_none() && hue.is_none();
    if brightness_factor.is_some() || contrast_factor.is_some() || (can_fuse_sat && sat.is_some()) {
        let mut rgba = img.to_rgba8();
        let b_factor = brightness_factor.unwrap_or(1.0);
        let c_factor = contrast_factor.unwrap_or(1.0);
        let s_factor = if can_fuse_sat {
            sat.unwrap_or(1.0)
        } else {
            1.0
        };
        let apply_sat = can_fuse_sat && sat.is_some();

        for pixel in rgba.pixels_mut() {
            let mut r = pixel[0] as f32;
            let mut g = pixel[1] as f32;
            let mut b = pixel[2] as f32;

            if brightness_factor.is_some() {
                r *= b_factor;
                g *= b_factor;
                b *= b_factor;
            }
            if contrast_factor.is_some() {
                r = (r - 128.0) * c_factor + 128.0;
                g = (g - 128.0) * c_factor + 128.0;
                b = (b - 128.0) * c_factor + 128.0;
            }
            if apply_sat {
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                r = luma + s_factor * (r - luma);
                g = luma + s_factor * (g - luma);
                b = luma + s_factor * (b - luma);
            }

            pixel[0] = r.clamp(0.0, 255.0) as u8;
            pixel[1] = g.clamp(0.0, 255.0) as u8;
            pixel[2] = b.clamp(0.0, 255.0) as u8;
        }
        img = DynamicImage::ImageRgba8(rgba);
    }

    if let Some(sigma) = blur_sigma {
        img = img.blur(sigma);
    }
    if let Some(degrees) = hue {
        img = img.huerotate(degrees);
    }

    // Saturation after blur/hue when those ops ran (preserve original filter order).
    if !can_fuse_sat {
        if let Some(saturation) = sat {
            let mut rgba = img.to_rgba8();
            for pixel in rgba.pixels_mut() {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                pixel[0] = (luma + saturation * (r - luma)).clamp(0.0, 255.0) as u8;
                pixel[1] = (luma + saturation * (g - luma)).clamp(0.0, 255.0) as u8;
                pixel[2] = (luma + saturation * (b - luma)).clamp(0.0, 255.0) as u8;
            }
            img = DynamicImage::ImageRgba8(rgba);
        }
    }

    if let Some(filter) = filter_name {
        match filter {
            "grayscale" => {
                img = DynamicImage::ImageLuma8(img.to_luma8());
            }
            "invert" => {
                img.invert();
            }
            "sepia" => {
                let mut rgba = img.to_rgba8();
                for pixel in rgba.pixels_mut() {
                    let r = pixel[0] as f32;
                    let g = pixel[1] as f32;
                    let b = pixel[2] as f32;
                    pixel[0] = (r * 0.393 + g * 0.769 + b * 0.189).min(255.0) as u8;
                    pixel[1] = (r * 0.349 + g * 0.686 + b * 0.168).min(255.0) as u8;
                    pixel[2] = (r * 0.272 + g * 0.534 + b * 0.131).min(255.0) as u8;
                }
                img = DynamicImage::ImageRgba8(rgba);
            }
            _ => {}
        }
    }

    img
}

#[cfg(target_os = "macos")]
pub fn get_thumbnail_with_sips(
    file_path: &str,
    thumbnail_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let temp_dir = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System clock error: {}", e))?
        .subsec_nanos();
    let temp_file = temp_dir.join(format!("thumb_{}.jpg", nanos));
    let temp_output = temp_file.to_str().ok_or("Invalid temp path")?;

    let output = Command::new("sips")
        .arg("--resampleHeight")
        .arg(thumbnail_size.to_string())
        .arg("-s")
        .arg("format")
        .arg("jpeg")
        .arg(file_path)
        .arg("--out")
        .arg(temp_output)
        .output()
        .map_err(|e| format!("Failed to run sips: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "sips failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let data = fs::read(&temp_file).map_err(|e| format!("Failed to read temp file: {}", e))?;
    let _ = fs::remove_file(temp_file);

    Ok(Some(data))
}

#[cfg(target_os = "macos")]
pub fn get_dimensions_with_sips(file_path: &str) -> Result<Option<(u32, u32)>, String> {
    let output = Command::new("sips")
        .arg("-g")
        .arg("pixelWidth")
        .arg("-g")
        .arg("pixelHeight")
        .arg(file_path)
        .output()
        .map_err(|e| format!("Failed to run sips for dimensions: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "sips dimension probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("pixelWidth:") {
            width = value.trim().parse::<u32>().ok();
        } else if let Some(value) = line.strip_prefix("pixelHeight:") {
            height = value.trim().parse::<u32>().ok();
        }
    }

    match (width, height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => Ok(Some((width, height))),
        _ => Ok(None),
    }
}

#[cfg(target_os = "macos")]
pub fn get_heic_thumbnail_with_sips(
    file_path: &str,
    thumbnail_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    get_thumbnail_with_sips(file_path, thumbnail_size)
}

const FILE_IMAGE_RESULT_CACHE_MAX: usize = 8;

#[derive(Clone)]
struct FileImageCacheEntry {
    signature: (u64, u128),
    data: Vec<u8>,
}

struct FileImageResultCache {
    entries: HashMap<String, FileImageCacheEntry>,
    order: VecDeque<String>,
}

impl FileImageResultCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, file_path: &str, signature: (u64, u128)) -> Option<Vec<u8>> {
        let entry = self.entries.get(file_path)?;
        if entry.signature != signature {
            self.entries.remove(file_path);
            self.order.retain(|item| item != file_path);
            return None;
        }

        self.order.retain(|item| item != file_path);
        self.order.push_back(file_path.to_string());
        Some(entry.data.clone())
    }

    fn insert(&mut self, file_path: String, signature: (u64, u128), data: Vec<u8>) {
        self.entries
            .insert(file_path.clone(), FileImageCacheEntry { signature, data });
        self.order.retain(|item| item != &file_path);
        self.order.push_back(file_path);

        while self.order.len() > FILE_IMAGE_RESULT_CACHE_MAX {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

static FILE_IMAGE_RESULT_CACHE: Lazy<Mutex<FileImageResultCache>> =
    Lazy::new(|| Mutex::new(FileImageResultCache::new()));

fn get_file_signature(file_path: &str) -> Result<(u64, u128), String> {
    let metadata = fs::metadata(file_path)
        .map_err(|e| format!("Failed to read file metadata for cache: {}", e))?;
    let modified = metadata
        .modified()
        .map_err(|e| format!("Failed to read file modified time for cache: {}", e))?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Invalid file modified time for cache: {}", e))?
        .as_millis();
    Ok((metadata.len(), modified))
}

pub async fn get_file_image_bytes_cached(file_path: &str) -> Result<Vec<u8>, String> {
    let file_type = t_utils::get_file_type(file_path).unwrap_or(0);
    let cache_signature = if should_generate_preview_for_file(file_path, file_type) {
        Some(get_file_signature(file_path)?)
    } else {
        None
    };

    if let Some(signature) = cache_signature {
        if let Ok(mut cache) = FILE_IMAGE_RESULT_CACHE.lock() {
            if let Some(cached) = cache.get(file_path, signature) {
                return Ok(cached);
            }
        }
    }

    let image_data = if file_type == 3 {
        get_raw_preview_image(file_path)?
            .ok_or_else(|| format!("Failed to resolve RAW preview image: {}", file_path))?
    } else if t_jxl::is_jxl_path(file_path) {
        t_jxl::get_jxl_preview_image(file_path, 4096)?
            .ok_or_else(|| format!("Failed to resolve JXL preview image: {}", file_path))?
    } else if is_heic_path(file_path) {
        #[cfg(target_os = "macos")]
        {
            get_thumbnail_with_sips(file_path, 4096)?
                .ok_or_else(|| format!("Failed to resolve HEIC preview image: {}", file_path))?
        }
        #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
        {
            crate::t_heif::get_heif_preview(file_path, get_image_orientation(file_path), 4096)?
                .ok_or_else(|| format!("Failed to resolve HEIC preview image: {}", file_path))?
        }
        #[cfg(all(not(target_os = "macos"), not(lap_has_libheif)))]
        {
            crate::t_video::get_video_thumbnail(file_path, 4096, None, None)
                .await?
                .ok_or_else(|| format!("Failed to resolve HEIC preview image: {}", file_path))?
        }
    } else if is_ffmpeg_backed_image_path(file_path) {
        crate::t_video::get_video_thumbnail(file_path, 4096, None, None)
            .await?
            .ok_or_else(|| {
                format!(
                    "Failed to resolve FFmpeg-backed preview image: {}",
                    file_path
                )
            })?
    } else if cfg!(target_os = "linux") && is_avif_path(file_path) {
        get_image_thumbnail(file_path, get_image_orientation(file_path), 4096)?
            .ok_or_else(|| format!("Failed to resolve AVIF preview image: {}", file_path))?
    } else if crate::t_libraw::is_tiff_path(file_path) {
        match get_raw_preview_image(file_path) {
            Ok(Some(data)) => data,
            _ => tokio::fs::read(file_path)
                .await
                .map_err(|e| format!("Failed to read the image: {}", e))?,
        }
    } else {
        tokio::fs::read(file_path)
            .await
            .map_err(|e| format!("Failed to read the image: {}", e))?
    };

    if let Some(signature) = cache_signature {
        if let Ok(mut cache) = FILE_IMAGE_RESULT_CACHE.lock() {
            cache.insert(file_path.to_string(), signature, image_data.clone());
        }
    }

    Ok(image_data)
}

/// --- Batch processing (Phase C) ---

static BATCH_CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

pub fn cancel_batch_process() {
    BATCH_CANCEL_FLAG.store(true, Ordering::SeqCst);
}

pub fn reset_batch_cancel_flag() {
    BATCH_CANCEL_FLAG.store(false, Ordering::SeqCst);
}

fn batch_is_cancelled() -> bool {
    BATCH_CANCEL_FLAG.load(Ordering::SeqCst)
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BatchFileInput {
    #[serde(rename = "sourceFilePath")]
    source_file_path: String,
    #[serde(default)]
    orientation: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BatchActionSpec {
    /// resize | crop | rotate | flip | brightness | contrast | saturation | hue | blur | filter | border | expand | watermark | text
    #[serde(rename = "type")]
    action_type: String,
    // resize
    mode: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    percent: Option<f32>,
    // crop
    #[serde(rename = "presetId")]
    preset_id: Option<String>,
    portrait: Option<bool>,
    #[serde(rename = "applyTargetPixels")]
    apply_target_pixels: Option<bool>,
    ratio_w: Option<f32>,
    ratio_h: Option<f32>,
    px_w: Option<u32>,
    px_h: Option<u32>,
    // rotate / flip / adjust
    degrees: Option<i32>,
    axis: Option<String>,
    value: Option<f32>,
    filter: Option<String>,
    // border / expand / watermark / text (Phase C2)
    color: Option<String>,
    top: Option<u32>,
    right: Option<u32>,
    bottom: Option<u32>,
    left: Option<u32>,
    #[serde(rename = "imagePath")]
    image_path: Option<String>,
    position: Option<String>,
    scale: Option<f32>,
    opacity: Option<f32>,
    margin: Option<u32>,
    text: Option<String>,
    #[serde(rename = "fontSize")]
    font_size: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchProcessParams {
    files: Vec<BatchFileInput>,
    actions: Vec<BatchActionSpec>,
    #[serde(rename = "outputDir")]
    output_dir: Option<String>,
    /// saveAs | overwrite
    #[serde(rename = "outputMode")]
    output_mode: String,
    #[serde(rename = "outputFormat")]
    output_format: String,
    quality: Option<u8>,
    /// original | prefix | suffix | sequence
    #[serde(rename = "nameMode")]
    name_mode: String,
    prefix: Option<String>,
    suffix: Option<String>,
    /// skip | overwrite | rename
    #[serde(rename = "overwritePolicy")]
    overwrite_policy: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchProcessResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: bool,
    pub errors: Vec<String>,
    /// Absolute paths written successfully (saveAs or overwrite). Failed/skipped omitted.
    #[serde(rename = "outputPaths")]
    pub output_paths: Vec<String>,
}

/// Cap concurrent batch workers so decode/encode does not thrash disk or RAM.
/// Aligns with indexing budget style: ~70% of logical cores, clamped.
fn batch_worker_limit() -> usize {
    let logical_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let n = ((logical_cores as f64) * 0.7).floor().max(1.0) as usize;
    n.clamp(2, 8)
}

/// One planned unit of batch work after serial destination resolution.
struct BatchWorkItem {
    index: usize,
    file: BatchFileInput,
    /// None means skip (exists + skip policy).
    dest: Option<String>,
}

/// Run batch edits with bounded concurrency. Emits `batch-process-progress` events.
/// Destination paths are resolved serially first (so rename/collision is race-free),
/// then decode/process/write run on a JoinSet worker pool.
/// Cancel is cooperative: stop spawning, abort in-flight tasks, wait for join.
pub async fn batch_process_images(
    app_handle: tauri::AppHandle,
    params: BatchProcessParams,
) -> Result<BatchProcessResult, String> {
    use tauri::Emitter;
    use tokio::task::JoinSet;

    reset_batch_cancel_flag();
    let total = params.files.len();
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut cancelled = false;
    let mut errors: Vec<String> = Vec::new();
    let mut output_paths: Vec<String> = Vec::new();
    let mut completed = 0usize;

    let output_mode = params.output_mode.to_ascii_lowercase();
    let overwrite_policy = params.overwrite_policy.to_ascii_lowercase();
    let fmt = params.output_format.to_ascii_lowercase();
    let ext = match fmt.as_str() {
        "png" => "png",
        "webp" => "webp",
        _ => "jpg",
    };
    let quality = params.quality.unwrap_or(90).clamp(1, 100);
    let name_mode = params.name_mode.as_str();
    let prefix = params.prefix.as_deref().unwrap_or("out");
    let suffix = params.suffix.as_deref().unwrap_or("edit");
    let output_dir = params.output_dir.as_deref();
    let actions = params.actions.clone();

    if output_mode == "saveas" {
        let dir = output_dir
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "outputDir is required for saveAs mode".to_string())?;
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create output folder: {}", e))?;
    }

    let _ = app_handle.emit(
        "batch-process-progress",
        serde_json::json!({
            "current": 0,
            "total": total,
            "status": "start",
            "filePath": "",
            "message": ""
        }),
    );

    // Serial path planning: reserve destinations so concurrent rename cannot collide.
    let mut reserved_dests: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut work: Vec<BatchWorkItem> = Vec::with_capacity(total);
    for (index, file) in params.files.iter().enumerate() {
        let src = file.source_file_path.trim();
        if src.is_empty() {
            failed += 1;
            completed += 1;
            errors.push(format!("#{}: empty path", index + 1));
            continue;
        }
        match resolve_batch_dest_path(
            src,
            &output_mode,
            output_dir,
            name_mode,
            prefix,
            suffix,
            index,
            ext,
            &overwrite_policy,
            &mut reserved_dests,
        ) {
            Ok(dest) => {
                if dest.is_none() {
                    // Exists + skip policy: count as skipped without a worker.
                    skipped += 1;
                    completed += 1;
                } else {
                    work.push(BatchWorkItem {
                        index,
                        file: file.clone(),
                        dest,
                    });
                }
            }
            Err(err) => {
                failed += 1;
                completed += 1;
                errors.push(format!("{}: {}", src, err));
            }
        }
    }

    let worker_limit = batch_worker_limit().min(work.len().max(1));
    let mut join_set: JoinSet<(usize, String, Result<BatchFileOutcome, String>)> = JoinSet::new();
    let mut next_work = 0usize;
    let work_total = work.len();

    while completed < total || !join_set.is_empty() {
        if batch_is_cancelled() {
            cancelled = true;
            join_set.abort_all();
            break;
        }

        while join_set.len() < worker_limit && next_work < work_total {
            if batch_is_cancelled() {
                cancelled = true;
                break;
            }
            let item = &work[next_work];
            let item_index = item.index;
            next_work += 1;

            let src = item.file.source_file_path.clone();
            let file = item.file.clone();
            let dest = item.dest.clone();
            let actions = actions.clone();
            let ext = ext.to_string();

            let _ = app_handle.emit(
                "batch-process-progress",
                serde_json::json!({
                    "current": completed + join_set.len() + 1,
                    "total": total,
                    "status": "processing",
                    "filePath": src,
                    "message": ""
                }),
            );

            join_set.spawn(async move {
                let outcome = process_one_batch_file(&file, &actions, dest, &ext, quality).await;
                (item_index, src, outcome)
            });
        }

        if cancelled {
            join_set.abort_all();
            break;
        }

        if join_set.is_empty() {
            break;
        }

        match join_set.join_next().await {
            Some(Ok((_index, src, outcome))) => {
                completed += 1;
                match outcome {
                    Ok(BatchFileOutcome::Ok(path)) => {
                        succeeded += 1;
                        output_paths.push(path);
                    }
                    Ok(BatchFileOutcome::Skipped) => skipped += 1,
                    Err(err) => {
                        if err == "cancelled" {
                            cancelled = true;
                            join_set.abort_all();
                        } else {
                            failed += 1;
                            errors.push(format!("{}: {}", src, err));
                            let _ = app_handle.emit(
                                "batch-process-progress",
                                serde_json::json!({
                                    "current": completed,
                                    "total": total,
                                    "status": "error",
                                    "filePath": src,
                                    "message": err
                                }),
                            );
                        }
                    }
                }
                if !cancelled {
                    let _ = app_handle.emit(
                        "batch-process-progress",
                        serde_json::json!({
                            "current": completed,
                            "total": total,
                            "status": "processing",
                            "filePath": src,
                            "message": ""
                        }),
                    );
                }
            }
            Some(Err(join_err)) => {
                if join_err.is_cancelled() {
                    cancelled = true;
                } else {
                    completed += 1;
                    failed += 1;
                    errors.push(format!("worker failed: {}", join_err));
                }
            }
            None => break,
        }
    }

    // Drain stragglers after cancel.
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok((_index, src, outcome)) => {
                completed += 1;
                match outcome {
                    Ok(BatchFileOutcome::Ok(path)) => {
                        succeeded += 1;
                        output_paths.push(path);
                    }
                    Ok(BatchFileOutcome::Skipped) => skipped += 1,
                    Err(err) if err == "cancelled" => {}
                    Err(err) => {
                        failed += 1;
                        errors.push(format!("{}: {}", src, err));
                    }
                }
            }
            Err(join_err) if join_err.is_cancelled() => {}
            Err(join_err) => {
                completed += 1;
                failed += 1;
                errors.push(format!("worker failed: {}", join_err));
            }
        }
    }

    let result = BatchProcessResult {
        total,
        succeeded,
        failed,
        skipped,
        cancelled,
        errors,
        output_paths,
    };
    let _ = app_handle.emit(
        "batch-process-progress",
        serde_json::json!({
            "current": completed.min(total),
            "total": total,
            "status": if cancelled { "cancelled" } else { "done" },
            "filePath": "",
            "message": "",
            "result": result
        }),
    );
    Ok(result)
}

enum BatchFileOutcome {
    /// Dest path that was written.
    Ok(String),
    Skipped,
}

async fn process_one_batch_file(
    file: &BatchFileInput,
    actions: &[BatchActionSpec],
    dest: Option<String>,
    ext: &str,
    quality: u8,
) -> Result<BatchFileOutcome, String> {
    let src = file.source_file_path.as_str();
    if dest.is_none() {
        return Ok(BatchFileOutcome::Skipped);
    }
    let dest = dest.unwrap();

    let mut img = load_image_for_edit(src).await?;
    // Prefer EXIF from host when provided; load_image_for_edit already orients for normal open path.
    let _ = file.orientation;

    for action in actions {
        if batch_is_cancelled() {
            return Err("cancelled".to_string());
        }
        img = apply_batch_action(img, action)?;
    }

    if let Some(parent) = Path::new(&dest).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("create dest dir: {}", e))?;
        }
    }

    save_dynamic_image(&img, &dest, ext, quality)?;
    Ok(BatchFileOutcome::Ok(dest))
}

fn resolve_batch_dest_path(
    src: &str,
    output_mode: &str,
    output_dir: Option<&str>,
    name_mode: &str,
    prefix: &str,
    suffix: &str,
    index: usize,
    ext: &str,
    overwrite_policy: &str,
    reserved: &mut std::collections::HashSet<String>,
) -> Result<Option<String>, String> {
    let src_path = Path::new(src);
    let stem = src_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let file_name = match name_mode {
        "prefix" => format!("{}_{}.{}", prefix, stem, ext),
        "suffix" => format!("{}_{}.{}", stem, suffix, ext),
        "sequence" => format!("{}_{:03}.{}", prefix, index + 1, ext),
        _ => format!("{}.{}", stem, ext),
    };

    let dest = if output_mode == "overwrite" {
        // Keep original stem path but force chosen extension in same folder.
        let parent = src_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        parent.join(format!("{}.{}", stem, ext))
    } else {
        let dir = output_dir.ok_or_else(|| "missing outputDir".to_string())?;
        Path::new(dir).join(&file_name)
    };

    let dest_str = dest.to_string_lossy().to_string();
    let taken = |path: &str, reserved: &std::collections::HashSet<String>| {
        reserved.contains(path) || Path::new(path).exists()
    };

    if taken(&dest_str, reserved) {
        match overwrite_policy {
            "overwrite" => {
                reserved.insert(dest_str.clone());
                Ok(Some(dest_str))
            }
            "rename" => {
                let parent = dest.parent().unwrap_or(Path::new("."));
                let stem2 = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                for n in 1..10000 {
                    let candidate = parent.join(format!("{}_{}.{}", stem2, n, ext));
                    let candidate_str = candidate.to_string_lossy().to_string();
                    if !taken(&candidate_str, reserved) {
                        reserved.insert(candidate_str.clone());
                        return Ok(Some(candidate_str));
                    }
                }
                Err("Could not find free rename path".to_string())
            }
            _ => Ok(None), // skip
        }
    } else {
        reserved.insert(dest_str.clone());
        Ok(Some(dest_str))
    }
}

fn save_dynamic_image(
    img: &DynamicImage,
    dest: &str,
    ext: &str,
    quality: u8,
) -> Result<(), String> {
    let path = Path::new(dest);
    match ext {
        "png" => img
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(|e| e.to_string()),
        "webp" => img
            .save_with_format(path, image::ImageFormat::WebP)
            .map_err(|e| e.to_string()),
        _ => {
            let rgb = img.to_rgb8();
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
            encoder.encode_image(&rgb).map_err(|e| e.to_string())
        }
    }
}

fn apply_batch_action(
    mut img: DynamicImage,
    action: &BatchActionSpec,
) -> Result<DynamicImage, String> {
    let kind = action.action_type.to_ascii_lowercase();
    match kind.as_str() {
        "resize" => {
            let (w, h) = img.dimensions();
            let mode = action.mode.as_deref().unwrap_or("longEdge");
            let (nw, nh) = match mode {
                "percent" => {
                    let p = (action.percent.unwrap_or(100.0) / 100.0).clamp(0.01, 5.0);
                    (
                        ((w as f32) * p).round().max(1.0) as u32,
                        ((h as f32) * p).round().max(1.0) as u32,
                    )
                }
                "width" => {
                    let tw = action.width.unwrap_or(w).max(1);
                    let nh = ((h as f64) * (tw as f64) / (w as f64)).round().max(1.0) as u32;
                    (tw, nh)
                }
                "height" => {
                    let th = action.height.unwrap_or(h).max(1);
                    let nw = ((w as f64) * (th as f64) / (h as f64)).round().max(1.0) as u32;
                    (nw, th)
                }
                "exact" => (
                    action.width.unwrap_or(w).max(1),
                    action.height.unwrap_or(h).max(1),
                ),
                _ => {
                    // longEdge
                    let target = action.width.or(action.height).unwrap_or(w.max(h)).max(1);
                    if w >= h {
                        let nh =
                            ((h as f64) * (target as f64) / (w as f64)).round().max(1.0) as u32;
                        (target, nh)
                    } else {
                        let nw =
                            ((w as f64) * (target as f64) / (h as f64)).round().max(1.0) as u32;
                        (nw, target)
                    }
                }
            };
            if nw != w || nh != h {
                let filter = pick_edit_resize_filter(w, h, nw, nh);
                img = img.resize_exact(nw, nh, filter);
            }
            Ok(img)
        }
        "crop" => {
            let (w, h) = img.dimensions();
            let mut ratio_w = action.ratio_w.unwrap_or(1.0);
            let mut ratio_h = action.ratio_h.unwrap_or(1.0);
            if let (Some(pw), Some(ph)) = (action.px_w, action.px_h) {
                if pw > 0 && ph > 0 {
                    ratio_w = pw as f32;
                    ratio_h = ph as f32;
                }
            }
            // Built-in preset ids when ratios not provided
            if action.ratio_w.is_none() && action.px_w.is_none() {
                if let Some((rw, rh)) = batch_preset_ratio(action.preset_id.as_deref()) {
                    ratio_w = rw;
                    ratio_h = rh;
                }
            }
            if action.portrait.unwrap_or(false) {
                std::mem::swap(&mut ratio_w, &mut ratio_h);
            }
            if ratio_w <= 0.0 || ratio_h <= 0.0 {
                return Ok(img);
            }
            let target_aspect = ratio_w / ratio_h;
            let src_aspect = w as f32 / h as f32;
            let (cw, ch) = if src_aspect > target_aspect {
                let ch = h;
                let cw = ((ch as f32) * target_aspect).round().max(1.0) as u32;
                (cw.min(w), ch)
            } else {
                let cw = w;
                let ch = ((cw as f32) / target_aspect).round().max(1.0) as u32;
                (cw, ch.min(h))
            };
            let x = w.saturating_sub(cw) / 2;
            let y = h.saturating_sub(ch) / 2;
            img = img.crop_imm(x, y, cw, ch);

            if action.apply_target_pixels.unwrap_or(true) {
                if let (Some(mut pw), Some(mut ph)) = (action.px_w, action.px_h) {
                    if action.portrait.unwrap_or(false) && pw > 0 && ph > 0 && pw != ph {
                        std::mem::swap(&mut pw, &mut ph);
                    }
                    if pw > 0 && ph > 0 {
                        let filter = pick_edit_resize_filter(img.width(), img.height(), pw, ph);
                        img = img.resize_exact(pw, ph, filter);
                    }
                } else if let Some((pw, ph)) = batch_preset_pixels(
                    action.preset_id.as_deref(),
                    action.portrait.unwrap_or(false),
                ) {
                    let filter = pick_edit_resize_filter(img.width(), img.height(), pw, ph);
                    img = img.resize_exact(pw, ph, filter);
                }
            }
            Ok(img)
        }
        "rotate" => {
            let d = action.degrees.unwrap_or(90);
            img = match d.rem_euclid(360) {
                90 => img.rotate90(),
                180 => img.rotate180(),
                270 => img.rotate270(),
                _ => img,
            };
            Ok(img)
        }
        "flip" => {
            match action.axis.as_deref().unwrap_or("horizontal") {
                "vertical" => img = img.flipv(),
                _ => img = img.fliph(),
            }
            Ok(img)
        }
        "brightness" => {
            let v = action.value.unwrap_or(0.0).round() as i32;
            Ok(apply_edit_color_adjustments(
                img,
                Some(v).filter(|&x| x != 0),
                None,
                None,
                None,
                None,
                None,
            ))
        }
        "contrast" => {
            let v = action.value.unwrap_or(0.0);
            Ok(apply_edit_color_adjustments(
                img,
                None,
                Some(v).filter(|&x| x != 0.0),
                None,
                None,
                None,
                None,
            ))
        }
        "saturation" => {
            // Frontend uses 0–200 (%); host uses multiplier with 1.0 normal.
            let percent = action.value.unwrap_or(100.0);
            let mult = (percent / 100.0).clamp(0.0, 3.0);
            Ok(apply_edit_color_adjustments(
                img,
                None,
                None,
                None,
                None,
                Some(mult),
                None,
            ))
        }
        "hue" => {
            let v = action.value.unwrap_or(0.0).round() as i32;
            Ok(apply_edit_color_adjustments(
                img,
                None,
                None,
                None,
                Some(v).filter(|&x| x != 0),
                None,
                None,
            ))
        }
        "blur" => {
            let v = action.value.unwrap_or(0.0).max(0.0);
            Ok(apply_edit_color_adjustments(
                img,
                None,
                None,
                Some(v).filter(|&x| x > 0.0),
                None,
                None,
                None,
            ))
        }
        "filter" => {
            let f = action.filter.as_deref();
            Ok(apply_edit_color_adjustments(
                img, None, None, None, None, None, f,
            ))
        }
        "border" => apply_batch_border(img, action),
        "expand" => apply_batch_expand(img, action),
        "watermark" => apply_batch_watermark(img, action),
        "text" => apply_batch_text(img, action),
        other => Err(format!("Unknown batch action: {}", other)),
    }
}

fn batch_parse_color(raw: Option<&str>, default: [u8; 3]) -> [u8; 3] {
    parse_hex_color(raw.unwrap_or("")).unwrap_or(default)
}

fn batch_anchor_xy(
    position: &str,
    canvas_w: u32,
    canvas_h: u32,
    item_w: u32,
    item_h: u32,
    margin: u32,
) -> (i64, i64) {
    let mw = margin.min(canvas_w / 2);
    let mh = margin.min(canvas_h / 2);
    let max_x = canvas_w.saturating_sub(item_w);
    let max_y = canvas_h.saturating_sub(item_h);
    let (x, y) = match position {
        "top-left" => (mw, mh),
        "top-right" => (max_x.saturating_sub(mw), mh),
        "bottom-left" => (mw, max_y.saturating_sub(mh)),
        "top" => (max_x / 2, mh),
        "bottom" => (max_x / 2, max_y.saturating_sub(mh)),
        "left" => (mw, max_y / 2),
        "right" => (max_x.saturating_sub(mw), max_y / 2),
        "center" => (max_x / 2, max_y / 2),
        _ => (max_x.saturating_sub(mw), max_y.saturating_sub(mh)),
    };
    (x as i64, y as i64)
}

fn apply_batch_border(img: DynamicImage, action: &BatchActionSpec) -> Result<DynamicImage, String> {
    let thickness = action
        .width
        .or(action.value.map(|v| v.max(0.0) as u32))
        .unwrap_or(16)
        .max(1);
    let color = batch_parse_color(action.color.as_deref(), [255, 255, 255]);
    let (w, h) = img.dimensions();
    let nw = w.saturating_add(thickness.saturating_mul(2)).max(1);
    let nh = h.saturating_add(thickness.saturating_mul(2)).max(1);
    let mut canvas = RgbaImage::from_pixel(nw, nh, Rgba([color[0], color[1], color[2], 255]));
    image::imageops::overlay(
        &mut canvas,
        &img.to_rgba8(),
        thickness as i64,
        thickness as i64,
    );
    Ok(DynamicImage::ImageRgba8(canvas))
}

fn apply_batch_expand(img: DynamicImage, action: &BatchActionSpec) -> Result<DynamicImage, String> {
    let top = action.top.unwrap_or(0);
    let right = action.right.unwrap_or(0);
    let bottom = action.bottom.unwrap_or(0);
    let left = action.left.unwrap_or(0);
    if top == 0 && right == 0 && bottom == 0 && left == 0 {
        return Ok(img);
    }
    let color = batch_parse_color(action.color.as_deref(), [255, 255, 255]);
    let (w, h) = img.dimensions();
    let nw = w.saturating_add(left).saturating_add(right).max(1);
    let nh = h.saturating_add(top).saturating_add(bottom).max(1);
    let mut canvas = RgbaImage::from_pixel(nw, nh, Rgba([color[0], color[1], color[2], 255]));
    image::imageops::overlay(&mut canvas, &img.to_rgba8(), left as i64, top as i64);
    Ok(DynamicImage::ImageRgba8(canvas))
}

fn apply_batch_watermark(
    img: DynamicImage,
    action: &BatchActionSpec,
) -> Result<DynamicImage, String> {
    let path = action
        .image_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "watermark imagePath is required".to_string())?;
    let mark = image::open(Path::new(path)).map_err(|e| format!("open watermark: {}", e))?;
    let (cw, ch) = img.dimensions();
    let short = cw.min(ch).max(1) as f32;
    let scale_pct = action.scale.unwrap_or(18.0).clamp(1.0, 100.0);
    let target = ((short * scale_pct / 100.0).round() as u32).max(8);
    let (mw, mh) = mark.dimensions();
    let (nw, nh) = if mw >= mh {
        let nh = ((mh as f64) * (target as f64) / (mw as f64))
            .round()
            .max(1.0) as u32;
        (target, nh)
    } else {
        let nw = ((mw as f64) * (target as f64) / (mh as f64))
            .round()
            .max(1.0) as u32;
        (nw, target)
    };
    let mut stamp = mark
        .resize_exact(nw, nh, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let opacity = (action.opacity.unwrap_or(70.0).clamp(0.0, 100.0) / 100.0) as f32;
    if opacity < 0.999 {
        for p in stamp.pixels_mut() {
            p[3] = ((p[3] as f32) * opacity).round().clamp(0.0, 255.0) as u8;
        }
    }
    let margin = action.margin.unwrap_or(24);
    let pos = action.position.as_deref().unwrap_or("bottom-right");
    let (x, y) = batch_anchor_xy(pos, cw, ch, stamp.width(), stamp.height(), margin);
    let mut canvas = img.to_rgba8();
    image::imageops::overlay(&mut canvas, &stamp, x, y);
    Ok(DynamicImage::ImageRgba8(canvas))
}

fn apply_batch_text(img: DynamicImage, action: &BatchActionSpec) -> Result<DynamicImage, String> {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

    let text = action
        .text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("PicAiPic");
    let font_size = action.font_size.unwrap_or(36.0).clamp(8.0, 400.0);
    let color = batch_parse_color(action.color.as_deref(), [255, 255, 255]);
    let opacity = (action.opacity.unwrap_or(85.0).clamp(0.0, 100.0) / 100.0) as f32;
    let margin = action.margin.unwrap_or(24);
    let pos = action.position.as_deref().unwrap_or("bottom-right");

    let font_bytes = load_system_font_bytes()?;
    let font = FontRef::try_from_slice(&font_bytes).map_err(|e| format!("font parse: {}", e))?;
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);

    let mut caret = 0.0f32;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let advance = scaled.h_advance(id);
        let glyph = id.with_scale(scale);
        if let Some(outlined) = font.outline_glyph(glyph) {
            let b = outlined.px_bounds();
            min_y = min_y.min(b.min.y);
            max_y = max_y.max(b.max.y);
        }
        caret += advance;
    }
    if !min_y.is_finite() || !max_y.is_finite() {
        min_y = 0.0;
        max_y = font_size;
    }
    let text_w = caret.ceil().max(1.0) as u32;
    let text_h = (max_y - min_y).ceil().max(font_size * 0.8).max(1.0) as u32;
    let pad = 2u32;
    let stamp_w = text_w.saturating_add(pad * 2).max(1);
    let stamp_h = text_h.saturating_add(pad * 2).max(1);
    let mut stamp = RgbaImage::from_pixel(stamp_w, stamp_h, Rgba([0, 0, 0, 0]));

    let mut x = pad as f32;
    let baseline = pad as f32 - min_y;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(x, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, v| {
                if v <= 0.0 {
                    return;
                }
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 {
                    return;
                }
                let px = px as u32;
                let py = py as u32;
                if px >= stamp_w || py >= stamp_h {
                    return;
                }
                let a = ((v * opacity) * 255.0).round().clamp(0.0, 255.0) as u8;
                let pixel = stamp.get_pixel_mut(px, py);
                pixel[0] = color[0];
                pixel[1] = color[1];
                pixel[2] = color[2];
                pixel[3] = a.max(pixel[3]);
            });
        }
        x += scaled.h_advance(id);
    }

    let (cw, ch) = img.dimensions();
    let (ox, oy) = batch_anchor_xy(pos, cw, ch, stamp_w, stamp_h, margin);
    let mut canvas = img.to_rgba8();
    image::imageops::overlay(&mut canvas, &stamp, ox, oy);
    Ok(DynamicImage::ImageRgba8(canvas))
}

fn load_system_font_bytes() -> Result<Vec<u8>, String> {
    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let candidates = [
            format!("{}\\Fonts\\segoeui.ttf", windir),
            format!("{}\\Fonts\\arial.ttf", windir),
            format!("{}\\Fonts\\msyh.ttc", windir),
            format!("{}\\Fonts\\simhei.ttf", windir),
            format!("{}\\Fonts\\simsun.ttc", windir),
        ];
        for p in candidates {
            if let Ok(bytes) = fs::read(&p) {
                return Ok(bytes);
            }
        }
        return Err("No system font found for text watermark".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/PingFang.ttc",
        ];
        for p in candidates {
            if let Ok(bytes) = fs::read(p) {
                return Ok(bytes);
            }
        }
        return Err("No system font found for text watermark".to_string());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let candidates = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        ];
        for p in candidates {
            if let Ok(bytes) = fs::read(p) {
                return Ok(bytes);
            }
        }
        return Err("No system font found for text watermark".to_string());
    }
}

fn batch_preset_ratio(preset_id: Option<&str>) -> Option<(f32, f32)> {
    match preset_id.unwrap_or("") {
        "ratio-1-1" => Some((1.0, 1.0)),
        "ratio-3-2" => Some((3.0, 2.0)),
        "ratio-4-3" => Some((4.0, 3.0)),
        "ratio-16-9" => Some((16.0, 9.0)),
        "photo-1r" => Some((295.0, 413.0)),
        "photo-2r" => Some((413.0, 579.0)),
        "photo-2r-large" => Some((413.0, 626.0)),
        "photo-cn-id" => Some((358.0, 441.0)),
        "photo-passport" => Some((390.0, 567.0)),
        "photo-3r" => Some((1500.0, 1050.0)),
        "photo-4r" => Some((1800.0, 1200.0)),
        "photo-5r" => Some((2100.0, 1500.0)),
        "photo-6r" => Some((2400.0, 1800.0)),
        "photo-8r" => Some((3000.0, 2400.0)),
        "photo-wallet-small" => Some((748.0, 1050.0)),
        "photo-wallet-large" => Some((898.0, 1200.0)),
        _ => None,
    }
}

fn batch_preset_pixels(preset_id: Option<&str>, portrait: bool) -> Option<(u32, u32)> {
    let (w, h) = match preset_id.unwrap_or("") {
        "photo-1r" => (295, 413),
        "photo-2r" => (413, 579),
        "photo-2r-large" => (413, 626),
        "photo-cn-id" => (358, 441),
        "photo-passport" => (390, 567),
        "photo-3r" => (1500, 1050),
        "photo-4r" => (1800, 1200),
        "photo-5r" => (2100, 1500),
        "photo-6r" => (2400, 1800),
        "photo-8r" => (3000, 2400),
        "photo-wallet-small" => (748, 1050),
        "photo-wallet-large" => (898, 1200),
        _ => return None,
    };
    if portrait && w > h {
        Some((h, w))
    } else if !portrait && h > w {
        Some((h, w))
    } else {
        Some((w, h))
    }
}

/// Print layout sheet export (冲印排版)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PrintLayoutCell {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    #[serde(rename = "sourceFilePath")]
    pub source_file_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PrintLayoutExportParams {
    #[serde(rename = "destFilePath")]
    dest_file_path: String,
    #[serde(rename = "outputFormat")]
    output_format: String,
    quality: Option<u8>,
    #[serde(rename = "paperWidth")]
    paper_width: u32,
    #[serde(rename = "paperHeight")]
    paper_height: u32,
    background: String,
    /// draw thin guide lines between cells
    #[serde(rename = "showGuides", default)]
    show_guides: Option<bool>,
    #[serde(rename = "guideColor", default)]
    guide_color: Option<String>,
    cells: Vec<PrintLayoutCell>,
}

pub async fn export_print_layout(params: PrintLayoutExportParams) -> Result<bool, String> {
    // Soft cap: enough for ~300DPI on large paper; avoids pathological multi-GB canvases.
    let out_w = params.paper_width.max(64).min(8000);
    let out_h = params.paper_height.max(64).min(8000);
    let bg = parse_hex_color(&params.background).unwrap_or([255, 255, 255]);
    let mut canvas = RgbaImage::from_pixel(out_w, out_h, Rgba([bg[0], bg[1], bg[2], 255]));

    // Per-source max cell size on this paper sheet. Source pixels larger than that
    // cannot improve the composite — downscale to cell need after decode.
    use std::collections::HashMap;
    let mut path_max_cell: HashMap<String, (u32, u32)> = HashMap::new();
    for cell in &params.cells {
        let p = cell.source_file_path.trim();
        if p.is_empty() || cell.w < 2 || cell.h < 2 {
            continue;
        }
        if cell.x >= out_w || cell.y >= out_h {
            continue;
        }
        let cw = cell.w.min(out_w.saturating_sub(cell.x));
        let ch = cell.h.min(out_h.saturating_sub(cell.y));
        let entry = path_max_cell.entry(p.to_string()).or_insert((0, 0));
        entry.0 = entry.0.max(cw);
        entry.1 = entry.1.max(ch);
    }

    // Decode each unique path once (concurrent). Cap decode edge by largest cell need
    // so a 40MP original used as a 1R tile is not fully retained in RAM.
    let mut cache: HashMap<String, DynamicImage> = HashMap::new();
    if !path_max_cell.is_empty() {
        let mut set = tokio::task::JoinSet::new();
        for (path, (max_cw, max_ch)) in path_max_cell.clone() {
            // Cover-fit may need slightly more than max(cw,ch) on the long side when
            // aspects differ; 1.25× is enough margin without keeping full-res masters.
            let max_edge = max_cw
                .max(max_ch)
                .saturating_mul(5)
                .saturating_add(4)
                / 4; // ceil * 1.25
            let max_edge = max_edge.clamp(64, 4096);
            set.spawn(async move {
                let result = load_image_for_layout(&path, max_edge).await;
                (path, max_cw, max_ch, result)
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((path, max_cw, max_ch, Ok(img))) => {
                    let fitted = downscale_image_for_cover_cells(img, max_cw, max_ch);
                    cache.insert(path, fitted);
                }
                Ok((path, _, _, Err(err))) => {
                    eprintln!("print layout skip {}: {}", path, err);
                }
                Err(err) => {
                    eprintln!("print layout load task failed: {}", err);
                }
            }
        }
    }

    for cell in &params.cells {
        if cell.w < 2 || cell.h < 2 || cell.source_file_path.trim().is_empty() {
            continue;
        }
        if cell.x >= out_w || cell.y >= out_h {
            continue;
        }
        let Some(img) = cache.get(cell.source_file_path.trim()) else {
            continue;
        };
        let cw = cell.w.min(out_w.saturating_sub(cell.x));
        let ch = cell.h.min(out_h.saturating_sub(cell.y));
        draw_fitted_cell(
            &mut canvas,
            img,
            cell.x,
            cell.y,
            cw,
            ch,
            CollageFillMode::Cover,
            0,
            0,
            [0, 0, 0],
        );
    }

    if params.show_guides.unwrap_or(false) {
        let guide = parse_hex_color(params.guide_color.as_deref().unwrap_or("#cccccc"))
            .unwrap_or([200, 200, 200]);
        for cell in &params.cells {
            draw_rect_stroke(
                &mut canvas,
                cell.x,
                cell.y,
                cell.w.min(out_w.saturating_sub(cell.x)),
                cell.h.min(out_h.saturating_sub(cell.y)),
                guide,
                1,
            );
        }
    }

    let path = Path::new(&params.dest_file_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output folder: {}", e))?;
        }
    }
    let format = match params.output_format.to_ascii_lowercase().as_str() {
        "png" => image::ImageFormat::Png,
        "webp" => image::ImageFormat::WebP,
        _ => image::ImageFormat::Jpeg,
    };
    let img = DynamicImage::ImageRgba8(canvas);
    let quality = params.quality.unwrap_or(92).clamp(1, 100);
    let save_ok = if format == image::ImageFormat::Jpeg {
        let rgb = img.to_rgb8();
        match std::fs::File::create(path) {
            Ok(file) => {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
                encoder.encode_image(&rgb).is_ok()
            }
            Err(_) => false,
        }
    } else {
        img.save_with_format(path, format).is_ok()
    };
    if !save_ok {
        return Err("Failed to write print layout image".to_string());
    }
    Ok(true)
}

fn draw_rect_stroke(
    canvas: &mut RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: [u8; 3],
    thickness: u32,
) {
    if w == 0 || h == 0 {
        return;
    }
    let t = thickness.max(1);
    let cw = canvas.width();
    let ch = canvas.height();
    let x2 = x.saturating_add(w).min(cw);
    let y2 = y.saturating_add(h).min(ch);
    let px = Rgba([color[0], color[1], color[2], 255]);
    for yy in y..y2.min(y.saturating_add(t)) {
        for xx in x..x2 {
            canvas.put_pixel(xx, yy, px);
        }
    }
    for yy in y2.saturating_sub(t)..y2 {
        for xx in x..x2 {
            canvas.put_pixel(xx, yy, px);
        }
    }
    for yy in y..y2 {
        for xx in x..x2.min(x.saturating_add(t)) {
            canvas.put_pixel(xx, yy, px);
        }
        for xx in x2.saturating_sub(t)..x2 {
            canvas.put_pixel(xx, yy, px);
        }
    }
}
