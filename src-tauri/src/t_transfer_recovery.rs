use crate::t_config;
use crate::t_sqlite::AFile;
use crate::t_utils::{self, FileConflictPolicy};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

static OUTSIDE_MOVE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutsideMoveJournal {
    id: String,
    library_id: String,
    file_id: i64,
    source_path: String,
    destination_path: String,
    destination_existed: bool,
    staged_path: String,
    backup_path: String,
}

fn journal_dir() -> Result<PathBuf, String> {
    let dir = t_config::get_app_data_dir()?.join("outside-move-journal");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create outside-move journal directory: {error}"))?;
    Ok(dir)
}

fn journal_path(journal: &OutsideMoveJournal) -> Result<PathBuf, String> {
    Ok(journal_dir()?.join(format!("{}.json", journal.id)))
}

fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}

fn write_journal(journal: &OutsideMoveJournal) -> Result<PathBuf, String> {
    let path = journal_path(journal)?;
    let temp = path.with_extension("json.tmp");
    let content = serde_json::to_vec_pretty(journal)
        .map_err(|error| format!("Failed to serialize outside-move journal: {error}"))?;
    let mut file = File::create(&temp)
        .map_err(|error| format!("Failed to create outside-move journal: {error}"))?;
    file.write_all(&content)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Failed to persist outside-move journal: {error}"))?;
    fs::rename(&temp, &path)
        .map_err(|error| format!("Failed to commit outside-move journal: {error}"))?;
    sync_parent(&path);
    Ok(path)
}

fn remove_journal(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to remove outside-move journal: {error}"))?;
        sync_parent(path);
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to remove '{}': {error}", path.display()))?;
    }
    Ok(())
}

fn files_match(left: &Path, right: &Path) -> Result<bool, String> {
    let left_meta = fs::metadata(left)
        .map_err(|error| format!("Failed to inspect '{}': {error}", left.display()))?;
    let right_meta = fs::metadata(right)
        .map_err(|error| format!("Failed to inspect '{}': {error}", right.display()))?;
    if left_meta.len() != right_meta.len() {
        return Ok(false);
    }

    let hash = |path: &Path| -> Result<blake3::Hash, String> {
        let mut file = File::open(path)
            .map_err(|error| format!("Failed to open '{}': {error}", path.display()))?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize())
    };
    Ok(hash(left)? == hash(right)?)
}

enum JournalRowState {
    Missing,
    Matches,
    Changed(String),
}

fn db_row_state(journal: &OutsideMoveJournal) -> Result<JournalRowState, String> {
    let Some(file) = AFile::get_file_info_for_library(journal.file_id, &journal.library_id)? else {
        return Ok(JournalRowState::Missing);
    };
    let Some(path) = file.file_path.as_deref() else {
        return Ok(JournalRowState::Changed("<missing path>".to_string()));
    };
    if t_utils::paths_refer_to_same_item(Path::new(path), Path::new(&journal.source_path)) {
        Ok(JournalRowState::Matches)
    } else {
        Ok(JournalRowState::Changed(path.to_string()))
    }
}

/// A superseded destination is user media, not a scratch artifact: move it to the
/// system trash so a Replace stays recoverable, matching `delete_file`. A trash
/// failure keeps the file in place instead of destroying it, and never blocks
/// journal cleanup.
fn trash_superseded(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(());
    }
    let path_str = path.to_string_lossy().into_owned();
    if let Err(error) = t_utils::trash_path(&path_str) {
        eprintln!(
            "Superseded item could not be moved to Trash '{}': {}. It was kept in place.",
            path_str, error
        );
    }
    Ok(())
}

fn cleanup_completed(journal: &OutsideMoveJournal, journal_path: &Path) -> Result<(), String> {
    // `staged_path` is a host-created temp; `backup_path` is whatever the move
    // overwrote and belongs to the user.
    remove_file_if_exists(Path::new(&journal.staged_path))?;
    trash_superseded(Path::new(&journal.backup_path))?;
    remove_journal(journal_path)
}

fn recover_filesystem_while_row_exists(
    journal: &OutsideMoveJournal,
    journal_path: &Path,
) -> Result<bool, String> {
    let source = Path::new(&journal.source_path);
    if !source.exists() {
        return Ok(false);
    }

    let destination = Path::new(&journal.destination_path);
    let staged = Path::new(&journal.staged_path);
    let backup = Path::new(&journal.backup_path);
    remove_file_if_exists(staged)?;
    if backup.exists() {
        if destination.exists() {
            if !files_match(source, destination)? {
                return Err(format!(
                    "Destination changed after interrupted move: {}",
                    destination.display()
                ));
            }
            remove_file_if_exists(destination)?;
        }
        fs::rename(backup, destination).map_err(|error| {
            format!(
                "Failed to restore previous destination '{}' from '{}': {error}",
                destination.display(),
                backup.display()
            )
        })?;
    } else if destination.exists() {
        if journal.destination_existed {
            remove_journal(journal_path)?;
            return Ok(true);
        }
        if !files_match(source, destination)? {
            return Err(format!(
                "Destination changed after interrupted move: {}",
                destination.display()
            ));
        }
        remove_file_if_exists(destination)?;
    }
    remove_journal(journal_path)?;
    Ok(true)
}

