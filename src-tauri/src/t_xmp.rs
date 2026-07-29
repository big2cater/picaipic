/**
 * XMP metadata parsing for Live Photo / Motion Photo support.
 *
 * Handles Google/Android Motion Photo detection by parsing XMP metadata
 * embedded in JPEG files. Motion Photos store a video segment appended
 * after the JPEG image data, with the video offset recorded in XMP.
 *
 * Extracted Motion video segments are cached under the app cache dir
 * (`motion_cache/`) with source-keyed filenames so long-press preview
 * can reuse bytes without re-slicing the JPEG each time.
 */
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use tauri::{AppHandle, Manager};

/// Subdirectory under `app_cache_dir` for Motion Photo extracted videos.
pub const MOTION_CACHE_DIR_NAME: &str = "motion_cache";
/// Minimum valid extracted MP4 size (bytes). Smaller files are treated as miss.
const MOTION_CACHE_MIN_BYTES: u64 = 1024;
/// Soft cap before pruning oldest entries.
const MOTION_CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Target size after pruning (~70% of max).
const MOTION_CACHE_TARGET_BYTES: u64 = (MOTION_CACHE_MAX_BYTES * 7) / 10;

/// Parsed Motion Photo metadata from XMP.
#[derive(Debug, Clone)]
pub struct MotionPhotoInfo {
    /// Byte offset of the embedded video within the JPEG file.
    pub video_offset: u64,
    /// Length of the embedded video segment (if known).
    pub video_length: Option<u64>,
}

/// Optional internal timing for Motion Photo detection during scan profiling.
#[derive(Default)]
pub struct MotionPhotoReadProfile {
    pub header_xmp_attempts: u64,
    pub header_xmp_elapsed: Duration,
    pub header_complete_check_attempts: u64,
    pub header_complete_check_elapsed: Duration,
    pub file_fallback_attempts: u64,
    pub file_fallback_elapsed: Duration,
    pub parse_attempts: u64,
    pub parse_elapsed: Duration,
}

/// Parse a Motion Photo `content_id` stored as `motion:<offset>:<length>`.
///
/// Empty / missing length (`motion:<offset>:` or `motion:<offset>`) yields
/// `video_length = None`. Single source of truth for `t_cmds` and
/// `t_live_photo` so encode/decode stay aligned.
pub fn parse_motion_content_id(content_id: &str) -> Option<MotionPhotoInfo> {
    let parts: Vec<&str> = content_id.split(':').collect();
    if parts.len() < 2 || parts[0] != "motion" {
        return None;
    }
    let video_offset = parts[1].parse::<u64>().ok()?;
    let video_length = if parts.len() >= 3 && !parts[2].is_empty() {
        parts[2].parse::<u64>().ok()
    } else {
        None
    };
    Some(MotionPhotoInfo {
        video_offset,
        video_length,
    })
}

/// Extract the raw XMP packet from a JPEG/HEIF file.
///
/// In JPEG, XMP is stored in an APP1 segment with namespace
/// `http://ns.adobe.com/xap/1.0/\0`. In HEIF, it may be in a
/// `xmp` item. We scan the file for `<x:xmpmeta` to `</x:xmpmeta>`.
pub fn extract_xmp_packet(file_path: &str) -> Option<String> {
    extract_xmp_packet_with_header(file_path, None)
}

/// Extract XMP while reusing an already-read image header when it is complete
/// through JPEG SOS/EOI. Incomplete headers retain the file-read fallback.
pub fn extract_xmp_packet_with_header(
    file_path: &str,
    file_header: Option<&[u8]>,
) -> Option<String> {
    extract_xmp_packet_with_header_profiled(file_path, file_header, None)
}

