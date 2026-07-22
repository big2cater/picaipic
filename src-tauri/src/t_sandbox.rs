//! Plugin process sandbox helpers.
//!
//! The default runtime path stages external input files into the plugin task
//! directory before invocation on **every supported platform**. That keeps
//! normal plugin runs from needing direct access to arbitrary source-image
//! paths. Staging is a host-side copy/rewrite and does not require OS
//! sandbox APIs.
//!
//! The Windows write-confinement path is still available for explicit testing:
//! set `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` to apply a temporary Windows
//! deny-ACE (`icacls /deny <user>:(W)`) on sensitive user directories before
//! the plugin process is spawned. The handle revokes those ACEs on drop.
//!
//! The ACL mode is opt-in because it changes ACLs on real user directories
//! for the current Windows account, so it can surface confusing system access
//! prompts in the host UI while a plugin is running.
//!
//! Phase 3–5 experimental flags:
//! - `PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX=1` — when runtime network is **not** granted,
//!   attempt OS outbound block (Windows: per-program firewall rule; soft-fail if
//!   elevation/API unavailable). Loopback for plugin HTTP is expected to keep working.
//! - `PICAIPIC_ENABLE_LINUX_LANDLOCK=1` — Linux Landlock FS ruleset on plugin start
//!   (soft-fail if kernel/ABI missing); RO system+plugin roots, RW plugin writable roots
//! - `PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1` — `env_clear` + allowlist rebuild on plugin start/setup
//!
//! See `docs/ai-plugin-sandbox-roadmap.md`. Do not claim full kernel isolation;
//! Phase 3 is opt-in and may soft-fail to policy-only mode.
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

/// Opt-in Phase 3: attempt OS outbound network block when runtime network is not granted.
pub fn network_sandbox_flag_enabled() -> bool {
    env_flag_enabled("PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX")
}

/// Opt-in Phase 4: Linux Landlock filesystem confinement for plugin children.
pub fn linux_landlock_flag_enabled() -> bool {
    env_flag_enabled("PICAIPIC_ENABLE_LINUX_LANDLOCK")
}

/// Opt-in Phase 5: rebuild plugin process env from an allowlist (still off by default).
pub fn env_hygiene_flag_enabled() -> bool {
    env_flag_enabled("PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE")
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var_os(name)
        .map(|v| {
            let s = v.to_string_lossy().to_ascii_lowercase();
            s == "1" || s == "true" || s == "yes"
        })
        .unwrap_or(false)
}

/// Whether default input staging is active for this build/config.
///
/// Staging is platform-agnostic (host copies external inputs into the task
/// directory and rewrites payload `path` fields). When false, staging is
/// skipped and the plugin receives original source paths directly.
/// Windows deny-ACL write confinement is controlled separately by
/// `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX` and is never implied by this flag.
pub fn sandbox_enabled() -> bool {
    !sandbox_disabled()
}

/// Decision for Phase 3 network policy (before OS apply).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkSandboxDecision {
    /// Master off or feature flag off — do nothing.
    NotEnforced,
    /// Flag on and user granted runtime network — allow outbound.
    AllowOutbound,
    /// Flag on and no runtime network grant — attempt outbound block.
    BlockOutbound,
}

pub fn decide_network_sandbox(runtime_network_granted: bool) -> NetworkSandboxDecision {
    if sandbox_disabled() || !network_sandbox_flag_enabled() {
        NetworkSandboxDecision::NotEnforced
    } else if runtime_network_granted {
        NetworkSandboxDecision::AllowOutbound
    } else {
        NetworkSandboxDecision::BlockOutbound
    }
}

/// Result of applying (or skipping) Phase 3 network confinement.
#[derive(Debug, Clone)]
pub struct NetworkSandboxStatus {
    pub decision: NetworkSandboxDecision,
    /// `not_enforced` | `allow` | `blocked` | `policy_only` | `soft_fail`
    pub mode: String,
    pub detail: String,
}

impl NetworkSandboxStatus {
    pub fn summary(&self) -> String {
        format!("network_os: {} ({})", self.mode, self.detail)
    }

