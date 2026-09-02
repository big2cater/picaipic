/**
 * Database storage, backup, and restore operations.
 * Handles custom DB storage directory, migration, backup/restore of library databases.
 * project: Lap
 * author:  julyx10
 * date:    2026-01-15
 */
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zip::ZipWriter;
use zip::read::ZipArchive;
use zip::write::FileOptions;

use crate::t_config::{self, AppConfig, Library, LibraryState};

static DB_MIGRATION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct DbMigrationGuard;

impl DbMigrationGuard {
    fn acquire() -> Result<Self, String> {
        DB_MIGRATION_IN_PROGRESS
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Database storage migration is already in progress.".to_string())?;
        Ok(Self)
    }
}

impl Drop for DbMigrationGuard {
    fn drop(&mut self) {
        DB_MIGRATION_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

fn get_db_storage_dir_from_config(config: &AppConfig) -> Result<PathBuf, String> {
    if let Some(dir) = config.db_storage_dir.as_deref() {
        let path = PathBuf::from(dir);
        fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create database storage directory: {}", e))?;
        Ok(path)
    } else {
        t_config::get_libraries_dir()
    }
}

fn get_library_db_path_from_config(config: &AppConfig, library_id: &str) -> Result<String, String> {
    let db_dir = get_db_storage_dir_from_config(config)?;
    Ok(db_dir
        .join(format!("{}.db", library_id))
        .to_string_lossy()
        .into_owned())
}

pub fn get_db_storage_dir() -> Result<String, String> {
    let config = t_config::load_app_config()?;
    get_db_storage_dir_from_config(&config).map(|p| p.to_string_lossy().into_owned())
}

pub fn is_using_custom_db_storage() -> Result<bool, String> {
    let config = t_config::load_app_config()?;
    Ok(config.db_storage_dir.is_some())
}

pub fn is_db_migration_in_progress() -> bool {
    DB_MIGRATION_IN_PROGRESS.load(Ordering::SeqCst)
}

/// Get the database file path for a library
pub fn get_library_db_path(library_id: &str) -> Result<String, String> {
    if is_db_migration_in_progress() {
        return Err("Database storage migration is in progress.".to_string());
    }
    let config = t_config::load_app_config()?;
    if !config
        .libraries
        .iter()
        .any(|library| library.id == library_id)
    {
        return Err(format!("Unknown library id: {}", library_id));
    }
    get_library_db_path_from_config(&config, library_id)
}

/// Get the current library's database file path
pub fn get_current_db_path() -> Result<String, String> {
    if is_db_migration_in_progress() {
        return Err("Database storage migration is in progress.".to_string());
    }
    let config = t_config::load_app_config()?;
    get_library_db_path_from_config(&config, &config.current_library_id)
}

fn checkpoint_db(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let conn = rusqlite::Connection::open(path)
        .map_err(|e| format!("Failed to open database for checkpoint: {}", e))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| format!("Failed to set SQLite busy timeout: {}", e))?;

    let run_checkpoint = |mode: &str| -> Result<(), String> {
        let pragma = format!("PRAGMA wal_checkpoint({})", mode);
        let (busy, log_frames, checkpointed_frames): (i64, i64, i64) = conn
            .query_row(&pragma, [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|e| format!("Failed to checkpoint database with mode {}: {}", mode, e))?;
        if busy != 0 || log_frames != checkpointed_frames {
            return Err(format!(
                "Database checkpoint with mode {} did not complete (busy={}, log_frames={}, checkpointed_frames={})",
                mode, busy, log_frames, checkpointed_frames
            ));
        }
        Ok(())
    };

    if let Err(truncate_err) = run_checkpoint("TRUNCATE") {
        eprintln!("{}", truncate_err);
        run_checkpoint("RESTART")?;
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|e| format!("Failed to open '{}' for hashing: {}", path.display(), e))?;
    let mut hasher = sha2::Sha256::default();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read '{}' for hashing: {}", path.display(), e))?;
        if read == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buffer[..read]);
    }
    Ok(format!("{:x}", sha2::Digest::finalize(hasher)))
}

