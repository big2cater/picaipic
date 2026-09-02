use crate::t_config;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const JOURNAL_DIR_NAME: &str = "output-temp-journal";

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputTempKind {
    Batch,
    Collage,
    PhotoFrame,
    /// A library file renamed aside before its DB row is deleted. Unlike the others
    /// it holds user media, so it is restored rather than removed.
    StagedDelete,
}

impl OutputTempKind {
    fn slug(self) -> &'static str {
        match self {
            Self::Batch => "batch",
            Self::Collage => "collage",
            Self::PhotoFrame => "photo-frame",
            Self::StagedDelete => "staged-delete",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputTempJournal {
    id: String,
    kind: OutputTempKind,
    temp_path: PathBuf,
    /// Set only for `StagedDelete`: the path the temp must be restored to.
    /// `#[serde(default)]` keeps journals written by older builds readable.
    #[serde(default)]
    original_path: Option<PathBuf>,
}

/// What dropping the guard must do with `temp_path`.
#[derive(Debug)]
enum DropAction {
    /// Scratch output that is safe to delete (batch/collage/photo-frame).
    RemoveTemp,
    /// User media staged for deletion: rename it back instead of deleting it.
    RestoreToOriginal(PathBuf),
}

#[derive(Debug)]
pub struct TrackedOutputTemp {
    temp_path: PathBuf,
    journal_path: PathBuf,
    drop_action: DropAction,
}

impl TrackedOutputTemp {
    pub fn create(destination: &Path, kind: OutputTempKind) -> Result<Self, String> {
        let journal_dir = t_config::get_app_data_dir()?.join(JOURNAL_DIR_NAME);
        Self::create_in(destination, kind, &journal_dir)
    }

    /// Register a library file that is about to be renamed aside before its DB row
    /// is deleted. The temp holds user media, so dropping the guard (and startup
    /// recovery) restore it instead of deleting it.
    pub fn create_staged_delete(original: &Path) -> Result<Self, String> {
        let journal_dir = t_config::get_app_data_dir()?.join(JOURNAL_DIR_NAME);
        Self::create_staged_delete_in(original, &journal_dir)
    }

    fn create_in(
        destination: &Path,
        kind: OutputTempKind,
        journal_dir: &Path,
    ) -> Result<Self, String> {
        Self::create_journaled(destination, kind, journal_dir, None)
    }

    fn create_staged_delete_in(original: &Path, journal_dir: &Path) -> Result<Self, String> {
        Self::create_journaled(
            original,
            OutputTempKind::StagedDelete,
            journal_dir,
            Some(original.to_path_buf()),
        )
    }

    fn create_journaled(
        destination: &Path,
        kind: OutputTempKind,
        journal_dir: &Path,
        original_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        fs::create_dir_all(journal_dir).map_err(|error| {
            format!(
                "Failed to create output-temp journal directory '{}': {error}",
                journal_dir.display()
            )
        })?;

        let id = Uuid::new_v4().to_string();
        let destination_name = destination
            .file_name()
            .ok_or_else(|| "Output destination must have a file name".to_string())?
            .to_string_lossy();
        let temp_name = format!("{destination_name}.picaipic-{}-{id}.tmp", kind.slug());
        let temp_path = destination.with_file_name(temp_name);
        let journal_path = journal_dir.join(format!("{id}.json"));
        let journal = OutputTempJournal {
            id,
            kind,
            temp_path: temp_path.clone(),
            original_path: original_path.clone(),
        };
        persist_journal(&journal_path, &journal)?;

        let drop_action = match original_path {
            Some(original) => DropAction::RestoreToOriginal(original),
            None => DropAction::RemoveTemp,
        };

        Ok(Self {
            temp_path,
            journal_path,
            drop_action,
        })
    }

    pub fn path(&self) -> &Path {
        &self.temp_path
    }
}

/// Put a staged-delete temp back at its original path.
///
/// No-op when the temp is already gone, and refuses to clobber a path that exists
/// (the user may have recreated the file since). Losing the DB delete is the safe
/// direction: if the row was already removed the file is simply re-indexed on the
/// next scan, whereas deleting it would destroy media the user never confirmed.
fn restore_staged_delete(temp: &Path, original: &Path) -> Result<(), String> {
    if !temp.exists() {
        return Ok(());
    }
    if original.exists() {
        return Err(format!(
            "Refusing to restore staged delete '{}' over the existing path '{}'",
            temp.display(),
            original.display()
        ));
    }
    if let Some(parent) = original.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to recreate directory '{}': {error}",
                    parent.display()
                )
            })?;
        }
    }
    fs::rename(temp, original).map_err(|error| {
        format!(
            "Failed to restore staged delete '{}' to '{}': {error}",
            temp.display(),
            original.display()
        )
    })
}

