/**
 * Live Photo / Motion Photo export orchestration.
 *
 * Modes:
 * - still: export still image (Motion Photos strip the video trailer)
 * - video: export companion / embedded video
 * - pair: export both into a destination folder with the same stem
 * - to_motion: convert Apple Live pair → single Google Motion Photo JPEG
 * - to_pair: convert Motion Photo → still JPEG + video (same stem)
 * - set_keyframe: export a still JPEG from the motion video at keyframe_sec
 *   (optional overwrite_original replaces the library still with confirmation from UI)
 */
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::t_sqlite::AFile;
use crate::t_utils::{FileConflictPolicy, copy_file_with_policy};
use crate::t_video;
use crate::t_xmp::{self, MotionPhotoInfo};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportLivePhotoOptions {
    #[serde(default = "default_conflict")]
    pub conflict: String,
    /// Optional preferred video extension for Motion extracts ("mp4" default).
    pub video_format: Option<String>,
    /// Optional preferred still extension when rewriting ("keep" default).
    pub still_format: Option<String>,
    /// Seconds into the motion video for set_keyframe (default 0).
    pub keyframe_sec: Option<f64>,
    /// When true (default), stamp a shared ContentIdentifier on to_pair outputs.
    pub stamp_content_id: Option<bool>,
    /// When true with set_keyframe: replace the library still in-place (destructive).
    /// UI must confirm before setting this. Default false.
    pub overwrite_original: Option<bool>,
}

fn default_conflict() -> String {
    "keep_both".to_string()
}

impl Default for ExportLivePhotoOptions {
    fn default() -> Self {
        Self {
            conflict: default_conflict(),
            video_format: None,
            still_format: None,
            keyframe_sec: None,
            stamp_content_id: Some(true),
            overwrite_original: Some(false),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportLivePhotoResult {
    pub outputs: Vec<String>,
    pub content_id: Option<String>,
    /// True when set_keyframe replaced the library original still.
    pub overwrote_original: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RescanLivePhotoResult {
    pub updated: usize,
    pub paired: usize,
}

#[derive(Debug, Clone)]
struct LiveSources {
    live_photo_type: i64,
    content_id: Option<String>,
    still_path: Option<String>,
    video_path: Option<String>,
    motion: Option<MotionPhotoInfo>,
}

/// Export a Live Photo / Motion Photo according to `mode`.
pub fn export_live_photo(
    app: &AppHandle,
    file_id: i64,
    mode: &str,
    dest_path: Option<&str>,
    dest_dir: Option<&str>,
    options: ExportLivePhotoOptions,
) -> Result<ExportLivePhotoResult, String> {
    let sources = resolve_live_sources(app, file_id)?;
    let policy = FileConflictPolicy::from_str(&options.conflict);

    match mode {
        "still" => {
            let dest = dest_path
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "dest_path is required for still export".to_string())?;
            let out = export_still(&sources, dest, &options, policy)?;
            Ok(ExportLivePhotoResult {
                outputs: vec![out],
                content_id: sources.content_id,
                overwrote_original: false,
            })
        }
        "video" => {
            let dest = dest_path
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "dest_path is required for video export".to_string())?;
            let out = export_video(app, &sources, dest, policy)?;
            Ok(ExportLivePhotoResult {
                outputs: vec![out],
                content_id: sources.content_id,
                overwrote_original: false,
            })
        }
        "pair" => {
            let dir = dest_dir
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "dest_dir is required for pair export".to_string())?;
            let outs = export_pair(app, &sources, dir, &options, policy)?;
            Ok(ExportLivePhotoResult {
                outputs: outs,
                content_id: sources.content_id,
                overwrote_original: false,
            })
        }
        "to_motion" => {
            let dest = dest_path
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "dest_path is required for to_motion export".to_string())?;
            let out = export_to_motion(app, &sources, dest, policy)?;
            Ok(ExportLivePhotoResult {
                outputs: vec![out],
                content_id: sources.content_id,
                overwrote_original: false,
            })
        }
        "to_pair" => {
            let dir = dest_dir
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "dest_dir is required for to_pair export".to_string())?;
            let (outs, content_id) = export_to_pair(app, &sources, dir, &options, policy)?;
            Ok(ExportLivePhotoResult {
                outputs: outs,
                content_id,
                overwrote_original: false,
            })
        }
        "set_keyframe" => {
            let overwrite = options.overwrite_original.unwrap_or(false);
            if overwrite {
                let out = overwrite_still_with_keyframe(app, &sources, &options)?;
                Ok(ExportLivePhotoResult {
                    outputs: vec![out],
                    content_id: sources.content_id,
                    overwrote_original: true,
                })
            } else {
                let dest = dest_path
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| "dest_path is required for set_keyframe export".to_string())?;
                let out = export_set_keyframe(app, &sources, dest, &options, policy)?;
                Ok(ExportLivePhotoResult {
                    outputs: vec![out],
                    content_id: sources.content_id,
                    overwrote_original: false,
                })
            }
        }
        other => Err(format!("Unsupported export mode: {}", other)),
    }
}

