use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use std::ptr;
use std::slice;
use std::time::SystemTime;

use image::DynamicImage;

use crate::t_image::resize_dynamic_image_to_jpeg;

// Minimal libheif FFI for primary still decode + HEIC-internal video item/sequence detection.
// We keep this hand-written to avoid pulling bindgen into the build.

#[repr(C)]
#[derive(Clone, Copy)]
struct HeifError {
    code: c_int,
    subcode: c_int,
    message: *const c_char,
}

#[repr(C)]
struct HeifContext(c_void);
#[repr(C)]
struct HeifImageHandle(c_void);
#[repr(C)]
struct HeifImage(c_void);
#[repr(C)]
struct HeifTrack(c_void);

type HeifItemId = u32;
type HeifTrackType = u32;

// libheif v1.21.x enums from heif_image.h
const HEIF_COLORSPACE_RGB: c_int = 1;
const HEIF_CHROMA_INTERLEAVED_RGB: c_int = 10;
const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

const fn heif_fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

const HEIF_ITEM_TYPE_MIME: u32 = heif_fourcc(b'm', b'i', b'm', b'e');
const HEIF_TRACK_TYPE_VIDEO: u32 = heif_fourcc(b'v', b'i', b'd', b'e');
const HEIF_TRACK_TYPE_PICT: u32 = heif_fourcc(b'p', b'i', b'c', b't');

const HEIC_VIDEO_CACHE_MIN_BYTES: u64 = 1024;

/// Result of probing a HEIC container for an embedded motion track.
#[derive(Debug, Clone)]
pub struct HeicEmbeddedVideoInfo {
    /// 0 = mime/video item, 1 = sequence track (ffmpeg demux path).
    pub kind: u8,
    pub item_id: Option<u32>,
    pub track_id: Option<u32>,
}

impl HeicEmbeddedVideoInfo {
    pub fn content_id_marker(&self) -> String {
        match self.kind {
            0 => format!("heifitem:{}", self.item_id.unwrap_or(0)),
            1 => format!("heifseq:{}", self.track_id.unwrap_or(0)),
            _ => "heif:unknown".to_string(),
        }
    }
}

unsafe extern "C" {
    fn heif_context_alloc() -> *mut HeifContext;
    fn heif_context_free(ctx: *mut HeifContext);
    fn heif_context_read_from_file(
        ctx: *mut HeifContext,
        filename: *const c_char,
        options: *const c_void,
    ) -> HeifError;
    fn heif_context_get_primary_image_handle(
        ctx: *mut HeifContext,
        handle: *mut *mut HeifImageHandle,
    ) -> HeifError;
    fn heif_image_handle_release(handle: *mut HeifImageHandle);
    fn heif_image_handle_get_width(handle: *const HeifImageHandle) -> c_int;
    fn heif_image_handle_get_height(handle: *const HeifImageHandle) -> c_int;
    fn heif_image_handle_has_alpha_channel(handle: *const HeifImageHandle) -> c_int;

    fn heif_decode_image(
        handle: *const HeifImageHandle,
        out_img: *mut *mut HeifImage,
        colorspace: c_int,
        chroma: c_int,
        options: *const c_void,
    ) -> HeifError;
    fn heif_image_release(img: *mut HeifImage);

    fn heif_image_get_width(img: *const HeifImage, channel: c_int) -> c_int;
    fn heif_image_get_height(img: *const HeifImage, channel: c_int) -> c_int;
    fn heif_image_get_plane_readonly(
        img: *const HeifImage,
        channel: c_int,
        out_stride: *mut c_int,
    ) -> *const u8;

    // Items API (heif_items.h)
    fn heif_context_get_number_of_items(ctx: *const HeifContext) -> c_int;
    fn heif_context_get_list_of_item_IDs(
        ctx: *const HeifContext,
        id_array: *mut HeifItemId,
        count: c_int,
    ) -> c_int;
    fn heif_item_get_item_type(ctx: *const HeifContext, item_id: HeifItemId) -> u32;
    fn heif_item_get_mime_item_content_type(
        ctx: *const HeifContext,
        item_id: HeifItemId,
    ) -> *const c_char;
    fn heif_item_get_item_data(
        ctx: *const HeifContext,
        item_id: HeifItemId,
        out_compression_format: *mut c_int,
        out_data: *mut *mut u8,
        out_data_size: *mut usize,
    ) -> HeifError;
    fn heif_release_item_data(ctx: *const HeifContext, item_data: *mut *mut u8);

    // Sequences API (heif_sequences.h)
    fn heif_context_has_sequence(ctx: *const HeifContext) -> c_int;
    fn heif_context_number_of_sequence_tracks(ctx: *const HeifContext) -> c_int;
    fn heif_context_get_track_ids(ctx: *const HeifContext, out_track_id_array: *mut u32);
    fn heif_context_get_track(ctx: *const HeifContext, id: u32) -> *mut HeifTrack;
    fn heif_track_release(track: *mut HeifTrack);
    fn heif_track_get_track_handler_type(track: *const HeifTrack) -> HeifTrackType;
}