impl Drop for TrackedOutputTemp {
    fn drop(&mut self) {
        // A staged delete holds user media: restore it rather than deleting it, and
        // keep the journal when the restore fails so the next start can retry.
        if let DropAction::RestoreToOriginal(original) = &self.drop_action {
            if let Err(error) = restore_staged_delete(&self.temp_path, original) {
                eprintln!(
                    "Failed to restore staged delete '{}' to '{}'; keeping it in place: {error}",
                    self.temp_path.display(),
                    original.display()
                );
                return;
            }
            if let Err(error) = remove_journal(&self.journal_path) {
                eprintln!("Failed to remove completed output-temp journal: {error}");
            }
            return;
        }

        match remove_file_if_present(&self.temp_path) {
            Ok(()) => {
                if let Err(error) = remove_journal(&self.journal_path) {
                    eprintln!("Failed to remove completed output-temp journal: {error}");
                }
            }
            Err(error) => {
                eprintln!(
                    "Failed to remove tracked output temp '{}'; retaining journal '{}': {error}",
                    self.temp_path.display(),
                    self.journal_path.display()
                );
            }
        }
    }
}

fn persist_journal(path: &Path, journal: &OutputTempJournal) -> Result<(), String> {
    let content = serde_json::to_vec_pretty(journal)
        .map_err(|error| format!("Failed to serialize output-temp journal: {error}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            format!(
                "Failed to create output-temp journal '{}': {error}",
                path.display()
            )
        })?;
    if let Err(error) = file.write_all(&content).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!(
            "Failed to persist output-temp journal '{}': {error}",
            path.display()
        ));
    }
    sync_parent(path);
    Ok(())
}

fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
}

fn remove_file_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_journal(path: &Path) -> Result<(), String> {
    remove_file_if_present(path)?;
    sync_parent(path);
    Ok(())
}

fn validate_journal(path: &Path, journal: &OutputTempJournal) -> Result<(), String> {
    let file_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "journal file name is not valid UTF-8".to_string())?;
    let parsed_id = Uuid::parse_str(&journal.id)
        .map_err(|error| format!("journal id is not a UUID: {error}"))?;
    if parsed_id.to_string() != journal.id || file_id != journal.id {
        return Err("journal file name and canonical UUID do not match".to_string());
    }

    let expected_suffix = format!(".picaipic-{}-{}.tmp", journal.kind.slug(), journal.id);
    let temp_name = journal
        .temp_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "temporary file name is not valid UTF-8".to_string())?;
    if !temp_name.ends_with(&expected_suffix) || temp_name == expected_suffix {
        return Err(
            "temporary file name does not match the registered output kind and UUID".into(),
        );
    }
    Ok(())
}

fn recover_in(journal_dir: &Path) -> Result<usize, String> {
    if !journal_dir.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(journal_dir).map_err(|error| {
        format!(
            "Failed to read output-temp journal directory '{}': {error}",
            journal_dir.display()
        )
    })?;
    let mut recovered = 0;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("Failed to inspect an output-temp journal entry: {error}");
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let result = (|| -> Result<(), String> {
            let content = fs::read(&path).map_err(|error| error.to_string())?;
            let journal: OutputTempJournal =
                serde_json::from_slice(&content).map_err(|error| error.to_string())?;
            validate_journal(&path, &journal)?;
            // Staged deletes hold user media and are restored; everything else is
            // scratch output that can be removed.
            match journal.original_path.as_deref() {
                Some(original) => restore_staged_delete(&journal.temp_path, original)?,
                None => remove_file_if_present(&journal.temp_path)?,
            }
            remove_journal(&path)?;
            Ok(())
        })();
        match result {
            Ok(()) => recovered += 1,
            Err(error) => eprintln!(
                "Retaining invalid or unrecoverable output-temp journal '{}': {error}",
                path.display()
            ),
        }
    }
    Ok(recovered)
}