    /// Value for `PICAIPIC_PLUGIN_NETWORK_POLICY` (`allow` | `deny` | `unrestricted`).
    pub fn policy_env_value(&self) -> &'static str {
        match self.decision {
            NetworkSandboxDecision::NotEnforced => "unrestricted",
            NetworkSandboxDecision::AllowOutbound => "allow",
            NetworkSandboxDecision::BlockOutbound => "deny",
        }
    }
}

/// Holds OS resources that must be released when the plugin stops (e.g. firewall rules).
#[derive(Default)]
pub struct NetworkSandboxHandle {
    plugin_id: String,
    /// Windows firewall rule name to delete on drop.
    firewall_rule_name: Option<String>,
    status: Option<NetworkSandboxStatus>,
}

impl NetworkSandboxHandle {
    pub fn status(&self) -> Option<&NetworkSandboxStatus> {
        self.status.as_ref()
    }

    pub fn summary(&self) -> String {
        self.status
            .as_ref()
            .map(|s| s.summary())
            .unwrap_or_else(|| "network_os: not_enforced".to_string())
    }
}

impl Drop for NetworkSandboxHandle {
    fn drop(&mut self) {
        if let Some(rule) = self.firewall_rule_name.take() {
            #[cfg(target_os = "windows")]
            {
                let _ = delete_windows_firewall_rule(&rule);
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = rule;
            }
        }
        let _ = self.plugin_id;
    }
}

/// Apply Phase 3 network policy for a plugin process.
///
/// - Flag off / master sandbox off → no-op status.
/// - Runtime network granted → allow (no OS block).
/// - Otherwise attempt OS outbound block for `program_path` (Windows firewall);
///   on failure return `policy_only` / `soft_fail` but still set cooperative
///   `PICAIPIC_PLUGIN_NETWORK_POLICY=deny` via the returned status.
///
/// Never fails hard — soft-fail so GPU plugins still start.
pub fn apply_network_sandbox(
    plugin_id: &str,
    program_path: &Path,
    runtime_network_granted: bool,
) -> NetworkSandboxHandle {
    let decision = decide_network_sandbox(runtime_network_granted);
    let mut handle = NetworkSandboxHandle {
        plugin_id: plugin_id.to_string(),
        firewall_rule_name: None,
        status: None,
    };

    match decision {
        NetworkSandboxDecision::NotEnforced => {
            handle.status = Some(NetworkSandboxStatus {
                decision,
                mode: "not_enforced".to_string(),
                detail: if sandbox_disabled() {
                    "PICAIPIC_DISABLE_PLUGIN_SANDBOX is set".to_string()
                } else {
                    "set PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX=1 to enable".to_string()
                },
            });
        }
        NetworkSandboxDecision::AllowOutbound => {
            handle.status = Some(NetworkSandboxStatus {
                decision,
                mode: "allow".to_string(),
                detail: "runtime network granted".to_string(),
            });
        }
        NetworkSandboxDecision::BlockOutbound => {
            #[cfg(target_os = "windows")]
            {
                let rule_name = format!("PicAiPicPluginNetDeny-{}", sanitize_rule_id(plugin_id));
                match add_windows_firewall_outbound_block(&rule_name, program_path) {
                    Ok(()) => {
                        handle.firewall_rule_name = Some(rule_name.clone());
                        handle.status = Some(NetworkSandboxStatus {
                            decision,
                            mode: "blocked".to_string(),
                            detail: format!(
                                "windows firewall outbound block on {} (rule {})",
                                program_path.display(),
                                rule_name
                            ),
                        });
                    }
                    Err(err) => {
                        handle.status = Some(NetworkSandboxStatus {
                            decision,
                            mode: "policy_only".to_string(),
                            detail: format!(
                                "OS block soft-failed ({}); cooperative PICAIPIC_PLUGIN_NETWORK_POLICY=deny only",
                                err
                            ),
                        });
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = program_path;
                // Full netns would break loopback unless `lo` is brought up; defer.
                handle.status = Some(NetworkSandboxStatus {
                    decision,
                    mode: "policy_only".to_string(),
                    detail: "non-windows: no OS outbound block yet; cooperative policy=deny only"
                        .to_string(),
                });
            }
        }
    }

    handle
}

fn sanitize_rule_id(plugin_id: &str) -> String {
    plugin_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

#[cfg(target_os = "windows")]
fn add_windows_firewall_outbound_block(rule_name: &str, program_path: &Path) -> Result<(), String> {
    let program = program_path
        .canonicalize()
        .unwrap_or_else(|_| program_path.to_path_buf());
    let program_str = program.to_string_lossy();
    // Remove any leftover rule with the same name (idempotent).
    let _ = delete_windows_firewall_rule(rule_name);

    let mut cmd = std::process::Command::new("netsh");
    cmd.args([
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={}", rule_name),
        "dir=out",
        "action=block",
        &format!("program={}", program_str),
        "enable=yes",
        "profile=any",
    ]);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("netsh spawn failed: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!(
            "netsh add rule failed (admin may be required): {} {}",
            stdout.trim(),
            stderr.trim()
        )
        .trim()
        .to_string())
    }
}

#[cfg(target_os = "windows")]
fn delete_windows_firewall_rule(rule_name: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("netsh");
    cmd.args([
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name={}", rule_name),
    ]);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("netsh delete spawn failed: {}", e))?;
    // delete is best-effort; missing rule is fine
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("netsh delete rule: {}", stderr.trim()))
    }
}