fn fmt_heif_error(err: HeifError) -> String {
    if err.code == 0 {
        return "ok".to_string();
    }
    unsafe {
        let msg: String = if err.message.is_null() {
            String::new()
        } else {
            std::ffi::CStr::from_ptr(err.message)
                .to_string_lossy()
                .into_owned()
        };
        format!(
            "libheif error code={} subcode={} msg={}",
            err.code, err.subcode, msg
        )
    }
}

fn decode_primary_rgb(file_path: &str) -> Result<(Vec<u8>, u32, u32, u32), String> {
    let c_path = CString::new(file_path).map_err(|_| "Invalid file path".to_string())?;
    unsafe {
        let ctx = heif_context_alloc();
        if ctx.is_null() {
            return Err("Failed to allocate heif context".to_string());
        }
        struct CtxGuard(*mut HeifContext);
        impl Drop for CtxGuard {
            fn drop(&mut self) {
                unsafe { heif_context_free(self.0) }
            }
        }
        let _ctx_guard = CtxGuard(ctx);

        let err = heif_context_read_from_file(ctx, c_path.as_ptr(), ptr::null());
        if err.code != 0 {
            return Err(fmt_heif_error(err));
        }

        let mut handle: *mut HeifImageHandle = ptr::null_mut();
        let err = heif_context_get_primary_image_handle(ctx, &mut handle);
        if err.code != 0 || handle.is_null() {
            return Err(fmt_heif_error(err));
        }
        struct HandleGuard(*mut HeifImageHandle);
        impl Drop for HandleGuard {
            fn drop(&mut self) {
                unsafe { heif_image_handle_release(self.0) }
            }
        }
        let _handle_guard = HandleGuard(handle);

        let _handle_w = heif_image_handle_get_width(handle);
        let _handle_h = heif_image_handle_get_height(handle);
        let has_alpha = heif_image_handle_has_alpha_channel(handle) != 0;

        let mut img: *mut HeifImage = ptr::null_mut();
        let chroma = if has_alpha {
            HEIF_CHROMA_INTERLEAVED_RGBA
        } else {
            HEIF_CHROMA_INTERLEAVED_RGB
        };
        let err = heif_decode_image(handle, &mut img, HEIF_COLORSPACE_RGB, chroma, ptr::null());
        if err.code != 0 || img.is_null() {
            return Err(fmt_heif_error(err));
        }
        struct ImgGuard(*mut HeifImage);
        impl Drop for ImgGuard {
            fn drop(&mut self) {
                unsafe { heif_image_release(self.0) }
            }
        }
        let _img_guard = ImgGuard(img);

        let width = heif_image_get_width(img, HEIF_CHANNEL_INTERLEAVED).max(0) as u32;
        let height = heif_image_get_height(img, HEIF_CHANNEL_INTERLEAVED).max(0) as u32;
        if width == 0 || height == 0 {
            return Err("libheif returned empty dimensions".to_string());
        }

        let mut stride: c_int = 0;
        let ptr_plane = heif_image_get_plane_readonly(img, HEIF_CHANNEL_INTERLEAVED, &mut stride);
        if ptr_plane.is_null() || stride <= 0 {
            return Err("libheif returned empty plane".to_string());
        }

        let stride_u = stride as u32;
        let decoded_row_bytes = width.saturating_mul(if has_alpha { 4 } else { 3 });
        if stride_u < decoded_row_bytes {
            return Err("libheif returned invalid stride".to_string());
        }

        let src = slice::from_raw_parts(ptr_plane, (stride_u * height) as usize);
        let mut out = vec![0u8; (width * height * 3) as usize];
        for y in 0..height {
            let src_off = (y * stride_u) as usize;
            let dst_off = (y * width * 3) as usize;
            if has_alpha {
                let src_row = &src[src_off..src_off + decoded_row_bytes as usize];
                let dst_row = &mut out[dst_off..dst_off + (width * 3) as usize];
                for (i, pixel) in src_row.chunks_exact(4).enumerate() {
                    dst_row[i * 3..i * 3 + 3].copy_from_slice(&pixel[0..3]);
                }
            } else {
                out[dst_off..dst_off + (width * 3) as usize]
                    .copy_from_slice(&src[src_off..src_off + (width * 3) as usize]);
            }
        }

        Ok((out, width, height, width * 3))
    }
}