pub fn recover_tracked_output_temps() -> Result<usize, String> {
    let journal_dir = t_config::get_app_data_dir()?.join(JOURNAL_DIR_NAME);
    recover_in(&journal_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("picaipic-output-temp-{label}-{}", Uuid::new_v4()))
    }

    #[test]
    fn guard_drop_removes_temp_and_journal() {
        let root = test_root("drop");
        let output_dir = root.join("exports");
        let journal_dir = root.join("journals");
        fs::create_dir_all(&output_dir).unwrap();
        let guard = TrackedOutputTemp::create_in(
            &output_dir.join("photo.jpg"),
            OutputTempKind::Batch,
            &journal_dir,
        )
        .unwrap();
        fs::write(guard.path(), b"partial").unwrap();
        let temp_path = guard.path().to_path_buf();
        drop(guard);

        assert!(!temp_path.exists());
        assert_eq!(fs::read_dir(&journal_dir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn startup_recovery_removes_only_registered_temp() {
        let root = test_root("recovery");
        let output_dir = root.join("exports");
        let journal_dir = root.join("journals");
        fs::create_dir_all(&output_dir).unwrap();
        let guard = TrackedOutputTemp::create_in(
            &output_dir.join("collage.png"),
            OutputTempKind::Collage,
            &journal_dir,
        )
        .unwrap();
        fs::write(guard.path(), b"partial").unwrap();
        let temp_path = guard.path().to_path_buf();
        std::mem::forget(guard);

        assert_eq!(recover_in(&journal_dir).unwrap(), 1);
        assert!(!temp_path.exists());
        assert_eq!(fs::read_dir(&journal_dir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn staged_delete_is_restored_instead_of_deleted() {
        let root = test_root("staged-delete");
        let library = root.join("library");
        let journal_dir = root.join("journals");
        fs::create_dir_all(&library).unwrap();
        let original = library.join("photo.jpg");
        fs::write(&original, b"user-media").unwrap();

        let guard = TrackedOutputTemp::create_staged_delete_in(&original, &journal_dir).unwrap();
        let staged = guard.path().to_path_buf();
        // Simulate the delete flow: rename aside first, then delete the DB row.
        fs::rename(&original, &staged).unwrap();
        assert!(!original.exists());

        // Crash before finalize: the guard's drop never runs.
        std::mem::forget(guard);

        assert_eq!(recover_in(&journal_dir).unwrap(), 1);
        assert_eq!(
            fs::read(&original).unwrap(),
            b"user-media",
            "recovery must restore the user's media, not delete it"
        );
        assert!(!staged.exists());
        assert_eq!(fs::read_dir(&journal_dir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn staged_delete_guard_restores_on_early_drop() {
        let root = test_root("staged-delete-drop");
        let library = root.join("library");
        let journal_dir = root.join("journals");
        fs::create_dir_all(&library).unwrap();
        let original = library.join("photo.jpg");
        fs::write(&original, b"user-media").unwrap();

        let guard = TrackedOutputTemp::create_staged_delete_in(&original, &journal_dir).unwrap();
        let staged = guard.path().to_path_buf();
        fs::rename(&original, &staged).unwrap();

        // An early return between staging and finalizing must not destroy the file.
        drop(guard);

        assert_eq!(fs::read(&original).unwrap(), b"user-media");
        assert!(!staged.exists());
        assert_eq!(fs::read_dir(&journal_dir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mismatched_journal_cannot_delete_an_arbitrary_file() {
        let root = test_root("invalid");
        let journal_dir = root.join("journals");
        let arbitrary = root.join("keep-me.txt");
        fs::create_dir_all(&journal_dir).unwrap();
        fs::write(&arbitrary, b"keep").unwrap();
        let journal_id = Uuid::new_v4().to_string();
        let journal = OutputTempJournal {
            id: Uuid::new_v4().to_string(),
            kind: OutputTempKind::Batch,
            temp_path: arbitrary.clone(),
            original_path: None,
        };
        let journal_path = journal_dir.join(format!("{journal_id}.json"));
        persist_journal(&journal_path, &journal).unwrap();

        assert_eq!(recover_in(&journal_dir).unwrap(), 0);
        assert!(arbitrary.exists());
        assert!(journal_path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