/// Start-log lines for Phase 3–5 (network decision without OS apply; apply is separate).
///
/// Prefer logging `NetworkSandboxHandle::summary()` after `apply_network_sandbox`.
/// This helper remains for landlock/env lines and a pre-apply network intent line.
pub fn experimental_confinement_log_lines(runtime_network_granted: bool) -> Vec<String> {
    let mut lines = Vec::new();

    // Phase 3 — network OS (decision only; apply_network_sandbox writes final status)
    let decision = decide_network_sandbox(runtime_network_granted);
    match decision {
        NetworkSandboxDecision::NotEnforced => {
            lines.push(if sandbox_disabled() {
                "network_os: not_enforced (PICAIPIC_DISABLE_PLUGIN_SANDBOX is set)".to_string()
            } else {
                "network_os: not_enforced (set PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX=1 to enable)"
                    .to_string()
            });
        }
        NetworkSandboxDecision::AllowOutbound => {
            lines.push("network_os: allow (runtime network granted)".to_string());
        }
        NetworkSandboxDecision::BlockOutbound => {
            lines.push(
                "network_os: block_outbound_requested (OS apply runs next; loopback expected to remain)"
                    .to_string(),
            );
        }
    }

    // Phase 4 — Landlock (decision/intent; final status from apply_linux_landlock)
    if !cfg!(target_os = "linux") {
        lines.push("landlock: unsupported_os".to_string());
    } else if sandbox_disabled() {
        lines.push("landlock: not_enforced (PICAIPIC_DISABLE_PLUGIN_SANDBOX is set)".to_string());
    } else if !linux_landlock_flag_enabled() {
        lines.push(
            "landlock: not_enforced (set PICAIPIC_ENABLE_LINUX_LANDLOCK=1 to enable)".to_string(),
        );
    } else {
        lines.push("landlock: enable_requested (apply_linux_landlock runs on spawn)".to_string());
    }

    // Phase 5 — env hygiene (real allowlist when flag on; still opt-in)
    if !env_hygiene_flag_enabled() {
        lines.push(
            "env_hygiene: inherit (default; set PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1 to rebuild env)"
                .to_string(),
        );
    } else {
        lines.push(
            "env_hygiene: allowlist (env_clear + keep PICAIPIC_*/runtime/GPU discovery vars)"
                .to_string(),
        );
    }

    lines
}