/// Lightweight Live Photo metadata repair for an already-indexed album.
///
/// Re-detects Motion Photo / HEIC-internal video for candidates that are still
/// untyped (`live_photo_type=0`) or already HEIC-internal (`=4`), then re-runs pairing.
/// Does not re-read full EXIF for every file.
pub fn rescan_live_photo_metadata(album_id: i64) -> Result<RescanLivePhotoResult, String> {
    let conn = crate::t_sqlite::open_conn()?;
    crate::t_migration::ensure_live_photo_columns(&conn)?;

    let sql = "SELECT a.id, a.name, b.path, a.file_type, a.live_photo_type, a.content_id
               FROM afiles a
               JOIN afolders b ON a.folder_id = b.id
               WHERE b.album_id = ?1
                 AND a.file_type IN (1, 2, 3)
                 AND COALESCE(a.live_photo_type, 0) IN (0, 4)";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![album_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut updated = 0usize;
    for row in rows {
        let (id, name, folder_path, file_type, live_type, old_content_id) =
            row.map_err(|e| e.to_string())?;
        let file_path = crate::t_utils::get_file_path(&folder_path, &name);
        if !Path::new(&file_path).exists() {
            continue;
        }

        let mut new_type = live_type;
        let mut new_content_id = old_content_id.clone();

        if file_type == 1 || file_type == 3 {
            // Prefer not to clobber an existing Apple UUID content_id on type 0
            // unless we find a stronger HEIC/Motion marker.
            let is_heic = crate::t_image::is_heic_path(&file_path);
            let looks_jpeg = Path::new(&file_path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "jpe"))
                .unwrap_or(false);

            // HEIC-internal video
            #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
            if is_heic && (live_type == 0 || live_type == 4) {
                if let Some(info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crate::t_heif::detect_heic_embedded_video(&file_path)
                }))
                .ok()
                .flatten()
                {
                    new_type = 4;
                    new_content_id = Some(info.content_id_marker());
                } else if live_type == 4 {
                    // Lost detection: clear type 4 back to none so UI is honest.
                    new_type = 0;
                    new_content_id = None;
                }
            }

            // Motion Photo (JPEG primarily; also HEIC XMP scan)
            if new_type == 0 && (looks_jpeg || is_heic) {
                if let Some(motion) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    t_xmp::detect_motion_photo(&file_path)
                }))
                .ok()
                .flatten()
                {
                    let length_str = motion
                        .video_length
                        .map(|l| l.to_string())
                        .unwrap_or_default();
                    new_type = 3;
                    new_content_id = Some(format!("motion:{}:{}", motion.video_offset, length_str));
                }
            }

            // Apple ContentIdentifier on still (only when still untyped)
            if new_type == 0 {
                if let Some(cid) = read_image_content_identifier(&file_path) {
                    new_type = 1;
                    new_content_id = Some(cid);
                }
            }
        } else if file_type == 2 && live_type == 0 {
            // Apple Live Photo video content id via ffprobe
            if let Ok(meta) = t_video::get_video_metadata(&file_path) {
                if let Some(cid) = meta.content_id.filter(|s| !s.is_empty()) {
                    new_type = 2;
                    new_content_id = Some(cid);
                }
            }
        }

        if new_type != live_type || new_content_id != old_content_id {
            AFile::update_column(id, "live_photo_type", &new_type)
                .map_err(|e| format!("Failed updating live_photo_type for {}: {}", id, e))?;
            AFile::update_column(id, "content_id", &new_content_id)
                .map_err(|e| format!("Failed updating content_id for {}: {}", id, e))?;
            // Clear stale pairing when type/content changed; pair pass will relink.
            AFile::update_column(id, "paired_file_id", &None::<i64>).ok();
            updated += 1;
        }
    }

    let paired = AFile::pair_live_photos(album_id).unwrap_or(0);
    Ok(RescanLivePhotoResult { updated, paired })
}

