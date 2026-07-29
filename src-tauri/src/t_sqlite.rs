/**
 * SQLite database operations.
 * project: Lap
 * author:  julyx10
 * date:    2024-08-08
 */
use crate::t_ai;
use crate::t_common;
use crate::t_config;
use crate::t_image;
use crate::t_lens;
use crate::t_libraw;
use crate::t_storage;
use crate::t_utils;
use crate::t_video;
use base64::{Engine, engine::general_purpose};
use exif::{In, Tag, Value};
use image::{GenericImageView, ImageFormat};
use instant_distance::{Builder as HnswBuilder, HnswMap, Search as HnswSearch};
use ndarray::Array4;
use rayon::{ThreadPoolBuilder, prelude::*};
use rusqlite::{
    Connection, OptionalExtension, Result, ToSql, params, params_from_iter, types::ValueRef,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::mem::size_of;
use std::ops::{Deref, DerefMut};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, State};

static THUMB_GENERATION_LOCKS: OnceLock<ThumbGenerationLocks> = OnceLock::new();
static THUMB_BACKGROUND_TASKS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static EMBED_FILE_TRACE_ENABLED: OnceLock<bool> = OnceLock::new();

fn embed_file_trace_enabled() -> bool {
    *EMBED_FILE_TRACE_ENABLED.get_or_init(|| {
        std::env::var("PICAIPIC_EMBED_FILE_TRACE")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(false)
    })
}

struct ThumbGenerationLocks {
    active: Mutex<HashSet<String>>,
    available: Condvar,
}

fn thumb_generation_locks() -> &'static ThumbGenerationLocks {
    THUMB_GENERATION_LOCKS.get_or_init(|| ThumbGenerationLocks {
        active: Mutex::new(HashSet::new()),
        available: Condvar::new(),
    })
}

fn thumb_background_tasks() -> &'static Mutex<HashSet<String>> {
    THUMB_BACKGROUND_TASKS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn has_active_thumb_background_tasks() -> bool {
    thumb_background_tasks()
        .lock()
        .map(|tasks| !tasks.is_empty())
        .unwrap_or(false)
}

struct ThumbGenerationGuard {
    key: String,
}

impl Drop for ThumbGenerationGuard {
    fn drop(&mut self) {
        let locks = thumb_generation_locks();
        let mut active = locks.active.lock().unwrap_or_else(|e| e.into_inner());
        active.remove(&self.key);
        locks.available.notify_all();
    }
}

/// Face Bounding Box struct (matching JSON storage)
#[derive(Debug, Deserialize)]
struct FaceBBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// Define the Album struct
#[derive(Debug, Serialize, Deserialize)]
pub struct Album {
    pub id: Option<i64>, // unique id (autoincrement by db)

    // album basic info
    pub name: String,             // album name (default is folder name)
    pub path: String,             // folder path
    pub created_at: Option<i64>,  // folder create time
    pub modified_at: Option<i64>, // folder modified time

    // extra info
    pub display_order_id: Option<i64>, // display order id
    pub cover_file_id: Option<i64>,    // album cover file id
    pub description: Option<String>,   // album description
    pub indexed: Option<u64>,          // indexed files count
    pub total: Option<u64>,            // total files count
    pub last_scan_time: Option<i64>,   // last scan time
}

impl Album {
    /// create a new album
    fn new(path: &str) -> Result<Self, String> {
        let file_info = t_utils::FileInfo::new(path)?;
        Ok(Self {
            id: None,
            name: file_info.file_name,
            path: file_info.file_path,
            created_at: file_info.created,
            modified_at: file_info.modified,
            display_order_id: None,
            cover_file_id: None,
            description: Some(String::new()),
            indexed: Some(0),
            total: Some(0),
            last_scan_time: Some(0),
        })
    }

    /// Function to construct `Self` from a database row
    fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            path: row.get(2)?,
            created_at: row.get(3)?,
            modified_at: row.get(4)?,
            display_order_id: row.get(5)?,
            cover_file_id: row.get(6)?,
            description: row.get(7)?,
            indexed: row.get(8)?,
            total: row.get(9)?,
            last_scan_time: row.get(10)?,
        })
    }

    /// fetch an album from db by path
    fn fetch(path: &str) -> Result<Option<Self>, String> {
        let conn = open_conn()?;
        let result = conn.query_row(
            "SELECT id, name, path, created_at, modified_at, display_order_id, cover_file_id, description, indexed, total, last_scan_time
            FROM albums WHERE path = ?1",
            params![path],
            Self::from_row
        ).optional().map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// insert an album into db
    fn insert(&mut self) -> Result<usize, String> {
        let conn = open_conn()?;

        // Determine the next display order id
        self.display_order_id = conn
            .query_row(
                "SELECT COALESCE(MAX(display_order_id), 0) + 1 FROM albums",
                params![],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        // Insert the new album into the db
        let result = conn.execute(
            "INSERT INTO albums (name, path, created_at, modified_at, display_order_id, cover_file_id, description, indexed, total, last_scan_time) 
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                self.name,
                self.path,
                self.created_at,
                self.modified_at,
                self.display_order_id,
                self.cover_file_id,
                self.description,
                self.indexed,
                self.total,
                self.last_scan_time,
            ],
        ).map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// add the album into db if not exists
    pub fn add_album_to_db(path: &str) -> Result<Self, String> {
        // Check if the path already exists
        let existing_album = Self::fetch(path);
        if let Ok(Some(album)) = existing_album {
            return Err(format!(
                "Album '{}' with the path '{}' already exists.",
                album.name, album.path
            ));
        }

        // Insert the new album into the database
        Self::new(path)?.insert()?;

        // return the newly inserted album
        let new_album = Self::fetch(path)?;
        Ok(new_album.unwrap())
    }

    /// delete an album from the db
    pub fn delete_from_db(id: i64) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute("DELETE FROM albums WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Get all albums(album_type = 1) from the db
    pub fn get_all_albums() -> Result<Vec<Self>, String> {
        let conn = open_conn()?;

        let query =
            "SELECT id, name, path, created_at, modified_at, display_order_id, cover_file_id, description, indexed, total, last_scan_time
            FROM albums
            ORDER BY display_order_id ASC";

        let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;

        // Execute the query and map the result to Album structs
        let albums_iter = stmt
            .query_map([], Self::from_row)
            .map_err(|e| e.to_string())?;

        // Collect the results into a Vec<Album>
        let mut albums = Vec::new();
        for album in albums_iter {
            match album {
                Ok(album) => albums.push(album),
                Err(e) => return Err(format!("Failed to retrieve row: {}", e)),
            }
        }
        Ok(albums)
    }

    /// get album info by id
    pub fn get_album_by_id(id: i64) -> Result<Self, String> {
        let conn = open_conn()?;
        let result = conn.query_row(
            "SELECT id, name, path, created_at, modified_at, display_order_id, cover_file_id, description, indexed, total, last_scan_time
            FROM albums WHERE id = ?1",
            params![id],
            Self::from_row
        ).map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// update a column value (allow-listed column names only)
    pub fn update_column(
        id: i64,
        column: &str,
        value: &dyn rusqlite::ToSql,
    ) -> Result<usize, String> {
        assert_allowed_column(
            column,
            &[
                "name",
                "description",
                "path",
                "display_order_id",
                "cover_file_id",
                "last_scan_time",
            ],
        )?;
        let conn = open_conn()?;
        let query = format!("UPDATE albums SET {} = ?1 WHERE id = ?2", column);
        let result = conn
            .execute(&query, params![value, id])
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// update last scan time
    pub fn update_last_scan_time(album_id: i64, scan_time: i64) -> Result<usize, String> {
        Self::update_column(album_id, "last_scan_time", &scan_time)
    }

    /// rename the album root metadata and matching folders in one transaction
    pub fn rename_root_folder(old_path: &str, new_path: &str) -> Result<(), String> {
        let new_name = t_utils::get_file_name(new_path);
        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE albums SET path = ?2 WHERE path = ?1",
            params![old_path, new_path],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE afolders
            SET path = CONCAT(?2, SUBSTRING(path, LENGTH(?1) + 1)), name = ?3
            WHERE path LIKE ?1 || '%'",
            params![old_path, new_path, new_name],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// update indexed and total progress
    pub fn update_progress(id: i64, indexed: u64, total: u64) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute(
                "UPDATE albums SET indexed = ?1, total = ?2 WHERE id = ?3",
                params![indexed, total, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// set album cover to the first file (image/video) if not set
    pub fn auto_set_cover(id: i64) -> Result<(), String> {
        let conn = open_conn()?;

        // 1. check if cover_file_id is set
        let cover_file_id: Option<i64> = conn
            .query_row(
                "SELECT cover_file_id FROM albums WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if cover_file_id.unwrap_or(0) > 0 {
            return Ok(());
        }

        // 2. get the first formatted file (image or video)
        let file_id: Option<i64> = conn
            .query_row(
                "SELECT a.id 
                FROM afiles a
                JOIN afolders b ON a.folder_id = b.id
                JOIN athumbs c ON a.id = c.file_id
                WHERE b.album_id = ?1 AND (a.file_type = 1 OR a.file_type = 2)
                ORDER BY a.taken_date ASC
                LIMIT 1",
                params![id],
                |row| row.get(0),
            )
            .optional() // returns Option<i64>
            .map_err(|e| e.to_string())?;

        // 3. update cover_file_id
        if let Some(fid) = file_id {
            let _ = conn
                .execute(
                    "UPDATE albums SET cover_file_id = ?1 WHERE id = ?2",
                    params![fid, id],
                )
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Recount files for an album from the database and update stored progress.
    /// A completed album stays completed after moving, copying, or deleting
    /// already-indexed files. Partial scan progress is preserved and clamped.
    pub fn recount_album(id: i64) -> Result<Self, String> {
        let conn = open_conn()?;
        let total: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM afiles a JOIN afolders b ON a.folder_id = b.id
                    WHERE b.album_id = ?1 AND {}",
                    AFile::search_exclusion_condition("b")
                ),
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let (indexed, previous_total): (i64, i64) = conn
            .query_row(
                "SELECT COALESCE(indexed, 0), COALESCE(total, 0)
                 FROM albums WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let next_indexed = if indexed >= previous_total {
            total
        } else {
            indexed.min(total).max(0)
        };
        conn.execute(
            "UPDATE albums SET total = ?1, indexed = ?2 WHERE id = ?3",
            params![total, next_indexed, id],
        )
        .map_err(|e| e.to_string())?;
        let result = Self::get_album_by_id(id)?;
        Ok(result)
    }
}

/// Define the album's folder struct
#[derive(Debug, Serialize, Deserialize)]
pub struct AFolder {
    pub id: Option<i64>, // unique id (autoincrement by db)
    pub album_id: i64,   // album id (from albums table)

    // folder basic info
    pub name: String,             // folder name
    pub path: String,             // folder path
    pub created_at: Option<i64>,  // folder create time
    pub modified_at: Option<i64>, // folder modified time

    // extra info
    pub is_favorite: Option<bool>,             // is favorite
    pub is_excluded_from_search: Option<bool>, // exclude folder and children from search
    pub file_count: Option<i64>,               // file count (populated by get_favorite_folders)
}

impl AFolder {
    /// create a new folder struct
    fn new(album_id: i64, folder_path: &str) -> Result<Self, String> {
        let file_info = t_utils::FileInfo::new(folder_path)?;
        Ok(Self {
            id: None,
            album_id,
            name: file_info.file_name,
            path: folder_path.to_string(),
            created_at: file_info.created,
            modified_at: Some(0), // force first sync
            is_favorite: None,
            is_excluded_from_search: Some(false),
            file_count: None,
        })
    }

    /// Function to construct `Self` from a database row
    fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: Some(row.get(0)?),
            album_id: row.get(1)?,
            name: row.get(2)?,
            path: row.get(3)?,
            created_at: row.get(4)?,
            modified_at: row.get(5)?,
            is_favorite: row.get(6)?,
            is_excluded_from_search: row.get(7)?,
            file_count: None,
        })
    }

    /// fetch a folder row from db (by path)
    pub fn fetch(folder_path: &str) -> Result<Option<Self>, String> {
        let conn = open_conn()?;
        Self::fetch_with_conn(&conn, folder_path)
    }

    pub fn fetch_with_conn(conn: &Connection, folder_path: &str) -> Result<Option<Self>, String> {
        conn.query_row(
            "SELECT id, album_id, name, path, created_at, modified_at, is_favorite, COALESCE(is_excluded_from_search, 0)
            FROM afolders
            WHERE path = ?1",
            params![folder_path],
            Self::from_row,
        )
        .optional()
        .map_err(|e| e.to_string())
    }

    /// fetch a folder row from db (by id)
    pub fn get_by_id(id: i64) -> Result<Option<Self>, String> {
        let conn = open_conn()?;
        let result = conn
            .query_row(
                "SELECT id, album_id, name, path, created_at, modified_at, is_favorite, COALESCE(is_excluded_from_search, 0)
                FROM afolders
                WHERE id = ?1",
                params![id],
                Self::from_row,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// fetch all folder rows in the current library database
    pub fn get_all() -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, album_id, name, path, created_at, modified_at, is_favorite, COALESCE(is_excluded_from_search, 0)
                FROM afolders",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![], Self::from_row)
            .map_err(|e| e.to_string())?;

        let mut folders = Vec::new();
        for folder in rows {
            folders.push(folder.map_err(|e| e.to_string())?);
        }
        Ok(folders)
    }

    fn insert_with_conn(&self, conn: &Connection) -> Result<usize, String> {
        conn.execute(
            "INSERT INTO afolders (album_id, name, path, created_at, modified_at, is_favorite, is_excluded_from_search)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.album_id, self.name, self.path,
                self.created_at, self.modified_at,
                self.is_favorite, self.is_excluded_from_search
            ],
        )
        .map_err(|e| e.to_string())
    }

    /// insert the folder to db if not exists
    pub fn add_to_db(album_id: i64, folder_path: &str) -> Result<Self, String> {
        let conn = open_conn()?;
        Self::add_to_db_with_conn(&conn, album_id, folder_path)
    }

    pub fn add_to_db_with_conn(
        conn: &Connection,
        album_id: i64,
        folder_path: &str,
    ) -> Result<Self, String> {
        if let Ok(Some(folder)) = Self::fetch_with_conn(conn, folder_path) {
            return Ok(folder);
        }
        Self::new(album_id, folder_path)?.insert_with_conn(conn)?;
        let new_folder = Self::fetch_with_conn(conn, folder_path)?;
        Ok(new_folder.unwrap())
    }

    /// move a folder (update path and album_id)
    pub fn move_folder(old_path: &str, new_album_id: i64, new_path: &str) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute(
                "UPDATE afolders
                SET path = CONCAT(?3, SUBSTRING(path, LENGTH(?1) + 1)), album_id = ?2
                WHERE path = ?1 OR path LIKE ?1 || ?4",
                params![
                    old_path,
                    new_album_id,
                    new_path,
                    format!("{}%", std::path::MAIN_SEPARATOR)
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Replace an existing destination folder subtree and move the source
    /// subtree in one transaction.
    pub fn replace_moved_folder(
        old_path: &str,
        new_album_id: i64,
        new_path: &str,
    ) -> Result<usize, String> {
        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let destination_pattern = format!("{}{}%", new_path, std::path::MAIN_SEPARATOR);

        let destination_folder_ids: Vec<i64> = {
            let mut stmt = tx
                .prepare("SELECT id FROM afolders WHERE path = ?1 OR path LIKE ?2")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![new_path, destination_pattern], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            rows.filter_map(|row| row.ok()).collect()
        };

        for folder_id in destination_folder_ids {
            tx.execute(
                "DELETE FROM afiles WHERE folder_id = ?1",
                params![folder_id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute(
            "DELETE FROM afolders WHERE path = ?1 OR path LIKE ?2",
            params![new_path, destination_pattern],
        )
        .map_err(|e| e.to_string())?;

        let result = tx
            .execute(
                "UPDATE afolders
                SET path = CONCAT(?3, SUBSTRING(path, LENGTH(?1) + 1)), album_id = ?2
                WHERE path = ?1 OR path LIKE ?1 || ?4",
                params![
                    old_path,
                    new_album_id,
                    new_path,
                    format!("{}%", std::path::MAIN_SEPARATOR)
                ],
            )
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }

    pub fn replace_copied_folder(album_id: i64, folder_path: &str) -> Result<Self, String> {
        let folder = Self::new(album_id, folder_path)?;
        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let destination_pattern = format!("{}{}%", folder_path, std::path::MAIN_SEPARATOR);

        let destination_folder_ids: Vec<i64> = {
            let mut stmt = tx
                .prepare("SELECT id FROM afolders WHERE path = ?1 OR path LIKE ?2")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![folder_path, destination_pattern], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            rows.filter_map(|row| row.ok()).collect()
        };
        for folder_id in destination_folder_ids {
            tx.execute(
                "DELETE FROM afiles WHERE folder_id = ?1",
                params![folder_id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute(
            "DELETE FROM afolders WHERE path = ?1 OR path LIKE ?2",
            params![folder_path, destination_pattern],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO afolders
            (album_id, name, path, created_at, modified_at, is_favorite, is_excluded_from_search)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                folder.album_id,
                folder.name,
                folder.path,
                folder.created_at,
                folder.modified_at,
                folder.is_favorite,
                folder.is_excluded_from_search
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Self::fetch(folder_path)?
            .ok_or_else(|| format!("Copied folder missing from DB: {}", folder_path))
    }

    /// delete a folder and all its child folders and files from db
    pub fn delete_folder(folder_path: &str) -> Result<usize, String> {
        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        // First, get all folder IDs that will be deleted (the folder itself and all children)
        let folder_ids: Vec<i64> = {
            let mut stmt = tx
                .prepare("SELECT id FROM afolders WHERE path = ?1 OR path LIKE ?2")
                .map_err(|e| e.to_string())?;

            let path_pattern = format!("{}{}%", folder_path, std::path::MAIN_SEPARATOR);
            let rows = stmt
                .query_map(params![folder_path, path_pattern], |row| row.get(0))
                .map_err(|e| e.to_string())?;

            rows.filter_map(|r| r.ok()).collect()
        };

        // Delete all files in those folders
        for folder_id in &folder_ids {
            tx.execute(
                "DELETE FROM afiles WHERE folder_id = ?1",
                params![folder_id],
            )
            .map_err(|e| e.to_string())?;
        }

        // Delete the folders (the folder and all its children)
        let path_pattern = format!("{}{}%", folder_path, std::path::MAIN_SEPARATOR);
        let result = tx
            .execute(
                "DELETE FROM afolders WHERE path = ?1 OR path LIKE ?2",
                params![folder_path, path_pattern],
            )
            .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }

    // update a column value (allow-listed column names only)
    pub fn update_column(
        id: i64,
        column: &str,
        value: &dyn rusqlite::ToSql,
    ) -> Result<usize, String> {
        assert_allowed_column(
            column,
            &["is_favorite", "is_excluded_from_search", "modified_at"],
        )?;
        let conn = open_conn()?;
        let query = format!("UPDATE afolders SET {} = ?1 WHERE id = ?2", column);
        let result = conn
            .execute(&query, params![value, id])
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    // get a folder's is_favorite status
    pub fn get_is_favorite(folder_path: &str) -> Result<Option<bool>, String> {
        let conn = open_conn()?;
        let result = conn
            .query_row(
                "SELECT is_favorite FROM afolders WHERE path = ?1",
                params![folder_path],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    // get a folder's is_excluded_from_search status
    pub fn get_is_excluded_from_search(folder_path: &str) -> Result<Option<bool>, String> {
        let conn = open_conn()?;
        let result = conn
            .query_row(
                "SELECT COALESCE(is_excluded_from_search, 0) FROM afolders WHERE path = ?1",
                params![folder_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    // get all favorite folders
    pub fn get_favorite_folders() -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let sep = std::path::MAIN_SEPARATOR.to_string().replace('\'', "''");

        let query = format!(
            "SELECT a.id, a.album_id, a.name, a.path, a.created_at, a.modified_at, a.is_favorite,
                EXISTS (
                    SELECT 1 FROM afolders xf
                    WHERE COALESCE(xf.is_excluded_from_search, 0) = 1
                    AND xf.album_id = a.album_id
                    AND (
                        a.path = xf.path
                        OR instr(a.path, xf.path || '{}') = 1
                    )
                ),
                (SELECT COUNT(*) FROM afiles WHERE folder_id = a.id)
            FROM afolders a
            WHERE a.is_favorite = 1
            ORDER BY a.name",
            sep
        );

        let mut stmt = conn.prepare(query.as_str()).map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                Ok(Self {
                    id: Some(row.get(0)?),
                    album_id: row.get(1)?,
                    name: row.get(2)?,
                    path: row.get(3)?,
                    created_at: row.get(4)?,
                    modified_at: row.get(5)?,
                    is_favorite: row.get(6)?,
                    is_excluded_from_search: row.get(7)?,
                    file_count: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut folders = Vec::new();
        for folder in rows {
            folders.push(folder.unwrap());
        }

        Ok(folders)
    }
}

/// Define the album file struct
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AFile {
    pub id: Option<i64>, // unique id (autoincrement by db)
    pub folder_id: i64,  // folder id (from folders table)

    // file basic info
    pub name: String,                 // file name
    pub name_pinyin: Option<String>,  // file name pinyin(for sort)
    pub size: i64,                    // file size
    pub file_type: Option<i64>,       // file type (0: all, 1: image, 2: video, 3: audio, 4: other)
    pub format_label: Option<String>, // normalized file format label (from file content)
    pub created_at: Option<i64>,      // file create timestamp
    pub modified_at: Option<i64>,     // file modified timestamp
    pub inode: Option<i64>,           // filesystem inode (for rename detection)
    pub taken_date: Option<i64>,      // taken date timestamp (e_date_time || modified_at)

    // image/video
    pub width: Option<u32>,    // image/video width
    pub height: Option<u32>,   // image/video height
    pub duration: Option<i64>, // video duration

    // extra info
    pub is_favorite: Option<bool>, // is favorite
    pub rating: Option<i32>,       // 0-5 stars
    pub rotate: Option<i32>,       // rotate angle (0, 90, 180, 270)
    pub comments: Option<String>,  // comments
    pub has_tags: Option<bool>,    // has tags
    pub has_faces: Option<i32>,    // has faces (0: unprocessed, 1: has faces, 2: no faces)

    // exif info
    pub e_make: Option<String>,  // camera make
    pub e_model: Option<String>, // camera model
    pub e_date_time: Option<String>,
    pub e_software: Option<String>,
    pub e_artist: Option<String>,
    pub e_copyright: Option<String>,
    pub e_description: Option<String>,
    pub e_lens_make: Option<String>,
    pub e_lens_model: Option<String>,
    pub e_exposure_bias: Option<String>,
    pub e_exposure_time: Option<String>,
    pub e_f_number: Option<String>,
    pub e_focal_length: Option<String>,
    pub e_iso_speed: Option<String>,
    pub e_flash: Option<String>,    // flash
    pub e_orientation: Option<u32>, // orientation

    // gps info
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub gps_altitude: Option<f64>,

    // geo info (from http://www.geonames.org/)
    pub geo_name: Option<String>,   // Location name
    pub geo_admin1: Option<String>, // Administrative district 1
    pub geo_admin2: Option<String>, // Administrative district 2
    pub geo_cc: Option<String>,     // Country code

    // Live Photo / Motion Photo pairing
    pub content_id: Option<String>, // Apple ContentIdentifier UUID or Motion Photo XMP marker
    pub paired_file_id: Option<i64>, // paired video/image file id
    pub live_photo_type: Option<i64>, // 0=none, 1=Apple image, 2=Apple video, 3=Motion Photo, 4=HEIC-internal video

    // output only
    pub file_path: Option<String>,   // file path (for webview)
    pub album_id: Option<i64>,       // album id (for webview)
    pub album_name: Option<String>,  // album name (for webview)
    pub has_thumbnail: Option<bool>, // has thumbnail (for webview)
    pub has_embedding: Option<bool>, // has embedding (for webview)
    pub last_scan_time: Option<i64>, // last scan timestamp
}

/// Define the timeline marker struct for scrollbar markers
#[derive(Debug, Serialize, Deserialize)]
pub struct ATimeLine {
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub date: Option<i32>,
    pub position: i64, // Row index in the sorted fileList
}

/// Define the query parameters struct for file queries
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    pub search_file_name: String, // file name search
    pub search_file_type: i64,
    pub sort_type: i64,
    pub sort_order: i64,
    pub search_all_subfolders: String,
    pub search_folder: String,
    pub start_date: i64,
    pub end_date: i64,
    pub calendar_sort: i64, // 0=taken asc … 5=modified desc (sort / 2 → column)
    pub make: String,
    pub model: String,
    pub lens_make: String,
    pub lens_model: String,
    pub location_admin1: String,
    pub location_name: String,
    pub is_favorite: bool,
    pub rating: i64,
    pub tag_id: i64,
    pub person_id: i64,
    // GPS bounding box filter (e.g. for "photos in this map area")
    #[serde(default)]
    pub gps_min_lat: Option<f64>,
    #[serde(default)]
    pub gps_max_lat: Option<f64>,
    #[serde(default)]
    pub gps_min_lon: Option<f64>,
    #[serde(default)]
    pub gps_max_lon: Option<f64>,
}

/// Define the AI image search parameters struct
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchParams {
    pub search_text: String,  // search image text (for AI search)
    pub file_id: Option<i64>, // file id (for similar image search)
    pub threshold: f32,       // search threshold
    pub limit: i64,           // search limit
    /// Bitmask matching QueryParams.search_file_type (0 all, 1 image, 2 video, 4 raw).
    #[serde(default)]
    pub search_file_type: i64,
}

struct ExifIdentity {
    taken_date: Option<i64>,
    make: Option<String>,
    model: Option<String>,
    date_time: Option<String>,
    software: Option<String>,
}

struct ExifDescription {
    artist: Option<String>,
    copyright: Option<String>,
    description: Option<String>,
    user_comment: Option<String>,
}

struct ExifCapture {
    lens_make: Option<String>,
    lens_model: Option<String>,
    exposure_bias: Option<String>,
    exposure_time: Option<String>,
    f_number: Option<String>,
    focal_length: Option<String>,
    iso_speed: Option<String>,
}

#[derive(Clone, Default)]
struct BinaryExifFallback {
    make: Option<String>,
    model: Option<String>,
    date_time_original: Option<String>,
    date_time: Option<String>,
    software: Option<String>,
    lens_make: Option<String>,
    lens_model: Option<String>,
    content_id: Option<String>,
    sony_orientation: Option<u16>,
}

struct RawMetadataTarget<'a> {
    make: &'a mut Option<String>,
    model: &'a mut Option<String>,
    software: &'a mut Option<String>,
    artist: &'a mut Option<String>,
    description: &'a mut Option<String>,
    iso_speed: &'a mut Option<String>,
    exposure_time: &'a mut Option<String>,
    f_number: &'a mut Option<String>,
    focal_length: &'a mut Option<String>,
    flash: &'a mut Option<String>,
    lens_make: &'a mut Option<String>,
    lens_model: &'a mut Option<String>,
    taken_date: &'a mut Option<i64>,
    modified_at: Option<i64>,
}

impl RawMetadataTarget<'_> {
    fn apply(self, meta: t_libraw::RawMeta) {
        if self.make.is_none() {
            *self.make = meta.make;
        }
        if self.model.is_none() {
            *self.model = meta.model;
        }
        if self.software.is_none() {
            *self.software = meta.software;
        }
        if self.artist.is_none() {
            *self.artist = meta.artist;
        }
        if self.description.is_none() {
            *self.description = meta.description;
        }
        if self.iso_speed.is_none() {
            *self.iso_speed = meta.iso_speed;
        }
        if self.exposure_time.is_none() {
            *self.exposure_time = meta.shutter;
        }
        if self.f_number.is_none() {
            *self.f_number = meta.aperture;
        }
        if self.focal_length.is_none() {
            *self.focal_length = meta.focal_len;
        }
        if self.flash.is_none() {
            *self.flash = meta.flash_used;
        }
        if self.lens_make.is_none() {
            *self.lens_make = meta.lens_make;
        }
        if self.lens_model.is_none() {
            *self.lens_model = meta.lens_model;
        }
        if *self.taken_date == self.modified_at {
            if let Some(timestamp) = meta.timestamp {
                *self.taken_date = Some(timestamp);
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct EmbeddingBatchOutcome {
    pub results: Vec<(i64, Result<(), String>)>,
    pub prepared_items: usize,
    pub prepare_elapsed: Duration,
    pub engine_elapsed: Duration,
    pub preprocess_elapsed: Duration,
    pub inference_elapsed: Duration,
    pub write_elapsed: Duration,
}

pub(crate) struct EmbeddingBatchSource {
    pub file_id: i64,
    pub file_path: String,
    pub file_type: i64,
    pub orientation: i32,
}

#[derive(Default)]
pub(crate) struct PreparedEmbeddingBatch {
    ids: Vec<i64>,
    image_input: Option<Array4<f32>>,
    preprocess_error: Option<String>,
    prepare_elapsed: Duration,
    preprocess_elapsed: Duration,
}

#[derive(Default)]
pub(crate) struct AFileAddProfile {
    enabled: bool,
    pub fetch_elapsed: Duration,
    pub stat_elapsed: Duration,
    pub metadata_elapsed: Duration,
    pub metadata_file_info_elapsed: Duration,
    pub metadata_header_elapsed: Duration,
    pub metadata_dimensions_elapsed: Duration,
    pub metadata_exif_elapsed: Duration,
    pub metadata_exif_header_attempts: u64,
    pub metadata_exif_header_elapsed: Duration,
    pub metadata_exif_container_attempts: u64,
    pub metadata_exif_container_elapsed: Duration,
    pub metadata_exif_signature_scan_elapsed: Duration,
    pub metadata_exif_raw_attempts: u64,
    pub metadata_exif_raw_elapsed: Duration,
    pub metadata_exif_file_fallback_attempts: u64,
    pub metadata_exif_file_fallback_elapsed: Duration,
    pub metadata_exif_extract_elapsed: Duration,
    pub metadata_exif_extract_basic_elapsed: Duration,
    pub metadata_exif_extract_orientation_elapsed: Duration,
    pub metadata_exif_extract_flash_elapsed: Duration,
    pub metadata_exif_extract_gps_elapsed: Duration,
    pub metadata_exif_extract_identity_elapsed: Duration,
    pub metadata_exif_extract_description_elapsed: Duration,
    pub metadata_exif_extract_capture_elapsed: Duration,
    pub metadata_capture_fallback_elapsed: Duration,
    pub metadata_raw_elapsed: Duration,
    pub metadata_binary_fallback_elapsed: Duration,
    pub metadata_binary_tiff_signature_attempts: u64,
    pub metadata_binary_tiff_signature_elapsed: Duration,
    pub metadata_binary_tiff_bases_found: u64,
    pub metadata_binary_complete_jpeg_without_exif_attempts: u64,
    pub metadata_binary_complete_jpeg_without_exif_tiff_bases_found: u64,
    pub metadata_binary_entry_scan_attempts: u64,
    pub metadata_binary_entry_scan_elapsed: Duration,
    pub metadata_binary_value_decode_elapsed: Duration,
    pub metadata_motion_elapsed: Duration,
    pub metadata_motion_header_xmp_attempts: u64,
    pub metadata_motion_header_xmp_elapsed: Duration,
    pub metadata_motion_header_complete_check_attempts: u64,
    pub metadata_motion_header_complete_check_elapsed: Duration,
    pub metadata_motion_file_fallback_attempts: u64,
    pub metadata_motion_file_fallback_elapsed: Duration,
    pub metadata_motion_parse_attempts: u64,
    pub metadata_motion_parse_elapsed: Duration,
    pub metadata_heic_elapsed: Duration,
    pub metadata_geocode_elapsed: Duration,
    pub metadata_prompt_elapsed: Duration,
    pub metadata_assemble_elapsed: Duration,
    pub refresh_elapsed: Duration,
    pub write_elapsed: Duration,
    pub refetch_elapsed: Duration,
    pub deferred_seen_file_id: Option<i64>,
}

/// Per-scan subset of an indexed file. Warm rescans only need these fields to
/// decide whether a row needs work; keeping full `AFile` rows here would retain
/// unrelated text metadata for every file in a large album.
#[derive(Clone, Debug)]
pub(crate) struct ScanFileState {
    id: i64,
    modified_at: Option<i64>,
    has_thumbnail: bool,
    has_embedding: bool,
    orientation: i32,
    size: i64,
    width: u32,
    height: u32,
    duration: Option<u64>,
}

pub(crate) type ScanFileStateCache = HashMap<(i64, String), ScanFileState>;

pub(crate) struct ScanFileIndexResult {
    pub file_id: i64,
    pub has_thumbnail: bool,
    pub has_embedding: bool,
    pub orientation: i32,
    pub size: i64,
    pub width: u32,
    pub height: u32,
    pub duration: Option<u64>,
    pub deferred_seen_file_id: Option<i64>,
    pub cache_hit: bool,
}

impl AFileAddProfile {
    pub(crate) fn scan_enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    fn started(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    fn elapsed(started: Option<Instant>) -> Duration {
        started.map(|started| started.elapsed()).unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum AFileMetadataPhase {
    FileInfo,
    Header,
    Dimensions,
    ExifHeader,
    ExifFileFallback,
    ExifExtractOrientation,
    ExifExtractFlash,
    ExifExtractGps,
    ExifExtractIdentity,
    ExifExtractDescription,
    ExifExtractCapture,
    CaptureFallback,
    Raw,
    BinaryFallback,
    Motion,
    Heic,
    Geocode,
    Prompt,
    Assemble,
}

/// Records cold-import metadata stages only when scan profiling is enabled.
struct AFileMetadataProfiler<'a> {
    profile: Option<&'a mut AFileAddProfile>,
}

/// Optional internal timing for the one-pass binary EXIF fallback.
#[derive(Default)]
struct BinaryExifFallbackProfile {
    tiff_signature_attempts: u64,
    tiff_signature_elapsed: Duration,
    tiff_bases_found: u64,
    complete_jpeg_without_exif_attempts: u64,
    complete_jpeg_without_exif_tiff_bases_found: u64,
    entry_scan_attempts: u64,
    entry_scan_elapsed: Duration,
    value_decode_elapsed: Duration,
}

impl<'a> AFileMetadataProfiler<'a> {
    fn new(profile: Option<&'a mut AFileAddProfile>) -> Self {
        Self { profile }
    }

    fn enabled(&self) -> bool {
        self.profile.as_ref().is_some_and(|profile| profile.enabled)
    }

    fn measure<T>(&mut self, phase: AFileMetadataPhase, work: impl FnOnce() -> T) -> T {
        let started = self
            .profile
            .as_ref()
            .and_then(|profile| profile.enabled.then(Instant::now));
        let result = work();
        if let (Some(profile), Some(started)) = (self.profile.as_deref_mut(), started) {
            let elapsed = started.elapsed();
            match phase {
                AFileMetadataPhase::FileInfo => profile.metadata_file_info_elapsed += elapsed,
                AFileMetadataPhase::Header => profile.metadata_header_elapsed += elapsed,
                AFileMetadataPhase::Dimensions => profile.metadata_dimensions_elapsed += elapsed,
                AFileMetadataPhase::ExifHeader => {
                    profile.metadata_exif_header_attempts += 1;
                    profile.metadata_exif_header_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifFileFallback => {
                    profile.metadata_exif_file_fallback_attempts += 1;
                    profile.metadata_exif_file_fallback_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifExtractOrientation => {
                    profile.metadata_exif_extract_elapsed += elapsed;
                    profile.metadata_exif_extract_basic_elapsed += elapsed;
                    profile.metadata_exif_extract_orientation_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifExtractFlash => {
                    profile.metadata_exif_extract_elapsed += elapsed;
                    profile.metadata_exif_extract_basic_elapsed += elapsed;
                    profile.metadata_exif_extract_flash_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifExtractGps => {
                    profile.metadata_exif_extract_elapsed += elapsed;
                    profile.metadata_exif_extract_basic_elapsed += elapsed;
                    profile.metadata_exif_extract_gps_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifExtractIdentity => {
                    profile.metadata_exif_extract_elapsed += elapsed;
                    profile.metadata_exif_extract_identity_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifExtractDescription => {
                    profile.metadata_exif_extract_elapsed += elapsed;
                    profile.metadata_exif_extract_description_elapsed += elapsed;
                }
                AFileMetadataPhase::ExifExtractCapture => {
                    profile.metadata_exif_extract_elapsed += elapsed;
                    profile.metadata_exif_extract_capture_elapsed += elapsed;
                }
                AFileMetadataPhase::CaptureFallback => {
                    profile.metadata_capture_fallback_elapsed += elapsed
                }
                AFileMetadataPhase::Raw => profile.metadata_raw_elapsed += elapsed,
                AFileMetadataPhase::BinaryFallback => {
                    profile.metadata_binary_fallback_elapsed += elapsed
                }
                AFileMetadataPhase::Motion => profile.metadata_motion_elapsed += elapsed,
                AFileMetadataPhase::Heic => profile.metadata_heic_elapsed += elapsed,
                AFileMetadataPhase::Geocode => profile.metadata_geocode_elapsed += elapsed,
                AFileMetadataPhase::Prompt => profile.metadata_prompt_elapsed += elapsed,
                AFileMetadataPhase::Assemble => profile.metadata_assemble_elapsed += elapsed,
            }
        }
        result
    }

    fn measure_exif<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> T {
        let started = self
            .profile
            .as_ref()
            .and_then(|profile| profile.enabled.then(Instant::now));
        let result = work(self);
        if let (Some(profile), Some(started)) = (self.profile.as_deref_mut(), started) {
            profile.metadata_exif_elapsed += started.elapsed();
        }
        result
    }
}

impl AFile {
    /// Exclude files whose folder path is the excluded folder itself or one of its children.
    /// The caller must pass the alias for the file's joined afolders row.
    fn search_exclusion_condition(folder_alias: &str) -> String {
        let sep = std::path::MAIN_SEPARATOR.to_string().replace('\'', "''");
        format!(
            "NOT EXISTS (
                SELECT 1 FROM afolders xf
                WHERE COALESCE(xf.is_excluded_from_search, 0) = 1
                AND xf.album_id = {folder_alias}.album_id
                AND (
                    {folder_alias}.path = xf.path
                    OR instr({folder_alias}.path, xf.path || '{}') = 1
                )
            )",
            sep
        )
    }

    fn new(folder_id: i64, file_path: &str, file_type: i64) -> Result<Self, String> {
        Self::new_profiled(folder_id, file_path, file_type, None)
    }

    fn new_profiled(
        folder_id: i64,
        file_path: &str,
        file_type: i64,
        profile: Option<&mut AFileAddProfile>,
    ) -> Result<Self, String> {
        let mut metadata_profile = AFileMetadataProfiler::new(profile);
        let file_info = metadata_profile.measure(AFileMetadataPhase::FileInfo, || {
            t_utils::FileInfo::new(file_path)
        })?;

        // get dimensions and duration based on file type
        let (mut width, mut height, mut duration) = (0u32, 0u32, 0u64);

        // Initialize metadata fields
        let mut taken_date: Option<i64> = None;
        let mut e_make: Option<String> = None;
        let mut e_model: Option<String> = None;
        let mut e_date_time: Option<String> = None;
        let mut e_software: Option<String> = None;
        let mut e_artist: Option<String> = None;
        let mut e_copyright: Option<String> = None;
        let mut e_description: Option<String> = None;
        let mut e_lens_make: Option<String> = None;
        let mut e_lens_model: Option<String> = None;
        let mut e_exposure_bias: Option<String> = None;
        let mut e_exposure_time: Option<String> = None;
        let mut e_f_number: Option<String> = None;
        let mut e_focal_length: Option<String> = None;
        let mut e_iso_speed: Option<String> = None;
        let mut e_flash: Option<String> = None;
        let mut e_orientation: Option<u32> = None;
        let mut gps_latitude: Option<f64> = None;
        let mut gps_longitude: Option<f64> = None;
        let mut gps_altitude: Option<f64> = None;

        // Live Photo / Motion Photo pairing
        let mut content_id: Option<String> = None;
        let mut live_photo_type: i64 = 0;
        // Optional EXIF UserComment for AI prompt import (JPEG etc.)
        let mut exif_user_comment: Option<String> = None;

        // Pre-read file header once for images (saves 3-4 redundant File::open per file).
        let file_header = metadata_profile.measure(AFileMetadataPhase::Header, || {
            Self::read_file_header(file_path, file_type)
        });
        let file_header_deref = file_header.as_deref();

        metadata_profile.measure(AFileMetadataPhase::Dimensions, || {
            match file_type {
                1 => {
                    let (w, h) = t_image::get_image_dimensions(file_path)?;
                    width = w;
                    height = h;
                }
                2 => {
                    let video_metadata = t_video::get_video_metadata(file_path)?;
                    width = video_metadata.width;
                    height = video_metadata.height;
                    duration = video_metadata.duration;
                    e_make = video_metadata.e_make;
                    e_model = video_metadata.e_model;
                    e_date_time = video_metadata.e_date_time;
                    e_software = video_metadata.e_software;
                    gps_latitude = video_metadata.gps_latitude;
                    gps_longitude = video_metadata.gps_longitude;
                    gps_altitude = video_metadata.gps_altitude;
                    // Apple Live Photo video: content identifier from MOV metadata
                    content_id = video_metadata.content_id;
                    if content_id.is_some() {
                        live_photo_type = 2; // Apple Live Photo video
                    }
                }
                3 => {
                    let (w, h) = t_image::get_raw_dimensions(file_path)?;
                    width = w;
                    height = h;
                }
                _ => {}
            }
            Ok::<(), String>(())
        })?;

        let format_label = if let Some(hdr) = file_header_deref {
            if file_type == 3 {
                Some("RAW".to_string())
            } else {
                t_utils::detect_label_from_header(hdr, file_type)
            }
        } else {
            t_utils::detect_file_format_label(file_path, file_type)
        };

        if file_type == 1 || file_type == 3 {
            // Image file — reuse the pre-read header when it contains EXIF.
            // Some older JPEGs place EXIF after large APP segments (such as an
            // ICC profile), beyond this header buffer. Fall back to scanning
            // the full JPEG so their capture settings are indexed as well.
            let exif = if metadata_profile.enabled() {
                metadata_profile.measure_exif(|metadata_profile| {
                    Self::read_image_exif_profiled(
                        file_path,
                        file_type,
                        file_header_deref,
                        metadata_profile,
                    )
                })
            } else {
                Self::read_image_exif(file_path, file_type, file_header_deref)
            };
            // One binary pass feeds both missing EXIF fields and Apple Live
            // ContentIdentifier fallback below.
            let mut binary_profile = BinaryExifFallbackProfile::default();
            let profile_binary = metadata_profile.enabled();
            let binary_exif = metadata_profile.measure(AFileMetadataPhase::BinaryFallback, || {
                file_header_deref
                    .filter(|header| Self::should_scrape_binary_exif_fallback(header))
                    .map(|header| {
                        Self::scrape_binary_exif_fallback_profiled(
                            header,
                            profile_binary.then_some(&mut binary_profile),
                        )
                    })
                    .unwrap_or_default()
            });
            if let Some(profile) = metadata_profile.profile.as_deref_mut() {
                profile.metadata_binary_tiff_signature_attempts +=
                    binary_profile.tiff_signature_attempts;
                profile.metadata_binary_tiff_signature_elapsed +=
                    binary_profile.tiff_signature_elapsed;
                profile.metadata_binary_tiff_bases_found += binary_profile.tiff_bases_found;
                profile.metadata_binary_complete_jpeg_without_exif_attempts +=
                    binary_profile.complete_jpeg_without_exif_attempts;
                profile.metadata_binary_complete_jpeg_without_exif_tiff_bases_found +=
                    binary_profile.complete_jpeg_without_exif_tiff_bases_found;
                profile.metadata_binary_entry_scan_attempts += binary_profile.entry_scan_attempts;
                profile.metadata_binary_entry_scan_elapsed += binary_profile.entry_scan_elapsed;
                profile.metadata_binary_value_decode_elapsed += binary_profile.value_decode_elapsed;
            }

            // Extracts EXIF orientation field.
            // 1: Horizontal (normal)
            // 2: Mirror horizontal
            // 3: Rotate 180
            // 4: Mirror vertical
            // 5: Mirror horizontal and rotate 270 CW
            // 6: Rotate 90 CW
            // 7: Mirror horizontal and rotate 90 CW
            // 8: Rotate 270 CW
            metadata_profile.measure(AFileMetadataPhase::ExifExtractOrientation, || {
                e_orientation = Some(Self::extract_orientation(&exif, file_header_deref));
            });

            metadata_profile.measure(AFileMetadataPhase::ExifExtractFlash, || {
                // Process flash data
                e_flash = exif.as_ref().and_then(|exif_data| {
                    exif_data
                        .get_field(Tag::Flash, In::PRIMARY)
                        .and_then(|field| field.value.get_uint(0))
                        .map(|val| {
                            if val & 1 == 1 {
                                "Fired".to_string()
                            } else {
                                "Not fired".to_string()
                            }
                        })
                });
            });

            metadata_profile.measure(AFileMetadataPhase::ExifExtractGps, || {
                let (lat, lon, alt) = Self::extract_gps_data(&exif);
                gps_latitude = lat;
                gps_longitude = lon;
                gps_altitude = alt;
            });

            metadata_profile.measure(AFileMetadataPhase::ExifExtractIdentity, || {
                let identity = Self::extract_exif_identity(&exif, file_info.modified);
                taken_date = identity.taken_date;
                e_make = identity.make;
                e_model = identity.model;
                e_date_time = identity.date_time;
                e_software = identity.software;
            });

            metadata_profile.measure(AFileMetadataPhase::ExifExtractDescription, || {
                let description = Self::extract_exif_description(&exif);
                e_artist = description.artist;
                e_copyright = description.copyright;
                e_description = description.description;
                exif_user_comment = description.user_comment;
            });

            metadata_profile.measure(AFileMetadataPhase::ExifExtractCapture, || {
                let capture = Self::extract_exif_capture(&exif);
                e_lens_make = capture.lens_make;
                e_lens_model = capture.lens_model;
                e_exposure_bias = capture.exposure_bias;
                e_exposure_time = capture.exposure_time;
                e_f_number = capture.f_number;
                e_focal_length = capture.focal_length;
                e_iso_speed = capture.iso_speed;
            });

            // The editor uses little_exif to preserve metadata. Some legacy
            // JPEGs are accepted by that reader but rejected by kamadak-exif,
            // which previously made capture settings appear only after an edit
            // was saved as a new image. Only use the same reader when
            // kamadak-exif found none of the capture settings, so a record
            // never combines partial values from two parsers.
            if t_image::is_jpeg_path(file_path)
                && e_exposure_time.is_none()
                && e_f_number.is_none()
                && e_focal_length.is_none()
                && e_iso_speed.is_none()
            {
                let capture_settings = metadata_profile
                    .measure(AFileMetadataPhase::CaptureFallback, || {
                        t_image::read_capture_settings_with_little_exif(file_path)
                    });
                e_exposure_time = e_exposure_time.or(capture_settings.exposure_time);
                e_f_number = e_f_number.or(capture_settings.f_number);
                e_focal_length = e_focal_length.or(capture_settings.focal_length);
                e_iso_speed = e_iso_speed.or(capture_settings.iso_speed);
            }

            // Fallback: infer lens make from lens model prefix when LensMake is missing.
            if e_lens_make.is_none() {
                if let Some(model) = e_lens_model.as_deref() {
                    e_lens_make = t_lens::infer_lens_make(model).map(|s| s.to_string());
                }
            }

            // For RAW files, LibRaw is the primary metadata source.
            // It reads the file directly and does not rely on the embedded JPEG
            // that the permissive EXIF reader scans, so it is robust against
            // RAW files whose EXIF data is stored outside the preview image.
            if file_type == 3 {
                if let Ok(meta) = metadata_profile.measure(AFileMetadataPhase::Raw, || {
                    t_libraw::get_raw_meta(file_path)
                }) {
                    RawMetadataTarget {
                        make: &mut e_make,
                        model: &mut e_model,
                        software: &mut e_software,
                        artist: &mut e_artist,
                        description: &mut e_description,
                        iso_speed: &mut e_iso_speed,
                        exposure_time: &mut e_exposure_time,
                        f_number: &mut e_f_number,
                        focal_length: &mut e_focal_length,
                        flash: &mut e_flash,
                        lens_make: &mut e_lens_make,
                        lens_model: &mut e_lens_model,
                        taken_date: &mut taken_date,
                        modified_at: file_info.modified,
                    }
                    .apply(meta);
                }
            }

            // Binary String Fallback if metadata is still missing (Industry standard for tough files)
            if e_make.is_none()
                || e_model.is_none()
                || e_date_time.is_none()
                || e_software.is_none()
                || e_lens_make.is_none()
                || e_lens_model.is_none()
            {
                if e_make.is_none() {
                    e_make = binary_exif.make.clone();
                }
                if e_model.is_none() {
                    e_model = binary_exif.model.clone();
                }
                if e_date_time.is_none() {
                    e_date_time = binary_exif
                        .date_time_original
                        .clone()
                        .or_else(|| binary_exif.date_time.clone());
                }
                if e_software.is_none() {
                    e_software = binary_exif.software.clone();
                }
                if e_lens_model.is_none() {
                    e_lens_model = binary_exif.lens_model.clone();
                }
                if e_lens_make.is_none() {
                    e_lens_make = binary_exif.lens_make.clone();
                }
                // Extra Orientation fallback for Sony MakerNotes (Tag 0x2000)
                if (e_orientation.is_none() || e_orientation == Some(1))
                    && let Some(so) = binary_exif.sony_orientation
                    && (1..=8).contains(&so)
                {
                    e_orientation = Some(so as u32);
                }
            }

            if e_lens_make.is_none() {
                if let Some(model) = e_lens_model.as_deref() {
                    e_lens_make = t_lens::infer_lens_make(model).map(|s| s.to_string());
                }
            }

            // Re-update taken_date if we found e_date_time via binary fallback
            if taken_date == file_info.modified {
                if let Some(dt) = e_date_time.as_ref() {
                    if let Some(ts) = t_utils::meta_date_to_timestamp(dt) {
                        taken_date = Some(ts);
                    }
                }
            }

            // Apple Live Photo: read ContentIdentifier (EXIF tag 0x0011 in TIFF/IFD0).
            // This UUID matches the MOV side's com.apple.quicktime.content.identifier.
            content_id = exif.as_ref().and_then(|exif_data| {
                exif_data
                    .get_field(Tag(exif::Context::Tiff, 0x0011), In::PRIMARY)
                    .and_then(|field| {
                        field
                            .value
                            .display_as(field.tag)
                            .to_string()
                            .strip_suffix('\0')
                            .map(|s| s.to_string())
                    })
                    .filter(|s| !s.is_empty())
            });
            // Binary fallback for ContentIdentifier
            if content_id.is_none() {
                content_id = binary_exif.content_id;
            }
            if content_id.is_some() {
                live_photo_type = 1; // Apple Live Photo image
            }

            // Google Motion Photo detection via XMP.
            // Guard with catch_unwind so a bad XMP packet never aborts indexing
            // for an otherwise normal JPEG/HEIC.
            if content_id.is_none() {
                let mut motion_profile = crate::t_xmp::MotionPhotoReadProfile::default();
                let profile_motion = metadata_profile.enabled();
                let motion_info = metadata_profile.measure(AFileMetadataPhase::Motion, || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::t_xmp::detect_motion_photo_with_header_profiled(
                            file_path,
                            file_header_deref,
                            profile_motion.then_some(&mut motion_profile),
                        )
                    }))
                    .unwrap_or_else(|_| {
                        eprintln!(
                            "Motion Photo detection panicked for {}; continuing without live metadata",
                            file_path
                        );
                        None
                    })
                });
                if let Some(profile) = metadata_profile.profile.as_deref_mut() {
                    profile.metadata_motion_header_xmp_attempts +=
                        motion_profile.header_xmp_attempts;
                    profile.metadata_motion_header_xmp_elapsed += motion_profile.header_xmp_elapsed;
                    profile.metadata_motion_header_complete_check_attempts +=
                        motion_profile.header_complete_check_attempts;
                    profile.metadata_motion_header_complete_check_elapsed +=
                        motion_profile.header_complete_check_elapsed;
                    profile.metadata_motion_file_fallback_attempts +=
                        motion_profile.file_fallback_attempts;
                    profile.metadata_motion_file_fallback_elapsed +=
                        motion_profile.file_fallback_elapsed;
                    profile.metadata_motion_parse_attempts += motion_profile.parse_attempts;
                    profile.metadata_motion_parse_elapsed += motion_profile.parse_elapsed;
                }
                if let Some(motion_info) = motion_info {
                    // Encode the video offset in content_id as "motion:<offset>:<length>"
                    let length_str = motion_info
                        .video_length
                        .map(|l| l.to_string())
                        .unwrap_or_default();
                    content_id = Some(format!(
                        "motion:{}:{}",
                        motion_info.video_offset, length_str
                    ));
                    live_photo_type = 3; // Google Motion Photo
                }
            }

            // HEIC container-internal video (item or sequence) when not already
            // classified as Apple Live (paired MOV) or Motion Photo.
            #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
            if content_id.is_none() && t_image::is_heic_path(file_path) {
                let heic_info = metadata_profile.measure(AFileMetadataPhase::Heic, || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::t_heif::detect_heic_embedded_video(file_path)
                    }))
                    .unwrap_or_else(|_| {
                        eprintln!(
                            "HEIC embedded-video detect panicked for {}; continuing",
                            file_path
                        );
                        None
                    })
                });
                if let Some(info) = heic_info {
                    content_id = Some(info.content_id_marker());
                    live_photo_type = 4; // HEIC-internal video
                }
            }
        } else if file_type == 2 {
            taken_date = e_date_time
                .as_ref()
                .and_then(|dt| t_utils::meta_date_to_timestamp(dt))
                .or(file_info.modified);
        }

        // Geocoding based on GPS coordinates from any source
        let (geo_name, geo_admin1, geo_admin2, geo_cc) =
            metadata_profile.measure(AFileMetadataPhase::Geocode, || {
                if let (Some(lat), Some(lon)) = (gps_latitude, gps_longitude) {
                    match t_utils::GEOCODER.search((lat, lon)) {
                        Some(result) => (
                            Some(result.record.name.clone()),
                            Some(result.record.admin1.clone()),
                            Some(result.record.admin2.clone()),
                            Some(result.record.cc.clone()),
                        ),
                        None => (None, None, None, None),
                    }
                } else {
                    (None, None, None, None)
                }
            });

        // RAW dimensions are already orientation-adjusted in `get_raw_dimensions`.
        let should_swap_dimensions_for_orientation =
            file_type != 3 && !t_image::is_heic_path(file_path);

        // Optional AI PNG/JPEG prompt → comments (empty-only fill on insert / change rescan).
        let comments = metadata_profile.measure(AFileMetadataPhase::Prompt, || {
            if file_type == 1 {
                crate::t_ai_prompt::extract_prompt_for_path(
                    file_path,
                    file_header_deref,
                    exif_user_comment.as_deref(),
                    e_description.as_deref(),
                )
            } else {
                None
            }
        });

        let file = metadata_profile.measure(AFileMetadataPhase::Assemble, || Self {
            id: None,
            folder_id,

            name: file_info.file_name.clone(),
            name_pinyin: Some(t_utils::natural_sort_key(
                &file_info.file_name.to_lowercase(),
            )), // natural sort key (case-insensitive, pinyin + zero-padded numbers)
            size: file_info.file_size,
            file_type: Some(file_type),
            format_label,
            created_at: file_info.created,
            modified_at: file_info.modified,
            inode: Some(file_info.inode as i64),

            taken_date,
            width: e_orientation
                .map(|orientation| {
                    if should_swap_dimensions_for_orientation && orientation > 4 {
                        height
                    } else {
                        width
                    }
                })
                .or(Some(width)),
            height: e_orientation
                .map(|orientation| {
                    if should_swap_dimensions_for_orientation && orientation > 4 {
                        width
                    } else {
                        height
                    }
                })
                .or(Some(height)),
            duration: Some(duration as i64),

            is_favorite: None,
            rating: Some(0),
            rotate: None,
            comments,
            has_tags: Some(false),
            has_faces: Some(0),

            e_make,
            e_model,
            e_date_time,
            e_software,
            e_artist,
            e_copyright,
            e_description,
            e_lens_make,
            e_lens_model,
            e_exposure_bias,
            e_exposure_time,
            e_f_number,
            e_focal_length,
            e_iso_speed,
            e_flash,
            e_orientation,

            gps_latitude,
            gps_longitude,
            gps_altitude,

            geo_name,
            geo_admin1,
            geo_admin2,
            geo_cc,

            content_id,
            paired_file_id: None,
            live_photo_type: Some(live_photo_type),

            file_path: None,
            album_id: None,
            album_name: None,
            has_thumbnail: None,
            has_embedding: None,
            last_scan_time: Some(0),
        });

        Ok(file)
    }

    fn read_file_header(file_path: &str, file_type: i64) -> Option<Vec<u8>> {
        if file_type != 1 && file_type != 3 {
            return None;
        }

        let mut file = std::fs::File::open(file_path).ok()?;
        use std::io::Read;
        let mut buf = vec![0u8; 128 * 1024];
        let read = file.read(&mut buf).ok()?;
        buf.truncate(read);
        Some(buf)
    }

    fn read_image_exif(
        file_path: &str,
        file_type: i64,
        file_header: Option<&[u8]>,
    ) -> Option<exif::Exif> {
        if file_type != 1 && file_type != 3 {
            return None;
        }
        if let Some(hdr) = file_header {
            if file_type == 1
                && t_image::is_jpeg_path(file_path)
                && t_image::jpeg_header_complete_without_exif(hdr)
            {
                return None;
            }
            return t_image::read_exif_from_bytes_permissive(hdr).or_else(|| {
                (file_type == 1 && t_image::is_jpeg_path(file_path))
                    .then(|| t_image::read_exif_permissive(file_path))
                    .flatten()
            });
        }
        t_image::read_exif_permissive(file_path)
    }

    fn read_image_exif_profiled(
        file_path: &str,
        file_type: i64,
        file_header: Option<&[u8]>,
        metadata_profile: &mut AFileMetadataProfiler<'_>,
    ) -> Option<exif::Exif> {
        if file_type != 1 && file_type != 3 {
            return None;
        }
        if let Some(hdr) = file_header {
            // A complete JPEG marker walk proves the header has reached SOS/EOI
            // without an EXIF APP1 segment, so neither reader recovery path can
            // find EXIF metadata. Keep all incomplete and EXIF-bearing headers
            // on the permissive path below.
            if file_type == 1
                && t_image::is_jpeg_path(file_path)
                && t_image::jpeg_header_complete_without_exif(hdr)
            {
                return None;
            }
            let mut exif_profile = t_image::ExifReadProfile::default();
            let exif = metadata_profile.measure(AFileMetadataPhase::ExifHeader, || {
                t_image::read_exif_from_bytes_permissive_profiled(hdr, &mut exif_profile)
            });
            if let Some(profile) = metadata_profile.profile.as_deref_mut() {
                profile.metadata_exif_container_attempts += exif_profile.container_attempts;
                profile.metadata_exif_container_elapsed += exif_profile.container_elapsed;
                profile.metadata_exif_signature_scan_elapsed += exif_profile.signature_scan_elapsed;
                profile.metadata_exif_raw_attempts += exif_profile.raw_attempts;
                profile.metadata_exif_raw_elapsed += exif_profile.raw_elapsed;
            }
            return exif.or_else(|| {
                (file_type == 1
                    && t_image::is_jpeg_path(file_path)
                    && !t_image::jpeg_header_complete_without_exif(hdr))
                .then(|| {
                    metadata_profile.measure(AFileMetadataPhase::ExifFileFallback, || {
                        t_image::read_exif_permissive(file_path)
                    })
                })
                .flatten()
            });
        }
        metadata_profile.measure(AFileMetadataPhase::ExifFileFallback, || {
            t_image::read_exif_permissive(file_path)
        })
    }

    fn extract_orientation(exif: &Option<exif::Exif>, file_header: Option<&[u8]>) -> u32 {
        let mut orientation = exif.as_ref().and_then(|exif_data| {
            exif_data
                .get_field(Tag::Orientation, In::PRIMARY)
                .or_else(|| exif_data.fields().find(|f| f.tag == Tag::Orientation))
                .and_then(|field| field.value.get_uint(0))
                .map(|v| v as u32)
        });

        if orientation.is_none() || orientation == Some(1) {
            if let Some(hdr) =
                file_header.filter(|hdr| !t_image::jpeg_header_complete_without_exif(hdr))
            {
                if let Some(binary_orientation) = t_image::scan_orientation_binary(hdr) {
                    orientation = Some(binary_orientation as u32);
                }
            }
        }

        orientation.unwrap_or(1)
    }

    fn extract_exif_identity(exif: &Option<exif::Exif>, modified_at: Option<i64>) -> ExifIdentity {
        let date_time = Self::get_exif_field(exif, Tag::DateTimeOriginal);
        ExifIdentity {
            taken_date: date_time
                .as_deref()
                .and_then(t_utils::meta_date_to_timestamp)
                .or(modified_at),
            make: Self::get_exif_field(exif, Tag::Make),
            model: Self::get_exif_field(exif, Tag::Model),
            date_time,
            software: Self::get_exif_field(exif, Tag::Software),
        }
    }

    fn extract_exif_description(exif: &Option<exif::Exif>) -> ExifDescription {
        ExifDescription {
            artist: Self::get_exif_field(exif, Tag::Artist),
            copyright: Self::get_exif_field(exif, Tag::Copyright),
            description: Self::get_exif_field(exif, Tag::ImageDescription),
            // Prefer raw UserComment decode (charset header) over display_value stripping.
            user_comment: exif
                .as_ref()
                .and_then(crate::t_ai_prompt::extract_user_comment_from_exif)
                .or_else(|| Self::get_exif_field(exif, Tag::UserComment)),
        }
    }

    fn extract_exif_capture(exif: &Option<exif::Exif>) -> ExifCapture {
        ExifCapture {
            lens_make: Self::get_exif_field(exif, Tag::LensMake),
            lens_model: Self::get_exif_field(exif, Tag::LensModel),
            exposure_bias: Self::get_exif_field(exif, Tag::ExposureBiasValue),
            exposure_time: Self::get_exif_field(exif, Tag::ExposureTime),
            f_number: Self::get_exif_field(exif, Tag::FNumber),
            focal_length: Self::get_exif_field(exif, Tag::FocalLength),
            iso_speed: Self::get_exif_field(exif, Tag::PhotographicSensitivity),
        }
    }

    fn extract_gps_data(exif: &Option<exif::Exif>) -> (Option<f64>, Option<f64>, Option<f64>) {
        let Some(exif_data) = exif else {
            return (None, None, None);
        };

        let lat_val = exif_data
            .get_field(Tag::GPSLatitude, In::PRIMARY)
            .or_else(|| exif_data.fields().find(|f| f.tag == Tag::GPSLatitude))
            .and_then(|f| match &f.value {
                Value::Rational(v) => Some(v.to_vec()),
                _ => None,
            });
        let lat_ref = exif_data
            .get_field(Tag::GPSLatitudeRef, In::PRIMARY)
            .or_else(|| exif_data.fields().find(|f| f.tag == Tag::GPSLatitudeRef))
            .map(|f| f.display_value().to_string());
        let lon_val = exif_data
            .get_field(Tag::GPSLongitude, In::PRIMARY)
            .or_else(|| exif_data.fields().find(|f| f.tag == Tag::GPSLongitude))
            .and_then(|f| match &f.value {
                Value::Rational(v) => Some(v.to_vec()),
                _ => None,
            });
        let lon_ref = exif_data
            .get_field(Tag::GPSLongitudeRef, In::PRIMARY)
            .or_else(|| exif_data.fields().find(|f| f.tag == Tag::GPSLongitudeRef))
            .map(|f| f.display_value().to_string());

        let (gps_lat, gps_lon) = if let (Some(lat_v), Some(lat_r), Some(lon_v), Some(lon_r)) =
            (lat_val, lat_ref, lon_val, lon_ref)
        {
            (
                Self::dms_to_decimal(&lat_v, &lat_r),
                Self::dms_to_decimal(&lon_v, &lon_r),
            )
        } else {
            (None, None)
        };

        let altitude = exif_data
            .get_field(Tag::GPSAltitude, In::PRIMARY)
            .and_then(|field| match &field.value {
                Value::Rational(v) if !v.is_empty() => Some(v[0].num as f64 / v[0].denom as f64),
                _ => None,
            });

        (gps_lat, gps_lon, altitude)
    }

    /// Converts DMS (degrees, minutes, seconds) to decimal degrees.
    fn dms_to_decimal(dms: &[exif::Rational], reference: &str) -> Option<f64> {
        if dms.len() != 3 {
            return None;
        }
        let degrees = dms[0].num as f64 / dms[0].denom as f64;
        let minutes = dms[1].num as f64 / dms[1].denom as f64;
        let seconds = dms[2].num as f64 / dms[2].denom as f64;

        let mut decimal = degrees + minutes / 60.0 + seconds / 3600.0;

        if reference.starts_with("S") || reference.starts_with("W") {
            decimal = -decimal;
        }
        Some(decimal)
    }

    /// Formats DMS coordinates as a string (e.g., "40°42'45\"N").
    // fn format_dms(dms: &[exif::Rational], reference: &str) -> String {
    //     if dms.len() < 3 {
    //         return String::new();
    //     }
    //     let degrees = dms[0].num as f64 / dms[0].denom as f64;
    //     let minutes = dms[1].num as f64 / dms[1].denom as f64;
    //     let seconds = dms[2].num as f64 / dms[2].denom as f64;
    //     format!("{:.0}°{:.0}′{:.0}″{}", degrees, minutes, seconds, reference.trim())
    // }

    /// Extracts an EXIF field as a string.
    pub fn get_exif_field(exif: &Option<exif::Exif>, tag: Tag) -> Option<String> {
        let ex = exif.as_ref()?;
        let field = ex
            .get_field(tag, In::PRIMARY)
            .or_else(|| ex.fields().find(|f| f.tag == tag))?;

        let raw = match &field.value {
            Value::Ascii(vec) => {
                let mut bytes = Vec::new();
                for line in vec {
                    let cleaned: Vec<u8> = line.iter().cloned().take_while(|&b| b != 0).collect();
                    bytes.extend(cleaned);
                }
                String::from_utf8_lossy(&bytes).into_owned()
            }
            // Keep unit spacing from the EXIF crate (e.g. "24 mm", "1/30 s")
            // so JPG and RAW display consistently after LibRaw spacing fixes.
            _ => field.display_value().with_unit(exif.as_ref()?).to_string(),
        };

        let cleaned = raw
            .replace(['"', '\''], "")
            .lines()
            .map(|line| {
                let mut s = line.trim().to_string();
                while let Some(last) = s.chars().last() {
                    if last.is_ascii_punctuation() && last != ')' && last != '(' {
                        s.pop();
                    } else {
                        break;
                    }
                }
                s
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let final_str = cleaned.trim();
        if final_str.is_empty() {
            None
        } else {
            Some(final_str.to_string())
        }
    }

    #[cfg(test)]
    fn scrape_binary_exif_fallback(data: &[u8]) -> BinaryExifFallback {
        Self::scrape_binary_exif_fallback_profiled(data, None)
    }

    /// A complete JPEG header without EXIF APP1 cannot contain the TIFF base
    /// sought by this fallback. Keep incomplete, non-JPEG, and EXIF-bearing
    /// headers on the tolerant scan path.
    fn should_scrape_binary_exif_fallback(data: &[u8]) -> bool {
        !t_image::jpeg_header_complete_without_exif(data)
    }

    fn scrape_binary_exif_fallback_profiled(
        data: &[u8],
        mut profile: Option<&mut BinaryExifFallbackProfile>,
    ) -> BinaryExifFallback {
        let complete_jpeg_without_exif = profile
            .as_ref()
            .is_some_and(|_| t_image::jpeg_header_complete_without_exif(data));
        if let Some(profile) = profile.as_deref_mut() {
            profile.complete_jpeg_without_exif_attempts += 1;
        }
        // Find the TIFF base (where the EXIF/TIFF header starts)
        let started = profile.as_ref().map(|_| Instant::now());
        let Some(tiff_base) = data
            .windows(4)
            .position(|w| w == b"II\x2a\x00" || w == b"MM\x00\x2a")
        else {
            if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
                profile.tiff_signature_attempts += 1;
                profile.tiff_signature_elapsed += started.elapsed();
            }
            return BinaryExifFallback::default();
        };
        if let (Some(profile), Some(started)) = (profile.as_deref_mut(), started) {
            profile.tiff_signature_attempts += 1;
            profile.tiff_signature_elapsed += started.elapsed();
            profile.tiff_bases_found += 1;
            profile.complete_jpeg_without_exif_tiff_bases_found +=
                u64::from(complete_jpeg_without_exif);
        }

        let mut fallback = BinaryExifFallback::default();
        let entry_scan_started = profile.as_ref().map(|_| Instant::now());
        for (pos, entry) in data.windows(12).enumerate() {
            let (tag, field_type, count, offset, little_endian) =
                if entry[2..4] == [0x02, 0x00] || entry[2..4] == [0x03, 0x00] {
                    (
                        u16::from_le_bytes([entry[0], entry[1]]),
                        u16::from_le_bytes([entry[2], entry[3]]),
                        u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]) as usize,
                        u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize,
                        true,
                    )
                } else if entry[2..4] == [0x00, 0x02] || entry[2..4] == [0x00, 0x03] {
                    (
                        u16::from_be_bytes([entry[0], entry[1]]),
                        u16::from_be_bytes([entry[2], entry[3]]),
                        u32::from_be_bytes([entry[4], entry[5], entry[6], entry[7]]) as usize,
                        u32::from_be_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize,
                        false,
                    )
                } else {
                    continue;
                };

            if field_type == 2
                && count > 1
                && count < 256
                && matches!(
                    tag,
                    0x010f | 0x0110 | 0x9003 | 0x0132 | 0x0131 | 0xa434 | 0xa433 | 0x0011
                )
            {
                let start = if count <= 4 {
                    pos + 8
                } else {
                    tiff_base + offset
                };
                let value_started = profile.as_ref().map(|_| Instant::now());
                let value = data
                    .get(start..start.saturating_add(count.saturating_sub(1)))
                    .and_then(|bytes| {
                        let value = String::from_utf8_lossy(bytes)
                            .trim()
                            .trim_matches('\0')
                            .trim()
                            .to_string();
                        (!value.is_empty()
                            && value
                                .chars()
                                .all(|c| c.is_ascii_graphic() || c.is_whitespace()))
                        .then_some(value)
                    });
                if let (Some(profile), Some(started)) = (profile.as_deref_mut(), value_started) {
                    profile.value_decode_elapsed += started.elapsed();
                }
                match tag {
                    0x010f if fallback.make.is_none() => fallback.make = value,
                    0x0110 if fallback.model.is_none() => fallback.model = value,
                    0x9003 if fallback.date_time_original.is_none() => {
                        fallback.date_time_original = value
                    }
                    0x0132 if fallback.date_time.is_none() => fallback.date_time = value,
                    0x0131 if fallback.software.is_none() => fallback.software = value,
                    0xa434 if fallback.lens_model.is_none() => fallback.lens_model = value,
                    0xa433 if fallback.lens_make.is_none() => fallback.lens_make = value,
                    0x0011 if fallback.content_id.is_none() => fallback.content_id = value,
                    _ => {}
                }
            } else if tag == 0x2000 && field_type == 3 && fallback.sony_orientation.is_none() {
                fallback.sony_orientation = match count {
                    0 => None,
                    _ if little_endian => Some(u16::from_le_bytes([entry[8], entry[9]])),
                    _ => Some(u16::from_be_bytes([entry[8], entry[9]])),
                };
            }
        }
        if let (Some(profile), Some(started)) = (profile.as_deref_mut(), entry_scan_started) {
            profile.entry_scan_attempts += 1;
            profile.entry_scan_elapsed += started.elapsed();
        }
        fallback
    }

    /// insert a file into db
    fn insert(&self) -> Result<usize, String> {
        let conn = open_conn()?;
        self.insert_with_conn(&conn)
    }

    fn insert_with_conn(&self, conn: &Connection) -> Result<usize, String> {
        let result = conn.execute(
            "INSERT INTO afiles (
                folder_id, 
                name, name_pinyin, size, file_type, format_label, created_at, modified_at, inode,
                taken_date,
                width, height, duration,
                is_favorite, rating, rotate, comments, has_tags,
                e_make, e_model, e_date_time, e_software, e_artist, e_copyright, e_description, e_lens_make, e_lens_model, e_exposure_bias, e_exposure_time, e_f_number, e_focal_length, e_iso_speed, e_flash, e_orientation,
                gps_latitude, gps_longitude, gps_altitude, geo_name, geo_admin1, geo_admin2, geo_cc,
                content_id, paired_file_id, live_photo_type,
                last_scan_time
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45)
            ON CONFLICT(folder_id, name) DO NOTHING",
            params![
                self.folder_id,

                self.name,
                self.name_pinyin,
                self.size,
                self.file_type,
                self.format_label,
                self.created_at,
                self.modified_at,
                self.inode,

                self.taken_date,

                self.width,
                self.height,
                self.duration,

                self.is_favorite,
                self.rating,
                self.rotate,
                self.comments,
                self.has_tags,

                self.e_make,
                self.e_model,
                self.e_date_time,
                self.e_software,
                self.e_artist,
                self.e_copyright,
                self.e_description,
                self.e_lens_make,
                self.e_lens_model,
                self.e_exposure_bias,
                self.e_exposure_time,
                self.e_f_number,
                self.e_focal_length,
                self.e_iso_speed,
                self.e_flash,
                self.e_orientation,

                self.gps_latitude,
                self.gps_longitude,
                self.gps_altitude,
                self.geo_name,
                self.geo_admin1,
                self.geo_admin2,
                self.geo_cc,
                self.content_id,
                self.paired_file_id,
                self.live_photo_type,
                self.last_scan_time,
            ]
        ).map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// update a file into db
    pub fn update(file_id: i64, file: &Self) -> Result<usize, String> {
        let conn = open_conn()?;
        Self::update_with_conn(&conn, file_id, file)
    }

    fn update_with_conn(conn: &Connection, file_id: i64, file: &Self) -> Result<usize, String> {
        let result = conn.execute(
            "UPDATE afiles SET
                name = ?1, name_pinyin = ?2, size = ?3, file_type = ?4, format_label = ?5, created_at = ?6, modified_at = ?7, inode = ?8,
                taken_date = ?9,
                width = ?10, height = ?11, duration = ?12,
                rating = ?13,
                e_make = ?14, e_model = ?15, e_date_time = ?16, e_software = ?17, e_artist = ?18, e_copyright = ?19, e_description = ?20, e_lens_make = ?21, e_lens_model = ?22, e_exposure_bias = ?23, e_exposure_time = ?24, e_f_number = ?25, e_focal_length = ?26, e_iso_speed = ?27, e_flash = ?28, e_orientation = ?29,
                gps_latitude = ?30, gps_longitude = ?31, gps_altitude = ?32, geo_name = ?33, geo_admin1 = ?34, geo_admin2 = ?35, geo_cc = ?36,
                content_id = ?37, paired_file_id = ?38, live_photo_type = ?39,
                last_scan_time = ?40
            WHERE id = ?41",
            params![
                file.name,
                file.name_pinyin,
                file.size,
                file.file_type,
                file.format_label,
                file.created_at,
                file.modified_at,
                file.inode,

                file.taken_date,

                file.width,
                file.height,
                file.duration,

                file.rating,
                file.e_make,
                file.e_model,
                file.e_date_time,
                file.e_software,
                file.e_artist,
                file.e_copyright,
                file.e_description,
                file.e_lens_make,
                file.e_lens_model,
                file.e_exposure_bias,
                file.e_exposure_time,
                file.e_f_number,
                file.e_focal_length,
                file.e_iso_speed,
                file.e_flash,
                file.e_orientation,

                file.gps_latitude,
                file.gps_longitude,
                file.gps_altitude,
                file.geo_name,
                file.geo_admin1,
                file.geo_admin2,
                file.geo_cc,
                file.content_id,
                file.paired_file_id,
                file.live_photo_type,
                file.last_scan_time,
                file_id,
            ]
        ).map_err(|e| e.to_string())?;
        Ok(result)
    }

    // delete a file from db
    pub fn delete(id: i64) -> Result<usize, String> {
        let mut conn = open_conn()?;
        Self::delete_with_conn(&mut conn, id)
    }

    fn delete_with_conn(conn: &mut Connection, id: i64) -> Result<usize, String> {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM athumbs WHERE file_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        // Best-effort hash hygiene (tables may not exist on ancient DBs).
        let _ = tx.execute("DELETE FROM file_hashes WHERE file_id = ?1", params![id]);
        let _ = tx.execute("DELETE FROM file_phashes WHERE file_id = ?1", params![id]);
        let result = tx
            .execute("DELETE FROM afiles WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        if result > 0 {
            invalidate_embed_matrix_for_current_db();
        }
        Ok(result)
    }

    pub fn batch_delete(ids: &[i64]) -> Result<usize, String> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut deleted = 0;
        {
            let mut thumb_stmt = tx
                .prepare_cached("DELETE FROM athumbs WHERE file_id = ?1")
                .map_err(|e| e.to_string())?;
            let mut file_stmt = tx
                .prepare_cached("DELETE FROM afiles WHERE id = ?1")
                .map_err(|e| e.to_string())?;
            let mut hash_stmt = tx
                .prepare_cached("DELETE FROM file_hashes WHERE file_id = ?1")
                .ok();
            let mut phash_stmt = tx
                .prepare_cached("DELETE FROM file_phashes WHERE file_id = ?1")
                .ok();
            for id in ids {
                thumb_stmt.execute(params![id]).map_err(|e| e.to_string())?;
                if let Some(ref mut s) = hash_stmt {
                    let _ = s.execute(params![id]);
                }
                if let Some(ref mut s) = phash_stmt {
                    let _ = s.execute(params![id]);
                }
                deleted += file_stmt.execute(params![id]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        if deleted > 0 {
            invalidate_embed_matrix_for_current_db();
        }
        Ok(deleted)
    }

    pub fn replace_moved_file(
        file_id: i64,
        replaced_file_id: i64,
        new_folder_id: i64,
    ) -> Result<usize, String> {
        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM athumbs WHERE file_id = ?1",
            params![replaced_file_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM afiles WHERE id = ?1",
            params![replaced_file_id],
        )
        .map_err(|e| e.to_string())?;
        let result = tx
            .execute(
                "UPDATE afiles SET folder_id = ?1 WHERE id = ?2",
                params![new_folder_id, file_id],
            )
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// get all file IDs for a specific album
    /// Returns a map of file path to file ID
    // pub fn get_all_ids_in_album(album_id: i64) -> Result<HashMap<String, i64>, String> {
    //     let conn = open_conn()?;
    //     let mut stmt = conn
    //         .prepare(
    //             "SELECT a.id, b.path, a.name
    //             FROM afiles a
    //             JOIN afolders b ON a.folder_id = b.id
    //             WHERE b.album_id = ?1",
    //         )
    //         .map_err(|e| e.to_string())?;

    //     let rows = stmt
    //         .query_map(params![album_id], |row| {
    //             Ok((
    //                 row.get::<_, i64>(0)?,
    //                 row.get::<_, String>(1)?,
    //                 row.get::<_, String>(2)?,
    //             ))
    //         })
    //         .map_err(|e| e.to_string())?;

    //     let mut files = HashMap::new();
    //     for row in rows {
    //         if let Ok((id, folder_path, name)) = row {
    //             let full_path = t_utils::get_file_path(&folder_path, &name);
    //             files.insert(full_path, id);
    //         }
    //     }
    //     Ok(files)
    // }

    // Helper function to build the count SQL query
    fn build_count_query() -> String {
        let base_query = "SELECT COUNT(*), SUM(a.size)
            FROM afiles a 
            LEFT JOIN afolders b ON a.folder_id = b.id
            LEFT JOIN albums c ON b.album_id = c.id";

        base_query.to_string()
    }

    // build the base SQL query
    fn build_base_query() -> String {
        String::from(
            "SELECT a.id, a.folder_id, 
                a.name, a.name_pinyin, a.size, a.file_type, a.format_label, a.created_at, a.modified_at, a.inode,
                a.taken_date,
                a.width, a.height, a.duration,
                a.is_favorite, a.rating, a.rotate, a.comments, a.has_tags,
                a.e_make, a.e_model, a.e_date_time, a.e_software, a.e_artist, a.e_copyright, a.e_description, a.e_lens_make, a.e_lens_model, a.e_exposure_bias, a.e_exposure_time, a.e_f_number, a.e_focal_length, a.e_iso_speed, a.e_flash, a.e_orientation,
                a.gps_latitude, a.gps_longitude, a.gps_altitude, a.geo_name, a.geo_admin1, a.geo_admin2, a.geo_cc,
                b.path,
                c.id AS album_id, c.name AS album_name,
                (SELECT 1 FROM athumbs t WHERE t.file_id = a.id LIMIT 1) AS has_thumbnail,
                CASE WHEN a.embeds IS NOT NULL THEN 1 ELSE 0 END AS has_embedding,
                a.has_faces,
                a.last_scan_time,
                a.content_id, a.paired_file_id, a.live_photo_type
            FROM afiles a 
            LEFT JOIN afolders b ON a.folder_id = b.id
            LEFT JOIN albums c ON b.album_id = c.id"
        )
    }

    // Function to construct `Self` from a database row
    fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: Some(row.get(0)?),
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

            e_make: row.get(19)?,
            e_model: row.get(20)?,
            e_date_time: row.get(21)?,
            e_software: row.get(22)?,
            e_artist: row.get(23)?,
            e_copyright: row.get(24)?,
            e_description: row.get(25)?,
            e_lens_make: row.get(26)?,
            e_lens_model: row.get(27)?,
            e_exposure_bias: row.get(28)?,
            e_exposure_time: row.get(29)?,
            e_f_number: row.get(30)?,
            e_focal_length: row.get(31)?,
            e_iso_speed: row.get(32)?,
            e_flash: row.get(33)?,
            e_orientation: row.get(34)?,

            gps_latitude: row.get(35)?,
            gps_longitude: row.get(36)?,
            gps_altitude: row.get(37)?,
            geo_name: row.get(38)?,
            geo_admin1: row.get(39)?,
            geo_admin2: row.get(40)?,
            geo_cc: row.get(41)?,

            // Folder path may be NULL if the folder row was removed / orphaned.
            // Fall back to empty path so one bad join does not drop the whole list.
            file_path: {
                let folder_path = row.get::<_, Option<String>>(42)?.unwrap_or_default();
                let name = row.get::<_, Option<String>>(2)?.unwrap_or_default();
                Some(t_utils::get_file_path(folder_path.as_str(), name.as_str()))
            },
            album_id: row.get(43)?,
            album_name: row.get(44)?,
            has_thumbnail: row.get::<_, Option<i64>>(45)?.map(|v| v == 1),
            has_embedding: row.get::<_, Option<i64>>(46)?.map(|v| v == 1),
            has_faces: row.get::<_, Option<i32>>(47)?,
            last_scan_time: row.get(48)?,

            content_id: row.get(49).unwrap_or(None),
            paired_file_id: row.get(50).unwrap_or(None),
            live_photo_type: row.get(51).unwrap_or(Some(0)),
        })
    }

    // query the count and sum by sql
    fn query_count_and_sum(
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> Result<(i64, i64), String> {
        let conn = open_conn()?;
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

        let result = stmt
            .query_row(params, |row| {
                let count: i64 = row.get(0)?;
                let sum: i64 = row.get(1).unwrap_or(0); // Handles NULL from SUM
                Ok((count, sum))
            })
            .map_err(|e| e.to_string())?;

        Ok(result)
    }

    /// query files by sql
    fn query_files(sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;

        let mut stmt = conn.prepare(sql).map_err(|e| {
            // Common failure after Live Photo schema change: query selects
            // content_id/paired_file_id/live_photo_type but migration did not
            // run. Surface a clear message instead of a raw SQLite error.
            let msg = e.to_string();
            if msg.contains("content_id")
                || msg.contains("paired_file_id")
                || msg.contains("live_photo_type")
            {
                format!(
                    "Database schema is missing Live Photo columns ({}). \
                     Restart the app to migrate, or re-open the library.",
                    msg
                )
            } else {
                msg
            }
        })?;

        let rows = stmt
            .query_map(params, Self::from_row)
            .map_err(|e| e.to_string())?;

        let mut files = Vec::new();
        let mut row_errors = 0usize;
        for file in rows {
            match file {
                Ok(f) => files.push(f),
                Err(e) => {
                    row_errors += 1;
                    if row_errors <= 5 {
                        eprintln!("Skipping unreadable afiles row: {}", e);
                    }
                }
            }
        }
        if row_errors > 0 {
            eprintln!(
                "query_files: skipped {} unreadable row(s); returning {}",
                row_errors,
                files.len()
            );
        }

        Ok(files)
    }

    /// fetch a file info from db by folder_id and file name
    pub fn fetch(folder_id: i64, file_path: &str) -> Result<Option<Self>, String> {
        let conn = open_conn()?;
        Self::fetch_with_conn(&conn, folder_id, file_path)
    }

    pub fn fetch_with_conn(
        conn: &Connection,
        folder_id: i64,
        file_path: &str,
    ) -> Result<Option<Self>, String> {
        let sql = format!(
            "{} WHERE a.folder_id = ?1 AND a.name = ?2",
            Self::build_base_query()
        );
        conn.query_row(
            &sql,
            params![folder_id, t_utils::get_file_name(file_path)],
            Self::from_row,
        )
        .optional()
        .map_err(|e| e.to_string())
    }

    fn scan_file_state_from_file(file: &Self) -> Result<ScanFileState, String> {
        let id = file
            .id
            .ok_or_else(|| "Indexed file is missing its database id".to_string())?;
        Ok(ScanFileState {
            id,
            modified_at: file.modified_at,
            has_thumbnail: file.has_thumbnail.unwrap_or(false),
            has_embedding: file.has_embedding.unwrap_or(false),
            orientation: file.e_orientation.unwrap_or(1) as i32,
            size: file.size,
            width: file.width.unwrap_or(0),
            height: file.height.unwrap_or(0),
            duration: file.duration.map(|duration| duration as u64),
        })
    }

    fn scan_file_index_result(
        state: &ScanFileState,
        deferred_seen_file_id: Option<i64>,
        cache_hit: bool,
    ) -> ScanFileIndexResult {
        ScanFileIndexResult {
            file_id: state.id,
            has_thumbnail: state.has_thumbnail,
            has_embedding: state.has_embedding,
            orientation: state.orientation,
            size: state.size,
            width: state.width,
            height: state.height,
            duration: state.duration,
            deferred_seen_file_id,
            cache_hit,
        }
    }

    /// Load just the fields needed by the synchronous scan decision tree.
    /// This cache is intentionally owned by one album scan and is never shared
    /// across scans, databases, or application lifetimes.
    pub(crate) fn load_scan_file_state_cache_for_album(
        album_id: i64,
    ) -> Result<ScanFileStateCache, String> {
        let conn = open_conn()?;
        Self::load_scan_file_state_cache_for_album_with_conn(&conn, album_id)
    }

    fn load_scan_file_state_cache_for_album_with_conn(
        conn: &Connection,
        album_id: i64,
    ) -> Result<ScanFileStateCache, String> {
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.folder_id, a.name, a.modified_at,
                    EXISTS(SELECT 1 FROM athumbs t WHERE t.file_id = a.id),
                    CASE WHEN a.embeds IS NOT NULL THEN 1 ELSE 0 END,
                    a.e_orientation, a.size, a.width, a.height, a.duration
                 FROM afiles a
                 INNER JOIN afolders f ON f.id = a.folder_id
                 WHERE f.album_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![album_id], |row| {
                Ok((
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    ScanFileState {
                        id: row.get(0)?,
                        modified_at: row.get(3)?,
                        has_thumbnail: row.get::<_, i64>(4)? != 0,
                        has_embedding: row.get::<_, i64>(5)? != 0,
                        orientation: row.get::<_, Option<i64>>(6)?.unwrap_or(1) as i32,
                        size: row.get(7)?,
                        width: row.get::<_, Option<u32>>(8)?.unwrap_or(0),
                        height: row.get::<_, Option<u32>>(9)?.unwrap_or(0),
                        duration: row.get::<_, Option<i64>>(10)?.map(|value| value as u64),
                    },
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut cache = ScanFileStateCache::new();
        for row in rows {
            let (folder_id, name, state) = row.map_err(|e| e.to_string())?;
            cache.insert((folder_id, name), state);
        }
        Ok(cache)
    }

    /// Bitmask: 1=image, 2=video, 4=raw, 8=Live/Motion still (live_photo_type 1/3/4).
    /// Companion Live videos (type 2) stay excluded by list queries separately.
    fn build_file_type_condition(mask: i64) -> Option<String> {
        if mask <= 0 {
            return None;
        }

        // All three traditional kinds (with or without LIVE) ≡ no type filter.
        let traditional = mask & 7;
        if traditional == 7 {
            return None;
        }

        let mut conditions = Vec::new();
        if mask & 1 == 1 {
            conditions.push("a.file_type = 1".to_string());
        }
        if mask & 2 == 2 {
            conditions.push("a.file_type = 2".to_string());
        }
        if mask & 4 == 4 {
            conditions.push("a.file_type = 3".to_string());
        }
        if mask & 8 == 8 {
            // Apple Live still, Google Motion Photo, HEIC-internal video still.
            conditions.push("COALESCE(a.live_photo_type, 0) IN (1, 3, 4)".to_string());
        }

        if conditions.is_empty() {
            None
        } else {
            Some(format!("({})", conditions.join(" OR ")))
        }
    }

    /// insert a file into db if not exists
    /// Returns (file, status)
    /// status: 0 - existing, 1 - new, 2 - updated
    pub fn add_to_db(
        folder_id: i64,
        file_path: &str,
        file_type: i64,
        last_scan_time: i64,
    ) -> Result<(Self, i32), String> {
        let mut profile = AFileAddProfile::default();
        Self::add_to_db_profiled_inner(
            folder_id,
            file_path,
            file_type,
            last_scan_time,
            false,
            &mut profile,
        )
    }

    /// Scan through a lightweight, per-album state cache when possible. Cache
    /// misses and rows needing metadata refresh deliberately reuse the existing
    /// database path so inserts and concurrent-insert recovery retain their
    /// established behavior.
    pub(crate) fn add_to_db_for_scan_with_state_cache(
        folder_id: i64,
        file_path: &str,
        file_type: i64,
        last_scan_time: i64,
        cache: &mut ScanFileStateCache,
        profile: &mut AFileAddProfile,
    ) -> Result<ScanFileIndexResult, String> {
        let name = t_utils::get_file_name(file_path);
        let key = (folder_id, name);
        if let Some(existing) = cache.get(&key).cloned() {
            let stat_started = profile.started();
            let file_info = t_utils::FileInfo::new(file_path);
            profile.stat_elapsed += AFileAddProfile::elapsed(stat_started);
            let file_info = file_info?;
            let modified = existing.modified_at != file_info.modified;
            let missing_thumb = !existing.has_thumbnail;

            if modified || missing_thumb {
                let refresh_started = profile.started();
                let updated_file = Self::update_file_info(existing.id, file_path, last_scan_time);
                profile.refresh_elapsed += AFileAddProfile::elapsed(refresh_started);
                if let Some(mut updated_file) = updated_file? {
                    let write_started = profile.started();
                    let _ = AThumb::delete(existing.id);
                    if modified {
                        let conn = open_conn()?;
                        let _ = conn.execute(
                            "UPDATE afiles SET embeds = NULL WHERE id = ?1",
                            params![existing.id],
                        );
                        invalidate_embed_matrix_for_current_db();
                        updated_file.has_embedding = Some(false);
                    }
                    profile.write_elapsed += AFileAddProfile::elapsed(write_started);

                    let refreshed = Self::scan_file_state_from_file(&updated_file)?;
                    cache.insert(key, refreshed.clone());
                    return Ok(Self::scan_file_index_result(&refreshed, None, true));
                }
            } else {
                profile.deferred_seen_file_id = Some(existing.id);
            }

            return Ok(Self::scan_file_index_result(
                &existing,
                profile.deferred_seen_file_id,
                true,
            ));
        }

        let (file, _) = Self::add_to_db_profiled_inner(
            folder_id,
            file_path,
            file_type,
            last_scan_time,
            true,
            profile,
        )?;
        let state = Self::scan_file_state_from_file(&file)?;
        cache.insert(key, state.clone());
        Ok(Self::scan_file_index_result(
            &state,
            profile.deferred_seen_file_id,
            false,
        ))
    }

    fn add_to_db_profiled_inner(
        folder_id: i64,
        file_path: &str,
        file_type: i64,
        last_scan_time: i64,
        defer_seen_write: bool,
        profile: &mut AFileAddProfile,
    ) -> Result<(Self, i32), String> {
        // Check if the file exists
        let fetch_started = profile.started();
        let existing_file = Self::fetch(folder_id, file_path);
        profile.fetch_elapsed += AFileAddProfile::elapsed(fetch_started);
        let existing_file = existing_file?;
        if let Some(file) = existing_file {
            // check file modified time or if thumbnail is missing
            let stat_started = profile.started();
            let file_info = t_utils::FileInfo::new(file_path);
            profile.stat_elapsed += AFileAddProfile::elapsed(stat_started);
            let file_info = file_info?;
            let modified = file.modified_at != file_info.modified;
            let missing_thumb = !file.has_thumbnail.unwrap_or(false);

            if modified || missing_thumb {
                if let Some(file_id) = file.id {
                    let refresh_started = profile.started();
                    let updated_file = Self::update_file_info(file_id, file_path, last_scan_time);
                    profile.refresh_elapsed += AFileAddProfile::elapsed(refresh_started);
                    if let Some(mut updated_file) = updated_file? {
                        // If modified, delete old thumbnail and remove embeds data
                        let write_started = profile.started();
                        if modified || missing_thumb {
                            let _ = AThumb::delete(file_id);
                            // remove embeds data
                            if modified {
                                let conn = open_conn()?;
                                let _ = conn.execute(
                                    "UPDATE afiles SET embeds = NULL WHERE id = ?1",
                                    params![file_id],
                                );
                                invalidate_embed_matrix_for_current_db();
                                updated_file.has_embedding = Some(false);
                            }
                        }
                        profile.write_elapsed += AFileAddProfile::elapsed(write_started);
                        return Ok((updated_file, 2));
                    }
                } else {
                    return Err(format!(
                        "Existing DB record is missing file id, skipping '{}'",
                        file_path
                    ));
                }
            } else {
                // Not modified and thumb exists, but we still need to update last_scan_time
                // for the mark-and-sweep deletion logic.
                if let Some(file_id) = file.id {
                    if defer_seen_write {
                        profile.deferred_seen_file_id = Some(file_id);
                    } else {
                        let write_started = profile.started();
                        let _ = Self::update_column(file_id, "last_scan_time", &last_scan_time);
                        profile.write_elapsed += AFileAddProfile::elapsed(write_started);
                    }
                }
            }
            return Ok((file, 0));
        }

        // insert the new file into the database
        let metadata_started = profile.started();
        let new_file_struct = Self::new_profiled(folder_id, file_path, file_type, Some(profile));
        profile.metadata_elapsed += AFileAddProfile::elapsed(metadata_started);
        let mut new_file_struct = new_file_struct?;
        new_file_struct.last_scan_time = Some(last_scan_time);
        let write_started = profile.started();
        let inserted = new_file_struct.insert();
        profile.write_elapsed += AFileAddProfile::elapsed(write_started);
        let inserted = inserted?;

        // A concurrent folder sync or album scan may have inserted the same
        // path after the SELECT above. Re-enter the existing-file path so the
        // winning row is marked as seen by this scan and receives any required
        // metadata or thumbnail refresh.
        if inserted == 0 {
            return Self::add_to_db_profiled_inner(
                folder_id,
                file_path,
                file_type,
                last_scan_time,
                defer_seen_write,
                profile,
            );
        }

        // return the newly inserted file
        let refetch_started = profile.started();
        let new_file = Self::fetch(folder_id, file_path);
        profile.refetch_elapsed += AFileAddProfile::elapsed(refetch_started);
        let new_file = new_file?;
        new_file
            .map(|f| (f, 1))
            .ok_or_else(|| format!("Inserted file missing from DB: {}", file_path))
    }

    pub(crate) fn mark_seen_batch(file_ids: &[i64], last_scan_time: i64) -> Result<usize, String> {
        if file_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = open_conn()?;
        Self::mark_seen_batch_with_conn(&mut conn, file_ids, last_scan_time)
    }

    pub(crate) fn mark_seen_batch_with_conn(
        conn: &mut Connection,
        file_ids: &[i64],
        last_scan_time: i64,
    ) -> Result<usize, String> {
        if file_ids.is_empty() {
            return Ok(0);
        }

        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut updated = 0;
        for chunk in file_ids.chunks(500) {
            let placeholders = std::iter::repeat("?")
                .take(chunk.len())
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                "UPDATE afiles SET last_scan_time = ?1 WHERE id IN ({})",
                placeholders
            );
            let values = std::iter::once(&last_scan_time as &dyn ToSql)
                .chain(chunk.iter().map(|file_id| file_id as &dyn ToSql));
            updated += tx
                .execute(&query, params_from_iter(values))
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(updated)
    }

    /// True when `file_path` is already indexed (folder path + basename match).
    pub fn exists_by_path(file_path: &str) -> Result<bool, String> {
        let path = Path::new(file_path);
        let parent = path
            .parent()
            .and_then(|p| p.to_str())
            .filter(|p| !p.is_empty())
            .ok_or_else(|| format!("Invalid file path: {}", file_path))?;
        let Some(folder) = AFolder::fetch(parent)? else {
            return Ok(false);
        };
        let Some(folder_id) = folder.id else {
            return Ok(false);
        };
        Ok(Self::fetch(folder_id, file_path)?.is_some())
    }

    /// get a file info from db by file_id
    pub fn get_file_info(file_id: i64) -> Result<Option<Self>, String> {
        let conn = open_conn()?;

        Self::get_file_info_with_conn(&conn, file_id)
    }

    /// Resolve a file against a specific library database. This is required
    /// for protocol requests that can outlive a frontend library switch.
    pub fn get_file_info_for_library(
        file_id: i64,
        library_id: &str,
    ) -> Result<Option<Self>, String> {
        let conn = open_conn_for_library(library_id)?;
        Self::get_file_info_with_conn(&conn, file_id)
    }

    fn get_file_info_with_conn(conn: &Connection, file_id: i64) -> Result<Option<Self>, String> {
        // Prepare the SQL query using the base query and adding the condition for file ID
        let sql = format!("{} WHERE a.id = ?1", Self::build_base_query());

        // Execute the query with file_id as the parameter
        let result = conn
            .query_row(&sql, params![file_id], Self::from_row)
            .optional()
            .map_err(|e| e.to_string())?;

        Ok(result)
    }

    /// update a file info
    pub fn update_file_info(
        file_id: i64,
        file_path: &str,
        last_scan_time: i64,
    ) -> Result<Option<Self>, String> {
        // get old file info
        let old_file_info =
            Self::get_file_info(file_id)?.ok_or_else(|| "File not found".to_string())?;

        // create a new file info
        let mut new_file_info = Self::new(
            old_file_info.folder_id,
            file_path,
            old_file_info.file_type.unwrap_or(0),
        )?;
        new_file_info.id = Some(file_id);
        new_file_info.is_favorite = old_file_info.is_favorite;
        new_file_info.rating = old_file_info.rating;
        new_file_info.rotate = old_file_info.rotate;
        // Preserve non-empty user comments; allow AI prompt fill only when empty.
        let old_comments_empty = old_file_info
            .comments
            .as_ref()
            .map(|c| c.trim().is_empty())
            .unwrap_or(true);
        let imported_comments = new_file_info.comments.clone();
        new_file_info.comments = if old_comments_empty {
            imported_comments.clone()
        } else {
            old_file_info.comments.clone()
        };
        new_file_info.has_tags = old_file_info.has_tags;
        new_file_info.has_thumbnail = old_file_info.has_thumbnail;
        new_file_info.has_embedding = old_file_info.has_embedding;
        new_file_info.last_scan_time = Some(last_scan_time);

        // update the file info
        Self::update(file_id, &new_file_info)?;

        // `update()` does not write comments; fill empty comments via column update.
        if old_comments_empty {
            if let Some(ref prompt) = imported_comments {
                if !prompt.trim().is_empty() {
                    let _ = Self::update_column(file_id, "comments", prompt);
                }
            }
        }

        Self::get_file_info(file_id)
    }

    /// update a file column value
    pub fn update_column(
        file_id: i64,
        column: &str,
        value: &dyn rusqlite::ToSql,
    ) -> Result<usize, String> {
        let conn = open_conn()?;
        Self::update_column_with_conn(&conn, file_id, column, value)
    }

    pub fn update_column_with_conn(
        conn: &Connection,
        file_id: i64,
        column: &str,
        value: &dyn rusqlite::ToSql,
    ) -> Result<usize, String> {
        assert_allowed_column(
            column,
            &[
                "name",
                "name_pinyin",
                "comments",
                "is_favorite",
                "rating",
                "rotate",
                "folder_id",
                "modified_at",
                "inode",
                "last_scan_time",
                "live_photo_type",
                "content_id",
                "paired_file_id",
            ],
        )?;
        let query = format!("UPDATE afiles SET {} = ?1 WHERE id = ?2", column);
        conn.execute(&query, params![value, file_id])
            .map_err(|e| e.to_string())
    }

    pub fn batch_update_metadata(
        file_ids: &[i64],
        is_favorite: Option<bool>,
        rating: Option<i32>,
        rotate_delta: Option<i32>,
        comment: Option<&str>,
    ) -> Result<usize, String> {
        if file_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut updated = 0;

        if let Some(value) = is_favorite {
            let mut stmt = tx
                .prepare_cached("UPDATE afiles SET is_favorite = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                updated += stmt
                    .execute(params![value, file_id])
                    .map_err(|e| e.to_string())?;
            }
        }
        if let Some(value) = rating {
            let clamped = value.clamp(0, 5);
            let mut stmt = tx
                .prepare_cached("UPDATE afiles SET rating = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                updated += stmt
                    .execute(params![clamped, file_id])
                    .map_err(|e| e.to_string())?;
            }
        }
        if let Some(value) = rotate_delta {
            let mut stmt = tx
                .prepare_cached(
                    "UPDATE afiles
                     SET rotate = ((COALESCE(rotate, 0) + ?1) % 360 + 360) % 360
                     WHERE id = ?2",
                )
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                updated += stmt
                    .execute(params![value, file_id])
                    .map_err(|e| e.to_string())?;
            }
        }
        if let Some(value) = comment {
            let mut stmt = tx
                .prepare_cached("UPDATE afiles SET comments = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                updated += stmt
                    .execute(params![value, file_id])
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(updated)
    }

    /// delete unseen files in an album (database only)
    pub fn delete_unseen_in_album(album_id: i64, current_scan_time: i64) -> Result<usize, String> {
        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let query = "DELETE FROM afiles 
            WHERE last_scan_time < ?1 
            AND folder_id IN (SELECT id FROM afolders WHERE album_id = ?2)";
        let result = tx
            .execute(query, params![current_scan_time, album_id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Pair Live Photo files by content_id within an album.
    /// Matches Apple Live Photo images (live_photo_type=1) with their
    /// companion MOV videos (live_photo_type=2) sharing the same content_id.
    /// Also pairs by file name stem as a fallback (e.g. IMG_1234.HEIC + IMG_1234.MOV).
    /// Returns the number of pairs updated.
    pub fn pair_live_photos(album_id: i64) -> Result<usize, String> {
        let mut conn = open_conn()?;
        // Ensure columns exist before pairing (covers DBs that skipped migrate).
        crate::t_migration::ensure_live_photo_columns(&conn)?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let mut updated = 0usize;

        // --- Strategy 1: Pair by Apple ContentIdentifier (content_id) ---
        // Find images (live_photo_type=1) and videos (live_photo_type=2) with
        // matching content_id that aren't already paired.
        let sql = "SELECT a.id, b.id
            FROM afiles a
            JOIN afiles b ON a.content_id = b.content_id
            JOIN afolders fa ON a.folder_id = fa.id
            JOIN afolders fb ON b.folder_id = fb.id
            WHERE fa.album_id = ?1 AND fb.album_id = ?1
              AND a.live_photo_type = 1 AND b.live_photo_type = 2
              AND a.paired_file_id IS NULL AND b.paired_file_id IS NULL";
        let mut stmt = tx.prepare(sql).map_err(|e| e.to_string())?;
        let pairs: Vec<(i64, i64)> = stmt
            .query_map(params![album_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);

        for (img_id, vid_id) in &pairs {
            tx.execute(
                "UPDATE afiles SET paired_file_id = ?1 WHERE id = ?2 AND paired_file_id IS NULL",
                params![vid_id, img_id],
            )
            .map_err(|e| e.to_string())?;
            tx.execute(
                "UPDATE afiles SET paired_file_id = ?1 WHERE id = ?2 AND paired_file_id IS NULL",
                params![img_id, vid_id],
            )
            .map_err(|e| e.to_string())?;
            updated += 1;
        }

        // --- Strategy 2: Pair by file name stem (fallback) ---
        // For images without content_id that share a file name stem with a
        // video in the same folder. e.g. IMG_1234.HEIC + IMG_1234.MOV
        let sql = "SELECT a.id, a.name, b.id, b.name
            FROM afiles a
            JOIN afiles b ON a.folder_id = b.folder_id
            JOIN afolders f ON a.folder_id = f.id
            WHERE f.album_id = ?1
              AND a.file_type IN (1, 3) AND b.file_type = 2
              AND a.paired_file_id IS NULL AND b.paired_file_id IS NULL";
        let mut stmt = tx.prepare(sql).map_err(|e| e.to_string())?;
        let candidates: Vec<(i64, String, i64, String)> = stmt
            .query_map(params![album_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);

        for (img_id, img_name, vid_id, vid_name) in &candidates {
            // Compare file name stems (without extension)
            let img_stem = Path::new(img_name).file_stem().and_then(|s| s.to_str());
            let vid_stem = Path::new(vid_name).file_stem().and_then(|s| s.to_str());
            if img_stem.is_none() || vid_stem.is_none() || img_stem != vid_stem {
                continue;
            }
            // Mark as Apple Live Photo type if not already typed
            tx.execute(
                "UPDATE afiles SET paired_file_id = ?1, live_photo_type = CASE WHEN live_photo_type > 0 THEN live_photo_type ELSE 1 END
                 WHERE id = ?2 AND paired_file_id IS NULL",
                params![vid_id, img_id],
            )
            .map_err(|e| e.to_string())?;
            tx.execute(
                "UPDATE afiles SET paired_file_id = ?1, live_photo_type = CASE WHEN live_photo_type > 0 THEN live_photo_type ELSE 2 END
                 WHERE id = ?2 AND paired_file_id IS NULL",
                params![img_id, vid_id],
            )
            .map_err(|e| e.to_string())?;
            updated += 1;
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(updated)
    }

    /// Get a file's has_tags status
    pub fn get_has_tags(file_id: i64) -> Result<bool, String> {
        let conn = open_conn()?;
        let result = conn
            .query_row(
                "SELECT has_tags FROM afiles WHERE id = ?1",
                params![file_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Exclude Apple Live Photo companion videos from normal browsing lists.
    /// Matches Lap v0.3 library browsing: type 2 is kept linked but hidden from grids.
    fn live_photo_companion_exclusion_condition() -> &'static str {
        "COALESCE(a.live_photo_type, 0) != 2"
    }

    /// get all taken dates from db
    /// `file_type_mask` matches QueryParams.search_file_type so calendar dots
    /// stay consistent with the active file-type filter in the content list.
    pub fn get_taken_dates(sort: i64, file_type_mask: i64) -> Result<Vec<(String, i64)>, String> {
        let conn = open_conn()?;

        // sort encodes both the date column and direction:
        //   sort / 2  →  0=taken_date, 1=created_at, 2=modified_at
        //   sort % 2  →  0=ASC, 1=DESC
        let sort_type = sort / 2;
        let order_clause = if sort % 2 == 0 { "ASC" } else { "DESC" };

        let date_col = match sort_type {
            0 => "a.taken_date",
            1 => "a.created_at",
            2 => "a.modified_at",
            _ => "a.taken_date",
        };

        let date_expr = format!(
            "strftime('%Y-%m-%d', {}, 'unixepoch', 'localtime')",
            date_col
        );

        // Keep the same filters as content queries so a day with dots never opens empty
        // just because calendar ignored file-type / folder-exclusion / Live companions.
        let mut conditions = vec![
            format!("{} IS NOT NULL", date_col),
            format!("{} >= 86400", date_col),
            Self::live_photo_companion_exclusion_condition().to_string(),
            Self::search_exclusion_condition("b"),
        ];
        if let Some(file_type_condition) = Self::build_file_type_condition(file_type_mask) {
            conditions.push(file_type_condition);
        }

        let query = format!(
            "SELECT {} AS group_date, COUNT(1)
            FROM afiles a
            LEFT JOIN afolders b ON a.folder_id = b.id
            WHERE {}
            GROUP BY {}
            ORDER BY group_date {}",
            date_expr,
            conditions.join(" AND "),
            date_expr,
            order_clause
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        // Use collect() to simplify result collection
        let results: Vec<(String, i64)> = stmt
            .query_map(params![], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| format!("Query execution failed: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to process rows: {}", e))?;

        Ok(results)
    }

    // get total count and size of files
    pub fn get_total_count_and_sum() -> Result<(i64, i64), String> {
        let sql = format!(
            "{} WHERE {} AND {}",
            Self::build_count_query(),
            Self::search_exclusion_condition("b"),
            Self::live_photo_companion_exclusion_condition()
        );
        Self::query_count_and_sum(&sql, &[])
    }

    // helper to build search query conditions and params
    // Returns (joins_clause, where_clause, params)
    fn build_search_query_parts(params: &QueryParams) -> (String, String, Vec<Box<dyn ToSql>>) {
        let mut joins = Vec::new();
        // Always hide Live companion videos in normal query lists (Lap-compatible).
        let mut conditions: Vec<String> =
            vec![Self::live_photo_companion_exclusion_condition().to_string()];
        let mut sql_params: Vec<Box<dyn ToSql>> = Vec::new();

        if !params.search_file_name.is_empty() {
            conditions.push("a.name LIKE ? COLLATE NOCASE".to_string());
            sql_params.push(Box::new(format!("%{}%", params.search_file_name)));
        }

        if let Some(condition) = Self::build_file_type_condition(params.search_file_type) {
            conditions.push(condition);
        }

        if !params.search_all_subfolders.is_empty() {
            // Match path that starts with search_folder followed by '/' or end of string
            conditions.push("(b.path = ? OR b.path LIKE ?)".to_string());
            sql_params.push(Box::new(params.search_all_subfolders.clone()));
            sql_params.push(Box::new(format!(
                "{}{}%",
                params.search_all_subfolders,
                std::path::MAIN_SEPARATOR
            )));
        }

        if !params.search_folder.is_empty() {
            conditions.push("(b.path = ?)".to_string());
            sql_params.push(Box::new(params.search_folder.clone()));
        }

        if params.start_date > 0 && params.end_date > 0 {
            let date_col = match params.calendar_sort / 2 {
                0 => "a.taken_date",
                1 => "a.created_at",
                2 => "a.modified_at",
                _ => "a.taken_date",
            };
            // Compare by local calendar day (same expression as get_taken_dates), not raw
            // unix-second ranges. This avoids JS timezone / float boundary mismatches that
            // produced "dots with counts but empty content list".
            conditions.push(format!(
                "strftime('%Y-%m-%d', {0}, 'unixepoch', 'localtime') \
                 >= strftime('%Y-%m-%d', ?, 'unixepoch', 'localtime') \
                 AND strftime('%Y-%m-%d', {0}, 'unixepoch', 'localtime') \
                 < strftime('%Y-%m-%d', ?, 'unixepoch', 'localtime')",
                date_col
            ));
            sql_params.push(Box::new(params.start_date));
            sql_params.push(Box::new(params.end_date));
        } else if params.start_date == -1 && params.end_date == -1 {
            // "On This Day" feature: find all photos taken on the same month and day as today
            let now = chrono::Local::now();
            let today_month_day = now.format("%m-%d").to_string();
            let date_col = match params.calendar_sort / 2 {
                0 => "a.taken_date",
                1 => "a.created_at",
                2 => "a.modified_at",
                _ => "a.taken_date",
            };
            conditions.push(format!(
                "strftime('%m-%d', {}, 'unixepoch', 'localtime') = ?",
                date_col
            ));
            sql_params.push(Box::new(today_month_day));
        }

        if !params.make.is_empty() {
            conditions.push("UPPER(a.e_make) = UPPER(?)".to_string());
            sql_params.push(Box::new(params.make.clone()));
            if !params.model.is_empty() {
                conditions.push("a.e_model = ?".to_string());
                sql_params.push(Box::new(params.model.clone()));
            }
        }

        if !params.lens_make.is_empty() {
            conditions.push("UPPER(a.e_lens_make) = UPPER(?)".to_string());
            sql_params.push(Box::new(params.lens_make.clone()));
            if !params.lens_model.is_empty() {
                conditions.push("a.e_lens_model = ?".to_string());
                sql_params.push(Box::new(params.lens_model.clone()));
            }
        }

        if !params.location_admin1.is_empty() {
            conditions.push("a.geo_admin1 = ?".to_string());
            sql_params.push(Box::new(params.location_admin1.clone()));
            if !params.location_name.is_empty() {
                conditions.push("a.geo_name = ?".to_string());
                sql_params.push(Box::new(params.location_name.clone()));
            }
        }

        if let (Some(min_lat), Some(max_lat), Some(min_lon), Some(max_lon)) = (
            params.gps_min_lat,
            params.gps_max_lat,
            params.gps_min_lon,
            params.gps_max_lon,
        ) {
            conditions.push("a.gps_latitude BETWEEN ? AND ?".to_string());
            sql_params.push(Box::new(min_lat));
            sql_params.push(Box::new(max_lat));

            if min_lon <= max_lon {
                conditions.push("a.gps_longitude BETWEEN ? AND ?".to_string());
                sql_params.push(Box::new(min_lon));
                sql_params.push(Box::new(max_lon));
            } else {
                // map view crosses the antimeridian (e.g. min=170, max=-170)
                conditions.push("(a.gps_longitude >= ? OR a.gps_longitude <= ?)".to_string());
                sql_params.push(Box::new(min_lon));
                sql_params.push(Box::new(max_lon));
            }
        }

        if params.is_favorite {
            conditions.push("a.is_favorite = 1".to_string());
        }

        if params.rating == 0 {
            conditions.push("(a.rating = 0 OR a.rating IS NULL)".to_string());
        } else if params.rating > 0 {
            conditions.push("a.rating = ?".to_string());
            sql_params.push(Box::new(params.rating));
        }

        if params.tag_id > 0 {
            joins.push("INNER JOIN afile_tags at ON a.id = at.file_id");
            conditions.push("at.tag_id = ?".to_string());
            sql_params.push(Box::new(params.tag_id));
        }

        if params.person_id > 0 {
            joins.push("INNER JOIN faces f ON a.id = f.file_id");
            conditions.push("f.person_id = ?".to_string());
            sql_params.push(Box::new(params.person_id));
        }

        conditions.push(Self::search_exclusion_condition("b"));

        let joins_clause = if !joins.is_empty() {
            format!(" {}", joins.join(" "))
        } else {
            String::new()
        };

        let where_clause = if !conditions.is_empty() {
            format!(" WHERE {}", conditions.join(" AND "))
        } else {
            String::new()
        };

        (joins_clause, where_clause, sql_params)
    }

    // get query count and sum
    pub fn get_query_count_and_sum(params: &QueryParams) -> Result<(i64, i64), String> {
        let (joins, where_clause, sql_params) = Self::build_search_query_parts(params);

        let sql = if params.person_id > 0 {
            // Use subquery with GROUP BY to handle potential duplicate rows when joining faces
            format!(
                "SELECT COUNT(*), SUM(size) FROM (SELECT a.id, a.size FROM afiles a 
                LEFT JOIN afolders b ON a.folder_id = b.id 
                LEFT JOIN albums c ON b.album_id = c.id 
                {}{} GROUP BY a.id)",
                joins, where_clause
            )
        } else {
            format!("{}{}{}", Self::build_count_query(), joins, where_clause)
        };

        let final_params: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
        Self::query_count_and_sum(&sql, &final_params)
    }

    // get query files
    pub fn get_query_files(
        params: &QueryParams,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Self>, String> {
        let (joins, where_clause, sql_params) = Self::build_search_query_parts(params);

        let mut query = Self::build_base_query();
        query.push_str(&joins);
        query.push_str(&where_clause);

        // fix issues that some files have multiple identical person_ids
        if params.person_id > 0 {
            query.push_str(" GROUP BY a.id");
        }

        // sort
        query.push_str(&format!(" ORDER BY {}", Self::build_order_clause(params)));

        // paging
        query.push_str(" LIMIT ? OFFSET ?");

        let mut final_params: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
        final_params.push(&limit);
        final_params.push(&offset);
        Self::query_files(&query, &final_params)
    }

    fn build_order_clause(params: &QueryParams) -> String {
        let dir = if params.sort_order == 1 {
            "DESC"
        } else {
            "ASC"
        };
        match params.sort_type {
            0 => format!("a.taken_date {}, a.id {}", dir, dir),
            1 => format!("a.created_at {}, a.id {}", dir, dir),
            2 => format!("a.modified_at {}, a.id {}", dir, dir),
            3 => format!("a.name_pinyin {}, a.id {}", dir, dir),
            4 => format!("a.size {}, a.id {}", dir, dir),
            5 => format!("a.width {}, a.height {}, a.id {}", dir, dir, dir),
            6 => format!("a.duration {}, a.id {}", dir, dir),
            7 => format!("a.rating {}, a.id {}", dir, dir),
            8 => "RANDOM()".to_string(),
            9 => "a.id ASC".to_string(), // internal: stable append order during scanning
            _ => format!("a.taken_date {}, a.id {}", dir, dir),
        }
    }

    pub fn get_query_file_position(
        params: &QueryParams,
        file_id: i64,
    ) -> Result<Option<i64>, String> {
        if file_id <= 0 {
            return Ok(None);
        }

        let (joins, where_clause, sql_params) = Self::build_search_query_parts(params);
        let mut query = format!(
            "WITH ranked_files AS (
                SELECT
                    a.id,
                    ROW_NUMBER() OVER (ORDER BY {}) - 1 AS position
                FROM afiles a
                LEFT JOIN afolders b ON a.folder_id = b.id
                LEFT JOIN albums c ON b.album_id = c.id
                {}
                {}
                {}
            )
            SELECT position FROM ranked_files WHERE id = ?",
            Self::build_order_clause(params),
            joins,
            where_clause,
            if params.person_id > 0 {
                " GROUP BY a.id"
            } else {
                ""
            }
        );

        // Keep SQL clean when where/group are empty to avoid odd spacing.
        query = query.replace("\n                \n", "\n");

        let conn = open_conn()?;
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let mut final_params: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
        final_params.push(&file_id);

        stmt.query_row(final_params.as_slice(), |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())
    }

    // get query timeline markers
    pub fn get_query_time_line(params: &QueryParams) -> Result<Vec<ATimeLine>, String> {
        // Only process for time-based sorts (0=taken_date, 1=created_at, 2=modified_at)
        if params.sort_type > 2 {
            return Ok(Vec::new());
        }

        let (joins, where_clause, sql_params) = Self::build_search_query_parts(params);

        // Determine date field and extraction logic based on sort_type
        let (date_field, year_extract, month_extract, date_extract) = match params.sort_type {
            0 => (
                "a.taken_date",
                "CAST(strftime('%Y', a.taken_date, 'unixepoch', 'localtime') AS INTEGER)",
                "CAST(strftime('%m', a.taken_date, 'unixepoch', 'localtime') AS INTEGER)",
                "CAST(strftime('%d', a.taken_date, 'unixepoch', 'localtime') AS INTEGER)",
            ),
            1 => (
                "a.created_at",
                "CAST(strftime('%Y', a.created_at, 'unixepoch', 'localtime') AS INTEGER)",
                "CAST(strftime('%m', a.created_at, 'unixepoch', 'localtime') AS INTEGER)",
                "CAST(strftime('%d', a.created_at, 'unixepoch', 'localtime') AS INTEGER)",
            ),
            2 => (
                "a.modified_at",
                "CAST(strftime('%Y', a.modified_at, 'unixepoch', 'localtime') AS INTEGER)",
                "CAST(strftime('%m', a.modified_at, 'unixepoch', 'localtime') AS INTEGER)",
                "CAST(strftime('%d', a.modified_at, 'unixepoch', 'localtime') AS INTEGER)",
            ),
            _ => unreachable!(),
        };

        let order_clause = if params.sort_order == 0 {
            "ASC"
        } else {
            "DESC"
        };

        // Build query with ROW_NUMBER to calculate positions
        let query = format!(
            "WITH ranked_files AS (
                SELECT 
                    ROW_NUMBER() OVER (ORDER BY {} {}) - 1 AS position,
                    {} AS year,
                    {} AS month,
                    {} AS date
                FROM afiles a
                LEFT JOIN afolders b ON a.folder_id = b.id
                {}
                {}
            )
            SELECT year, month, date, MIN(position) as position
            FROM ranked_files
            WHERE year IS NOT NULL
            GROUP BY year, month, date
            ORDER BY position ASC",
            date_field,
            order_clause,
            year_extract,
            month_extract,
            date_extract,
            joins,
            where_clause
        );

        let conn = open_conn()?;
        let final_params: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

        let timelines = stmt
            .query_map(final_params.as_slice(), |row| {
                Ok(ATimeLine {
                    year: row.get(0)?,
                    month: row.get(1)?,
                    date: row.get(2)?,
                    position: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(timelines)
    }

    // get all files in a folder by folder id (DB only)
    pub fn get_files_by_folder_id(folder_id: i64) -> Result<Vec<Self>, String> {
        let sql = format!(
            "{} WHERE a.folder_id = ?1 ORDER BY a.name ASC",
            Self::build_base_query()
        );
        Self::query_files(&sql, &[&folder_id])
    }

    // --- AI Logic ---

    /// check ai status
    pub fn check_ai_status(state: &State<t_ai::AiState>) -> String {
        let engine = t_common::lock_mutex(&state.0);
        if engine.is_loaded() {
            "AI Models Loaded".to_string()
        } else {
            "AI Engine Initialized (Models Not Loaded)".to_string()
        }
    }

    /// get query embedding from search text or similar image id
    pub fn get_query_embedding(
        state: &State<t_ai::AiState>,
        params: &ImageSearchParams,
    ) -> Result<Option<Vec<f32>>, String> {
        if !params.search_text.is_empty() {
            // encode_text applies short-label CLIP template (a photo of a …).
            let mut engine = t_common::lock_mutex(&state.0);
            Ok(Some(engine.encode_text(&params.search_text)?))
        } else if let Some(file_id) = params.file_id.filter(|&id| id > 0) {
            match Self::get_embedding_by_id(file_id) {
                Ok(emb) => Ok(Some(emb)),
                Err(_) => {
                    Self::generate_embedding(state, file_id)?;
                    Ok(Some(Self::get_embedding_by_id(file_id)?))
                }
            }
        } else {
            Ok(None)
        }
    }

    /// generate embedding for a file
    pub fn generate_embedding(
        state: &State<t_ai::AiState>,
        file_id: i64,
    ) -> Result<String, String> {
        // 1. Fetch file info to get path
        let file_opt = Self::get_file_info(file_id).map_err(|e| e.to_string())?;
        let file = file_opt.ok_or("File not found")?;

        // 2. Check if it's an image
        // file_type: 1 = normal image (JPEG/PNG/…), 3 = RAW (RW2/CR2/…)
        // (HEIC is also type 1 with special decode paths elsewhere.)
        let file_type = file.file_type.unwrap_or(0);
        if file_type != 1 && file_type != 3 {
            return Err("File is not an image".to_string());
        }

        let file_path = file.file_path.ok_or("File path not resolved")?;
        let orientation = file.e_orientation.unwrap_or(1) as i32;

        // 3. Check if embedding exists
        if let Ok(embeds) = Self::get_embedding_by_id(file_id) {
            if !embeds.is_empty() {
                return Ok("Embedding already exists".to_string());
            }
        }

        // 4. Decode/I/O **outside** AiEngine mutex so concurrent embed tasks can
        // read/decode in parallel; only ONNX forward holds the lock.
        // Quality ladder:
        //   RAW  → LibRaw preview @ EMBED_SOURCE_MAX_EDGE
        //   JPEG → libjpeg-turbo scaled decode
        //   else → open + longest-edge cap
        //   last → stored UI thumbnail
        let edge = crate::t_common::EMBED_SOURCE_MAX_EDGE;
        let prepared = match panic::catch_unwind(AssertUnwindSafe(|| {
            t_image::load_image_for_clip_embed(&file_path, file_type, orientation)
        })) {
            Ok(Ok(pair)) => Some(pair),
            Ok(Err(e)) => {
                eprintln!("embed prepare failed file_id={file_id}: {e}");
                None
            }
            Err(_) => {
                eprintln!("embed prepare panicked file_id={file_id}");
                None
            }
        };

        let ui_thumb_bytes: Option<Vec<u8>> = if prepared.is_none() {
            AThumb::fetch(file_id)
                .ok()
                .flatten()
                .and_then(|t| t.thumb_data)
        } else {
            None
        };

        let mut engine = t_common::lock_mutex(&state.0);
        let (embedding, source) = if let Some((img, src)) = prepared {
            match panic::catch_unwind(AssertUnwindSafe(|| engine.encode_image_from_dynamic(img))) {
                Ok(Ok(emb)) => (emb, src),
                Ok(Err(enc_err)) => {
                    let bytes = ui_thumb_bytes.or_else(|| {
                        AThumb::fetch(file_id)
                            .ok()
                            .flatten()
                            .and_then(|t| t.thumb_data)
                    });
                    match bytes {
                        Some(b) => match panic::catch_unwind(AssertUnwindSafe(|| {
                            engine.encode_image_from_bytes(&b)
                        })) {
                            Ok(Ok(emb)) => {
                                if embed_file_trace_enabled() {
                                    println!(
                                        "embed file_id={file_id} used=thumbnail (encode failed: {enc_err})"
                                    );
                                }
                                (emb, "thumbnail")
                            }
                            Ok(Err(t_err)) => {
                                return Err(format!(
                                    "encode failed ({enc_err}); thumbnail failed: {t_err}"
                                ));
                            }
                            Err(_) => {
                                return Err(format!(
                                    "encode failed ({enc_err}); thumbnail encode panicked"
                                ));
                            }
                        },
                        None => return Err(enc_err),
                    }
                }
                Err(_) => {
                    return Err(format!(
                        "encode_image_from_dynamic panicked for file_id={file_id}"
                    ));
                }
            }
        } else {
            let bytes = ui_thumb_bytes.or_else(|| {
                AThumb::fetch(file_id)
                    .ok()
                    .flatten()
                    .and_then(|t| t.thumb_data)
            });
            match bytes {
                Some(b) => match panic::catch_unwind(AssertUnwindSafe(|| {
                    engine.encode_image_from_bytes(&b)
                })) {
                    Ok(Ok(emb)) => {
                        if embed_file_trace_enabled() {
                            println!(
                                "embed file_id={file_id} used=thumbnail (prepare failed; edge={edge})"
                            );
                        }
                        (emb, "thumbnail")
                    }
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        return Err(format!("thumbnail encode panicked for file_id={file_id}"));
                    }
                },
                None => {
                    return Err(format!(
                        "embed prepare failed and no UI thumbnail for file_id={file_id}"
                    ));
                }
            }
        };
        drop(engine);

        if embed_file_trace_enabled() {
            println!("embed file_id={file_id} used={source} edge={edge}");
        }

        // 5. Save to DB
        let _ =
            Self::update_embedding(file_id, embedding).map_err(|e| format!("DB Error: {}", e))?;

        Ok("Embedding generated and saved".to_string())
    }

    /// Decode and preprocess one batch without holding the AI engine mutex.
    pub(crate) fn prepare_embeddings_batch(
        sources: Vec<EmbeddingBatchSource>,
    ) -> PreparedEmbeddingBatch {
        let prepare_started = Instant::now();
        let mut prepared = Vec::new();
        for source in sources {
            if !matches!(source.file_type, 1 | 3) {
                continue;
            }
            if let Ok((img, _)) = t_image::load_image_for_clip_embed(
                &source.file_path,
                source.file_type,
                source.orientation,
            ) {
                prepared.push((source.file_id, img));
            }
        }
        let prepare_elapsed = prepare_started.elapsed();
        if prepared.is_empty() {
            return PreparedEmbeddingBatch {
                prepare_elapsed,
                ..PreparedEmbeddingBatch::default()
            };
        }
        let ids: Vec<i64> = prepared.iter().map(|(id, _)| *id).collect();
        let images: Vec<image::DynamicImage> = prepared.into_iter().map(|(_, img)| img).collect();
        let preprocess_started = Instant::now();
        let preprocessed = t_ai::AiEngine::preprocess_dynamic_images(images);
        let preprocess_elapsed = preprocess_started.elapsed();
        match preprocessed {
            Ok(image_input) => PreparedEmbeddingBatch {
                ids,
                image_input: Some(image_input),
                prepare_elapsed,
                preprocess_elapsed,
                ..PreparedEmbeddingBatch::default()
            },
            Err(error) => PreparedEmbeddingBatch {
                ids,
                preprocess_error: Some(error),
                prepare_elapsed,
                preprocess_elapsed,
                ..PreparedEmbeddingBatch::default()
            },
        }
    }

    /// Run and persist a preprocessed dynamic batch through the single ONNX session.
    pub(crate) fn generate_prepared_embeddings_batch(
        state: &State<t_ai::AiState>,
        prepared: PreparedEmbeddingBatch,
    ) -> EmbeddingBatchOutcome {
        let PreparedEmbeddingBatch {
            ids,
            image_input,
            preprocess_error,
            prepare_elapsed,
            preprocess_elapsed,
        } = prepared;
        let prepared_items = ids.len();
        if prepared_items == 0 {
            return EmbeddingBatchOutcome {
                prepare_elapsed,
                preprocess_elapsed,
                ..EmbeddingBatchOutcome::default()
            };
        }
        let engine_started = Instant::now();
        let encoding = match (image_input, preprocess_error) {
            (Some(image_input), _) => {
                let mut engine = t_common::lock_mutex(&state.0);
                engine.encode_preprocessed_images_profiled(image_input, prepared_items)
            }
            (None, Some(error)) => Err(error),
            (None, None) => Err("Embedding batch has no preprocessed input".to_string()),
        };
        let locked_engine_elapsed = engine_started.elapsed();
        let engine_elapsed = preprocess_elapsed.saturating_add(locked_engine_elapsed);
        let (embeddings, inference_elapsed) = match encoding {
            Ok((embeddings, inference_elapsed)) => (Ok(embeddings), inference_elapsed),
            Err(error) => (Err(error), Duration::default()),
        };
        let (results, write_elapsed) = match embeddings {
            Ok(values) => {
                let write_started = Instant::now();
                let embeddings: Vec<(i64, Vec<f32>)> = ids.into_iter().zip(values).collect();
                let results = match Self::update_embeddings_batch(&embeddings) {
                    Ok(_) => embeddings
                        .into_iter()
                        .map(|(file_id, _)| (file_id, Ok(())))
                        .collect(),
                    Err(error) => embeddings
                        .into_iter()
                        .map(|(file_id, _)| (file_id, Err(error.clone())))
                        .collect(),
                };
                (results, write_started.elapsed())
            }
            Err(error) => (
                ids.into_iter()
                    .map(|file_id| (file_id, Err(error.clone())))
                    .collect(),
                Duration::default(),
            ),
        };
        EmbeddingBatchOutcome {
            results,
            prepared_items,
            prepare_elapsed,
            engine_elapsed,
            preprocess_elapsed,
            inference_elapsed,
            write_elapsed,
        }
    }

    /// Update embedding for a file
    pub fn update_embedding(file_id: i64, embedding: Vec<f32>) -> Result<usize, String> {
        let mut conn = open_conn()?;
        let result = Self::update_embeddings_with_conn(&mut conn, &[(file_id, embedding)])?;
        if result > 0 {
            invalidate_embed_matrix_for_current_db();
        }
        Ok(result)
    }

    /// Persist a completed inference batch atomically. A batch DB failure is
    /// returned to the scan worker, which preserves its existing per-file
    /// fallback path instead of reporting partial success.
    fn update_embeddings_batch(embeddings: &[(i64, Vec<f32>)]) -> Result<usize, String> {
        if embeddings.is_empty() {
            return Ok(0);
        }
        let mut conn = open_conn()?;
        let updated = Self::update_embeddings_with_conn(&mut conn, embeddings)?;
        if updated > 0 {
            invalidate_embed_matrix_for_current_db();
        }
        Ok(updated)
    }

    fn update_embeddings_with_conn(
        conn: &mut Connection,
        embeddings: &[(i64, Vec<f32>)],
    ) -> Result<usize, String> {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut stmt = tx
            .prepare_cached("UPDATE afiles SET embeds = ?1 WHERE id = ?2")
            .map_err(|e| e.to_string())?;
        let mut updated = 0;
        for (file_id, embedding) in embeddings {
            let mut bytes = Vec::with_capacity(embedding.len() * 4);
            for value in embedding {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            updated += stmt
                .execute(params![bytes, file_id])
                .map_err(|e| e.to_string())?;
        }
        drop(stmt);
        tx.commit().map_err(|e| e.to_string())?;
        Ok(updated)
    }

    pub fn get_embedding_by_id(file_id: i64) -> Result<Vec<f32>, String> {
        let conn = open_conn()?;
        let embeds_blob: Vec<u8> = conn
            .query_row(
                "SELECT embeds FROM afiles WHERE id = ?1 AND embeds IS NOT NULL",
                params![file_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Image embedding not found".to_string())?;

        let embedding: Vec<f32> = embeds_blob
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        Ok(embedding)
    }

    /// search similar images
    pub fn search_similar_images(
        state: &State<t_ai::AiState>,
        params: ImageSearchParams,
    ) -> Result<Vec<Self>, String> {
        // 1. Determine Target Embedding
        let embedding_opt = Self::get_query_embedding(state, &params)?;
        let embedding =
            embedding_opt.ok_or_else(|| "No file_id or search_text provided".to_string())?;

        // Precompute query norm once for blob cosine (avoids per-candidate sqrt on query).
        let query_norm_sq: f32 = embedding.iter().map(|x| x * x).sum();
        if query_norm_sq <= 0.0 {
            return Ok(Vec::new());
        }
        let query_norm = query_norm_sq.sqrt();

        // 2. Vector search — prefer in-memory embed matrix; SQL blob stream as fallback.
        let conn = open_conn()?;

        // Similarity slider (settings_thr) owns the primary cut.
        // Text→image CLIP band ~0.18–0.30; image→image is much higher (~0.55–0.95).
        //   1) absolute floor: text uses max(0.16, thr*0.85); similar-from-file uses image ladder
        //   2) rank by cosine desc
        //   3) top-K: user limit hard cap ∩ thr_cap (soft max 200)
        //   4) relative floor (top1*0.85) only if absolute cut emptied a non-empty candidate set
        //   5) exclude query file itself on similar-from-file (score≈1.0 otherwise masks the slider)
        let settings_thr = if params.threshold > 0.0 {
            params.threshold
        } else {
            0.20 // align with calibrated Medium when caller omits thr
        };
        let is_image_query =
            params.search_text.is_empty() && params.file_id.filter(|&id| id > 0).is_some();
        let exclude_id = if is_image_query {
            params.file_id.filter(|&id| id > 0)
        } else {
            None
        };
        let absolute_floor = if is_image_query {
            image_image_absolute_floor(settings_thr)
        } else {
            (settings_thr * 0.85).max(0.16)
        };

        let mut scores: Vec<(i64, f32)> = Vec::new();
        let mut candidates: u32 = 0;
        let mut max_score = f32::NEG_INFINITY;
        let mut band_gt = [0u32; 5]; // >0.18, >0.22, >0.28, >0.34, >0.40
        // 0=sql blob, 1=exact matrix, 2=ann+exact rerank
        let mut matrix_flag: u8 = 0;

        // MVP cache: all embeds + search exclusions. File-type filter → SQL blob path.
        let matrix_opt = if params.search_file_type == 0 {
            get_or_load_embed_matrix(&conn)
                .ok()
                .flatten()
                .filter(|m| m.dim == embedding.len() && m.dim > 0)
        } else {
            None
        };

        if let Some(matrix) = matrix_opt {
            candidates = matrix.ids.len() as u32;
            let ann = get_or_build_embed_ann(&matrix);
            let (scored, max_s, bands, used_ann) =
                score_embed_matrix_auto(matrix.as_ref(), ann.as_deref(), &embedding, query_norm);
            matrix_flag = if used_ann { 2 } else { 1 };
            scores = scored;
            max_score = max_s;
            band_gt = bands;
        } else {
            let mut query = "SELECT a.id, a.embeds 
            FROM afiles a
            LEFT JOIN afolders b ON a.folder_id = b.id
            WHERE a.embeds IS NOT NULL"
                .to_string();

            query.push_str(" AND ");
            query.push_str(&Self::search_exclusion_condition("b"));

            if let Some(file_type_condition) =
                Self::build_file_type_condition(params.search_file_type)
            {
                query.push_str(" AND (");
                query.push_str(&file_type_condition);
                query.push(')');
            }

            let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let embeds_blob: Vec<u8> = row.get(1)?;
                    Ok((id, embeds_blob))
                })
                .map_err(|e| e.to_string())?;

            // Stream cosine over LE f32 blobs without allocating a Vec per candidate.
            for row in rows {
                let (id, embeds_blob) = row.map_err(|e| e.to_string())?;
                let score = Self::cosine_similarity_blob(&embedding, query_norm, &embeds_blob);
                candidates += 1;
                if score > max_score {
                    max_score = score;
                }
                if score > 0.18 {
                    band_gt[0] += 1;
                }
                if score > 0.22 {
                    band_gt[1] += 1;
                }
                if score > 0.28 {
                    band_gt[2] += 1;
                }
                if score > 0.34 {
                    band_gt[3] += 1;
                }
                if score > 0.40 {
                    band_gt[4] += 1;
                }
                if score >= 0.16 {
                    scores.push((id, score));
                }
            }
        }

        // Similar-from-file: drop the query image (cosine≈1.0). Keep text-search self hits.
        if let Some(ex) = exclude_id {
            scores.retain(|(id, _)| *id != ex);
            if scores.is_empty() {
                max_score = f32::NEG_INFINITY;
            } else {
                max_score = scores
                    .iter()
                    .map(|(_, s)| *s)
                    .fold(f32::NEG_INFINITY, f32::max);
            }
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Strictness → max results. User limit is a hard cap for every tier; thr_cap soft max.
        let (thr_cap, top_k) = if is_image_query {
            image_image_top_k(settings_thr, params.limit)
        } else {
            image_search_top_k(settings_thr, params.limit)
        };

        let top1 = scores.first().map(|(_, s)| *s).unwrap_or(0.0);
        // Fallback band near top1 (must be able to go *below* abs when slider emptied the list).
        let relative_floor = if top1 > 0.0 {
            if is_image_query {
                // Image-image: stay near the best match so Low doesn't dump half the library.
                (top1 * 0.92).max(absolute_floor * 0.9)
            } else {
                top1 * 0.85
            }
        } else if is_image_query {
            absolute_floor
        } else {
            0.16
        };

        // Primary cut = absolute_floor (slider). Relative / all only if abs empties non-empty scores.
        let mut ranked: Vec<(i64, f32)> = scores
            .iter()
            .copied()
            .filter(|(_, s)| *s >= absolute_floor)
            .collect();
        let mut floor_mode = "abs";
        if ranked.is_empty() && !scores.is_empty() {
            ranked = scores
                .iter()
                .copied()
                .filter(|(_, s)| *s >= relative_floor)
                .collect();
            floor_mode = "rel_fallback";
            if ranked.is_empty() {
                ranked = scores.clone();
                floor_mode = "all_fallback";
            }
        }
        let above_floor = ranked.len();
        let final_ids: Vec<i64> = ranked.iter().take(top_k).map(|(id, _)| *id).collect();
        let returned = final_ids.len();
        let top3: Vec<String> = ranked
            .iter()
            .take(3)
            .map(|(id, s)| format!("{}:{:.3}", id, s))
            .collect();
        let max_disp = if candidates == 0 || !max_score.is_finite() {
            0.0
        } else {
            max_score
        };
        // Log preview only — full prompt is encoded (token max 77). Half-line ≠ encode cut.
        let mode = if is_image_query { "image" } else { "text" };
        let q_hint = if !params.search_text.is_empty() {
            let raw = params.search_text.as_str();
            let encoded = t_ai::AiEngine::normalize_clip_text_query(raw);
            let full_chars = raw.chars().count();
            let preview: String = raw.chars().take(40).collect();
            let enc_preview: String = encoded.chars().take(48).collect();
            let templated = encoded != raw.trim();
            format!(
                "text_chars={full_chars} preview={preview:?} enc_preview={enc_preview:?} templated={templated}"
            )
        } else if let Some(fid) = params.file_id {
            format!("file_id={fid}")
        } else {
            "query=?".into()
        };
        println!(
            "search_similar mode={mode} {q_hint} matrix={matrix_flag} settings_thr={settings_thr:.3} floor={absolute_floor:.3} rel_floor={relative_floor:.3} floor_mode={floor_mode} thr_cap={thr_cap} top_k={top_k} candidates={candidates} above_floor={above_floor} returned={returned} max={max_disp:.4} >0.18={} >0.22={} >0.28={} >0.34={} >0.40={} top3=[{}]",
            band_gt[0],
            band_gt[1],
            band_gt[2],
            band_gt[3],
            band_gt[4],
            top3.join(", ")
        );

        if final_ids.is_empty() {
            return Ok(Vec::new());
        }

        // 3. Batch hydrate full rows (preserve score order)
        let by_id = Self::get_files_by_ids(&final_ids)?;
        let mut results = Vec::with_capacity(final_ids.len());
        for id in final_ids {
            if let Some(file) = by_id.get(&id) {
                results.push(file.clone());
            }
        }

        println!("Returning {} files", results.len());

        Ok(results)
    }

    /// Fetch full file rows for a list of ids, chunked IN-lists (≤500).
    /// Returned map is unordered; callers should re-order by their id list.
    pub fn get_files_by_ids(ids: &[i64]) -> Result<HashMap<i64, Self>, String> {
        let mut out: HashMap<i64, Self> = HashMap::with_capacity(ids.len());
        if ids.is_empty() {
            return Ok(out);
        }
        const CHUNK: usize = 500;
        let conn = open_conn()?;
        let base = Self::build_base_query();

        for chunk in ids.chunks(CHUNK) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("{} WHERE a.id IN ({})", base, placeholders);
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt
                .query_map(params.as_slice(), Self::from_row)
                .map_err(|e| e.to_string())?;
            for row in rows {
                let file = row.map_err(|e| e.to_string())?;
                if let Some(id) = file.id {
                    out.insert(id, file);
                }
            }
        }
        Ok(out)
    }

    /// Cosine similarity against a little-endian f32 embedding blob.
    /// Avoids allocating a full `Vec<f32>` per candidate (large-library win).
    fn cosine_similarity_blob(query: &[f32], query_norm: f32, blob: &[u8]) -> f32 {
        if query_norm <= 0.0 || blob.is_empty() {
            return 0.0;
        }
        let mut dot = 0.0f32;
        let mut file_norm_sq = 0.0f32;
        let mut i = 0usize;
        for chunk in blob.chunks_exact(4) {
            if i >= query.len() {
                break;
            }
            let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            dot += query[i] * v;
            file_norm_sq += v * v;
            i += 1;
        }
        // Dimension mismatch or empty file vector.
        if i == 0 || i != query.len() || file_norm_sq <= 0.0 {
            return 0.0;
        }
        let file_norm = file_norm_sq.sqrt();
        if file_norm <= 0.0 {
            0.0
        } else {
            dot / (query_norm * file_norm)
        }
    }

    #[allow(dead_code)]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }
}

/// User limit is a hard cap for every similarity tier. thr_cap is a soft tier max.
/// Soft global max remains 200.
fn image_search_top_k(settings_thr: f32, params_limit: i64) -> (usize, usize) {
    let thr_cap: usize = if settings_thr >= 0.27 {
        30
    } else if settings_thr >= 0.23 {
        40
    } else if settings_thr >= 0.19 {
        50
    } else {
        200
    };
    let requested = if params_limit > 0 {
        params_limit as usize
    } else {
        thr_cap
    };
    let top_k = requested.min(thr_cap).min(200).max(1);
    (thr_cap, top_k)
}

/// Image→image cosine is typically ~0.55–0.95 (far above text→image).
/// Map the same UI ladder (0.28/0.24/0.20/0.16) onto image floors so Low ≠ Very High.
fn image_image_absolute_floor(settings_thr: f32) -> f32 {
    if settings_thr >= 0.27 {
        0.88
    } else if settings_thr >= 0.23 {
        0.82
    } else if settings_thr >= 0.19 {
        0.74
    } else {
        0.62
    }
}

/// Stricter caps for similar-from-file so default limit=50 does not hide the slider.
fn image_image_top_k(settings_thr: f32, params_limit: i64) -> (usize, usize) {
    let thr_cap: usize = if settings_thr >= 0.27 {
        12
    } else if settings_thr >= 0.23 {
        24
    } else if settings_thr >= 0.19 {
        40
    } else {
        100
    };
    let requested = if params_limit > 0 {
        params_limit as usize
    } else {
        thr_cap
    };
    let top_k = requested.min(thr_cap).min(200).max(1);
    (thr_cap, top_k)
}

/// Process-local image-search embedding matrix for one library DB.
/// Layout: ids[i] ↔ data[i*dim .. (i+1)*dim] (row-major f32).
struct EmbedMatrix {
    db_key: String,
    generation: u64,
    dim: usize,
    ids: Vec<i64>,
    data: Vec<f32>,
    norms: Vec<f32>,
}

#[derive(Default)]
struct EmbedMatrixLoadProfile {
    cache_hit: bool,
    sqlite_elapsed: Duration,
    build_elapsed: Duration,
    rows_seen: usize,
    rows_loaded: usize,
    rows_skipped: usize,
    matrix_bytes: usize,
}

impl EmbedMatrix {
    fn allocated_bytes(&self) -> usize {
        self.db_key
            .capacity()
            .saturating_add(self.ids.capacity().saturating_mul(size_of::<i64>()))
            .saturating_add(self.data.capacity().saturating_mul(size_of::<f32>()))
            .saturating_add(self.norms.capacity().saturating_mul(size_of::<f32>()))
    }
}

/// L2-normalized embedding row for HNSW (distance = 1 - dot = cosine distance).
#[derive(Clone)]
struct EmbedPoint(Arc<[f32]>);

impl instant_distance::Point for EmbedPoint {
    fn distance(&self, other: &Self) -> f32 {
        let a = &self.0;
        let b = &other.0;
        if a.len() != b.len() || a.is_empty() {
            return 2.0;
        }
        let mut dot = 0.0f32;
        for i in 0..a.len() {
            dot += a[i] * b[i];
        }
        1.0 - dot.clamp(-1.0, 1.0)
    }
}

struct EmbedAnnIndex {
    db_key: String,
    generation: u64,
    /// HNSW over unit vectors; values = row index into EmbedMatrix.
    map: HnswMap<EmbedPoint, usize>,
}

struct EmbedMatrixCache {
    current: Option<Arc<EmbedMatrix>>,
    ann: Option<Arc<EmbedAnnIndex>>,
    generations: HashMap<String, u64>,
    /// (db_key, generation) pairs where ANN build returned None — skip rebuild until generation bumps.
    ann_build_failed: HashSet<(String, u64)>,
    /// Generations with a background ANN build in flight (avoid duplicate workers).
    ann_building: HashSet<(String, u64)>,
}

fn embed_matrix_cache() -> &'static Mutex<EmbedMatrixCache> {
    static CACHE: OnceLock<Mutex<EmbedMatrixCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(EmbedMatrixCache {
            current: None,
            ann: None,
            generations: HashMap::new(),
            ann_build_failed: HashSet::new(),
            ann_building: HashSet::new(),
        })
    })
}

/// Soft cap: skip caching if matrix data would exceed this many bytes.
const EMBED_MATRIX_MAX_BYTES: usize = 512 * 1024 * 1024;

pub(crate) fn bump_embed_matrix_generation(db_path_key: &str) {
    if let Ok(mut cache) = embed_matrix_cache().lock() {
        let g = cache
            .generations
            .entry(db_path_key.to_string())
            .or_insert(0);
        *g = g.saturating_add(1);
        let new_g = *g;
        if cache.current.as_ref().map(|m| m.db_key.as_str()) == Some(db_path_key) {
            cache.current = None;
        }
        if cache.ann.as_ref().map(|a| a.db_key.as_str()) == Some(db_path_key) {
            cache.ann = None;
        }
        cache
            .ann_build_failed
            .retain(|(k, _)| k.as_str() != db_path_key);
        cache
            .ann_building
            .retain(|(k, _)| k.as_str() != db_path_key);
        let _ = new_g;
    }
}

pub(crate) fn invalidate_embed_matrix_for_current_db() {
    if let Ok(path) = t_storage::get_current_db_path() {
        let key = normalize_db_path_key(&path);
        bump_embed_matrix_generation(&key);
    }
}

/// Drop any cached matrix (e.g. library switch / storage migrate).
pub(crate) fn clear_embed_matrix_cache() {
    if let Ok(mut cache) = embed_matrix_cache().lock() {
        cache.current = None;
        cache.ann = None;
        cache.ann_build_failed.clear();
        cache.ann_building.clear();
        // Bump all known generations so a concurrent load is discarded.
        for g in cache.generations.values_mut() {
            *g = g.saturating_add(1);
        }
    }
}

fn parse_embed_warm_profile_enabled(value: Option<&str>) -> bool {
    value
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

fn embed_warm_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        parse_embed_warm_profile_enabled(
            std::env::var("PICAIPIC_EMBED_WARM_PROFILE").ok().as_deref(),
        )
    })
}

#[derive(Clone, Copy)]
struct ProcessResourceSnapshot {
    cpu_100ns: u64,
    working_set_bytes: u64,
}

#[cfg(target_os = "windows")]
fn process_resource_snapshot() -> Option<ProcessResourceSnapshot> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let process = unsafe { GetCurrentProcess() };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let mut memory = PROCESS_MEMORY_COUNTERS::default();
    let times_ok =
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) } != 0;
    let memory_ok = unsafe {
        K32GetProcessMemoryInfo(
            process,
            &mut memory,
            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    } != 0;
    if !times_ok || !memory_ok {
        return None;
    }
    let filetime_value =
        |value: FILETIME| ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64;
    Some(ProcessResourceSnapshot {
        cpu_100ns: filetime_value(kernel).saturating_add(filetime_value(user)),
        working_set_bytes: memory.WorkingSetSize as u64,
    })
}

#[cfg(not(target_os = "windows"))]
fn process_resource_snapshot() -> Option<ProcessResourceSnapshot> {
    None
}

fn normalized_process_cpu_percent(
    before_cpu_100ns: u64,
    after_cpu_100ns: u64,
    wall_seconds: f64,
    logical_cpus: usize,
) -> f64 {
    let cpu_seconds = after_cpu_100ns.saturating_sub(before_cpu_100ns) as f64 / 10_000_000.0;
    (cpu_seconds / wall_seconds.max(f64::EPSILON) / logical_cpus.max(1) as f64 * 100.0)
        .clamp(0.0, 100.0)
}

fn schedule_embed_warm_idle_profile(db_key: String) {
    std::thread::Builder::new()
        .name("embed-warm-idle-profile".into())
        .spawn(move || {
            const SETTLE_SECONDS: u64 = 5;
            const SAMPLE_SECONDS: u64 = 5;
            std::thread::sleep(Duration::from_secs(SETTLE_SECONDS));
            let Some(before) = process_resource_snapshot() else {
                println!("embed_matrix idle_profile db={db_key} unsupported_platform=1");
                return;
            };
            let wall_started = Instant::now();
            std::thread::sleep(Duration::from_secs(SAMPLE_SECONDS));
            let Some(after) = process_resource_snapshot() else {
                return;
            };
            let wall_seconds = wall_started.elapsed().as_secs_f64().max(f64::EPSILON);
            let logical_cpus = std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(1);
            let cpu_percent = normalized_process_cpu_percent(
                before.cpu_100ns,
                after.cpu_100ns,
                wall_seconds,
                logical_cpus,
            );
            println!(
                "embed_matrix idle_profile db={} settle_seconds={} window_seconds={:.3} process_cpu_percent={:.2} working_set_mib={:.1} logical_cpus={}",
                db_key,
                SETTLE_SECONDS,
                wall_seconds,
                cpu_percent,
                after.working_set_bytes as f64 / (1024.0 * 1024.0),
                logical_cpus
            );
        })
        .ok();
}

/// Preload the in-memory embed matrix off the first-search path. ANN stays lazy:
/// building it for every app launch is expensive for 100k+ libraries, while the
/// exact in-memory matrix already serves the first query.
/// Disk persistence of the full matrix is deferred (large, versioned artifact);
/// warming after library open/switch removes the matrix load from the first query.
pub fn warm_embed_matrix_cache() {
    std::thread::Builder::new()
        .name("embed-matrix-warm".into())
        .spawn(|| {
            let warm_started = Instant::now();
            let open_started = Instant::now();
            let Ok(conn) = open_conn() else {
                return;
            };
            let open_elapsed = open_started.elapsed();
            let detailed_profile = embed_warm_profile_enabled();
            let mut load_profile = EmbedMatrixLoadProfile::default();
            let loaded = if detailed_profile {
                get_or_load_embed_matrix_profiled(&conn, Some(&mut load_profile))
            } else {
                get_or_load_embed_matrix(&conn)
            };
            match loaded {
                Ok(Some(matrix)) => {
                    let ann_mode = if embed_ann_enabled() {
                        "lazy"
                    } else {
                        "disabled"
                    };
                    let total_elapsed = warm_started.elapsed();
                    println!(
                        "embed_matrix warm ready db={} n={} dim={} ann={} elapsed_seconds={:.3} matrix_mib={:.1}",
                        matrix.db_key,
                        matrix.ids.len(),
                        matrix.dim,
                        ann_mode,
                        total_elapsed.as_secs_f64(),
                        matrix.allocated_bytes() as f64 / (1024.0 * 1024.0)
                    );
                    if detailed_profile {
                        let accounted = open_elapsed
                            .saturating_add(load_profile.sqlite_elapsed)
                            .saturating_add(load_profile.build_elapsed);
                        println!(
                            "embed_matrix warm_profile db={} cache_hit={} open_seconds={:.3} sqlite_seconds={:.3} build_seconds={:.3} other_seconds={:.3} total_seconds={:.3} rows_seen={} rows_loaded={} rows_skipped={} matrix_mib={:.1} working_set_mib={}",
                            matrix.db_key,
                            load_profile.cache_hit,
                            open_elapsed.as_secs_f64(),
                            load_profile.sqlite_elapsed.as_secs_f64(),
                            load_profile.build_elapsed.as_secs_f64(),
                            total_elapsed.saturating_sub(accounted).as_secs_f64(),
                            total_elapsed.as_secs_f64(),
                            load_profile.rows_seen,
                            load_profile.rows_loaded,
                            load_profile.rows_skipped,
                            load_profile.matrix_bytes as f64 / (1024.0 * 1024.0),
                            process_resource_snapshot()
                                .map(|snapshot| format!("{:.1}", snapshot.working_set_bytes as f64 / (1024.0 * 1024.0)))
                                .unwrap_or_else(|| "na".to_string())
                        );
                        schedule_embed_warm_idle_profile(matrix.db_key.clone());
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("embed_matrix warm failed: {e}");
                }
            }
        })
        .ok();
}

fn load_embed_matrix(
    conn: &Connection,
    db_key: &str,
    generation: u64,
    mut profile: Option<&mut EmbedMatrixLoadProfile>,
) -> Result<Option<EmbedMatrix>, String> {
    let exclusion = AFile::search_exclusion_condition("b");
    let sql = format!(
        "SELECT a.id, a.embeds
         FROM afiles a
         LEFT JOIN afolders b ON a.folder_id = b.id
         WHERE a.embeds IS NOT NULL AND {exclusion}"
    );
    let sqlite_started = profile.as_ref().map(|_| Instant::now());
    let stmt_result = conn.prepare(&sql);
    if let (Some(started), Some(profile)) = (sqlite_started, profile.as_deref_mut()) {
        profile.sqlite_elapsed += started.elapsed();
    }
    let mut stmt = stmt_result.map_err(|e| e.to_string())?;

    let mut ids: Vec<i64> = Vec::new();
    let mut data: Vec<f32> = Vec::new();
    let mut norms: Vec<f32> = Vec::new();
    let mut dim: Option<usize> = None;

    let sqlite_started = profile.as_ref().map(|_| Instant::now());
    let rows_result = stmt.query([]);
    if let (Some(started), Some(profile)) = (sqlite_started, profile.as_deref_mut()) {
        profile.sqlite_elapsed += started.elapsed();
    }
    let mut rows = rows_result.map_err(|e| e.to_string())?;

    loop {
        let sqlite_started = profile.as_ref().map(|_| Instant::now());
        let row_result = rows.next();
        let row = match row_result {
            Ok(Some(row)) => row,
            Ok(None) => {
                if let (Some(started), Some(profile)) = (sqlite_started, profile.as_deref_mut()) {
                    profile.sqlite_elapsed += started.elapsed();
                }
                break;
            }
            Err(error) => return Err(error.to_string()),
        };
        let id: i64 = row.get(0).map_err(|error| error.to_string())?;
        let blob_value = row.get_ref(1).map_err(|error| error.to_string())?;
        let blob = match blob_value {
            ValueRef::Blob(blob) => blob,
            value => {
                return Err(format!(
                    "embedding row {id} has non-BLOB value type {:?}",
                    value.data_type()
                ));
            }
        };
        if let (Some(started), Some(profile)) = (sqlite_started, profile.as_deref_mut()) {
            profile.sqlite_elapsed += started.elapsed();
        }
        if let Some(profile) = profile.as_deref_mut() {
            profile.rows_seen = profile.rows_seen.saturating_add(1);
        }
        let build_started = profile.as_ref().map(|_| Instant::now());
        if blob.is_empty() || blob.len() % 4 != 0 {
            if let Some(profile) = profile.as_deref_mut() {
                profile.rows_skipped = profile.rows_skipped.saturating_add(1);
                if let Some(started) = build_started {
                    profile.build_elapsed += started.elapsed();
                }
            }
            continue;
        }
        let row_dim = blob.len() / 4;
        if let Some(d) = dim {
            if row_dim != d {
                if let Some(profile) = profile.as_deref_mut() {
                    profile.rows_skipped = profile.rows_skipped.saturating_add(1);
                    if let Some(started) = build_started {
                        profile.build_elapsed += started.elapsed();
                    }
                }
                continue; // skip mismatched rows
            }
        } else {
            dim = Some(row_dim);
        }
        let d = dim.unwrap();
        if ids
            .len()
            .saturating_add(1)
            .saturating_mul(d)
            .saturating_mul(4)
            > EMBED_MATRIX_MAX_BYTES
        {
            if let (Some(started), Some(profile)) = (build_started, profile.as_deref_mut()) {
                profile.build_elapsed += started.elapsed();
            }
            return Ok(None);
        }
        let mut norm_sq = 0.0f32;
        for chunk in blob.chunks_exact(4) {
            let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            data.push(v);
            norm_sq += v * v;
        }
        ids.push(id);
        norms.push(if norm_sq > 0.0 { norm_sq.sqrt() } else { 0.0 });
        if let Some(profile) = profile.as_deref_mut() {
            profile.rows_loaded = profile.rows_loaded.saturating_add(1);
            if let Some(started) = build_started {
                profile.build_elapsed += started.elapsed();
            }
        }
    }

    let shrink_started = profile.as_ref().map(|_| Instant::now());
    ids.shrink_to_fit();
    data.shrink_to_fit();
    norms.shrink_to_fit();
    if let (Some(started), Some(profile)) = (shrink_started, profile.as_deref_mut()) {
        profile.build_elapsed += started.elapsed();
    }

    let d = dim.unwrap_or(0);
    let matrix = EmbedMatrix {
        db_key: db_key.to_string(),
        generation,
        dim: d,
        ids,
        data,
        norms,
    };
    if let Some(profile) = profile.as_deref_mut() {
        profile.matrix_bytes = matrix.allocated_bytes();
    }
    Ok(Some(matrix))
}

fn get_or_load_embed_matrix(conn: &Connection) -> Result<Option<Arc<EmbedMatrix>>, String> {
    get_or_load_embed_matrix_profiled(conn, None)
}

fn get_or_load_embed_matrix_profiled(
    conn: &Connection,
    mut profile: Option<&mut EmbedMatrixLoadProfile>,
) -> Result<Option<Arc<EmbedMatrix>>, String> {
    let path = t_storage::get_current_db_path().map_err(|e| e.to_string())?;
    let db_key = normalize_db_path_key(&path);

    let generation = {
        let mut cache = embed_matrix_cache().lock().map_err(|e| e.to_string())?;
        *cache.generations.entry(db_key.clone()).or_insert(0)
    };

    {
        let cache = embed_matrix_cache().lock().map_err(|e| e.to_string())?;
        if let Some(ref m) = cache.current {
            if m.db_key == db_key && m.generation == generation {
                if let Some(profile) = profile.as_deref_mut() {
                    profile.cache_hit = true;
                    profile.matrix_bytes = m.allocated_bytes();
                    profile.rows_seen = m.ids.len();
                    profile.rows_loaded = m.ids.len();
                }
                return Ok(Some(m.clone()));
            }
        }
    }

    let loaded = load_embed_matrix(conn, &db_key, generation, profile.as_deref_mut())?;
    let Some(matrix) = loaded else {
        return Ok(None);
    };
    let arc = Arc::new(matrix);
    if let Ok(mut cache) = embed_matrix_cache().lock() {
        let g_now = *cache.generations.entry(db_key.clone()).or_insert(0);
        if g_now == generation {
            cache.current = Some(arc.clone());
            return Ok(Some(arc));
        }
    }
    // Generation advanced while loading — return loaded matrix for this query only.
    Ok(Some(arc))
}

/// Prefer serial below this row count (thread-pool overhead not worth it).
const SCORE_EMBED_MATRIX_PARALLEL_MIN_N: usize = 256;

/// Score all matrix rows; returns (scores >= 0.16, max_score, band histogram).
/// Uses rayon when N is large; serial otherwise. Semantics match `score_embed_matrix_serial`.
fn score_embed_matrix(
    matrix: &EmbedMatrix,
    query: &[f32],
    query_norm: f32,
) -> (Vec<(i64, f32)>, f32, [u32; 5]) {
    if matrix.ids.len() < SCORE_EMBED_MATRIX_PARALLEL_MIN_N {
        return score_embed_matrix_serial(matrix, query, query_norm);
    }
    score_embed_matrix_parallel(matrix, query, query_norm)
}

/// Auto: ANN candidate + exact rerank when index present and large N; else full matrix.
/// Returns (scores, max, bands, used_ann).
fn score_embed_matrix_auto(
    matrix: &EmbedMatrix,
    ann: Option<&EmbedAnnIndex>,
    query: &[f32],
    query_norm: f32,
) -> (Vec<(i64, f32)>, f32, [u32; 5], bool) {
    if let Some(ann) = ann {
        if ann.db_key == matrix.db_key
            && ann.generation == matrix.generation
            && matrix.ids.len() >= t_common::IMAGE_SEARCH_ANN_MIN_N
        {
            if let Some((scores, max_s, bands)) =
                score_embed_matrix_ann(matrix, ann, query, query_norm)
            {
                return (scores, max_s, bands, true);
            }
        }
    }
    let (s, m, b) = score_embed_matrix(matrix, query, query_norm);
    (s, m, b, false)
}

fn l2_normalize_owned(v: &[f32]) -> Option<Vec<f32>> {
    let mut norm_sq = 0.0f32;
    for &x in v {
        norm_sq += x * x;
    }
    if norm_sq <= 0.0 {
        return None;
    }
    let n = norm_sq.sqrt();
    Some(v.iter().map(|x| x / n).collect())
}

fn build_embed_ann(matrix: &EmbedMatrix) -> Option<EmbedAnnIndex> {
    if matrix.dim == 0 || matrix.ids.is_empty() {
        return None;
    }
    let mut points: Vec<EmbedPoint> = Vec::with_capacity(matrix.ids.len());
    let mut values: Vec<usize> = Vec::with_capacity(matrix.ids.len());
    for i in 0..matrix.ids.len() {
        if matrix.norms[i] <= 0.0 {
            continue;
        }
        let start = i * matrix.dim;
        let row = &matrix.data[start..start + matrix.dim];
        let Some(unit) = l2_normalize_owned(row) else {
            continue;
        };
        points.push(EmbedPoint(Arc::from(unit.into_boxed_slice())));
        values.push(i);
    }
    if points.is_empty() {
        return None;
    }
    let ef_search = t_common::IMAGE_SEARCH_ANN_EF_SEARCH;
    let ef_construction = t_common::IMAGE_SEARCH_ANN_EF_CONSTRUCTION.max(ef_search);
    let map = HnswBuilder::default()
        .ef_search(ef_search)
        .ef_construction(ef_construction)
        .seed(0xA11E_5EAC_u64)
        .build(points, values);
    Some(EmbedAnnIndex {
        db_key: matrix.db_key.clone(),
        generation: matrix.generation,
        map,
    })
}

fn get_or_build_embed_ann(matrix: &Arc<EmbedMatrix>) -> Option<Arc<EmbedAnnIndex>> {
    if !embed_ann_enabled() || matrix.ids.len() < t_common::IMAGE_SEARCH_ANN_MIN_N {
        return None;
    }
    let fail_key = (matrix.db_key.clone(), matrix.generation);
    {
        let cache = embed_matrix_cache().lock().ok()?;
        if let Some(ref ann) = cache.ann {
            if ann.db_key == matrix.db_key && ann.generation == matrix.generation {
                return Some(ann.clone());
            }
        }
        if cache.ann_build_failed.contains(&fail_key) {
            return None;
        }
        // Background build in flight — use exact matrix this query.
        if cache.ann_building.contains(&fail_key) {
            return None;
        }
    }

    // Non-blocking: schedule background build; this search uses exact matrix.
    schedule_background_ann_build(Arc::clone(matrix));
    None
}

const DEFAULT_EMBED_ANN_BUILD_THREADS: usize = 2;

fn parse_embed_ann_enabled(value: Option<&str>) -> bool {
    value
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

fn embed_ann_enabled() -> bool {
    parse_embed_ann_enabled(std::env::var("PICAIPIC_EMBED_ANN").ok().as_deref())
}

fn parse_embed_ann_build_threads(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|&value| (1..=8).contains(&value))
        .unwrap_or(DEFAULT_EMBED_ANN_BUILD_THREADS)
}

fn embed_ann_build_threads() -> usize {
    parse_embed_ann_build_threads(
        std::env::var("PICAIPIC_EMBED_ANN_BUILD_THREADS")
            .ok()
            .as_deref(),
    )
}

/// Build HNSW off the search path so the first large-library query stays responsive.
fn schedule_background_ann_build(matrix: Arc<EmbedMatrix>) {
    let fail_key = (matrix.db_key.clone(), matrix.generation);
    {
        let Ok(mut cache) = embed_matrix_cache().lock() else {
            return;
        };
        if cache.ann_build_failed.contains(&fail_key) || cache.ann_building.contains(&fail_key) {
            return;
        }
        if let Some(ref ann) = cache.ann {
            if ann.db_key == matrix.db_key && ann.generation == matrix.generation {
                return;
            }
        }
        cache.ann_building.insert(fail_key.clone());
    }

    let build_threads = embed_ann_build_threads();
    println!(
        "embed_ann build scheduled db={} gen={} n={} threads={}",
        matrix.db_key,
        matrix.generation,
        matrix.ids.len(),
        build_threads
    );

    std::thread::Builder::new()
        .name("embed-ann-build".into())
        .spawn(move || {
            let built = match ThreadPoolBuilder::new()
                .num_threads(build_threads)
                .thread_name(|index| format!("embed-ann-rayon-{index}"))
                .build()
            {
                Ok(pool) => pool.install(|| build_embed_ann(&matrix)),
                Err(error) => {
                    eprintln!("embed_ann worker pool failed: {error}");
                    None
                }
            };
            if let Ok(mut cache) = embed_matrix_cache().lock() {
                cache.ann_building.remove(&fail_key);
                let g_now = *cache
                    .generations
                    .get(&matrix.db_key)
                    .unwrap_or(&matrix.generation);
                if g_now != matrix.generation {
                    return;
                }
                match built {
                    Some(ann) => {
                        cache.ann_build_failed.remove(&fail_key);
                        cache.ann = Some(Arc::new(ann));
                        println!(
                            "embed_ann ready db={} gen={} n={}",
                            matrix.db_key,
                            matrix.generation,
                            matrix.ids.len()
                        );
                    }
                    None => {
                        cache.ann_build_failed.insert(fail_key);
                        eprintln!(
                            "embed_ann build failed db={} gen={} — using exact matrix until reindex",
                            matrix.db_key, matrix.generation
                        );
                    }
                }
            }
        })
        .ok();
}

#[cfg(test)]
mod embed_ann_config_tests {
    use super::{
        normalized_process_cpu_percent, parse_embed_ann_build_threads, parse_embed_ann_enabled,
        parse_embed_warm_profile_enabled,
    };

    #[test]
    fn ann_is_opt_in() {
        assert!(!parse_embed_ann_enabled(None));
        assert!(!parse_embed_ann_enabled(Some("0")));
        assert!(!parse_embed_ann_enabled(Some("false")));
        assert!(parse_embed_ann_enabled(Some("1")));
        assert!(parse_embed_ann_enabled(Some(" TRUE ")));
        assert!(parse_embed_ann_enabled(Some("yes")));
    }

    #[test]
    fn accepts_bounded_thread_counts_and_defaults_invalid_values() {
        assert_eq!(parse_embed_ann_build_threads(Some("1")), 1);
        assert_eq!(parse_embed_ann_build_threads(Some(" 4 ")), 4);
        assert_eq!(parse_embed_ann_build_threads(Some("8")), 8);
        assert_eq!(parse_embed_ann_build_threads(Some("0")), 2);
        assert_eq!(parse_embed_ann_build_threads(Some("9")), 2);
        assert_eq!(parse_embed_ann_build_threads(Some("invalid")), 2);
        assert_eq!(parse_embed_ann_build_threads(None), 2);
    }

    #[test]
    fn warm_profile_is_opt_in() {
        assert!(!parse_embed_warm_profile_enabled(None));
        assert!(!parse_embed_warm_profile_enabled(Some("0")));
        assert!(!parse_embed_warm_profile_enabled(Some("false")));
        assert!(parse_embed_warm_profile_enabled(Some("1")));
        assert!(parse_embed_warm_profile_enabled(Some(" TRUE ")));
        assert!(parse_embed_warm_profile_enabled(Some("yes")));
    }

    #[test]
    fn process_cpu_is_normalized_to_task_manager_scale() {
        assert_eq!(normalized_process_cpu_percent(0, 10_000_000, 5.0, 8), 2.5);
        assert_eq!(normalized_process_cpu_percent(0, 80_000_000, 1.0, 8), 100.0);
        assert_eq!(normalized_process_cpu_percent(20, 10, 5.0, 8), 0.0);
    }
}

/// ANN retrieve then exact cosine on candidates only (band histogram is candidate-local).
fn score_embed_matrix_ann(
    matrix: &EmbedMatrix,
    ann: &EmbedAnnIndex,
    query: &[f32],
    query_norm: f32,
) -> Option<(Vec<(i64, f32)>, f32, [u32; 5])> {
    if matrix.dim == 0 || query.len() != matrix.dim || query_norm <= 0.0 {
        return None;
    }
    let unit_q = l2_normalize_owned(query)?;
    let q_point = EmbedPoint(Arc::from(unit_q.into_boxed_slice()));
    let mut search = HnswSearch::default();
    let limit = t_common::IMAGE_SEARCH_ANN_CANDIDATES
        .min(matrix.ids.len())
        .max(1);

    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(limit);
    let mut max_score = f32::NEG_INFINITY;
    let mut band_gt = [0u32; 5];

    for item in ann.map.search(&q_point, &mut search) {
        if seen.len() >= limit {
            break;
        }
        let row_i = *item.value;
        if !seen.insert(row_i) {
            continue;
        }
        if row_i >= matrix.ids.len() {
            continue;
        }
        let id = matrix.ids[row_i];
        let Some((_, score)) = score_one_row(matrix, query, query_norm, row_i, id) else {
            continue;
        };
        update_score_bands(score, &mut max_score, &mut band_gt);
        if score >= 0.16 {
            out.push((id, score));
        }
    }
    Some((out, max_score, band_gt))
}

fn score_one_row(
    matrix: &EmbedMatrix,
    query: &[f32],
    query_norm: f32,
    i: usize,
    id: i64,
) -> Option<(i64, f32)> {
    let row_norm = matrix.norms[i];
    if row_norm <= 0.0 {
        return None;
    }
    let start = i * matrix.dim;
    let row = &matrix.data[start..start + matrix.dim];
    let mut dot = 0.0f32;
    for j in 0..matrix.dim {
        dot += query[j] * row[j];
    }
    Some((id, dot / (query_norm * row_norm)))
}

fn update_score_bands(score: f32, max_score: &mut f32, band_gt: &mut [u32; 5]) {
    if score > *max_score {
        *max_score = score;
    }
    if score > 0.18 {
        band_gt[0] += 1;
    }
    if score > 0.22 {
        band_gt[1] += 1;
    }
    if score > 0.28 {
        band_gt[2] += 1;
    }
    if score > 0.34 {
        band_gt[3] += 1;
    }
    if score > 0.40 {
        band_gt[4] += 1;
    }
}

fn score_embed_matrix_serial(
    matrix: &EmbedMatrix,
    query: &[f32],
    query_norm: f32,
) -> (Vec<(i64, f32)>, f32, [u32; 5]) {
    let mut out = Vec::with_capacity(matrix.ids.len());
    let mut max_score = f32::NEG_INFINITY;
    let mut band_gt = [0u32; 5];
    if matrix.dim == 0 || query.len() != matrix.dim || query_norm <= 0.0 {
        return (out, max_score, band_gt);
    }
    for (i, &id) in matrix.ids.iter().enumerate() {
        let Some((id, score)) = score_one_row(matrix, query, query_norm, i, id) else {
            continue;
        };
        update_score_bands(score, &mut max_score, &mut band_gt);
        if score >= 0.16 {
            out.push((id, score));
        }
    }
    (out, max_score, band_gt)
}

fn score_embed_matrix_parallel(
    matrix: &EmbedMatrix,
    query: &[f32],
    query_norm: f32,
) -> (Vec<(i64, f32)>, f32, [u32; 5]) {
    let empty = (Vec::new(), f32::NEG_INFINITY, [0u32; 5]);
    if matrix.dim == 0 || query.len() != matrix.dim || query_norm <= 0.0 {
        return empty;
    }

    // fold → reduce: no shared mutable band/max (histogram would race otherwise).
    matrix
        .ids
        .par_iter()
        .enumerate()
        .fold(
            || (Vec::new(), f32::NEG_INFINITY, [0u32; 5]),
            |(mut out, mut max_score, mut band_gt), (i, &id)| {
                if let Some((id, score)) = score_one_row(matrix, query, query_norm, i, id) {
                    update_score_bands(score, &mut max_score, &mut band_gt);
                    if score >= 0.16 {
                        out.push((id, score));
                    }
                }
                (out, max_score, band_gt)
            },
        )
        .reduce(
            || (Vec::new(), f32::NEG_INFINITY, [0u32; 5]),
            |(mut a_out, a_max, a_band), (b_out, b_max, b_band)| {
                a_out.extend(b_out);
                let max_score = a_max.max(b_max);
                let mut band_gt = a_band;
                for i in 0..5 {
                    band_gt[i] = band_gt[i].saturating_add(b_band[i]);
                }
                (a_out, max_score, band_gt)
            },
        )
}

#[cfg(test)]
mod image_search_top_k_tests {
    use super::{image_image_absolute_floor, image_image_top_k, image_search_top_k};

    #[test]
    fn low_respects_user_limit_20() {
        let (cap, k) = image_search_top_k(0.16, 20);
        assert_eq!(cap, 200);
        assert_eq!(k, 20);
    }

    #[test]
    fn low_default_limit_50_caps_at_50() {
        let (_cap, k) = image_search_top_k(0.16, 50);
        assert_eq!(k, 50);
    }

    #[test]
    fn very_high_never_exceeds_30() {
        let (cap, k) = image_search_top_k(0.28, 1000);
        assert_eq!(cap, 30);
        assert_eq!(k, 30);
    }

    #[test]
    fn medium_honors_smaller_limit() {
        let (_cap, k) = image_search_top_k(0.20, 10);
        assert_eq!(k, 10);
    }

    #[test]
    fn image_image_floors_spread_across_slider() {
        let low = image_image_absolute_floor(0.16);
        let med = image_image_absolute_floor(0.20);
        let high = image_image_absolute_floor(0.24);
        let vh = image_image_absolute_floor(0.28);
        assert!(low < med && med < high && high < vh);
        assert!((low - 0.62).abs() < 1e-5);
        assert!((vh - 0.88).abs() < 1e-5);
        // Text floors must not be reused for image queries (would make Low≈VH).
        assert!(low > 0.50);
    }

    #[test]
    fn image_image_top_k_stricter_than_text() {
        let (vh_cap, vh_k) = image_image_top_k(0.28, 50);
        let (low_cap, low_k) = image_image_top_k(0.16, 50);
        assert_eq!(vh_cap, 12);
        assert_eq!(vh_k, 12);
        assert_eq!(low_cap, 100);
        assert_eq!(low_k, 50);
        assert!(vh_k < low_k);
    }
}

#[cfg(test)]
mod score_embed_matrix_tests {
    use super::{
        EmbedMatrix, SCORE_EMBED_MATRIX_PARALLEL_MIN_N, score_embed_matrix_parallel,
        score_embed_matrix_serial,
    };
    use std::collections::HashMap;

    fn synthetic_matrix(n: usize, dim: usize) -> (EmbedMatrix, Vec<f32>, f32) {
        let mut ids = Vec::with_capacity(n);
        let mut data = Vec::with_capacity(n * dim);
        let mut norms = Vec::with_capacity(n);
        for i in 0..n {
            ids.push(i as i64 + 1);
            let mut norm_sq = 0.0f32;
            for j in 0..dim {
                // Deterministic non-zero pattern
                let v = (((i * 17 + j * 3) % 97) as f32) * 0.01 - 0.4;
                data.push(v);
                norm_sq += v * v;
            }
            norms.push(if norm_sq > 0.0 { norm_sq.sqrt() } else { 0.0 });
        }
        let mut query = vec![0.0f32; dim];
        let mut qn_sq = 0.0f32;
        for j in 0..dim {
            let v = ((j * 5 % 53) as f32) * 0.02 - 0.3;
            query[j] = v;
            qn_sq += v * v;
        }
        let query_norm = qn_sq.sqrt();
        let matrix = EmbedMatrix {
            db_key: "test".into(),
            generation: 0,
            dim,
            ids,
            data,
            norms,
        };
        (matrix, query, query_norm)
    }

    fn sort_scores(mut v: Vec<(i64, f32)>) -> Vec<(i64, f32)> {
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    #[test]
    fn parallel_matches_serial_on_large_matrix() {
        let n = SCORE_EMBED_MATRIX_PARALLEL_MIN_N.max(300);
        let (matrix, query, query_norm) = synthetic_matrix(n, 32);
        let (s_scores, s_max, s_band) = score_embed_matrix_serial(&matrix, &query, query_norm);
        let (p_scores, p_max, p_band) = score_embed_matrix_parallel(&matrix, &query, query_norm);

        assert_eq!(s_band, p_band, "band histogram must match");
        assert!(
            (s_max - p_max).abs() < 1e-5,
            "max_score serial={s_max} parallel={p_max}"
        );

        let s_map: HashMap<i64, f32> = sort_scores(s_scores).into_iter().collect();
        let p_map: HashMap<i64, f32> = sort_scores(p_scores).into_iter().collect();
        assert_eq!(s_map.len(), p_map.len(), "score set size");
        for (id, s) in &s_map {
            let p = p_map.get(id).unwrap_or_else(|| panic!("missing id {id}"));
            assert!((s - p).abs() < 1e-5, "id={id} serial={s} parallel={p}");
        }
    }

    #[test]
    fn serial_empty_on_dim_mismatch() {
        let (matrix, mut query, query_norm) = synthetic_matrix(4, 8);
        query.push(0.1);
        let (scores, max_score, band) = score_embed_matrix_serial(&matrix, &query, query_norm);
        assert!(scores.is_empty());
        assert!(!max_score.is_finite() || max_score == f32::NEG_INFINITY);
        assert_eq!(band, [0; 5]);
    }

    #[test]
    fn ann_rerank_includes_query_nearest_cluster() {
        use super::{build_embed_ann, score_embed_matrix_ann, score_embed_matrix_serial};

        // Small hand-built matrix: first rows aligned with query, rest orthogonal-ish.
        let dim = 16;
        let n = 64;
        let mut ids = Vec::new();
        let mut data = Vec::new();
        let mut norms = Vec::new();
        for i in 0..n {
            ids.push(i as i64 + 1);
            let mut row = vec![0.0f32; dim];
            if i < 8 {
                // Near query (mostly e0)
                row[0] = 1.0;
                row[1] = 0.05 * (i as f32);
            } else {
                row[2] = 1.0;
                row[3] = 0.1 * ((i % 7) as f32);
            }
            let mut nsq = 0.0f32;
            for &v in &row {
                nsq += v * v;
            }
            let norm = nsq.sqrt();
            norms.push(norm);
            data.extend(row);
        }
        let matrix = EmbedMatrix {
            db_key: "ann-test".into(),
            generation: 1,
            dim,
            ids,
            data,
            norms,
        };
        let mut query = vec![0.0f32; dim];
        query[0] = 1.0;
        let query_norm = 1.0f32;

        let ann = build_embed_ann(&matrix).expect("ann build");
        let (ann_scores, _, _) =
            score_embed_matrix_ann(&matrix, &ann, &query, query_norm).expect("ann score");
        let (exact_scores, _, _) = score_embed_matrix_serial(&matrix, &query, query_norm);

        let exact_top: Vec<i64> = {
            let mut v = exact_scores;
            v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            v.into_iter().take(5).map(|(id, _)| id).collect()
        };
        let ann_ids: HashMap<i64, f32> = ann_scores.into_iter().collect();
        for id in exact_top {
            assert!(
                ann_ids.contains_key(&id),
                "ANN candidates should include exact top id {id}"
            );
        }
    }
}

#[cfg(test)]
mod query_builder_tests {
    use super::{AFile, QueryParams, SmartQueryParams, SmartRule};
    use rusqlite::ToSql;

    fn empty_query_params() -> QueryParams {
        QueryParams {
            search_file_name: String::new(),
            search_file_type: 0,
            sort_type: 0,
            sort_order: 0,
            search_all_subfolders: String::new(),
            search_folder: String::new(),
            start_date: 0,
            end_date: 0,
            calendar_sort: 0,
            make: String::new(),
            model: String::new(),
            lens_make: String::new(),
            lens_model: String::new(),
            location_admin1: String::new(),
            location_name: String::new(),
            is_favorite: false,
            rating: -1,
            tag_id: 0,
            person_id: 0,
            gps_min_lat: None,
            gps_max_lat: None,
            gps_min_lon: None,
            gps_max_lon: None,
        }
    }

    fn rule(field: &str, operator: &str, value: serde_json::Value) -> SmartRule {
        SmartRule {
            id: "t".into(),
            field: field.into(),
            operator: operator.into(),
            value,
        }
    }

    #[test]
    fn file_type_mask_image_and_live() {
        let c = AFile::build_file_type_condition(1 | 8).expect("mask");
        assert!(c.contains("a.file_type = 1"));
        assert!(c.contains("live_photo_type"));
        assert!(c.contains(" OR "));
        assert!(AFile::build_file_type_condition(0).is_none());
        // All three traditional kinds ≡ no type filter.
        assert!(AFile::build_file_type_condition(7).is_none());
    }

    #[test]
    fn search_parts_always_exclude_live_companion_and_excluded_folders() {
        let (_joins, where_clause, params) = AFile::build_search_query_parts(&empty_query_params());
        assert!(where_clause.contains("live_photo_type"));
        assert!(where_clause.contains("is_excluded_from_search"));
        assert!(params.is_empty());
    }

    #[test]
    fn search_parts_name_favorite_rating_tag_person() {
        let mut q = empty_query_params();
        q.search_file_name = "Holiday".into();
        q.is_favorite = true;
        q.rating = 5;
        q.tag_id = 12;
        q.person_id = 3;
        let (joins, where_clause, params) = AFile::build_search_query_parts(&q);
        assert!(where_clause.contains("a.name LIKE ? COLLATE NOCASE"));
        assert!(where_clause.contains("a.is_favorite = 1"));
        assert!(where_clause.contains("a.rating = ?"));
        assert!(joins.contains("afile_tags"));
        assert!(joins.contains("faces"));
        // name, rating, tag_id, person_id
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn search_parts_rating_zero_means_unrated() {
        let mut q = empty_query_params();
        q.rating = 0;
        let (_j, where_clause, params) = AFile::build_search_query_parts(&q);
        assert!(where_clause.contains("(a.rating = 0 OR a.rating IS NULL)"));
        assert!(params.is_empty());
    }

    #[test]
    fn search_parts_date_range_uses_local_calendar_day() {
        let mut q = empty_query_params();
        q.start_date = 1_700_000_000;
        q.end_date = 1_700_086_400;
        q.calendar_sort = 0; // taken_date
        let (_j, where_clause, params) = AFile::build_search_query_parts(&q);
        assert!(
            where_clause.contains("strftime('%Y-%m-%d', a.taken_date, 'unixepoch', 'localtime')")
        );
        assert!(where_clause.contains("localtime"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn search_parts_gps_antimeridian_or_clause() {
        let mut q = empty_query_params();
        q.gps_min_lat = Some(-10.0);
        q.gps_max_lat = Some(10.0);
        q.gps_min_lon = Some(170.0);
        q.gps_max_lon = Some(-170.0);
        let (_j, where_clause, params) = AFile::build_search_query_parts(&q);
        assert!(where_clause.contains("gps_latitude BETWEEN"));
        assert!(where_clause.contains("gps_longitude >= ? OR a.gps_longitude <= ?"));
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn smart_size_mb_to_bytes_in_condition() {
        let mut joins = Vec::new();
        let mut needs_group = false;
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        let cond = AFile::build_smart_rule_condition(
            &rule("size", "gt", serde_json::json!(1)),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert_eq!(cond, "a.size > ?");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn smart_name_favorite_and_rating_empty() {
        let mut joins = Vec::new();
        let mut needs_group = false;
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        let name = AFile::build_smart_rule_condition(
            &rule("name", "contains", serde_json::json!("cat")),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert_eq!(name, "a.name LIKE ? COLLATE NOCASE");
        assert_eq!(params.len(), 1);

        let fav = AFile::build_smart_rule_condition(
            &rule("favorite", "is", serde_json::json!(true)),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert_eq!(fav, "a.is_favorite = 1");

        let empty_rating = AFile::build_smart_rule_condition(
            &rule("rating", "empty", serde_json::Value::Null),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert_eq!(empty_rating, "(a.rating IS NULL OR a.rating = 0)");
    }

    #[test]
    fn smart_tag_has_sets_join_and_group() {
        let mut joins = Vec::new();
        let mut needs_group = false;
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        let cond = AFile::build_smart_rule_condition(
            &rule("tag", "has", serde_json::json!(9)),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert_eq!(cond, "at_smart.tag_id = ?");
        assert!(needs_group);
        assert!(joins.iter().any(|j| j.contains("afile_tags")));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn smart_person_not_has_uses_not_exists() {
        let mut joins = Vec::new();
        let mut needs_group = false;
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        let cond = AFile::build_smart_rule_condition(
            &rule("person", "not_has", serde_json::json!(4)),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert!(cond.contains("NOT EXISTS"));
        assert!(cond.contains("person_id"));
        assert!(!needs_group);
        assert!(joins.is_empty());
    }

    #[test]
    fn smart_date_before_uses_local_day_compare() {
        let mut joins = Vec::new();
        let mut needs_group = false;
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        let cond = AFile::build_smart_rule_condition(
            &rule("date_taken", "before", serde_json::json!(1_700_000_000)),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap();
        assert!(cond.contains("a.taken_date"));
        assert!(cond.contains("localtime"));
        assert!(cond.contains('<'));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn smart_unsupported_field_errors() {
        let mut joins = Vec::new();
        let mut needs_group = false;
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();
        let err = AFile::build_smart_rule_condition(
            &rule("nope", "is", serde_json::json!(1)),
            &mut joins,
            &mut needs_group,
            &mut params,
        )
        .unwrap_err();
        assert!(err.contains("Unsupported smart field"));
    }

    #[test]
    fn smart_query_requires_rules_and_match_any_ors() {
        let empty = AFile::build_smart_query_parts(&SmartQueryParams {
            version: 1,
            match_mode: "all".into(),
            rules: vec![],
            sort_type: 0,
            sort_order: 0,
            calendar_sort: 0,
        });
        assert!(empty.is_err());

        let (_j, where_all, _, _) = AFile::build_smart_query_parts(&SmartQueryParams {
            version: 1,
            match_mode: "all".into(),
            rules: vec![
                rule("favorite", "is", serde_json::json!(true)),
                rule("rating", "gte", serde_json::json!(3)),
            ],
            sort_type: 0,
            sort_order: 0,
            calendar_sort: 0,
        })
        .unwrap();
        assert!(where_all.contains("a.is_favorite = 1"));
        assert!(where_all.contains(" AND "));

        let (_j, where_any, _, _) = AFile::build_smart_query_parts(&SmartQueryParams {
            version: 1,
            match_mode: "any".into(),
            rules: vec![
                rule("favorite", "is", serde_json::json!(true)),
                rule("rating", "gte", serde_json::json!(3)),
            ],
            sort_type: 0,
            sort_order: 0,
            calendar_sort: 0,
        })
        .unwrap();
        assert!(where_any.contains(" OR "));
        assert!(where_any.contains("is_excluded_from_search"));
        assert!(where_any.contains("live_photo_type"));
    }

    #[test]
    fn smart_sj_helpers_parse_mixed_json() {
        assert_eq!(AFile::sj_i64(&serde_json::json!(3)), Some(3));
        assert_eq!(AFile::sj_i64(&serde_json::json!("7")), Some(7));
        assert_eq!(AFile::sj_bool(&serde_json::json!("yes")), Some(true));
        assert_eq!(AFile::sj_bool(&serde_json::json!(0)), Some(false));
        assert_eq!(AFile::sj_str(&serde_json::json!(12)), Some("12".into()));
    }
}

#[cfg(test)]
mod crud_tests {
    use super::{
        AFile, AFileAddProfile, AFileMetadataProfiler, AFolder, BinaryExifFallbackProfile,
        RawMetadataTarget,
    };
    use crate::t_libraw::RawMeta;
    use rusqlite::{Connection, params};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    #[ignore = "requires PICAIPIC_SCAN_PROFILE_DIR with a representative media library"]
    fn profile_afile_new_for_directory() {
        let root = std::env::var("PICAIPIC_SCAN_PROFILE_DIR")
            .expect("set PICAIPIC_SCAN_PROFILE_DIR to a read-only media directory");
        let limit = std::env::var("PICAIPIC_SCAN_PROFILE_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        let started = std::time::Instant::now();
        let mut processed = 0usize;
        let mut failed = 0usize;

        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            if processed >= limit {
                break;
            }
            let Some(path) = entry.path().to_str() else {
                failed += 1;
                continue;
            };
            if AFile::new(1, path, 1).is_err() {
                failed += 1;
            }
            processed += 1;
        }

        let elapsed = started.elapsed().as_secs_f64();
        eprintln!(
            "[scan-profile] root='{}' files={} failed={} seconds={:.3} files_per_second={:.1}",
            root,
            processed,
            failed,
            elapsed,
            processed as f64 / elapsed.max(0.001)
        );
        assert!(processed > 0, "profile directory contained no files");
    }

    #[test]
    #[ignore = "requires PICAIPIC_SCAN_PROFILE_DIR; writes only a temporary SQLite fixture"]
    fn profile_directory_index_and_thumbnails() {
        let root = std::env::var("PICAIPIC_SCAN_PROFILE_DIR")
            .expect("set PICAIPIC_SCAN_PROFILE_DIR to a read-only media directory");
        let limit = std::env::var("PICAIPIC_SCAN_PROFILE_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        let thumbnail_size = std::env::var("PICAIPIC_SCAN_PROFILE_THUMB_SIZE")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(200)
            .max(1);
        let token = format!(
            "picaipic-scan-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let db_path = std::env::temp_dir().join(format!("{token}.db"));
        let conn = Connection::open(&db_path).expect("open temporary profiling DB");
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE afolders (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 album_id INTEGER NOT NULL,
                 name TEXT NOT NULL,
                 path TEXT NOT NULL UNIQUE,
                 created_at INTEGER,
                 modified_at INTEGER,
                 is_favorite INTEGER DEFAULT 0,
                 is_excluded_from_search INTEGER DEFAULT 0
             );
             CREATE TABLE afiles (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 folder_id INTEGER NOT NULL,
                 name TEXT NOT NULL,
                 name_pinyin TEXT, size INTEGER NOT NULL, file_type INTEGER, format_label TEXT,
                 created_at INTEGER, modified_at INTEGER, inode INTEGER, taken_date INTEGER,
                 width INTEGER, height INTEGER, duration INTEGER, is_favorite INTEGER,
                 rating INTEGER NOT NULL DEFAULT 0, rotate INTEGER, comments TEXT, has_tags INTEGER,
                 e_make TEXT, e_model TEXT, e_date_time TEXT, e_software TEXT, e_artist TEXT,
                 e_copyright TEXT, e_description TEXT, e_lens_make TEXT, e_lens_model TEXT,
                 e_exposure_bias TEXT, e_exposure_time TEXT, e_f_number TEXT, e_focal_length TEXT,
                 e_iso_speed TEXT, e_flash TEXT, e_orientation INTEGER, gps_latitude REAL,
                 gps_longitude REAL, gps_altitude REAL, geo_name TEXT, geo_admin1 TEXT,
                 geo_admin2 TEXT, geo_cc TEXT, content_id TEXT, paired_file_id INTEGER,
                 live_photo_type INTEGER DEFAULT 0, last_scan_time INTEGER DEFAULT 0,
                 UNIQUE(folder_id, name),
                 FOREIGN KEY(folder_id) REFERENCES afolders(id) ON DELETE CASCADE
             );
             CREATE TABLE athumbs (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL UNIQUE,
                 error_code INTEGER NOT NULL,
                 thumb_data BLOB,
                 FOREIGN KEY(file_id) REFERENCES afiles(id) ON DELETE CASCADE
             );",
        )
        .expect("create temporary profiling schema");

        let started = std::time::Instant::now();
        let mut index_elapsed = std::time::Duration::ZERO;
        let mut thumbnail_elapsed = std::time::Duration::ZERO;
        let mut folders = std::collections::HashMap::<String, i64>::new();
        let mut processed = 0usize;
        let mut index_failed = 0usize;
        let mut thumbnail_failed = 0usize;
        let mut thumbnail_bytes = 0u64;

        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            if processed >= limit {
                break;
            }
            let path = entry.path();
            let Some(path_str) = path.to_str() else {
                index_failed += 1;
                continue;
            };
            let Some(file_type) = crate::t_utils::get_file_type(path_str) else {
                continue;
            };
            let parent = path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(&root))
                .to_string_lossy()
                .into_owned();
            let folder_id = if let Some(id) = folders.get(&parent) {
                *id
            } else {
                let folder = AFolder::add_to_db_with_conn(&conn, 1, &parent)
                    .expect("insert profiling folder");
                let id = folder.id.expect("profiling folder id");
                folders.insert(parent, id);
                id
            };

            let index_started = std::time::Instant::now();
            let file = AFile::new(folder_id, path_str, file_type);
            let file = match file {
                Ok(mut file) => {
                    file.last_scan_time = Some(1);
                    if file.insert_with_conn(&conn).is_err() {
                        index_failed += 1;
                        index_elapsed += index_started.elapsed();
                        processed += 1;
                        continue;
                    }
                    file
                }
                Err(_) => {
                    index_failed += 1;
                    index_elapsed += index_started.elapsed();
                    processed += 1;
                    continue;
                }
            };
            let file_id = conn.last_insert_rowid();
            index_elapsed += index_started.elapsed();

            let thumbnail_started = std::time::Instant::now();
            let thumbnail = match file_type {
                1 => crate::t_image::get_image_thumbnail(
                    path_str,
                    file.e_orientation.unwrap_or(1) as i32,
                    thumbnail_size,
                ),
                3 => crate::t_image::get_raw_thumbnail(
                    path_str,
                    file.e_orientation.unwrap_or(1) as i32,
                    thumbnail_size,
                ),
                _ => Ok(None),
            };
            let (error_code, thumbnail) = match thumbnail {
                Ok(Some(bytes)) => {
                    thumbnail_bytes += bytes.len() as u64;
                    (0, Some(bytes))
                }
                _ => {
                    thumbnail_failed += 1;
                    (1, None)
                }
            };
            conn.execute(
                "INSERT INTO athumbs (file_id, error_code, thumb_data) VALUES (?1, ?2, ?3)",
                params![file_id, error_code, thumbnail],
            )
            .expect("insert profiling thumbnail row");
            thumbnail_elapsed += thumbnail_started.elapsed();
            processed += 1;
        }

        let elapsed = started.elapsed().as_secs_f64();
        eprintln!(
            "[scan-profile-direct] root='{}' files={} folders={} index_failed={} thumbnail_failed={} thumbnail_size={} index_seconds={:.3} thumbnail_seconds={:.3} thumbnail_bytes={} total_seconds={:.3} files_per_second={:.1} temp_db='{}'",
            root,
            processed,
            folders.len(),
            index_failed,
            thumbnail_failed,
            thumbnail_size,
            index_elapsed.as_secs_f64(),
            thumbnail_elapsed.as_secs_f64(),
            thumbnail_bytes,
            elapsed,
            processed as f64 / elapsed.max(0.001),
            db_path.display(),
        );
        drop(conn);
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(db_path.with_extension("db-wal"));
        let _ = fs::remove_file(db_path.with_extension("db-shm"));
        assert!(
            processed > 0,
            "profile directory contained no supported media files"
        );
    }

    fn fixture() -> (Connection, PathBuf, PathBuf) {
        let token = format!(
            "picaipic-crud-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let db_path = std::env::temp_dir().join(format!("{token}.db"));
        let media_path = std::env::temp_dir().join(format!("{token}.txt"));
        fs::write(&media_path, b"fixture media").unwrap();

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE afolders (
                 id INTEGER PRIMARY KEY,
                 album_id INTEGER NOT NULL,
                 name TEXT NOT NULL,
                 path TEXT NOT NULL,
                 is_excluded_from_search INTEGER DEFAULT 0
             );
             CREATE TABLE afiles (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 folder_id INTEGER NOT NULL,
                 name TEXT NOT NULL,
                 name_pinyin TEXT,
                 size INTEGER NOT NULL,
                 file_type INTEGER,
                 format_label TEXT,
                 created_at INTEGER,
                 modified_at INTEGER,
                 inode INTEGER,
                 taken_date INTEGER,
                 width INTEGER,
                 height INTEGER,
                 duration INTEGER,
                 is_favorite INTEGER,
                 rating INTEGER NOT NULL DEFAULT 0,
                 rotate INTEGER,
                 comments TEXT,
                 has_tags INTEGER,
                 e_make TEXT,
                 e_model TEXT,
                 e_date_time TEXT,
                 e_software TEXT,
                 e_artist TEXT,
                 e_copyright TEXT,
                 e_description TEXT,
                 e_lens_make TEXT,
                 e_lens_model TEXT,
                 e_exposure_bias TEXT,
                 e_exposure_time TEXT,
                 e_f_number TEXT,
                 e_focal_length TEXT,
                 e_iso_speed TEXT,
                 e_flash TEXT,
                 e_orientation INTEGER,
                 gps_latitude REAL,
                 gps_longitude REAL,
                 gps_altitude REAL,
                 geo_name TEXT,
                 geo_admin1 TEXT,
                 geo_admin2 TEXT,
                 geo_cc TEXT,
                 content_id TEXT,
                 paired_file_id INTEGER,
                 live_photo_type INTEGER DEFAULT 0,
                 embeds BLOB,
                 last_scan_time INTEGER DEFAULT 0,
                 UNIQUE(folder_id, name),
                 FOREIGN KEY(folder_id) REFERENCES afolders(id) ON DELETE CASCADE
             );
             CREATE TABLE athumbs (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id INTEGER NOT NULL,
                 error_code INTEGER NOT NULL,
                 FOREIGN KEY(file_id) REFERENCES afiles(id) ON DELETE CASCADE
             );
             INSERT INTO afolders (id, album_id, name, path) VALUES (1, 1, 'fixture', '/fixture');",
        )
        .unwrap();

        (conn, db_path, media_path)
    }

    #[test]
    fn header_pre_read_is_limited_to_image_and_raw_types() {
        let media_path = std::env::temp_dir().join(format!(
            "picaipic-header-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&media_path, b"header-fixture").unwrap();
        let path = media_path.to_string_lossy();

        assert_eq!(
            AFile::read_file_header(&path, 1).as_deref(),
            Some(b"header-fixture".as_slice())
        );
        assert_eq!(
            AFile::read_file_header(&path, 3).as_deref(),
            Some(b"header-fixture".as_slice())
        );
        assert!(AFile::read_file_header(&path, 0).is_none());

        let _ = fs::remove_file(media_path);
    }

    #[test]
    fn exif_read_strategy_ignores_non_media_and_bad_headers() {
        let media_path = std::env::temp_dir().join(format!(
            "picaipic-exif-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&media_path, b"not-exif").unwrap();
        let path = media_path.to_string_lossy();
        assert!(AFile::read_image_exif(&path, 0, Some(b"not-exif")).is_none());
        assert!(AFile::read_image_exif(&path, 1, Some(b"not-exif")).is_none());
        let _ = fs::remove_file(media_path);
    }

    #[test]
    fn profiled_exif_read_separates_header_and_file_fallback() {
        let mut profile = AFileAddProfile::scan_enabled();
        let mut profiler = AFileMetadataProfiler::new(Some(&mut profile));

        let header_exif = profiler.measure_exif(|profiler| {
            AFile::read_image_exif_profiled(
                "missing.jpg",
                1,
                Some(&orientation_tiff_fixture()),
                profiler,
            )
        });
        assert!(header_exif.is_some());
        drop(profiler);
        assert_eq!(profile.metadata_exif_header_attempts, 1);
        assert_eq!(profile.metadata_exif_file_fallback_attempts, 0);

        let mut profiler = AFileMetadataProfiler::new(Some(&mut profile));
        let fallback_exif = profiler.measure_exif(|profiler| {
            AFile::read_image_exif_profiled("missing.jpg", 1, Some(b"not-exif"), profiler)
        });
        assert!(fallback_exif.is_none());
        drop(profiler);
        assert_eq!(profile.metadata_exif_header_attempts, 2);
        assert_eq!(profile.metadata_exif_file_fallback_attempts, 1);
        assert!(profile.metadata_exif_elapsed >= profile.metadata_exif_header_elapsed);
    }

    #[test]
    fn profiled_exif_read_skips_complete_jpeg_without_exif() {
        let mut profile = AFileAddProfile::scan_enabled();
        let mut profiler = AFileMetadataProfiler::new(Some(&mut profile));
        let jpeg = [
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x02, // empty APP0
            0xff, 0xda, // SOS
        ];

        let exif = profiler.measure_exif(|profiler| {
            AFile::read_image_exif_profiled("missing.jpg", 1, Some(&jpeg), profiler)
        });
        assert!(exif.is_none());
        drop(profiler);
        assert_eq!(profile.metadata_exif_header_attempts, 0);
        assert_eq!(profile.metadata_exif_file_fallback_attempts, 0);
    }

    #[test]
    fn complete_jpeg_without_exif_uses_default_orientation() {
        let jpeg = [
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x02, // empty APP0
            0xff, 0xda, // SOS
        ];

        assert_eq!(AFile::extract_orientation(&None, Some(&jpeg)), 1);
    }

    fn orientation_tiff_fixture() -> Vec<u8> {
        vec![
            b'I', b'I', 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00, // TIFF header + IFD offset
            0x01, 0x00, // one IFD entry
            0x12, 0x01, 0x03, 0x00, // Orientation, SHORT
            0x01, 0x00, 0x00, 0x00, // one value
            0x06, 0x00, 0x00, 0x00, // rotate 90 CW
            0x00, 0x00, 0x00, 0x00, // no next IFD
        ]
    }

    fn identity_tiff_fixture() -> Vec<u8> {
        let mut data = vec![b'I', b'I', 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00];
        let make = b"Canon\0";
        let model = b"EOS R\0";
        let software = b"PicAiPic\0";
        let artist = b"Alice\0";
        let copyright = b"2026 Co\0";
        let description = b"Sunset\0";
        let lens_make = b"Canon\0";
        let lens_model = b"RF50\0";
        let date = b"2026:07:27 13:45:00\0";
        let user_comment = b"ASCII\0\0\0prompt text\0";
        let make_offset = 98u32;
        let model_offset = make_offset + make.len() as u32;
        let software_offset = model_offset + model.len() as u32;
        let artist_offset = software_offset + software.len() as u32;
        let copyright_offset = artist_offset + artist.len() as u32;
        let description_offset = copyright_offset + copyright.len() as u32;
        let exif_offset = description_offset + description.len() as u32;
        let date_offset = exif_offset + 114;
        let user_comment_offset = date_offset + date.len() as u32;
        let lens_make_offset = user_comment_offset + user_comment.len() as u32;
        let lens_model_offset = lens_make_offset + lens_make.len() as u32;
        let exposure_time_offset = lens_model_offset + lens_model.len() as u32;
        let f_number_offset = exposure_time_offset + 8;
        let exposure_bias_offset = f_number_offset + 8;
        let focal_length_offset = exposure_bias_offset + 8;
        let push_u16 = |data: &mut Vec<u8>, value: u16| data.extend(value.to_le_bytes());
        let push_u32 = |data: &mut Vec<u8>, value: u32| data.extend(value.to_le_bytes());
        let push_i32 = |data: &mut Vec<u8>, value: i32| data.extend(value.to_le_bytes());

        push_u16(&mut data, 7);
        for (tag, count, offset) in [
            (0x010e, description.len() as u32, description_offset),
            (0x010f, make.len() as u32, make_offset),
            (0x0110, model.len() as u32, model_offset),
            (0x0131, software.len() as u32, software_offset),
            (0x013b, artist.len() as u32, artist_offset),
            (0x8298, copyright.len() as u32, copyright_offset),
        ] {
            push_u16(&mut data, tag);
            push_u16(&mut data, 2);
            push_u32(&mut data, count);
            push_u32(&mut data, offset);
        }
        push_u16(&mut data, 0x8769);
        push_u16(&mut data, 4);
        push_u32(&mut data, 1);
        push_u32(&mut data, exif_offset);
        push_u32(&mut data, 0);
        data.extend(make);
        data.extend(model);
        data.extend(software);
        data.extend(artist);
        data.extend(copyright);
        data.extend(description);
        push_u16(&mut data, 9);
        push_u16(&mut data, 0x829a);
        push_u16(&mut data, 5);
        push_u32(&mut data, 1);
        push_u32(&mut data, exposure_time_offset);
        push_u16(&mut data, 0x829d);
        push_u16(&mut data, 5);
        push_u32(&mut data, 1);
        push_u32(&mut data, f_number_offset);
        push_u16(&mut data, 0x8827);
        push_u16(&mut data, 3);
        push_u32(&mut data, 1);
        data.extend([200, 0, 0, 0]);
        push_u16(&mut data, 0x9003);
        push_u16(&mut data, 2);
        push_u32(&mut data, date.len() as u32);
        push_u32(&mut data, date_offset);
        push_u16(&mut data, 0x9204);
        push_u16(&mut data, 10);
        push_u32(&mut data, 1);
        push_u32(&mut data, exposure_bias_offset);
        push_u16(&mut data, 0x920a);
        push_u16(&mut data, 5);
        push_u32(&mut data, 1);
        push_u32(&mut data, focal_length_offset);
        push_u16(&mut data, 0x9286);
        push_u16(&mut data, 7);
        push_u32(&mut data, user_comment.len() as u32);
        push_u32(&mut data, user_comment_offset);
        push_u16(&mut data, 0xa433);
        push_u16(&mut data, 2);
        push_u32(&mut data, lens_make.len() as u32);
        push_u32(&mut data, lens_make_offset);
        push_u16(&mut data, 0xa434);
        push_u16(&mut data, 2);
        push_u32(&mut data, lens_model.len() as u32);
        push_u32(&mut data, lens_model_offset);
        push_u32(&mut data, 0);
        data.extend(date);
        data.extend(user_comment);
        data.extend(lens_make);
        data.extend(lens_model);
        push_u32(&mut data, 1);
        push_u32(&mut data, 125);
        push_u32(&mut data, 28);
        push_u32(&mut data, 10);
        push_i32(&mut data, 0);
        push_i32(&mut data, 1);
        push_u32(&mut data, 50);
        push_u32(&mut data, 1);
        data
    }

    #[test]
    fn exif_fixture_drives_orientation_extraction() {
        let fixture = orientation_tiff_fixture();
        let exif = AFile::read_image_exif("missing.jpg", 1, Some(&fixture))
            .expect("minimal TIFF EXIF should parse");
        assert_eq!(AFile::extract_orientation(&Some(exif), Some(&fixture)), 6);
    }

    #[test]
    fn exif_fixture_drives_identity_extraction() {
        let fixture = identity_tiff_fixture();
        let exif = AFile::read_image_exif("missing.jpg", 1, Some(&fixture))
            .expect("identity TIFF EXIF should parse");
        let identity = AFile::extract_exif_identity(&Some(exif), Some(7));
        assert_eq!(identity.make.as_deref(), Some("Canon"));
        assert_eq!(identity.model.as_deref(), Some("EOS R"));
        assert_eq!(identity.software.as_deref(), Some("PicAiPic"));
        assert_eq!(identity.date_time.as_deref(), Some("2026:07:27 13:45:00"));
        assert!(identity.taken_date.is_some());
        assert_ne!(identity.taken_date, Some(7));

        let fallback = AFile::extract_exif_identity(&None, Some(7));
        assert_eq!(fallback.taken_date, Some(7));
    }

    #[test]
    fn exif_fixture_drives_description_extraction() {
        let fixture = identity_tiff_fixture();
        let exif = AFile::read_image_exif("missing.jpg", 1, Some(&fixture))
            .expect("description TIFF EXIF should parse");
        let description = AFile::extract_exif_description(&Some(exif));
        assert_eq!(description.artist.as_deref(), Some("Alice"));
        assert_eq!(description.copyright.as_deref(), Some("2026 Co"));
        assert_eq!(description.description.as_deref(), Some("Sunset"));
        assert_eq!(description.user_comment.as_deref(), Some("prompt text"));
    }

    #[test]
    fn exif_fixture_drives_capture_extraction() {
        let fixture = identity_tiff_fixture();
        let exif = AFile::read_image_exif("missing.jpg", 1, Some(&fixture))
            .expect("capture TIFF EXIF should parse");
        let capture = AFile::extract_exif_capture(&Some(exif));
        assert_eq!(capture.lens_make.as_deref(), Some("Canon"));
        assert_eq!(capture.lens_model.as_deref(), Some("RF50"));
        assert_eq!(capture.exposure_time.as_deref(), Some("1/125 s"));
        assert_eq!(capture.f_number.as_deref(), Some("f/2.8"));
        assert_eq!(capture.exposure_bias.as_deref(), Some("0 EV"));
        assert_eq!(capture.focal_length.as_deref(), Some("50 mm"));
        assert_eq!(capture.iso_speed.as_deref(), Some("200"));
    }

    #[test]
    fn binary_exif_fallback_collects_all_requested_fields_in_one_pass() {
        let fallback = AFile::scrape_binary_exif_fallback(&identity_tiff_fixture());

        assert_eq!(fallback.make.as_deref(), Some("Canon"));
        assert_eq!(fallback.model.as_deref(), Some("EOS R"));
        assert_eq!(
            fallback.date_time_original.as_deref(),
            Some("2026:07:27 13:45:00")
        );
        assert_eq!(fallback.software.as_deref(), Some("PicAiPic"));
        assert_eq!(fallback.lens_make.as_deref(), Some("Canon"));
        assert_eq!(fallback.lens_model.as_deref(), Some("RF50"));
    }

    #[test]
    fn binary_exif_profile_skips_entry_scan_without_tiff_signature() {
        let mut profile = BinaryExifFallbackProfile::default();

        assert!(
            AFile::scrape_binary_exif_fallback_profiled(b"not a TIFF header", Some(&mut profile),)
                .make
                .is_none()
        );
        assert_eq!(profile.tiff_signature_attempts, 1);
        assert_eq!(profile.tiff_bases_found, 0);
        assert_eq!(profile.entry_scan_attempts, 0);
    }

    #[test]
    fn binary_exif_fallback_skips_complete_jpeg_without_exif() {
        let jpeg = [
            0xff, 0xd8, // SOI
            0xff, 0xe0, 0x00, 0x02, // empty APP0
            0xff, 0xda, // SOS
        ];

        assert!(!AFile::should_scrape_binary_exif_fallback(&jpeg));
        assert!(AFile::should_scrape_binary_exif_fallback(
            &identity_tiff_fixture()
        ));
    }

    #[test]
    fn raw_metadata_overlay_fills_only_missing_fields() {
        let mut make = Some("EXIF make".to_string());
        let mut model = None;
        let mut software = None;
        let mut artist = None;
        let mut description = None;
        let mut iso_speed = None;
        let mut exposure_time = Some("1/125 s".to_string());
        let mut f_number = None;
        let mut focal_length = None;
        let mut flash = None;
        let mut lens_make = None;
        let mut lens_model = None;
        let mut taken_date = Some(7);
        RawMetadataTarget {
            make: &mut make,
            model: &mut model,
            software: &mut software,
            artist: &mut artist,
            description: &mut description,
            iso_speed: &mut iso_speed,
            exposure_time: &mut exposure_time,
            f_number: &mut f_number,
            focal_length: &mut focal_length,
            flash: &mut flash,
            lens_make: &mut lens_make,
            lens_model: &mut lens_model,
            taken_date: &mut taken_date,
            modified_at: Some(7),
        }
        .apply(RawMeta {
            make: Some("RAW make".into()),
            model: Some("RAW model".into()),
            software: Some("RAW software".into()),
            artist: Some("RAW artist".into()),
            description: Some("RAW description".into()),
            timestamp: Some(9),
            iso_speed: Some("400".into()),
            shutter: Some("1/30 s".into()),
            aperture: Some("f/4".into()),
            focal_len: Some("35 mm".into()),
            flash_used: Some("Not fired".into()),
            lens_make: Some("RAW lens make".into()),
            lens_model: Some("RAW lens model".into()),
        });

        assert_eq!(make.as_deref(), Some("EXIF make"));
        assert_eq!(exposure_time.as_deref(), Some("1/125 s"));
        assert_eq!(model.as_deref(), Some("RAW model"));
        assert_eq!(iso_speed.as_deref(), Some("400"));
        assert_eq!(lens_model.as_deref(), Some("RAW lens model"));
        assert_eq!(taken_date, Some(9));
    }

    #[test]
    fn raw_metadata_overlay_keeps_exif_taken_date() {
        let mut make = None;
        let mut model = None;
        let mut software = None;
        let mut artist = None;
        let mut description = None;
        let mut iso_speed = None;
        let mut exposure_time = None;
        let mut f_number = None;
        let mut focal_length = None;
        let mut flash = None;
        let mut lens_make = None;
        let mut lens_model = None;
        let mut taken_date = Some(8);
        RawMetadataTarget {
            make: &mut make,
            model: &mut model,
            software: &mut software,
            artist: &mut artist,
            description: &mut description,
            iso_speed: &mut iso_speed,
            exposure_time: &mut exposure_time,
            f_number: &mut f_number,
            focal_length: &mut focal_length,
            flash: &mut flash,
            lens_make: &mut lens_make,
            lens_model: &mut lens_model,
            taken_date: &mut taken_date,
            modified_at: Some(7),
        }
        .apply(RawMeta {
            make: None,
            model: None,
            software: None,
            artist: None,
            description: None,
            timestamp: Some(9),
            iso_speed: None,
            shutter: None,
            aperture: None,
            focal_len: None,
            flash_used: None,
            lens_make: None,
            lens_model: None,
        });
        assert_eq!(taken_date, Some(8));
    }

    #[test]
    fn afile_crud_round_trip_uses_temporary_sqlite_fixture() {
        let (mut conn, db_path, media_path) = fixture();
        let path = media_path.to_string_lossy();
        let mut file = AFile::new(1, &path, 0).expect("build fixture AFile");

        assert_eq!(file.insert_with_conn(&conn).unwrap(), 1);
        let file_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO athumbs (file_id, error_code) VALUES (?1, 0)",
            [file_id],
        )
        .unwrap();

        file.name = "renamed.txt".into();
        file.rating = Some(4);
        file.gps_latitude = Some(12.5);
        assert_eq!(AFile::update_with_conn(&conn, file_id, &file).unwrap(), 1);

        let (name, rating, latitude): (String, i32, f64) = conn
            .query_row(
                "SELECT name, rating, gps_latitude FROM afiles WHERE id = ?1",
                [file_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "renamed.txt");
        assert_eq!(rating, 4);
        assert!((latitude - 12.5).abs() < f64::EPSILON);

        assert_eq!(AFile::delete_with_conn(&mut conn, file_id).unwrap(), 1);
        assert!(
            conn.query_row(
                "SELECT COUNT(*) FROM afiles WHERE id = ?1",
                [file_id],
                |row| { row.get::<_, i64>(0) }
            )
            .unwrap()
                == 0
        );
        assert!(
            conn.query_row(
                "SELECT COUNT(*) FROM athumbs WHERE file_id = ?1",
                [file_id],
                |row| { row.get::<_, i64>(0) }
            )
            .unwrap()
                == 0
        );

        drop(conn);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(media_path);
    }

    #[test]
    fn seen_batch_updates_all_requested_rows_in_one_transaction() {
        let (mut conn, db_path, media_path) = fixture();
        let path = media_path.to_string_lossy();
        let first = AFile::new(1, &path, 0).expect("build first fixture AFile");
        assert_eq!(first.insert_with_conn(&conn).unwrap(), 1);
        let first_id = conn.last_insert_rowid();

        let mut second = AFile::new(1, &path, 0).expect("build second fixture AFile");
        second.name = "second.txt".into();
        assert_eq!(second.insert_with_conn(&conn).unwrap(), 1);
        let second_id = conn.last_insert_rowid();

        assert_eq!(
            AFile::mark_seen_batch_with_conn(&mut conn, &[first_id, second_id], 123).unwrap(),
            2
        );
        let times: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT last_scan_time FROM afiles ORDER BY id")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(times, vec![123, 123]);

        drop(conn);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(media_path);
    }

    #[test]
    fn scan_state_cache_loads_and_hits_existing_rows() {
        let (conn, db_path, media_path) = fixture();
        let path = media_path.to_string_lossy();
        let mut file = AFile::new(1, &path, 0).expect("build fixture AFile");
        file.e_orientation = Some(6);
        file.width = Some(640);
        file.height = Some(480);
        file.duration = Some(12);
        assert_eq!(file.insert_with_conn(&conn).unwrap(), 1);
        let file_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO athumbs (file_id, error_code) VALUES (?1, 0)",
            [file_id],
        )
        .unwrap();
        conn.execute("UPDATE afiles SET embeds = x'01' WHERE id = ?1", [file_id])
            .unwrap();

        let mut cache = AFile::load_scan_file_state_cache_for_album_with_conn(&conn, 1).unwrap();
        assert_eq!(cache.len(), 1);
        let state = cache.get(&(1, file.name.clone())).unwrap();
        assert_eq!(state.id, file_id);
        assert!(state.has_thumbnail);
        assert!(state.has_embedding);
        assert_eq!(state.orientation, 6);
        assert_eq!(
            (state.width, state.height, state.duration),
            (640, 480, Some(12))
        );

        let mut profile = AFileAddProfile::default();
        let result =
            AFile::add_to_db_for_scan_with_state_cache(1, &path, 0, 123, &mut cache, &mut profile)
                .unwrap();
        assert!(result.cache_hit);
        assert_eq!(result.file_id, file_id);
        assert_eq!(result.deferred_seen_file_id, Some(file_id));

        drop(conn);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(media_path);
    }

    #[test]
    fn embedding_batch_writes_all_rows_in_one_transaction() {
        let (mut conn, db_path, media_path) = fixture();
        let path = media_path.to_string_lossy();
        let first = AFile::new(1, &path, 0).expect("build first fixture AFile");
        assert_eq!(first.insert_with_conn(&conn).unwrap(), 1);
        let first_id = conn.last_insert_rowid();

        let mut second = AFile::new(1, &path, 0).expect("build second fixture AFile");
        second.name = "second.txt".into();
        assert_eq!(second.insert_with_conn(&conn).unwrap(), 1);
        let second_id = conn.last_insert_rowid();

        assert_eq!(
            AFile::update_embeddings_with_conn(
                &mut conn,
                &[(first_id, vec![0.5, -1.0]), (second_id, vec![2.0])],
            )
            .unwrap(),
            2
        );
        let first_bytes: Vec<u8> = conn
            .query_row(
                "SELECT embeds FROM afiles WHERE id = ?1",
                [first_id],
                |row| row.get(0),
            )
            .unwrap();
        let second_bytes: Vec<u8> = conn
            .query_row(
                "SELECT embeds FROM afiles WHERE id = ?1",
                [second_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            first_bytes,
            [0.5f32.to_le_bytes(), (-1.0f32).to_le_bytes()].concat()
        );
        assert_eq!(second_bytes, 2.0f32.to_le_bytes().to_vec());

        drop(conn);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(media_path);
    }
}

/// Define the album thumbnail struct
#[derive(Debug, Serialize, Deserialize)]
pub struct AThumb {
    pub id: Option<i64>, // unique id (autoincrement by db)
    pub file_id: i64,    // file id (from files table)
    pub error_code: i64, // error code (0: success, 1: error, 2: use original)

    #[serde(skip)]
    pub thumb_data: Option<Vec<u8>>, // thumbnail data (store into db as BLOB)

    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_mtime: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,

    // output only
    pub thumb_data_base64: Option<String>, // fetch thumbnail data as base64 string (for webview)
}

impl AThumb {
    fn should_use_original_image_for_library(
        file_id: i64,
        file_type: i64,
        thumbnail_size: u32,
        library_id: &str,
    ) -> bool {
        AFile::get_file_info_for_library(file_id, library_id)
            .ok()
            .flatten()
            .map(|file| Self::should_use_original_image_for_file(&file, file_type, thumbnail_size))
            .unwrap_or(false)
    }

    fn should_use_original_image_for_file(
        file: &AFile,
        file_type: i64,
        thumbnail_size: u32,
    ) -> bool {
        if file_type != 1 || thumbnail_size == 0 {
            return false;
        }

        #[cfg(target_os = "linux")]
        if file
            .file_path
            .as_deref()
            .is_some_and(|path| path.to_ascii_lowercase().ends_with(".avif"))
        {
            return false;
        }

        if file
            .file_path
            .as_deref()
            .is_some_and(t_image::is_ffmpeg_backed_image_path)
        {
            return false;
        }

        let width = file.width.unwrap_or(0).max(0) as u32;
        let height = file.height.unwrap_or(0).max(0) as u32;
        width > 0 && height > 0 && width <= thumbnail_size && height <= thumbnail_size
    }

    fn is_png_bytes(data: &[u8]) -> bool {
        data.starts_with(&[0x89, 0x50, 0x4E, 0x47])
    }

    fn is_complete_jpeg(data: &[u8]) -> bool {
        data.starts_with(&[0xFF, 0xD8, 0xFF]) && data.ends_with(&[0xFF, 0xD9])
    }

    fn generation_lock_key(file_id: i64, thumbnail_size: u32) -> String {
        format!("{}:{}", file_id, thumbnail_size)
    }

    fn acquire_generation_guard(file_id: i64, thumbnail_size: u32) -> ThumbGenerationGuard {
        let key = Self::generation_lock_key(file_id, thumbnail_size);
        let locks = thumb_generation_locks();
        let mut active = locks.active.lock().unwrap_or_else(|e| e.into_inner());

        loop {
            if !active.contains(&key) {
                active.insert(key.clone());
                return ThumbGenerationGuard { key };
            }

            active = locks
                .available
                .wait(active)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    pub(crate) fn try_begin_background_task(file_id: i64, thumbnail_size: u32) -> bool {
        let key = Self::generation_lock_key(file_id, thumbnail_size);
        let Ok(mut tasks) = thumb_background_tasks().lock() else {
            return false;
        };
        if tasks.contains(&key) {
            return false;
        }
        tasks.insert(key);
        true
    }

    pub(crate) fn finish_background_task(file_id: i64, thumbnail_size: u32) {
        let key = Self::generation_lock_key(file_id, thumbnail_size);
        if let Ok(mut tasks) = thumb_background_tasks().lock() {
            tasks.remove(&key);
        }
    }

    fn now_ts() -> i64 {
        chrono::Utc::now().timestamp()
    }

    fn get_source_mtime(file_path: &str) -> Option<i64> {
        fs::metadata(file_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    }

    fn get_current_library_id() -> String {
        t_config::load_app_config()
            .map(|c| c.current_library_id)
            .unwrap_or_else(|_| "default".to_string())
    }

    fn build_thumb_key(
        library_id: &str,
        file_id: i64,
        thumbnail_size: u32,
        source_mtime: Option<i64>,
        orientation: i32,
    ) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"lap-thumb-v1");
        hasher.update(library_id.as_bytes());
        hasher.update(&file_id.to_le_bytes());
        hasher.update(&thumbnail_size.to_le_bytes());
        hasher.update(&orientation.to_le_bytes());
        hasher.update(&source_mtime.unwrap_or_default().to_le_bytes());
        hasher.finalize().to_hex().to_string()
    }

    fn get_file_album_id(file_id: i64) -> Result<Option<i64>, String> {
        AFile::get_file_info(file_id)
            .map(|file| file.and_then(|f| f.album_id))
            .map_err(|e| e.to_string())
    }

    fn get_file_album_id_for_library(
        file_id: i64,
        library_id: &str,
    ) -> Result<Option<i64>, String> {
        AFile::get_file_info_for_library(file_id, library_id)
            .map(|file| file.and_then(|f| f.album_id))
            .map_err(|e| e.to_string())
    }

    fn get_thumb_cache_path_for_key(
        library_id: &str,
        album_id: i64,
        thumb_key: &str,
    ) -> Result<PathBuf, String> {
        if thumb_key.len() < 2 {
            return Err("Invalid thumbnail cache key".to_string());
        }

        let cache_root = t_config::get_app_cache_dir()?
            .join(library_id)
            .join(album_id.to_string());
        Ok(cache_root
            .join(&thumb_key[0..2])
            .join(format!("{}.jpg", thumb_key)))
    }

    fn read_thumb_cache_bytes(
        library_id: &str,
        album_id: i64,
        thumb_key: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let path = Self::get_thumb_cache_path_for_key(library_id, album_id, thumb_key)?;
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(path).map_err(|e| e.to_string())?;
        Ok(Self::is_complete_jpeg(&data).then_some(data))
    }

    fn write_thumb_cache_bytes(
        library_id: &str,
        album_id: i64,
        thumb_key: &str,
        data: &[u8],
    ) -> Result<PathBuf, String> {
        let path = Self::get_thumb_cache_path_for_key(library_id, album_id, thumb_key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if !Self::is_complete_jpeg(data) {
            return Err("Invalid JPEG thumbnail data".to_string());
        }

        let temp_path = path.with_extension(format!(
            "{}.{}.tmp",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(&temp_path, data).map_err(|e| e.to_string())?;
        if let Err(first_error) = fs::rename(&temp_path, &path) {
            // Windows does not replace an existing destination with rename.
            if !path.exists()
                || fs::remove_file(&path).is_err()
                || fs::rename(&temp_path, &path).is_err()
            {
                let _ = fs::remove_file(&temp_path);
                return Err(first_error.to_string());
            }
        }
        Ok(path)
    }

    fn delete_thumb_cache_for_key(library_id: &str, album_id: i64, thumb_key: &str) {
        if let Ok(path) = Self::get_thumb_cache_path_for_key(library_id, album_id, thumb_key) {
            let _ = fs::remove_file(path);
        }
    }

    pub fn relocate_for_file(
        file_id: i64,
        old_album_id: i64,
        new_album_id: i64,
    ) -> Result<(), String> {
        if old_album_id == new_album_id {
            return Ok(());
        }

        let Some(thumb_key) = Self::fetch_thumb_key(file_id)? else {
            return Ok(());
        };

        let library_id = Self::get_current_library_id();
        let old_path = Self::get_thumb_cache_path_for_key(&library_id, old_album_id, &thumb_key)?;
        if !old_path.exists() {
            return Ok(());
        }

        let new_path = Self::get_thumb_cache_path_for_key(&library_id, new_album_id, &thumb_key)?;
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        match fs::rename(&old_path, &new_path) {
            Ok(_) => Ok(()),
            Err(_) => {
                fs::copy(&old_path, &new_path).map_err(|e| e.to_string())?;
                let _ = fs::remove_file(old_path);
                Ok(())
            }
        }
    }

    /// Create a new thumbnail struct
    fn new_for_library(
        file_id: i64,
        file_path: &str,
        file_type: i64,
        orientation: i32,
        thumbnail_size: u32,
        library_id: &str,
        known_duration: Option<u64>,
        seek_percent: Option<u8>,
    ) -> Result<Option<Self>, String> {
        let (thumb_data, error_code) = match file_type {
            1 => {
                // image
                if let Some(ext) = t_utils::get_file_extension(file_path) {
                    match ext.to_lowercase().as_str() {
                        "heic" | "heif" | "hif" => {
                            // heic/heif/hif
                            #[cfg(target_os = "macos")]
                            match t_image::get_heic_thumbnail_with_sips(file_path, thumbnail_size) {
                                Ok(Some(data)) => (Some(data), 0),
                                Ok(None) => (None, 1), // empty thumb
                                Err(_) => (None, 1),   // error
                            }
                            #[cfg(all(not(target_os = "macos"), lap_has_libheif))]
                            match crate::t_heif::get_heif_thumbnail(
                                file_path,
                                orientation,
                                thumbnail_size,
                            ) {
                                Ok(Some(data)) => (Some(data), 0),
                                Ok(None) => (None, 1), // empty thumb
                                Err(_) => (None, 1),   // error
                            }
                            #[cfg(all(not(target_os = "macos"), not(lap_has_libheif)))]
                            match t_video::get_video_thumbnail_sync(
                                file_path,
                                thumbnail_size,
                                known_duration,
                                None,
                            ) {
                                Ok(Some(data)) => (Some(data), 0),
                                Ok(None) => (None, 1), // empty thumb
                                Err(_) => (None, 1),   // error
                            }
                        }
                        _ => {
                            // other images
                            match t_image::get_image_thumbnail(
                                file_path,
                                orientation,
                                thumbnail_size,
                            ) {
                                Ok(Some(data)) => (Some(data), 0),
                                Ok(None) => (None, 1),
                                Err(_) => (None, 1),
                            }
                        }
                    }
                } else {
                    (None, 1)
                }
            }
            2 => {
                // video
                match t_video::get_video_thumbnail_sync(
                    file_path,
                    thumbnail_size,
                    known_duration,
                    seek_percent,
                ) {
                    Ok(Some(data)) => (Some(data), 0),
                    Ok(None) => (None, 1),
                    Err(_) => (None, 1),
                }
            }
            3 => {
                // raw image
                match t_image::get_raw_thumbnail(file_path, orientation, thumbnail_size) {
                    Ok(Some(data)) => (Some(data), 0),
                    Ok(None) => (None, 1),
                    Err(_) => (None, 1),
                }
            }
            _ => (None, 1),
        };

        let thumb_mtime = Self::get_source_mtime(file_path);
        let thumb_key = thumb_data.as_ref().map(|_| {
            Self::build_thumb_key(
                library_id,
                file_id,
                thumbnail_size,
                thumb_mtime,
                orientation,
            )
        });

        Ok(Some(Self {
            id: None,
            file_id,
            error_code,
            thumb_data,
            thumb_key,
            thumb_mtime,
            thumb_size: Some(thumbnail_size as i64),
            updated_at: Some(Self::now_ts()),
            thumb_data_base64: None,
        }))
    }

    // pub fn new(
    //     file_id: i64,
    //     file_path: &str,
    //     file_type: i64,
    //     orientation: i32,
    //     thumbnail_size: u32,
    // ) -> Result<Option<Self>, String> {
    //     let library_id = Self::get_current_library_id();
    //     Self::new_for_library(
    //         file_id,
    //         file_path,
    //         file_type,
    //         orientation,
    //         thumbnail_size,
    //         &library_id,
    //     )
    // }

    fn insert_for_library(&self, library_id: &str) -> Result<usize, String> {
        let conn = open_conn_for_library(library_id)?;
        self.insert_with_conn(&conn)
    }

    fn insert_with_conn(&self, conn: &Connection) -> Result<usize, String> {
        let result = conn
            .execute(
                "INSERT OR REPLACE INTO athumbs (file_id, error_code, thumb_data, thumb_key, thumb_mtime, thumb_size, updated_at) 
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    self.file_id,
                    self.error_code,
                    self.thumb_data,
                    self.thumb_key,
                    self.thumb_mtime,
                    self.thumb_size,
                    self.updated_at,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(result) // 0: already exists, ignore, 1: inserted
    }

    fn hydrate_output_bytes_for_library(mut thumb: Self, library_id: &str) -> Result<Self, String> {
        if thumb.thumb_data.is_none() {
            if let Some(key) = thumb.thumb_key.as_ref() {
                if let Some(album_id) =
                    Self::get_file_album_id_for_library(thumb.file_id, library_id)?
                {
                    thumb.thumb_data = Self::read_thumb_cache_bytes(library_id, album_id, key)?;
                }
            }
        }
        thumb.thumb_data_base64 = thumb
            .thumb_data
            .as_ref()
            .map(|data| general_purpose::STANDARD.encode(data));
        Ok(thumb)
    }

    /// fetch a thumbnail from db by file_id
    pub fn fetch(file_id: i64) -> Result<Option<Self>, String> {
        let library_id = Self::get_current_library_id();
        Self::fetch_for_library(file_id, &library_id)
    }

    pub fn fetch_for_library(file_id: i64, library_id: &str) -> Result<Option<Self>, String> {
        let conn = open_conn_for_library(library_id)?;
        let result = conn
            .query_row(
                "SELECT id, file_id, error_code, thumb_data, thumb_key, thumb_mtime, thumb_size, updated_at
                FROM athumbs WHERE file_id = ?1",
                params![file_id],
                |row| {
                    Ok(Self {
                        id: Some(row.get(0)?),
                        file_id: row.get(1)?,
                        error_code: row.get(2)?,
                        thumb_data: row.get(3)?,
                        thumb_key: row.get(4)?,
                        thumb_mtime: row.get(5)?,
                        thumb_size: row.get(6)?,
                        updated_at: row.get(7)?,
                        thumb_data_base64: None,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        result
            .map(|thumb| Self::hydrate_output_bytes_for_library(thumb, library_id))
            .transpose()
    }

    pub fn fetch_many(file_ids: &[i64]) -> Result<HashMap<i64, Self>, String> {
        let library_id = Self::get_current_library_id();
        Self::fetch_many_for_library(file_ids, &library_id)
    }

    pub fn fetch_many_for_library(
        file_ids: &[i64],
        library_id: &str,
    ) -> Result<HashMap<i64, Self>, String> {
        if file_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = std::iter::repeat("?")
            .take(file_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT id, file_id, error_code, thumb_data, thumb_key, thumb_mtime, thumb_size, updated_at
            FROM athumbs WHERE file_id IN ({})",
            placeholders
        );
        let conn = open_conn_for_library(library_id)?;
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params_from_iter(file_ids.iter()), |row| {
                Ok(Self {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    error_code: row.get(2)?,
                    thumb_data: row.get(3)?,
                    thumb_key: row.get(4)?,
                    thumb_mtime: row.get(5)?,
                    thumb_size: row.get(6)?,
                    updated_at: row.get(7)?,
                    thumb_data_base64: None,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut thumbs = HashMap::with_capacity(file_ids.len());
        for row in rows {
            let thumb = Self::hydrate_output_bytes_for_library(
                row.map_err(|e| e.to_string())?,
                library_id,
            )?;
            thumbs.insert(thumb.file_id, thumb);
        }
        Ok(thumbs)
    }

    fn is_stale(&self, file_path: &str, thumbnail_size: u32) -> bool {
        if self.thumb_size != Some(thumbnail_size as i64) {
            return true;
        }

        let current_mtime = Self::get_source_mtime(file_path);
        match (self.thumb_mtime, current_mtime) {
            (Some(cached_mtime), Some(source_mtime)) => cached_mtime != source_mtime,
            (None, Some(_)) => true,
            // The album root may have been temporarily renamed outside Lap.
            // Keep existing thumbnails so they work again when the path returns.
            (_, None) => false,
        }
    }

    fn fetch_thumb_key(file_id: i64) -> Result<Option<String>, String> {
        let conn = open_conn()?;
        conn.query_row(
            "SELECT thumb_key FROM athumbs WHERE file_id = ?1",
            params![file_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())
    }

    fn persist_cache_and_clear_blob(
        mut thumbnail: Self,
        file_path: &str,
        thumbnail_size: u32,
        orientation: i32,
    ) -> Result<Self, String> {
        let Some(data) = thumbnail.thumb_data.as_ref() else {
            return Self::hydrate_output_bytes_for_library(
                thumbnail,
                &Self::get_current_library_id(),
            );
        };

        if Self::is_png_bytes(data) {
            return Ok(Self {
                thumb_data: None,
                thumb_key: None,
                thumb_data_base64: None,
                ..thumbnail
            });
        }

        let library_id = Self::get_current_library_id();
        let thumb_mtime = Self::get_source_mtime(file_path);
        let now = Self::now_ts();
        let thumb_key = thumbnail.thumb_key.clone().unwrap_or_else(|| {
            Self::build_thumb_key(
                &library_id,
                thumbnail.file_id,
                thumbnail_size,
                thumb_mtime,
                orientation,
            )
        });

        let album_id = Self::get_file_album_id(thumbnail.file_id)?
            .ok_or_else(|| format!("Album not found for thumbnail file: {}", thumbnail.file_id))?;
        Self::write_thumb_cache_bytes(&library_id, album_id, &thumb_key, data)?;

        let conn = open_conn()?;
        conn.execute(
            "UPDATE athumbs
            SET thumb_key = ?2, thumb_mtime = ?3, thumb_size = ?4, updated_at = ?5, thumb_data = NULL
            WHERE file_id = ?1",
            params![
                thumbnail.file_id,
                thumb_key,
                thumb_mtime,
                thumbnail_size as i64,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

        thumbnail.thumb_key = Some(thumb_key);
        thumbnail.thumb_mtime = thumb_mtime;
        thumbnail.thumb_size = Some(thumbnail_size as i64);
        thumbnail.updated_at = Some(now);
        Self::hydrate_output_bytes_for_library(thumbnail, &library_id)
    }

    fn ensure_cached(
        thumbnail: Self,
        file_path: &str,
        thumbnail_size: u32,
        orientation: i32,
    ) -> Result<Self, String> {
        if thumbnail.error_code != 0 {
            return Ok(thumbnail);
        }

        if thumbnail.thumb_data.is_some() {
            return Self::persist_cache_and_clear_blob(
                thumbnail,
                file_path,
                thumbnail_size,
                orientation,
            );
        }

        if thumbnail.thumb_key.is_some() {
            return Self::hydrate_output_bytes_for_library(
                thumbnail,
                &Self::get_current_library_id(),
            );
        }

        Ok(thumbnail)
    }

    fn create_cache_backed_thumb_for_library(
        file_id: i64,
        file_path: &str,
        file_type: i64,
        orientation: i32,
        thumbnail_size: u32,
        library_id: &str,
        known_duration: Option<u64>,
        seek_percent: Option<u8>,
    ) -> Result<Option<Self>, String> {
        if Self::should_use_original_image_for_library(
            file_id,
            file_type,
            thumbnail_size,
            library_id,
        ) {
            let athumb = Self {
                id: None,
                file_id,
                error_code: 2,
                thumb_data: None,
                thumb_key: None,
                thumb_mtime: Self::get_source_mtime(file_path),
                thumb_size: Some(thumbnail_size as i64),
                updated_at: Some(Self::now_ts()),
                thumb_data_base64: None,
            };
            athumb.insert_for_library(library_id)?;
            return Self::fetch_for_library(file_id, library_id);
        }

        let mut athumb = match Self::new_for_library(
            file_id,
            file_path,
            file_type,
            orientation,
            thumbnail_size,
            library_id,
            known_duration,
            seek_percent,
        ) {
            Ok(Some(athumb)) => athumb,
            _ => Self {
                id: None,
                file_id,
                error_code: 1,
                thumb_data: None,
                thumb_key: None,
                thumb_mtime: Self::get_source_mtime(file_path),
                thumb_size: Some(thumbnail_size as i64),
                updated_at: Some(Self::now_ts()),
                thumb_data_base64: None,
            },
        };

        if athumb.error_code == 0 {
            if let (Some(data), Some(key)) = (athumb.thumb_data.as_ref(), athumb.thumb_key.as_ref())
            {
                let album_id = Self::get_file_album_id_for_library(file_id, library_id)?
                    .ok_or_else(|| format!("Album not found for thumbnail file: {}", file_id))?;
                Self::write_thumb_cache_bytes(library_id, album_id, key, data)?;
                athumb.thumb_data = None;
            }
        }

        athumb.insert_for_library(library_id)?;
        Self::fetch_for_library(file_id, library_id)
    }

    fn create_cache_backed_thumb(
        file_id: i64,
        file_path: &str,
        file_type: i64,
        orientation: i32,
        thumbnail_size: u32,
        known_duration: Option<u64>,
        seek_percent: Option<u8>,
    ) -> Result<Option<Self>, String> {
        let library_id = Self::get_current_library_id();
        Self::create_cache_backed_thumb_for_library(
            file_id,
            file_path,
            file_type,
            orientation,
            thumbnail_size,
            &library_id,
            known_duration,
            seek_percent,
        )
    }

    pub fn get_thumb_if_available(
        file_id: i64,
        file_path: &str,
        thumbnail_size: u32,
        orientation: i32,
        force_regenerate: bool,
    ) -> Result<Option<Self>, String> {
        if force_regenerate {
            let _ = Self::delete(file_id);
            return Ok(None);
        }

        if let Ok(Some(thumbnail)) = Self::fetch(file_id) {
            if thumbnail.error_code == 1 {
                if thumbnail.is_stale(file_path, thumbnail_size) {
                    let _ = Self::delete(file_id);
                    return Ok(None);
                }
                return Ok(Some(thumbnail));
            }

            if thumbnail.error_code == 2 {
                return Ok(Some(thumbnail));
            }

            if thumbnail.is_stale(file_path, thumbnail_size) {
                let _ = Self::delete(file_id);
                return Ok(None);
            }

            let hydrated = Self::ensure_cached(thumbnail, file_path, thumbnail_size, orientation)?;
            if hydrated.thumb_data.is_some() {
                return Ok(Some(hydrated));
            }

            let _ = Self::delete(file_id);
        }

        Ok(None)
    }

    pub fn resolve_fetched_thumb_if_available(
        thumbnail: Self,
        file_path: &str,
        thumbnail_size: u32,
        orientation: i32,
        force_regenerate: bool,
    ) -> Result<Option<Self>, String> {
        if force_regenerate {
            let _ = Self::delete(thumbnail.file_id);
            return Ok(None);
        }

        if thumbnail.error_code == 1 {
            if thumbnail.is_stale(file_path, thumbnail_size) {
                let _ = Self::delete(thumbnail.file_id);
                return Ok(None);
            }
            return Ok(Some(thumbnail));
        }

        if thumbnail.error_code == 2 {
            return Ok(Some(thumbnail));
        }

        if thumbnail.is_stale(file_path, thumbnail_size) {
            let _ = Self::delete(thumbnail.file_id);
            return Ok(None);
        }

        let hydrated = Self::ensure_cached(thumbnail, file_path, thumbnail_size, orientation)?;
        if hydrated.thumb_data.is_some() {
            return Ok(Some(hydrated));
        }

        let _ = Self::delete(hydrated.file_id);
        Ok(None)
    }

    pub fn schedule_background_generation_for_library(
        app_handle: tauri::AppHandle,
        file_id: i64,
        file_path: String,
        file_type: i64,
        orientation: i32,
        thumbnail_size: u32,
        album_id: i64,
        force_regenerate: bool,
        seek_percent: Option<u8>,
    ) {
        if !Self::try_begin_background_task(file_id, thumbnail_size) {
            return;
        }

        tauri::async_runtime::spawn(async move {
            let generated = tauri::async_runtime::spawn_blocking(move || {
                let duration = if file_type == 2 {
                    AFile::get_file_info(file_id)
                        .ok()
                        .flatten()
                        .and_then(|f| f.duration.map(|d| d as u64))
                } else {
                    None
                };

                Self::get_or_create_thumb(
                    file_id,
                    &file_path,
                    file_type,
                    orientation,
                    thumbnail_size,
                    force_regenerate,
                    duration,
                    seek_percent,
                )
            })
            .await;

            if matches!(generated, Ok(Ok(Some(_)))) && album_id > 0 {
                let _ = app_handle.emit(
                    "thumbnail_ready",
                    serde_json::json!({
                        "album_id": album_id,
                        "file_ids": [file_id],
                    }),
                );
            }

            Self::finish_background_task(file_id, thumbnail_size);
        });
    }

    /// get or create a thumbnail
    pub fn get_or_create_thumb(
        file_id: i64,
        file_path: &str,
        file_type: i64,
        orientation: i32,
        thumbnail_size: u32,
        force_regenerate: bool,
        known_duration: Option<u64>,
        seek_percent: Option<u8>,
    ) -> Result<Option<Self>, String> {
        if force_regenerate {
            let _ = Self::delete(file_id);
        } else if let Some(thumb) =
            Self::get_thumb_if_available(file_id, file_path, thumbnail_size, orientation, false)?
        {
            if thumb.error_code != 1 {
                return Ok(Some(thumb));
            }
        }

        let _generation_guard = Self::acquire_generation_guard(file_id, thumbnail_size);

        if !force_regenerate {
            if let Some(hydrated) = Self::get_thumb_if_available(
                file_id,
                file_path,
                thumbnail_size,
                orientation,
                false,
            )? {
                if hydrated.error_code != 1 {
                    return Ok(Some(hydrated));
                }
            }
        }

        Self::create_cache_backed_thumb(
            file_id,
            file_path,
            file_type,
            orientation,
            thumbnail_size,
            known_duration,
            seek_percent,
        )
    }

    /// fetch raw thumbnail bytes for protocol handler
    pub fn fetch_raw_for_library(
        file_id: i64,
        library_id: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let thumb = Self::fetch_for_library(file_id, library_id)?;

        // error_code 2: image is small enough to use the original file directly
        if let Some(ref thumb) = thumb {
            if thumb.error_code == 2 {
                if let Ok(Some(file)) = AFile::get_file_info_for_library(file_id, library_id) {
                    if let Some(ref file_path) = file.file_path {
                        if let Ok(data) = std::fs::read(file_path) {
                            return Ok(Some(data));
                        }
                    }
                }
            }
        }

        if let Some(thumb) = thumb.filter(|t| t.error_code == 0) {
            if let Some(data) = thumb.thumb_data {
                return Ok(Some(data));
            }

            let file = AFile::get_file_info_for_library(file_id, library_id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("File not found for thumbnail: {}", file_id))?;
            let file_path = file
                .file_path
                .ok_or_else(|| format!("File path not found for thumbnail: {}", file_id))?;
            let file_type = file.file_type.unwrap_or(0);
            let orientation = file.e_orientation.unwrap_or(1) as i32;
            let thumbnail_size = thumb.thumb_size.unwrap_or(200).max(1) as u32;

            return Ok(Self::create_cache_backed_thumb_for_library(
                file_id,
                &file_path,
                file_type,
                orientation,
                thumbnail_size,
                library_id,
                file.duration.map(|d| d as u64),
                None,
            )?
            .and_then(|thumb| thumb.thumb_data));
        }

        let Some(file) = AFile::get_file_info_for_library(file_id, library_id)? else {
            return Ok(None);
        };
        let file_type = file.file_type.unwrap_or(0);
        let use_original = Self::should_use_original_image_for_file(&file, file_type, 200);
        let Some(file_path) = file.file_path else {
            return Ok(None);
        };
        if use_original {
            return std::fs::read(&file_path)
                .map(Some)
                .map_err(|e| e.to_string());
        }
        Ok(Self::create_cache_backed_thumb_for_library(
            file_id,
            &file_path,
            file_type,
            file.e_orientation.unwrap_or(1) as i32,
            200,
            library_id,
            file.duration.map(|d| d as u64),
            None,
        )?
        .and_then(|thumb| thumb.thumb_data))
    }

    /// delete a thumbnail from db
    pub fn delete(file_id: i64) -> Result<usize, String> {
        if let Ok(Some(key)) = Self::fetch_thumb_key(file_id) {
            let library_id = Self::get_current_library_id();
            if let Ok(Some(album_id)) = Self::get_file_album_id(file_id) {
                Self::delete_thumb_cache_for_key(&library_id, album_id, &key);
            }
        }
        let conn = open_conn()?;
        let result = conn
            .execute("DELETE FROM athumbs WHERE file_id = ?1", params![file_id])
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// get the thumbnail count of the folder
    pub fn get_folder_thumb_count(file_type: i64, folder_id: i64) -> Result<i64, String> {
        let conn = open_conn()?;

        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();

        conditions.push("a.folder_id = ?".to_string());
        params.push(&folder_id);

        if let Some(condition) = AFile::build_file_type_condition(file_type) {
            conditions.push(condition);
        }

        let mut query =
            "SELECT COUNT(b.id) FROM afiles a JOIN athumbs b ON a.id = b.file_id".to_string();
        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        let result = conn
            .query_row(&query, rusqlite::params_from_iter(params), |row| row.get(0))
            .map_err(|e| e.to_string())?;

        Ok(result)
    }
}

/// Define the Tag struct
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ATag {
    pub id: i64,
    pub name: String,
    pub count: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ATagSelectionCount {
    pub tag_id: i64,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ATagFileState {
    pub file_id: i64,
    pub has_tags: bool,
}

impl ATag {
    /// Function to construct `Self` from a database row
    fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            count: row.get(2)?,
        })
    }

    /// Add a new tag. If the tag already exists, return the existing one.
    pub fn add(name: &str) -> Result<Self, String> {
        let conn = open_conn()?;
        // First, try to fetch the tag to see if it already exists.
        let existing_tag = conn
            .query_row(
                "SELECT id, name, 0 as count FROM atags WHERE name = ?1",
                params![name],
                Self::from_row,
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some(tag) = existing_tag {
            Ok(tag)
        } else {
            // The tag doesn't exist, so insert it.
            conn.execute("INSERT INTO atags (name) VALUES (?1)", params![name])
                .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Self {
                id,
                name: name.to_string(),
                count: Some(0),
            })
        }
    }

    /// Get all tags from the db
    pub fn get_all(sort: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let order_clause = match sort {
            1 => "atags.name DESC",
            2 => "count ASC, atags.name ASC",
            3 => "count DESC, atags.name ASC",
            _ => "atags.name ASC",
        };
        let query = "SELECT atags.id, atags.name, SUM(CASE WHEN afiles.id IS NOT NULL THEN 1 ELSE 0 END) AS count 
            FROM atags 
            LEFT JOIN afile_tags ON atags.id = afile_tags.tag_id
            LEFT JOIN afiles ON afile_tags.file_id = afiles.id
            GROUP BY atags.id
            ORDER BY "
            .to_string()
            + order_clause;
        let mut stmt = conn.prepare(query.as_str()).map_err(|e| e.to_string())?;

        let tags_iter = stmt
            .query_map([], Self::from_row)
            .map_err(|e| e.to_string())?;

        let mut tags = Vec::new();
        for tag in tags_iter {
            tags.push(tag.map_err(|e| e.to_string())?);
        }
        Ok(tags)
    }

    /// Get tag name by id
    pub fn get_name(tag_id: i64) -> Result<String, String> {
        let conn = open_conn()?;
        let result = conn
            .query_row(
                "SELECT name FROM atags WHERE id = ?1",
                params![tag_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Get all tags for a specific file
    pub fn get_tags_for_file(file_id: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.name, 0 as count
                FROM atags t
                INNER JOIN afile_tags ft ON t.id = ft.tag_id
                WHERE ft.file_id = ?1
                ORDER BY t.name ASC",
            )
            .map_err(|e| e.to_string())?;

        let tags_iter = stmt
            .query_map(params![file_id], Self::from_row)
            .map_err(|e| e.to_string())?;

        let mut tags = Vec::new();
        for tag in tags_iter {
            tags.push(tag.map_err(|e| e.to_string())?);
        }
        Ok(tags)
    }

    /// Add a tag to a file.
    pub fn add_tag_to_file(file_id: i64, tag_id: i64) -> Result<(), String> {
        let conn = open_conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO afile_tags (file_id, tag_id) VALUES (?1, ?2)",
            params![file_id, tag_id],
        )
        .map_err(|e| e.to_string())?;

        // Update has_tags in afiles table
        conn.execute(
            "UPDATE afiles SET has_tags = 1 WHERE id = ?1",
            params![file_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Remove a tag from a file
    pub fn remove_tag_from_file(file_id: i64, tag_id: i64) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute(
                "DELETE FROM afile_tags WHERE file_id = ?1 AND tag_id = ?2",
                params![file_id, tag_id],
            )
            .map_err(|e| e.to_string())?;

        // Check if the file still has any tags
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM afile_tags WHERE file_id = ?1",
                params![file_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if count == 0 {
            // If no tags left, set has_tags to false
            conn.execute(
                "UPDATE afiles SET has_tags = 0 WHERE id = ?1",
                params![file_id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(result)
    }

    pub fn get_selection_counts(file_ids: &[i64]) -> Result<Vec<ATagSelectionCount>, String> {
        if file_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "CREATE TEMP TABLE IF NOT EXISTS selected_file_ids (id INTEGER PRIMARY KEY)",
            [],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM selected_file_ids", [])
            .map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare_cached("INSERT OR IGNORE INTO selected_file_ids (id) VALUES (?1)")
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                stmt.execute(params![file_id]).map_err(|e| e.to_string())?;
            }
        }

        let counts = {
            let mut stmt = tx
                .prepare(
                    "SELECT ft.tag_id, COUNT(*)
                     FROM afile_tags ft
                     INNER JOIN selected_file_ids selected ON selected.id = ft.file_id
                     GROUP BY ft.tag_id",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(ATagSelectionCount {
                        tag_id: row.get(0)?,
                        count: row.get(1)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };

        tx.execute("DELETE FROM selected_file_ids", [])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(counts)
    }

    pub fn apply_to_files(
        file_ids: &[i64],
        add_tag_ids: &[i64],
        remove_tag_ids: &[i64],
    ) -> Result<Vec<ATagFileState>, String> {
        if file_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = open_conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        {
            let mut add_stmt = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO afile_tags (file_id, tag_id) VALUES (?1, ?2)",
                )
                .map_err(|e| e.to_string())?;
            let mut remove_stmt = tx
                .prepare_cached("DELETE FROM afile_tags WHERE file_id = ?1 AND tag_id = ?2")
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                for tag_id in add_tag_ids {
                    add_stmt
                        .execute(params![file_id, tag_id])
                        .map_err(|e| e.to_string())?;
                }
                for tag_id in remove_tag_ids {
                    remove_stmt
                        .execute(params![file_id, tag_id])
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        let mut states = Vec::with_capacity(file_ids.len());
        {
            let mut update_stmt = tx
                .prepare_cached(
                    "UPDATE afiles
                 SET has_tags = EXISTS (
                     SELECT 1 FROM afile_tags WHERE afile_tags.file_id = afiles.id
                 )
                 WHERE id = ?1",
                )
                .map_err(|e| e.to_string())?;
            let mut state_stmt = tx
                .prepare_cached("SELECT COALESCE(has_tags, 0) FROM afiles WHERE id = ?1")
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                update_stmt
                    .execute(params![file_id])
                    .map_err(|e| e.to_string())?;
                let has_tags = state_stmt
                    .query_row(params![file_id], |row| row.get(0))
                    .map_err(|e| e.to_string())?;
                states.push(ATagFileState {
                    file_id: *file_id,
                    has_tags,
                });
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(states)
    }

    /// Delete a tag from the database. This will also remove all its associations with files.
    pub fn delete(tag_id: i64) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute("DELETE FROM atags WHERE id = ?1", params![tag_id])
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Rename a tag
    pub fn rename(tag_id: i64, new_name: &str) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute(
                "UPDATE atags SET name = ?1 WHERE id = ?2",
                params![new_name, tag_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }
}

/// Person struct for face recognition
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person {
    pub id: i64,
    pub name: Option<String>,
    pub count: Option<i64>,
    pub thumbnail: Option<String>, // Base64 encoded face thumbnail
}

impl Person {
    /// Get all persons with face counts and pre-stored thumbnail
    /// Optimized: single query, no runtime image processing
    pub fn get_all(sort: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;

        // Single query with JOIN for count, directly fetch pre-stored thumbnail
        let query = "
            SELECT p.id, p.name, COUNT(f.id) as count, p.thumbnail
            FROM persons p
            LEFT JOIN faces f ON f.person_id = p.id
            GROUP BY p.id
            ORDER BY {order_clause}
        ";
        let order_clause = match sort {
            1 => "p.name DESC",
            2 => "count ASC, p.name ASC",
            3 => "count DESC, p.name ASC",
            _ => "p.name ASC",
        };
        let query = query.replace("{order_clause}", order_clause);
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;

        let persons_iter = stmt
            .query_map([], |row| {
                let thumb_data: Option<Vec<u8>> = row.get(3)?;
                let thumbnail = thumb_data
                    .as_ref()
                    .map(|data| general_purpose::STANDARD.encode(data));
                Ok(Self {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    count: row.get(2)?,
                    thumbnail,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut persons = Vec::new();
        for person_result in persons_iter {
            persons.push(person_result.map_err(|e| e.to_string())?);
        }

        Ok(persons)
    }

    /// Generate thumbnail for a person from their cover face or best quality face
    /// Returns the thumbnail as JPEG bytes
    fn generate_thumbnail(
        conn: &Connection,
        person_id: i64,
        cover_face_id: Option<i64>,
    ) -> Result<Option<Vec<u8>>, String> {
        // 1. Determine which face to use
        let get_best_face = || -> Result<i64, rusqlite::Error> {
            conn.query_row(
                "SELECT id FROM faces WHERE person_id = ?1 ORDER BY (json_extract(bbox, '$.width') * json_extract(bbox, '$.height')) DESC LIMIT 1",
                params![person_id],
                |row| row.get(0),
            )
        };

        let face_id = if let Some(fid) = cover_face_id {
            // Validate that cover_face_id actually belongs to this person
            let is_valid: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM faces WHERE id = ?1 AND person_id = ?2)",
                    params![fid, person_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if is_valid {
                fid
            } else {
                match get_best_face() {
                    Ok(fid) => fid,
                    Err(_) => return Ok(None),
                }
            }
        } else {
            match get_best_face() {
                Ok(fid) => fid,
                Err(_) => return Ok(None),
            }
        };

        // 2. Get face info and file info
        let query = "
            SELECT f.id, faces.bbox, f.width, f.height, f.e_orientation, f.name, fd.path
            FROM faces 
            JOIN afiles f ON faces.file_id = f.id
            JOIN afolders fd ON f.folder_id = fd.id
            WHERE faces.id = ?1
        ";

        let row: Result<
            (
                i64,
                String,
                Option<u32>,
                Option<u32>,
                Option<i32>,
                String,
                String,
            ),
            _,
        > = conn.query_row(query, params![face_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        });

        let (file_id, bbox_json, orig_w_opt, orig_h_opt, orientation_opt, file_name, folder_path) =
            match row {
                Ok(r) => r,
                Err(_) => return Ok(None),
            };

        let bbox: FaceBBox = match serde_json::from_str(&bbox_json) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };

        let orientation = orientation_opt.unwrap_or(1); // Default to Normal

        // 3. Load Image (Original or Thumbnail)
        let full_path = std::path::Path::new(&folder_path).join(&file_name);

        // Helper to load and rotate original image
        let load_original = || -> Option<(image::DynamicImage, u32, u32)> {
            let mut dyn_img = image::open(&full_path).ok()?;
            dyn_img = match orientation {
                3 => dyn_img.rotate180(),
                6 => dyn_img.rotate90(),
                8 => dyn_img.rotate270(),
                _ => dyn_img,
            };
            let (w, h) = dyn_img.dimensions();
            Some((dyn_img, w, h))
        };

        // Helper to load thumbnail from cache-backed thumbnail storage
        let load_thumbnail = || -> Option<(image::DynamicImage, u32, u32)> {
            let data = AThumb::fetch(file_id).ok()??.thumb_data?;
            let img = image::load_from_memory(&data).ok()?;
            let (w, h) = img.dimensions();
            Some((img, w, h))
        };

        let (mut img, img_w, img_h) = match load_original().or_else(load_thumbnail) {
            Some(res) => res,
            None => return Ok(None),
        };

        // 4. Calculate Dimensions & BBox
        let (ref_w, ref_h) = if let (Some(ow), Some(oh)) = (orig_w_opt, orig_h_opt) {
            match orientation {
                6 | 8 => (oh, ow),
                _ => (ow, oh),
            }
        } else {
            (img_w, img_h)
        };

        let transformed_bbox = if orig_w_opt.is_some() && orig_h_opt.is_some() {
            let orig_w = orig_w_opt.unwrap();
            let orig_h = orig_h_opt.unwrap();
            match orientation {
                6 => FaceBBox {
                    x: orig_h as f32 - bbox.y - bbox.height,
                    y: bbox.x,
                    width: bbox.height,
                    height: bbox.width,
                },
                8 => FaceBBox {
                    x: bbox.y,
                    y: orig_w as f32 - bbox.x - bbox.width,
                    width: bbox.height,
                    height: bbox.width,
                },
                3 => FaceBBox {
                    x: orig_w as f32 - bbox.x - bbox.width,
                    y: orig_h as f32 - bbox.y - bbox.height,
                    width: bbox.width,
                    height: bbox.height,
                },
                _ => bbox,
            }
        } else {
            bbox
        };

        // 5. Crop and Resize
        let scale_x = img_w as f32 / ref_w as f32;
        let scale_y = img_h as f32 / ref_h as f32;
        let expansion = 0.2;

        let face_x = transformed_bbox.x * scale_x;
        let face_y = transformed_bbox.y * scale_y;
        let face_w = transformed_bbox.width * scale_x;
        let face_h = transformed_bbox.height * scale_y;

        let crop_x = (face_x - face_w * expansion).max(0.0) as u32;
        let crop_y = (face_y - face_h * expansion).max(0.0) as u32;
        let crop_w =
            (face_w * (1.0 + 2.0 * expansion)).min((img_w.saturating_sub(crop_x)) as f32) as u32;
        let crop_h =
            (face_h * (1.0 + 2.0 * expansion)).min((img_h.saturating_sub(crop_y)) as f32) as u32;

        if crop_w > 0 && crop_h > 0 && crop_x < img_w && crop_y < img_h {
            // Use crop() for DynamicImage type consistency
            let mut cropped = img.crop(
                crop_x,
                crop_y,
                crop_w.min(img_w - crop_x),
                crop_h.min(img_h - crop_y),
            );

            // Resize if too large
            let max_thumb_size = 200;
            if cropped.width() > max_thumb_size || cropped.height() > max_thumb_size {
                cropped = cropped.resize(
                    max_thumb_size,
                    max_thumb_size,
                    image::imageops::FilterType::Lanczos3,
                );
            }

            // Encode to JPEG (with RGB8 conversion for transparency support)
            let rgb_img = cropped.to_rgb8();
            let mut buffer = Cursor::new(Vec::new());
            if rgb_img.write_to(&mut buffer, ImageFormat::Jpeg).is_ok() {
                return Ok(Some(buffer.into_inner()));
            }
        }

        Ok(None)
    }

    /// Update thumbnail for a specific person
    #[allow(dead_code)]
    pub fn update_thumbnail(person_id: i64) -> Result<(), String> {
        let conn = open_conn()?;

        // Get cover_face_id for this person
        let cover_face_id: Option<i64> = conn
            .query_row(
                "SELECT cover_face_id FROM persons WHERE id = ?1",
                params![person_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();

        // Generate thumbnail
        let thumbnail = Self::generate_thumbnail(&conn, person_id, cover_face_id)?;

        // Update in database
        conn.execute(
            "UPDATE persons SET thumbnail = ?1 WHERE id = ?2",
            params![thumbnail, person_id],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Update thumbnails for all persons (called after clustering)
    pub fn update_all_thumbnails() -> Result<(), String> {
        let conn = open_conn()?;

        // Get all person IDs and their cover_face_ids
        let mut stmt = conn
            .prepare("SELECT id, cover_face_id FROM persons")
            .map_err(|e| e.to_string())?;

        let persons: Vec<(i64, Option<i64>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        // Generate and update thumbnail for each person
        for (person_id, cover_face_id) in persons {
            if let Ok(Some(thumbnail)) = Self::generate_thumbnail(&conn, person_id, cover_face_id) {
                let _ = conn.execute(
                    "UPDATE persons SET thumbnail = ?1 WHERE id = ?2",
                    params![thumbnail, person_id],
                );
            }
        }

        Ok(())
    }

    /// Rename a person
    pub fn rename(person_id: i64, new_name: &str) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute(
                "UPDATE persons SET name = ?1 WHERE id = ?2",
                params![new_name, person_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Delete a person (faces will have person_id set to NULL)
    pub fn delete(person_id: i64) -> Result<usize, String> {
        let conn = open_conn()?;

        // First, unlink all faces from this person
        conn.execute(
            "UPDATE faces SET person_id = NULL WHERE person_id = ?1",
            params![person_id],
        )
        .map_err(|e| e.to_string())?;

        // Then delete the person
        let result = conn
            .execute("DELETE FROM persons WHERE id = ?1", params![person_id])
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Create a new person (usually from face clustering)
    pub fn create(name: Option<&str>) -> Result<i64, String> {
        let conn = open_conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO persons (name, created_at) VALUES (?1, ?2)",
            params![name, now],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }
}

/// Face struct for storing detected faces
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Face {
    pub id: i64,
    pub file_id: i64,
    pub bbox: String, // JSON: {"x": f32, "y": f32, "width": f32, "height": f32, "confidence": f32}
    pub embedding: Option<Vec<u8>>, // 512-dimensional float32 embedding as bytes
    pub person_id: Option<i64>,
    pub person_name: Option<String>,
    pub created_at: i64,
}

impl Face {
    /// Add a new face using an existing connection (avoids repeated open_conn during batch indexing)
    #[allow(dead_code)]
    pub fn add_with_conn(
        conn: &Connection,
        file_id: i64,
        bbox: &str,
        embedding: &[f32],
    ) -> Result<i64, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Convert f32 embedding to bytes
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        conn.execute(
            "INSERT INTO faces (file_id, bbox, embedding, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![file_id, bbox, embedding_bytes, now],
        )
        .map_err(|e| e.to_string())?;

        Ok(conn.last_insert_rowid())
    }

    /// Check if a file already has faces detected
    /// Check if a file has faces
    #[allow(dead_code)]
    pub fn file_has_faces(file_id: i64) -> Result<bool, String> {
        let conn = open_conn()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM faces WHERE file_id = ?1",
                params![file_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    /// Reset all face data: delete all faces and persons
    pub fn reset_all() -> Result<(), String> {
        let conn = open_conn()?;

        // Use a transaction
        conn.execute("BEGIN TRANSACTION", params![])
            .map_err(|e| e.to_string())?;

        if let Err(e) = conn.execute("DELETE FROM faces", params![]) {
            let _ = conn.execute("ROLLBACK", params![]);
            return Err(e.to_string());
        }

        if let Err(e) = conn.execute("DELETE FROM persons", params![]) {
            let _ = conn.execute("ROLLBACK", params![]);
            return Err(e.to_string());
        }

        // Reset has_faces flag in afiles
        if let Err(e) = conn.execute("UPDATE afiles SET has_faces = 0", params![]) {
            let _ = conn.execute("ROLLBACK", params![]);
            return Err(e.to_string());
        }

        // Vacuum to reclaim space (optional, but good for reset)
        // Note: VACUUM cannot be run inside a transaction in some SQLite versions/modes,
        // but here we just commit first.

        conn.execute("COMMIT", params![])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Get faces for a specific file
    pub fn get_for_file(file_id: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.file_id, f.bbox, f.embedding, f.person_id, f.created_at, p.name 
                 FROM faces f
                 LEFT JOIN persons p ON f.person_id = p.id
                 WHERE f.file_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let faces = stmt
            .query_map([file_id], |row| {
                Ok(Self {
                    id: row.get(0)?,
                    file_id: row.get(1)?,
                    bbox: row.get(2)?,
                    embedding: row.get(3)?,
                    person_id: row.get(4)?,
                    created_at: row.get(5)?,
                    person_name: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(faces)
    }

    /// Get slim face data for clustering: (face_id, file_id, person_id, embedding_bytes)
    /// Avoids loading full Face structs (bbox JSON, created_at) to reduce memory.
    /// person_id is preserved so re-clustering can keep manual / prior assignments.
    pub fn get_all_for_clustering() -> Result<Vec<(i64, i64, Option<i64>, Option<Vec<u8>>)>, String>
    {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare("SELECT id, file_id, person_id, embedding FROM faces")
            .map_err(|e| e.to_string())?;

        let faces = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let file_id: i64 = row.get(1)?;
                let person_id: Option<i64> = row.get(2)?;
                let embedding: Option<Vec<u8>> = row.get(3)?;
                Ok((id, file_id, person_id, embedding))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(faces)
    }

    /// Next free "Person N" suffix for auto-created clusters (does not reuse existing numbers).
    pub fn next_auto_person_number() -> Result<usize, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare("SELECT name FROM persons WHERE name LIKE 'Person %'")
            .map_err(|e| e.to_string())?;
        let names = stmt
            .query_map([], |row| row.get::<_, Option<String>>(0))
            .map_err(|e| e.to_string())?;

        let mut max_n: usize = 0;
        for name in names {
            let name = name.map_err(|e| e.to_string())?;
            let Some(name) = name else {
                continue;
            };
            if let Some(rest) = name.strip_prefix("Person ") {
                if let Ok(n) = rest.trim().parse::<usize>() {
                    max_n = max_n.max(n);
                }
            }
        }
        Ok(max_n.saturating_add(1).max(1))
    }

    /// Reset all face assignments and delete all persons.
    /// Kept for explicit full-reset paths only — normal re-clustering must not call this.
    #[allow(dead_code)]
    pub fn reset_all_assignments() -> Result<(), String> {
        let conn = open_conn()?;

        // Clear all person_id from faces
        conn.execute("UPDATE faces SET person_id = NULL", [])
            .map_err(|e| e.to_string())?;

        // Delete all persons
        conn.execute("DELETE FROM persons", [])
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Assign a face to a person
    pub fn assign_to_person(face_id: i64, person_id: i64) -> Result<usize, String> {
        let conn = open_conn()?;
        let result = conn
            .execute(
                "UPDATE faces SET person_id = ?1 WHERE id = ?2",
                params![person_id, face_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Get all image file IDs that haven't been processed for faces yet
    /// Returns: Vec<(id, file_path, width, height)>
    pub fn get_unprocessed_image_files() -> Result<Vec<(i64, String, i64, i64)>, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT a.id, f.path || '/' || a.name as file_path, a.width, a.height
                 FROM afiles a 
                 JOIN afolders f ON a.folder_id = f.id
                 WHERE a.file_type = 1 
                   AND (a.has_faces IS NULL OR a.has_faces = 0)
                   AND a.width IS NOT NULL AND a.height IS NOT NULL
                 ORDER BY a.id",
            )
            .map_err(|e| e.to_string())?;

        let files = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(files)
    }

    /// Mark a file as scanned using an existing connection
    #[allow(dead_code)]
    pub fn mark_scanned_with_conn(
        conn: &Connection,
        file_id: i64,
        status: i32,
    ) -> Result<(), String> {
        conn.execute(
            "UPDATE afiles SET has_faces = ?1 WHERE id = ?2",
            params![status, file_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Apply a batch of scan results in one SQLite transaction.
    /// Each item is (file_id, has_faces_status, faces[(bbox_json, embedding)]).
    /// Returns total faces inserted in this batch.
    pub fn apply_scan_batch_with_conn(
        conn: &Connection,
        items: &[(i64, i32, Vec<(String, Vec<f32>)>)],
    ) -> Result<usize, String> {
        if items.is_empty() {
            return Ok(0);
        }
        // Match existing Face helpers that use BEGIN/COMMIT on &Connection
        // (rusqlite Transaction requires &mut Connection).
        conn.execute("BEGIN IMMEDIATE", params![])
            .map_err(|e| e.to_string())?;

        let mut faces_inserted = 0usize;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let result = (|| {
            let mut mark_stmt = conn
                .prepare("UPDATE afiles SET has_faces = ?1 WHERE id = ?2")
                .map_err(|e| e.to_string())?;
            let mut insert_stmt = conn
                .prepare(
                    "INSERT INTO faces (file_id, bbox, embedding, created_at) VALUES (?1, ?2, ?3, ?4)",
                )
                .map_err(|e| e.to_string())?;

            for (file_id, status, faces) in items {
                mark_stmt
                    .execute(params![status, file_id])
                    .map_err(|e| e.to_string())?;
                for (bbox, embedding) in faces {
                    let embedding_bytes: Vec<u8> =
                        embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
                    insert_stmt
                        .execute(params![file_id, bbox, embedding_bytes, now])
                        .map_err(|e| e.to_string())?;
                    faces_inserted += 1;
                }
            }
            Ok::<(), String>(())
        })();

        match result {
            Ok(()) => {
                conn.execute("COMMIT", params![])
                    .map_err(|e| e.to_string())?;
                Ok(faces_inserted)
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", params![]);
                Err(e)
            }
        }
    }

    /// Get statistics for face indexing
    /// Returns (processed_count, total_faces)
    pub fn get_stats() -> Result<(usize, usize), String> {
        let conn = open_conn()?;

        // Count processed files (has_faces > 0)
        let processed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM afiles WHERE has_faces > 0 AND file_type = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Count total faces
        let faces: i64 = conn
            .query_row("SELECT COUNT(*) FROM faces", [], |row| row.get(0))
            .unwrap_or(0);

        Ok((processed as usize, faces as usize))
    }

    /// Get full statistics for face indexing
    /// Returns (total_images, processed_images, unprocessed_images, total_faces)
    pub fn get_stats_full() -> Result<(usize, usize, usize, usize), String> {
        let conn = open_conn()?;

        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM afiles WHERE file_type = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let processed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM afiles WHERE has_faces > 0 AND file_type = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let faces: i64 = conn
            .query_row("SELECT COUNT(*) FROM faces", [], |row| row.get(0))
            .unwrap_or(0);

        let unprocessed = total - processed;

        Ok((
            total as usize,
            processed as usize,
            unprocessed as usize,
            faces as usize,
        ))
    }
}

// ---------------------------------------------------------------------------
// Smart Albums (rule-based saved queries; definitions live in LibraryState)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartRule {
    pub id: String,
    pub field: String,
    pub operator: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartQueryParams {
    #[serde(default = "default_smart_query_version")]
    pub version: i32,
    #[serde(default = "default_smart_query_match", rename = "match")]
    pub match_mode: String,
    #[serde(default)]
    pub rules: Vec<SmartRule>,
    pub sort_type: i64,
    pub sort_order: i64,
    #[serde(default)]
    pub calendar_sort: i64,
}

fn default_smart_query_version() -> i32 {
    1
}
fn default_smart_query_match() -> String {
    "all".to_string()
}

impl AFile {
    fn sj_i64(v: &serde_json::Value) -> Option<i64> {
        match v {
            serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            serde_json::Value::String(s) => s.trim().parse().ok(),
            serde_json::Value::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }
    fn sj_bool(v: &serde_json::Value) -> Option<bool> {
        match v {
            serde_json::Value::Bool(b) => Some(*b),
            serde_json::Value::Number(n) => Some(n.as_i64().unwrap_or(0) != 0),
            serde_json::Value::String(s) => match s.to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
    fn sj_str(v: &serde_json::Value) -> Option<String> {
        match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    fn build_smart_rule_condition(
        rule: &SmartRule,
        joins: &mut Vec<String>,
        needs_group: &mut bool,
        sql_params: &mut Vec<Box<dyn ToSql>>,
    ) -> Result<String, String> {
        let field = rule.field.as_str();
        let op = rule.operator.as_str();
        let value = &rule.value;
        match field {
            "name" => {
                let s = Self::sj_str(value).unwrap_or_default();
                match op {
                    "contains" => {
                        sql_params.push(Box::new(format!("%{}%", s)));
                        Ok("a.name LIKE ? COLLATE NOCASE".into())
                    }
                    "not_contains" => {
                        sql_params.push(Box::new(format!("%{}%", s)));
                        Ok("a.name NOT LIKE ? COLLATE NOCASE".into())
                    }
                    "is" | "eq" => {
                        sql_params.push(Box::new(s));
                        Ok("a.name = ? COLLATE NOCASE".into())
                    }
                    _ => Err(format!("Unsupported name op {}", op)),
                }
            }
            "favorite" => {
                let desired = Self::sj_bool(value).unwrap_or(true);
                let want = if matches!(op, "is" | "eq") {
                    desired
                } else {
                    !desired
                };
                Ok(if want {
                    "a.is_favorite = 1".into()
                } else {
                    "(a.is_favorite = 0 OR a.is_favorite IS NULL)".into()
                })
            }
            "rating" => match op {
                "is" | "eq" => {
                    let v = Self::sj_i64(value).ok_or("rating")?;
                    sql_params.push(Box::new(v));
                    Ok("a.rating = ?".into())
                }
                "is_not" | "neq" => {
                    let v = Self::sj_i64(value).ok_or("rating")?;
                    sql_params.push(Box::new(v));
                    Ok("a.rating != ?".into())
                }
                "gt" => {
                    let v = Self::sj_i64(value).ok_or("rating")?;
                    sql_params.push(Box::new(v));
                    Ok("a.rating > ?".into())
                }
                "gte" => {
                    let v = Self::sj_i64(value).ok_or("rating")?;
                    sql_params.push(Box::new(v));
                    Ok("a.rating >= ?".into())
                }
                "lt" => {
                    let v = Self::sj_i64(value).ok_or("rating")?;
                    sql_params.push(Box::new(v));
                    Ok("a.rating < ?".into())
                }
                "lte" => {
                    let v = Self::sj_i64(value).ok_or("rating")?;
                    sql_params.push(Box::new(v));
                    Ok("a.rating <= ?".into())
                }
                "empty" => Ok("(a.rating IS NULL OR a.rating = 0)".into()),
                "not_empty" => Ok("(a.rating IS NOT NULL AND a.rating > 0)".into()),
                _ => Err(format!("Unsupported rating op {}", op)),
            },
            "file_type" => {
                let mask = Self::sj_i64(value).unwrap_or(0);
                let condition =
                    Self::build_file_type_condition(mask).unwrap_or_else(|| "1 = 1".into());
                Ok(if matches!(op, "is_not" | "neq") {
                    format!("NOT ({})", condition)
                } else {
                    condition
                })
            }
            "extension" => {
                let ext = Self::sj_str(value)
                    .unwrap_or_default()
                    .trim_start_matches('.')
                    .to_ascii_lowercase();
                sql_params.push(Box::new(format!("%.{}", ext)));
                let cond = "lower(a.name) LIKE ?".to_string();
                Ok(if matches!(op, "is_not" | "neq") {
                    format!("NOT ({})", cond)
                } else {
                    cond
                })
            }
            "size" => {
                // UI always labels values as MB; accept fractional MB (e.g. 0.5).
                // Always convert MB → bytes so large numbers are never misread as raw bytes.
                let mb = match value {
                    serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                    serde_json::Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
                    _ => 0.0,
                };
                let n = if mb > 0.0 {
                    (mb * 1_000_000.0).round() as i64
                } else {
                    0
                };
                match op {
                    "gt" => {
                        sql_params.push(Box::new(n));
                        Ok("a.size > ?".into())
                    }
                    "gte" => {
                        sql_params.push(Box::new(n));
                        Ok("a.size >= ?".into())
                    }
                    "lt" => {
                        sql_params.push(Box::new(n));
                        Ok("a.size < ?".into())
                    }
                    "lte" => {
                        sql_params.push(Box::new(n));
                        Ok("a.size <= ?".into())
                    }
                    "eq" | "is" => {
                        sql_params.push(Box::new(n));
                        Ok("a.size = ?".into())
                    }
                    "is_not" | "neq" => {
                        sql_params.push(Box::new(n));
                        Ok("a.size != ?".into())
                    }
                    "empty" => Ok("(a.size IS NULL OR a.size = 0)".into()),
                    "not_empty" => Ok("(a.size IS NOT NULL AND a.size > 0)".into()),
                    _ => Err(format!("Unsupported size op {}", op)),
                }
            }
            "orientation" => {
                let kind = Self::sj_str(value).unwrap_or_default().to_ascii_lowercase();
                Ok(match kind.as_str() {
                    "landscape" => "a.width > a.height".into(),
                    "portrait" => "a.height > a.width".into(),
                    "square" => "a.width = a.height".into(),
                    _ => "1 = 1".into(),
                })
            }
            "tag" => match op {
                "has" => {
                    let id = Self::sj_i64(value).ok_or("tag id")?;
                    joins.push("INNER JOIN afile_tags at_smart ON a.id = at_smart.file_id".into());
                    *needs_group = true;
                    sql_params.push(Box::new(id));
                    Ok("at_smart.tag_id = ?".into())
                }
                "not_has" => {
                    let id = Self::sj_i64(value).ok_or("tag id")?;
                    sql_params.push(Box::new(id));
                    Ok(
                        "NOT EXISTS (SELECT 1 FROM afile_tags atx WHERE atx.file_id = a.id AND atx.tag_id = ?)"
                            .into(),
                    )
                }
                "empty" => {
                    Ok("NOT EXISTS (SELECT 1 FROM afile_tags atx WHERE atx.file_id = a.id)".into())
                }
                "not_empty" => {
                    Ok("EXISTS (SELECT 1 FROM afile_tags atx WHERE atx.file_id = a.id)".into())
                }
                _ => Err(format!("Unsupported tag op {}", op)),
            },
            "person" => match op {
                "has" => {
                    let id = Self::sj_i64(value).ok_or("person id")?;
                    joins.push("INNER JOIN faces f_smart ON a.id = f_smart.file_id".into());
                    *needs_group = true;
                    sql_params.push(Box::new(id));
                    Ok("f_smart.person_id = ?".into())
                }
                "not_has" => {
                    let id = Self::sj_i64(value).ok_or("person id")?;
                    sql_params.push(Box::new(id));
                    Ok(
                        "NOT EXISTS (SELECT 1 FROM faces fx WHERE fx.file_id = a.id AND fx.person_id = ?)"
                            .into(),
                    )
                }
                "empty" => Ok(
                    "NOT EXISTS (SELECT 1 FROM faces fx WHERE fx.file_id = a.id AND fx.person_id IS NOT NULL)"
                        .into(),
                ),
                "not_empty" => Ok(
                    "EXISTS (SELECT 1 FROM faces fx WHERE fx.file_id = a.id AND fx.person_id IS NOT NULL)"
                        .into(),
                ),
                _ => Err(format!("Unsupported person op {}", op)),
            },
            "date_taken" | "date_created" | "date_modified" => {
                let col = match field {
                    "date_created" => "a.created_at",
                    "date_modified" => "a.modified_at",
                    _ => "a.taken_date",
                };
                match op {
                    // Compare by local calendar day (same as calendar/content range filters)
                    // so timezone offsets do not shift boundary days.
                    "before" => {
                        let ts = Self::sj_i64(value).ok_or("ts")?;
                        sql_params.push(Box::new(ts));
                        Ok(format!(
                            "strftime('%Y-%m-%d', {0}, 'unixepoch', 'localtime') \
                             < strftime('%Y-%m-%d', ?, 'unixepoch', 'localtime')",
                            col
                        ))
                    }
                    "after" => {
                        let ts = Self::sj_i64(value).ok_or("ts")?;
                        sql_params.push(Box::new(ts));
                        Ok(format!(
                            "strftime('%Y-%m-%d', {0}, 'unixepoch', 'localtime') \
                             >= strftime('%Y-%m-%d', ?, 'unixepoch', 'localtime')",
                            col
                        ))
                    }
                    "between" => {
                        let start = value
                            .get("start")
                            .and_then(Self::sj_i64)
                            .or_else(|| value.as_array().and_then(|a| a.get(0)).and_then(Self::sj_i64))
                            .ok_or("start")?;
                        let end = value
                            .get("end")
                            .and_then(Self::sj_i64)
                            .or_else(|| value.as_array().and_then(|a| a.get(1)).and_then(Self::sj_i64))
                            .ok_or("end")?;
                        sql_params.push(Box::new(start));
                        sql_params.push(Box::new(end));
                        Ok(format!(
                            "strftime('%Y-%m-%d', {0}, 'unixepoch', 'localtime') \
                             >= strftime('%Y-%m-%d', ?, 'unixepoch', 'localtime') \
                             AND strftime('%Y-%m-%d', {0}, 'unixepoch', 'localtime') \
                             < strftime('%Y-%m-%d', ?, 'unixepoch', 'localtime')",
                            col
                        ))
                    }
                    "in_last" => {
                        let amount = value
                            .get("amount")
                            .and_then(Self::sj_i64)
                            .or_else(|| Self::sj_i64(value))
                            .unwrap_or(7)
                            .max(1);
                        let unit = value
                            .get("unit")
                            .and_then(Self::sj_str)
                            .unwrap_or_else(|| "day".into());
                        let secs = match unit.as_str() {
                            "week" => amount * 7 * 86400,
                            "month" => amount * 30 * 86400,
                            "year" => amount * 365 * 86400,
                            _ => amount * 86400,
                        };
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        sql_params.push(Box::new(now - secs));
                        Ok(format!("{} >= ?", col))
                    }
                    "older_than" => {
                        let amount = value
                            .get("amount")
                            .and_then(Self::sj_i64)
                            .or_else(|| Self::sj_i64(value))
                            .unwrap_or(365)
                            .max(1);
                        let unit = value
                            .get("unit")
                            .and_then(Self::sj_str)
                            .unwrap_or_else(|| "day".into());
                        let secs = match unit.as_str() {
                            "week" => amount * 7 * 86400,
                            "month" => amount * 30 * 86400,
                            "year" => amount * 365 * 86400,
                            _ => amount * 86400,
                        };
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        sql_params.push(Box::new(now - secs));
                        Ok(format!("{} < ?", col))
                    }
                    "empty" => Ok(format!("({} IS NULL OR {} = 0)", col, col)),
                    "not_empty" => Ok(format!("({} IS NOT NULL AND {} != 0)", col, col)),
                    _ => Err(format!("Unsupported date op {}", op)),
                }
            }
            "has_gps" => {
                let desired = Self::sj_bool(value).unwrap_or(true);
                let want = if matches!(op, "is" | "eq") {
                    desired
                } else {
                    !desired
                };
                Ok(if want {
                    "(a.gps_latitude IS NOT NULL AND a.gps_longitude IS NOT NULL)".into()
                } else {
                    "(a.gps_latitude IS NULL OR a.gps_longitude IS NULL)".into()
                })
            }
            "camera" => {
                let raw = Self::sj_str(value).unwrap_or_default();
                let parts: Vec<&str> = raw.splitn(2, "||").collect();
                let make = parts.first().copied().unwrap_or("").trim();
                let model = parts.get(1).copied().unwrap_or("").trim();
                if make.is_empty() {
                    return Ok("1 = 1".into());
                }
                sql_params.push(Box::new(make.to_string()));
                let mut cond = "UPPER(a.e_make) = UPPER(?)".to_string();
                if !model.is_empty() {
                    sql_params.push(Box::new(model.to_string()));
                    cond.push_str(" AND a.e_model = ?");
                }
                Ok(if matches!(op, "is_not" | "neq") {
                    format!("NOT ({})", cond)
                } else {
                    cond
                })
            }
            "lens" => {
                let raw = Self::sj_str(value).unwrap_or_default();
                let parts: Vec<&str> = raw.splitn(2, "||").collect();
                let make = parts.first().copied().unwrap_or("").trim();
                let model = parts.get(1).copied().unwrap_or("").trim();
                if make.is_empty() {
                    return Ok("1 = 1".into());
                }
                sql_params.push(Box::new(make.to_string()));
                let mut cond = "UPPER(a.e_lens_make) = UPPER(?)".to_string();
                if !model.is_empty() {
                    sql_params.push(Box::new(model.to_string()));
                    cond.push_str(" AND a.e_lens_model = ?");
                }
                Ok(if matches!(op, "is_not" | "neq") {
                    format!("NOT ({})", cond)
                } else {
                    cond
                })
            }
            _ => Err(format!("Unsupported smart field: {}", field)),
        }
    }

    fn build_smart_query_parts(
        params: &SmartQueryParams,
    ) -> Result<(String, String, Vec<Box<dyn ToSql>>, bool), String> {
        if params.rules.is_empty() {
            return Err("Smart query requires at least one rule".into());
        }
        let mut joins = Vec::new();
        let mut conditions = Vec::new();
        let mut sql_params: Vec<Box<dyn ToSql>> = Vec::new();
        let mut needs_group = false;
        for rule in &params.rules {
            conditions.push(Self::build_smart_rule_condition(
                rule,
                &mut joins,
                &mut needs_group,
                &mut sql_params,
            )?);
        }
        conditions.push(Self::search_exclusion_condition("b"));
        conditions.push("COALESCE(a.live_photo_type, 0) != 2".into());
        let joiner = if params.match_mode == "any" {
            " OR "
        } else {
            " AND "
        };
        let rule_count = params.rules.len();
        let (rule_conditions, trailing) = conditions.split_at(rule_count);
        let mut grouped = vec![format!("({})", rule_conditions.join(joiner))];
        grouped.extend(trailing.iter().cloned());
        let where_clause = format!(" WHERE {}", grouped.join(" AND "));
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        for j in joins {
            if seen.insert(j.clone()) {
                unique.push(j);
            }
        }
        let joins_clause = if unique.is_empty() {
            String::new()
        } else {
            format!(" {}", unique.join(" "))
        };
        Ok((joins_clause, where_clause, sql_params, needs_group))
    }

    pub fn get_smart_query_count_and_sum(params: &SmartQueryParams) -> Result<(i64, i64), String> {
        let (joins, where_clause, sql_params, needs_group) = Self::build_smart_query_parts(params)?;
        let sql = if needs_group {
            format!(
                "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM (SELECT a.id, a.size FROM afiles a LEFT JOIN afolders b ON a.folder_id = b.id LEFT JOIN albums c ON b.album_id = c.id {}{} GROUP BY a.id)",
                joins, where_clause
            )
        } else {
            format!(
                "SELECT COUNT(*), COALESCE(SUM(a.size), 0) FROM afiles a LEFT JOIN afolders b ON a.folder_id = b.id LEFT JOIN albums c ON b.album_id = c.id {}{}",
                joins, where_clause
            )
        };
        let final_params: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
        Self::query_count_and_sum(&sql, &final_params)
    }

    pub fn get_smart_query_files(
        params: &SmartQueryParams,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Self>, String> {
        let (joins, where_clause, sql_params, needs_group) = Self::build_smart_query_parts(params)?;
        let mut query = Self::build_base_query();
        query.push_str(&joins);
        query.push_str(&where_clause);
        if needs_group {
            query.push_str(" GROUP BY a.id");
        }
        let sort_params = QueryParams {
            search_file_name: String::new(),
            search_file_type: 0,
            sort_type: params.sort_type,
            sort_order: params.sort_order,
            search_all_subfolders: String::new(),
            search_folder: String::new(),
            start_date: 0,
            end_date: 0,
            calendar_sort: params.calendar_sort,
            make: String::new(),
            model: String::new(),
            lens_make: String::new(),
            lens_model: String::new(),
            location_admin1: String::new(),
            location_name: String::new(),
            is_favorite: false,
            rating: -1,
            tag_id: 0,
            person_id: 0,
            gps_min_lat: None,
            gps_max_lat: None,
            gps_min_lon: None,
            gps_max_lon: None,
        };
        query.push_str(&format!(
            " ORDER BY {}",
            Self::build_order_clause(&sort_params)
        ));
        query.push_str(" LIMIT ? OFFSET ?");
        let mut final_params: Vec<&dyn ToSql> = sql_params.iter().map(|p| p.as_ref()).collect();
        final_params.push(&limit);
        final_params.push(&offset);
        Self::query_files(&query, &final_params)
    }
}

/// Virtual collection of library files (manual sets; max 10 per library).
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ACollection {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ACollectionOrder {
    pub id: i64,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollectionAddResult {
    pub added: usize,
    pub skipped: usize,
}

impl ACollection {
    pub const MAX_COLLECTIONS: usize = 10;

    fn now_secs() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// Count membership excluding Apple Live companion videos (type 2).
    fn count_files_sql() -> &'static str {
        "SELECT COUNT(*) FROM acollections_files cf
         JOIN afiles a ON a.id = cf.file_id
         WHERE cf.collection_id = ?1
           AND COALESCE(a.live_photo_type, 0) != 2"
    }

    pub fn list() -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, sort_order, created_at, updated_at
                 FROM acollections
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            let (id, name, sort_order, created_at, updated_at) = row.map_err(|e| e.to_string())?;
            let count: i64 = conn
                .query_row(Self::count_files_sql(), params![id], |r| r.get(0))
                .unwrap_or(0);
            out.push(Self {
                id,
                name,
                sort_order,
                count,
                created_at,
                updated_at,
            });
        }
        Ok(out)
    }

    pub fn create(name: &str) -> Result<Self, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Collection name is empty".to_string());
        }
        let conn = open_conn()?;
        let existing: i64 = conn
            .query_row("SELECT COUNT(*) FROM acollections", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if existing as usize >= Self::MAX_COLLECTIONS {
            return Err(format!(
                "Maximum of {} collections reached",
                Self::MAX_COLLECTIONS
            ));
        }
        let now = Self::now_secs();
        let sort_order = existing;
        conn.execute(
            "INSERT INTO acollections (name, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![name, sort_order, now, now],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        Ok(Self {
            id,
            name: name.to_string(),
            sort_order,
            count: 0,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn rename(id: i64, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Collection name is empty".to_string());
        }
        let conn = open_conn()?;
        let now = Self::now_secs();
        let n = conn
            .execute(
                "UPDATE acollections SET name = ?1, updated_at = ?2 WHERE id = ?3",
                params![name, now, id],
            )
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err(format!("Collection not found: {}", id));
        }
        Ok(())
    }

    pub fn delete(id: i64) -> Result<(), String> {
        let conn = open_conn()?;
        conn.execute("DELETE FROM acollections WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reorder(items: &[ACollectionOrder]) -> Result<(), String> {
        let conn = open_conn()?;
        let now = Self::now_secs();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare("UPDATE acollections SET sort_order = ?1, updated_at = ?2 WHERE id = ?3")
                .map_err(|e| e.to_string())?;
            for item in items {
                stmt.execute(params![item.sort_order, now, item.id])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_files(id: i64, file_ids: &[i64]) -> Result<CollectionAddResult, String> {
        let conn = open_conn()?;
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM acollections WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists == 0 {
            return Err(format!("Collection not found: {}", id));
        }
        let now = Self::now_secs();
        let mut added = 0usize;
        let mut skipped = 0usize;
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR IGNORE INTO acollections_files (collection_id, file_id, added_at)
                     VALUES (?1, ?2, ?3)",
                )
                .map_err(|e| e.to_string())?;
            for file_id in file_ids {
                if *file_id <= 0 {
                    skipped += 1;
                    continue;
                }
                let n = stmt
                    .execute(params![id, file_id, now])
                    .map_err(|e| e.to_string())?;
                if n > 0 {
                    added += 1;
                } else {
                    skipped += 1;
                }
            }
            tx.execute(
                "UPDATE acollections SET updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(CollectionAddResult { added, skipped })
    }

    pub fn remove_files(id: i64, file_ids: &[i64]) -> Result<usize, String> {
        if file_ids.is_empty() {
            return Ok(0);
        }
        let conn = open_conn()?;
        let now = Self::now_secs();
        let placeholders = file_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "DELETE FROM acollections_files WHERE collection_id = ?1 AND file_id IN ({})",
            placeholders
        );
        let mut params_vec: Vec<&dyn ToSql> = Vec::with_capacity(1 + file_ids.len());
        params_vec.push(&id);
        for f in file_ids {
            params_vec.push(f);
        }
        let n = conn
            .execute(&sql, params_vec.as_slice())
            .map_err(|e| e.to_string())?;
        let _ = conn.execute(
            "UPDATE acollections SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        );
        Ok(n)
    }

    pub fn clear(id: i64) -> Result<(), String> {
        let conn = open_conn()?;
        let now = Self::now_secs();
        conn.execute(
            "DELETE FROM acollections_files WHERE collection_id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE acollections SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn file_ids(id: i64) -> Result<Vec<i64>, String> {
        let conn = open_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT cf.file_id FROM acollections_files cf
                 JOIN afiles a ON a.id = cf.file_id
                 WHERE cf.collection_id = ?1
                   AND COALESCE(a.live_photo_type, 0) != 2
                 ORDER BY cf.added_at DESC, cf.file_id DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Collection membership + normal QueryParams filters (sort/type/etc.).
    pub fn get_files(
        id: i64,
        params: &QueryParams,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<AFile>, String> {
        let (joins, where_clause, sql_params) = AFile::build_search_query_parts(params);
        let mut query = AFile::build_base_query();
        query.push_str(
            " INNER JOIN acollections_files cf ON cf.file_id = a.id AND cf.collection_id = ? ",
        );
        query.push_str(&joins);
        // Append membership filters into WHERE
        if where_clause.trim().is_empty() {
            query.push_str(" WHERE COALESCE(a.live_photo_type, 0) != 2 ");
        } else {
            // where_clause starts with " WHERE "
            query.push_str(&where_clause);
            query.push_str(" AND COALESCE(a.live_photo_type, 0) != 2 ");
        }
        if params.person_id > 0 {
            query.push_str(" GROUP BY a.id");
        }
        query.push_str(&format!(" ORDER BY {}", AFile::build_order_clause(params)));
        // Prefer "date added to collection" when default date sorts would dominate:
        // still honor user sort; only add cf.added_at as stable secondary when sort is id.
        query.push_str(" LIMIT ? OFFSET ?");

        let mut final_params: Vec<&dyn ToSql> = Vec::new();
        final_params.push(&id);
        for p in &sql_params {
            final_params.push(p.as_ref());
        }
        final_params.push(&limit);
        final_params.push(&offset);
        AFile::query_files(&query, &final_params)
    }

    pub fn get_count_and_sum(id: i64, params: &QueryParams) -> Result<(i64, i64), String> {
        let (joins, where_clause, sql_params) = AFile::build_search_query_parts(params);
        let mut sql = String::from(
            "SELECT COUNT(*), COALESCE(SUM(a.size), 0) FROM afiles a
             INNER JOIN acollections_files cf ON cf.file_id = a.id AND cf.collection_id = ?
             LEFT JOIN afolders b ON a.folder_id = b.id
             LEFT JOIN albums c ON b.album_id = c.id ",
        );
        // build_search_query_parts already joins folders/albums via joins string for tags etc.
        // Prefer using joins from builder for tag/person.
        if !joins.is_empty() {
            // rebuild with builder joins only (avoid double left join)
            sql = format!(
                "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM (
                    SELECT a.id, a.size FROM afiles a
                    INNER JOIN acollections_files cf ON cf.file_id = a.id AND cf.collection_id = ?
                    LEFT JOIN afolders b ON a.folder_id = b.id
                    LEFT JOIN albums c ON b.album_id = c.id
                    {}{} AND COALESCE(a.live_photo_type, 0) != 2
                    GROUP BY a.id
                 )",
                joins,
                if where_clause.trim().is_empty() {
                    " WHERE 1=1 "
                } else {
                    where_clause.as_str()
                }
            );
        } else if where_clause.trim().is_empty() {
            sql.push_str(" WHERE COALESCE(a.live_photo_type, 0) != 2 ");
        } else {
            sql.push_str(&where_clause);
            sql.push_str(" AND COALESCE(a.live_photo_type, 0) != 2 ");
        }

        let mut final_params: Vec<&dyn ToSql> = Vec::new();
        final_params.push(&id);
        for p in &sql_params {
            final_params.push(p.as_ref());
        }
        AFile::query_count_and_sum(&sql, &final_params)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ACamera {
    pub make: String,
    pub models: Vec<String>,
    pub counts: Vec<i64>,
}

fn sort_labeled_counts(labels: &mut Vec<String>, counts: &mut Vec<i64>, sort: i64) {
    let mut pairs: Vec<(String, i64)> = labels.drain(..).zip(counts.drain(..)).collect();

    match sort {
        1 => pairs.sort_by(|a, b| b.0.cmp(&a.0)),
        2 => pairs.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))),
        3 => pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))),
        _ => pairs.sort_by(|a, b| a.0.cmp(&b.0)),
    }

    for (label, count) in pairs {
        labels.push(label);
        counts.push(count);
    }
}

impl ACamera {
    // get all camera makes and models from db
    pub fn get_from_db(sort: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let query = "SELECT UPPER(a.e_make), a.e_model, count(a.id) as count
            FROM afiles a
            WHERE a.e_make IS NOT NULL AND a.e_model IS NOT NULL
            GROUP BY UPPER(a.e_make), a.e_model
            ORDER BY UPPER(a.e_make), a.e_model"
            .to_string();

        let mut stmt = conn.prepare(query.as_str()).map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                let make: String = row.get(0)?;
                let model: String = row.get(1)?;
                let count: i64 = row.get(2)?;
                Ok((make, model, count))
            })
            .map_err(|e| e.to_string())?;

        let mut hash_map: HashMap<String, (Vec<String>, Vec<i64>)> = HashMap::new();

        for row_result in rows {
            let (make, model, count) = row_result.map_err(|e| e.to_string())?;
            let entry = hash_map
                .entry(make)
                .or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(model); // Push model to Vec<String>
            entry.1.push(count); // Push count to Vec<i64>
        }

        let mut cameras: Vec<Self> = hash_map
            .into_iter()
            .map(|(make, (mut models, mut counts))| {
                sort_labeled_counts(&mut models, &mut counts, sort);
                Self {
                    make,
                    models,
                    counts,
                }
            })
            .collect();

        match sort {
            1 => cameras.sort_by(|a, b| b.make.cmp(&a.make)),
            2 => cameras.sort_by(|a, b| {
                a.counts
                    .iter()
                    .sum::<i64>()
                    .cmp(&b.counts.iter().sum::<i64>())
                    .then_with(|| a.make.cmp(&b.make))
            }),
            3 => cameras.sort_by(|a, b| {
                b.counts
                    .iter()
                    .sum::<i64>()
                    .cmp(&a.counts.iter().sum::<i64>())
                    .then_with(|| a.make.cmp(&b.make))
            }),
            _ => cameras.sort_by(|a, b| a.make.cmp(&b.make)),
        }

        Ok(cameras)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ALens {
    pub make: String,
    pub models: Vec<String>,
    pub counts: Vec<i64>,
}

impl ALens {
    // get all lens makes and models from db
    pub fn get_from_db(sort: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;
        let query = "SELECT UPPER(a.e_lens_make), a.e_lens_model, count(a.id) as count
            FROM afiles a
            WHERE a.e_lens_make IS NOT NULL AND a.e_lens_model IS NOT NULL
            GROUP BY UPPER(a.e_lens_make), a.e_lens_model
            ORDER BY UPPER(a.e_lens_make), a.e_lens_model"
            .to_string();

        let mut stmt = conn.prepare(query.as_str()).map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                let make: String = row.get(0)?;
                let model: String = row.get(1)?;
                let count: i64 = row.get(2)?;
                Ok((make, model, count))
            })
            .map_err(|e| e.to_string())?;

        let mut hash_map: HashMap<String, (Vec<String>, Vec<i64>)> = HashMap::new();

        for row_result in rows {
            let (make, model, count) = row_result.map_err(|e| e.to_string())?;
            let entry = hash_map
                .entry(make)
                .or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(model);
            entry.1.push(count);
        }

        let mut lenses: Vec<Self> = hash_map
            .into_iter()
            .map(|(make, (mut models, mut counts))| {
                sort_labeled_counts(&mut models, &mut counts, sort);
                Self {
                    make,
                    models,
                    counts,
                }
            })
            .collect();

        match sort {
            1 => lenses.sort_by(|a, b| b.make.cmp(&a.make)),
            2 => lenses.sort_by(|a, b| {
                a.counts
                    .iter()
                    .sum::<i64>()
                    .cmp(&b.counts.iter().sum::<i64>())
                    .then_with(|| a.make.cmp(&b.make))
            }),
            3 => lenses.sort_by(|a, b| {
                b.counts
                    .iter()
                    .sum::<i64>()
                    .cmp(&a.counts.iter().sum::<i64>())
                    .then_with(|| a.make.cmp(&b.make))
            }),
            _ => lenses.sort_by(|a, b| a.make.cmp(&b.make)),
        }

        Ok(lenses)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ALocation {
    pub cc: String,
    pub admin1: String,
    pub names: Vec<String>,
    pub counts: Vec<i64>,
}

impl ALocation {
    // get all location admin1 and names from db
    pub fn get_from_db(sort: i64) -> Result<Vec<Self>, String> {
        let conn = open_conn()?;

        let query = "SELECT COALESCE(a.geo_cc, ''), a.geo_admin1, a.geo_name, count(a.id) as count
            FROM afiles a
            WHERE COALESCE(a.geo_admin1, '') <> '' AND COALESCE(a.geo_name, '') <> ''
            GROUP BY a.geo_cc, a.geo_admin1, a.geo_name
            ORDER BY a.geo_cc, a.geo_admin1, a.geo_name"
            .to_string();

        let mut stmt = conn.prepare(query.as_str()).map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![], |row| {
                let cc: String = row.get(0)?;
                let admin1: String = row.get(1)?;
                let name: String = row.get(2)?;
                let count: i64 = row.get(3)?;
                Ok((cc, admin1, name, count))
            })
            .map_err(|e| e.to_string())?;

        let mut hash_map: HashMap<(String, String), (Vec<String>, Vec<i64>)> = HashMap::new();

        for row in rows {
            let (cc, admin1, name, count) = row.map_err(|e| e.to_string())?;
            let entry = hash_map
                .entry((cc, admin1))
                .or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(name); // Push name to Vec<String>
            entry.1.push(count); // Push count to Vec<i64>
        }

        let mut locations: Vec<Self> = hash_map
            .into_iter()
            .map(|((cc, admin1), (mut names, mut counts))| {
                sort_labeled_counts(&mut names, &mut counts, sort);
                Self {
                    cc,
                    admin1,
                    names,
                    counts,
                }
            })
            .collect();

        // Sort the locations by admin1
        match sort {
            1 => locations.sort_by(|a, b| b.admin1.cmp(&a.admin1)),
            2 => locations.sort_by(|a, b| {
                a.counts
                    .iter()
                    .sum::<i64>()
                    .cmp(&b.counts.iter().sum::<i64>())
                    .then_with(|| a.admin1.cmp(&b.admin1))
            }),
            3 => locations.sort_by(|a, b| {
                b.counts
                    .iter()
                    .sum::<i64>()
                    .cmp(&a.counts.iter().sum::<i64>())
                    .then_with(|| a.admin1.cmp(&b.admin1))
            }),
            _ => locations.sort_by(|a, b| a.admin1.cmp(&b.admin1)),
        }

        Ok(locations)
    }
}

/// A grid cell of aggregated GPS density, used for heatmap rendering.
/// `lat`/`lon` are the average coordinates of the photos within that
/// cell (cells are ~1.1km, grouped by rounded coordinates), `count` is
/// the number of photos within that cell.
#[derive(Debug, Serialize, Deserialize)]
pub struct AGpsHeatPoint {
    pub lat: f64,
    pub lon: f64,
    pub count: i64,
}

impl AGpsHeatPoint {
    /// Aggregate all GPS coordinates into grid cells on the backend, so the
    /// frontend never has to handle one row per photo (important for large libraries).
    pub fn get_heatmap_from_db() -> Result<Vec<Self>, String> {
        let conn = open_conn()?;

        let mut stmt = conn
            .prepare(
                "SELECT AVG(gps_latitude) AS lat, AVG(gps_longitude) AS lon, COUNT(*) AS cnt
                 FROM afiles
                 WHERE gps_latitude IS NOT NULL AND gps_longitude IS NOT NULL
                 GROUP BY ROUND(gps_latitude, 2), ROUND(gps_longitude, 2)",
            )
            .map_err(|e| e.to_string())?;

        let points = stmt
            .query_map(params![], |row| {
                Ok(Self {
                    lat: row.get(0)?,
                    lon: row.get(1)?,
                    count: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(points)
    }
}

/// get connection to the db
static CONN_POOL: Mutex<Vec<(String, Connection)>> = Mutex::new(Vec::new());

/// Soft cap on idle pooled connections. Excess Drop just closes the connection
/// instead of growing the pool without bound under concurrent bursts.
const MAX_CONN_POOL: usize = 8;

/// Normalize DB path keys so pool reuse is not broken by slash/case variance.
fn normalize_db_path_key(path: &str) -> String {
    let p = Path::new(path);
    let raw = match p.canonicalize() {
        Ok(c) => c.to_string_lossy().into_owned(),
        Err(_) => path.to_string(),
    };
    let mut s = raw.replace('\\', "/");
    // Windows extended-length prefix from canonicalize.
    if let Some(stripped) = s.strip_prefix("//?/") {
        s = stripped.to_string();
    }
    #[cfg(windows)]
    {
        s = s.to_ascii_lowercase();
    }
    // Drop trailing slash (except drive root like "c:/").
    if s.len() > 3 && s.ends_with('/') {
        s.pop();
    }
    s
}

/// Reject dynamic SQL identifiers that are not on an explicit allow-list.
fn assert_allowed_column(column: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.iter().any(|c| *c == column) {
        Ok(())
    } else {
        Err(format!("Invalid or disallowed column name: {}", column))
    }
}

/// A pooled connection that returns to the global pool on Drop.
pub(crate) struct PooledConn(Option<(String, Connection)>);

impl Drop for PooledConn {
    fn drop(&mut self) {
        if let Some(entry) = self.0.take() {
            if let Ok(mut pool) = CONN_POOL.lock() {
                if pool.len() < MAX_CONN_POOL {
                    pool.push(entry);
                }
                // else: drop connection (closes) — pool already full
            }
        }
    }
}

impl Deref for PooledConn {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.0.as_ref().unwrap().1
    }
}

impl DerefMut for PooledConn {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.0.as_mut().unwrap().1
    }
}

fn setup_conn(conn: &Connection) -> Result<(), String> {
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| format!("Failed to set SQLite busy timeout: {}", e))?;
    conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
        row.get::<_, String>(0)
    })
    .map_err(|e| format!("Failed to enable WAL mode: {}", e))?;
    conn.execute("PRAGMA synchronous = NORMAL", [])
        .map_err(|e| format!("Failed to set SQLite synchronous mode: {}", e))?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;
    Ok(())
}

fn create_conn() -> Result<(String, Connection), String> {
    let path = t_storage::get_current_db_path()
        .map_err(|e| format!("Failed to get the database file path: {}", e))?;
    create_conn_for_path(path)
}

fn create_conn_for_path(path: String) -> Result<(String, Connection), String> {
    let conn = Connection::open(&path)
        .map_err(|e| format!("Failed to open database connection: {}", e))?;
    setup_conn(&conn)?;
    // Cheap idempotent repair for Live Photo columns so file queries never
    // select missing columns on an older library DB opened without create_db().
    if let Err(e) = crate::t_migration::ensure_live_photo_columns(&conn) {
        eprintln!("ensure_live_photo_columns on open: {}", e);
    }
    if let Err(e) = crate::t_migration::ensure_collections_tables(&conn) {
        eprintln!("ensure_collections_tables on open: {}", e);
    }
    // Pool key after open so canonicalize can succeed once the file exists.
    Ok((normalize_db_path_key(&path), conn))
}

pub(crate) fn clear_conn_pool() {
    if let Ok(mut pool) = CONN_POOL.lock() {
        pool.clear();
    }
    // Library switch / storage migrate: drop any cached embedding matrix.
    clear_embed_matrix_cache();
}

pub(crate) fn open_conn() -> Result<PooledConn, String> {
    let current_path = t_storage::get_current_db_path()
        .map_err(|e| format!("Failed to get the database file path: {}", e))?;
    let current_key = normalize_db_path_key(&current_path);
    if let Ok(mut pool) = CONN_POOL.lock() {
        // Only reuse connections pointing to the same DB file
        while let Some((path, conn)) = pool.pop() {
            if path == current_key {
                return Ok(PooledConn(Some((path, conn))));
            }
            // Stale connection for a different library — drop it
        }
    }
    Ok(PooledConn(Some(create_conn()?)))
}

pub(crate) fn open_conn_for_library(library_id: &str) -> Result<PooledConn, String> {
    let path = t_storage::get_library_db_path(library_id).map_err(|e| {
        format!(
            "Failed to get database path for library '{}': {}",
            library_id, e
        )
    })?;
    let path_key = normalize_db_path_key(&path);
    if let Ok(mut pool) = CONN_POOL.lock() {
        while let Some((pooled_path, conn)) = pool.pop() {
            if pooled_path == path_key {
                return Ok(PooledConn(Some((pooled_path, conn))));
            }
        }
    }
    Ok(PooledConn(Some(create_conn_for_path(path)?)))
}

/// create all tables if not exists
pub fn create_db() -> Result<(), String> {
    let result = match create_db_internal() {
        Ok(_) => Ok(()),
        Err(err) => {
            if !should_recover_db(&err) {
                return Err(err);
            }

            eprintln!("create_db failed: {}. Trying recovery...", err);
            recover_current_db_file()?;
            create_db_internal().map_err(|e| format!("Database recovery retry failed: {}", e))
        }
    };
    if result.is_ok() {
        // Best-effort: warm semantic-search matrix so first query is not the cold load.
        warm_embed_matrix_cache();
    }
    result
}

fn create_db_internal() -> Result<(), String> {
    let conn = open_conn()?;

    // albums table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            created_at INTEGER,
            modified_at INTEGER,
            display_order_id INTEGER,
            cover_file_id INTEGER,
            description TEXT,
            indexed INTEGER DEFAULT 0,
            total INTEGER DEFAULT 0,
            last_scan_time INTEGER DEFAULT 0
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_albums_name ON albums(name)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_albums_path ON albums(path)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // folders table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS afolders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            album_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            path TEXT NOT NULL,
            created_at INTEGER,
            modified_at INTEGER,
            is_favorite INTEGER,
            is_excluded_from_search INTEGER DEFAULT 0,
            FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afolders_album_id ON afolders(album_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afolders_name ON afolders(name)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afolders_path ON afolders(path)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afolders_is_favorite ON afolders(is_favorite)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // files table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS afiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            folder_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            name_pinyin TEXT,
            size INTEGER NOT NULL,
            file_type INTEGER,
            format_label TEXT,
            created_at INTEGER,
            modified_at INTEGER,
            inode INTEGER,
            taken_date INTEGER,
            width INTEGER,
            height INTEGER,
            duration INTEGER,
            is_favorite INTEGER,
            rating INTEGER NOT NULL DEFAULT 0,
            rotate INTEGER,
            comments TEXT,
            has_tags INTEGER,
            has_faces INTEGER DEFAULT 0,
            e_make TEXT,
            e_model TEXT,
            e_date_time TEXT,
            e_software TEXT,
            e_artist TEXT,
            e_copyright TEXT,
            e_description TEXT,
            e_lens_make TEXT,
            e_lens_model TEXT,
            e_exposure_bias TEXT,
            e_exposure_time TEXT,
            e_f_number TEXT,
            e_focal_length TEXT,
            e_iso_speed TEXT,
            e_flash TEXT,
            e_orientation INTEGER,
            gps_latitude REAL,
            gps_longitude REAL,
            gps_altitude REAL,
            geo_name TEXT,
            geo_admin1 TEXT,
            geo_admin2 TEXT,
            geo_cc TEXT,
            embeds BLOB,
            last_scan_time INTEGER DEFAULT 0,
            content_id TEXT,
            paired_file_id INTEGER,
            live_photo_type INTEGER DEFAULT 0,
            FOREIGN KEY (folder_id) REFERENCES afolders(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS uidx_afiles_folder_id_name ON afiles(folder_id, name)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_folder_id ON afiles(folder_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_name ON afiles(name)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_name_pinyin ON afiles(name_pinyin)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_file_type ON afiles(file_type)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_taken_date ON afiles(taken_date)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_is_favorite ON afiles(is_favorite)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_has_tags ON afiles(has_tags)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // Migration: Add has_faces column if it doesn't exist
    // We try to add it, if it fails it likely exists.
    // Ideally we should check strict versioning but for now this is robust enough for simple addition.
    let _ = conn.execute(
        "ALTER TABLE afiles ADD COLUMN has_faces INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE afiles ADD COLUMN rating INTEGER NOT NULL DEFAULT 0",
        [],
    );

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_rating ON afiles(rating)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // Create index for has_faces
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_has_faces ON afiles(has_faces)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_make_model ON afiles(e_make, e_model)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_lens_make_model ON afiles(e_lens_make, e_lens_model)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_geo_name ON afiles(geo_name)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_geo_admin1 ON afiles(geo_admin1)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_geo_admin2 ON afiles(geo_admin2)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_geo_cc ON afiles(geo_cc)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_content_id ON afiles(content_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_paired_file_id ON afiles(paired_file_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afiles_live_photo_type ON afiles(live_photo_type)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // file thumbnail table
    // NOTE: New columns (thumb_key, thumb_mtime, thumb_size, updated_at) are added
    // by migration v3. They are included here so that fresh databases get the full
    // schema immediately; for existing databases CREATE TABLE IF NOT EXISTS is a
    // no-op and migration v3 will ALTER TABLE to add them.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS athumbs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL UNIQUE,
            error_code INTEGER NOT NULL,
            thumb_data BLOB,
            thumb_key TEXT,
            thumb_mtime INTEGER,
            thumb_size INTEGER,
            updated_at INTEGER,
            FOREIGN KEY (file_id) REFERENCES afiles(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_athumbs_file_id ON athumbs(file_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    // thumb_key index: may fail on pre-migration DBs where the column doesn't
    // exist yet. Migration v3 will create it after adding the column.
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_athumbs_thumb_key ON athumbs(thumb_key)",
        [],
    );

    // tags table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS atags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_atags_name ON atags(name)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // file_tags table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS afile_tags (
            file_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            PRIMARY KEY (file_id, tag_id),
            FOREIGN KEY (file_id) REFERENCES afiles(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES atags(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afile_tags_file_id ON afile_tags(file_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_afile_tags_tag_id ON afile_tags(tag_id)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // persons table (for face recognition)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS persons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            cover_face_id INTEGER,
            thumbnail BLOB,
            created_at INTEGER
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    // Migration: add thumbnail column if not exists (for existing databases)
    let _ = conn.execute("ALTER TABLE persons ADD COLUMN thumbnail BLOB", []);
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_persons_name ON persons(name)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // faces table (for face recognition)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS faces (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL,
            bbox TEXT,
            embedding BLOB,
            person_id INTEGER,
            created_at INTEGER,
            FOREIGN KEY (file_id) REFERENCES afiles(id) ON DELETE CASCADE,
            FOREIGN KEY (person_id) REFERENCES persons(id) ON DELETE SET NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_faces_file_id ON faces(file_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_faces_person_id ON faces(person_id)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // file hashes table (for deduplication)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS file_hashes (
            file_id INTEGER PRIMARY KEY,
            hash TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            mtime INTEGER NOT NULL,
            computed_at INTEGER NOT NULL,
            FOREIGN KEY (file_id) REFERENCES afiles(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_hashes_hash_size ON file_hashes(hash, file_size)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_hashes_mtime ON file_hashes(mtime)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // duplicate groups table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS duplicate_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hash TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            file_count INTEGER NOT NULL,
            total_size INTEGER NOT NULL,
            reviewed INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS uidx_duplicate_groups_hash_size ON duplicate_groups(hash, file_size)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // duplicate group items table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS duplicate_group_items (
            group_id INTEGER NOT NULL,
            file_id INTEGER NOT NULL,
            is_keep INTEGER NOT NULL DEFAULT 0,
            is_selected INTEGER NOT NULL DEFAULT 0,
            score REAL NOT NULL DEFAULT 0,
            PRIMARY KEY (group_id, file_id),
            FOREIGN KEY (group_id) REFERENCES duplicate_groups(id) ON DELETE CASCADE,
            FOREIGN KEY (file_id) REFERENCES afiles(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_dup_items_group ON duplicate_group_items(group_id)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_dup_items_file ON duplicate_group_items(file_id)",
        [],
    )
    .map_err(|e| e.to_string())?;

    // Run schema migrations after base tables are ensured.
    crate::t_migration::check_and_migrate(&conn)?;

    Ok(())
}

fn recover_current_db_file() -> Result<(), String> {
    let db_path = t_storage::get_current_db_path()
        .map_err(|e| format!("Failed to get current db path during recovery: {}", e))?;
    let db_path = PathBuf::from(db_path);

    if !db_path.exists() {
        // Nothing to quarantine, next create_db_internal will create a new DB.
        return Ok(());
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("Failed to get timestamp for db recovery: {}", e))?
        .as_secs();

    let db_name = db_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("library.db")
        .to_string();

    let backup_db = db_path.with_file_name(format!("{}.corrupt-{}", db_name, stamp));
    move_or_copy(&db_path, &backup_db)?;

    let wal_path = path_with_suffix(&db_path, "-wal");
    if wal_path.exists() {
        let backup_wal = path_with_suffix(&backup_db, "-wal");
        let _ = move_or_copy(&wal_path, &backup_wal);
    }

    let shm_path = path_with_suffix(&db_path, "-shm");
    if shm_path.exists() {
        let backup_shm = path_with_suffix(&backup_db, "-shm");
        let _ = move_or_copy(&shm_path, &backup_shm);
    }

    eprintln!(
        "Database file quarantined for recovery: '{}' -> '{}'",
        db_path.display(),
        backup_db.display()
    );

    Ok(())
}

fn should_recover_db(err: &str) -> bool {
    let err = err.to_lowercase();
    err.contains("database disk image is malformed") || err.contains("file is not a database")
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let s = format!("{}{}", path.to_string_lossy(), suffix);
    PathBuf::from(s)
}

fn move_or_copy(src: &Path, dst: &Path) -> Result<(), String> {
    match fs::rename(src, dst) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            fs::copy(src, dst).map_err(|copy_err| {
                format!(
                    "Failed to move '{}' to '{}' (rename: {}, copy: {})",
                    src.display(),
                    dst.display(),
                    rename_err,
                    copy_err
                )
            })?;
            fs::remove_file(src)
                .map_err(|e| format!("Failed to remove source file '{}': {}", src.display(), e))
        }
    }
}
