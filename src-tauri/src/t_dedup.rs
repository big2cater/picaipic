use crate::t_sqlite::{AFile, AThumb, QueryParams};
use crate::t_utils;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;

/// Max Hamming distance for dHash pairs to share a similar-duplicate group.
const DHASH_HAMMING_THRESHOLD: u32 = 8;

// ----------------------------------------------------------------------------
// Types and Structs
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DedupScanStatus {
    pub state: String, // "running", "idle", "finished", "error"
    pub processed: u64,
    pub total: u64,
    pub groups: u64,
    pub is_scanning: bool,
}

impl Default for DedupScanStatus {
    fn default() -> Self {
        Self {
            state: "idle".to_string(),
            processed: 0,
            total: 0,
            groups: 0,
            is_scanning: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DedupDeleteResult {
    pub deleted_file_ids: Vec<i64>,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

#[derive(Default)]
pub struct DedupState {
    pub is_scanning: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
    pub status: Arc<Mutex<DedupScanStatus>>,
}

#[derive(Clone)]
struct KeepCandidate {
    id: i64,
    taken_date: i64,
    created_at: i64,
}

// ----------------------------------------------------------------------------
// Core Logic
// ----------------------------------------------------------------------------

pub fn start_scan(
    app_handle: tauri::AppHandle,
    dedup_state: tauri::State<'_, DedupState>,
    query_params: Option<QueryParams>,
    mode: Option<String>,
) -> Result<(), String> {
    if dedup_state
        .is_scanning
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A deduplication scan is already running.".into());
    }
    dedup_state.cancel_flag.store(false, Ordering::SeqCst);

    let status_clone = dedup_state.status.clone();
    let is_scanning_clone = dedup_state.is_scanning.clone();
    let cancel_flag_clone = dedup_state.cancel_flag.clone();
    let similar = matches!(
        mode.as_deref().map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("similar")
    );

    // Reset status
    {
        let mut status = status_clone.lock().unwrap();
        status.state = "running".to_string();
        status.processed = 0;
        status.total = 0;
        status.groups = 0;
        status.is_scanning = true;
    }

    std::thread::spawn(move || {
        let result = if similar {
            scan_and_phash_files(&app_handle, &status_clone, &cancel_flag_clone, query_params)
        } else {
            scan_and_hash_files(&app_handle, &status_clone, &cancel_flag_clone, query_params)
        };

        let mut final_status = status_clone.lock().unwrap();
        match result {
            Ok(_) => {
                if cancel_flag_clone.load(Ordering::SeqCst) {
                    final_status.state = "idle".to_string();
                } else {
                    final_status.state = "finished".to_string();
                }
            }
            Err(e) => {
                eprintln!("Dedup scan error: {}", e);
                final_status.state = "error".to_string();
            }
        }

        is_scanning_clone.store(false, Ordering::SeqCst);
        final_status.is_scanning = false;
        let _ = app_handle.emit("dedup-scan-progress", final_status.clone());
    });

    Ok(())
}

fn scan_and_hash_files(
    app_handle: &tauri::AppHandle,
    status_mutex: &Arc<Mutex<DedupScanStatus>>,
    cancel_flag: &Arc<AtomicBool>,
    query_params: Option<QueryParams>,
) -> Result<(), String> {
    let mut conn = get_db_conn()?;
    let has_scope = query_params.is_some();

    let files_to_check = if let Some(params) = query_params.as_ref() {
        get_files_by_query(params)?
    } else {
        // Step 1: Find suspicious sizes (sizes shared by >1 file)
        let suspicious_sizes = get_suspicious_file_sizes(&conn)?;
        if suspicious_sizes.is_empty() {
            rebuild_duplicate_groups(&mut conn, None)?;
            return Ok(());
        }
        // Step 2: Get all files with those sizes
        get_files_by_sizes(&conn, &suspicious_sizes)?
    };

    let files_to_check = filter_suspicious_files(files_to_check);
    let scoped_file_ids = if has_scope {
        Some(
            files_to_check
                .iter()
                .filter_map(|file| file.id)
                .collect::<Vec<i64>>(),
        )
    } else {
        None
    };
    if files_to_check.is_empty() {
        rebuild_duplicate_groups(&mut conn, scoped_file_ids.as_deref())?;
        return Ok(());
    }

    let total_files = files_to_check.len() as u64;
    {
        let mut status = status_mutex.lock().unwrap();
        status.total = total_files;
        status.processed = 0;
    }
    let _ = app_handle.emit("dedup-scan-progress", status_mutex.lock().unwrap().clone());

    // Step 3: Hash them
    let mut processed = 0;

    // We do batch inserts to speed up DB operations
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for file in files_to_check {
        if cancel_flag.load(Ordering::SeqCst) {
            break;
        }

        // Only hash if mtime changed or hash is missing
        let needs_hash = check_if_needs_hash(&tx, &file)?;

        if needs_hash {
            let Some(file_id) = file.id else {
                continue;
            };
            if let Some(path) = &file.file_path {
                match compute_blake3_hash(path) {
                    Ok(hash) => {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        let mtime = file.modified_at.unwrap_or(0);

                        tx.execute(
                            "INSERT OR REPLACE INTO file_hashes (file_id, hash, file_size, mtime, computed_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![file_id, hash, file.size, mtime, now],
                        ).map_err(|e| e.to_string())?;
                    }
                    Err(e) => eprintln!("Failed to hash file {}: {}", path, e),
                }
            }
        }

        processed += 1;
        if processed % 10 == 0 {
            {
                let mut status = status_mutex.lock().unwrap();
                status.processed = processed;
            }
            let _ = app_handle.emit("dedup-scan-progress", status_mutex.lock().unwrap().clone());
        }
    }

    tx.commit().map_err(|e| e.to_string())?;

    // Step 4: Rebuild duplicate groups
    if !cancel_flag.load(Ordering::SeqCst) {
        rebuild_duplicate_groups(&mut conn, scoped_file_ids.as_deref())?;

        // Count total groups
        let groups_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM duplicate_groups", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        {
            let mut status = status_mutex.lock().unwrap();
            status.processed = processed;
            status.groups = groups_count as u64;
        }
        let _ = app_handle.emit("dedup-scan-progress", status_mutex.lock().unwrap().clone());
    }

    Ok(())
}

fn get_db_conn() -> Result<Connection, String> {
    let path = crate::t_storage::get_current_db_path()
        .map_err(|e| format!("Failed to get db path: {}", e))?;
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open db: {}", e))?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;
    Ok(conn)
}

fn get_suspicious_file_sizes(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT size 
         FROM afiles 
         GROUP BY size 
         HAVING COUNT(size) > 1 AND size > 0",
        )
        .map_err(|e| e.to_string())?;

    let sizes = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(sizes)
}

fn get_files_by_sizes(conn: &Connection, sizes: &[i64]) -> Result<Vec<AFile>, String> {
    if sizes.is_empty() {
        return Ok(Vec::new());
    }

    // Reuse sizes already computed by get_suspicious_file_sizes.
    // SQLite default max variable count is 999; chunk to stay under the limit.
    const CHUNK_SIZE: usize = 900;
    let select_sql = "SELECT a.id, a.folder_id, a.name, a.name_pinyin, a.size, a.file_type, a.format_label,
                a.created_at, a.modified_at, a.inode, a.taken_date, a.width, a.height, a.duration,
                a.is_favorite, a.rating, a.rotate, a.comments, a.has_tags, a.has_faces, 
                a.e_make, a.e_model, a.e_date_time, a.e_software, a.e_artist, a.e_copyright, 
                a.e_description, a.e_lens_make, a.e_lens_model, a.e_exposure_bias, a.e_exposure_time, 
                a.e_f_number, a.e_focal_length, a.e_iso_speed, a.e_flash, a.e_orientation, 
                a.gps_latitude, a.gps_longitude, a.gps_altitude, 
                a.geo_name, a.geo_admin1, a.geo_admin2, a.geo_cc,
                f.path || '/' || a.name as file_path
         FROM afiles a
         JOIN afolders f ON a.folder_id = f.id
         WHERE a.size IN (";

    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<AFile> {
        Ok(AFile {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            name: row.get(2)?,
            name_pinyin: row.get(3)?,
            size: row.get(4)?,
            file_type: row.get(5)?,
            format_label: row.get(6)?,
            created_at: row.get(7)?,
            modified_at: row.get(8)?,
            inode: row.get(9)?,
            taken_date: row.get(10)?,
            width: row.get(11)?,
            height: row.get(12)?,
            duration: row.get(13)?,
            is_favorite: row.get(14)?,
            rating: row.get(15)?,
            rotate: row.get(16)?,
            comments: row.get(17)?,
            has_tags: row.get(18)?,
            has_faces: row.get(19)?,
            e_make: row.get(20)?,
            e_model: row.get(21)?,
            e_date_time: row.get(22)?,
            e_software: row.get(23)?,
            e_artist: row.get(24)?,
            e_copyright: row.get(25)?,
            e_description: row.get(26)?,
            e_lens_make: row.get(27)?,
            e_lens_model: row.get(28)?,
            e_exposure_bias: row.get(29)?,
            e_exposure_time: row.get(30)?,
            e_f_number: row.get(31)?,
            e_focal_length: row.get(32)?,
            e_iso_speed: row.get(33)?,
            e_flash: row.get(34)?,
            e_orientation: row.get(35)?,
            gps_latitude: row.get(36)?,
            gps_longitude: row.get(37)?,
            gps_altitude: row.get(38)?,
            geo_name: row.get(39)?,
            geo_admin1: row.get(40)?,
            geo_admin2: row.get(41)?,
            geo_cc: row.get(42)?,
            file_path: row.get(43)?,
            album_id: None,
            album_name: None,
            has_thumbnail: None,
            has_embedding: None,
            last_scan_time: Some(0),
            content_id: None,
            paired_file_id: None,
            live_photo_type: Some(0),
        })
    };

    let mut files = Vec::new();
    for chunk in sizes.chunks(CHUNK_SIZE) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let query = format!("{}{}) ORDER BY a.size DESC", select_sql, placeholders);
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let iter = stmt
            .query_map(rusqlite::params_from_iter(chunk.iter()), map_row)
            .map_err(|e| e.to_string())?;

        for f in iter {
            if let Ok(file) = f {
                files.push(file);
            }
        }
    }

    Ok(files)
}

fn get_files_by_query(params: &QueryParams) -> Result<Vec<AFile>, String> {
    let mut all_files = Vec::new();
    let mut offset: i64 = 0;
    let chunk_size: i64 = 2000;

    loop {
        let files = AFile::get_query_files(params, offset, chunk_size)?;
        let fetched = files.len() as i64;
        if fetched == 0 {
            break;
        }
        all_files.extend(files);
        if fetched < chunk_size {
            break;
        }
        offset += chunk_size;
    }

    Ok(all_files)
}

fn filter_suspicious_files(files: Vec<AFile>) -> Vec<AFile> {
    let mut size_count: HashMap<i64, usize> = HashMap::new();
    for file in &files {
        if file.size > 0 {
            *size_count.entry(file.size).or_insert(0) += 1;
        }
    }

    files
        .into_iter()
        .filter(|file| file.size > 0 && size_count.get(&file.size).copied().unwrap_or(0) > 1)
        .collect()
}

fn check_if_needs_hash(conn: &Connection, file: &AFile) -> Result<bool, String> {
    let mtime = file.modified_at.unwrap_or(0);
    let Some(id) = file.id else {
        // Rows without an id cannot be hashed into file_hashes; treat as no-op.
        return Ok(false);
    };

    let db_mtime: Option<i64> = conn
        .query_row(
            "SELECT mtime FROM file_hashes WHERE file_id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    match db_mtime {
        Some(stored_mtime) => Ok(stored_mtime != mtime),
        None => Ok(true),
    }
}

/// Difference hash (dHash) 64-bit from grayscale 9x8 → horizontal gradients.
fn compute_dhash_u64_from_gray(gray: &image::GrayImage) -> u64 {
    // Expect at least 9x8; resize caller guarantees.
    let mut hash: u64 = 0;
    let mut bit: u64 = 1;
    for y in 0..8 {
        for x in 0..8 {
            let left = gray.get_pixel(x, y)[0];
            let right = gray.get_pixel(x + 1, y)[0];
            if left > right {
                hash |= bit;
            }
            bit <<= 1;
        }
    }
    hash
}

fn hamming64(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

fn compute_dhash_for_file(file: &AFile) -> Option<u64> {
    let file_id = file.id?;
    // Prefer stored thumbnail (fast, consistent).
    if let Ok(Some(thumb)) = AThumb::fetch(file_id) {
        if let Some(bytes) = thumb.thumb_data {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let gray = img
                    .resize_exact(9, 8, image::imageops::FilterType::Triangle)
                    .to_luma8();
                return Some(compute_dhash_u64_from_gray(&gray));
            }
        }
    }
    // Fallback: open original (images only).
    let path = file.file_path.as_ref()?;
    let ft = file.file_type.unwrap_or(0);
    if ft != 1 && ft != 3 {
        return None;
    }
    let img = image::open(path).ok()?;
    let gray = img
        .resize_exact(9, 8, image::imageops::FilterType::Triangle)
        .to_luma8();
    Some(compute_dhash_u64_from_gray(&gray))
}

fn check_if_needs_phash(conn: &Connection, file: &AFile) -> Result<bool, String> {
    let Some(file_id) = file.id else {
        return Ok(false);
    };
    let mtime = file.modified_at.unwrap_or(0);
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT hash, mtime FROM file_phashes WHERE file_id = ?1",
            params![file_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match row {
        None => Ok(true),
        Some((_h, stored_mtime)) => Ok(stored_mtime != mtime),
    }
}

fn scan_and_phash_files(
    app_handle: &tauri::AppHandle,
    status_mutex: &Arc<Mutex<DedupScanStatus>>,
    cancel_flag: &Arc<AtomicBool>,
    query_params: Option<QueryParams>,
) -> Result<(), String> {
    let mut conn = get_db_conn()?;
    let _ = crate::t_migration::ensure_similar_dedup_tables(&conn);

    let files_to_check = if let Some(params) = query_params.as_ref() {
        get_files_by_query(params)?
    } else {
        // Whole library: images + RAW only (type 1/3).
        let mut stmt = conn
            .prepare("SELECT a.id FROM afiles a WHERE a.file_type IN (1, 3)")
            .map_err(|e| e.to_string())?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        if ids.is_empty() {
            rebuild_similar_groups(&mut conn, None)?;
            return Ok(());
        }
        let mut out = Vec::new();
        for chunk in ids.chunks(500) {
            let map = AFile::get_files_by_ids(chunk).map_err(|e| e.to_string())?;
            for id in chunk {
                if let Some(f) = map.get(id) {
                    out.push(f.clone());
                }
            }
        }
        out
    };

    let files_to_check: Vec<AFile> = files_to_check
        .into_iter()
        .filter(|f| {
            let t = f.file_type.unwrap_or(0);
            t == 1 || t == 3
        })
        .collect();

    let scoped_file_ids = if query_params.is_some() {
        Some(
            files_to_check
                .iter()
                .filter_map(|f| f.id)
                .collect::<Vec<i64>>(),
        )
    } else {
        None
    };

    if files_to_check.is_empty() {
        rebuild_similar_groups(&mut conn, scoped_file_ids.as_deref())?;
        return Ok(());
    }

    let total_files = files_to_check.len() as u64;
    {
        let mut status = status_mutex.lock().unwrap();
        status.total = total_files;
        status.processed = 0;
    }
    let _ = app_handle.emit("dedup-scan-progress", status_mutex.lock().unwrap().clone());

    let mut processed = 0u64;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for file in &files_to_check {
        if cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        let needs = check_if_needs_phash(&tx, file)?;
        if needs {
            if let (Some(file_id), Some(hash)) = (file.id, compute_dhash_for_file(file)) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let mtime = file.modified_at.unwrap_or(0);
                tx.execute(
                    "INSERT OR REPLACE INTO file_phashes (file_id, hash, mtime, computed_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![file_id, hash as i64, mtime, now],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        processed += 1;
        if processed % 10 == 0 {
            {
                let mut status = status_mutex.lock().unwrap();
                status.processed = processed;
            }
            let _ = app_handle.emit("dedup-scan-progress", status_mutex.lock().unwrap().clone());
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    if !cancel_flag.load(Ordering::SeqCst) {
        rebuild_similar_groups(&mut conn, scoped_file_ids.as_deref())?;
        let groups_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM similar_duplicate_groups",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        {
            let mut status = status_mutex.lock().unwrap();
            status.processed = processed;
            status.groups = groups_count as u64;
        }
        let _ = app_handle.emit("dedup-scan-progress", status_mutex.lock().unwrap().clone());
    }
    Ok(())
}

/// Union-Find for similar groups by Hamming distance on dHash.
fn rebuild_similar_groups(
    conn: &mut Connection,
    scope_file_ids: Option<&[i64]>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Scoped rebuild: only remove groups that touch the scope (keep global results outside).
    // Full rebuild: clear all similar groups.
    if let Some(scope_ids) = scope_file_ids {
        tx.execute("DROP TABLE IF EXISTS temp_scope_ids", [])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "CREATE TEMP TABLE temp_scope_ids (file_id INTEGER PRIMARY KEY)",
            [],
        )
        .map_err(|e| e.to_string())?;
        if scope_ids.is_empty() {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        }
        {
            let mut ins = tx
                .prepare("INSERT OR IGNORE INTO temp_scope_ids (file_id) VALUES (?1)")
                .map_err(|e| e.to_string())?;
            for id in scope_ids {
                ins.execute(params![id]).map_err(|e| e.to_string())?;
            }
        }
        tx.execute(
            "DELETE FROM similar_duplicate_group_items
             WHERE group_id IN (
               SELECT DISTINCT group_id FROM similar_duplicate_group_items
               WHERE file_id IN (SELECT file_id FROM temp_scope_ids)
             )",
            [],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM similar_duplicate_groups
             WHERE id NOT IN (SELECT DISTINCT group_id FROM similar_duplicate_group_items)",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        tx.execute("DELETE FROM similar_duplicate_group_items", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM similar_duplicate_groups", [])
            .map_err(|e| e.to_string())?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let rows: Vec<(i64, u64, i64, i64, i64)> = {
        let sql = if scope_file_ids.is_some() {
            "SELECT fp.file_id, fp.hash, a.size, a.taken_date, a.created_at
             FROM file_phashes fp
             JOIN afiles a ON a.id = fp.file_id
             JOIN temp_scope_ids ts ON ts.file_id = fp.file_id"
        } else {
            "SELECT fp.file_id, fp.hash, a.size, a.taken_date, a.created_at
             FROM file_phashes fp
             JOIN afiles a ON a.id = fp.file_id"
        };
        let mut stmt = tx.prepare(sql).map_err(|e| e.to_string())?;
        let iter = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2).unwrap_or(0),
                    row.get::<_, i64>(3).unwrap_or(0),
                    row.get::<_, i64>(4).unwrap_or(0),
                ))
            })
            .map_err(|e| e.to_string())?;
        iter.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    if rows.len() < 2 {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let n = rows.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    fn union(p: &mut [usize], a: usize, b: usize) {
        let ra = find(p, a);
        let rb = find(p, b);
        if ra != rb {
            p[rb] = ra;
        }
    }

    // O(n²) Hamming — OK for scoped/medium libraries; whole-library large N may be slow
    // but still acceptable for offline dedup scan (cancelable outer loop already ran).
    for i in 0..n {
        for j in (i + 1)..n {
            if hamming64(rows[i].1, rows[j].1) <= DHASH_HAMMING_THRESHOLD {
                union(&mut parent, i, j);
            }
        }
    }

    let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        clusters.entry(r).or_default().push(i);
    }

    for (_root, members) in clusters {
        if members.len() < 2 {
            continue;
        }
        let mut candidates: Vec<KeepCandidate> = members
            .iter()
            .map(|&i| KeepCandidate {
                id: rows[i].0,
                taken_date: rows[i].3,
                created_at: rows[i].4,
            })
            .collect();
        candidates.sort_by(compare_best_quality);

        // Representative hash string + max size for reclaimable estimate
        let rep_hash = format!("{:016x}", rows[members[0]].1);
        let max_size = members
            .iter()
            .map(|&i| rows[i].2)
            .max()
            .unwrap_or(0);
        let count = members.len() as i64;
        let total_size = members.iter().map(|&i| rows[i].2).sum::<i64>();

        tx.execute(
            "INSERT INTO similar_duplicate_groups (hash, file_size, file_count, total_size, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![rep_hash, max_size, count, total_size, now],
        )
        .map_err(|e| e.to_string())?;
        let gid = tx.last_insert_rowid();
        let total_c = candidates.len() as f64;
        for (i, c) in candidates.iter().enumerate() {
            let is_keep = if i == 0 { 1 } else { 0 };
            let score = total_c - i as f64;
            tx.execute(
                "INSERT INTO similar_duplicate_group_items (group_id, file_id, is_keep, is_selected, score)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![gid, c.id, is_keep, 0, score],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn compute_blake3_hash(path: &str) -> Result<String, io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();

    // Read in chunks
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

fn rebuild_duplicate_groups(
    conn: &mut Connection,
    scope_file_ids: Option<&[i64]>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Scoped rebuild: only drop groups that touch the scope (preserve out-of-scope exact groups).
    // Full rebuild: clear all exact groups.
    if let Some(scope_ids) = scope_file_ids {
        tx.execute("DROP TABLE IF EXISTS temp_scope_ids", [])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "CREATE TEMP TABLE temp_scope_ids (file_id INTEGER PRIMARY KEY)",
            [],
        )
        .map_err(|e| e.to_string())?;

        if scope_ids.is_empty() {
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(());
        }

        {
            let mut insert_stmt = tx
                .prepare("INSERT OR IGNORE INTO temp_scope_ids (file_id) VALUES (?1)")
                .map_err(|e| e.to_string())?;
            for file_id in scope_ids {
                insert_stmt
                    .execute(params![file_id])
                    .map_err(|e| e.to_string())?;
            }
        }

        // Exact groups are keyed by (hash, size). Drop any group whose hash+size still appears
        // among scoped files (those buckets will be rebuilt from scope). Also drop groups that
        // contain any scoped member so stale membership cannot linger.
        tx.execute(
            "DELETE FROM duplicate_group_items
             WHERE group_id IN (
               SELECT DISTINCT dgi.group_id
               FROM duplicate_group_items dgi
               WHERE dgi.file_id IN (SELECT file_id FROM temp_scope_ids)
             )
             OR group_id IN (
               SELECT dg.id FROM duplicate_groups dg
               WHERE EXISTS (
                 SELECT 1 FROM file_hashes fh
                 JOIN temp_scope_ids ts ON ts.file_id = fh.file_id
                 WHERE fh.hash = dg.hash AND fh.file_size = dg.file_size
               )
             )",
            [],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM duplicate_groups
             WHERE id NOT IN (SELECT DISTINCT group_id FROM duplicate_group_items)",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        tx.execute("DELETE FROM duplicate_group_items", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM duplicate_groups", [])
            .map_err(|e| e.to_string())?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Find dups
    let group_query = if scope_file_ids.is_some() {
        "SELECT fh.hash, fh.file_size, COUNT(fh.file_id) as cnt
         FROM file_hashes fh
         JOIN temp_scope_ids ts ON ts.file_id = fh.file_id
         GROUP BY fh.hash, fh.file_size
         HAVING cnt > 1"
    } else {
        "SELECT hash, file_size, COUNT(file_id) as cnt
         FROM file_hashes
         GROUP BY hash, file_size
         HAVING cnt > 1"
    };

    let mut stmt = tx.prepare(group_query).map_err(|e| e.to_string())?;

    let rows: Vec<(String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    for (hash, size, count) in rows {
        let total_size = size * count;

        // Insert group
        tx.execute(
            "INSERT INTO duplicate_groups (hash, file_size, file_count, total_size, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hash, size, count, total_size, now],
        )
        .map_err(|e| e.to_string())?;

        // target_group_id computation is replaced by retrieving it exactly via last_insert_rowid if it works
        // or re-query. We just use last_insert_rowid as an SQLite function on connection, but for tx we do:
        let target_group_id = tx.last_insert_rowid();

        // Let's get the files for this group
        let item_query = if scope_file_ids.is_some() {
            "SELECT a.id, a.taken_date, a.created_at
             FROM file_hashes fh
             JOIN afiles a ON fh.file_id = a.id
             JOIN temp_scope_ids ts ON ts.file_id = a.id
             WHERE fh.hash = ?1 AND fh.file_size = ?2"
        } else {
            "SELECT a.id, a.taken_date, a.created_at
             FROM file_hashes fh
             JOIN afiles a ON fh.file_id = a.id
             WHERE fh.hash = ?1 AND fh.file_size = ?2"
        };

        let mut f_stmt = tx.prepare(item_query).map_err(|e| e.to_string())?;

        let mut keep_candidates: Vec<KeepCandidate> = Vec::new();
        let iter = f_stmt
            .query_map(params![hash, size], |row| {
                let id: i64 = row.get(0)?;
                let tk: i64 = row.get(1).unwrap_or(0);
                let ca: i64 = row.get(2).unwrap_or(0);
                Ok((id, tk, ca))
            })
            .map_err(|e| e.to_string())?;

        for r in iter {
            let (id, tk, ca) = r.map_err(|e| e.to_string())?;
            keep_candidates.push(KeepCandidate {
                id,
                taken_date: tk,
                created_at: ca,
            });
        }

        keep_candidates.sort_by(compare_best_quality);

        let total_candidates = keep_candidates.len() as f64;
        for (i, candidate) in keep_candidates.iter().enumerate() {
            let is_keep = if i == 0 { 1 } else { 0 };
            let score = total_candidates - i as f64;

            tx.execute(
                "INSERT INTO duplicate_group_items (group_id, file_id, is_keep, is_selected, score)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![target_group_id, candidate.id, is_keep, 0, score],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    drop(stmt);

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn compare_best_quality(a: &KeepCandidate, b: &KeepCandidate) -> std::cmp::Ordering {
    (b.taken_date > 0)
        .cmp(&(a.taken_date > 0))
        .then_with(|| a.created_at.cmp(&b.created_at))
        .then_with(|| a.id.cmp(&b.id))
}

// ----------------------------------------------------------------------------
// Retrieval APIs
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct DedupGroup {
    pub id: i64,
    pub hash: String,
    pub file_size: i64,
    pub file_count: i64,
    pub total_size: i64,
    pub reviewed: i32,
    pub updated_at: i64,
    pub items: Vec<DedupGroupItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DedupGroupItem {
    pub group_id: i64,
    pub file_id: i64,
    pub is_keep: i32,
    pub is_selected: i32,
    pub score: f64,
    pub file: Option<AFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DedupOverview {
    pub total_groups: i64,
    pub total_files: i64,
    pub total_reclaimable_bytes: i64,
}

pub fn get_overview() -> Result<DedupOverview, String> {
    get_overview_for_tables("duplicate_groups", "duplicate_group_items")
}

pub fn get_similar_overview() -> Result<DedupOverview, String> {
    let conn = get_db_conn()?;
    let _ = crate::t_migration::ensure_similar_dedup_tables(&conn);
    get_overview_for_tables("similar_duplicate_groups", "similar_duplicate_group_items")
}

fn get_overview_for_tables(groups: &str, items: &str) -> Result<DedupOverview, String> {
    let conn = get_db_conn()?;
    let sql = format!(
        "SELECT 
            COALESCE(COUNT(*), 0),
            COALESCE(SUM(current_file_count - 1), 0),
            COALESCE(SUM((current_file_count - 1) * file_size), 0)
         FROM (
            SELECT file_size,
                   (SELECT COUNT(*)
                    FROM {items}
                    WHERE group_id = {groups}.id) AS current_file_count
            FROM {groups}
         )
         WHERE current_file_count > 1"
    );
    let (total_groups, total_files, total_reclaimable_bytes) = conn
        .query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| e.to_string())?;

    Ok(DedupOverview {
        total_groups,
        total_files,
        total_reclaimable_bytes,
    })
}

pub fn list_groups(
    page: u32,
    page_size: u32,
    sort_by: &str,
    filter: &str,
) -> Result<Vec<DedupGroup>, String> {
    list_groups_from(
        "duplicate_groups",
        "duplicate_group_items",
        page,
        page_size,
        sort_by,
        filter,
        false,
    )
}

pub fn list_similar_groups(
    page: u32,
    page_size: u32,
    sort_by: &str,
    filter: &str,
) -> Result<Vec<DedupGroup>, String> {
    let conn = get_db_conn()?;
    let _ = crate::t_migration::ensure_similar_dedup_tables(&conn);
    list_groups_from(
        "similar_duplicate_groups",
        "similar_duplicate_group_items",
        page,
        page_size,
        sort_by,
        filter,
        true,
    )
}

fn list_groups_from(
    groups_table: &str,
    items_table: &str,
    page: u32,
    page_size: u32,
    sort_by: &str,
    filter: &str,
    similar: bool,
) -> Result<Vec<DedupGroup>, String> {
    let conn = get_db_conn()?;
    let offset = (page.saturating_sub(1)) * page_size;

    let order_clause = match sort_by {
        "size_desc" => "cur_size DESC",
        "size_asc" => "cur_size ASC",
        "count_desc" => "cur_count DESC",
        "count_asc" => "cur_count ASC",
        _ => "cur_size DESC",
    };

    let filter_clause = match filter {
        "unreviewed" => format!(
            "WHERE reviewed = 0 AND (SELECT COUNT(*) FROM {items_table} WHERE group_id = {groups_table}.id) > 1"
        ),
        "reviewed" => format!(
            "WHERE reviewed = 1 AND (SELECT COUNT(*) FROM {items_table} WHERE group_id = {groups_table}.id) > 1"
        ),
        _ => format!(
            "WHERE (SELECT COUNT(*) FROM {items_table} WHERE group_id = {groups_table}.id) > 1"
        ),
    };

    // Exact groups use file_size * count for reclaimable; similar stores true total_size sum.
    let size_expr = if similar {
        format!(
            "(SELECT COALESCE(SUM(a.size), 0) FROM {items_table} gi JOIN afiles a ON a.id = gi.file_id WHERE gi.group_id = {groups_table}.id)"
        )
    } else {
        format!(
            "((SELECT COUNT(*) FROM {items_table} WHERE group_id = {groups_table}.id) * file_size)"
        )
    };

    let query = format!(
        "SELECT id, hash, file_size, 
                (SELECT COUNT(*) FROM {items_table} WHERE group_id = {groups_table}.id) as cur_count,
                {size_expr} as cur_size,
                reviewed, updated_at
         FROM {groups_table}
         {filter_clause}
         ORDER BY {order_clause}
         {}",
        if page_size == 0 {
            ""
        } else {
            "LIMIT ?1 OFFSET ?2"
        }
    );

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
    let query_params = if page_size == 0 {
        Vec::new()
    } else {
        vec![page_size, offset]
    };
    let groups_iter = stmt
        .query_map(rusqlite::params_from_iter(query_params), |row| {
            Ok(DedupGroup {
                id: row.get(0)?,
                hash: row.get(1)?,
                file_size: row.get(2)?,
                file_count: row.get(3)?,
                total_size: row.get(4)?,
                reviewed: row.get(5)?,
                updated_at: row.get(6)?,
                items: Vec::new(),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut groups = Vec::new();
    for g in groups_iter {
        if let Ok(mut group) = g {
            group.items = get_group_items_from(&conn, items_table, group.id)?;
            groups.push(group);
        }
    }

    Ok(groups)
}

fn get_group_items_from(
    conn: &Connection,
    items_table: &str,
    group_id: i64,
) -> Result<Vec<DedupGroupItem>, String> {
    let sql = format!(
        "SELECT group_id, file_id, is_keep, is_selected, score
         FROM {items_table}
         WHERE group_id = ?1
         ORDER BY is_keep DESC, score DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let iter = stmt
        .query_map(params![group_id], |row| {
            Ok(DedupGroupItem {
                group_id: row.get(0)?,
                file_id: row.get(1)?,
                is_keep: row.get(2)?,
                is_selected: row.get(3)?,
                score: row.get(4)?,
                file: None,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    for it in iter {
        if let Ok(mut item) = it {
            if let Ok(Some(file_info)) = AFile::get_file_info(item.file_id) {
                item.file = Some(file_info);
            }
            items.push(item);
        }
    }

    Ok(items)
}

pub fn set_keep(group_id: i64, file_id: i64) -> Result<(), String> {
    set_keep_in(
        group_id,
        file_id,
        "duplicate_groups",
        "duplicate_group_items",
    )
}

pub fn set_similar_keep(group_id: i64, file_id: i64) -> Result<(), String> {
    set_keep_in(
        group_id,
        file_id,
        "similar_duplicate_groups",
        "similar_duplicate_group_items",
    )
}

fn set_keep_in(
    group_id: i64,
    file_id: i64,
    groups_table: &str,
    items_table: &str,
) -> Result<(), String> {
    let mut conn = get_db_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        &format!("UPDATE {items_table} SET is_keep = 0 WHERE group_id = ?1"),
        params![group_id],
    )
    .map_err(|e| e.to_string())?;

    let changed = tx
        .execute(
            &format!(
                "UPDATE {items_table} SET is_keep = 1 WHERE group_id = ?1 AND file_id = ?2"
            ),
            params![group_id, file_id],
        )
        .map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("Item not found in group".into());
    }

    tx.execute(
        &format!("UPDATE {groups_table} SET reviewed = 1 WHERE id = ?1"),
        params![group_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_selected(
    group_ids: Option<Vec<i64>>,
    file_ids: Option<Vec<i64>>,
    mode: Option<String>,
) -> Result<DedupDeleteResult, String> {
    let similar = matches!(
        mode.as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref(),
        Some("similar")
    );
    let items_table = if similar {
        "similar_duplicate_group_items"
    } else {
        "duplicate_group_items"
    };
    let groups_table = if similar {
        "similar_duplicate_groups"
    } else {
        "duplicate_groups"
    };

    let mut conn = get_db_conn()?;
    if similar {
        let _ = crate::t_migration::ensure_similar_dedup_tables(&conn);
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let mut files_to_delete: Vec<(i64, String)> = Vec::new();

    // Prefer resolving path via AFile (correct separators / library roots) after collecting ids.
    let mut candidate_ids: Vec<i64> = Vec::new();

    if let Some(ids) = file_ids {
        let sql = format!(
            "SELECT dgi.file_id
             FROM {items_table} dgi
             WHERE dgi.file_id = ?1 AND dgi.is_keep = 0"
        );
        let mut stmt = tx.prepare(&sql).map_err(|e| e.to_string())?;
        for id in ids {
            let mut iter = stmt
                .query_map(params![id], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())?;
            for row in &mut iter {
                candidate_ids.push(row.map_err(|e| e.to_string())?);
            }
        }
    } else if let Some(gids) = group_ids {
        let sql = format!(
            "SELECT dgi.file_id
             FROM {items_table} dgi
             WHERE dgi.group_id = ?1 AND dgi.is_keep = 0 AND dgi.is_selected = 1"
        );
        for gid in gids {
            let mut stmt = tx.prepare(&sql).map_err(|e| e.to_string())?;
            let mut iter = stmt
                .query_map(params![gid], |row| row.get::<_, i64>(0))
                .map_err(|e| e.to_string())?;
            for row in &mut iter {
                candidate_ids.push(row.map_err(|e| e.to_string())?);
            }
        }
    } else {
        let sql = format!(
            "SELECT dgi.file_id
             FROM {items_table} dgi
             WHERE dgi.is_keep = 0 AND dgi.is_selected = 1"
        );
        let mut stmt = tx.prepare(&sql).map_err(|e| e.to_string())?;
        let mut iter = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        for row in &mut iter {
            candidate_ids.push(row.map_err(|e| e.to_string())?);
        }
    }

    // Dedup ids while preserving order.
    {
        let mut seen = std::collections::HashSet::new();
        candidate_ids.retain(|id| seen.insert(*id));
    }

    tx.commit().map_err(|e| e.to_string())?;

    for file_id in candidate_ids {
        match AFile::get_file_info(file_id) {
            Ok(Some(file)) => {
                if let Some(path) = file.file_path {
                    files_to_delete.push((file_id, path));
                } else {
                    // Still allow DB cleanup if path missing
                    files_to_delete.push((file_id, String::new()));
                }
            }
            Ok(None) => {}
            Err(e) => {
                // collect later as failure after loop setup
                files_to_delete.push((file_id, format!("__err__:{e}")));
            }
        }
    }

    let mut failures: Vec<String> = Vec::new();
    let mut deleted_file_ids: Vec<i64> = Vec::new();
    for (file_id, file_path) in files_to_delete {
        if file_path.starts_with("__err__:") {
            failures.push(format!(
                "Failed to resolve file id={}: {}",
                file_id,
                file_path.trim_start_matches("__err__:")
            ));
            continue;
        }
        if !file_path.is_empty() {
            if let Err(e) = t_utils::trash_path(&file_path) {
                failures.push(format!("Failed to move to trash: {} ({})", file_path, e));
                continue;
            }
        }
        match AFile::delete(file_id) {
            Ok(0) => failures.push(format!("File not removed from DB: id={}", file_id)),
            Ok(_) => deleted_file_ids.push(file_id),
            Err(e) => failures.push(format!("Failed to delete DB row for id={}: {}", file_id, e)),
        }
    }

    // Clean up empty groups + orphan membership for this mode's tables.
    match get_db_conn() {
        Ok(conn) => {
            // Drop items whose file row is already gone (FK may have cascade; be explicit).
            let _ = conn.execute(
                &format!(
                    "DELETE FROM {items_table}
                     WHERE file_id NOT IN (SELECT id FROM afiles)"
                ),
                [],
            );
            if let Err(e) = conn.execute(
                &format!(
                    "DELETE FROM {groups_table}
                     WHERE id NOT IN (SELECT DISTINCT group_id FROM {items_table})"
                ),
                [],
            ) {
                failures.push(format!(
                    "Failed to clean up empty {groups_table}: {}",
                    e
                ));
            }
            // Also drop groups that no longer have 2+ members.
            let _ = conn.execute(
                &format!(
                    "DELETE FROM {groups_table}
                     WHERE (SELECT COUNT(*) FROM {items_table} WHERE group_id = {groups_table}.id) < 2"
                ),
                [],
            );
            // Exact also used size-based reclaim; similar stores sum sizes — no further action.
            if similar {
                // Remove phash rows for deleted files (cascade should handle; belt-and-suspenders).
                let _ = conn.execute(
                    "DELETE FROM file_phashes WHERE file_id NOT IN (SELECT id FROM afiles)",
                    [],
                );
            } else {
                let _ = conn.execute(
                    "DELETE FROM file_hashes WHERE file_id NOT IN (SELECT id FROM afiles)",
                    [],
                );
            }
        }
        Err(e) => failures.push(format!(
            "Failed to open DB for duplicate group cleanup: {}",
            e
        )),
    }

    Ok(DedupDeleteResult {
        deleted_file_ids,
        failed_count: failures.len(),
        errors: failures,
    })
}

#[cfg(test)]
mod dhash_tests {
    use super::{compute_dhash_u64_from_gray, hamming64};

    #[test]
    fn identical_images_have_zero_hamming() {
        let mut img = image::GrayImage::new(9, 8);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Luma([((x + y) * 10) as u8]);
        }
        let a = compute_dhash_u64_from_gray(&img);
        let b = compute_dhash_u64_from_gray(&img);
        assert_eq!(hamming64(a, b), 0);
    }

    #[test]
    fn checkerboard_vs_flat_are_far() {
        let mut a_img = image::GrayImage::new(9, 8);
        let mut b_img = image::GrayImage::new(9, 8);
        for (x, y, p) in a_img.enumerate_pixels_mut() {
            *p = image::Luma([if (x + y) % 2 == 0 { 255 } else { 0 }]);
        }
        for (_x, _y, p) in b_img.enumerate_pixels_mut() {
            *p = image::Luma([128]);
        }
        let a = compute_dhash_u64_from_gray(&a_img);
        let b = compute_dhash_u64_from_gray(&b_img);
        // Flat image → all gradients 0; checkerboard → many 1 bits.
        assert!(hamming64(a, b) > super::DHASH_HAMMING_THRESHOLD, "dist={}", hamming64(a, b));
    }
}