fn read_image_content_identifier(file_path: &str) -> Option<String> {
    use exif::{In, Tag};
    let exif = crate::t_image::read_exif_permissive(file_path)?;
    exif.get_field(Tag(exif::Context::Tiff, 0x0011), In::PRIMARY)
        .and_then(|field| {
            field
                .value
                .display_as(field.tag)
                .to_string()
                .strip_suffix('\0')
                .map(|s| s.to_string())
                .or_else(|| {
                    let s = field.value.display_as(field.tag).to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
        })
        .filter(|s| !s.is_empty())
}

fn resolve_live_sources(app: &AppHandle, file_id: i64) -> Result<LiveSources, String> {
    let file = AFile::get_file_info(file_id)
        .map_err(|e| format!("Error while getting file info: {}", e))?
        .ok_or_else(|| format!("File not found: {}", file_id))?;

    let live_photo_type = file.live_photo_type.unwrap_or(0);
    if live_photo_type == 0 {
        return Err("File is not a Live Photo or Motion Photo".to_string());
    }

    let content_id = file.content_id.clone();
    let file_path = file
        .file_path
        .clone()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "File path not available".to_string())?;

    match live_photo_type {
        1 => {
            // Apple image: still is self; video via paired_file_id
            let video_path = file
                .paired_file_id
                .and_then(|pid| AFile::get_file_info(pid).ok().flatten())
                .and_then(|p| p.file_path)
                .filter(|p| !p.is_empty());
            Ok(LiveSources {
                live_photo_type,
                content_id,
                still_path: Some(file_path),
                video_path,
                motion: None,
            })
        }
        2 => {
            // Apple video: video is self; still via paired_file_id
            let still_path = file
                .paired_file_id
                .and_then(|pid| AFile::get_file_info(pid).ok().flatten())
                .and_then(|p| p.file_path)
                .filter(|p| !p.is_empty());
            Ok(LiveSources {
                live_photo_type,
                content_id,
                still_path,
                video_path: Some(file_path),
                motion: None,
            })
        }
        3 => {
            let motion = content_id
                .as_deref()
                .and_then(t_xmp::parse_motion_content_id)
                .ok_or_else(|| "Invalid Motion Photo content_id".to_string())?;
            // Touch app so motion cache dir stays warm for video export.
            let _ = t_xmp::motion_cache_dir(app);
            Ok(LiveSources {
                live_photo_type,
                content_id,
                still_path: Some(file_path),
                video_path: None,
                motion: Some(motion),
            })
        }
        4 => {
            // HEIC-internal video: still is the HEIC; video extracted on demand.
            let _ = t_xmp::motion_cache_dir(app);
            Ok(LiveSources {
                live_photo_type,
                content_id,
                still_path: Some(file_path),
                video_path: None,
                motion: None,
            })
        }
        other => Err(format!("Unsupported live_photo_type: {}", other)),
    }
}

fn export_still(
    sources: &LiveSources,
    dest_path: &str,
    options: &ExportLivePhotoOptions,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    let still = sources
        .still_path
        .as_ref()
        .ok_or_else(|| "No still image available for this Live Photo".to_string())?;

    if let Some(motion) = &sources.motion {
        // Strip trailing video segment from Motion Photo JPEG.
        return write_motion_still_stripped(still, motion, dest_path, policy);
    }

    // Apple: copy original still (keep format unless still_format forces later).
    let _ = options.still_format.as_ref();
    copy_to_dest_path(still, dest_path, policy)
}

fn export_video(
    app: &AppHandle,
    sources: &LiveSources,
    dest_path: &str,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    if let Some(video) = &sources.video_path {
        return copy_to_dest_path(video, dest_path, policy);
    }

    if let Some(motion) = &sources.motion {
        let still = sources
            .still_path
            .as_ref()
            .ok_or_else(|| "Motion Photo path missing".to_string())?;
        let cache_dir = t_xmp::motion_cache_dir(app)?;
        let extracted = t_xmp::extract_motion_video_to_cache(still, motion, &cache_dir)?;
        return copy_to_dest_path(&extracted, dest_path, policy);
    }

    // HEIC-internal video (type 4)
    if sources.live_photo_type == 4 {
        let still = sources
            .still_path
            .as_ref()
            .ok_or_else(|| "HEIC path missing".to_string())?;
        let cache_dir = t_xmp::motion_cache_dir(app)?;
        let extracted = extract_heic_video_cached(still, &cache_dir)?;
        return copy_to_dest_path(&extracted, dest_path, policy);
    }

    Err("No video available for this Live Photo".to_string())
}

fn extract_heic_video_cached(file_path: &str, cache_dir: &Path) -> Result<String, String> {
    #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
    {
        crate::t_heif::extract_heic_embedded_video_to_cache(file_path, cache_dir)
    }
    #[cfg(not(all(not(target_os = "macos"), lap_has_libheif)))]
    {
        let _ = (file_path, cache_dir);
        Err("HEIC embedded video extraction requires libheif".to_string())
    }
}

fn export_pair(
    app: &AppHandle,
    sources: &LiveSources,
    dest_dir: &str,
    options: &ExportLivePhotoOptions,
    policy: FileConflictPolicy,
) -> Result<Vec<String>, String> {
    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create destination folder: {}", e))?;

    let stem = pair_stem(sources);
    let mut outputs = Vec::new();

    // Still
    if let Some(still) = &sources.still_path {
        let still_ext = if sources.motion.is_some() {
            "jpg".to_string()
        } else {
            Path::new(still)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg")
                .to_lowercase()
        };
        let still_dest = PathBuf::from(dest_dir).join(format!("{}.{}", stem, still_ext));
        let still_dest_str = still_dest.to_string_lossy().to_string();
        if let Some(motion) = &sources.motion {
            outputs.push(write_motion_still_stripped(
                still,
                motion,
                &still_dest_str,
                policy,
            )?);
        } else {
            // type 4 HEIC still is the HEIC itself (copy); Apple still same.
            let _ = options.still_format.as_ref();
            outputs.push(copy_to_dest_path(still, &still_dest_str, policy)?);
        }
    } else {
        return Err("No still image available for pair export".to_string());
    }

    // Video
    let video_ext = options
        .video_format
        .as_deref()
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if let Some(video) = &sources.video_path {
                Path::new(video)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("mp4")
                    .to_lowercase()
            } else {
                "mp4".to_string()
            }
        });
    let video_dest = PathBuf::from(dest_dir).join(format!("{}.{}", stem, video_ext));
    let video_dest_str = video_dest.to_string_lossy().to_string();
    outputs.push(export_video(app, sources, &video_dest_str, policy)?);

    Ok(outputs)
}

