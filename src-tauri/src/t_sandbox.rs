//! Plugin process sandbox helpers.
//!
//! The default runtime path stages external input files into the plugin task
//! directory before invocation. That keeps normal plugin runs from needing
//! direct access to arbitrary source-image paths.
//!
//! The old v1 write-confinement path is still available for explicit testing:
//! set `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` to apply a temporary Windows
//! deny-ACE (`icacls /deny <user>:(W)`) on sensitive user directories before
//! the plugin process is spawned. The handle revokes those ACEs on drop.
//!
//! The ACL mode is opt-in because it changes ACLs on real user directories
//! for the current Windows account, so it can surface confusing system access
//! prompts in the host UI while a plugin is running.
// Fields/methods are part of the sandbox's public surface used by t_plugin
// and reserved for future diagnostics; allow dead code at module level.
#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Records the paths this sandbox has applied a deny-ACE on, so they can be
/// revoked when the owning plugin process is torn down.
#[derive(Default)]
pub struct SandboxHandle {
    plugin_id: String,
    user: String,
    denied_paths: Vec<PathBuf>,
}

impl SandboxHandle {
    fn inactive(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            user: String::new(),
            denied_paths: Vec::new(),
        }
    }

    fn new(plugin_id: &str, user: String) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            user,
            denied_paths: Vec::new(),
        }
    }

    /// Whether any deny-ACE was actually applied. `false` on non-Windows,
    /// dev mode, or when there is nothing sensitive to deny.
    pub fn is_active(&self) -> bool {
        !self.denied_paths.is_empty()
    }

    /// Human-readable summary for the start log.
    pub fn summary(&self) -> String {
        if self.denied_paths.is_empty() {
            if cfg!(target_os = "windows") {
                if sandbox_disabled() {
                    "disabled (PICAIPIC_DISABLE_PLUGIN_SANDBOX is set)".to_string()
                } else {
                    "deny-ACL disabled (set PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1 to enable)"
                        .to_string()
                }
            } else {
                "disabled (non-windows)".to_string()
            }
        } else {
            format!(
                "applied deny-W on {} path(s) for user '{}'",
                self.denied_paths.len(),
                self.user
            )
        }
    }
}

impl Drop for SandboxHandle {
    fn drop(&mut self) {
        if self.denied_paths.is_empty() {
            return;
        }
        // Best-effort revoke. icacls /remove:d is idempotent: removing an ACE
        // that does not exist is a no-op, so a leftover from a crashed prior
        // run is cleaned up safely on the next apply.
        //
        // The icacls helpers are Windows-only; on other platforms
        // `denied_paths` is always empty (see `SandboxHandle::inactive`),
        // so this body is unreachable there and gated to keep the compiler
        // happy without resolving Windows-only symbols.
        #[cfg(target_os = "windows")]
        {
            let rt = match tokio::runtime::Handle::try_current() {
                Ok(rt) => rt,
                Err(_) => {
                    // No tokio runtime available (e.g. dropping during shutdown).
                    // Fall back to a blocking std thread so we still attempt cleanup.
                    let paths = std::mem::take(&mut self.denied_paths);
                    let user = self.user.clone();
                    std::thread::spawn(move || {
                        for p in &paths {
                            let _ = run_icacls_blocking(p, &user, IcaclsOp::RemoveDeny);
                        }
                    });
                    return;
                }
            };
            let paths = std::mem::take(&mut self.denied_paths);
            let user = self.user.clone();
            rt.spawn(async move {
                for p in &paths {
                    let _ = run_icacls_async(p, &user, IcaclsOp::RemoveDeny).await;
                }
            });
        }
    }
}

/// Apply optional Windows deny-ACL write confinement before spawning a plugin.
///
/// `writable_dirs` are the host-designated directories the plugin must be
/// able to write to (plugin-data/cache/outputs, plugin root, runtime dirs).
/// Any sensitive path that falls inside one of these is skipped so we never
/// deny the plugin's own working area.
///
/// Returns a handle whose drop revokes the applied deny-ACEs. On non-Windows,
/// when the sandbox is disabled (`PICAIPIC_DISABLE_PLUGIN_SANDBOX=1`), or
/// when ACL mode is not explicitly enabled, returns an inactive handle.
pub async fn apply_plugin_sandbox(
    plugin_id: &str,
    writable_dirs: &[PathBuf],
) -> Result<SandboxHandle, String> {
    // Non-Windows: sandbox is a no-op; return an inactive handle.
    #[cfg(not(target_os = "windows"))]
    {
        let _ = writable_dirs;
        return Ok(SandboxHandle::inactive(plugin_id));
    }

    // Windows path below. ACL write confinement is opt-in because it modifies
    // real directory ACLs for the current user account while the plugin runs.
    #[cfg(target_os = "windows")]
    {
        if sandbox_disabled() {
            return Ok(SandboxHandle::inactive(plugin_id));
        }

        let user = current_username()
            .ok_or_else(|| "Cannot determine current user for sandbox ACL".to_string())?;
        let targets = sensitive_write_targets(&user);

        if !acl_sandbox_enabled() {
            // Clean up stale deny ACEs left by older builds or crashed runs,
            // then run without touching real user-directory ACLs.
            for target in &targets {
                let _ = run_icacls_async(target, &user, IcaclsOp::RemoveDeny).await;
            }
            return Ok(SandboxHandle::inactive(plugin_id));
        }

        let mut handle = SandboxHandle::new(plugin_id, user.clone());

        // Idempotent pre-clean: revoke any leftover deny-ACE from a prior
        // crashed run before re-applying. /remove:d is a no-op if absent.
        for target in &targets {
            let _ = run_icacls_async(target, &user, IcaclsOp::RemoveDeny).await;
        }

        for target in targets {
            // Never deny a path the plugin must be able to write to.
            if writable_dirs.iter().any(|w| is_path_inside(&target, w)) {
                continue;
            }
            // Skip if the sensitive dir does not exist on this machine.
            if !target.is_dir() {
                continue;
            }
            match run_icacls_async(&target, &user, IcaclsOp::DenyWrite).await {
                Ok(()) => handle.denied_paths.push(target),
                Err(e) => {
                    // Log but continue — a single failed deny should not block
                    // plugin startup. The remaining paths still get confined.
                    eprintln!("[sandbox] failed to deny-W on {}: {}", target.display(), e);
                }
            }
        }
        Ok(handle)
    }
}