fn quick_check_db(path: &Path) -> Result<(), String> {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| {
                format!(
                    "Failed to open migrated database '{}': {}",
                    path.display(),
                    e
                )
            })?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| format!("Failed to set SQLite busy timeout: {}", e))?;
    let result: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| format!("Failed to quick_check '{}': {}", path.display(), e))?;
    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(format!(
            "SQLite quick_check failed for '{}': {}",
            path.display(),
            result
        ))
    }
}

fn verify_database_copy(source: &Path, target: &Path) -> Result<(), String> {
    let source_hash = hash_file(source)?;
    let target_hash = hash_file(target)?;
    if source_hash != target_hash {
        return Err(format!(
            "Database copy hash mismatch for '{}' -> '{}'",
            source.display(),
            target.display()
        ));
    }
    quick_check_db(target)
}

fn migration_transfer_path(path: &Path, label: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("database");
    parent.join(format!(
        ".{file_name}.picaipic-{label}-{}.tmp",
        Uuid::new_v4()
    ))
}

#[derive(Debug)]
struct StagedMigrationFile {
    original: PathBuf,
    staged: PathBuf,
}

fn stage_existing_file(path: &Path, label: &str) -> Result<Option<StagedMigrationFile>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let staged = migration_transfer_path(path, label);
    fs::rename(path, &staged).map_err(|e| {
        format!(
            "Failed to stage '{}' for migration cleanup: {}",
            path.display(),
            e
        )
    })?;
    Ok(Some(StagedMigrationFile {
        original: path.to_path_buf(),
        staged,
    }))
}

fn restore_staged_files(staged_files: &[StagedMigrationFile]) {
    for file in staged_files.iter().rev() {
        if file.original.exists() {
            continue;
        }
        if let Err(error) = fs::rename(&file.staged, &file.original) {
            eprintln!(
                "Failed to restore staged migration file '{}' from '{}': {}",
                file.original.display(),
                file.staged.display(),
                error
            );
        }
    }
}

fn cleanup_staged_files(staged_files: &[StagedMigrationFile]) {
    for file in staged_files {
        if let Err(error) = fs::remove_file(&file.staged) {
            eprintln!(
                "Failed to remove staged migration source '{}': {}",
                file.staged.display(),
                error
            );
        }
    }
}

#[derive(Debug)]
struct MigratedTarget {
    target: PathBuf,
    previous_target: Option<PathBuf>,
}

fn copy_database_to_verified_target(
    source: &Path,
    target: &Path,
) -> Result<MigratedTarget, String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create target database directory '{}': {}",
                parent.display(),
                e
            )
        })?;
    }

    let tmp = migration_transfer_path(target, "migrate-copy");
    if let Err(error) = fs::copy(source, &tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "Failed to copy database '{}' to '{}': {}",
            source.display(),
            target.display(),
            error
        ));
    }
    if let Err(error) = verify_database_copy(source, &tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    // Clean up the verified copy if staging fails, otherwise a several-hundred-MB
    // temp file stays behind in the user's chosen storage directory.
    let previous_target = match stage_existing_file(target, "migrate-replace") {
        Ok(previous_target) => previous_target,
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&tmp, target) {
        if let Some(previous) = previous_target.as_ref() {
            let _ = fs::rename(&previous.staged, &previous.original);
        }
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "Failed to finalize migrated database '{}': {}",
            target.display(),
            error
        ));
    }

    Ok(MigratedTarget {
        target: target.to_path_buf(),
        previous_target: previous_target.map(|file| file.staged),
    })
}

fn rollback_migrated_targets(targets: &[MigratedTarget]) {
    for target in targets.iter().rev() {
        if let Err(error) = fs::remove_file(&target.target) {
            if target.target.exists() {
                eprintln!(
                    "Failed to remove migrated target '{}': {}",
                    target.target.display(),
                    error
                );
            }
        }
        if let Some(previous) = target.previous_target.as_ref() {
            if let Err(error) = fs::rename(previous, &target.target) {
                eprintln!(
                    "Failed to restore previous target '{}' from '{}': {}",
                    target.target.display(),
                    previous.display(),
                    error
                );
            }
        }
    }
}

fn cleanup_target_backups(targets: &[MigratedTarget]) {
    for target in targets {
        if let Some(previous) = target.previous_target.as_ref() {
            if let Err(error) = fs::remove_file(previous) {
                eprintln!(
                    "Failed to remove previous migrated target '{}': {}",
                    previous.display(),
                    error
                );
            }
        }
    }
}