/// Apple Live (still + video) → single Google Motion Photo JPEG.
fn export_to_motion(
    app: &AppHandle,
    sources: &LiveSources,
    dest_path: &str,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    let still = sources
        .still_path
        .as_ref()
        .ok_or_else(|| "to_motion requires a still image (Apple Live Photo image)".to_string())?;
    let video = sources.video_path.as_ref().ok_or_else(|| {
        "to_motion requires a paired video (Apple Live Photo MOV). Pair the files first."
            .to_string()
    })?;

    // Work under motion_cache so intermediate files are cleaned with video cache.
    let work_dir = t_xmp::motion_cache_dir(app)?;
    let stamp = uuid::Uuid::new_v4();
    let still_jpeg_path = work_dir.join(format!("export_still_{}.jpg", stamp));
    let video_mp4_path = work_dir.join(format!("export_vid_{}.mp4", stamp));

    // Convert still to JPEG bytes (copy if already JPEG; else decode/re-encode).
    let still_jpeg = still_to_jpeg_bytes(still, &still_jpeg_path)?;

    // Remux companion video to MP4.
    t_video::remux_or_transcode_to_mp4(video, video_mp4_path.to_string_lossy().as_ref())?;
    let video_bytes =
        fs::read(&video_mp4_path).map_err(|e| format!("Failed to read remuxed video: {}", e))?;

    let packaged = t_xmp::package_motion_photo_jpeg(&still_jpeg, &video_bytes)?;

    // Best-effort validate with detector (on temp file).
    let validate_path = work_dir.join(format!("export_motion_validate_{}.jpg", stamp));
    let _ = fs::write(&validate_path, &packaged);
    let ok = t_xmp::detect_motion_photo(validate_path.to_string_lossy().as_ref()).is_some();
    let _ = fs::remove_file(&validate_path);
    let _ = fs::remove_file(&still_jpeg_path);
    let _ = fs::remove_file(&video_mp4_path);
    if !ok {
        eprintln!(
            "Warning: packaged Motion Photo did not pass detect_motion_photo; writing anyway"
        );
    }

    let dest = resolve_final_dest(dest_path, policy)?;
    write_bytes_staged(&dest, &packaged)?;
    Ok(dest.to_string_lossy().to_string())
}