/// Host/base env keys preserved when Phase 5 hygiene rebuilds the process environment.
///
/// Keep this conservative: GPU discovery and venv activation break without PATH
/// and a small set of CUDA/ROCm/Windows system variables. Secrets like AWS_*,
/// tokens, and user shell custom vars are intentionally dropped.
pub fn env_hygiene_base_allowlist() -> &'static [&'static str] {
    &[
        // Process/path basics
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "SYSTEMROOT",
        "windir",
        "WINDIR",
        "ComSpec",
        "COMSPEC",
        "TEMP",
        "TMP",
        "TMPDIR",
        "HOME",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
        "USERNAME",
        "USER",
        "LOGNAME",
        "USERDOMAIN",
        "COMPUTERNAME",
        "NUMBER_OF_PROCESSORS",
        "PROCESSOR_ARCHITECTURE",
        "PROCESSOR_IDENTIFIER",
        "OS",
        "APPDATA",
        "LOCALAPPDATA",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "CommonProgramFiles",
        "CommonProgramFiles(x86)",
        "PUBLIC",
        // Locale / terminal
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "LANGUAGE",
        "TERM",
        "COLORTERM",
        // Python / venv
        "PYTHONHOME",
        "PYTHONPATH",
        "PYTHONNOUSERSITE",
        "PYTHONUTF8",
        "PYTHONIOENCODING",
        "VIRTUAL_ENV",
        "VIRTUAL_ENV_PROMPT",
        "CONDA_PREFIX",
        "CONDA_DEFAULT_ENV",
        "CONDA_PYTHON_EXE",
        // GPU / compute discovery (must not strip wholesale)
        "CUDA_PATH",
        "CUDA_PATH_V11_8",
        "CUDA_PATH_V12_0",
        "CUDA_PATH_V12_1",
        "CUDA_PATH_V12_2",
        "CUDA_PATH_V12_3",
        "CUDA_PATH_V12_4",
        "CUDA_PATH_V12_5",
        "CUDA_PATH_V12_6",
        "CUDA_HOME",
        "CUDA_VISIBLE_DEVICES",
        "CUDA_DEVICE_ORDER",
        "NVIDIA_VISIBLE_DEVICES",
        "ROCM_PATH",
        "HIP_PATH",
        "HIP_VISIBLE_DEVICES",
        "HSA_OVERRIDE_GFX_VERSION",
        "LD_LIBRARY_PATH",
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        // Windows GPU / DirectML helpers
        "DirectXShaderCompiler",
    ]
}

fn is_env_key_allowed(key: &str) -> bool {
    if key.starts_with("PICAIPIC_") {
        return true;
    }
    // CUDA toolkit versioned installs: CUDA_PATH_V12_4, etc.
    if key.starts_with("CUDA_PATH") || key.starts_with("CUDA_") {
        return true;
    }
    if key.starts_with("HIP_") || key.starts_with("ROCR_") || key.starts_with("HSA_") {
        return true;
    }
    // Windows dynamic PATH expansions sometimes use these.
    if key.eq_ignore_ascii_case("Path") {
        return true;
    }
    env_hygiene_base_allowlist()
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(key))
}