fn stage_library_source_files(
    current_dir: &Path,
    library_id: &str,
) -> Result<Vec<StagedMigrationFile>, String> {
    let mut staged = Vec::new();
    for suffix in [".db", ".db-wal", ".db-shm"] {
        let path = current_dir.join(format!("{library_id}{suffix}"));
        match stage_existing_file(&path, "migrate-source") {
            Ok(Some(file)) => staged.push(file),
            Ok(None) => {}
            Err(error) => {
                restore_staged_files(&staged);
                return Err(error);
            }
        }
    }
    Ok(staged)
}

fn migrate_db_storage_dir(
    config: &mut AppConfig,
    target_dir: PathBuf,
    new_db_storage_dir: Option<String>,
) -> Result<String, String> {
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create target database directory: {}", e))?;

    let original_config = config.clone();
    let current_dir = get_db_storage_dir_from_config(&original_config)?;
    let current_dir_canon = fs::canonicalize(&current_dir).unwrap_or(current_dir.clone());
    let target_dir_canon = fs::canonicalize(&target_dir).unwrap_or(target_dir.clone());
    let target_dir_string = target_dir_canon.to_string_lossy().into_owned();

    if current_dir_canon == target_dir_canon {
        config.db_storage_dir = new_db_storage_dir.map(|_| target_dir_string.clone());
        t_config::save_app_config(config)?;
        return Ok(target_dir_string);
    }

    let mut migrated_targets = Vec::new();
    let prepare_result = (|| -> Result<(), String> {
        for library in &original_config.libraries {
            let source_path = PathBuf::from(get_library_db_path_from_config(
                &original_config,
                &library.id,
            )?);
            let target_path = target_dir_canon.join(format!("{}.db", library.id));

            if !source_path.exists() {
                continue;
            }

            checkpoint_db(&source_path)?;
            let migrated = copy_database_to_verified_target(&source_path, &target_path)
                .map_err(|e| format!("Failed to migrate database '{}': {}", library.name, e))?;
            migrated_targets.push(migrated);
        }

        Ok(())
    })();

    if let Err(error) = prepare_result {
        rollback_migrated_targets(&migrated_targets);
        return Err(error);
    }

    config.db_storage_dir = new_db_storage_dir.map(|_| target_dir_string.clone());
    if let Err(error) = t_config::save_app_config(config) {
        rollback_migrated_targets(&migrated_targets);
        return Err(error);
    }

    // Configuration now points only at verified targets. Source cleanup is
    // staged afterward so a crash before config save never hides the old DB.
    for library in &original_config.libraries {
        match stage_library_source_files(&current_dir, &library.id) {
            Ok(staged_sources) => cleanup_staged_files(&staged_sources),
            Err(error) => eprintln!(
                "Database migration completed, but old source cleanup for '{}' was skipped: {}",
                library.name, error
            ),
        }
    }
    cleanup_target_backups(&migrated_targets);
    Ok(target_dir_string)
}

pub fn change_db_storage_dir(new_dir: &str) -> Result<String, String> {
    let _migration_guard = DbMigrationGuard::acquire()?;
    let mut config = t_config::load_app_config()?;
    let target_dir = PathBuf::from(new_dir);
    migrate_db_storage_dir(&mut config, target_dir, Some(String::new()))
}

pub fn reset_db_storage_dir() -> Result<String, String> {
    let _migration_guard = DbMigrationGuard::acquire()?;
    let mut config = t_config::load_app_config()?;
    let target_dir = t_config::get_libraries_dir()?;
    migrate_db_storage_dir(&mut config, target_dir, None)
}