/// Motion Photo → still JPEG + video pair with optional ContentIdentifier.
fn export_to_pair(
    app: &AppHandle,
    sources: &LiveSources,
    dest_dir: &str,
    options: &ExportLivePhotoOptions,
    policy: FileConflictPolicy,
) -> Result<(Vec<String>, Option<String>), String> {
    let motion = sources
        .motion
        .as_ref()
        .ok_or_else(|| "to_pair requires a Google Motion Photo (live_photo_type=3)".to_string())?;
    let still_src = sources
        .still_path
        .as_ref()
        .ok_or_else(|| "Motion Photo path missing".to_string())?;

    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create destination folder: {}", e))?;

    let stem = pair_stem(sources);
    let still_dest = PathBuf::from(dest_dir).join(format!("{}.jpg", stem));
    let video_ext = options
        .video_format
        .as_deref()
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mp4".to_string());
    let video_dest = PathBuf::from(dest_dir).join(format!("{}.{}", stem, video_ext));

    // Still: strip trailer.
    let still_out = write_motion_still_stripped(
        still_src,
        motion,
        still_dest.to_string_lossy().as_ref(),
        policy,
    )?;

    // Video: extract then remux (and optionally stamp content id).
    let cache_dir = t_xmp::motion_cache_dir(app)?;
    let extracted = t_xmp::extract_motion_video_to_cache(still_src, motion, &cache_dir)?;

    let stamp = options.stamp_content_id.unwrap_or(true);
    let content_id = if stamp {
        Some(uuid::Uuid::new_v4().to_string().to_uppercase())
    } else {
        None
    };

    let video_tmp = cache_dir.join(format!("export_pair_vid_{}.mp4", uuid::Uuid::new_v4()));
    if let Some(cid) = content_id.as_ref() {
        t_video::remux_with_content_id(&extracted, video_tmp.to_string_lossy().as_ref(), cid)?;
    } else {
        t_video::remux_or_transcode_to_mp4(&extracted, video_tmp.to_string_lossy().as_ref())?;
    }
    let video_out = copy_to_dest_path(
        video_tmp.to_string_lossy().as_ref(),
        video_dest.to_string_lossy().as_ref(),
        policy,
    )?;
    let _ = fs::remove_file(&video_tmp);

    Ok((vec![still_out, video_out], content_id))
}