fn extract_xmp_packet_with_header_profiled(
    file_path: &str,
    file_header: Option<&[u8]>,
    mut profile: Option<&mut MotionPhotoReadProfile>,
) -> Option<String> {
    let path = Path::new(file_path);
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => {
            if let Some(header) = file_header {
                let started = profile.as_ref().map(|_| Instant::now());
                let header_complete_without_xmp = jpeg_header_complete_without_xmp(header);
                if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
                    profile.header_complete_check_attempts += 1;
                    profile.header_complete_check_elapsed += started.elapsed();
                }
                if header_complete_without_xmp {
                    return None;
                }
                let started = profile.as_ref().map(|_| Instant::now());
                let xmp = extract_xmp_from_jpeg_bytes(header);
                if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
                    profile.header_xmp_attempts += 1;
                    profile.header_xmp_elapsed += started.elapsed();
                }
                if let Some(xmp) = xmp {
                    return Some(xmp);
                }
                if jpeg_header_reaches_scan(header) {
                    return None;
                }
            }
            let started = profile.as_ref().map(|_| Instant::now());
            let xmp = extract_xmp_from_jpeg(file_path);
            if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
                profile.file_fallback_attempts += 1;
                profile.file_fallback_elapsed += started.elapsed();
            }
            xmp
        }
        "heic" | "heif" | "hif" => {
            let started = profile.as_ref().map(|_| Instant::now());
            let xmp = extract_xmp_from_heic(file_path);
            if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
                profile.file_fallback_attempts += 1;
                profile.file_fallback_elapsed += started.elapsed();
            }
            xmp
        }
        _ => None,
    }
}

/// Extract XMP from a JPEG file by scanning APP1 segments.
fn extract_xmp_from_jpeg(file_path: &str) -> Option<String> {
    let mut file = fs::File::open(file_path).ok()?;
    let mut buf = vec![0u8; 512 * 1024]; // Read up to 512KB (XMP is near the start)
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);
    drop(file);

    extract_xmp_from_jpeg_bytes(&buf)
}

fn extract_xmp_from_jpeg_bytes(buf: &[u8]) -> Option<String> {
    // Search for the XMP packet start marker
    let start_marker = b"<x:xmpmeta";
    let end_marker = b"</x:xmpmeta>";

    // First try the standard XMP APP1 namespace marker
    let xmp_ns = b"http://ns.adobe.com/xap/1.0/\0";

    // Find XMP by scanning for the namespace or the packet start
    let xmp_start = find_subslice(&buf, start_marker).or_else(|| {
        // If no <x:xmpmeta>, try the namespace marker
        find_subslice(&buf, xmp_ns).and_then(|pos| {
            // Skip past the namespace + length bytes to find the XML
            let search_start = pos + xmp_ns.len();
            if search_start >= buf.len() {
                return None;
            }
            // Look for <x:xmpmeta after the namespace
            find_subslice(&buf[search_start..], start_marker).map(|p| search_start + p)
        })
    })?;

    let xmp_end =
        find_subslice(&buf[xmp_start..], end_marker).map(|p| xmp_start + p + end_marker.len())?;

    let xmp_bytes = &buf[xmp_start..xmp_end];
    String::from_utf8(xmp_bytes.to_vec()).ok()
}

/// True if `data` reaches JPEG scan data or EOI after a complete marker walk.
/// XMP lives in APP metadata before that boundary, so a complete header with no
/// XMP cannot gain one from reopening the source file.
fn jpeg_header_reaches_scan(data: &[u8]) -> bool {
    if data.len() < 2 || data[0] != 0xff || data[1] != 0xd8 {
        return false;
    }

    let mut offset = 2usize;
    while offset + 2 <= data.len() {
        if data[offset] != 0xff {
            return false;
        }
        while offset < data.len() && data[offset] == 0xff {
            offset += 1;
        }
        if offset >= data.len() {
            return false;
        }
        let marker = data[offset];
        offset += 1;
        if marker == 0xd9 || marker == 0xda {
            return true;
        }
        if marker == 0x00 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if offset + 2 > data.len() {
            return false;
        }
        let length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        if length < 2 || offset + length > data.len() {
            return false;
        }
        offset += length;
    }
    false
}