/// When `PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1`, clear the process environment and
/// re-inject only allowlisted host vars. Host-injected `PICAIPIC_*` / setup keys
/// should be applied **after** this call (or will be kept if already set and
/// allowlisted).
///
/// Default (flag off): no-op — full host env inheritance (Phase 0–2 behavior).
pub fn apply_env_hygiene(cmd: &mut Command) -> EnvHygieneResult {
    if !env_hygiene_flag_enabled() {
        return EnvHygieneResult {
            applied: false,
            kept: 0,
            dropped_hint: 0,
        };
    }

    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let total = host_env.len();
    let mut kept_pairs: Vec<(String, String)> = Vec::new();
    for (key, value) in host_env {
        if is_env_key_allowed(&key) {
            kept_pairs.push((key, value));
        }
    }
    let kept = kept_pairs.len();
    let dropped_hint = total.saturating_sub(kept);

    cmd.env_clear();
    for (key, value) in kept_pairs {
        cmd.env(key, value);
    }

    EnvHygieneResult {
        applied: true,
        kept,
        dropped_hint,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EnvHygieneResult {
    pub applied: bool,
    pub kept: usize,
    /// Approximate count of host vars not re-injected (for start.log only).
    pub dropped_hint: usize,
}

impl EnvHygieneResult {
    pub fn summary(&self) -> String {
        if !self.applied {
            "inherit".to_string()
        } else {
            format!(
                "allowlist applied (kept {}, dropped ~{})",
                self.kept, self.dropped_hint
            )
        }
    }
}

/// Backward-compatible name used by older call sites / docs.
pub fn apply_env_hygiene_scaffold(cmd: &mut Command) {
    let _ = apply_env_hygiene(cmd);
}

/// Result of Phase 4 Landlock apply (always soft — never blocks spawn).
#[derive(Debug, Clone)]
pub struct LandlockStatus {
    /// `not_enforced` | `unsupported_os` | `applied` | `soft_fail`
    pub mode: String,
    pub detail: String,
    pub ro_paths: usize,
    pub rw_paths: usize,
}

impl LandlockStatus {
    pub fn summary(&self) -> String {
        format!(
            "landlock: {} (ro={}, rw={}; {})",
            self.mode, self.ro_paths, self.rw_paths, self.detail
        )
    }
}

/// Backward-compatible no-arg hook (no paths → cannot apply).
pub fn apply_linux_landlock_scaffold(cmd: &mut Command) {
    let _ = apply_linux_landlock(cmd, &[], &[]);
}

/// Apply Linux Landlock to `cmd` when the opt-in flag is set.
///
/// - **RW** under `writable_dirs` (plugin data/cache/outputs/runtimes/task…).
/// - **RO** under `extra_ro_dirs` plus common system prefixes needed for Python/GPU
///   (`/usr`, `/lib`, `/dev`, `/proc`, `/sys`, `/etc`, `/tmp`, … when present).
///
/// Soft-fails on non-Linux, missing ABI, or rule errors so plugins still start.
pub fn apply_linux_landlock(
    cmd: &mut Command,
    writable_dirs: &[PathBuf],
    extra_ro_dirs: &[PathBuf],
) -> LandlockStatus {
    if !cfg!(target_os = "linux") {
        return LandlockStatus {
            mode: "unsupported_os".to_string(),
            detail: "Landlock is Linux-only".to_string(),
            ro_paths: 0,
            rw_paths: 0,
        };
    }
    if sandbox_disabled() {
        return LandlockStatus {
            mode: "not_enforced".to_string(),
            detail: "PICAIPIC_DISABLE_PLUGIN_SANDBOX is set".to_string(),
            ro_paths: 0,
            rw_paths: 0,
        };
    }
    if !linux_landlock_flag_enabled() {
        return LandlockStatus {
            mode: "not_enforced".to_string(),
            detail: "set PICAIPIC_ENABLE_LINUX_LANDLOCK=1 to enable".to_string(),
            ro_paths: 0,
            rw_paths: 0,
        };
    }

    #[cfg(target_os = "linux")]
    {
        return landlock_linux::apply(cmd, writable_dirs, extra_ro_dirs);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (cmd, writable_dirs, extra_ro_dirs);
        LandlockStatus {
            mode: "unsupported_os".to_string(),
            detail: "unreachable".to_string(),
            ro_paths: 0,
            rw_paths: 0,
        }
    }
}

#[cfg(target_os = "linux")]
mod landlock_linux {
    use super::*;
    use std::fs::OpenOptions;
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
    use std::os::unix::process::CommandExt as _;

    // landlock ABI 1 access rights (subset used here).
    const ACCESS_FS_EXECUTE: u64 = 1 << 0;
    const ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
    const ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
    const ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
    const ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
    const ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
    const ACCESS_FS_MAKE_REG: u64 = 1 << 8;
    const ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
    const ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
    const ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
    const ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
    // ABI 2+
    const ACCESS_FS_REFER: u64 = 1 << 13;
    // ABI 3+
    const ACCESS_FS_TRUNCATE: u64 = 1 << 14;

    const ACCESS_RO: u64 = ACCESS_FS_EXECUTE | ACCESS_FS_READ_FILE | ACCESS_FS_READ_DIR;
    const ACCESS_RW: u64 = ACCESS_RO
        | ACCESS_FS_WRITE_FILE
        | ACCESS_FS_REMOVE_DIR
        | ACCESS_FS_REMOVE_FILE
        | ACCESS_FS_MAKE_CHAR
        | ACCESS_FS_MAKE_DIR
        | ACCESS_FS_MAKE_REG
        | ACCESS_FS_MAKE_SOCK
        | ACCESS_FS_MAKE_FIFO
        | ACCESS_FS_MAKE_BLOCK
        | ACCESS_FS_MAKE_SYM
        | ACCESS_FS_REFER
        | ACCESS_FS_TRUNCATE;

    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1 << 0;
    const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
    }

    #[repr(C)]
    struct PathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    fn nr_create_ruleset() -> i64 {
        // x86_64 / aarch64 modern Linux
        444
    }
    fn nr_add_rule() -> i64 {
        445
    }
    fn nr_restrict_self() -> i64 {
        446
    }

    fn create_ruleset(handled: u64) -> Result<OwnedFd, String> {
        let attr = RulesetAttr {
            handled_access_fs: handled,
            handled_access_net: 0,
        };
        let fd = unsafe {
            libc::syscall(
                nr_create_ruleset(),
                &attr as *const RulesetAttr,
                std::mem::size_of::<RulesetAttr>(),
                0u32,
            )
        };
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("landlock_create_ruleset: {}", err));
        }
        Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
    }

    fn probe_abi() -> Result<u64, String> {
        // Query highest supported ABI by passing LANDLOCK_CREATE_RULESET_VERSION.
        let abi = unsafe {
            libc::syscall(
                nr_create_ruleset(),
                std::ptr::null::<RulesetAttr>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if abi < 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("landlock ABI probe failed: {}", err));
        }
        Ok(abi as u64)
    }

    fn handled_access_for_abi(abi: u64) -> u64 {
        let mut handled = ACCESS_RO
            | ACCESS_FS_WRITE_FILE
            | ACCESS_FS_REMOVE_DIR
            | ACCESS_FS_REMOVE_FILE
            | ACCESS_FS_MAKE_CHAR
            | ACCESS_FS_MAKE_DIR
            | ACCESS_FS_MAKE_REG
            | ACCESS_FS_MAKE_SOCK
            | ACCESS_FS_MAKE_FIFO
            | ACCESS_FS_MAKE_BLOCK
            | ACCESS_FS_MAKE_SYM;
        if abi >= 2 {
            handled |= ACCESS_FS_REFER;
        }
        if abi >= 3 {
            handled |= ACCESS_FS_TRUNCATE;
        }
        handled
    }

    fn add_path_rule(ruleset: &OwnedFd, path: &Path, access: u64) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|e| format!("open {}: {}", path.display(), e))?;
        let attr = PathBeneathAttr {
            allowed_access: access,
            parent_fd: file.as_raw_fd(),
        };
        let rc = unsafe {
            libc::syscall(
                nr_add_rule(),
                ruleset.as_raw_fd(),
                LANDLOCK_RULE_PATH_BENEATH,
                &attr as *const PathBeneathAttr,
                0u32,
            )
        };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("landlock_add_rule {}: {}", path.display(), err));
        }
        Ok(())
    }

    fn default_system_ro_roots() -> Vec<PathBuf> {
        [
            "/usr",
            "/lib",
            "/lib64",
            "/lib32",
            "/bin",
            "/sbin",
            "/opt",
            "/etc",
            "/dev",
            "/proc",
            "/sys",
            "/tmp",
            "/var/tmp",
            "/run",
            // Common GPU / ROCm / CUDA install prefixes
            "/opt/rocm",
            "/opt/cuda",
            "/usr/local/cuda",
        ]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect()
    }

    fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for p in paths {
            let canon = p.canonicalize().unwrap_or(p);
            if !out.iter().any(|e: &PathBuf| e == &canon) {
                out.push(canon);
            }
        }
        out
    }

    pub fn apply(
        cmd: &mut Command,
        writable_dirs: &[PathBuf],
        extra_ro_dirs: &[PathBuf],
    ) -> LandlockStatus {
        let abi = match probe_abi() {
            Ok(v) => v,
            Err(e) => {
                return LandlockStatus {
                    mode: "soft_fail".to_string(),
                    detail: e,
                    ro_paths: 0,
                    rw_paths: 0,
                };
            }
        };

        let handled = handled_access_for_abi(abi);
        let ruleset = match create_ruleset(handled) {
            Ok(fd) => fd,
            Err(e) => {
                return LandlockStatus {
                    mode: "soft_fail".to_string(),
                    detail: e,
                    ro_paths: 0,
                    rw_paths: 0,
                };
            }
        };

        let mut ro = default_system_ro_roots();
        ro.extend(extra_ro_dirs.iter().cloned());
        // Writable roots also need to be readable; RW rule covers both.
        let rw = dedupe_paths(writable_dirs.to_vec());
        let ro = dedupe_paths(ro);

        let mut ro_ok = 0usize;
        let mut rw_ok = 0usize;
        let mut errors: Vec<String> = Vec::new();

        for path in &rw {
            let access = ACCESS_RW & handled;
            match add_path_rule(&ruleset, path, access) {
                Ok(()) if path.exists() => rw_ok += 1,
                Ok(()) => {}
                Err(e) => errors.push(e),
            }
        }
        for path in &ro {
            // Skip if already covered as RW (same path).
            if rw.iter().any(|w| w == path) {
                continue;
            }
            let access = ACCESS_RO & handled;
            match add_path_rule(&ruleset, path, access) {
                Ok(()) if path.exists() => ro_ok += 1,
                Ok(()) => {}
                Err(e) => errors.push(e),
            }
        }

        if rw_ok == 0 && ro_ok == 0 {
            return LandlockStatus {
                mode: "soft_fail".to_string(),
                detail: format!("no path rules added (abi={}); {}", abi, errors.join("; ")),
                ro_paths: 0,
                rw_paths: 0,
            };
        }

        // Move ruleset fd into pre_exec: restrict_self then close.
        let ruleset_fd = ruleset.into_raw_fd();
        unsafe {
            cmd.pre_exec(move || {
                // Required before landlock_restrict_self.
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let rc = libc::syscall(nr_restrict_self(), ruleset_fd, 0u32);
                let _ = libc::close(ruleset_fd);
                if rc < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let detail = if errors.is_empty() {
            format!("abi={}, ruleset applied via pre_exec", abi)
        } else {
            format!(
                "abi={}, applied with {} rule warning(s): {}",
                abi,
                errors.len(),
                errors.into_iter().take(3).collect::<Vec<_>>().join("; ")
            )
        };

        LandlockStatus {
            mode: "applied".to_string(),
            detail,
            ro_paths: ro_ok,
            rw_paths: rw_ok,
        }
    }
}

#[cfg(test)]
mod env_hygiene_tests {
    use super::*;

    #[test]
    fn allowlist_keeps_picaipic_and_path() {
        assert!(is_env_key_allowed("PICAIPIC_PLUGIN_ID"));
        assert!(is_env_key_allowed("PATH"));
        assert!(is_env_key_allowed("Path"));
        assert!(is_env_key_allowed("CUDA_PATH"));
        assert!(is_env_key_allowed("CUDA_PATH_V12_4"));
        assert!(is_env_key_allowed("VIRTUAL_ENV"));
        assert!(is_env_key_allowed("ROCM_PATH"));
    }

    #[test]
    fn allowlist_drops_common_secrets() {
        assert!(!is_env_key_allowed("AWS_SECRET_ACCESS_KEY"));
        assert!(!is_env_key_allowed("AWS_ACCESS_KEY_ID"));
        assert!(!is_env_key_allowed("OPENAI_API_KEY"));
        assert!(!is_env_key_allowed("GITHUB_TOKEN"));
        assert!(!is_env_key_allowed("SSH_AUTH_SOCK"));
        assert!(!is_env_key_allowed("MY_CUSTOM_TOKEN"));
    }
}

#[cfg(test)]
mod network_sandbox_tests {
    use super::*;

    #[test]
    fn decide_not_enforced_when_flag_off() {
        // Cannot force env in a shared process safely for all tests; assert pure mapping
        // when flag appears off (default in CI unless set).
        if !network_sandbox_flag_enabled() && !sandbox_disabled() {
            assert_eq!(
                decide_network_sandbox(false),
                NetworkSandboxDecision::NotEnforced
            );
            assert_eq!(
                decide_network_sandbox(true),
                NetworkSandboxDecision::NotEnforced
            );
        }
    }

    #[test]
    fn sanitize_rule_id_strips_specials() {
        assert_eq!(sanitize_rule_id("picai/salut color!"), "picai_salut_color_");
        assert!(sanitize_rule_id(&"x".repeat(100)).len() <= 64);
    }

    #[test]
    fn policy_env_values() {
        let allow = NetworkSandboxStatus {
            decision: NetworkSandboxDecision::AllowOutbound,
            mode: "allow".into(),
            detail: "ok".into(),
        };
        let deny = NetworkSandboxStatus {
            decision: NetworkSandboxDecision::BlockOutbound,
            mode: "blocked".into(),
            detail: "ok".into(),
        };
        let off = NetworkSandboxStatus {
            decision: NetworkSandboxDecision::NotEnforced,
            mode: "not_enforced".into(),
            detail: "ok".into(),
        };
        assert_eq!(allow.policy_env_value(), "allow");
        assert_eq!(deny.policy_env_value(), "deny");
        assert_eq!(off.policy_env_value(), "unrestricted");
    }
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