/// Export a still JPEG taken from the motion video at `keyframe_sec`.
fn export_set_keyframe(
    app: &AppHandle,
    sources: &LiveSources,
    dest_path: &str,
    options: &ExportLivePhotoOptions,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    let video_src = resolve_video_source_path(app, sources)?;
    let sec = options.keyframe_sec.unwrap_or(0.0);
    let dest = resolve_final_dest(dest_path, policy)?;
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let staged = parent.join(format!(".lap-keyframe-{}.jpg", uuid::Uuid::new_v4()));
    t_video::extract_keyframe_jpeg(&video_src, sec, staged.to_string_lossy().as_ref())?;
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    fs::rename(&staged, &dest).map_err(|e| {
        let _ = fs::remove_file(&staged);
        format!("Failed to finalize keyframe export: {}", e)
    })?;
    Ok(dest.to_string_lossy().to_string())
}

/// Replace the library still with a keyframe extracted from the motion video.
///
/// Supported:
/// - Apple Live image that is already JPEG
/// - Google Motion Photo JPEG (repackages still + existing trailer video)
///
/// Not supported (clear error): HEIC stills / type 4 HEIC-internal (would require
/// rewriting a HEIC container). Export-only keyframe remains available.
fn overwrite_still_with_keyframe(
    app: &AppHandle,
    sources: &LiveSources,
    options: &ExportLivePhotoOptions,
) -> Result<String, String> {
    let still = sources
        .still_path
        .as_ref()
        .ok_or_else(|| "No still image available to overwrite".to_string())?;
    let still_path = Path::new(still);
    if !still_path.exists() {
        return Err(format!("Still file does not exist: {}", still));
    }

    let ext = still_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_jpeg = matches!(ext.as_str(), "jpg" | "jpeg" | "jpe");

    if sources.live_photo_type == 4 || (!is_jpeg && crate::t_image::is_heic_path(still)) {
        return Err(
            "Cannot overwrite HEIC still with a keyframe JPEG. Export the keyframe as a new file instead."
                .to_string(),
        );
    }
    if !is_jpeg {
        return Err(format!(
            "Cannot overwrite still with extension '.{}'. Only JPEG stills are supported for in-place keyframe replace.",
            ext
        ));
    }

    let video_src = resolve_video_source_path(app, sources)?;
    let sec = options.keyframe_sec.unwrap_or(0.0);
    let parent = still_path.parent().unwrap_or_else(|| Path::new("."));
    let keyframe_tmp = parent.join(format!(".lap-keyframe-src-{}.jpg", uuid::Uuid::new_v4()));
    t_video::extract_keyframe_jpeg(&video_src, sec, keyframe_tmp.to_string_lossy().as_ref())?;
    let keyframe_bytes =
        fs::read(&keyframe_tmp).map_err(|e| format!("Failed to read keyframe: {}", e))?;
    if keyframe_bytes.len() < 128 {
        let _ = fs::remove_file(&keyframe_tmp);
        return Err("Keyframe extract produced empty image".to_string());
    }

    let final_bytes = if let Some(motion) = &sources.motion {
        // Motion Photo: keep embedded video trailer, replace still portion only.
        let cache_dir = t_xmp::motion_cache_dir(app)?;
        let extracted =
            t_xmp::extract_motion_video_to_cache(still, motion, &cache_dir)?;
        let video_bytes =
            fs::read(&extracted).map_err(|e| format!("Failed to read motion video: {}", e))?;
        t_xmp::package_motion_photo_jpeg(&keyframe_bytes, &video_bytes)?
    } else {
        keyframe_bytes
    };

    // Staged promote next to original (same-volume rename).
    let staged = parent.join(format!(".lap-keyframe-overwrite-{}", uuid::Uuid::new_v4()));
    fs::write(&staged, &final_bytes)
        .map_err(|e| format!("Failed to stage keyframe overwrite: {}", e))?;

    // Backup original briefly for rollback on rename failure.
    let backup = parent.join(format!(".lap-keyframe-backup-{}", uuid::Uuid::new_v4()));
    if let Err(e) = fs::rename(still_path, &backup) {
        let _ = fs::remove_file(&staged);
        let _ = fs::remove_file(&keyframe_tmp);
        return Err(format!("Failed to move original still aside: {}", e));
    }
    if let Err(e) = fs::rename(&staged, still_path) {
        // Roll back original
        let _ = fs::rename(&backup, still_path);
        let _ = fs::remove_file(&staged);
        let _ = fs::remove_file(&keyframe_tmp);
        return Err(format!("Failed to promote keyframe still: {}", e));
    }
    let _ = fs::remove_file(&backup);
    let _ = fs::remove_file(&keyframe_tmp);

    Ok(still.to_string())
}