pub fn sandbox_disabled() -> bool {
    env_flag_enabled("PICAIPIC_DISABLE_PLUGIN_SANDBOX")
}

fn acl_sandbox_enabled() -> bool {
    env_flag_enabled("PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX")
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var_os(name)
        .map(|v| {
            let s = v.to_string_lossy().to_ascii_lowercase();
            s == "1" || s == "true" || s == "yes"
        })
        .unwrap_or(false)
}

/// Whether input staging is active for this build/config. When false, staging
/// is skipped and the plugin receives original source paths directly.
pub fn sandbox_enabled() -> bool {
    cfg!(target_os = "windows") && !sandbox_disabled()
}

#[cfg(target_os = "windows")]
fn current_username() -> Option<String> {
    std::env::var_os("USERNAME")
        .filter(|s| !s.is_empty())
        .and_then(|s| s.into_string().ok())
        .or_else(|| {
            // Fallback: derive from USERPROFILE (e.g. C:\Users\alice -> alice).
            std::env::var_os("USERPROFILE").and_then(|p| {
                PathBuf::from(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
        })
}

/// Build the list of sensitive user directories to deny writes to.
///
/// These are the high-value targets a malicious plugin could damage or
/// exfiltrate-by-overwrite. We deny writes to the directories themselves
/// (non-recursive, `/L`) — the plugin can still read them, and subdirectory
/// writes outside these exact dirs are not blocked (a deliberate v1
/// trade-off to avoid breaking GPU/runtime access).
#[cfg(target_os = "windows")]
fn sensitive_write_targets(user: &str) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        for sub in ["Desktop", "Documents", "Pictures", "Videos"] {
            targets.push(home.join(sub));
        }
    }
    // Extra deny paths from env (semicolon-separated), for custom library
    // roots the user wants protected.
    if let Some(extra) = std::env::var_os("PICAIPIC_SANDBOX_DENY_PATHS") {
        for p in extra.to_string_lossy().split(';') {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                targets.push(PathBuf::from(trimmed));
            }
        }
    }
    let _ = user; // user is used by the caller for the ACE trustee
    targets
}

#[derive(Clone, Copy)]
enum IcaclsOp {
    DenyWrite,
    RemoveDeny,
}

/// Asynchronous icacls invocation used on the apply path (we are inside a
/// tokio runtime and want to capture stdout/stderr for diagnostics).
#[cfg(target_os = "windows")]
async fn run_icacls_async(path: &Path, user: &str, op: IcaclsOp) -> Result<(), String> {
    let mut cmd = Command::new("icacls");
    cmd.arg(path);
    match op {
        IcaclsOp::DenyWrite => {
            cmd.args(["/deny", &format!("{}:(W)", user), "/L"]);
        }
        IcaclsOp::RemoveDeny => {
            cmd.args(["/remove:d", user]);
        }
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("icacls spawn failed: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "icacls failed (rc={}): {} {}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ))
    }
}

/// Best-effort synchronous icacls used from `Drop` when no async runtime is
/// available (e.g. process shutdown). Detaches the child and does not wait.
#[cfg(target_os = "windows")]
fn run_icacls_blocking(path: &Path, user: &str, op: IcaclsOp) -> Result<(), String> {
    let mut cmd = std::process::Command::new("icacls");
    cmd.arg(path);
    match op {
        IcaclsOp::DenyWrite => {
            cmd.args(["/deny", &format!("{}:(W)", user), "/L"]);
        }
        IcaclsOp::RemoveDeny => {
            cmd.args(["/remove:d", user]);
        }
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    use std::os::windows::process::CommandExt as _;
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd.spawn();
    Ok(())
}

fn is_path_inside(path: &Path, root: &Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    path.starts_with(root)
}