pub fn get_heif_thumbnail(
    file_path: &str,
    _orientation: i32,
    thumbnail_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    let (rgb, width, height, _row_bytes) = decode_primary_rgb(file_path)?;
    // Build a DynamicImage to reuse existing orientation + alpha handling logic.
    // libheif decode gives us RGB, no alpha here.
    let img = image::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| "Failed to build RGB image from libheif buffer".to_string())?;
    let dyn_img = DynamicImage::ImageRgb8(img);
    // libheif already applies HEIF geometric transformations (rotation/mirroring/crop).
    resize_dynamic_image_to_jpeg(dyn_img, 1, thumbnail_size).map(Some)
}

pub fn get_heif_preview(
    file_path: &str,
    _orientation: i32,
    max_size: u32,
) -> Result<Option<Vec<u8>>, String> {
    let (rgb, width, height, _row_bytes) = decode_primary_rgb(file_path)?;
    let img = image::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| "Failed to build RGB image from libheif buffer".to_string())?;
    // Preview path: keep it JPEG encoded at up to max_size (same as thumbnail sizing semantics).
    // libheif already applies HEIF geometric transformations (rotation/mirroring/crop).
    resize_dynamic_image_to_jpeg(DynamicImage::ImageRgb8(img), 1, max_size).map(Some)
}

/// Detect whether a HEIC/HEIF file contains an embedded video payload.
///
/// Priority:
/// 1. mime items with video/* content type (raw MP4/MOV blob)
/// 2. sequence tracks with handler type `vide` or `pict`
pub fn detect_heic_embedded_video(file_path: &str) -> Option<HeicEmbeddedVideoInfo> {
    let c_path = CString::new(file_path).ok()?;
    unsafe {
        let ctx = heif_context_alloc();
        if ctx.is_null() {
            return None;
        }
        struct CtxGuard(*mut HeifContext);
        impl Drop for CtxGuard {
            fn drop(&mut self) {
                unsafe { heif_context_free(self.0) }
            }
        }
        let _ctx_guard = CtxGuard(ctx);

        let err = heif_context_read_from_file(ctx, c_path.as_ptr(), ptr::null());
        if err.code != 0 {
            return None;
        }

        if let Some(info) = find_video_mime_item(ctx) {
            return Some(info);
        }
        find_visual_sequence_track(ctx)
    }
}