fn resolve_video_source_path(app: &AppHandle, sources: &LiveSources) -> Result<String, String> {
    if let Some(video) = &sources.video_path {
        return Ok(video.clone());
    }
    if let Some(motion) = &sources.motion {
        let still = sources
            .still_path
            .as_ref()
            .ok_or_else(|| "Motion Photo path missing".to_string())?;
        let cache_dir = t_xmp::motion_cache_dir(app)?;
        return t_xmp::extract_motion_video_to_cache(still, motion, &cache_dir);
    }
    if sources.live_photo_type == 4 {
        let still = sources
            .still_path
            .as_ref()
            .ok_or_else(|| "HEIC path missing".to_string())?;
        let cache_dir = t_xmp::motion_cache_dir(app)?;
        return extract_heic_video_cached(still, &cache_dir);
    }
    Err("No video available for keyframe export".to_string())
}

/// Produce JPEG bytes for packaging: copy JPEG files, decode HEIC/other via image/libheif path.
fn still_to_jpeg_bytes(still_path: &str, work_jpeg_path: &Path) -> Result<Vec<u8>, String> {
    let ext = Path::new(still_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "jpg" | "jpeg" | "jpe") {
        // If this is a Motion Photo already, strip trailer first.
        if let Some(info) = t_xmp::detect_motion_photo(still_path) {
            return t_xmp::read_motion_still_bytes(still_path, &info);
        }
        return fs::read(still_path).map_err(|e| format!("Failed to read still JPEG: {}", e));
    }

    // HEIC / other: prefer libheif preview, then generic image open.
    #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
    if crate::t_image::is_heic_path(still_path) {
        if let Ok(Some(bytes)) = crate::t_heif::get_heif_preview(still_path, 1, 8192) {
            fs::write(work_jpeg_path, &bytes)
                .map_err(|e| format!("Failed to write HEIC JPEG: {}", e))?;
            return Ok(bytes);
        }
    }

    let img = image::open(still_path)
        .map_err(|e| format!("Failed to open still image for JPEG conversion: {}", e))?;
    let rgb = img.to_rgb8();
    let bytes = crate::t_image::resize_dynamic_image_to_jpeg(
        image::DynamicImage::ImageRgb8(rgb),
        1,
        // Use a large "thumbnail" size so resize is effectively a full encode.
        16384,
    )?;
    fs::write(work_jpeg_path, &bytes).map_err(|e| format!("Failed to write work JPEG: {}", e))?;
    Ok(bytes)
}