fn recover_one(journal: &OutsideMoveJournal, path: &Path) -> Result<(), String> {
    match db_row_state(journal)? {
        JournalRowState::Missing => {
            let source = Path::new(&journal.source_path);
            let destination = Path::new(&journal.destination_path);
            if !source.exists() && destination.exists() {
                return cleanup_completed(journal, path);
            }
            return Err(format!(
                "Pending move DB row is missing but disk state is not a completed move (source_exists={}, destination_exists={})",
                source.exists(),
                destination.exists()
            ));
        }
        JournalRowState::Matches => {}
        JournalRowState::Changed(current_path) => {
            return Err(format!(
                "Pending move DB row changed path (library={}, file_id={}, expected={}, current={})",
                journal.library_id, journal.file_id, journal.source_path, current_path
            ));
        }
    }

    if recover_filesystem_while_row_exists(journal, path)? {
        return Ok(());
    }

    let source = Path::new(&journal.source_path);
    let destination = Path::new(&journal.destination_path);

    if destination.exists() {
        let deleted = AFile::delete_for_library(journal.file_id, &journal.library_id)?;
        if deleted != 1 {
            return Err(format!(
                "Pending move DB row was not deleted (library={}, file_id={}, deleted={})",
                journal.library_id, journal.file_id, deleted
            ));
        }
        return cleanup_completed(journal, path);
    }

    Err(format!(
        "Interrupted move is ambiguous; source and destination are both missing (source={}, destination={})",
        source.display(),
        destination.display()
    ))
}

pub(crate) fn move_file_outside_library(
    file_id: i64,
    file_path: &str,
    new_folder_path: &str,
    policy: FileConflictPolicy,
) -> Result<String, String> {
    let _guard = OUTSIDE_MOVE_LOCK
        .lock()
        .map_err(|_| "Outside-move recovery lock poisoned".to_string())?;
    let _operation_guard = t_utils::ImportGuard::acquire()?;
    let config = t_config::load_app_config()?;
    let library_id = config.current_library_id;
    let Some(file) = AFile::get_file_info_for_library(file_id, &library_id)? else {
        return Err(format!("File not found in current library: {file_id}"));
    };
    let Some(indexed_path) = file.file_path.as_deref() else {
        return Err(format!("Indexed file has no path: {file_id}"));
    };
    if !t_utils::paths_refer_to_same_item(Path::new(indexed_path), Path::new(file_path)) {
        return Err("File path does not match the current library record".to_string());
    }

    let destination =
        t_utils::resolve_file_transfer_destination(file_path, new_folder_path, policy)?;
    let id = Uuid::new_v4().to_string();
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let journal = OutsideMoveJournal {
        id: id.clone(),
        library_id,
        file_id,
        source_path: file_path.to_string(),
        destination_path: destination.to_string_lossy().into_owned(),
        destination_existed: destination.exists(),
        staged_path: parent
            .join(format!(".picaipic-outside-move-{id}.stage"))
            .to_string_lossy()
            .into_owned(),
        backup_path: parent
            .join(format!(".picaipic-outside-move-{id}.backup"))
            .to_string_lossy()
            .into_owned(),
    };
    let path = write_journal(&journal)?;

    let transfer = match t_utils::move_file_with_recovery_paths(
        file_path,
        &destination,
        policy,
        Path::new(&journal.staged_path),
        Path::new(&journal.backup_path),
    ) {
        Ok(transfer) => transfer,
        Err(error) => {
            let recovery_error = recover_filesystem_while_row_exists(&journal, &path).err();
            return Err(match recovery_error {
                Some(recovery_error) => {
                    format!("{error}; recovery also failed: {recovery_error}")
                }
                None => error,
            });
        }
    };

    match AFile::delete_for_library(file_id, &journal.library_id) {
        Ok(0 | 1) => {}
        Ok(deleted) => return Err(format!("Unexpected deleted row count: {deleted}")),
        Err(error) => {
            let rollback_error = transfer.rollback_move(Path::new(file_path)).err();
            if rollback_error.is_none() {
                let _ = remove_journal(&path);
            }
            return Err(match rollback_error {
                Some(rollback_error) => format!(
                    "Error while removing file from DB: {error}; rollback also failed: {rollback_error}"
                ),
                None => format!("Error while removing file from DB: {error}; move rolled back"),
            });
        }
    }

    let backup_path = transfer.backup_path().map(Path::to_path_buf);
    let final_path = transfer.finalize()?;
    if backup_path.as_ref().is_some_and(|backup| backup.exists()) {
        eprintln!(
            "Outside move completed but backup cleanup is pending: {}",
            backup_path.unwrap().display()
        );
    } else {
        remove_journal(&path)?;
    }
    Ok(final_path)
}