/// True when a complete JPEG marker walk reaches SOS/EOI without an APP1 XMP
/// packet. Motion Photo XMP belongs in APP1, so the generic byte scanner and
/// file fallback cannot find it after this proof.
fn jpeg_header_complete_without_xmp(data: &[u8]) -> bool {
    if data.len() < 2 || data[0] != 0xff || data[1] != 0xd8 {
        return false;
    }

    let mut offset = 2usize;
    while offset + 2 <= data.len() {
        if data[offset] != 0xff {
            return false;
        }
        while offset < data.len() && data[offset] == 0xff {
            offset += 1;
        }
        if offset >= data.len() {
            return false;
        }
        let marker = data[offset];
        offset += 1;
        if marker == 0xd9 || marker == 0xda {
            return true;
        }
        if marker == 0x00 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if offset + 2 > data.len() {
            return false;
        }
        let length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        if length < 2 || offset + length > data.len() {
            return false;
        }
        let segment_end = offset + length;
        if marker == 0xe1 {
            let payload = &data[offset + 2..segment_end];
            if find_subslice(payload, b"http://ns.adobe.com/xap/1.0/\0").is_some()
                || find_subslice(payload, b"<x:xmpmeta").is_some()
            {
                return false;
            }
        }
        offset = segment_end;
    }
    false
}

/// Extract XMP from a HEIC file. HEIF stores XMP as a meta item.
/// For now we do a raw binary scan for the XMP packet markers.
fn extract_xmp_from_heic(file_path: &str) -> Option<String> {
    let mut file = fs::File::open(file_path).ok()?;
    let mut buf = vec![0u8; 512 * 1024]; // Read up to 512KB
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);
    drop(file);

    let start_marker = b"<x:xmpmeta";
    let end_marker = b"</x:xmpmeta>";

    let xmp_start = find_subslice(&buf, start_marker)?;
    let xmp_end =
        find_subslice(&buf[xmp_start..], end_marker).map(|p| xmp_start + p + end_marker.len())?;

    let xmp_bytes = &buf[xmp_start..xmp_end];
    String::from_utf8(xmp_bytes.to_vec()).ok()
}

/// Detect if a file is a Google Motion Photo and extract video offset info.
///
/// Google Motion Photos store XMP metadata with the `GCamera` namespace
/// containing `MotionPhoto=1` and a `Container:Directory` that lists
/// the image and video segments with their byte offsets.
pub fn detect_motion_photo(file_path: &str) -> Option<MotionPhotoInfo> {
    detect_motion_photo_with_header(file_path, None)
}

/// Detect Motion Photo metadata while reusing an already-read JPEG header when
/// possible. Callers with no header retain the original file-read behavior.
pub fn detect_motion_photo_with_header(
    file_path: &str,
    file_header: Option<&[u8]>,
) -> Option<MotionPhotoInfo> {
    detect_motion_photo_with_header_profiled(file_path, file_header, None)
}

/// Profiled variant used only by opt-in scan timing.
pub fn detect_motion_photo_with_header_profiled(
    file_path: &str,
    file_header: Option<&[u8]>,
    mut profile: Option<&mut MotionPhotoReadProfile>,
) -> Option<MotionPhotoInfo> {
    let xmp =
        extract_xmp_packet_with_header_profiled(file_path, file_header, profile.as_deref_mut())?;

    let started = profile.as_ref().map(|_| Instant::now());

    // Check for MotionPhoto flag
    // The XMP uses rdf:Description with GCamera namespace attributes
    let result = if !xmp.contains("MotionPhoto") {
        None
    } else if let Some(offset) = extract_xmp_value(&xmp, "MotionPhotoOffset") {
        offset
            .parse::<u64>()
            .ok()
            .map(|offset_val| MotionPhotoInfo {
                video_offset: offset_val,
                video_length: None,
            })
    } else {
        parse_container_directory(&xmp)
    };

    if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
        profile.parse_attempts += 1;
        profile.parse_elapsed += started.elapsed();
    }
    result
}