fn pair_stem(sources: &LiveSources) -> String {
    let path = sources
        .still_path
        .as_ref()
        .or(sources.video_path.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("live_photo");
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("live_photo")
        .to_string()
}

/// Copy `source` to an exact `dest_path`, applying conflict policy on the basename.
fn copy_to_dest_path(
    source: &str,
    dest_path: &str,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    let dest = Path::new(dest_path);
    let parent = dest
        .parent()
        .ok_or_else(|| format!("Invalid destination path: {}", dest_path))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create destination folder: {}", e))?;

    // Prefer policy-aware copy into the folder, then rename if the caller
    // requested a different final name than the source basename.
    let result = copy_file_with_policy(source, &parent.to_string_lossy(), policy)?;
    let written = PathBuf::from(&result.path);

    let desired_name = dest
        .file_name()
        .ok_or_else(|| format!("Invalid destination path: {}", dest_path))?;
    if written.file_name() == Some(desired_name) {
        return Ok(result.path);
    }

    let mut final_path = parent.join(desired_name);
    if final_path.exists() {
        match policy {
            FileConflictPolicy::Replace => {
                fs::remove_file(&final_path)
                    .map_err(|e| format!("Failed to replace existing file: {}", e))?;
            }
            FileConflictPolicy::KeepBoth => {
                final_path = unique_path(final_path);
            }
        }
    }
    if let Err(rename_err) = fs::rename(&written, &final_path) {
        // Fall back to copy+delete if cross-device rename fails.
        fs::copy(&written, &final_path).map_err(|copy_err| {
            format!(
                "Failed to place export at {}: rename {} / copy {}",
                final_path.display(),
                rename_err,
                copy_err
            )
        })?;
        let _ = fs::remove_file(&written);
    }
    Ok(final_path.to_string_lossy().to_string())
}

fn write_motion_still_stripped(
    source: &str,
    motion: &MotionPhotoInfo,
    dest_path: &str,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    if motion.video_offset == 0 {
        return Err("Motion Photo video offset is zero; still export aborted".to_string());
    }

    let mut file = fs::File::open(source).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut still_buf = vec![0u8; motion.video_offset as usize];
    file.read_exact(&mut still_buf)
        .map_err(|e| format!("Failed to read still segment: {}", e))?;

    // Basic JPEG SOI check for safety.
    if still_buf.len() < 2 || still_buf[0] != 0xFF || still_buf[1] != 0xD8 {
        return Err("Motion Photo still segment does not look like a JPEG".to_string());
    }

    let dest = resolve_final_dest(dest_path, policy)?;
    write_bytes_staged(&dest, &still_buf)?;
    Ok(dest.to_string_lossy().to_string())
}

fn resolve_final_dest(dest_path: &str, policy: FileConflictPolicy) -> Result<PathBuf, String> {
    let mut dest = PathBuf::from(dest_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create destination folder: {}", e))?;
    }
    if dest.exists() {
        match policy {
            FileConflictPolicy::Replace => {
                fs::remove_file(&dest)
                    .map_err(|e| format!("Failed to replace existing file: {}", e))?;
            }
            FileConflictPolicy::KeepBoth => {
                dest = unique_path(dest);
            }
        }
    }
    Ok(dest)
}

fn write_bytes_staged(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let staged = parent.join(format!(".lap-export-{}", uuid::Uuid::new_v4()));
    {
        let mut f = fs::File::create(&staged)
            .map_err(|e| format!("Failed to create staged export: {}", e))?;
        f.write_all(bytes)
            .map_err(|e| format!("Failed to write staged export: {}", e))?;
        f.sync_all()
            .map_err(|e| format!("Failed to sync staged export: {}", e))?;
    }
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(&staged, dest).map_err(|e| {
        let _ = fs::remove_file(&staged);
        format!("Failed to finalize export at {}: {}", dest.display(), e)
    })?;
    Ok(())
}

fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    let mut i = 1u32;
    loop {
        let candidate = parent.join(format!("{}_{}{}", stem, i, ext));
        if !candidate.exists() {
            return candidate;
        }
        i += 1;
    }
}

/// Helper used by video export path validation in tests/debug.
#[allow(dead_code)]
fn seek_ok(path: &str, offset: u64) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    f.seek(SeekFrom::Start(offset)).is_ok()
}