/// Extract embedded HEIC video into `cache_dir`.
///
/// - mime item: write raw item bytes as `.mp4`
/// - sequence track: try ffmpeg stream-copy demux of the HEIC container
pub fn extract_heic_embedded_video_to_cache(
    file_path: &str,
    cache_dir: &Path,
) -> Result<String, String> {
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)
            .map_err(|e| format!("Failed to create motion cache dir: {}", e))?;
    }

    let info = detect_heic_embedded_video(file_path)
        .ok_or_else(|| "HEIC has no embedded video item or sequence track".to_string())?;

    let cache_name = heic_video_cache_filename(file_path, &info)?;
    let cache_path = cache_dir.join(&cache_name);
    if let Ok(meta) = fs::metadata(&cache_path) {
        if meta.is_file() && meta.len() >= HEIC_VIDEO_CACHE_MIN_BYTES {
            return Ok(cache_path.to_string_lossy().to_string());
        }
    }

    match info.kind {
        0 => {
            let item_id = info
                .item_id
                .ok_or_else(|| "Missing HEIC video item id".to_string())?;
            let bytes = read_item_bytes(file_path, item_id)?;
            if (bytes.len() as u64) < HEIC_VIDEO_CACHE_MIN_BYTES {
                return Err(format!("HEIC video item too small ({} bytes)", bytes.len()));
            }
            write_cache_bytes(&cache_path, &cache_name, cache_dir, &bytes)?;
            Ok(cache_path.to_string_lossy().to_string())
        }
        1 => {
            // Sequence track: demux via ffmpeg (stream copy preferred).
            crate::t_video::remux_or_transcode_to_mp4(
                file_path,
                cache_path.to_string_lossy().as_ref(),
            )?;
            if let Ok(meta) = fs::metadata(&cache_path) {
                if meta.len() < HEIC_VIDEO_CACHE_MIN_BYTES {
                    let _ = fs::remove_file(&cache_path);
                    return Err("HEIC sequence track demux produced empty/invalid MP4".to_string());
                }
            }
            Ok(cache_path.to_string_lossy().to_string())
        }
        other => Err(format!("Unknown HEIC embedded video kind: {}", other)),
    }
}

fn heic_video_cache_filename(
    file_path: &str,
    info: &HeicEmbeddedVideoInfo,
) -> Result<String, String> {
    let meta = fs::metadata(file_path).map_err(|e| format!("Failed to stat HEIC: {}", e))?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(format!(
        "picaipic_heicvid_{:x}_{}_{}_{}_{}_{}.mp4",
        fnv1a64(file_path),
        size,
        mtime,
        info.kind,
        info.item_id.unwrap_or(0),
        info.track_id.unwrap_or(0)
    ))
}

fn fnv1a64(value: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in value.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn write_cache_bytes(
    cache_path: &Path,
    cache_name: &str,
    cache_dir: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    let tmp_path = cache_dir.join(format!("{}.tmp", cache_name));
    fs::write(&tmp_path, bytes)
        .map_err(|e| format!("Failed to write HEIC video cache temp: {}", e))?;
    if cache_path.exists() {
        let _ = fs::remove_file(cache_path);
    }
    fs::rename(&tmp_path, cache_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("Failed to finalize HEIC video cache: {}", e)
    })?;
    Ok(())
}

fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()) }
}

fn is_video_mime(mime: &str) -> bool {
    let m = mime.to_ascii_lowercase();
    m.starts_with("video/") || m.contains("mp4") || m.contains("quicktime") || m.contains("mpeg")
}

fn find_video_mime_item(ctx: *mut HeifContext) -> Option<HeicEmbeddedVideoInfo> {
    unsafe {
        let n = heif_context_get_number_of_items(ctx);
        if n <= 0 {
            return None;
        }
        let mut ids = vec![0u32; n as usize];
        let filled = heif_context_get_list_of_item_IDs(ctx, ids.as_mut_ptr(), n);
        if filled <= 0 {
            return None;
        }
        ids.truncate(filled as usize);

        for id in ids {
            let item_type = heif_item_get_item_type(ctx, id);
            if item_type != HEIF_ITEM_TYPE_MIME {
                continue;
            }
            let mime =
                cstr_to_string(heif_item_get_mime_item_content_type(ctx, id)).unwrap_or_default();
            if !is_video_mime(&mime) {
                continue;
            }
            // Cheap size check: skip empty items.
            let mut data_ptr: *mut u8 = ptr::null_mut();
            let mut data_size: usize = 0;
            let err =
                heif_item_get_item_data(ctx, id, ptr::null_mut(), &mut data_ptr, &mut data_size);
            if !data_ptr.is_null() {
                heif_release_item_data(ctx, &mut data_ptr);
            }
            if err.code != 0 || data_size < HEIC_VIDEO_CACHE_MIN_BYTES as usize {
                continue;
            }
            return Some(HeicEmbeddedVideoInfo {
                kind: 0,
                item_id: Some(id),
                track_id: None,
            });
        }
        None
    }
}