// ============================================================================
// Backup / Restore
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbStorageInfo {
    pub library_id: String,
    pub library_name: String,
    pub db_file_size: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub file_path: String,
    pub file_size: i64,
    pub library_count: usize,
    pub library_names: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMetaLibrary {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub db_size: i64,
    #[serde(default)]
    pub backup_file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMetaData {
    pub created_at: i64,
    pub libraries: Vec<BackupMetaLibrary>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSelection {
    pub library_name: String,
    pub should_rename: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored_count: usize,
    pub restored_names: Vec<String>,
}

pub fn get_db_storage_info() -> Result<Vec<DbStorageInfo>, String> {
    let config = t_config::load_app_config()?;

    let mut results = Vec::new();
    for lib in &config.libraries {
        let db_path = get_library_db_path_from_config(&config, &lib.id)?;
        let db_file_size = {
            let p = Path::new(&db_path);
            if p.exists() {
                fs::metadata(p).ok().map(|m| m.len() as i64).unwrap_or(0)
            } else {
                0
            }
        };

        results.push(DbStorageInfo {
            library_id: lib.id.clone(),
            library_name: lib.name.clone(),
            db_file_size,
        });
    }

    Ok(results)
}

/// Writes a backup archive to `dest` and returns the names of the libraries it
/// contains.
///
/// `dest` must be a scratch path: the archive is only published once
/// `zip.finish()` succeeded, so a failed backup never leaves a half-written and
/// unreadable zip at the destination the user picked.
fn write_backup_archive(
    selected: &[&Library],
    config: &AppConfig,
    dest: &Path,
) -> Result<Vec<String>, String> {
    let file =
        fs::File::create(dest).map_err(|e| format!("Failed to create backup file: {}", e))?;
    let mut zip = ZipWriter::new(file);

    let backup_info = BackupMetaData {
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
        libraries: selected
            .iter()
            .map(|lib| {
                let db_path = get_library_db_path_from_config(&config, &lib.id).ok();
                let size = db_path
                    .as_ref()
                    .and_then(|p| fs::metadata(p).ok())
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                BackupMetaLibrary {
                    id: Some(lib.id.clone()),
                    name: lib.name.clone(),
                    db_size: size,
                    backup_file: Some(format!("databases/{}.db", lib.id)),
                }
            })
            .collect(),
    };

    let info_json = serde_json::to_string_pretty(&backup_info)
        .map_err(|e| format!("Failed to serialize backup info: {}", e))?;

    let options =
        FileOptions::<'_, ()>::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("backup-info.json", options)
        .map_err(|e| format!("Failed to write backup-info.json to zip: {}", e))?;
    zip.write_all(info_json.as_bytes())
        .map_err(|e| format!("Failed to write backup-info.json content: {}", e))?;

    let mut library_names = Vec::new();
    for lib in selected {
        let db_path = get_library_db_path_from_config(&config, &lib.id)?;
        let path = Path::new(&db_path);
        if !path.exists() {
            continue;
        }

        let mut db_file = fs::File::open(path)
            .map_err(|e| format!("Failed to read database '{}': {}", lib.name, e))?;
        let zip_path = format!("databases/{}.db", lib.id);

        let options =
            FileOptions::<'_, ()>::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(&zip_path, options)
            .map_err(|e| format!("Failed to write {} to zip: {}", zip_path, e))?;
        std::io::copy(&mut db_file, &mut zip)
            .map_err(|e| format!("Failed to write {} content: {}", zip_path, e))?;

        library_names.push(lib.name.clone());
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize backup zip: {}", e))?;

    Ok(library_names)
}

pub fn backup_databases(library_ids: &[String], dest_path: &str) -> Result<BackupResult, String> {
    let _migration_guard = DbMigrationGuard::acquire()?;
    let config = t_config::load_app_config()?;

    let selected: Vec<&Library> = library_ids
        .iter()
        .filter_map(|id| config.libraries.iter().find(|l| l.id == *id))
        .collect();

    if selected.is_empty() {
        return Err("No libraries selected for backup.".to_string());
    }

    for lib in &selected {
        let db_path = get_library_db_path_from_config(&config, &lib.id)?;
        checkpoint_db(Path::new(&db_path))?;
    }

    // Build the archive beside the destination so a failure leaves the user's path
    // untouched instead of holding an unreadable partial zip.
    let dest = Path::new(dest_path);
    let tmp_path = migration_transfer_path(dest, "backup");
    let library_names = match write_backup_archive(&selected, &config, &tmp_path) {
        Ok(library_names) => library_names,
        Err(error) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&tmp_path, dest) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "Failed to finalize backup '{}': {}",
            dest.display(),
            error
        ));
    }

    let final_size = fs::metadata(dest).map(|m| m.len() as i64).unwrap_or(0);

    Ok(BackupResult {
        file_path: dest_path.to_string(),
        file_size: final_size,
        library_count: library_names.len(),
        library_names,
    })
}

pub fn parse_backup_file(path: &str) -> Result<BackupMetaData, String> {
    let file = fs::File::open(path).map_err(|e| format!("Failed to open backup file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read backup zip: {}", e))?;

    let info_index = archive
        .index_for_name("backup-info.json")
        .ok_or_else(|| "Invalid backup file: missing backup-info.json".to_string())?;
    let mut info_file = archive
        .by_index(info_index)
        .map_err(|e| format!("Failed to read backup-info.json: {}", e))?;
    let mut info_content = String::new();
    info_file
        .read_to_string(&mut info_content)
        .map_err(|e| format!("Failed to read backup-info.json: {}", e))?;

    let info: BackupMetaData = serde_json::from_str(&info_content)
        .map_err(|e| format!("Failed to parse backup-info.json: {}", e))?;

    Ok(info)
}

pub fn restore_databases(
    backup_path: &str,
    selections: &[RestoreSelection],
) -> Result<RestoreResult, String> {
    let _migration_guard = DbMigrationGuard::acquire()?;
    let mut config = t_config::load_app_config()?;

    let file =
        fs::File::open(backup_path).map_err(|e| format!("Failed to open backup file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read backup zip: {}", e))?;
    let backup_info = read_backup_metadata(&mut archive)?;

    let mut restored_names = Vec::new();
    let mut written_db_paths: Vec<PathBuf> = Vec::new();
    let mut existing_names: std::collections::HashSet<String> =
        config.libraries.iter().map(|l| l.name.clone()).collect();

    for selection in selections {
        let backup_library = backup_info
            .libraries
            .iter()
            .find(|library| library.name == selection.library_name)
            .ok_or_else(|| {
                format!(
                    "Selected library '{}' is not present in backup metadata",
                    selection.library_name
                )
            })?;
        let backup_file = backup_file_for_library(backup_library);

        let final_lib_name = if selection.should_rename {
            resolve_unique_name(&selection.library_name, &existing_names)
        } else {
            selection.library_name.clone()
        };

        let lib_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let new_lib = Library {
            id: lib_id.clone(),
            name: final_lib_name.clone(),
            created_at: now,
            state: LibraryState::default(),
            hidden: false,
        };

        let db_path = get_library_db_path_from_config(&config, &lib_id)?;
        let db_path_obj = Path::new(&db_path);
        if let Some(parent) = db_path_obj.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for library: {}", e))?;
        }
        let mut entry = archive.by_name(&backup_file).map_err(|e| {
            format!(
                "Failed to read database entry '{}' for '{}': {}",
                backup_file, selection.library_name, e
            )
        })?;
        if backup_library.db_size >= 0 && entry.size() != backup_library.db_size as u64 {
            cleanup_restored_db_files(&written_db_paths);
            return Err(format!(
                "Backup database size mismatch for '{}': metadata {}, zip {}",
                selection.library_name,
                backup_library.db_size,
                entry.size()
            ));
        }
        // New library id → new path. Still write via temp+rename so a crash
        // cannot leave a half-written .db that later looks openable.
        if let Err(e) = write_file_atomic_from_reader(db_path_obj, &mut entry) {
            cleanup_restored_db_files(&written_db_paths);
            return Err(format!(
                "Failed to write database file for '{}': {}",
                final_lib_name, e
            ));
        }
        drop(entry);
        if let Err(e) = quick_check_db(db_path_obj) {
            let _ = fs::remove_file(db_path_obj);
            cleanup_restored_db_files(&written_db_paths);
            return Err(format!(
                "Restored database for '{}' failed integrity validation: {}",
                final_lib_name, e
            ));
        }
        written_db_paths.push(db_path_obj.to_path_buf());

        config.libraries.push(new_lib);
        existing_names.insert(final_lib_name.clone());
        restored_names.push(final_lib_name);
    }

    if let Err(e) = t_config::save_app_config(&config) {
        // Config never recorded the new libraries — drop orphan db files.
        cleanup_restored_db_files(&written_db_paths);
        return Err(e);
    }
    Ok(RestoreResult {
        restored_count: restored_names.len(),
        restored_names,
    })
}

