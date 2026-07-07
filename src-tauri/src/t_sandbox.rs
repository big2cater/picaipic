//! Plugin process sandbox — Approach C (v1: deny-ACL write confinement).
//!
//! Security model: a plugin process is confined so it cannot **write** to
//! sensitive user directories (Desktop, Documents, Pictures, Videos, plus
//! any extra paths supplied via `PICAIPIC_SANDBOX_DENY_PATHS`). Writes to
//! the plugin's own designated directories remain allowed.
//!
//! Mechanism: the `icacls` command applies a deny-ACE (`/deny <user>:(W)`,
//! non-recursive via `/L`) on each sensitive path before the plugin
//! process is spawned. The `SandboxHandle` records the paths it successfully
//! denied and revokes them (`/remove:d`) on drop — so revocation is tied to
//! the lifetime of the `RunningPlugin` that owns the handle.
//!
//! Why deny-ACL and not a restricted token / Job Object: the security doc
//! (docs/ai-plugin-security-hardening.md) notes that restricted tokens may
//! drop GPU access. The sandbox_gpu_spike confirmed that `icacls /deny` on a
//! directory does **not** break ROCm/CUDA driver initialization — the plugin
//! can still run GPU work — while still blocking writes into the denied dir.
//!
//! v1 scope: write confinement only. Network blocking, macOS Seatbelt, and
//! Linux seccomp are future work (see security-hardening doc).
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
                "disabled (no sensitive paths or dev mode)".to_string()
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

/// Apply the plugin sandbox before spawning the plugin process.
///
/// `writable_dirs` are the host-designated directories the plugin must be
/// able to write to (plugin-data/cache/outputs, plugin root, runtime dirs).
/// Any sensitive path that falls inside one of these is skipped so we never
/// deny the plugin's own working area.
///
/// Returns a handle whose drop revokes the applied deny-ACEs. On non-Windows
/// or when the sandbox is disabled (`PICAIPIC_DISABLE_PLUGIN_SANDBOX=1`),
/// returns an inactive handle.
pub fn apply_plugin_sandbox(
    plugin_id: &str,
    writable_dirs: &[PathBuf],
) -> Result<SandboxHandle, String> {
    // Non-Windows: sandbox is a no-op; return an inactive handle.
    #[cfg(not(target_os = "windows"))]
    {
        let _ = writable_dirs;
        return Ok(SandboxHandle::inactive(plugin_id));
    }

    // Windows path below. `sandbox_disabled()` short-circuits to an inactive
    // handle so dev mode (`PICAIPIC_DISABLE_PLUGIN_SANDBOX=1`) skips ACL work.
    #[cfg(target_os = "windows")]
    {
        if sandbox_disabled() {
            return Ok(SandboxHandle::inactive(plugin_id));
        }

        let user = current_username()
            .ok_or_else(|| "Cannot determine current user for sandbox ACL".to_string())?;
        let targets = sensitive_write_targets(&user);
        let mut handle = SandboxHandle::new(plugin_id, user.clone());

        let rt = tokio::runtime::Handle::try_current()
            .map_err(|e| format!("Sandbox apply requires a tokio runtime: {}", e))?;

        // Idempotent pre-clean: revoke any leftover deny-ACE from a prior
        // crashed run before re-applying. /remove:d is a no-op if absent.
        for target in &targets {
            let _ = rt.block_on(run_icacls_async(target, &user, IcaclsOp::RemoveDeny));
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
            match rt.block_on(run_icacls_async(&target, &user, IcaclsOp::DenyWrite)) {
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
    std::env::var_os("PICAIPIC_DISABLE_PLUGIN_SANDBOX")
        .map(|v| {
            let s = v.to_string_lossy().to_ascii_lowercase();
            s == "1" || s == "true" || s == "yes"
        })
        .unwrap_or(false)
}

/// Whether the sandbox is in effect for this build/config. When false,
/// input staging is skipped (no need to copy source files into the plugin's
/// readable area — the plugin can read them directly).
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