/// Parse the Container:Directory XMP structure to find the video segment.
fn parse_container_directory(xmp: &str) -> Option<MotionPhotoInfo> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xmp);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut in_directory = false;
    let mut image_length: Option<u64> = None;
    let mut video_item: Option<(Option<u64>, Option<u64>)> = None; // (length, padding)

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let name = String::from_utf8_lossy(&name_bytes).to_string();

                if name.contains("Directory") {
                    in_directory = true;
                }

                if in_directory && name.contains("Item") {
                    let mut semantic = String::new();
                    let mut length: Option<u64> = None;
                    let mut padding: Option<u64> = None;

                    for attr in e.attributes().with_checks(true).flatten() {
                        let key_str = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = attr
                            .unescape_value()
                            .ok()
                            .map(|s| s.to_string())
                            .unwrap_or_default();

                        if key_str.contains("Semantic") {
                            semantic = val;
                        } else if key_str.contains("Length") {
                            length = val.parse::<u64>().ok();
                        } else if key_str.contains("Padding") {
                            padding = val.parse::<u64>().ok();
                        }
                    }

                    if semantic.contains("Image") || semantic.contains("Primary") {
                        image_length = length;
                    } else if semantic.contains("Motion") || semantic.contains("Video") {
                        video_item = Some((length, padding));
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name_bytes = e.name().as_ref().to_vec();
                let name = String::from_utf8_lossy(&name_bytes).to_string();
                if name.contains("Directory") {
                    in_directory = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // Calculate video offset from image length + padding
    if let Some((video_length, _video_padding)) = video_item {
        let offset = image_length.unwrap_or(0);
        return Some(MotionPhotoInfo {
            video_offset: offset,
            video_length: video_length,
        });
    }

    None
}

/// Extract a text value from XMP by tag name (simple regex-free approach).
fn extract_xmp_value(xmp: &str, tag: &str) -> Option<String> {
    // Try <Namespace:Tag>value</Namespace:Tag>
    let patterns = [format!(">{}", tag), format!(":{}>", tag)];

    // Find the tag and extract its content
    for line in xmp.lines() {
        for pattern in &patterns {
            if line.contains(pattern) || line.contains(tag) {
                // Look for >value</ ...>
                if let Some(start) = line.find('>') {
                    if let Some(end_rel) = line[start + 1..].find('<') {
                        let value = &line[start + 1..start + 1 + end_rel];
                        let trimmed = value.trim();
                        if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Resolve `app_cache_dir()/motion_cache`, creating it if needed.
pub fn motion_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve app cache dir: {}", e))?
        .join(MOTION_CACHE_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create motion cache dir: {}", e))?;
    }
    Ok(dir)
}

/// Create the motion cache directory on startup and purge legacy OS-temp extracts.
pub fn init_motion_cache(app: &AppHandle) {
    if let Err(e) = motion_cache_dir(app) {
        eprintln!("Failed to init motion cache: {}", e);
    }
    purge_legacy_motion_temp_files();
}

/// Remove historical `picaipic_motion_*.mp4` files left in the system temp dir.
pub fn purge_legacy_motion_temp_files() {
    let temp_dir = std::env::temp_dir();
    let Ok(entries) = fs::read_dir(&temp_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("picaipic_motion_") && name.ends_with(".mp4") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Clear the motion cache directory (recreate empty).
pub fn clear_motion_cache_dir(app: &AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve app cache dir: {}", e))?
        .join(MOTION_CACHE_DIR_NAME);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Failed to remove motion cache: {}", e))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to recreate motion cache: {}", e))?;
    Ok(())
}

/// Extract the embedded video segment from a Motion Photo into `cache_dir`.
///
/// Cache key is derived from source path + size + mtime + offset + length so a
/// second long-press does not re-read the JPEG. Writes go through a `.tmp`
/// file then rename. After a successful write, prunes the cache if over budget.
pub fn extract_motion_video_to_cache(
    file_path: &str,
    info: &MotionPhotoInfo,
    cache_dir: &Path,
) -> Result<String, String> {
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)
            .map_err(|e| format!("Failed to create motion cache dir: {}", e))?;
    }

    let cache_name = motion_cache_filename(file_path, info)?;
    let cache_path = cache_dir.join(&cache_name);
    if let Ok(meta) = fs::metadata(&cache_path) {
        if meta.is_file() && meta.len() >= MOTION_CACHE_MIN_BYTES {
            return Ok(cache_path.to_string_lossy().to_string());
        }
    }

    let mut file = fs::File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    file.seek(SeekFrom::Start(info.video_offset))
        .map_err(|e| format!("Failed to seek to video offset: {}", e))?;

    let video_data = if let Some(length) = info.video_length {
        let mut buf = vec![0u8; length as usize];
        file.read_exact(&mut buf)
            .map_err(|e| format!("Failed to read video segment: {}", e))?;
        buf
    } else {
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read to end: {}", e))?;
        buf
    };

    if (video_data.len() as u64) < MOTION_CACHE_MIN_BYTES {
        return Err(format!(
            "Motion Photo video segment too small ({} bytes)",
            video_data.len()
        ));
    }

    let tmp_path = cache_dir.join(format!("{}.tmp", cache_name));
    fs::write(&tmp_path, &video_data)
        .map_err(|e| format!("Failed to write motion cache temp: {}", e))?;

    // Windows: destination must not exist for rename to succeed.
    if cache_path.exists() {
        let _ = fs::remove_file(&cache_path);
    }
    fs::rename(&tmp_path, &cache_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("Failed to finalize motion cache file: {}", e)
    })?;

    auto_cleanup_motion_cache(cache_dir);

    Ok(cache_path.to_string_lossy().to_string())
}

/// Package a still JPEG and an MP4 byte buffer into a Google Motion Photo JPEG.
///
/// Inserts a GCamera/Container XMP APP1 segment and appends the video payload.
/// The Primary item Length equals the final still portion size (video starts at that offset).
pub fn package_motion_photo_jpeg(still_jpeg: &[u8], video_mp4: &[u8]) -> Result<Vec<u8>, String> {
    if still_jpeg.len() < 4 || still_jpeg[0] != 0xFF || still_jpeg[1] != 0xD8 {
        return Err("Still image is not a valid JPEG (missing SOI)".to_string());
    }
    if video_mp4.len() < 32 {
        return Err("Video payload too small to package as Motion Photo".to_string());
    }

    let core = strip_jpeg_xmp_app1(still_jpeg);
    if core.len() < 4 || !core.ends_with(&[0xFF, 0xD9]) {
        // Some writers omit EOI when trailer follows; ensure EOI for a standalone still core.
        let mut with_eoi = core;
        if with_eoi.len() < 2 || with_eoi[with_eoi.len() - 2..] != [0xFF, 0xD9] {
            with_eoi.extend_from_slice(&[0xFF, 0xD9]);
        }
        return package_motion_photo_jpeg_core(&with_eoi, video_mp4);
    }
    package_motion_photo_jpeg_core(&core, video_mp4)
}

fn package_motion_photo_jpeg_core(core_jpeg: &[u8], video_mp4: &[u8]) -> Result<Vec<u8>, String> {
    const XMP_NS: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    let video_len = video_mp4.len() as u64;

    // Resolve image length / XMP size iteratively (XMP contains the length).
    let mut image_len = core_jpeg.len() as u64 + 1000;
    let mut xmp = String::new();
    for _ in 0..6 {
        xmp = build_motion_photo_xmp(image_len, video_len);
        let app1_payload_len = XMP_NS.len() + xmp.len(); // without marker/length
        let app1_total = 2 + 2 + app1_payload_len; // FFE1 + LL + payload
        let next = core_jpeg.len() as u64 + app1_total as u64;
        if next == image_len {
            break;
        }
        image_len = next;
    }

    if image_len > u32::MAX as u64 {
        return Err("Motion Photo still portion is too large".to_string());
    }

    let app1_payload_len = XMP_NS.len() + xmp.len();
    if app1_payload_len + 2 > 0xFFFF {
        return Err("Motion Photo XMP segment exceeds APP1 size limit".to_string());
    }
    let app1_len_field = (app1_payload_len + 2) as u16; // includes the 2 length bytes

    // Insert APP1 right after SOI (FFD8).
    let mut out = Vec::with_capacity(image_len as usize + video_mp4.len());
    out.extend_from_slice(&core_jpeg[..2]); // SOI
    out.push(0xFF);
    out.push(0xE1);
    out.push((app1_len_field >> 8) as u8);
    out.push((app1_len_field & 0xFF) as u8);
    out.extend_from_slice(XMP_NS);
    out.extend_from_slice(xmp.as_bytes());
    out.extend_from_slice(&core_jpeg[2..]);
    if out.len() as u64 != image_len {
        // Keep going with actual size; detector uses Container Image Length if present.
        // Adjust is best-effort; mismatch can happen if core already had APP markers.
    }
    out.extend_from_slice(video_mp4);

    // Sanity: our detector should accept the product.
    // Write is validated by the caller via detect_motion_photo if needed.
    Ok(out)
}

fn build_motion_photo_xmp(image_len: u64, video_len: u64) -> String {
    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="PicAiPic">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:GCamera="http://ns.google.com/photos/1.0/camera/"
    xmlns:Container="http://ns.google.com/photos/1.0/container/"
    xmlns:Item="http://ns.google.com/photos/1.0/container/item/"
   GCamera:MotionPhoto="1"
   GCamera:MotionPhotoVersion="1"
   GCamera:MotionPhotoPresentationTimestampUs="0"
   GCamera:MicroVideo="1"
   GCamera:MicroVideoVersion="1"
   GCamera:MicroVideoOffset="{video_len}"
   GCamera:MicroVideoPresentationTimestampUs="0">
   <Container:Directory>
    <rdf:Seq>
     <rdf:li rdf:parseType="Resource">
      <Container:Item
       Item:Mime="image/jpeg"
       Item:Semantic="Primary"
       Item:Length="{image_len}"
       Item:Padding="0"/>
     </rdf:li>
     <rdf:li rdf:parseType="Resource">
      <Container:Item
       Item:Mime="video/mp4"
       Item:Semantic="MotionPhoto"
       Item:Length="{video_len}"
       Item:Padding="0"/>
     </rdf:li>
    </rdf:Seq>
   </Container:Directory>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#
    )
}

/// Remove existing XMP APP1 segments from a JPEG buffer (best-effort).
fn strip_jpeg_xmp_app1(jpeg: &[u8]) -> Vec<u8> {
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return jpeg.to_vec();
    }
    let mut out = Vec::with_capacity(jpeg.len());
    out.extend_from_slice(&jpeg[..2]);
    let mut i = 2usize;
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xFF {
            out.extend_from_slice(&jpeg[i..]);
            break;
        }
        // Skip fill bytes
        while i < jpeg.len() && jpeg[i] == 0xFF {
            if i + 1 < jpeg.len() && jpeg[i + 1] == 0xFF {
                i += 1;
                continue;
            }
            break;
        }
        if i + 1 >= jpeg.len() {
            break;
        }
        let marker = jpeg[i + 1];
        // Standalone markers without length
        if marker == 0xD9 {
            // EOI
            out.extend_from_slice(&jpeg[i..]);
            break;
        }
        if marker == 0xDA {
            // SOS — copy rest (entropy-coded data + EOI)
            out.extend_from_slice(&jpeg[i..]);
            break;
        }
        if marker == 0x00 || marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            out.push(0xFF);
            out.push(marker);
            i += 2;
            continue;
        }
        if i + 4 > jpeg.len() {
            out.extend_from_slice(&jpeg[i..]);
            break;
        }
        let seg_len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if seg_len < 2 || i + 2 + seg_len > jpeg.len() {
            out.extend_from_slice(&jpeg[i..]);
            break;
        }
        let seg_start = i;
        let seg_end = i + 2 + seg_len;
        let is_xmp = marker == 0xE1
            && seg_end.saturating_sub(seg_start + 4) >= 29
            && &jpeg[seg_start + 4..seg_start + 4 + 28] == b"http://ns.adobe.com/xap/1.0/";
        if !is_xmp {
            out.extend_from_slice(&jpeg[seg_start..seg_end]);
        }
        i = seg_end;
    }
    out
}