fn find_visual_sequence_track(ctx: *mut HeifContext) -> Option<HeicEmbeddedVideoInfo> {
    unsafe {
        if heif_context_has_sequence(ctx) == 0 {
            return None;
        }
        let n = heif_context_number_of_sequence_tracks(ctx);
        if n <= 0 {
            return None;
        }
        let mut ids = vec![0u32; n as usize];
        heif_context_get_track_ids(ctx, ids.as_mut_ptr());

        for track_id in ids {
            if track_id == 0 {
                continue;
            }
            let track = heif_context_get_track(ctx, track_id);
            if track.is_null() {
                continue;
            }
            struct TrackGuard(*mut HeifTrack);
            impl Drop for TrackGuard {
                fn drop(&mut self) {
                    unsafe { heif_track_release(self.0) }
                }
            }
            let _guard = TrackGuard(track);
            let handler = heif_track_get_track_handler_type(track);
            if handler == HEIF_TRACK_TYPE_VIDEO || handler == HEIF_TRACK_TYPE_PICT {
                return Some(HeicEmbeddedVideoInfo {
                    kind: 1,
                    item_id: None,
                    track_id: Some(track_id),
                });
            }
        }
        None
    }
}

fn read_item_bytes(file_path: &str, item_id: u32) -> Result<Vec<u8>, String> {
    let c_path = CString::new(file_path).map_err(|_| "Invalid file path".to_string())?;
    unsafe {
        let ctx = heif_context_alloc();
        if ctx.is_null() {
            return Err("Failed to allocate heif context".to_string());
        }
        struct CtxGuard(*mut HeifContext);
        impl Drop for CtxGuard {
            fn drop(&mut self) {
                unsafe { heif_context_free(self.0) }
            }
        }
        let _ctx_guard = CtxGuard(ctx);

        let err = heif_context_read_from_file(ctx, c_path.as_ptr(), ptr::null());
        if err.code != 0 {
            return Err(fmt_heif_error(err));
        }

        let mut data_ptr: *mut u8 = ptr::null_mut();
        let mut data_size: usize = 0;
        let err =
            heif_item_get_item_data(ctx, item_id, ptr::null_mut(), &mut data_ptr, &mut data_size);
        if err.code != 0 || data_ptr.is_null() || data_size == 0 {
            if !data_ptr.is_null() {
                heif_release_item_data(ctx, &mut data_ptr);
            }
            return Err(fmt_heif_error(err));
        }
        let bytes = slice::from_raw_parts(data_ptr, data_size).to_vec();
        heif_release_item_data(ctx, &mut data_ptr);
        Ok(bytes)
    }
}

/// Parse `heifitem:N` / `heifseq:N` content_id markers.
#[allow(dead_code)]
pub fn parse_heic_content_id(content_id: &str) -> Option<HeicEmbeddedVideoInfo> {
    let parts: Vec<&str> = content_id.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    match parts[0] {
        "heifitem" => {
            let id = parts[1].parse::<u32>().ok()?;
            Some(HeicEmbeddedVideoInfo {
                kind: 0,
                item_id: Some(id),
                track_id: None,
            })
        }
        "heifseq" => {
            let id = parts[1].parse::<u32>().ok()?;
            Some(HeicEmbeddedVideoInfo {
                kind: 1,
                item_id: None,
                track_id: Some(id),
            })
        }
        _ => None,
    }
}