pub(crate) fn recover_pending_outside_moves() -> Result<(), String> {
    let _guard = OUTSIDE_MOVE_LOCK
        .lock()
        .map_err(|_| "Outside-move recovery lock poisoned".to_string())?;
    let dir = journal_dir()?;
    let mut failures = Vec::new();
    for entry in fs::read_dir(&dir)
        .map_err(|error| format!("Failed to read outside-move journal directory: {error}"))?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let result = fs::read(&path)
            .map_err(|error| format!("Failed to read '{}': {error}", path.display()))
            .and_then(|content| {
                serde_json::from_slice::<OutsideMoveJournal>(&content)
                    .map_err(|error| format!("Failed to parse '{}': {error}", path.display()))
            })
            .and_then(|journal| recover_one(&journal, &path));
        if let Err(error) = result {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::OutsideMoveJournal;
    use std::fs;
    use std::path::PathBuf;

    fn fixture(name: &str) -> (PathBuf, OutsideMoveJournal, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "picaipic-outside-move-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.jpg");
        let destination = root.join("destination.jpg");
        let staged = root.join("move.stage");
        let backup = root.join("move.backup");
        let journal_path = root.join("journal.json");
        let journal = OutsideMoveJournal {
            id: "test".to_string(),
            library_id: "test".to_string(),
            file_id: 1,
            source_path: source.to_string_lossy().into_owned(),
            destination_path: destination.to_string_lossy().into_owned(),
            destination_existed: true,
            staged_path: staged.to_string_lossy().into_owned(),
            backup_path: backup.to_string_lossy().into_owned(),
        };
        (root, journal, journal_path)
    }

    #[test]
    fn matching_source_and_destination_restore_replaced_target() {
        let (root, journal, journal_path) = fixture("restore-replace");
        fs::write(&journal.source_path, b"source").unwrap();
        fs::write(&journal.destination_path, b"source").unwrap();
        fs::write(&journal.backup_path, b"previous-target").unwrap();
        fs::write(&journal_path, b"journal").unwrap();

        recover_filesystem_while_row_exists(&journal, &journal_path).unwrap();

        assert_eq!(fs::read(&journal.source_path).unwrap(), b"source");
        assert_eq!(
            fs::read(&journal.destination_path).unwrap(),
            b"previous-target"
        );
        assert!(!PathBuf::from(&journal.backup_path).exists());
        assert!(!journal_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changed_destination_is_left_untouched_for_manual_recovery() {
        let (root, journal, journal_path) = fixture("changed-destination");
        fs::write(&journal.source_path, b"source").unwrap();
        fs::write(&journal.destination_path, b"changed").unwrap();
        fs::write(&journal.backup_path, b"previous-target").unwrap();
        fs::write(&journal_path, b"journal").unwrap();

        let error = recover_filesystem_while_row_exists(&journal, &journal_path).unwrap_err();

        assert!(error.contains("Destination changed"));
        assert_eq!(fs::read(&journal.destination_path).unwrap(), b"changed");
        assert_eq!(fs::read(&journal.backup_path).unwrap(), b"previous-target");
        assert!(journal_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepared_replace_keeps_original_destination() {
        let (root, journal, journal_path) = fixture("prepared-replace");
        fs::write(&journal.source_path, b"source").unwrap();
        fs::write(&journal.destination_path, b"previous-target").unwrap();
        fs::write(&journal.staged_path, b"partial-stage").unwrap();
        fs::write(&journal_path, b"journal").unwrap();

        recover_filesystem_while_row_exists(&journal, &journal_path).unwrap();

        assert_eq!(fs::read(&journal.source_path).unwrap(), b"source");
        assert_eq!(
            fs::read(&journal.destination_path).unwrap(),
            b"previous-target"
        );
        assert!(!PathBuf::from(&journal.staged_path).exists());
        assert!(!journal_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn recover_filesystem_while_row_exists(
        journal: &OutsideMoveJournal,
        journal_path: &std::path::Path,
    ) -> Result<(), String> {
        super::recover_filesystem_while_row_exists(journal, journal_path).map(|_| ())
    }
}