/// Read still JPEG bytes from a Motion Photo (everything before video_offset).
pub fn read_motion_still_bytes(file_path: &str, info: &MotionPhotoInfo) -> Result<Vec<u8>, String> {
    if info.video_offset == 0 {
        return Err("Motion Photo video offset is zero".to_string());
    }
    let mut file = fs::File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut still_buf = vec![0u8; info.video_offset as usize];
    file.read_exact(&mut still_buf)
        .map_err(|e| format!("Failed to read still segment: {}", e))?;
    if still_buf.len() < 2 || still_buf[0] != 0xFF || still_buf[1] != 0xD8 {
        return Err("Motion Photo still segment does not look like a JPEG".to_string());
    }
    Ok(still_buf)
}

fn motion_cache_filename(file_path: &str, info: &MotionPhotoInfo) -> Result<String, String> {
    let meta = fs::metadata(file_path).map_err(|e| format!("Failed to stat source: {}", e))?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let length = info.video_length.unwrap_or(0);
    Ok(format!(
        "picaipic_motion_{:x}_{}_{}_{}_{}.mp4",
        fnv1a64(file_path),
        size,
        mtime,
        info.video_offset,
        length
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

/// Drop oldest motion cache files until total size is at or below target.
fn auto_cleanup_motion_cache(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    let mut total: u64 = 0;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".tmp") {
            continue;
        }
        total = total.saturating_add(meta.len());
        files.push((
            entry.path(),
            meta.len(),
            meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        ));
    }
    if total <= MOTION_CACHE_MAX_BYTES {
        return;
    }
    files.sort_by_key(|f| f.2);
    for (path, size, _) in files {
        if total <= MOTION_CACHE_TARGET_BYTES {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

/// Find a subslice in a byte buffer.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_jpeg_header_reaches_metadata_boundary() {
        assert!(jpeg_header_reaches_scan(&[
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x02, // empty APP0
            0xff, 0xda, // SOS
        ]));
        assert!(!jpeg_header_reaches_scan(&[
            0xff, 0xd8, 0xff, 0xe1, 0x00, 0x10, // incomplete APP1
        ]));
    }

    #[test]
    fn complete_jpeg_without_xmp_is_proven_empty() {
        assert!(jpeg_header_complete_without_xmp(&[
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x02, // empty APP0
            0xff, 0xda, // SOS
        ]));

        let xmp = b"<x:xmpmeta></x:xmpmeta>";
        let app1_len = (xmp.len() + 2) as u16;
        let mut header = vec![0xff, 0xd8, 0xff, 0xe1];
        header.extend_from_slice(&app1_len.to_be_bytes());
        header.extend_from_slice(xmp);
        header.extend_from_slice(&[0xff, 0xda]);
        assert!(!jpeg_header_complete_without_xmp(&header));
    }

    #[test]
    fn motion_photo_detection_reuses_complete_jpeg_header() {
        let xmp = concat!(
            "<x:xmpmeta><rdf:RDF><rdf:Description ",
            "xmlns:GCamera=\"http://ns.google.com/photos/1.0/camera/\">",
            "<GCamera:MotionPhoto>1</GCamera:MotionPhoto>",
            "<GCamera:MotionPhotoOffset>42</GCamera:MotionPhotoOffset>",
            "</rdf:Description></rdf:RDF></x:xmpmeta>"
        );
        let app1_len = (xmp.len() + 2) as u16;
        let mut header = vec![0xff, 0xd8, 0xff, 0xe1];
        header.extend_from_slice(&app1_len.to_be_bytes());
        header.extend_from_slice(xmp.as_bytes());
        header.extend_from_slice(&[0xff, 0xda]);

        let motion = detect_motion_photo_with_header("missing.jpg", Some(&header));
        assert_eq!(motion.map(|info| info.video_offset), Some(42));
    }

    #[test]
    fn profiled_motion_detection_skips_file_fallback_for_complete_header() {
        let header = [
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x02, // empty APP0
            0xff, 0xda, // SOS
        ];
        let mut profile = MotionPhotoReadProfile::default();

        assert!(
            detect_motion_photo_with_header_profiled(
                "missing.jpg",
                Some(&header),
                Some(&mut profile),
            )
            .is_none()
        );
        assert_eq!(profile.header_xmp_attempts, 0);
        assert_eq!(profile.header_complete_check_attempts, 1);
        assert_eq!(profile.file_fallback_attempts, 0);
        assert_eq!(profile.parse_attempts, 0);
    }
}