/// Write `bytes` to `path` via a sibling temp file + rename (crash-safe create).
/// Refuses to replace an existing destination — restore always targets a new library id.
fn write_file_atomic_from_reader<R: Read>(path: &Path, reader: &mut R) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "Refusing to overwrite existing database path: {}",
            path.display()
        ));
    }
    let tmp = migration_transfer_path(path, "restore");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("Failed to create temp database '{}': {}", tmp.display(), e))?;
    if let Err(error) = std::io::copy(reader, &mut output).and_then(|_| output.sync_all()) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "Failed to write temp database '{}': {}",
            tmp.display(),
            error
        ));
    }
    drop(output);
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(format!(
                "Failed to finalize database '{}': {}",
                path.display(),
                e
            ))
        }
    }
}

fn read_backup_metadata<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<BackupMetaData, String> {
    let info_index = archive
        .index_for_name("backup-info.json")
        .ok_or_else(|| "Invalid backup file: missing backup-info.json".to_string())?;
    let mut info_file = archive
        .by_index(info_index)
        .map_err(|e| format!("Failed to read backup-info.json: {}", e))?;
    let mut info_content = String::new();
    info_file
        .read_to_string(&mut info_content)
        .map_err(|e| format!("Failed to read backup-info.json: {}", e))?;
    serde_json::from_str(&info_content)
        .map_err(|e| format!("Failed to parse backup-info.json: {}", e))
}

fn backup_file_for_library(library: &BackupMetaLibrary) -> String {
    library
        .backup_file
        .clone()
        .unwrap_or_else(|| format!("{}.db", sanitize_filename(&library.name)))
}

fn cleanup_restored_db_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn resolve_unique_name(name: &str, existing: &std::collections::HashSet<String>) -> String {
    if !existing.contains(name) {
        return name.to_string();
    }
    for i in 1..=999 {
        let candidate = format!("{} ({})", name, i);
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    format!("{} ({})", name, rand::random::<u16>())
}

#[cfg(test)]
mod restore_write_tests {
    use super::*;

    fn test_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("picaipic-storage-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_file_atomic_creates_target_without_tmp_left_behind() {
        let dir = test_dir("restore-atom");
        let target = dir.join("lib.db");
        let mut input = std::io::Cursor::new(b"sqlite-bytes");
        write_file_atomic_from_reader(&target, &mut input).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"sqlite-bytes");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "temporary restore files must be cleaned up"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_file_atomic_refuses_existing_destination() {
        let dir = test_dir("restore-exist");
        let target = dir.join("lib.db");
        fs::write(&target, b"old").unwrap();
        let mut input = std::io::Cursor::new(b"new");
        let err = write_file_atomic_from_reader(&target, &mut input).unwrap_err();
        assert!(err.contains("Refusing to overwrite"), "{err}");
        assert_eq!(fs::read(&target).unwrap(), b"old");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn backup_file_uses_library_id_and_supports_legacy_name_entries() {
        let current = BackupMetaLibrary {
            id: Some("library-uuid".to_string()),
            name: "a/b".to_string(),
            db_size: 1,
            backup_file: Some("databases/library-uuid.db".to_string()),
        };
        let legacy = BackupMetaLibrary {
            id: None,
            name: "a/b".to_string(),
            db_size: 1,
            backup_file: None,
        };

        assert_eq!(
            backup_file_for_library(&current),
            "databases/library-uuid.db"
        );
        assert_eq!(backup_file_for_library(&legacy), "a_b.db");
    }

    #[test]
    fn verified_copy_requires_identical_valid_sqlite_database() {
        let dir = test_dir("verified-copy");
        let source = dir.join("source.db");
        let target = dir.join("target.db");
        let conn = rusqlite::Connection::open(&source).unwrap();
        conn.execute_batch(
            "CREATE TABLE files (id INTEGER PRIMARY KEY); INSERT INTO files VALUES (1);",
        )
        .unwrap();
        drop(conn);

        fs::copy(&source, &target).unwrap();
        verify_database_copy(&source, &target).unwrap();

        fs::write(&target, b"corrupt").unwrap();
        assert!(verify_database_copy(&source, &target).is_err());
        assert!(quick_check_db(&target).is_err());
        assert!(
            source.exists(),
            "source must remain available after a failed check"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
