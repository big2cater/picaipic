use crate::t_config;
use crate::t_sandbox;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uuid::Uuid;
use zip::ZipArchive;

const MANIFEST_FILE_NAME: &str = "picaipic.plugin.json";
const PLUGIN_REGISTRY_FILE_NAME: &str = "plugin-registry.json";
const PLUGIN_API_MAJOR: i64 = 1;
const PLUGIN_STARTUP_TIMEOUT_MS: u64 = 12_000;
const PLUGIN_INVOKE_TIMEOUT_MS: u64 = 120_000;
const PLUGIN_DIAGNOSTICS_TIMEOUT_MS: u64 = 4_000;
const PLUGIN_SMOKE_TEST_TIMEOUT_MS: u64 = 120_000;
const PLUGIN_TASK_POLL_INTERVAL_MS: u64 = 1_000;
const PLUGIN_TASK_POLL_TIMEOUT_MS: u64 = 120_000;
const PLUGIN_TASK_CANCEL_TIMEOUT_MS: u64 = 3_000;
const PLUGIN_PROCESS_KILL_TIMEOUT_MS: u64 = 3_000;
const PYTHON_RUNTIME_PROBE_TIMEOUT_MS: u64 = 15_000;
const PLUGIN_LOG_TAIL_BYTES: u64 = 64 * 1024;
const PYTHON_RUNTIME_DISCOVERY_LIMIT: usize = 80;
const EXTERNAL_RUNTIME_PROBE_TTL_SECS: i64 = 24 * 60 * 60;
const SHARED_RUNTIME_PROBE_TTL_SECS: i64 = 24 * 60 * 60;
const PLUGIN_RUNTIME_PROBE_TTL_SECS: i64 = 7 * 24 * 60 * 60;
const PLUGIN_TASK_CACHE_TTL_SECS: u64 = 24 * 60 * 60;
const PLUGIN_TASK_SUCCESS_TTL_SECS: u64 = 24 * 60 * 60;
const PLUGIN_TASK_TMP_TTL_SECS: u64 = 15 * 60;
const PLUGIN_PACKAGE_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const PLUGIN_PACKAGE_MAX_UNPACKED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const PLUGIN_DIRECTORY_REMOVE_RETRIES: usize = 3;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static PLUGIN_PACKAGE_MUTATION_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct PluginPackageMutationGuard;

impl PluginPackageMutationGuard {
    fn acquire() -> Result<Self, String> {
        PLUGIN_PACKAGE_MUTATION_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                "Another plugin install or uninstall is already in progress".to_string()
            })?;
        Ok(Self)
    }
}

impl Drop for PluginPackageMutationGuard {
    fn drop(&mut self) {
        PLUGIN_PACKAGE_MUTATION_ACTIVE.store(false, Ordering::Release);
    }
}

enum SetupCommandOutcome {
    Completed,
    Cancelled,
}

#[derive(Default)]
pub struct AiPluginRuntimeState {
    processes: Mutex<HashMap<String, RunningPlugin>>,
}

struct RunningPlugin {
    child: Child,
    port: u16,
    base_url: String,
    start_signature: Option<String>,
    auth_token: String,
    /// Sandbox handle whose Drop revokes the deny-ACEs applied before spawn.
    /// `None` on non-Windows or when the sandbox is disabled. The field is
    /// never read — its sole purpose is to tie ACL revocation to the
    /// RunningPlugin's lifetime via Drop.
    #[allow(dead_code)]
    sandbox: Option<t_sandbox::SandboxHandle>,
    /// Phase 3 network handle: Drop removes Windows firewall rule if applied.
    #[allow(dead_code)]
    network_sandbox: Option<t_sandbox::NetworkSandboxHandle>,
}

impl AiPluginRuntimeState {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
        }
    }

    pub async fn stop_all(&self) {
        let mut plugin_ids: HashSet<String> = {
            let processes = self.processes.lock().await;
            processes.keys().cloned().collect()
        };

        if let Ok(manifest_paths) = discover_manifest_paths() {
            for manifest_path in manifest_paths {
                if let Ok(manifest) = read_manifest(&manifest_path) {
                    if !manifest.id.trim().is_empty() {
                        plugin_ids.insert(manifest.id);
                    }
                }
            }
        }

        for plugin_id in plugin_ids {
            let _ = stop_ai_plugin_runtime(plugin_id, self).await;
        }
    }
}

/// Tracks setup job cancellation requests so a running setup command can be
/// killed cooperatively.
#[derive(Default)]
pub struct SetupCancellationState {
    cancellations: Mutex<HashMap<String, bool>>,
}

impl SetupCancellationState {
    pub fn new() -> Self {
        Self {
            cancellations: Mutex::new(HashMap::new()),
        }
    }

    pub async fn request_cancel(&self, job_id: &str) {
        self.cancellations
            .lock()
            .await
            .insert(job_id.to_string(), true);
    }

    pub async fn is_cancelled(&self, job_id: &str) -> bool {
        self.cancellations
            .lock()
            .await
            .get(job_id)
            .copied()
            .unwrap_or(false)
    }

    pub async fn clear(&self, job_id: &str) {
        self.cancellations.lock().await.remove(job_id);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginRegistry {
    #[serde(default)]
    pub registered_paths: Vec<String>,
    #[serde(default)]
    pub permission_grants: HashMap<String, AiPluginPermissionGrant>,
    #[serde(default)]
    pub profile_states: HashMap<String, AiPluginProfileState>,
    #[serde(default)]
    pub setup_jobs: HashMap<String, AiPluginSetupJob>,
    #[serde(default)]
    pub runtime_probe_states: HashMap<String, AiPluginRuntimeProbeState>,
    #[serde(default)]
    pub task_states: HashMap<String, AiPluginTaskState>,
    #[serde(default)]
    pub trusted_publishers: HashMap<String, AiPluginTrustedPublisher>,
    /// Local denylist of Ed25519 public keys (base64). Install fails closed if package key is listed.
    #[serde(default)]
    pub revoked_keys: Vec<AiPluginRevokedKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginProfileState {
    pub plugin_id: String,
    pub profile_id: String,
    pub backend: String,
    pub capability: String,
    pub status: String,
    pub verified: bool,
    pub updated_at: String,
    #[serde(default)]
    pub setup_attempted: bool,
    #[serde(default)]
    pub setup_job_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
    /// Per-profile persisted external model directory bindings.
    /// Key = `PluginModelBinding.id`, value = user-selected directory absolute path.
    #[serde(default)]
    pub model_dir_bindings: HashMap<String, String>,
}

/// Cap in-memory setup job log lines (verbose plugins otherwise grow without bound).
const SETUP_JOB_LOG_MAX_LINES: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginSetupJob {
    pub id: String,
    pub plugin_id: String,
    pub profile_id: String,
    pub backend: String,
    pub capability: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub progress: u8,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub log: Vec<String>,
}

impl AiPluginSetupJob {
    /// Append a log line, keeping only the newest `SETUP_JOB_LOG_MAX_LINES`.
    fn push_log(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > SETUP_JOB_LOG_MAX_LINES {
            let drop_n = self.log.len() - SETUP_JOB_LOG_MAX_LINES;
            self.log.drain(0..drop_n);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginRuntimeProbeState {
    pub plugin_id: String,
    pub profile_id: String,
    pub backend: String,
    pub capability: String,
    pub status: String,
    pub available: bool,
    pub probed_at: String,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub stale_reason: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
    #[serde(default)]
    pub fingerprint: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginTaskState {
    pub plugin_id: String,
    pub capability_id: String,
    pub task_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub task_dir: String,
    pub output_dir: String,
    #[serde(default)]
    pub result_policy: Option<String>,
    #[serde(default)]
    pub adopted: bool,
    #[serde(default)]
    pub outputs: Vec<Value>,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_domain: Option<String>,
    #[serde(default)]
    pub error_details: Option<Value>,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default)]
    pub request_snapshot: Option<AiPluginInvokeRequestSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPermissionGrant {
    pub plugin_id: String,
    #[serde(default)]
    pub runtime_network: bool,
    #[serde(default)]
    pub setup_downloads: bool,
    #[serde(default)]
    pub upload_selected_files: bool,
    #[serde(default)]
    pub upload_outputs: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginSetupPreview {
    pub plugin_id: String,
    pub profile_id: String,
    pub backend: String,
    pub capability: String,
    pub command: String,
    pub command_path: String,
    pub working_dir: String,
    pub env_dir: Option<String>,
    pub env_path: Option<String>,
    pub requirements: Option<String>,
    pub requirements_path: Option<String>,
    pub runtime_binding: Option<PluginRuntimeBinding>,
    pub environment: HashMap<String, String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginManifest {
    #[serde(default)]
    pub schema_version: i64,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub runtimes: Vec<String>,
    #[serde(default)]
    pub runtime: Option<PluginRuntime>,
    #[serde(default)]
    pub compatibility: Option<PluginCompatibility>,
    #[serde(default)]
    pub entry: Option<PluginEntry>,
    #[serde(default)]
    pub install: Option<PluginInstall>,
    #[serde(default)]
    pub install_profiles: Vec<PluginInstallProfile>,
    #[serde(default)]
    pub smoke_test: Option<PluginSmokeTest>,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub contributes: Option<PluginContributes>,
    #[serde(default)]
    pub permissions: Option<Value>,
    #[serde(default)]
    pub hardware: Option<Value>,
    #[serde(default)]
    pub models: Vec<Value>,
    #[serde(default)]
    pub model_bindings: Vec<PluginModelBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginNetworkPermissionsSummary {
    #[serde(default)]
    pub runtime: bool,
    #[serde(default)]
    pub setup_downloads: bool,
    #[serde(default)]
    pub upload_selected_files: bool,
    #[serde(default)]
    pub upload_outputs: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPermissionsSummary {
    #[serde(default)]
    pub read_selected_files: bool,
    #[serde(default)]
    pub write_output_dir: bool,
    #[serde(default)]
    pub write_source_files: bool,
    #[serde(default)]
    pub launch_child_processes: bool,
    pub network: AiPluginNetworkPermissionsSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCompatibility {
    #[serde(default)]
    pub min_pic_ai_pic_version: Option<String>,
    #[serde(default)]
    pub max_pic_ai_pic_version: Option<String>,
    #[serde(default)]
    pub plugin_api: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub start_command: Option<String>,
    #[serde(default)]
    pub stop_command: Option<String>,
    #[serde(default)]
    pub default_port: Option<u16>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub health: Option<PluginHttpEndpoint>,
    #[serde(default)]
    pub status: Option<PluginHttpEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstall {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub estimated_disk_mb: Option<u64>,
    #[serde(default)]
    pub requires_admin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntime {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub cuda_api_compatible: Option<bool>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeBinding {
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub python: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub requirements: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub support_level: Option<String>,
    #[serde(default)]
    pub derived_from: Option<String>,
    #[serde(default)]
    pub env_dir: Option<String>,
    #[serde(default)]
    pub requirements: Option<String>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
    #[serde(default)]
    pub runtime_bindings: Vec<PluginRuntimeBinding>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// A user-configurable external model directory binding declared in the manifest.
///
/// The host persists a per-profile selected directory (see
/// `AiPluginProfileState.model_dir_bindings`) and injects it as the declared
/// `env_var` (and any extra `env_vars`) into the plugin process environment,
/// replacing the need to hand-edit `.local.env`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginModelBinding {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    /// Primary env var whose value is the bound directory absolute path.
    #[serde(default)]
    pub env_var: String,
    /// Optional additional env vars that should receive the same directory path.
    #[serde(default)]
    pub env_vars: Vec<String>,
    /// `"files"` (directory directly holds model files) or `"sourceTree"`
    /// (directory is a source checkout with models under subpaths).
    #[serde(default)]
    pub layout: String,
    /// Relative file paths expected inside the bound directory.
    #[serde(default)]
    pub expected_files: Vec<String>,
    /// Optional glob patterns (relative to the bound dir) used for validation.
    #[serde(default)]
    pub expected_globs: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSmokeTest {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginHttpEndpoint {
    #[serde(default = "default_get_method")]
    pub method: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub ready_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapability {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub inputs: Vec<Value>,
    #[serde(default)]
    pub outputs: Vec<Value>,
    #[serde(default)]
    pub invoke: Option<Value>,
    #[serde(default)]
    pub parameters: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginContributes {
    #[serde(default)]
    pub menus: Vec<PluginMenuContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMenuContribution {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub capability: String,
    #[serde(default)]
    pub contexts: Vec<String>,
    #[serde(default)]
    pub placements: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub order: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginValidationReport {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: Option<String>,
    pub path: String,
    pub manifest_path: String,
    pub platform_supported: bool,
    pub validation: PluginValidationReport,
    pub permissions: AiPluginPermissionsSummary,
    pub permission_grant: Option<AiPluginPermissionGrant>,
    pub runtimes: Vec<String>,
    pub runtime: Option<PluginRuntimeSummary>,
    pub entry: Option<PluginEntrySummary>,
    pub install: Option<PluginInstallSummary>,
    pub storage: AiPluginStorageSummary,
    pub install_profiles: Vec<PluginInstallProfileSummary>,
    pub smoke_test: Option<PluginSmokeTestSummary>,
    pub capabilities: Vec<PluginCapabilitySummary>,
    pub contributes: PluginContributesSummary,
    pub task_states: Vec<AiPluginTaskState>,
    #[serde(default)]
    pub model_bindings: Vec<PluginModelBinding>,
    /// Declared model files under the managed plugin model directory, with
    /// current on-disk presence. Used by Settings for open/validate/import UX.
    #[serde(default)]
    pub model_files: Vec<AiPluginModelFileSummary>,
}

/// One file copied into the managed plugin model directory by the import helper.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginModelImportItem {
    pub model_id: String,
    pub model_name: String,
    pub source_path: String,
    pub target_path: String,
}

/// Result of importing user-selected model files into `plugin-data/<id>/models`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginModelImportResult {
    pub plugin_id: String,
    pub model_dir: String,
    pub imported: Vec<AiPluginModelImportItem>,
    pub unmatched: Vec<String>,
    pub model_files: Vec<AiPluginModelFileSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginStorageSummary {
    pub store_dir: String,
    pub code_dir: String,
    pub data_dir: String,
    pub model_dir: String,
    pub model_dirs: Vec<String>,
    pub log_dir: String,
    pub config_path: String,
    pub runtime_dir: String,
    pub runtime_dirs: Vec<String>,
    pub cache_dir: String,
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginStoreInfo {
    pub path: String,
    pub configured_path: Option<String>,
    pub default_path: String,
    pub env_override: Option<String>,
    pub using_custom: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeSummary {
    pub kind: String,
    pub cuda_api_compatible: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginEntrySummary {
    pub kind: String,
    pub base_url: Option<String>,
    pub start_command: Option<String>,
    pub status_path: Option<String>,
    pub health_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallSummary {
    pub kind: String,
    pub command: Option<String>,
    pub estimated_disk_mb: Option<u64>,
    pub requires_admin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallProfileSummary {
    pub id: String,
    pub backend: String,
    pub label: Option<String>,
    pub support_level: String,
    pub derived_from: Option<String>,
    pub env_dir: Option<String>,
    pub requirements: Option<String>,
    pub runtime_binding: Option<PluginRuntimeBinding>,
    pub runtime_bindings: Vec<PluginRuntimeBinding>,
    pub notes: Option<String>,
    #[serde(default)]
    pub resolved_runtime_dir: Option<String>,
    pub state: Option<AiPluginProfileState>,
    pub setup_job: Option<AiPluginSetupJob>,
    pub runtime_probe_state: Option<AiPluginRuntimeProbeState>,
    #[serde(default)]
    pub runtime_probe_states: Vec<AiPluginRuntimeProbeState>,
    #[serde(default)]
    pub runtime_conflicts: Vec<RuntimeConflict>,
    #[serde(default)]
    pub model_binding_checks: Vec<AiPluginModelBindingSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSmokeTestSummary {
    pub command: Option<String>,
    pub capability: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCapabilitySummary {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub version: Option<String>,
    pub inputs: Vec<Value>,
    pub outputs: Vec<Value>,
    pub parameters: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginContributesSummary {
    pub menus: Vec<PluginMenuContributionSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMenuContributionSummary {
    pub id: String,
    pub label: String,
    pub capability: String,
    pub contexts: Vec<String>,
    pub placements: Vec<String>,
    pub icon: Option<String>,
    pub order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifestValidationResult {
    pub manifest_path: String,
    pub manifest: Option<AiPluginManifest>,
    pub validation: PluginValidationReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPackageManifest {
    #[serde(default)]
    pub schema_version: i64,
    #[serde(default)]
    pub package_kind: String,
    #[serde(default)]
    pub plugin_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default)]
    pub files: Vec<AiPluginPackageFile>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<AiPluginPackageSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPackageSignature {
    #[serde(default)]
    pub algorithm: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub value: String,
}

/// One trusted Ed25519 public key for a publisher (rotation-friendly).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginTrustedKey {
    pub public_key: String,
    pub trusted_at: String,
    /// `active` | `retired` (retired keys no longer accept new installs).
    #[serde(default = "default_trusted_key_status")]
    pub status: String,
}

fn default_trusted_key_status() -> String {
    "active".to_string()
}

/// Locally revoked package-signing public key.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginRevokedKey {
    pub public_key: String,
    pub revoked_at: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginTrustedPublisher {
    pub publisher: String,
    /// Primary/display key (legacy single-key field; kept for older UI/API clients).
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub trusted_at: String,
    /// All trusted keys for this publisher (multi-key rotation).
    #[serde(default)]
    pub keys: Vec<AiPluginTrustedKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPackageFile {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPackageInstallResult {
    pub plugin_id: String,
    pub version: String,
    pub installed_path: String,
    pub registered_paths: Vec<String>,
    pub package_warnings: Vec<String>,
    pub validation: PluginValidationReport,
    pub storage: AiPluginStorageSummary,
    pub model_files: Vec<AiPluginModelFileSummary>,
    #[serde(default)]
    pub signature_verified: bool,
    #[serde(default)]
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginModelFileSummary {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub path: String,
    pub exists: bool,
    pub purpose: Option<String>,
}

/// Validation result for a single model directory binding check.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginModelBindingCheck {
    pub binding_id: String,
    pub dir: String,
    pub present_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub ok: bool,
}

/// Summary of a manifest-declared model directory binding, returned by
/// `list_ai_plugins` so the frontend can render binding status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginModelBindingSummary {
    pub id: String,
    pub label: Option<String>,
    pub env_var: String,
    pub env_vars: Vec<String>,
    pub layout: String,
    pub expected_files: Vec<String>,
    pub expected_globs: Vec<String>,
    pub description: Option<String>,
    /// User-selected directory absolute path, if a binding is persisted.
    pub dir: Option<String>,
    pub present_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginUninstallResult {
    pub plugin_id: String,
    pub removed_path: String,
    pub registered_paths: Vec<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub removed_extra_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginStatus {
    pub plugin_id: String,
    pub reachable: bool,
    pub managed: bool,
    pub url: Option<String>,
    pub status: Option<Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_domain: Option<String>,
    #[serde(default)]
    pub error_details: Option<Value>,
    #[serde(default)]
    pub log_tail: Option<AiPluginLogFile>,
    #[serde(default)]
    pub advice: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginDiagnostics {
    pub plugin_id: String,
    pub reachable: bool,
    pub url: Option<String>,
    pub diagnostics: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginSmokeTestRequest {
    pub profile_id: String,
    pub backend: String,
    pub capability: String,
    #[serde(default)]
    pub runtime_binding_id: Option<String>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginProfileSetupRequest {
    pub profile_id: String,
    pub backend: String,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub runtime_binding_id: Option<String>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
    #[serde(default)]
    pub allow_command_execution: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginStartRequest {
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub runtime_binding_id: Option<String>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginSmokeTestResult {
    pub plugin_id: String,
    pub profile_id: String,
    pub backend: String,
    pub capability: String,
    pub reachable: bool,
    pub url: Option<String>,
    pub passed: bool,
    pub duration_ms: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub startup_status: Option<AiPluginStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginLogFile {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginLogs {
    pub plugin_id: String,
    pub files: Vec<AiPluginLogFile>,
    pub error: Option<String>,
}

fn ai_plugin_status(
    plugin_id: String,
    reachable: bool,
    managed: bool,
    url: Option<String>,
    status: Option<Value>,
    error: Option<String>,
) -> AiPluginStatus {
    AiPluginStatus {
        plugin_id,
        reachable,
        managed,
        url,
        status,
        error,
        error_code: None,
        error_domain: None,
        error_details: None,
        log_tail: None,
        advice: Vec::new(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginHostEnvironment {
    pub os: String,
    pub arch: String,
    pub platform: String,
    pub gpus: Vec<AiPluginHostGpu>,
    pub candidate_backends: Vec<String>,
    pub python_runtimes: Vec<AiPluginPythonRuntime>,
    pub probe_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginHostGpu {
    pub name: String,
    pub vendor: String,
    pub backend_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPythonRuntime {
    pub id: String,
    pub label: String,
    pub scope: String,
    pub python: String,
    pub root: Option<String>,
    pub source: String,
    pub version: Option<String>,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPythonRuntimeProbeRequest {
    pub python: String,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub plugin_id: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub capability: Option<String>,
    #[serde(default)]
    pub runtime_binding: Option<PluginRuntimeBinding>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPythonRuntimeProbeResult {
    pub python: String,
    pub backend: Option<String>,
    pub available: bool,
    pub duration_ms: u128,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub state: Option<AiPluginRuntimeProbeState>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginInvokeRequest {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub inputs: Value,
    #[serde(default)]
    pub parameters: Value,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub runtime: Option<Value>,
    #[serde(default)]
    pub result_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginInvokeRequestSnapshot {
    pub inputs: Value,
    pub parameters: Value,
    pub runtime: Option<Value>,
    pub result_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginInvokeResponse {
    pub plugin_id: String,
    pub capability_id: String,
    pub task_id: String,
    pub url: String,
    pub result: Value,
    pub task_state: Option<AiPluginTaskState>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginTaskAdoptRequest {
    pub task_id: String,
    #[serde(default)]
    pub delete_task_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginTaskStatusResponse {
    pub plugin_id: String,
    pub task_id: String,
    pub state: AiPluginTaskState,
    #[serde(default)]
    pub plugin_status: Option<Value>,
    #[serde(default)]
    pub plugin_status_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPluginPermissionGrantRequest {
    #[serde(default)]
    pub runtime_network: bool,
    #[serde(default)]
    pub setup_downloads: bool,
    #[serde(default)]
    pub upload_selected_files: bool,
    #[serde(default)]
    pub upload_outputs: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

fn default_get_method() -> String {
    "GET".to_string()
}

fn registry_path() -> Result<PathBuf, String> {
    let app_dir = t_config::get_app_data_dir()?;
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    Ok(app_dir.join(PLUGIN_REGISTRY_FILE_NAME))
}

fn plugin_home_dir() -> Result<PathBuf, String> {
    let dir = plugin_store_dir()?.join("plugins");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create plugins directory: {}", e))?;
    Ok(dir)
}

fn default_plugin_store_dir() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    {
        let base_dir = std::env::current_dir()
            .map_err(|e| format!("Failed to resolve current directory: {}", e))?;
        return Ok(base_dir.join("picaipic-local"));
    }

    #[cfg(not(debug_assertions))]
    {
        let app_data_store = t_config::get_app_data_dir()?.join("picaipic-local");
        let legacy_store = std::env::current_exe()
            .map_err(|e| format!("Failed to resolve executable path: {}", e))?
            .parent()
            .map(|parent| parent.join("picaipic-local"))
            .ok_or_else(|| "Executable path has no parent directory".to_string())?;

        Ok(default_plugin_store_dir_for_paths(
            &app_data_store,
            &legacy_store,
        ))
    }
}

#[cfg(any(not(debug_assertions), test))]
fn default_plugin_store_dir_for_paths(app_data_store: &Path, legacy_store: &Path) -> PathBuf {
    // Preserve existing installs without silently hiding large plugin models/runtimes.
    if legacy_store.is_dir() && !app_data_store.exists() {
        legacy_store.to_path_buf()
    } else {
        app_data_store.to_path_buf()
    }
}

fn resolve_plugin_store_dir() -> Result<PathBuf, String> {
    if let Some(configured) = std::env::var_os("PICAIPIC_PLUGIN_STORE_DIR") {
        return Ok(PathBuf::from(configured));
    }

    if let Some(configured) = t_config::get_plugin_store_dir_config()? {
        let trimmed = configured.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    default_plugin_store_dir()
}

fn ensure_plugin_store_dir(dir: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "Failed to create plugin store directory '{}': {}. Move PicAiPic to a writable directory or set PICAIPIC_PLUGIN_STORE_DIR.",
            dir.display(),
            e
        )
    })?;
    Ok(dir)
}

fn plugin_store_dir() -> Result<PathBuf, String> {
    ensure_plugin_store_dir(resolve_plugin_store_dir()?)
}

fn plugin_store_info() -> Result<AiPluginStoreInfo, String> {
    let default_dir = default_plugin_store_dir()?;
    let configured_path = t_config::get_plugin_store_dir_config()?.and_then(|path| {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let env_override = std::env::var_os("PICAIPIC_PLUGIN_STORE_DIR")
        .map(PathBuf::from)
        .map(|path| normalize_path(&path));
    let effective_dir = ensure_plugin_store_dir(resolve_plugin_store_dir()?)?;
    let effective = normalize_path(&effective_dir);
    let default_path = normalize_path(&default_dir);
    let using_custom = env_override.is_some()
        || configured_path
            .as_ref()
            .map(|path| normalize_path(Path::new(path)) != default_path)
            .unwrap_or(false);

    Ok(AiPluginStoreInfo {
        path: effective,
        configured_path,
        default_path,
        env_override,
        using_custom,
    })
}

#[tauri::command]
pub fn get_ai_plugin_store_info() -> Result<AiPluginStoreInfo, String> {
    plugin_store_info()
}

#[tauri::command]
pub fn set_ai_plugin_store_dir(path: &str) -> Result<AiPluginStoreInfo, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Plugin store directory cannot be empty.".to_string());
    }
    let dir = ensure_plugin_store_dir(PathBuf::from(trimmed))?;
    t_config::set_plugin_store_dir_config(Some(normalize_path(&dir)))?;
    plugin_store_info()
}

#[tauri::command]
pub fn reset_ai_plugin_store_dir() -> Result<AiPluginStoreInfo, String> {
    t_config::set_plugin_store_dir_config(None)?;
    plugin_store_info()
}

/// Lexically collapse `.` / `..` without touching the filesystem.
/// Used when `canonicalize` is unavailable (path does not exist yet).
fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            std::path::Component::RootDir => out.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                // Do not climb above a root/prefix-only path.
                if matches!(
                    out.components().next_back(),
                    Some(std::path::Component::Normal(_))
                ) {
                    out.pop();
                }
            }
            std::path::Component::Normal(seg) => out.push(seg),
        }
    }
    out
}

/// True when `path` is inside `root` (inclusive of equality).
/// Prefer real `canonicalize` when both paths exist so symlinks resolve.
/// When either path is missing, fall back to lexical `..` collapse — never
/// treat raw `root/../evil` as inside `root` via unnormalized `starts_with`.
fn is_path_inside(path: &Path, root: &Path) -> bool {
    if let (Ok(path_c), Ok(root_c)) = (path.canonicalize(), root.canonicalize()) {
        return path_c.starts_with(&root_c);
    }
    let path_n = normalize_path_lexically(path);
    let root_n = normalize_path_lexically(root);
    if root_n.as_os_str().is_empty() {
        return false;
    }
    path_n.starts_with(&root_n)
}

/// True when `path` is inside any of the allow-listed roots (after canonicalize).
fn path_is_inside_any(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| is_path_inside(path, root))
}

/// Collect shared-runtime directories declared by a manifest's install profiles.
///
/// Shared runtimes live under `shared-runtimes/<id>` and must remain writable
/// for setup/start even though they are outside `plugin-runtimes/<plugin-id>`.
fn collect_shared_runtime_roots(manifest: &AiPluginManifest) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();
    for profile in &manifest.install_profiles {
        for binding in profile_runtime_bindings(profile) {
            if !binding.scope.eq_ignore_ascii_case("shared") {
                continue;
            }
            let runtime_id = binding
                .id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .unwrap_or(profile.id.as_str());
            let Ok(dir) = shared_runtime_root(runtime_id) else {
                continue;
            };
            let key = normalize_path(&dir);
            if seen.insert(key) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// User-bound external model directories for this plugin (any profile).
///
/// These are not under the store root but are intentionally granted to the
/// plugin process via env injection; staging must not re-copy them and ACL
/// deny mode must not lock them out.
fn collect_bound_model_dirs(plugin_id: &str) -> Vec<PathBuf> {
    let Ok(registry) = load_registry() else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();
    for state in registry.profile_states.values() {
        if state.plugin_id != plugin_id {
            continue;
        }
        for path in state.model_dir_bindings.values() {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                continue;
            }
            let dir = PathBuf::from(trimmed);
            if !dir.is_dir() {
                continue;
            }
            let key = normalize_path(&dir);
            if seen.insert(key) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// Host-side write/read allow-list roots for a managed plugin process.
///
/// This is **not** an OS sandbox. It is the single source of truth for:
/// - input staging (paths already under these roots are not re-copied)
/// - Windows deny-ACL exclusion lists
///
/// Always includes:
/// - `plugin-data/<id>` (covers models/logs/config)
/// - `plugin-cache/<id>`
/// - `plugin-outputs/<id>`
/// - `plugin-runtimes/<id>`
/// - installed/registered code root
/// - shared runtimes declared by the manifest (when provided)
/// - persisted external model-dir bindings
/// plus any `extra_dirs` (task dir, task output dir, etc.).
fn plugin_writable_roots(
    plugin_id: &str,
    code_root: &Path,
    manifest: Option<&AiPluginManifest>,
    extra_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |path: PathBuf| {
        if path.as_os_str().is_empty() {
            return;
        }
        let key = normalize_path(&path);
        if seen.insert(key) {
            roots.push(path);
        }
    };

    push(plugin_data_dir(plugin_id)?);
    push(plugin_cache_dir(plugin_id)?);
    push(plugin_output_dir(plugin_id)?);
    push(plugin_runtime_root(plugin_id)?);
    push(code_root.to_path_buf());

    if let Some(manifest) = manifest {
        for dir in collect_shared_runtime_roots(manifest) {
            push(dir);
        }
    }
    for dir in collect_bound_model_dirs(plugin_id) {
        push(dir);
    }
    for dir in extra_dirs {
        push(dir);
    }
    Ok(roots)
}

fn program_data_plugin_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("PROGRAMDATA").map(|p| PathBuf::from(p).join("PicAiPic").join("plugins"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

fn load_registry() -> Result<AiPluginRegistry, String> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(AiPluginRegistry::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read plugin registry: {}", e))?;
    let mut registry =
        serde_json::from_str::<AiPluginRegistry>(content.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("Failed to parse plugin registry: {}", e))?;
    normalize_registry_trust(&mut registry);
    Ok(registry)
}

/// Migrate legacy single-key trust entries and keep display fields in sync.
fn normalize_registry_trust(registry: &mut AiPluginRegistry) {
    for tp in registry.trusted_publishers.values_mut() {
        normalize_trusted_publisher(tp);
    }
    // Drop empty revoke entries.
    registry
        .revoked_keys
        .retain(|k| !k.public_key.trim().is_empty());
}

fn normalize_trusted_publisher(tp: &mut AiPluginTrustedPublisher) {
    if tp.keys.is_empty() {
        let pk = tp.public_key.trim();
        if !pk.is_empty() {
            tp.keys.push(AiPluginTrustedKey {
                public_key: pk.to_string(),
                trusted_at: if tp.trusted_at.trim().is_empty() {
                    Utc::now().to_rfc3339()
                } else {
                    tp.trusted_at.clone()
                },
                status: default_trusted_key_status(),
            });
        }
    }
    // Prefer first active key for legacy display fields.
    if let Some(active) = tp
        .keys
        .iter()
        .find(|k| k.status.eq_ignore_ascii_case("active") && !k.public_key.trim().is_empty())
        .cloned()
        .or_else(|| tp.keys.first().cloned())
    {
        if tp.public_key.trim().is_empty() {
            tp.public_key = active.public_key.clone();
        }
        if tp.trusted_at.trim().is_empty() {
            tp.trusted_at = active.trusted_at.clone();
        }
    }
}

fn is_public_key_revoked(registry: &AiPluginRegistry, public_key: &str) -> bool {
    let pk = public_key.trim();
    if pk.is_empty() {
        return false;
    }
    registry
        .revoked_keys
        .iter()
        .any(|k| k.public_key.trim() == pk)
}

fn publisher_accepts_public_key(tp: &AiPluginTrustedPublisher, public_key: &str) -> bool {
    let pk = public_key.trim();
    if pk.is_empty() {
        return false;
    }
    if tp
        .keys
        .iter()
        .any(|k| k.public_key.trim() == pk && k.status.eq_ignore_ascii_case("active"))
    {
        return true;
    }
    // Legacy single-key field when keys list empty/unnormalized.
    tp.keys.is_empty() && tp.public_key.trim() == pk
}

fn trust_publisher_key_in_registry(
    registry: &mut AiPluginRegistry,
    publisher: String,
    public_key: String,
) {
    // Re-trusting a key removes it from the local revoke list (explicit user action).
    registry
        .revoked_keys
        .retain(|k| k.public_key.trim() != public_key.trim());

    let now = Utc::now().to_rfc3339();
    let entry = registry
        .trusted_publishers
        .entry(publisher.clone())
        .or_insert_with(|| AiPluginTrustedPublisher {
            publisher: publisher.clone(),
            public_key: public_key.clone(),
            trusted_at: now.clone(),
            keys: Vec::new(),
        });

    if let Some(existing) = entry
        .keys
        .iter_mut()
        .find(|k| k.public_key.trim() == public_key.trim())
    {
        existing.status = default_trusted_key_status();
        existing.trusted_at = now.clone();
    } else {
        entry.keys.push(AiPluginTrustedKey {
            public_key: public_key.clone(),
            trusted_at: now.clone(),
            status: default_trusted_key_status(),
        });
    }
    entry.public_key = public_key;
    entry.trusted_at = now;
    entry.publisher = publisher;
    normalize_trusted_publisher(entry);
}

fn save_registry(registry: &AiPluginRegistry) -> Result<(), String> {
    let path = registry_path()?;
    let content = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Failed to serialize plugin registry: {}", e))?;
    let parent = path
        .parent()
        .ok_or_else(|| "Plugin registry path has no parent directory".to_string())?;
    let temp = parent.join(format!(".plugin-registry-{}.tmp", Uuid::new_v4()));
    let backup = parent.join(format!(".plugin-registry-{}.bak", Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("Failed to create plugin registry temp file: {}", e))?;
    if let Err(error) = file
        .write_all(content.as_bytes())
        .and_then(|_| file.sync_all())
    {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "Failed to write plugin registry temp file: {}",
            error
        ));
    }
    drop(file);

    let had_previous = path.exists();
    if had_previous {
        if let Err(error) = fs::rename(&path, &backup) {
            let _ = fs::remove_file(&temp);
            return Err(format!(
                "Failed to stage previous plugin registry: {}",
                error
            ));
        }
    }
    if let Err(error) = fs::rename(&temp, &path) {
        let restore_error = if had_previous {
            fs::rename(&backup, &path).err()
        } else {
            None
        };
        let _ = fs::remove_file(&temp);
        return Err(match restore_error {
            Some(restore_error) => format!(
                "Failed to replace plugin registry: {}; rollback also failed: {}",
                error, restore_error
            ),
            None => format!("Failed to replace plugin registry: {}", error),
        });
    }
    if had_previous {
        if let Err(error) = fs::remove_file(&backup) {
            eprintln!(
                "Updated plugin registry but could not remove backup '{}': {}",
                backup.display(),
                error
            );
        }
    }
    Ok(())
}

fn profile_state_key(plugin_id: &str, profile_id: &str) -> String {
    format!("{}:{}", plugin_id, profile_id)
}

fn permission_grant_key(plugin_id: &str) -> String {
    plugin_id.to_string()
}

fn plugin_task_state_key(plugin_id: &str, task_id: &str) -> String {
    format!("{}:{}", plugin_id, task_id)
}

fn runtime_probe_key(
    plugin_id: &str,
    profile_id: &str,
    backend: &str,
    runtime_binding: Option<&PluginRuntimeBinding>,
) -> String {
    let runtime = runtime_binding
        .and_then(|binding| binding.python.as_deref().or(binding.id.as_deref()))
        .unwrap_or("runtime");
    format!("{}:{}:{}:{}", plugin_id, profile_id, backend, runtime)
}

fn save_profile_state(state: AiPluginProfileState) -> Result<(), String> {
    let mut registry = load_registry()?;
    registry.profile_states.insert(
        profile_state_key(&state.plugin_id, &state.profile_id),
        state,
    );
    save_registry(&registry)
}

/// Returns the currently persisted `model_dir_bindings` for a profile, so that
/// setup/smoke flows re-constructing the profile state do not clobber a
/// user-configured external model directory binding.
fn persisted_model_dir_bindings(plugin_id: &str, profile_id: &str) -> HashMap<String, String> {
    load_registry()
        .ok()
        .and_then(|registry| {
            registry
                .profile_states
                .get(&profile_state_key(plugin_id, profile_id))
                .map(|state| state.model_dir_bindings.clone())
        })
        .unwrap_or_default()
}

fn save_runtime_probe_state(state: AiPluginRuntimeProbeState) -> Result<(), String> {
    let mut registry = load_registry()?;
    registry.runtime_probe_states.insert(
        runtime_probe_key(
            &state.plugin_id,
            &state.profile_id,
            &state.backend,
            state.runtime_binding.as_ref(),
        ),
        state,
    );
    save_registry(&registry)
}

fn clear_profile_runtime_probe_states(
    plugin_id: &str,
    profile_id: &str,
    backend: &str,
) -> Result<(), String> {
    let mut registry = load_registry()?;
    let prefix = format!("{}:{}:{}:", plugin_id, profile_id, backend);
    registry
        .runtime_probe_states
        .retain(|key, _state| !key.starts_with(&prefix));
    save_registry(&registry)
}

fn save_setup_job(job: AiPluginSetupJob) -> Result<(), String> {
    let mut registry = load_registry()?;
    registry.setup_jobs.insert(job.id.clone(), job);
    save_registry(&registry)
}

fn save_task_state(state: AiPluginTaskState) -> Result<(), String> {
    let mut registry = load_registry()?;
    registry.task_states.insert(
        plugin_task_state_key(&state.plugin_id, &state.task_id),
        state,
    );
    save_registry(&registry)
}

fn parse_bool_field(object: &serde_json::Map<String, Value>, key: &str) -> bool {
    object.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn parse_string_array_field(object: &serde_json::Map<String, Value>, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_plugin_permissions(permissions: Option<&Value>) -> AiPluginPermissionsSummary {
    let Some(Value::Object(root)) = permissions else {
        return AiPluginPermissionsSummary::default();
    };

    let mut summary = AiPluginPermissionsSummary {
        read_selected_files: parse_bool_field(root, "readSelectedFiles"),
        write_output_dir: parse_bool_field(root, "writeOutputDir"),
        write_source_files: parse_bool_field(root, "writeSourceFiles"),
        launch_child_processes: parse_bool_field(root, "launchChildProcesses"),
        ..AiPluginPermissionsSummary::default()
    };

    match root.get("network") {
        Some(Value::Bool(enabled)) => {
            summary.network.runtime = *enabled;
        }
        Some(Value::Object(network)) => {
            summary.network.runtime = parse_bool_field(network, "runtime");
            summary.network.setup_downloads = parse_bool_field(network, "setupDownloads");
            summary.network.upload_selected_files =
                parse_bool_field(network, "uploadSelectedFiles");
            summary.network.upload_outputs = parse_bool_field(network, "uploadOutputs");
            summary.network.allowed_domains = parse_string_array_field(network, "allowedDomains");
        }
        _ => {}
    }

    summary
}

fn append_setup_log(plugin_id: &str, job: &AiPluginSetupJob) -> Result<PathBuf, String> {
    let logs_dir = plugin_logs_dir(plugin_id)?;
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Failed to create plugin logs directory: {}", e))?;
    let path = logs_dir.join(format!("setup-{}.log", job.id));
    let content = job.log.join("\n");
    fs::write(&path, format!("{}\n", content))
        .map_err(|e| format!("Failed to write setup log '{}': {}", path.display(), e))?;
    Ok(path)
}

fn file_hash(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(blake3::hash(&bytes).to_hex().to_string())
}

fn file_mtime_ms(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
}

fn runtime_binding_signature(runtime_binding: Option<&PluginRuntimeBinding>) -> Option<String> {
    let binding = runtime_binding?;
    let value = serde_json::to_vec(binding).ok()?;
    Some(blake3::hash(&value).to_hex().to_string())
}

/// A single parsed line from a Python requirements file: a package name plus an
/// optional PEP 440 version specifier such as `==1.26.4` or `>=2`.
#[derive(Debug, Clone)]
struct RequirementSpec {
    package: String,
    spec: String,
}

/// A detected mismatch between a requirements-declared version spec and the
/// version actually installed in the probed runtime.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConflict {
    pub package: String,
    pub declared_spec: String,
    pub installed_version: String,
    pub available: bool,
    pub kind: String,
    pub message: String,
}

/// Map import names used by the probe script to canonical pip package names
/// declared in requirements files, and vice versa. The probe reports
/// `opencv-python` (it imports `cv2`), so the only frequent mismatch is
/// callers passing `cv2`. We normalize everything to the pip name.
fn normalize_package_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "cv2" => "opencv-python".to_string(),
        "pil" | "pillow" => "pillow".to_string(),
        "skimage" => "scikit-image".to_string(),
        "yaml" => "pyyaml".to_string(),
        "torch_directml" => "torch-directml".to_string(),
        _ => lower,
    }
}

/// Parse a Python requirements file into a list of package + specifier pairs.
/// Lines that are blank, comments (`#`), option flags (`-r`, `--index-url`,
/// `--extra-index-url`, `-e`, `--editable`, etc.), or bare URLs are skipped.
/// Lines like `numpy==1.26.4` become `RequirementSpec { package: "numpy",
/// spec: "==1.26.4" }`. A bare version with no operator is treated as `==`.
fn parse_requirements_file(content: &str) -> Vec<RequirementSpec> {
    let mut specs = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Skip pip option lines and URL-only lines (e.g. ROCm direct wheels).
        if line.starts_with('-')
            || line.starts_with("--")
            || line.starts_with("https://")
            || line.starts_with("http://")
            || line.starts_with("git+")
            || line.starts_with("file:")
        {
            continue;
        }
        // Strip environment markers and extras like `pkg[extra]==1.0 ; python_version<"3"`.
        let line = line.split_whitespace().next().unwrap_or(line);
        let line = line.split(';').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }
        // Split package name from version specifier at the first operator char.
        let split_at =
            line.find(|c: char| c == '=' || c == '<' || c == '>' || c == '~' || c == '!');
        let (package, spec) = match split_at {
            Some(pos) => (
                line[..pos].trim().to_string(),
                line[pos..].trim().to_string(),
            ),
            None => (line.to_string(), String::new()),
        };
        // Strip extras like `opencv-python[headless]` -> `opencv-python`.
        let package = package.split('[').next().unwrap_or(&package).to_string();
        if package.is_empty() {
            continue;
        }
        specs.push(RequirementSpec {
            package: normalize_package_name(&package),
            spec,
        });
    }
    specs
}

/// Parse a version string like `2.9.1+rocm7.2.1` into a vector of numeric
/// components `[2, 9, 1]`. Local version segments after `+` are stripped.
/// Non-numeric components are parsed as 0 so that `1.0rc1` does not crash.
fn parse_version(v: &str) -> Vec<u64> {
    let v = v.split('+').next().unwrap_or(v).trim();
    let v = v.split('-').next().unwrap_or(v); // drop post/pre release dashes
    v.split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<u64>().unwrap_or(0)
        })
        .collect()
}

/// Compare two version vectors lexicographically, padding the shorter one
/// with zeros so `[2,9]` vs `[2,9,0]` compare equal.
fn compare_versions(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    let len = a.len().max(b.len());
    for i in 0..len {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        match av.cmp(&bv) {
            std::cmp::Ordering::Equal => continue,
            order => return order,
        }
    }
    std::cmp::Ordering::Equal
}

/// Check whether an installed version string satisfies a PEP 440 specifier.
/// Supports `==`, `!=`, `>=`, `<=`, `>`, `<`, `~=` and bare versions (treated
/// as `==`). Compound specs like `>=1.0,<2.0` are split on commas and all
/// must hold. An empty spec means "any version is fine".
fn spec_satisfied(spec: &str, installed: &str) -> bool {
    let spec = spec.trim();
    if spec.is_empty() {
        return true;
    }
    let installed_vec = parse_version(installed);
    for clause in spec.split(',') {
        let clause = clause.trim();
        if clause.is_empty() {
            continue;
        }
        let (op, version_str) = if let Some(rest) = clause.strip_prefix("==") {
            ("==", rest.trim())
        } else if let Some(rest) = clause.strip_prefix("!=") {
            ("!=", rest.trim())
        } else if let Some(rest) = clause.strip_prefix(">=") {
            (">=", rest.trim())
        } else if let Some(rest) = clause.strip_prefix("<=") {
            ("<=", rest.trim())
        } else if let Some(rest) = clause.strip_prefix("~=") {
            ("~=", rest.trim())
        } else if let Some(rest) = clause.strip_prefix('>') {
            (">", rest.trim())
        } else if let Some(rest) = clause.strip_prefix('<') {
            ("<", rest.trim())
        } else {
            // Bare version with no operator -> exact match.
            ("==", clause)
        };
        let clause_vec = parse_version(version_str);
        let cmp = compare_versions(&installed_vec, &clause_vec);
        let satisfied = match op {
            "==" => cmp == std::cmp::Ordering::Equal,
            "!=" => cmp != std::cmp::Ordering::Equal,
            ">=" => cmp != std::cmp::Ordering::Less,
            "<=" => cmp != std::cmp::Ordering::Greater,
            ">" => cmp == std::cmp::Ordering::Greater,
            "<" => cmp == std::cmp::Ordering::Less,
            "~=" => {
                // Compatible release: ~=X.Y means >=X.Y,<X+1; ~=X.Y.Z means
                // >=X.Y.Z,<X.Y+1. Requires at least two components.
                if clause_vec.len() < 2 {
                    cmp != std::cmp::Ordering::Less
                } else {
                    let mut upper = clause_vec.clone();
                    // Zero out the last component then increment the
                    // second-to-last.
                    let bump_idx = upper.len() - 2;
                    upper[bump_idx] += 1;
                    for i in (bump_idx + 1)..upper.len() {
                        upper[i] = 0;
                    }
                    cmp != std::cmp::Ordering::Less
                        && compare_versions(&installed_vec, &upper) == std::cmp::Ordering::Less
                }
            }
            _ => true,
        };
        if !satisfied {
            return false;
        }
    }
    true
}

/// Compare a plugin's declared requirements against the versions reported by a
/// probe result. `probe_result` is the JSON object emitted by
/// `python_runtime_probe_script` (field `packages` maps pip name -> {available,
/// version}). Returns a list of conflicts; `version_mismatch` and `missing`
/// kinds are blocking, `unprobed` is informational.
fn detect_runtime_conflicts(
    requirements_path: &Path,
    probe_result: &Value,
) -> Vec<RuntimeConflict> {
    let Ok(content) = fs::read_to_string(requirements_path) else {
        return Vec::new();
    };
    let specs = parse_requirements_file(&content);
    let packages = probe_result.get("packages").and_then(|p| p.as_object());
    let mut conflicts = Vec::new();
    for spec in specs {
        let canonical = normalize_package_name(&spec.package);
        let Some(packages) = packages else {
            conflicts.push(RuntimeConflict {
                package: canonical.clone(),
                declared_spec: spec.spec.clone(),
                installed_version: String::new(),
                available: false,
                kind: "unprobed".to_string(),
                message: format!(
                    "{} declared {} but probe did not report package versions",
                    canonical, spec.spec
                ),
            });
            continue;
        };
        let Some(info) = packages.get(&canonical) else {
            // Package declared in requirements but not probed at all (e.g.
            // NAFNet's timm/skimage). Informational, not blocking.
            conflicts.push(RuntimeConflict {
                package: canonical.clone(),
                declared_spec: spec.spec.clone(),
                installed_version: String::new(),
                available: false,
                kind: "unprobed".to_string(),
                message: format!(
                    "{} declared {} but probe did not inspect this package",
                    canonical, spec.spec
                ),
            });
            continue;
        };
        let available = info
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let installed_version = info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !available {
            conflicts.push(RuntimeConflict {
                package: canonical.clone(),
                declared_spec: spec.spec.clone(),
                installed_version: String::new(),
                available: false,
                kind: "missing".to_string(),
                message: format!(
                    "{} declared {} but is not installed in the probed runtime",
                    canonical, spec.spec
                ),
            });
            continue;
        }
        if !spec_satisfied(&spec.spec, &installed_version) {
            conflicts.push(RuntimeConflict {
                package: canonical.clone(),
                declared_spec: spec.spec.clone(),
                installed_version: installed_version.clone(),
                available: true,
                kind: "version_mismatch".to_string(),
                message: format!(
                    "{} declared {} but installed {}",
                    canonical, spec.spec, installed_version
                ),
            });
        }
    }
    conflicts
}

fn runtime_root_from_python(python: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        python
            .parent()
            .and_then(|scripts| scripts.parent())
            .map(PathBuf::from)
    } else {
        python
            .parent()
            .and_then(|bin| bin.parent())
            .map(PathBuf::from)
    }
}

fn runtime_probe_fingerprint(
    plugin_root: &Path,
    profile: &PluginInstallProfile,
    runtime_binding: Option<&PluginRuntimeBinding>,
) -> Value {
    let python = runtime_binding
        .and_then(|binding| binding.python.as_deref())
        .filter(|value| !value.trim().is_empty());
    let python_path = python.map(PathBuf::from);
    let python_metadata = python_path
        .as_ref()
        .and_then(|path| fs::metadata(path).ok());
    let runtime_root = runtime_binding
        .and_then(|binding| binding.root.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| python_path.as_deref().and_then(runtime_root_from_python));
    let pyvenv_cfg = runtime_root.as_ref().map(|root| root.join("pyvenv.cfg"));
    let requirements =
        profile_requirements(profile).map(|requirements| plugin_root.join(requirements));

    serde_json::json!({
        "pythonPath": python_path.as_ref().map(|path| normalize_path(path)),
        "pythonExists": python_path.as_ref().map(|path| path.is_file()).unwrap_or(false),
        "pythonExeSize": python_metadata.as_ref().map(|metadata| metadata.len()),
        "pythonExeMtimeMs": python_metadata.as_ref().and_then(file_mtime_ms),
        "runtimeRoot": runtime_root.as_ref().map(|path| normalize_path(path)),
        "pyvenvCfgHash": pyvenv_cfg.as_ref().and_then(|path| file_hash(path)),
        "requirementsPath": requirements.as_ref().map(|path| normalize_path(path)),
        "requirementsHash": requirements.as_ref().and_then(|path| file_hash(path)),
        "runtimeBindingHash": runtime_binding_signature(runtime_binding),
    })
}

fn runtime_probe_ttl_secs(runtime_binding: Option<&PluginRuntimeBinding>) -> i64 {
    match runtime_binding.map(|binding| binding.scope.as_str()) {
        Some("plugin") => PLUGIN_RUNTIME_PROBE_TTL_SECS,
        Some("shared") => SHARED_RUNTIME_PROBE_TTL_SECS,
        _ => EXTERNAL_RUNTIME_PROBE_TTL_SECS,
    }
}

fn mark_runtime_probe_staleness(
    mut state: AiPluginRuntimeProbeState,
    current_fingerprint: Value,
) -> AiPluginRuntimeProbeState {
    let mut stale_reason = None;
    if current_fingerprint
        .get("pythonExists")
        .and_then(Value::as_bool)
        != Some(true)
    {
        stale_reason = Some("python_missing".to_string());
    } else if state.fingerprint.as_ref() != Some(&current_fingerprint) {
        stale_reason = Some("fingerprint_changed".to_string());
    } else if let Ok(probed_at) = chrono::DateTime::parse_from_rfc3339(&state.probed_at) {
        let ttl = runtime_probe_ttl_secs(state.runtime_binding.as_ref());
        if Utc::now()
            .signed_duration_since(probed_at.with_timezone(&Utc))
            .num_seconds()
            > ttl
        {
            stale_reason = Some("ttl_expired".to_string());
        }
    } else {
        stale_reason = Some("invalid_probed_at".to_string());
    }

    if let Some(reason) = stale_reason {
        state.stale = true;
        state.stale_reason = Some(reason);
        if state.status == "passed" {
            state.status = "stale".to_string();
        }
    } else {
        state.stale = false;
        state.stale_reason = None;
    }
    state
}

fn parse_json_object_from_process_stdout(stdout: &str) -> Result<Value, String> {
    match serde_json::from_str::<Value>(stdout) {
        Ok(value) => Ok(value),
        Err(first_error) => {
            for (index, _) in stdout.match_indices('{') {
                let candidate = stdout[index..].trim();
                if let Ok(value) = serde_json::from_str::<Value>(candidate) {
                    return Ok(value);
                }
            }

            let preview: String = stdout.chars().take(500).collect();
            Err(format!("{}; stdout started with: {}", first_error, preview))
        }
    }
}

fn parse_optional_json_object_from_process_stdout(stdout: &str) -> Option<Value> {
    if stdout.trim().is_empty() {
        None
    } else {
        parse_json_object_from_process_stdout(stdout).ok()
    }
}

fn maybe_persist_runtime_probe_state(
    request: &AiPluginPythonRuntimeProbeRequest,
    available: bool,
    duration_ms: u64,
    result: Option<Value>,
    error: Option<String>,
) -> Result<Option<AiPluginRuntimeProbeState>, String> {
    let Some(plugin_id) = request
        .plugin_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let Some(profile_id) = request
        .profile_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };

    let (manifest_path, manifest) = find_plugin_manifest(plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let profile = find_manifest_profile(&manifest, plugin_id, profile_id)?;
    let runtime_binding = request
        .runtime_binding
        .clone()
        .or_else(|| selected_runtime_binding(profile, None, None).ok().flatten());
    let effective_profile = profile_with_runtime_binding(profile, runtime_binding.clone());
    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let capability = request
        .capability
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_smoke_capability(&manifest));
    let backend = request
        .backend
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| profile.backend.clone());
    let state = AiPluginRuntimeProbeState {
        plugin_id: plugin_id.to_string(),
        profile_id: profile_id.to_string(),
        backend,
        capability,
        status: if available { "passed" } else { "failed" }.to_string(),
        available,
        probed_at: Utc::now().to_rfc3339(),
        stale: false,
        stale_reason: None,
        duration_ms: Some(duration_ms),
        error,
        result,
        runtime_binding: runtime_binding.clone(),
        fingerprint: Some(runtime_probe_fingerprint(
            root,
            &effective_profile,
            runtime_binding.as_ref(),
        )),
    };
    save_runtime_probe_state(state.clone())?;
    Ok(Some(state))
}

fn safe_profile_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && path.is_relative()
        && !path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
}

fn prepare_profile_local_artifacts(
    root: &Path,
    profile: &PluginInstallProfile,
    job: &mut AiPluginSetupJob,
) -> Result<(), String> {
    if let Some(env_dir) = profile
        .env_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if !safe_profile_relative_path(env_dir) {
            return Err(format!(
                "Install profile '{}' has unsafe envDir '{}'",
                profile.id, env_dir
            ));
        }
        let env_path =
            profile_runtime_dir(&job.plugin_id, profile)?.unwrap_or_else(|| root.join(env_dir));
        job.push_log(format!(
            "Profile environment directory declared: {}",
            env_path.display()
        ));
    } else {
        job.push_log("No envDir declared for this profile.".to_string());
    }

    if let Some(requirements) = profile_requirements(profile) {
        if !safe_profile_relative_path(requirements) {
            return Err(format!(
                "Install profile '{}' has unsafe requirements path '{}'",
                profile.id, requirements
            ));
        }
        let requirements_path = root.join(requirements);
        if requirements_path.is_file() {
            job.push_log(format!(
                "Requirements file is present: {}",
                requirements_path.display()
            ));
        } else {
            job.push_log(format!(
                "Requirements file is not present yet: {}",
                requirements_path.display()
            ));
        }
    }

    let setup_log_path = plugin_logs_dir(&job.plugin_id)?.join(format!("setup-{}.log", job.id));
    job.push_log(format!("Wrote setup log: {}", setup_log_path.display()));
    append_setup_log(&job.plugin_id, job)?;
    Ok(())
}

fn find_manifest_profile<'a>(
    manifest: &'a AiPluginManifest,
    plugin_id: &str,
    profile_id: &str,
) -> Result<&'a PluginInstallProfile, String> {
    manifest
        .install_profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| {
            format!(
                "Install profile '{}' was not found in plugin '{}'",
                profile_id, plugin_id
            )
        })
}

fn setup_capability(
    manifest: &AiPluginManifest,
    requested: Option<String>,
) -> Result<String, String> {
    let capability = requested
        .filter(|capability| !capability.trim().is_empty())
        .unwrap_or_else(|| default_smoke_capability(manifest));
    if !capability.is_empty() {
        find_plugin_capability(manifest, &capability)?;
    }
    Ok(capability)
}

fn new_setup_job(
    plugin_id: &str,
    profile: &PluginInstallProfile,
    backend: &str,
    capability: &str,
    root: &Path,
    message: &str,
) -> AiPluginSetupJob {
    let now = Utc::now().to_rfc3339();
    AiPluginSetupJob {
        id: Uuid::new_v4().to_string(),
        plugin_id: plugin_id.to_string(),
        profile_id: profile.id.clone(),
        backend: backend.to_string(),
        capability: capability.to_string(),
        status: "running".to_string(),
        created_at: now.clone(),
        updated_at: now,
        progress: 10,
        message: Some(message.to_string()),
        error: None,
        log: vec![
            "Created setup job record.".to_string(),
            format!("Plugin root: {}", root.display()),
            format!("Profile: {}", profile.id),
            format!("Backend: {}", backend),
        ],
    }
}

fn build_setup_environment(
    root: &Path,
    plugin_id: &str,
    profile: &PluginInstallProfile,
    backend: &str,
    capability: &str,
    model_bindings: &[PluginModelBinding],
) -> Result<HashMap<String, String>, String> {
    let mut environment = HashMap::new();
    let data_dir = plugin_data_dir(plugin_id)?;
    let cache_dir = plugin_cache_dir(plugin_id)?;
    let logs_dir = plugin_logs_dir(plugin_id)?;
    let model_dir = plugin_model_dir(plugin_id)?;
    let config_path = plugin_config_path(plugin_id)?;
    let runtime_root = profile_runtime_root(plugin_id, profile)?;

    environment.insert("PICAIPIC_PLUGIN_ID".to_string(), plugin_id.to_string());
    environment.insert("PICAIPIC_PLUGIN_PROFILE_ID".to_string(), profile.id.clone());
    environment.insert("PICAIPIC_PLUGIN_BACKEND".to_string(), backend.to_string());
    environment.insert(
        "PICAIPIC_PLUGIN_CAPABILITY".to_string(),
        capability.to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_ROOT".to_string(),
        root.display().to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_DATA_DIR".to_string(),
        data_dir.display().to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_CACHE_DIR".to_string(),
        cache_dir.display().to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_LOG_DIR".to_string(),
        logs_dir.display().to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_MODEL_DIR".to_string(),
        model_dir.display().to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_CONFIG_PATH".to_string(),
        config_path.display().to_string(),
    );
    environment.insert(
        "PICAIPIC_PLUGIN_RUNTIME_DIR".to_string(),
        runtime_root.display().to_string(),
    );
    if let Some(binding) = profile.runtime_binding.as_ref() {
        if !binding.scope.trim().is_empty() {
            environment.insert(
                "PICAIPIC_PLUGIN_RUNTIME_SCOPE".to_string(),
                binding.scope.clone(),
            );
        }
        if let Some(kind) = binding
            .kind
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            environment.insert("PICAIPIC_PLUGIN_RUNTIME_KIND".to_string(), kind.to_string());
        }
        if let Some(id) = binding
            .id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            environment.insert("PICAIPIC_PLUGIN_RUNTIME_ID".to_string(), id.to_string());
        }
        if let Some(python) = binding
            .python
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            environment.insert("PICAIPIC_PLUGIN_PYTHON".to_string(), python.to_string());
        }
        if let Some(root_path) = binding
            .root
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            environment.insert(
                "PICAIPIC_PLUGIN_RUNTIME_ROOT".to_string(),
                root_path.to_string(),
            );
        }
    }
    if let Some(env_dir) = profile
        .env_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if !safe_profile_relative_path(env_dir) {
            return Err(format!(
                "Install profile '{}' has unsafe envDir '{}'",
                profile.id, env_dir
            ));
        }
        if let Some(runtime_dir) = profile_runtime_dir(plugin_id, profile)? {
            let runtime_dir = runtime_dir.display().to_string();
            environment.insert("PICAIPIC_PLUGIN_ENV_DIR".to_string(), runtime_dir.clone());
            environment.insert("PICAIPIC_PLUGIN_ENV_PATH".to_string(), runtime_dir);
        }
    }
    if let Some(requirements) = profile_requirements(profile) {
        environment.insert(
            "PICAIPIC_PLUGIN_REQUIREMENTS".to_string(),
            requirements.to_string(),
        );
        environment.insert(
            "PICAIPIC_PLUGIN_REQUIREMENTS_PATH".to_string(),
            root.join(requirements).display().to_string(),
        );
    }
    // Inject user-configured external model directory bindings. Each manifest
    // binding declares an env var; the host looks up the persisted user-selected
    // directory for this profile and injects it. This replaces hand-editing
    // `.local.env` for ordinary users. Bindings without a persisted directory
    // are skipped so the plugin falls back to its default model resolution.
    let bindings = persisted_model_dir_bindings(plugin_id, &profile.id);
    for binding in model_bindings {
        if binding.env_var.trim().is_empty() {
            continue;
        }
        let Some(dir) = bindings.get(&binding.id) else {
            continue;
        };
        let dir = dir.trim();
        if dir.is_empty() {
            continue;
        }
        environment.insert(binding.env_var.clone(), dir.to_string());
        for extra in &binding.env_vars {
            let extra = extra.trim();
            if !extra.is_empty() {
                environment.insert(extra.to_string(), dir.to_string());
            }
        }
    }
    Ok(environment)
}

fn profile_requirements(profile: &PluginInstallProfile) -> Option<&str> {
    profile
        .runtime_binding
        .as_ref()
        .and_then(|binding| binding.requirements.as_deref())
        .or(profile.requirements.as_deref())
        .filter(|value| !value.trim().is_empty())
}

fn profile_state_priority(state: &AiPluginProfileState) -> i32 {
    if state.verified || state.status.eq_ignore_ascii_case("verified") {
        return 3;
    }
    if state.status.eq_ignore_ascii_case("needsVerify") {
        return 2;
    }
    if state.status.eq_ignore_ascii_case("installing") {
        return 1;
    }
    0
}

fn preferred_profile_state_for_plugin(
    registry: &AiPluginRegistry,
    plugin_id: &str,
) -> Option<AiPluginProfileState> {
    registry
        .profile_states
        .values()
        .filter(|state| state.plugin_id == plugin_id)
        .cloned()
        .max_by(|a, b| {
            profile_state_priority(a)
                .cmp(&profile_state_priority(b))
                .then_with(|| a.updated_at.cmp(&b.updated_at))
        })
}

fn selected_runtime_binding(
    profile: &PluginInstallProfile,
    runtime_binding_id: Option<&str>,
    runtime_binding: Option<PluginRuntimeBinding>,
) -> Result<Option<PluginRuntimeBinding>, String> {
    if let Some(binding) = runtime_binding {
        if let Some(id) = runtime_binding_id.filter(|value| !value.trim().is_empty()) {
            if binding.id.as_deref() != Some(id) {
                return Err(format!(
                    "Runtime binding override id does not match requested runtime binding '{}'",
                    id
                ));
            }
        }
        return Ok(Some(binding));
    }

    let bindings = profile_runtime_bindings(profile);
    if bindings.is_empty() {
        if let Some(id) = runtime_binding_id.filter(|value| !value.trim().is_empty()) {
            return Err(format!(
                "Install profile '{}' does not declare runtime binding '{}'",
                profile.id, id
            ));
        }
        return Ok(None);
    }

    if let Some(id) = runtime_binding_id.filter(|value| !value.trim().is_empty()) {
        return bindings
            .into_iter()
            .find(|binding| binding.id.as_deref() == Some(id))
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "Runtime binding '{}' was not found in install profile '{}'",
                    id, profile.id
                )
            });
    }

    Ok(bindings.into_iter().next())
}

fn profile_runtime_bindings(profile: &PluginInstallProfile) -> Vec<PluginRuntimeBinding> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    if let Some(binding) = profile.runtime_binding.clone() {
        let key = binding
            .id
            .clone()
            .unwrap_or_else(|| format!("{}:{}", binding.scope, bindings.len()));
        seen.insert(key);
        bindings.push(binding);
    }
    for binding in &profile.runtime_bindings {
        let key = binding
            .id
            .clone()
            .unwrap_or_else(|| format!("{}:{}", binding.scope, bindings.len()));
        if seen.insert(key) {
            bindings.push(binding.clone());
        }
    }
    bindings
}

fn profile_with_runtime_binding(
    profile: &PluginInstallProfile,
    runtime_binding: Option<PluginRuntimeBinding>,
) -> PluginInstallProfile {
    let mut effective = profile.clone();
    effective.runtime_binding = runtime_binding;
    effective
}

/// Build a synthetic plugin-private runtime binding for a profile.
///
/// Shared profiles declare `envDir` for the private isolation path
/// (`plugin-runtimes/<plugin-id>/<envDir>`). The host can switch a profile to
/// this binding after user confirmation when a shared runtime has blocking
/// package conflicts. The binding is intentionally not required in the
/// plugin manifest so authors can keep shared defaults.
fn plugin_private_runtime_binding(
    profile: &PluginInstallProfile,
) -> Result<PluginRuntimeBinding, String> {
    let env_dir = profile
        .env_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "Install profile '{}' does not declare envDir required for a plugin-private runtime",
                profile.id
            )
        })?;
    if !safe_profile_relative_path(env_dir) {
        return Err(format!(
            "Install profile '{}' has unsafe envDir '{}'",
            profile.id, env_dir
        ));
    }
    let label = profile
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(profile.id.as_str());
    Ok(PluginRuntimeBinding {
        scope: "plugin".to_string(),
        kind: Some("python".to_string()),
        id: Some(format!("plugin-private:{}", profile.id)),
        label: Some(format!("Plugin-private {}", label)),
        python: None,
        root: None,
        requirements: profile_requirements(profile).map(|value| value.to_string()),
        notes: Some(
            "Isolated plugin-private runtime under plugin-runtimes/<plugin-id>/<envDir>. Shared runtimes are left unchanged."
                .to_string(),
        ),
    })
}

fn start_profile_signature(
    start_profile: Option<&(PluginInstallProfile, String, String)>,
) -> Option<String> {
    start_profile.map(|(profile, backend, capability)| {
        let binding_signature = runtime_binding_signature(profile.runtime_binding.as_ref())
            .unwrap_or_else(|| "runtime".to_string());
        format!(
            "{}:{}:{}:{}",
            profile.id, backend, capability, binding_signature
        )
    })
}

fn resolve_start_profile(
    manifest: &AiPluginManifest,
    plugin_id: &str,
    request: Option<&AiPluginStartRequest>,
) -> Result<Option<(PluginInstallProfile, String, String)>, String> {
    if let Some(request) = request {
        if let Some(profile_id) = request
            .profile_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let profile = find_manifest_profile(manifest, plugin_id, profile_id)?;
            let runtime_binding = selected_runtime_binding(
                profile,
                request.runtime_binding_id.as_deref(),
                request.runtime_binding.clone(),
            )?;
            let effective_profile = profile_with_runtime_binding(profile, runtime_binding);
            let backend = request
                .backend
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(effective_profile.backend.as_str())
                .to_string();
            let capability = setup_capability(manifest, request.capability.clone())?;
            return Ok(Some((effective_profile, backend, capability)));
        }
    }

    let registry = load_registry()?;
    let Some(saved_state) = preferred_profile_state_for_plugin(&registry, plugin_id) else {
        return Ok(None);
    };
    let profile = find_manifest_profile(manifest, plugin_id, &saved_state.profile_id)?;
    let runtime_binding = selected_runtime_binding(
        profile,
        saved_state
            .runtime_binding
            .as_ref()
            .and_then(|binding| binding.id.as_deref()),
        saved_state.runtime_binding.clone(),
    )?;
    let effective_profile = profile_with_runtime_binding(profile, runtime_binding);
    let backend = if saved_state.backend.trim().is_empty() {
        effective_profile.backend.clone()
    } else {
        saved_state.backend.clone()
    };
    let capability = if saved_state.capability.trim().is_empty() {
        default_smoke_capability(manifest)
    } else {
        saved_state.capability.clone()
    };
    Ok(Some((effective_profile, backend, capability)))
}

fn build_setup_preview(
    plugin_id: &str,
    root: &Path,
    command: &str,
    profile: &PluginInstallProfile,
    backend: &str,
    capability: &str,
    runtime_binding: Option<PluginRuntimeBinding>,
    model_bindings: &[PluginModelBinding],
) -> AiPluginSetupPreview {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    if !is_safe_relative_command(command) {
        errors.push("setup command must be a safe relative path".to_string());
    }
    let command_path = root.join(command);
    if !command_path.exists() {
        errors.push(format!("setup command '{}' does not exist", command));
    }

    let (env_dir, env_path) = if let Some(env_dir) = profile
        .env_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if !safe_profile_relative_path(env_dir) {
            errors.push(format!(
                "Install profile '{}' has unsafe envDir '{}'",
                profile.id, env_dir
            ));
        }
        let env_path = match profile_runtime_dir(plugin_id, profile) {
            Ok(Some(path)) => path,
            Ok(None) => root.join(env_dir),
            Err(error) => {
                errors.push(error);
                root.join(env_dir)
            }
        };
        if env_path.exists() && !env_path.is_dir() {
            errors.push(format!(
                "profile envDir '{}' exists but is not a directory",
                env_path.display()
            ));
        } else if !env_path.exists() {
            warnings.push(format!(
                "profile envDir '{}' will be created",
                env_path.display()
            ));
        }
        (
            Some(env_dir.to_string()),
            Some(env_path.display().to_string()),
        )
    } else {
        warnings.push("profile does not declare envDir".to_string());
        (None, None)
    };

    if let Some(binding) = runtime_binding.as_ref() {
        match binding.scope.as_str() {
            "external" => {
                if let Some(python) = binding
                    .python
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                {
                    if !Path::new(python).is_file() {
                        warnings.push(format!(
                            "external runtime python '{}' was not found",
                            python
                        ));
                    }
                } else {
                    warnings.push("external runtime binding does not declare python".to_string());
                }
            }
            "shared" | "plugin" => {}
            "" => warnings.push("runtime binding is missing scope".to_string()),
            other => warnings.push(format!("runtime binding uses unknown scope '{}'", other)),
        }
    }

    let (requirements, requirements_path) =
        if let Some(requirements) = profile_requirements(profile) {
            if !safe_profile_relative_path(requirements) {
                errors.push(format!(
                    "Install profile '{}' has unsafe requirements path '{}'",
                    profile.id, requirements
                ));
            }
            let requirements_path = root.join(requirements);
            if !requirements_path.is_file() {
                warnings.push(format!(
                    "requirements file '{}' is not present yet",
                    requirements_path.display()
                ));
            }
            (
                Some(requirements.to_string()),
                Some(requirements_path.display().to_string()),
            )
        } else {
            warnings.push("profile does not declare requirements".to_string());
            (None, None)
        };

    AiPluginSetupPreview {
        plugin_id: plugin_id.to_string(),
        profile_id: profile.id.clone(),
        backend: backend.to_string(),
        capability: capability.to_string(),
        command: command.to_string(),
        command_path: command_path.display().to_string(),
        working_dir: root.display().to_string(),
        env_dir,
        env_path,
        requirements,
        requirements_path,
        runtime_binding: runtime_binding.clone(),
        environment: match build_setup_environment(
            root,
            plugin_id,
            profile,
            backend,
            capability,
            model_bindings,
        ) {
            Ok(environment) => environment,
            Err(error) => {
                errors.push(error);
                HashMap::new()
            }
        },
        warnings,
        errors,
    }
}

async fn run_setup_command(
    root: &Path,
    command: &str,
    plugin_id: &str,
    profile: &PluginInstallProfile,
    backend: &str,
    capability: &str,
    job: &mut AiPluginSetupJob,
    cancel_state: Option<&SetupCancellationState>,
    model_bindings: &[PluginModelBinding],
) -> Result<SetupCommandOutcome, String> {
    if !is_safe_relative_command(command) {
        return Err("setup command must be a safe relative path".to_string());
    }

    let command_path = root.join(command);
    if !command_path.exists() {
        return Err(format!("setup command '{}' does not exist", command));
    }

    job.push_log(format!("Executing setup command: {}", command));
    job.push_log("Injected setup environment:".to_string());
    job.push_log(format!("PICAIPIC_PLUGIN_ID={}", plugin_id));
    job.push_log(format!("PICAIPIC_PLUGIN_PROFILE_ID={}", profile.id));
    job.push_log(format!("PICAIPIC_PLUGIN_BACKEND={}", backend));
    job.push_log(format!("PICAIPIC_PLUGIN_CAPABILITY={}", capability));
    let environment = build_setup_environment(
        root,
        plugin_id,
        profile,
        backend,
        capability,
        model_bindings,
    )?;
    for key in [
        "PICAIPIC_PLUGIN_ID",
        "PICAIPIC_PLUGIN_PROFILE_ID",
        "PICAIPIC_PLUGIN_BACKEND",
        "PICAIPIC_PLUGIN_CAPABILITY",
        "PICAIPIC_PLUGIN_ROOT",
        "PICAIPIC_PLUGIN_DATA_DIR",
        "PICAIPIC_PLUGIN_CACHE_DIR",
        "PICAIPIC_PLUGIN_LOG_DIR",
        "PICAIPIC_PLUGIN_MODEL_DIR",
        "PICAIPIC_PLUGIN_CONFIG_PATH",
        "PICAIPIC_PLUGIN_RUNTIME_DIR",
        "PICAIPIC_PLUGIN_ENV_DIR",
        "PICAIPIC_PLUGIN_ENV_PATH",
        "PICAIPIC_PLUGIN_REQUIREMENTS",
        "PICAIPIC_PLUGIN_REQUIREMENTS_PATH",
    ] {
        if let Some(value) = environment.get(key) {
            job.push_log(format!("{}={}", key, value));
        }
    }
    append_setup_log(plugin_id, job)?;

    let mut cmd = Command::new(&command_path);
    cmd.current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Phase 5: clear ambient secrets first, then re-inject setup env map.
    let hygiene = t_sandbox::apply_env_hygiene(&mut cmd);
    if hygiene.applied {
        job.push_log(format!("env_hygiene: {}", hygiene.summary()));
        let _ = append_setup_log(plugin_id, job);
    }
    for (key, value) in &environment {
        cmd.env(key, value);
    }
    hide_command_window(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to execute setup command '{}': {}", command, e))?;

    job.status = "running".to_string();
    job.message = Some("Setup command is running...".to_string());
    job.updated_at = Utc::now().to_rfc3339();
    save_setup_job(job.clone())?;

    let mut stdout_reader = child
        .stdout
        .take()
        .map(|stdout| tokio::io::BufReader::new(stdout).lines());
    let mut stderr_reader = child
        .stderr
        .take()
        .map(|stderr| tokio::io::BufReader::new(stderr).lines());

    let job_id = job.id.clone();
    let mut line_counter: u32 = 0;
    let mut stdout_done = stdout_reader.is_none();
    let mut stderr_done = stderr_reader.is_none();

    while !stdout_done || !stderr_done {
        tokio::select! {
            stdout_line = async {
                match stdout_reader.as_mut() {
                    Some(reader) if !stdout_done => reader.next_line().await,
                    _ => std::future::pending().await,
                }
            } => match stdout_line {
                Ok(Some(line)) => {
                    job.push_log(line);
                    line_counter += 1;
                }
                Ok(None) => {
                    stdout_done = true;
                }
                Err(error) => {
                    return Err(format!("Failed to read setup stdout: {}", error));
                }
            },
            stderr_line = async {
                match stderr_reader.as_mut() {
                    Some(reader) if !stderr_done => reader.next_line().await,
                    _ => std::future::pending().await,
                }
            } => match stderr_line {
                Ok(Some(line)) => {
                    job.push_log(line);
                    line_counter += 1;
                }
                Ok(None) => {
                    stderr_done = true;
                }
                Err(error) => {
                    return Err(format!("Failed to read setup stderr: {}", error));
                }
            },
            _ = async {
                if let Some(cancel_state) = cancel_state {
                    loop {
                        if cancel_state.is_cancelled(&job_id).await {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                job.status = "cancelled".to_string();
                job.progress = 100;
                job.message = Some("Setup command was cancelled.".to_string());
                job.push_log("Setup command cancelled by user.".to_string());
                job.updated_at = Utc::now().to_rfc3339();
                if let Some(cancel_state) = cancel_state {
                    cancel_state.clear(&job_id).await;
                }
                append_setup_log(plugin_id, job)?;
                save_setup_job(job.clone())?;
                return Ok(SetupCommandOutcome::Cancelled);
            }
        }

        if line_counter > 0 && line_counter % 5 == 0 {
            job.updated_at = Utc::now().to_rfc3339();
            save_setup_job(job.clone())?;
            line_counter = 0;
        }
    }

    tokio::select! {
        wait_result = child.wait() => {
            let status = wait_result
                .map_err(|e| format!("Failed to wait for setup command: {}", e))?;
            if let Some(cancel_state) = cancel_state {
                if cancel_state.is_cancelled(&job_id).await {
                    job.status = "cancelled".to_string();
                    job.progress = 100;
                    job.message = Some("Setup command was cancelled.".to_string());
                    job.push_log("Setup command cancelled by user.".to_string());
                    job.updated_at = Utc::now().to_rfc3339();
                    cancel_state.clear(&job_id).await;
                    append_setup_log(plugin_id, job)?;
                    save_setup_job(job.clone())?;
                    return Ok(SetupCommandOutcome::Cancelled);
                }
            }
            if !status.success() {
                job.push_log(format!("Setup command exited with {}", status));
                return Err(format!(
                    "setup command '{}' exited with {}",
                    command, status
                ));
            }
        },
        _ = async {
            if let Some(cancel_state) = cancel_state {
                loop {
                    if cancel_state.is_cancelled(&job_id).await {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            } else {
                std::future::pending::<()>().await;
            }
        } => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            job.status = "cancelled".to_string();
            job.progress = 100;
            job.message = Some("Setup command was cancelled.".to_string());
            job.push_log("Setup command cancelled by user.".to_string());
            job.updated_at = Utc::now().to_rfc3339();
            if let Some(cancel_state) = cancel_state {
                cancel_state.clear(&job_id).await;
            }
            append_setup_log(plugin_id, job)?;
            save_setup_job(job.clone())?;
            return Ok(SetupCommandOutcome::Cancelled);
        }
    }

    // Save final streamed logs before the caller marks the job outcome.
    job.updated_at = Utc::now().to_rfc3339();
    append_setup_log(plugin_id, job)?;
    save_setup_job(job.clone())?;
    Ok(SetupCommandOutcome::Completed)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn is_manifest_path(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(MANIFEST_FILE_NAME)
}

fn push_manifest_if_exists(
    manifests: &mut Vec<PathBuf>,
    seen: &mut HashSet<String>,
    path: PathBuf,
) {
    let manifest_path = if path.is_file() && is_manifest_path(&path) {
        path
    } else if path.is_dir() {
        path.join(MANIFEST_FILE_NAME)
    } else {
        return;
    };

    if !manifest_path.exists() {
        return;
    }

    let key = normalize_path(&manifest_path);
    if seen.insert(key) {
        manifests.push(manifest_path);
    }
}

fn collect_child_manifests(root: &Path, manifests: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
    if !root.exists() || !root.is_dir() {
        return;
    }

    if root.join(MANIFEST_FILE_NAME).exists() {
        push_manifest_if_exists(manifests, seen, root.to_path_buf());
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Interrupted install/uninstall transactions keep recoverable hidden
        // directories here. They are never discoverable plugins.
        let hidden_transaction_dir = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| {
                name.starts_with(".installing-")
                    || name.starts_with(".replacing-")
                    || name.starts_with(".uninstalling-")
                    || name.starts_with(".package-snapshot-")
            })
            .unwrap_or(false);
        if path.is_dir() && !hidden_transaction_dir {
            push_manifest_if_exists(manifests, seen, path);
        }
    }
}

fn env_registered_paths() -> Vec<PathBuf> {
    let Some(value) = std::env::var_os("PICAIPIC_PLUGIN_PATHS") else {
        return Vec::new();
    };

    std::env::split_paths(&value).collect()
}

fn discover_manifest_paths() -> Result<Vec<PathBuf>, String> {
    let registry = load_registry()?;
    let mut manifests = Vec::new();
    let mut seen = HashSet::new();

    collect_child_manifests(&plugin_home_dir()?, &mut manifests, &mut seen);
    if let Some(program_data) = program_data_plugin_dir() {
        collect_child_manifests(&program_data, &mut manifests, &mut seen);
    }

    for path in registry.registered_paths {
        let path = PathBuf::from(path);
        if path.is_dir() && !path.join(MANIFEST_FILE_NAME).exists() {
            collect_child_manifests(&path, &mut manifests, &mut seen);
        } else {
            push_manifest_if_exists(&mut manifests, &mut seen, path);
        }
    }

    for path in env_registered_paths() {
        if path.is_dir() && !path.join(MANIFEST_FILE_NAME).exists() {
            collect_child_manifests(&path, &mut manifests, &mut seen);
        } else {
            push_manifest_if_exists(&mut manifests, &mut seen, path);
        }
    }

    Ok(manifests)
}

fn read_manifest(path: &Path) -> Result<AiPluginManifest, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read manifest '{}': {}", path.display(), e))?;
    serde_json::from_str::<AiPluginManifest>(&content)
        .map_err(|e| format!("Failed to parse manifest '{}': {}", path.display(), e))
}

fn current_platform() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        std::env::consts::OS
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        std::env::consts::ARCH
    };

    format!("{}-{}", os, arch)
}

fn current_os() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

fn current_arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "x64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "arm64".to_string()
    } else {
        std::env::consts::ARCH.to_string()
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|item| item == value) {
        values.push(value.to_string());
    }
}

fn gpu_vendor_from_text(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("rtx") {
        "nvidia".to_string()
    } else if lower.contains("amd")
        || lower.contains("advanced micro devices")
        || lower.contains("radeon")
    {
        "amd".to_string()
    } else if lower.contains("intel") || lower.contains("arc") || lower.contains("iris") {
        "intel".to_string()
    } else if lower.contains("apple") {
        "apple".to_string()
    } else {
        "unknown".to_string()
    }
}

fn backend_candidates_for_vendor(vendor: &str) -> Vec<String> {
    let mut backends = Vec::new();
    match vendor {
        "nvidia" => {
            push_unique(&mut backends, "cuda");
        }
        "amd" => {
            if cfg!(target_os = "windows") || cfg!(target_os = "linux") {
                push_unique(&mut backends, "rocm");
            }
            if cfg!(target_os = "windows") {
                push_unique(&mut backends, "directml");
            }
        }
        "intel" => {
            push_unique(&mut backends, "openvino");
            if cfg!(target_os = "windows") {
                push_unique(&mut backends, "directml");
            }
        }
        "apple" => {
            if cfg!(target_os = "macos") {
                push_unique(&mut backends, "mps");
            }
        }
        _ => {}
    }
    push_unique(&mut backends, "cpu");
    backends
}

#[cfg(target_os = "windows")]
fn detect_host_gpus() -> Result<Vec<AiPluginHostGpu>, String> {
    let script = "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterCompatibility | ConvertTo-Json -Compress";
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", script])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to probe GPU with PowerShell: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "GPU probe command failed".to_string()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    let value: Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse GPU probe output: {}", e))?;
    let items: Vec<Value> = match value {
        Value::Array(items) => items,
        Value::Object(_) => vec![value],
        _ => Vec::new(),
    };

    let mut gpus = Vec::new();
    for item in items {
        let name = item
            .get("Name")
            .and_then(Value::as_str)
            .unwrap_or("Unknown GPU")
            .trim()
            .to_string();
        let adapter = item
            .get("AdapterCompatibility")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let vendor = gpu_vendor_from_text(&format!("{} {}", name, adapter));
        gpus.push(AiPluginHostGpu {
            name,
            vendor: vendor.clone(),
            backend_candidates: backend_candidates_for_vendor(&vendor),
        });
    }
    Ok(gpus)
}

#[cfg(target_os = "macos")]
fn detect_host_gpus() -> Result<Vec<AiPluginHostGpu>, String> {
    Ok(vec![AiPluginHostGpu {
        name: "Apple GPU".to_string(),
        vendor: "apple".to_string(),
        backend_candidates: backend_candidates_for_vendor("apple"),
    }])
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn detect_host_gpus() -> Result<Vec<AiPluginHostGpu>, String> {
    Ok(Vec::new())
}

fn python_runtime_from_path(
    id: String,
    label: String,
    scope: String,
    python: String,
    root: Option<String>,
    source: String,
) -> AiPluginPythonRuntime {
    let mut cmd = std::process::Command::new(&python);
    cmd.arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let version = if !stdout.is_empty() {
                stdout
            } else {
                stderr.clone()
            };
            AiPluginPythonRuntime {
                id,
                label,
                scope,
                python,
                root,
                source,
                version: if version.is_empty() {
                    None
                } else {
                    Some(version)
                },
                available: output.status.success(),
                error: if output.status.success() {
                    None
                } else if stderr.is_empty() {
                    Some(format!("Python probe exited with {}", output.status))
                } else {
                    Some(stderr)
                },
            }
        }
        Err(error) => AiPluginPythonRuntime {
            id,
            label,
            scope,
            python,
            root,
            source,
            version: None,
            available: false,
            error: Some(error.to_string()),
        },
    }
}

fn discover_path_python_runtimes() -> Vec<AiPluginPythonRuntime> {
    let mut candidates = Vec::new();
    if cfg!(target_os = "windows") {
        candidates.push(("python".to_string(), "PATH python".to_string()));
        candidates.push(("py".to_string(), "Python launcher".to_string()));
    } else {
        candidates.push(("python3".to_string(), "PATH python3".to_string()));
        candidates.push(("python".to_string(), "PATH python".to_string()));
    }

    candidates
        .into_iter()
        .map(|(python, label)| {
            python_runtime_from_path(
                format!("path:{}", python),
                label,
                "external".to_string(),
                python,
                None,
                "path".to_string(),
            )
        })
        .collect()
}

fn python_executable_in_env(env_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        env_dir.join("Scripts").join("python.exe")
    } else {
        env_dir.join("bin").join("python")
    }
}

fn push_python_candidate(
    candidates: &mut Vec<(String, String, String, String, Option<String>, String)>,
    seen: &mut HashSet<String>,
    id: String,
    label: String,
    scope: String,
    python: PathBuf,
    root: Option<PathBuf>,
    source: String,
) {
    if candidates.len() >= PYTHON_RUNTIME_DISCOVERY_LIMIT {
        return;
    }
    if !python.is_file() {
        return;
    }
    let normalized = normalize_path(&python);
    if seen.insert(normalized.clone().to_lowercase()) {
        candidates.push((
            id,
            label,
            scope,
            normalized,
            root.map(|path| normalize_path(&path)),
            source,
        ));
    }
}

fn common_python_env_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    let mut push_root = |path: PathBuf| {
        let key = normalize_path(&path).to_lowercase();
        if seen.insert(key) {
            roots.push(path);
        }
    };

    if let Ok(cwd) = std::env::current_dir() {
        push_root(cwd);
    }
    if let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    {
        push_root(home.clone());
        push_root(home.join(".virtualenvs"));
        push_root(home.join("venvs"));
        push_root(home.join("miniconda3").join("envs"));
        push_root(home.join("anaconda3").join("envs"));
        push_root(home.join("mambaforge").join("envs"));
        push_root(home.join("micromamba").join("envs"));
    }

    if cfg!(target_os = "windows") {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            push_root(
                local_app_data
                    .join("pypoetry")
                    .join("Cache")
                    .join("virtualenvs"),
            );
        }
        if let Some(program_data) = std::env::var_os("PROGRAMDATA").map(PathBuf::from) {
            push_root(program_data.join("Anaconda3").join("envs"));
            push_root(program_data.join("Miniconda3").join("envs"));
            push_root(program_data.join("mambaforge").join("envs"));
        }
        push_root(PathBuf::from(r"C:\ProgramData\Anaconda3\envs"));
        push_root(PathBuf::from(r"C:\ProgramData\Miniconda3\envs"));
    }

    roots
}

fn discover_common_venv_python_runtimes() -> Vec<AiPluginPythonRuntime> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    let Ok(manifests) = discover_manifest_paths() else {
        return Vec::new();
    };
    for manifest_path in manifests {
        if candidates.len() >= PYTHON_RUNTIME_DISCOVERY_LIMIT {
            break;
        }
        let Some(plugin_root) = manifest_path.parent() else {
            continue;
        };
        for env_name in [".venv", "venv", "env"] {
            let env_dir = plugin_root.join(env_name);
            let python = python_executable_in_env(&env_dir);
            push_python_candidate(
                &mut candidates,
                &mut seen,
                format!("venv:{}", normalize_path(&env_dir)),
                format!("Plugin {} Python", env_name),
                "plugin".to_string(),
                python,
                Some(env_dir),
                "common-venv".to_string(),
            );
        }
    }

    candidates
        .into_iter()
        .map(|(id, label, scope, python, root, source)| {
            python_runtime_from_path(id, label, scope, python, root, source)
        })
        .collect()
}

fn discover_conda_python_runtimes() -> Vec<AiPluginPythonRuntime> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for env_root in common_python_env_roots() {
        if candidates.len() >= PYTHON_RUNTIME_DISCOVERY_LIMIT {
            break;
        }
        let Ok(entries) = fs::read_dir(&env_root) else {
            continue;
        };
        for entry in entries.flatten() {
            if candidates.len() >= PYTHON_RUNTIME_DISCOVERY_LIMIT {
                break;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let env_dir = entry.path();
            let python = python_executable_in_env(&env_dir);
            let env_name = env_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Python environment")
                .to_string();
            push_python_candidate(
                &mut candidates,
                &mut seen,
                format!("conda:{}", normalize_path(&env_dir)),
                format!("Conda/venv {}", env_name),
                "external".to_string(),
                python,
                Some(env_dir),
                "conda-or-venv".to_string(),
            );
        }
    }

    candidates
        .into_iter()
        .map(|(id, label, scope, python, root, source)| {
            python_runtime_from_path(id, label, scope, python, root, source)
        })
        .collect()
}

fn discover_manifest_python_runtimes() -> Vec<AiPluginPythonRuntime> {
    let Ok(manifests) = discover_manifest_paths() else {
        return Vec::new();
    };
    let mut runtimes = Vec::new();
    let mut seen = HashSet::new();
    for manifest_path in manifests {
        let Ok(manifest) = read_manifest(&manifest_path) else {
            continue;
        };
        for profile in &manifest.install_profiles {
            for binding in profile_runtime_bindings(profile) {
                if binding.scope != "external" {
                    continue;
                }
                let Some(python) = binding
                    .python
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                else {
                    continue;
                };
                if !seen.insert(python.clone()) {
                    continue;
                }
                let id = binding
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("external:{}", python));
                let label = binding
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("{} external Python", manifest.name));
                runtimes.push(python_runtime_from_path(
                    id,
                    label,
                    binding.scope,
                    python,
                    binding.root.clone(),
                    format!("manifest:{}", manifest.id),
                ));
            }
        }
    }
    runtimes
}

fn discover_python_runtimes() -> Vec<AiPluginPythonRuntime> {
    let mut runtimes = Vec::new();
    let mut seen = HashSet::new();
    for runtime in discover_manifest_python_runtimes()
        .into_iter()
        .chain(discover_common_venv_python_runtimes())
        .chain(discover_conda_python_runtimes())
        .chain(discover_path_python_runtimes())
    {
        let key = runtime.python.to_lowercase();
        if seen.insert(key) && runtimes.len() < PYTHON_RUNTIME_DISCOVERY_LIMIT {
            runtimes.push(runtime);
        }
    }
    runtimes
}

fn python_runtime_probe_script() -> &'static str {
    r#"
import importlib
import json
import platform
import sys
import time

requested_backend = (sys.argv[1] if len(sys.argv) > 1 else "").lower()

def package_version(name):
    try:
        module = importlib.import_module(name)
        return {
            "available": True,
            "version": getattr(module, "__version__", None),
        }
    except Exception as exc:
        return {
            "available": False,
            "error": str(exc),
        }

started = time.perf_counter()
result = {
    "python": {
        "executable": sys.executable,
        "version": platform.python_version(),
        "platform": platform.platform(),
    },
    "requestedBackend": requested_backend or None,
    "packages": {},
    "backends": {
        "cpu": {"available": True},
    },
}

torch_info = package_version("torch")
result["packages"]["torch"] = torch_info
if torch_info.get("available"):
    import torch
    torch_backend = {
        "available": True,
        "version": getattr(torch, "__version__", None),
        "cudaAvailable": bool(torch.cuda.is_available()),
        "cudaDeviceCount": int(torch.cuda.device_count()) if torch.cuda.is_available() else 0,
        "cudaVersion": getattr(torch.version, "cuda", None),
        "hipVersion": getattr(torch.version, "hip", None),
        "mpsAvailable": bool(getattr(getattr(torch.backends, "mps", None), "is_available", lambda: False)()),
    }
    result["torch"] = torch_backend
    result["backends"]["cuda"] = {
        "available": bool(torch_backend["cudaAvailable"] and not torch_backend.get("hipVersion")),
        "deviceCount": torch_backend["cudaDeviceCount"],
        "version": torch_backend.get("cudaVersion"),
    }
    result["backends"]["rocm"] = {
        "available": bool(torch_backend["cudaAvailable"] and torch_backend.get("hipVersion")),
        "deviceCount": torch_backend["cudaDeviceCount"],
        "version": torch_backend.get("hipVersion"),
    }
    result["backends"]["mps"] = {
        "available": torch_backend["mpsAvailable"],
    }
    if requested_backend in ("cuda", "rocm") and torch.cuda.is_available():
        try:
            tensor = torch.ones((1,), device="cuda")
            result["backends"][requested_backend]["probe"] = {
                "ok": bool((tensor + 1).detach().cpu().item() == 2),
            }
        except Exception as exc:
            result["backends"][requested_backend]["probe"] = {
                "ok": False,
                "error": str(exc),
            }
            result["backends"][requested_backend]["available"] = False

directml_info = package_version("torch_directml")
result["packages"]["torchDirectML"] = directml_info
result["backends"]["directml"] = {
    "available": bool(directml_info.get("available")),
}
if requested_backend == "directml" and directml_info.get("available"):
    try:
        import torch_directml
        result["backends"]["directml"]["device"] = str(torch_directml.device())
    except Exception as exc:
        result["backends"]["directml"]["probe"] = {
            "ok": False,
            "error": str(exc),
        }
        result["backends"]["directml"]["available"] = False

onnx_info = package_version("onnxruntime")
result["packages"]["onnxruntime"] = onnx_info
if onnx_info.get("available"):
    try:
        import onnxruntime
        result["onnxruntime"] = {
            "available": True,
            "version": getattr(onnxruntime, "__version__", None),
            "providers": list(onnxruntime.get_available_providers()),
        }
        providers = set(result["onnxruntime"]["providers"])
        result["backends"]["openvino"] = {
            "available": "OpenVINOExecutionProvider" in providers,
        }
        if "DmlExecutionProvider" in providers:
            result["backends"]["directml"]["available"] = True
    except Exception as exc:
        result["onnxruntime"] = {
            "available": False,
            "error": str(exc),
        }

result["packages"]["numpy"] = package_version("numpy")
result["packages"]["opencv-python"] = package_version("cv2")
result["packages"]["rawpy"] = package_version("rawpy")

result["elapsedMs"] = int((time.perf_counter() - started) * 1000)
print(json.dumps(result, ensure_ascii=False))
"#
}

async fn probe_python_runtime(
    request: AiPluginPythonRuntimeProbeRequest,
) -> Result<AiPluginPythonRuntimeProbeResult, String> {
    let python = request.python.trim().to_string();
    if python.is_empty() {
        return Err("Python runtime probe requires a python path".to_string());
    }

    let started = std::time::Instant::now();
    let mut cmd = Command::new(&python);
    cmd.arg("-c")
        .arg(python_runtime_probe_script())
        .arg(request.backend.clone().unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_command_window(&mut cmd);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start Python runtime probe: {}", e))?;
    let output = match tokio::time::timeout(
        Duration::from_millis(PYTHON_RUNTIME_PROBE_TIMEOUT_MS),
        child.wait_with_output(),
    )
    .await
    {
        Ok(result) => result.map_err(|e| format!("Python runtime probe failed: {}", e))?,
        Err(_) => {
            let error = Some(format!(
                "Python runtime probe timed out after {}ms",
                PYTHON_RUNTIME_PROBE_TIMEOUT_MS
            ));
            let state = maybe_persist_runtime_probe_state(
                &request,
                false,
                started.elapsed().as_millis() as u64,
                None,
                error.clone(),
            )?;
            return Ok(AiPluginPythonRuntimeProbeResult {
                python,
                backend: request.backend,
                available: false,
                duration_ms: started.elapsed().as_millis(),
                result: None,
                error,
                state,
            });
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let result = parse_optional_json_object_from_process_stdout(&stdout);
        let error = Some(if stderr.is_empty() {
            format!("Python runtime probe exited with {}", output.status)
        } else {
            stderr
        });
        let state = maybe_persist_runtime_probe_state(
            &request,
            false,
            started.elapsed().as_millis() as u64,
            result.clone(),
            error.clone(),
        )?;
        return Ok(AiPluginPythonRuntimeProbeResult {
            python,
            backend: request.backend,
            available: false,
            duration_ms: started.elapsed().as_millis(),
            result,
            error,
            state,
        });
    }

    let result = parse_json_object_from_process_stdout(&stdout)
        .map_err(|e| format!("Failed to parse Python runtime probe output: {}", e))?;
    let backend = request
        .backend
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase());
    let backend_available = backend.as_deref().map_or(true, |backend| {
        result
            .get("backends")
            .and_then(|backends| backends.get(backend))
            .and_then(|backend| backend.get("available"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    });

    let state = maybe_persist_runtime_probe_state(
        &request,
        backend_available,
        started.elapsed().as_millis() as u64,
        Some(result.clone()),
        None,
    )?;

    Ok(AiPluginPythonRuntimeProbeResult {
        python,
        backend: request.backend,
        available: backend_available,
        duration_ms: started.elapsed().as_millis(),
        result: Some(result),
        error: None,
        state,
    })
}

fn build_host_environment() -> AiPluginHostEnvironment {
    let os = current_os();
    let arch = current_arch();
    let platform = format!("{}-{}", os, arch);
    let mut probe_error = None;
    let gpus = match detect_host_gpus() {
        Ok(gpus) => gpus,
        Err(error) => {
            probe_error = Some(error);
            Vec::new()
        }
    };

    let mut candidate_backends = Vec::new();
    for gpu in &gpus {
        for backend in &gpu.backend_candidates {
            push_unique(&mut candidate_backends, backend);
        }
    }

    if gpus.is_empty() {
        if cfg!(target_os = "macos") {
            push_unique(&mut candidate_backends, "mps");
        }
        push_unique(&mut candidate_backends, "cpu");
    }

    AiPluginHostEnvironment {
        os,
        arch,
        platform,
        gpus,
        candidate_backends,
        python_runtimes: discover_python_runtimes(),
        probe_error,
    }
}

fn is_safe_relative_command(command: &str) -> bool {
    let path = Path::new(command);
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
}

/// A POSIX-style env var name: starts with letter or underscore, followed by
/// letters, digits, or underscores. Used to validate `modelBindings[].envVar`.
fn is_valid_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn plugin_api_major_compatible(plugin_api: &str) -> bool {
    let trimmed = plugin_api.trim();
    if trimmed.is_empty() {
        return true;
    }

    if let Some(rest) = trimmed.strip_prefix('^') {
        return rest
            .split('.')
            .next()
            .and_then(|part| part.parse::<i64>().ok())
            == Some(PLUGIN_API_MAJOR);
    }

    trimmed
        .split('.')
        .next()
        .and_then(|part| part.parse::<i64>().ok())
        == Some(PLUGIN_API_MAJOR)
}

fn parse_app_version(version: &str) -> Option<Vec<u64>> {
    let core = version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()?;
    if core.is_empty() {
        return None;
    }
    let parts = core
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then_some(parts)
}

fn compare_app_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = parse_app_version(left)?;
    let right = parse_app_version(right)?;
    let width = left.len().max(right.len());
    for index in 0..width {
        let ordering = left
            .get(index)
            .copied()
            .unwrap_or_default()
            .cmp(&right.get(index).copied().unwrap_or_default());
        if !ordering.is_eq() {
            return Some(ordering);
        }
    }
    Some(std::cmp::Ordering::Equal)
}

fn validate_manifest(manifest: &AiPluginManifest, root: &Path) -> PluginValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if manifest.schema_version != 1 {
        errors.push(format!(
            "Unsupported schemaVersion {}; expected 1",
            manifest.schema_version
        ));
    }

    if manifest.id.trim().is_empty() {
        errors.push("Missing plugin id".to_string());
    } else if !manifest
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
    {
        errors.push("Plugin id must use lowercase letters, numbers, dots, or hyphens".to_string());
    }

    if manifest.name.trim().is_empty() {
        errors.push("Missing plugin name".to_string());
    }

    if manifest.version.trim().is_empty() {
        errors.push("Missing plugin version".to_string());
    }

    if manifest.platforms.is_empty() {
        warnings.push("No platforms declared".to_string());
    } else if !platform_supported(manifest) {
        errors.push(format!(
            "Current platform '{}' is not declared in manifest",
            current_platform()
        ));
    }

    if let Some(compatibility) = &manifest.compatibility {
        let host_version = env!("CARGO_PKG_VERSION");
        if let Some(min_version) = compatibility.min_pic_ai_pic_version.as_deref() {
            match compare_app_versions(host_version, min_version) {
                Some(std::cmp::Ordering::Less) => errors.push(format!(
                    "Plugin requires PicAiPic {} or newer; current host is {}",
                    min_version, host_version
                )),
                Some(_) => {}
                None => errors.push(format!(
                    "Invalid compatibility.minPicAiPicVersion '{}'",
                    min_version
                )),
            }
        }
        if let Some(max_version) = compatibility.max_pic_ai_pic_version.as_deref() {
            match compare_app_versions(host_version, max_version) {
                Some(std::cmp::Ordering::Greater) => errors.push(format!(
                    "Plugin supports PicAiPic up to {}; current host is {}",
                    max_version, host_version
                )),
                Some(_) => {}
                None => errors.push(format!(
                    "Invalid compatibility.maxPicAiPicVersion '{}'",
                    max_version
                )),
            }
        }
        if let Some(plugin_api) = &compatibility.plugin_api {
            if !plugin_api_major_compatible(plugin_api) {
                errors.push(format!(
                    "Unsupported pluginApi '{}'; expected major version {}",
                    plugin_api, PLUGIN_API_MAJOR
                ));
            }
        }
    }

    let Some(entry) = &manifest.entry else {
        errors.push("Missing entry".to_string());
        return PluginValidationReport {
            valid: errors.is_empty(),
            errors,
            warnings,
        };
    };

    match entry.kind.as_str() {
        "local-http" => {
            let Some(base_url) = &entry.base_url else {
                errors.push("local-http entry requires baseUrl".to_string());
                return PluginValidationReport {
                    valid: errors.is_empty(),
                    errors,
                    warnings,
                };
            };

            if !(base_url.starts_with("http://127.0.0.1")
                || base_url.starts_with("http://localhost"))
            {
                errors.push("local-http baseUrl must bind to loopback by default".to_string());
            }

            if let Some(command) = &entry.start_command {
                if !is_safe_relative_command(command) {
                    errors.push("startCommand must be a safe relative path".to_string());
                } else if !root.join(command).exists() {
                    warnings.push(format!("startCommand '{}' does not exist yet", command));
                }
            }

            if let Some(command) = &entry.stop_command {
                if !is_safe_relative_command(command) {
                    errors.push("stopCommand must be a safe relative path".to_string());
                }
            }
        }
        "local-command" => {
            let Some(command) = &entry.command else {
                errors.push("local-command entry requires command".to_string());
                return PluginValidationReport {
                    valid: errors.is_empty(),
                    errors,
                    warnings,
                };
            };

            if !is_safe_relative_command(command) {
                errors.push("command must be a safe relative path".to_string());
            } else if !root.join(command).exists() {
                warnings.push(format!("command '{}' does not exist yet", command));
            }
        }
        "" => errors.push("Missing entry kind".to_string()),
        other => errors.push(format!("Unsupported entry kind '{}'", other)),
    }

    if let Some(install) = &manifest.install {
        if install.kind.trim().is_empty() {
            warnings.push("Install entry is missing kind".to_string());
        }

        if let Some(command) = install.command.as_deref() {
            if !is_safe_relative_command(command) {
                errors.push("install command must be a safe relative path".to_string());
            } else if !root.join(command).exists() {
                warnings.push(format!("install command '{}' does not exist yet", command));
            }
        }
    }

    if let Some(runtime) = &manifest.runtime {
        if runtime.kind.trim().is_empty() {
            warnings.push("Runtime entry is missing kind".to_string());
        }
    }

    let mut install_profile_ids = HashSet::new();
    for profile in &manifest.install_profiles {
        if profile.id.trim().is_empty() {
            errors.push("Install profile is missing id".to_string());
        } else if !install_profile_ids.insert(profile.id.clone()) {
            errors.push(format!("Duplicate install profile id '{}'", profile.id));
        }

        if profile.backend.trim().is_empty() {
            errors.push(format!(
                "Install profile '{}' is missing backend",
                profile.id
            ));
        } else if !matches!(
            profile.backend.as_str(),
            "cuda" | "rocm" | "directml" | "openvino" | "mps" | "cpu"
        ) {
            warnings.push(format!(
                "Install profile '{}' uses unknown backend '{}'",
                profile.id, profile.backend
            ));
        }

        for binding in profile_runtime_bindings(profile) {
            match binding.scope.as_str() {
                "external" => {
                    if binding
                        .python
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                    {
                        warnings.push(format!(
                            "Install profile '{}' external runtime binding is missing python",
                            profile.id
                        ));
                    }
                }
                "shared" | "plugin" => {}
                "" => warnings.push(format!(
                    "Install profile '{}' runtime binding is missing scope",
                    profile.id
                )),
                other => warnings.push(format!(
                    "Install profile '{}' uses unknown runtime binding scope '{}'",
                    profile.id, other
                )),
            }
        }
    }

    if manifest.capabilities.is_empty() {
        warnings.push("No capabilities declared".to_string());
    }

    let mut capability_ids = HashSet::new();
    for capability in &manifest.capabilities {
        if capability.id.trim().is_empty() {
            errors.push("Capability is missing id".to_string());
        } else if !capability_ids.insert(capability.id.clone()) {
            errors.push(format!("Duplicate capability id '{}'", capability.id));
        }

        if capability.kind.trim().is_empty() {
            errors.push(format!("Capability '{}' is missing kind", capability.id));
        }

        if capability.name.trim().is_empty() {
            warnings.push(format!(
                "Capability '{}' is missing display name",
                capability.id
            ));
        }
    }

    if let Some(contributes) = &manifest.contributes {
        let mut menu_ids = HashSet::new();
        for menu in &contributes.menus {
            if menu.id.trim().is_empty() {
                errors.push("Menu contribution is missing id".to_string());
            } else if !menu_ids.insert(menu.id.clone()) {
                errors.push(format!("Duplicate menu contribution id '{}'", menu.id));
            }

            if menu.label.trim().is_empty() {
                warnings.push(format!(
                    "Menu contribution '{}' is missing display label",
                    menu.id
                ));
            }

            if menu.capability.trim().is_empty() {
                errors.push(format!(
                    "Menu contribution '{}' is missing capability",
                    menu.id
                ));
            } else if !capability_ids.contains(&menu.capability) {
                errors.push(format!(
                    "Menu contribution '{}' references unknown capability '{}'",
                    menu.id, menu.capability
                ));
            }

            if menu.contexts.is_empty() {
                warnings.push(format!("Menu contribution '{}' has no contexts", menu.id));
            }

            if menu.placements.is_empty() {
                warnings.push(format!("Menu contribution '{}' has no placements", menu.id));
            }
        }
    }

    let mut model_binding_ids = HashSet::new();
    for binding in &manifest.model_bindings {
        if binding.id.trim().is_empty() {
            warnings.push("Model binding is missing id".to_string());
        } else if !model_binding_ids.insert(binding.id.clone()) {
            warnings.push(format!("Duplicate model binding id '{}'", binding.id));
        }

        if binding.env_var.trim().is_empty() {
            warnings.push(format!("Model binding '{}' is missing envVar", binding.id));
        } else if !is_valid_env_var_name(&binding.env_var) {
            warnings.push(format!(
                "Model binding '{}' envVar '{}' is not a valid environment variable name",
                binding.id, binding.env_var
            ));
        }

        for extra in &binding.env_vars {
            if extra.trim().is_empty() {
                warnings.push(format!(
                    "Model binding '{}' has an empty extra envVar entry",
                    binding.id
                ));
            } else if !is_valid_env_var_name(extra) {
                warnings.push(format!(
                    "Model binding '{}' extra envVar '{}' is not a valid name",
                    binding.id, extra
                ));
            }
        }

        match binding.layout.as_str() {
            "" | "files" | "sourceTree" => {}
            other => warnings.push(format!(
                "Model binding '{}' uses unknown layout '{}'",
                binding.id, other
            )),
        }
    }

    PluginValidationReport {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

fn platform_supported(manifest: &AiPluginManifest) -> bool {
    if manifest.platforms.is_empty() {
        return true;
    }

    let current = current_platform();
    manifest
        .platforms
        .iter()
        .any(|platform| platform == &current || platform == "all")
}

fn plugin_storage_summary(plugin_id: &str, code_dir: &Path) -> AiPluginStorageSummary {
    let path_or_empty = |result: Result<PathBuf, String>| {
        result
            .map(|path| normalize_path(&path))
            .unwrap_or_else(|_| String::new())
    };
    let model_dirs = find_plugin_manifest(plugin_id)
        .and_then(|(_, manifest)| plugin_model_drop_dirs(&manifest))
        .unwrap_or_default()
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect();
    let runtime_dirs = find_plugin_manifest(plugin_id)
        .map(|(_, manifest)| plugin_runtime_display_dirs(plugin_id, &manifest))
        .unwrap_or_default()
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect();
    AiPluginStorageSummary {
        store_dir: path_or_empty(plugin_store_dir()),
        code_dir: normalize_path(code_dir),
        data_dir: path_or_empty(plugin_data_dir(plugin_id)),
        model_dir: path_or_empty(plugin_model_dir(plugin_id)),
        model_dirs,
        log_dir: path_or_empty(plugin_logs_dir(plugin_id)),
        config_path: path_or_empty(plugin_config_path(plugin_id)),
        runtime_dir: path_or_empty(plugin_runtime_root(plugin_id)),
        runtime_dirs,
        cache_dir: path_or_empty(plugin_cache_dir(plugin_id)),
        output_dir: path_or_empty(plugin_output_dir(plugin_id)),
    }
}

fn manifest_to_summary(
    manifest_path: &Path,
    manifest: AiPluginManifest,
    validation: PluginValidationReport,
    permission_grant: Option<AiPluginPermissionGrant>,
    profile_states: &HashMap<String, AiPluginProfileState>,
    setup_jobs: &HashMap<String, AiPluginSetupJob>,
    runtime_probe_states: &HashMap<String, AiPluginRuntimeProbeState>,
    task_states: &HashMap<String, AiPluginTaskState>,
) -> AiPluginSummary {
    let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));
    let platform_supported = platform_supported(&manifest);
    let plugin_id = manifest.id.clone();
    let storage = plugin_storage_summary(&plugin_id, root);
    let permissions = normalize_plugin_permissions(manifest.permissions.as_ref());
    let entry = manifest.entry.as_ref().map(|entry| PluginEntrySummary {
        kind: entry.kind.clone(),
        base_url: entry.base_url.clone(),
        start_command: entry
            .start_command
            .clone()
            .or_else(|| entry.command.clone()),
        status_path: entry.status.as_ref().map(|status| status.path.clone()),
        health_path: entry.health.as_ref().map(|health| health.path.clone()),
    });
    let install = manifest
        .install
        .as_ref()
        .map(|install| PluginInstallSummary {
            kind: install.kind.clone(),
            command: install.command.clone(),
            estimated_disk_mb: install.estimated_disk_mb,
            requires_admin: install.requires_admin.unwrap_or(false),
        });
    let runtime = manifest
        .runtime
        .as_ref()
        .map(|runtime| PluginRuntimeSummary {
            kind: runtime.kind.clone(),
            cuda_api_compatible: runtime.cuda_api_compatible.unwrap_or(false),
            notes: runtime.notes.clone(),
        });
    let install_profiles = manifest
        .install_profiles
        .iter()
        .map(|profile| {
            let state = profile_states
                .get(&profile_state_key(&plugin_id, &profile.id))
                .cloned();
            let setup_job = state
                .as_ref()
                .and_then(|state| state.setup_job_id.as_ref())
                .and_then(|job_id| setup_jobs.get(job_id))
                .cloned();
            // Prefer the user-selected/persisted binding (including synthetic
            // plugin-private fallbacks) so probe cards, path chips, and conflict
            // detection all follow the active runtime, not only the manifest default.
            let active_binding = state
                .as_ref()
                .and_then(|s| s.runtime_binding.clone())
                .or_else(|| profile.runtime_binding.clone());
            let effective_profile = profile_with_runtime_binding(profile, active_binding.clone());
            let runtime_probe_state = runtime_probe_states
                .get(&runtime_probe_key(
                    &plugin_id,
                    &profile.id,
                    &profile.backend,
                    active_binding.as_ref(),
                ))
                .cloned()
                .map(|state| {
                    mark_runtime_probe_staleness(
                        state,
                        runtime_probe_fingerprint(
                            root,
                            &effective_profile,
                            active_binding.as_ref(),
                        ),
                    )
                });
            // Collect probe states for all bindings of this plugin+profile
            let profile_prefix = format!("{}:{}:", plugin_id, profile.id);
            let runtime_probe_states: Vec<AiPluginRuntimeProbeState> = runtime_probe_states
                .iter()
                .filter(|(key, _)| key.starts_with(&profile_prefix))
                .map(|(_, state)| {
                    mark_runtime_probe_staleness(
                        state.clone(),
                        runtime_probe_fingerprint(
                            root,
                            &effective_profile,
                            state.runtime_binding.as_ref(),
                        ),
                    )
                })
                .collect();
            let effective_for_path = profile_with_runtime_binding(profile, active_binding.clone());
            let resolved_runtime_dir = (|| {
                let scope = active_binding
                    .as_ref()
                    .map(|b| b.scope.to_ascii_lowercase())
                    .unwrap_or_default();
                if scope == "external" {
                    if let Some(root) = active_binding
                        .as_ref()
                        .and_then(|b| b.root.clone())
                        .filter(|root| !root.trim().is_empty())
                    {
                        return Some(root);
                    }
                    return active_binding
                        .as_ref()
                        .and_then(|b| b.python.clone())
                        .and_then(|python| {
                            Path::new(&python)
                                .parent()
                                .map(|parent| parent.to_string_lossy().to_string())
                        });
                }
                if scope == "shared" {
                    if let Ok(dir) = profile_runtime_root(&plugin_id, &effective_for_path) {
                        return Some(dir.to_string_lossy().to_string());
                    }
                }
                if let Ok(Some(dir)) = profile_runtime_dir(&plugin_id, &effective_for_path) {
                    return Some(dir.to_string_lossy().to_string());
                }
                if let Ok(dir) = profile_runtime_root(&plugin_id, &effective_for_path) {
                    return Some(dir.to_string_lossy().to_string());
                }
                None
            })();
            // Detect version conflicts between declared requirements and the
            // versions reported by the probe. Only run when we have a
            // non-stale, passed probe so the comparison is against current
            // runtime state.
            let runtime_conflicts = (|| {
                let probe_state = runtime_probe_state.as_ref()?;
                if probe_state.stale || probe_state.status != "passed" {
                    return Some(Vec::new());
                }
                let requirements_rel = profile_requirements(profile)?;
                if !safe_profile_relative_path(requirements_rel) {
                    return Some(Vec::new());
                }
                let requirements_path = root.join(requirements_rel);
                if !requirements_path.is_file() {
                    return Some(Vec::new());
                }
                let probe_result = probe_state.result.as_ref()?;
                Some(detect_runtime_conflicts(&requirements_path, probe_result))
            })()
            .unwrap_or_default();
            let model_binding_checks =
                model_binding_summaries(&manifest, &profile.id).unwrap_or_default();
            // Surface a synthetic plugin-private option whenever the profile
            // has an envDir. Authors keep shared defaults; users can opt into
            // isolation after a shared conflict without editing the manifest.
            let mut runtime_bindings = profile.runtime_bindings.clone();
            if let Ok(private_binding) = plugin_private_runtime_binding(profile) {
                let private_id = private_binding.id.clone();
                let already_declared = runtime_bindings.iter().any(|binding| {
                    binding.scope.eq_ignore_ascii_case("plugin")
                        && (private_id.is_some() && binding.id == private_id
                            || private_id.is_none())
                }) || profile
                    .runtime_binding
                    .as_ref()
                    .map(|binding| {
                        binding.scope.eq_ignore_ascii_case("plugin")
                            && (private_id.is_some() && binding.id == private_id
                                || private_id.is_none())
                    })
                    .unwrap_or(false);
                if !already_declared {
                    runtime_bindings.push(private_binding);
                }
            }
            PluginInstallProfileSummary {
                id: profile.id.clone(),
                backend: profile.backend.clone(),
                label: profile.label.clone(),
                support_level: profile
                    .support_level
                    .clone()
                    .unwrap_or_else(|| "experimental".to_string()),
                derived_from: profile.derived_from.clone(),
                env_dir: profile.env_dir.clone(),
                requirements: profile.requirements.clone(),
                // Expose the active binding first so UI path/probe/conflict
                // cards follow the selected shared or private runtime.
                runtime_binding: active_binding
                    .clone()
                    .or_else(|| profile.runtime_binding.clone()),
                runtime_bindings,
                notes: profile.notes.clone(),
                resolved_runtime_dir,
                state,
                setup_job,
                runtime_probe_state,
                runtime_probe_states,
                runtime_conflicts,
                model_binding_checks,
            }
        })
        .collect();
    let smoke_test = manifest
        .smoke_test
        .as_ref()
        .map(|smoke_test| PluginSmokeTestSummary {
            command: smoke_test.command.clone(),
            capability: smoke_test.capability.clone(),
            timeout_ms: smoke_test.timeout_ms,
        });
    let capabilities = manifest
        .capabilities
        .iter()
        .map(|capability| PluginCapabilitySummary {
            id: capability.id.clone(),
            kind: capability.kind.clone(),
            name: capability.name.clone(),
            version: capability.version.clone(),
            inputs: capability.inputs.clone(),
            outputs: capability.outputs.clone(),
            parameters: capability.parameters.clone(),
        })
        .collect();
    let contributes = PluginContributesSummary {
        menus: manifest
            .contributes
            .as_ref()
            .map(|contributes| {
                contributes
                    .menus
                    .iter()
                    .map(|menu| PluginMenuContributionSummary {
                        id: menu.id.clone(),
                        label: menu.label.clone(),
                        capability: menu.capability.clone(),
                        contexts: menu.contexts.clone(),
                        placements: menu.placements.clone(),
                        icon: menu.icon.clone(),
                        order: menu.order.unwrap_or(1000),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    let mut plugin_task_states: Vec<AiPluginTaskState> = task_states
        .values()
        .filter(|state| state.plugin_id == plugin_id)
        .cloned()
        .collect();
    plugin_task_states.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    plugin_task_states.truncate(8);

    let model_files = plugin_model_file_summaries(&manifest).unwrap_or_default();
    AiPluginSummary {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        publisher: manifest.publisher,
        path: normalize_path(root),
        manifest_path: normalize_path(manifest_path),
        platform_supported,
        validation,
        permissions,
        permission_grant,
        runtimes: manifest.runtimes,
        runtime,
        entry,
        install,
        storage,
        install_profiles,
        smoke_test,
        capabilities,
        contributes,
        task_states: plugin_task_states,
        model_bindings: manifest.model_bindings,
        model_files,
    }
}

fn status_path_for(entry: &PluginEntry) -> String {
    entry
        .status
        .as_ref()
        .map(|status| status.path.clone())
        .unwrap_or_else(|| {
            if let Some(health) = &entry.health {
                if health.path == "/status" {
                    return health.path.clone();
                }
            }
            "/status".to_string()
        })
}

fn health_path_for(entry: &PluginEntry) -> String {
    entry
        .health
        .as_ref()
        .map(|health| health.path.clone())
        .unwrap_or_else(|| "/health".to_string())
}

fn invoke_path_for(capability: &PluginCapability) -> String {
    capability
        .invoke
        .as_ref()
        .and_then(|invoke| invoke.get("path"))
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(|path| path.to_string())
        .unwrap_or_else(|| format!("/invoke/{}", capability.id))
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    format!("{}{}", base, path)
}

fn loopback_base_url(port: u16) -> String {
    format!("http://127.0.0.1:{}", port)
}

/// Generate a cryptographically random auth token (32 bytes, hex-encoded to 64
/// characters). Injected into the plugin process as `PICAIPIC_PLUGIN_AUTH_TOKEN`
/// and sent by the host as `Authorization: Bearer <token>` on every request
/// except `/health`.
fn generate_plugin_auth_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::thread_rng().r#gen();
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

/// Build a `reqwest::Client` with an optional `Authorization: Bearer <token>`
/// default header. When `token` is `None`, no auth header is added (used for
/// stale-service reachability probes and post-stop liveness checks).
fn plugin_http_client(token: Option<&str>, timeout_ms: u64) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms));
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)) {
            headers.insert(reqwest::header::AUTHORIZATION, value);
            builder = builder.default_headers(headers);
        }
    }
    builder
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

/// Look up the auth token for a managed plugin runtime, if it is currently
/// tracked by the host.
async fn runtime_auth_token(state: &AiPluginRuntimeState, plugin_id: &str) -> Option<String> {
    let processes = state.processes.lock().await;
    processes
        .get(plugin_id)
        .map(|runtime| runtime.auth_token.clone())
}

/// Resolve the bearer token for a plugin call, failing closed.
///
/// A host-managed runtime always has a token the host generated, so a missing one
/// means the tracked process is gone (crashed, exited, or reaped) and the manifest
/// port may now be answered by something else. Sending the payload unauthenticated
/// in that state would hand staged input paths, output dirs, and task parameters to
/// whatever owns the port, so the call is refused. Externally managed services (a
/// manifest `baseUrl` with no `startCommand`) have no host-issued token by design
/// and remain the only path allowed to call without one.
async fn resolve_plugin_auth_token(
    state: &AiPluginRuntimeState,
    plugin_id: &str,
    entry: &PluginEntry,
) -> Result<String, String> {
    match runtime_auth_token(state, plugin_id).await {
        Some(token) if !token.is_empty() => Ok(token),
        _ if entry.start_command.is_none() => Ok(String::new()),
        _ => Err(format!(
            "Refusing to call plugin '{plugin_id}' without its runtime auth token; start the plugin again"
        )),
    }
}

fn plugin_port(entry: &PluginEntry) -> Option<u16> {
    if let Some(port) = entry.default_port {
        return Some(port);
    }

    let base_url = entry.base_url.as_deref()?;
    let after_scheme = base_url.split("://").nth(1).unwrap_or(base_url);
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
    let port_text = host_port.rsplit(':').next()?;
    port_text.parse::<u16>().ok()
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

async fn allocate_plugin_port(
    plugin_id: &str,
    entry: &PluginEntry,
    state: &AiPluginRuntimeState,
) -> Result<u16, String> {
    let used_ports: HashSet<u16> = {
        let processes = state.processes.lock().await;
        processes
            .iter()
            .filter(|(id, _)| id.as_str() != plugin_id)
            .map(|(_, runtime)| runtime.port)
            .collect()
    };

    if let Some(port) = plugin_port(entry) {
        if !used_ports.contains(&port) && is_port_available(port) {
            return Ok(port);
        }
    }

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("Failed to allocate plugin port: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to inspect allocated plugin port: {}", e))?
        .port();
    Ok(port)
}

async fn runtime_base_url(state: Option<&AiPluginRuntimeState>, plugin_id: &str) -> Option<String> {
    let state = state?;
    let processes = state.processes.lock().await;
    processes
        .get(plugin_id)
        .map(|runtime| runtime.base_url.clone())
}

async fn plugin_base_url(
    plugin_id: &str,
    entry: &PluginEntry,
    state: Option<&AiPluginRuntimeState>,
) -> Option<String> {
    runtime_base_url(state, plugin_id)
        .await
        .or_else(|| entry.base_url.clone())
}

fn plugin_data_dir(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_store_dir()?.join("plugin-data").join(plugin_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin data directory: {}", e))?;
    Ok(dir)
}

fn plugin_logs_dir(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_data_dir(plugin_id)?.join("logs");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin logs directory: {}", e))?;
    Ok(dir)
}

fn plugin_model_dir(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_data_dir(plugin_id)?.join("models");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin model directory: {}", e))?;
    Ok(dir)
}

fn plugin_config_path(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_data_dir(plugin_id)?.join("config");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin config directory: {}", e))?;
    Ok(dir.join("plugin.local.json"))
}

fn plugin_runtime_root(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_store_dir()?.join("plugin-runtimes").join(plugin_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin runtime directory: {}", e))?;
    Ok(dir)
}

fn shared_runtime_root(runtime_id: &str) -> Result<PathBuf, String> {
    let runtime_id = runtime_id.trim();
    if runtime_id.is_empty() || safe_relative_path_from_str(runtime_id).is_none() {
        return Err(format!("Shared runtime id is not safe: {}", runtime_id));
    }
    let dir = plugin_store_dir()?.join("shared-runtimes").join(runtime_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create shared runtime directory: {}", e))?;
    Ok(dir)
}

fn profile_runtime_root(
    plugin_id: &str,
    profile: &PluginInstallProfile,
) -> Result<PathBuf, String> {
    if profile
        .runtime_binding
        .as_ref()
        .map(|binding| binding.scope.eq_ignore_ascii_case("shared"))
        .unwrap_or(false)
    {
        let runtime_id = profile
            .runtime_binding
            .as_ref()
            .and_then(|binding| binding.id.as_deref())
            .filter(|id| !id.trim().is_empty())
            .unwrap_or(&profile.id);
        shared_runtime_root(runtime_id)
    } else {
        plugin_runtime_root(plugin_id)
    }
}

fn profile_runtime_dir(
    plugin_id: &str,
    profile: &PluginInstallProfile,
) -> Result<Option<PathBuf>, String> {
    let Some(env_dir) = profile
        .env_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    if !safe_profile_relative_path(env_dir) {
        return Err(format!(
            "Install profile '{}' has unsafe envDir '{}'",
            profile.id, env_dir
        ));
    }
    let dir = if profile
        .runtime_binding
        .as_ref()
        .map(|binding| binding.scope.eq_ignore_ascii_case("shared"))
        .unwrap_or(false)
    {
        profile_runtime_root(plugin_id, profile)?
    } else {
        profile_runtime_root(plugin_id, profile)?.join(env_dir)
    };
    Ok(Some(dir))
}

fn plugin_cache_dir(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_store_dir()?.join("plugin-cache").join(plugin_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin cache directory: {}", e))?;
    Ok(dir)
}

fn plugin_task_temp_dir(plugin_id: &str, task_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_cache_dir(plugin_id)?.join("tasks").join(task_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin task directory: {}", e))?;
    Ok(dir)
}

fn plugin_task_output_dir(plugin_id: &str, task_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_task_temp_dir(plugin_id, task_id)?.join("outputs");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin task output directory: {}", e))?;
    Ok(dir)
}

fn plugin_output_dir(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = plugin_store_dir()?.join("plugin-outputs").join(plugin_id);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create plugin output directory: {}", e))?;
    Ok(dir)
}

fn path_modified_age_secs(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .map(|duration| duration.as_secs())
}

fn task_updated_age_secs(state: &AiPluginTaskState) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(&state.updated_at)
        .ok()
        .map(|updated_at| {
            Utc::now()
                .signed_duration_since(updated_at.with_timezone(&Utc))
                .num_seconds()
                .max(0) as u64
        })
}

fn plugin_task_dir_matches(state: &AiPluginTaskState, path: &Path) -> bool {
    let state_dir = PathBuf::from(&state.task_dir);
    let canonical_state = state_dir.canonicalize().unwrap_or(state_dir);
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical_state == canonical_path
}

fn cleanup_orphan_plugin_task_dirs(
    plugin_id: &str,
    registry: &AiPluginRegistry,
) -> Result<(), String> {
    let tasks_dir = plugin_cache_dir(plugin_id)?.join("tasks");
    let Ok(entries) = fs::read_dir(&tasks_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("tmp"))
                .unwrap_or(false)
                && path_modified_age_secs(&path).unwrap_or(0) > PLUGIN_TASK_TMP_TTL_SECS
            {
                let _ = fs::remove_file(&path);
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let has_ledger_record = registry
            .task_states
            .values()
            .any(|state| state.plugin_id == plugin_id && plugin_task_dir_matches(state, &path));
        if !has_ledger_record
            && path_modified_age_secs(&path).unwrap_or(0) > PLUGIN_TASK_CACHE_TTL_SECS
        {
            let _ = fs::remove_dir_all(&path);
        }
    }
    Ok(())
}

fn cleanup_stale_plugin_tasks_in_registry(
    plugin_id: &str,
    registry: &mut AiPluginRegistry,
) -> bool {
    let mut changed = false;
    for state in registry.task_states.values_mut() {
        if state.plugin_id != plugin_id {
            continue;
        }
        if matches!(state.status.as_str(), "failed" | "cancelled" | "canceled") {
            cleanup_failed_plugin_task_dir(state);
            continue;
        }
        if state.status == "succeeded"
            && !state.adopted
            && task_updated_age_secs(state).unwrap_or(0) > PLUGIN_TASK_SUCCESS_TTL_SECS
        {
            let task_dir = PathBuf::from(&state.task_dir);
            if task_dir.exists() {
                let _ = fs::remove_dir_all(&task_dir);
            }
            state.status = "discarded".to_string();
            state.updated_at = Utc::now().to_rfc3339();
            state.outputs.clear();
            state.progress = Some(100);
            state.message = Some("Unadopted outputs expired and were cleaned up.".to_string());
            state.error = None;
            state.error_code = None;
            state.error_domain = None;
            state.error_details = None;
            state.retryable = false;
            changed = true;
        }
    }
    changed
}

fn cleanup_plugin_task_cache_with_registry(
    plugin_id: &str,
    registry: &mut AiPluginRegistry,
) -> Result<bool, String> {
    let changed = cleanup_stale_plugin_tasks_in_registry(plugin_id, registry);
    cleanup_orphan_plugin_task_dirs(plugin_id, registry)?;
    Ok(changed)
}

fn cleanup_plugin_task_cache(plugin_id: &str) -> Result<(), String> {
    let mut registry = load_registry()?;
    let changed = cleanup_plugin_task_cache_with_registry(plugin_id, &mut registry)?;
    if changed {
        save_registry(&registry)?;
    }
    Ok(())
}

fn validate_plugin_output_paths(result: &Value, output_dir: &Path) -> Result<(), String> {
    let Some(outputs) = result.get("outputs").and_then(Value::as_array) else {
        return Ok(());
    };
    for output in outputs {
        let Some(path) = output.get("path").and_then(Value::as_str) else {
            continue;
        };
        let output_path = PathBuf::from(path);
        let canonical = output_path
            .canonicalize()
            .map_err(|e| format!("Plugin returned missing output '{}': {}", path, e))?;
        // Host adoption only trusts outputs under the task output root (not the
        // full writable allow-list — library paths must never appear here).
        if !is_path_inside(&canonical, output_dir) {
            return Err(format!(
                "Plugin returned output outside the task output directory: {}",
                path
            ));
        }
        let metadata = fs::metadata(&canonical)
            .map_err(|e| format!("Failed to read plugin output '{}': {}", path, e))?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(format!(
                "Plugin returned empty or non-file output: {}",
                path
            ));
        }
    }
    Ok(())
}

/// How one external input was materialized under the staging directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageMaterializeMethod {
    /// Same-volume hard link (near zero-copy). Preferred when the OS allows it.
    Hardlink,
    /// Full byte copy (cross-volume or hardlink unsupported/failed).
    Copy,
}

/// Diagnostics for one invoke-time input staging pass.
///
/// Surfaced in the task `message` and written to
/// `<task_dir>/inputs/staging-report.json` when staging ran (or was disabled).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct InputStagingReport {
    enabled: bool,
    staged_files: u32,
    /// Logical size of staged inputs (file length). For hardlinks this is not
    /// extra disk usage; for copies it approximates transferred bytes.
    staged_bytes: u64,
    hardlinked_files: u32,
    copied_files: u32,
    skipped_writable: u32,
    skipped_missing: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    staging_dir: Option<String>,
}

impl InputStagingReport {
    fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    fn queue_message(&self) -> String {
        if !self.enabled {
            return "Queued (input staging disabled)".to_string();
        }
        if self.staged_files == 0 {
            return format!(
                "Queued (staging: 0 files, {} already writable, {} missing)",
                self.skipped_writable, self.skipped_missing
            );
        }
        format!(
            "Queued (staging: {} file(s), {} bytes, {} hardlink, {} copy, {} already writable)",
            self.staged_files,
            self.staged_bytes,
            self.hardlinked_files,
            self.copied_files,
            self.skipped_writable
        )
    }

    fn record_materialized(&mut self, bytes: u64, method: StageMaterializeMethod) {
        self.staged_files = self.staged_files.saturating_add(1);
        self.staged_bytes = self.staged_bytes.saturating_add(bytes);
        match method {
            StageMaterializeMethod::Hardlink => {
                self.hardlinked_files = self.hardlinked_files.saturating_add(1);
            }
            StageMaterializeMethod::Copy => {
                self.copied_files = self.copied_files.saturating_add(1);
            }
        }
    }
}

/// Stage external input files into the plugin's readable area for sandboxing.
///
/// When default input staging is enabled (all supported platforms unless
/// `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1`), this rewrites every JSON `path` field
/// in `inputs` that points outside the plugin's writable directories. The file is
/// materialized under `<task_dir>/inputs/` and the path is replaced with the
/// staged location. Files already inside a writable dir are left untouched.
///
/// Materialization is a **copy** unless `allow_hardlink` is set, which the caller
/// only does for manifests declaring `writeSourceFiles`; see `stage_one_file`.
///
/// Returns the (possibly rewritten) inputs value plus a diagnostics report.
/// Staging materialize failures fail closed (error, no original external path left).
fn stage_input_files_for_sandbox(
    plugin_id: &str,
    task_id: &str,
    inputs: &Value,
    writable_dirs: &[PathBuf],
    allow_hardlink: bool,
) -> Result<(Value, InputStagingReport), String> {
    if !t_sandbox::sandbox_enabled() {
        return Ok((inputs.clone(), InputStagingReport::disabled()));
    }
    let task_dir = plugin_task_temp_dir(plugin_id, task_id)?;
    let staging_dir = task_dir.join("inputs");
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to create input staging directory: {}", e))?;
    let mut staged = inputs.clone();
    let mut report = InputStagingReport {
        enabled: true,
        staging_dir: Some(normalize_path(&staging_dir)),
        ..InputStagingReport::default()
    };
    // Fail closed: a staging error must not leave the original external path in
    // the payload (that would silently bypass default confinement).
    stage_paths_in_value(
        &mut staged,
        &staging_dir,
        writable_dirs,
        &mut report,
        allow_hardlink,
    )?;
    write_input_staging_report(&staging_dir, &report);
    Ok((staged, report))
}

/// Best-effort diagnostics file next to staged inputs for support/debug.
fn write_input_staging_report(staging_dir: &Path, report: &InputStagingReport) {
    let path = staging_dir.join("staging-report.json");
    if let Ok(json) = serde_json::to_vec_pretty(report) {
        let _ = fs::write(path, json);
    }
}

/// Recursively walk a JSON value, rewriting every `path` string field that
/// points outside the writable dirs to a staged file under `staging_dir`.
/// Returns an error if any required staging materialize fails.
fn stage_paths_in_value(
    value: &mut Value,
    staging_dir: &Path,
    writable_dirs: &[PathBuf],
    report: &mut InputStagingReport,
    allow_hardlink: bool,
) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            if let Some(path_str) = map.get("path").and_then(Value::as_str) {
                let path = PathBuf::from(path_str);
                if !path.is_absolute() {
                    // Relative paths are plugin-local; leave them alone.
                } else if !path.exists() {
                    report.skipped_missing = report.skipped_missing.saturating_add(1);
                } else {
                    let inside_writable = path_is_inside_any(&path, writable_dirs);
                    let inside_staging = is_path_inside(&path, staging_dir);
                    if inside_writable || inside_staging {
                        report.skipped_writable = report.skipped_writable.saturating_add(1);
                    } else {
                        let (staged_path, bytes, method) =
                            stage_one_file(&path, staging_dir, allow_hardlink)?;
                        map["path"] = Value::String(staged_path.to_string_lossy().into_owned());
                        report.record_materialized(bytes, method);
                    }
                }
            }
            for (_, child) in map.iter_mut() {
                stage_paths_in_value(child, staging_dir, writable_dirs, report, allow_hardlink)?;
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                stage_paths_in_value(item, staging_dir, writable_dirs, report, allow_hardlink)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Materialize a single file into the staging directory, generating a unique
/// name to avoid collisions when multiple inputs share a basename.
///
/// `allow_hardlink` must be true **only** when the manifest declares
/// `writeSourceFiles`. A hardlink shares the source inode, so a plugin that writes
/// back to its input path would silently rewrite the user's original media.
/// Everything else gets a real copy, which keeps the library file immutable even
/// though the staged path is inside the plugin's writable roots.
///
/// With `allow_hardlink`, the order is (Phase 2):
/// 1. `hard_link` — near zero-copy when source and staging share a volume
/// 2. `copy` — fallback for cross-volume paths or hardlink errors
///
/// Returns `(staged_path, logical_bytes, method)`.
fn stage_one_file(
    src: &Path,
    staging_dir: &Path,
    allow_hardlink: bool,
) -> Result<(PathBuf, u64, StageMaterializeMethod), String> {
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "input".to_string());
    let ext = src
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    // Disambiguate collisions with a counter.
    let mut candidate = staging_dir.join(format!("{}{}", stem, ext));
    let mut counter = 1;
    while candidate.exists() {
        candidate = staging_dir.join(format!("{}_{}{}", stem, counter, ext));
        counter += 1;
    }

    let logical_bytes = fs::metadata(src).map(|m| m.len()).unwrap_or(0);

    // Hardlink only for capabilities the user explicitly allowed to edit sources
    // in place; it is near zero-copy for large video/RAW but shares the inode.
    if allow_hardlink {
        match fs::hard_link(src, &candidate) {
            Ok(()) => return Ok((candidate, logical_bytes, StageMaterializeMethod::Hardlink)),
            Err(_hardlink_err) => {
                // Clean a partial hardlink target if the OS left one (rare).
                let _ = fs::remove_file(&candidate);
            }
        }
    }

    let bytes = fs::copy(src, &candidate).map_err(|e| {
        format!(
            "Failed to stage input '{}' (copying into the staging directory failed: {})",
            src.display(),
            e
        )
    })?;
    Ok((candidate, bytes, StageMaterializeMethod::Copy))
}

fn cleanup_failed_plugin_task_dir(state: &AiPluginTaskState) {
    if !matches!(state.status.as_str(), "failed" | "cancelled" | "canceled") {
        return;
    }
    let task_dir = PathBuf::from(&state.task_dir);
    if task_dir.exists() {
        let _ = fs::remove_dir_all(task_dir);
    }
}

fn task_error_retryable(code: &str, domain: Option<&str>) -> bool {
    let code = code.to_ascii_uppercase();
    if matches!(
        code.as_str(),
        "DEVICE_OOM"
            | "PLUGIN_NOT_READY"
            | "TIMEOUT"
            | "HTTP_ERROR"
            | "RESPONSE_PARSE_FAILED"
            | "OUTPUT_VALIDATION_FAILED"
    ) {
        return true;
    }
    matches!(
        domain,
        Some("device_backend") | Some("runtime") | Some("transport")
    )
}

fn apply_task_error(
    state: &mut AiPluginTaskState,
    code: &str,
    domain: Option<&str>,
    message: String,
    details: Option<Value>,
) {
    state.status = "failed".to_string();
    state.updated_at = Utc::now().to_rfc3339();
    state.error = Some(message);
    state.message = state.error.clone();
    state.error_code = Some(code.to_string());
    state.error_domain = domain.map(str::to_string);
    state.error_details = details;
    state.retryable = task_error_retryable(code, domain);
    cleanup_failed_plugin_task_dir(state);
}

fn apply_plugin_task_progress(state: &mut AiPluginTaskState, result: &Value) {
    if let Some(progress) = result.get("progress").and_then(Value::as_u64) {
        state.progress = Some(progress.min(100) as u8);
    }
    if let Some(message) = result.get("message").and_then(Value::as_str) {
        state.message = Some(message.to_string());
    }
}

fn apply_plugin_error(state: &mut AiPluginTaskState, result: &Value, fallback: String) {
    let error = result.get("error").unwrap_or(&Value::Null);
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("PLUGIN_ERROR");
    let domain = error.get("domain").and_then(Value::as_str);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or(fallback);
    let details = error.get("details").cloned();
    apply_task_error(state, code, domain, message, details);
}

fn plugin_task_status(result: &Value) -> Option<&str> {
    result
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| result.get("taskStatus").and_then(Value::as_str))
}

fn is_plugin_task_terminal(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "completed" | "failed" | "error" | "cancelled" | "canceled"
    )
}

fn apply_plugin_task_result(
    state: &mut AiPluginTaskState,
    result: &Value,
    output_dir: &Path,
) -> Result<(), String> {
    let status = plugin_task_status(result)
        .unwrap_or_else(|| {
            if result.get("ok").and_then(Value::as_bool) == Some(false) {
                "failed"
            } else {
                ""
            }
        })
        .to_ascii_lowercase();

    match status.as_str() {
        "queued" | "running" | "cancelling" => {
            state.status = status;
            state.updated_at = Utc::now().to_rfc3339();
            apply_plugin_task_progress(state, result);
        }
        "succeeded" | "completed" => {
            if let Err(error) = validate_plugin_output_paths(result, output_dir) {
                apply_task_error(
                    state,
                    "OUTPUT_VALIDATION_FAILED",
                    Some("filesystem"),
                    error.clone(),
                    None,
                );
                return Err(error);
            }
            state.status = "succeeded".to_string();
            state.updated_at = Utc::now().to_rfc3339();
            state.outputs = result
                .get("outputs")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            state.progress = Some(100);
            apply_plugin_task_progress(state, result);
            if state.message.is_none() {
                state.message = Some("Completed".to_string());
            }
            state.error = None;
            state.error_code = None;
            state.error_domain = None;
            state.error_details = None;
            state.retryable = false;
        }
        "cancelled" | "canceled" => {
            state.status = "cancelled".to_string();
            state.updated_at = Utc::now().to_rfc3339();
            apply_plugin_task_progress(state, result);
            state.error = result
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string);
            state.error_code = result
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str)
                .map(str::to_string);
            state.error_domain = result
                .get("error")
                .and_then(|error| error.get("domain"))
                .and_then(Value::as_str)
                .map(str::to_string);
            state.error_details = result
                .get("error")
                .and_then(|error| error.get("details"))
                .cloned();
            state.retryable = false;
            cleanup_failed_plugin_task_dir(state);
        }
        "failed" | "error" => {
            apply_plugin_task_progress(state, result);
            apply_plugin_error(state, result, "Plugin task failed".to_string());
            cleanup_failed_plugin_task_dir(state);
        }
        "" => {}
        other => {
            state.status = other.to_string();
            state.updated_at = Utc::now().to_rfc3339();
        }
    }

    Ok(())
}

fn apply_plugin_task_event(
    state: &mut AiPluginTaskState,
    event: &Value,
    output_dir: &Path,
) -> Result<(), String> {
    if let Some(task_state) = event.get("state") {
        apply_plugin_task_result(state, task_state, output_dir)
    } else {
        apply_plugin_task_result(state, event, output_dir)
    }
}

async fn poll_plugin_task_until_terminal(
    base_url: &str,
    task_id: &str,
    token: &str,
    state: &mut AiPluginTaskState,
    output_dir: &Path,
) -> Result<Value, String> {
    let url = join_url(base_url, &format!("/tasks/{}", task_id));
    let events_url = join_url(base_url, &format!("/tasks/{}/events", task_id));
    let client = plugin_http_client(Some(token), 30_000)?;
    let deadline = tokio::time::Instant::now()
        .checked_add(Duration::from_millis(PLUGIN_TASK_POLL_TIMEOUT_MS))
        .unwrap_or_else(tokio::time::Instant::now);
    let mut event_cursor: i64 = 0;
    let mut events_supported = true;

    loop {
        if tokio::time::Instant::now() >= deadline {
            let _ = request_plugin_task_cancel(base_url, task_id, token).await;
            apply_task_error(
                state,
                "TIMEOUT",
                Some("transport"),
                format!(
                    "Plugin task did not finish within {} ms",
                    PLUGIN_TASK_POLL_TIMEOUT_MS
                ),
                None,
            );
            save_task_state(state.clone())?;
            return Err(state
                .error
                .clone()
                .unwrap_or_else(|| "Plugin task polling timed out".to_string()));
        }

        let response = if events_supported {
            match client
                .get(&events_url)
                .query(&[
                    ("after", event_cursor.to_string()),
                    ("timeoutMs", "25000".to_string()),
                ])
                .send()
                .await
            {
                Ok(response) if response.status().as_u16() != 404 => Some(response),
                Ok(_) => {
                    events_supported = false;
                    None
                }
                Err(_) => {
                    events_supported = false;
                    None
                }
            }
        } else {
            None
        };

        let response = match response {
            Some(response) => response,
            None => match client.get(&url).send().await {
                Ok(response) => response,
                Err(error) => {
                    apply_task_error(
                        state,
                        "HTTP_ERROR",
                        Some("transport"),
                        format!("Failed to query plugin task status: {}", error),
                        None,
                    );
                    save_task_state(state.clone())?;
                    return Err(format!("Failed to query plugin task status: {}", error));
                }
            },
        };
        let status_code = response.status();
        let result = match response.json::<Value>().await {
            Ok(result) => result,
            Err(error) => {
                apply_task_error(
                    state,
                    "RESPONSE_PARSE_FAILED",
                    Some("transport"),
                    format!("Failed to parse plugin task status: {}", error),
                    None,
                );
                save_task_state(state.clone())?;
                return Err(format!("Failed to parse plugin task status: {}", error));
            }
        };

        if !status_code.is_success() {
            apply_plugin_error(
                state,
                &result,
                format!("Task status endpoint returned {}", status_code),
            );
            save_task_state(state.clone())?;
            return Err(format!(
                "Task status endpoint returned {}: {}",
                status_code, result
            ));
        }

        if events_supported {
            if let Some(events) = result.get("events").and_then(Value::as_array) {
                for event in events {
                    apply_plugin_task_event(state, event, output_dir)?;
                    if let Some(seq) = event.get("seq").and_then(Value::as_i64) {
                        event_cursor = event_cursor.max(seq);
                    }
                }
                if events.is_empty() {
                    if let Some(plugin_state) = result.get("state") {
                        apply_plugin_task_result(state, plugin_state, output_dir)?;
                    }
                }
            } else {
                apply_plugin_task_result(state, &result, output_dir)?;
            }
        } else {
            apply_plugin_task_result(state, &result, output_dir)?;
        }
        save_task_state(state.clone())?;

        if is_plugin_task_terminal(&state.status) {
            return Ok(result);
        }

        tokio::time::sleep(Duration::from_millis(PLUGIN_TASK_POLL_INTERVAL_MS)).await;
    }
}

async fn request_plugin_task_cancel(
    base_url: &str,
    task_id: &str,
    token: &str,
) -> Result<(), String> {
    let url = join_url(base_url, &format!("/tasks/{}/cancel", task_id));
    let client = plugin_http_client(Some(token), PLUGIN_TASK_CANCEL_TIMEOUT_MS)?;
    let response = client
        .post(&url)
        .json(&serde_json::json!({ "taskId": task_id }))
        .send()
        .await
        .map_err(|e| format!("Failed to request plugin task cancel: {}", e))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "Plugin task cancel endpoint returned {}",
            response.status()
        ))
    }
}

fn spawn_plugin_task_poll(
    base_url: String,
    task_id: String,
    token: String,
    mut state: AiPluginTaskState,
    output_dir: PathBuf,
) {
    tokio::spawn(async move {
        let _ =
            poll_plugin_task_until_terminal(&base_url, &task_id, &token, &mut state, &output_dir)
                .await;
    });
}

async fn query_plugin_task_once(
    base_url: &str,
    task_id: &str,
    token: &str,
    state: &mut AiPluginTaskState,
    output_dir: &Path,
) -> Result<Value, String> {
    let client = plugin_http_client(Some(token), PLUGIN_DIAGNOSTICS_TIMEOUT_MS)?;
    let events_url = join_url(base_url, &format!("/tasks/{}/events", task_id));
    let status_url = join_url(base_url, &format!("/tasks/{}", task_id));

    if let Ok(response) = client
        .get(&events_url)
        .query(&[("after", "0"), ("timeoutMs", "0")])
        .send()
        .await
    {
        if response.status().is_success() {
            let value = response
                .json::<Value>()
                .await
                .map_err(|e| format!("Failed to parse plugin task events: {}", e))?;
            if let Some(events) = value.get("events").and_then(Value::as_array) {
                for event in events {
                    apply_plugin_task_event(state, event, output_dir)?;
                }
            }
            if let Some(plugin_state) = value.get("state") {
                apply_plugin_task_result(state, plugin_state, output_dir)?;
            }
            save_task_state(state.clone())?;
            return Ok(value);
        }
    }

    let response = client
        .get(&status_url)
        .send()
        .await
        .map_err(|e| format!("Failed to query plugin task status: {}", e))?;
    let status_code = response.status();
    let value = response
        .json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse plugin task status: {}", e))?;
    if !status_code.is_success() {
        return Err(format!(
            "Task status endpoint returned {}: {}",
            status_code, value
        ));
    }
    apply_plugin_task_result(state, &value, output_dir)?;
    save_task_state(state.clone())?;
    Ok(value)
}

fn find_plugin_manifest(plugin_id: &str) -> Result<(PathBuf, AiPluginManifest), String> {
    for manifest_path in discover_manifest_paths()? {
        let manifest = read_manifest(&manifest_path)?;
        if manifest.id == plugin_id {
            return Ok((manifest_path, manifest));
        }
    }
    Err(format!("Plugin '{}' was not found", plugin_id))
}

fn find_plugin_capability<'a>(
    manifest: &'a AiPluginManifest,
    capability_id: &str,
) -> Result<&'a PluginCapability, String> {
    manifest
        .capabilities
        .iter()
        .find(|capability| capability.id == capability_id)
        .ok_or_else(|| {
            format!(
                "Capability '{}' was not found in plugin '{}'",
                capability_id, manifest.id
            )
        })
}

fn default_smoke_capability(manifest: &AiPluginManifest) -> String {
    manifest
        .smoke_test
        .as_ref()
        .and_then(|smoke_test| smoke_test.capability.clone())
        .or_else(|| {
            manifest
                .capabilities
                .first()
                .map(|capability| capability.id.clone())
        })
        .unwrap_or_default()
}

fn requested_backend_from_runtime(runtime: Option<&Value>) -> Option<String> {
    runtime
        .and_then(|runtime| runtime.get("preferredDevice"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto")
        .map(|value| value.to_lowercase())
}

fn profile_for_invocation<'a>(
    manifest: &'a AiPluginManifest,
    runtime: Option<&Value>,
) -> Option<&'a PluginInstallProfile> {
    if let Some(backend) = requested_backend_from_runtime(runtime) {
        return manifest
            .install_profiles
            .iter()
            .find(|profile| profile.backend.eq_ignore_ascii_case(&backend));
    }

    manifest
        .install_profiles
        .iter()
        .find(|profile| {
            profile
                .runtime_binding
                .as_ref()
                .and_then(|binding| binding.python.as_ref())
                .is_some()
        })
        .or_else(|| {
            manifest.install_profiles.iter().find(|profile| {
                profile
                    .runtime_bindings
                    .iter()
                    .any(|binding| binding.python.as_ref().is_some())
            })
        })
}

fn ensure_runtime_probe_gate(
    plugin_id: &str,
    capability_id: &str,
    manifest_path: &Path,
    manifest: &AiPluginManifest,
    runtime: Option<&Value>,
) -> Result<(), String> {
    let Some(profile) = profile_for_invocation(manifest, runtime) else {
        return Ok(());
    };
    let registry = load_registry()?;
    // Honor the user-selected/persisted binding first (shared default or
    // confirmed plugin-private fallback). Manifest defaults only apply when no
    // profile state has chosen a runtime yet.
    let runtime_binding = registry
        .profile_states
        .get(&profile_state_key(plugin_id, &profile.id))
        .and_then(|state| state.runtime_binding.clone())
        .map(Some)
        .unwrap_or(selected_runtime_binding(profile, None, None)?);
    let Some(binding) = runtime_binding.as_ref() else {
        return Ok(());
    };
    // Managed shared/plugin scopes may not yet have a concrete python path
    // until setup creates the venv. Only skip the gate when there is truly no
    // selectable binding identity (no python and no id).
    if binding
        .python
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && binding.id.as_deref().unwrap_or_default().trim().is_empty()
    {
        return Ok(());
    }

    if let Some(profile_state) = registry
        .profile_states
        .get(&profile_state_key(plugin_id, &profile.id))
    {
        let same_backend = profile_state.backend.trim().is_empty()
            || profile_state.backend.eq_ignore_ascii_case(&profile.backend);
        let same_binding = runtime_binding_signature(profile_state.runtime_binding.as_ref())
            == runtime_binding_signature(runtime_binding.as_ref());
        if profile_state.verified
            && profile_state.status == "verified"
            && same_backend
            && same_binding
        {
            return Ok(());
        }
    }
    let key = runtime_probe_key(
        plugin_id,
        &profile.id,
        &profile.backend,
        runtime_binding.as_ref(),
    );
    let Some(state) = registry.runtime_probe_states.get(&key).cloned() else {
        return Err(format!(
            "Runtime probe is required before invoking '{}'. Probe profile '{}' ({}) first.",
            capability_id, profile.id, profile.backend
        ));
    };
    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let effective_profile = profile_with_runtime_binding(profile, runtime_binding.clone());
    let state = mark_runtime_probe_staleness(
        state,
        runtime_probe_fingerprint(root, &effective_profile, runtime_binding.as_ref()),
    );
    if state.stale {
        return Err(format!(
            "Runtime probe for profile '{}' is stale: {}. Run Probe again before invoking '{}'.",
            profile.id,
            state.stale_reason.unwrap_or_else(|| "unknown".to_string()),
            capability_id
        ));
    }
    if !state.available || state.status != "passed" {
        return Err(format!(
            "Runtime probe for profile '{}' failed. {}",
            profile.id,
            state
                .error
                .unwrap_or_else(|| "Run Probe and inspect the runtime diagnostics.".to_string())
        ));
    }
    // Block invocation when declared requirements conflict with the versions
    // installed in the probed runtime. This catches environment drift (e.g.
    // numpy upgraded to 2.x in a shared runtime that still pins ==1.26.4)
    // before a capability call fails with a confusing import or ABI error.
    if let Some(requirements_rel) = profile_requirements(profile) {
        if safe_profile_relative_path(requirements_rel) {
            let requirements_path = root.join(requirements_rel);
            if requirements_path.is_file() {
                if let Some(probe_result) = state.result.as_ref() {
                    let conflicts = detect_runtime_conflicts(&requirements_path, probe_result);
                    let blocking: Vec<&RuntimeConflict> = conflicts
                        .iter()
                        .filter(|c| c.kind == "version_mismatch" || c.kind == "missing")
                        .collect();
                    if !blocking.is_empty() {
                        let summary = blocking
                            .iter()
                            .map(|c| c.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ");
                        return Err(format!(
                            "Runtime conflicts for profile '{}': {}. Switch to a plugin-private runtime or re-run Setup.",
                            profile.id, summary
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn ensure_valid_manifest(manifest_path: &Path, manifest: &AiPluginManifest) -> Result<(), String> {
    let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));
    let validation = validate_manifest(manifest, root);
    if validation.valid {
        return Ok(());
    }

    Err(format!(
        "Plugin manifest is invalid: {}",
        validation.errors.join("; ")
    ))
}

#[tauri::command]
pub fn get_ai_plugin_registry() -> Result<AiPluginRegistry, String> {
    load_registry()
}

#[tauri::command]
pub fn grant_ai_plugin_permissions(
    plugin_id: String,
    request: AiPluginPermissionGrantRequest,
) -> Result<AiPluginPermissionGrant, String> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err("Plugin id is required".to_string());
    }

    let mut registry = load_registry()?;
    let grant = AiPluginPermissionGrant {
        plugin_id: plugin_id.clone(),
        runtime_network: request.runtime_network,
        setup_downloads: request.setup_downloads,
        upload_selected_files: request.upload_selected_files,
        upload_outputs: request.upload_outputs,
        allowed_domains: request
            .allowed_domains
            .into_iter()
            .map(|domain| domain.trim().to_string())
            .filter(|domain| !domain.is_empty())
            .collect(),
        updated_at: Some(Utc::now().to_rfc3339()),
    };
    registry
        .permission_grants
        .insert(permission_grant_key(&plugin_id), grant.clone());
    save_registry(&registry)?;
    Ok(grant)
}

#[tauri::command]
pub fn revoke_ai_plugin_permissions(plugin_id: String) -> Result<AiPluginRegistry, String> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err("Plugin id is required".to_string());
    }

    let mut registry = load_registry()?;
    registry
        .permission_grants
        .remove(&permission_grant_key(&plugin_id));
    save_registry(&registry)?;
    Ok(registry)
}

/// Whether the user has granted runtime outbound network for this plugin.
/// Used only for Phase 3 scaffold logging today (no OS enforcement).
fn plugin_runtime_network_granted(
    plugin_id: &str,
    _manifest: &AiPluginManifest,
) -> Result<bool, String> {
    let registry = load_registry()?;
    Ok(registry
        .permission_grants
        .get(&permission_grant_key(plugin_id))
        .map(|g| g.runtime_network)
        .unwrap_or(false))
}

fn resolve_runtime_network_grant(result: Result<bool, String>, plugin_id: &str) -> bool {
    match result {
        Ok(granted) => granted,
        Err(error) => {
            eprintln!(
                "Failed to read runtime network permission for '{}'; denying network: {}",
                plugin_id, error
            );
            false
        }
    }
}

#[tauri::command]
pub fn list_trusted_publishers() -> Result<Vec<AiPluginTrustedPublisher>, String> {
    let registry = load_registry()?;
    Ok(registry.trusted_publishers.values().cloned().collect())
}

/// Trust (or re-trust) a publisher public key. Multi-key safe: adds the key without
/// dropping previously trusted keys for the same publisher.
#[tauri::command]
pub fn trust_publisher(
    publisher: String,
    public_key: String,
) -> Result<Vec<AiPluginTrustedPublisher>, String> {
    let publisher = publisher.trim().to_string();
    let public_key = public_key.trim().to_string();
    if publisher.is_empty() || public_key.is_empty() {
        return Err("Publisher and public key are required".to_string());
    }
    let mut registry = load_registry()?;
    trust_publisher_key_in_registry(&mut registry, publisher, public_key);
    save_registry(&registry)?;
    Ok(registry.trusted_publishers.values().cloned().collect())
}

#[tauri::command]
pub fn remove_trusted_publisher(
    publisher: String,
) -> Result<Vec<AiPluginTrustedPublisher>, String> {
    let publisher = publisher.trim().to_string();
    let mut registry = load_registry()?;
    registry.trusted_publishers.remove(&publisher);
    save_registry(&registry)?;
    Ok(registry.trusted_publishers.values().cloned().collect())
}

/// Revoke a package-signing public key locally. Future installs signed with this key fail closed.
/// Also retires the key inside any trusted publisher entry (does not remove other keys).
#[tauri::command]
pub fn revoke_publisher_key(
    public_key: String,
    reason: Option<String>,
) -> Result<AiPluginRegistry, String> {
    let public_key = public_key.trim().to_string();
    if public_key.is_empty() {
        return Err("public_key is required".to_string());
    }
    let mut registry = load_registry()?;
    if !is_public_key_revoked(&registry, &public_key) {
        registry.revoked_keys.push(AiPluginRevokedKey {
            public_key: public_key.clone(),
            revoked_at: Utc::now().to_rfc3339(),
            reason: reason
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        });
    }
    for tp in registry.trusted_publishers.values_mut() {
        for key in tp.keys.iter_mut() {
            if key.public_key.trim() == public_key {
                key.status = "retired".to_string();
            }
        }
        // If primary display key was revoked, point at another active key if any.
        if tp.public_key.trim() == public_key {
            if let Some(active) = tp
                .keys
                .iter()
                .find(|k| k.status.eq_ignore_ascii_case("active"))
            {
                tp.public_key = active.public_key.clone();
                tp.trusted_at = active.trusted_at.clone();
            }
        }
        normalize_trusted_publisher(tp);
    }
    save_registry(&registry)?;
    Ok(registry)
}

#[tauri::command]
pub fn list_revoked_keys() -> Result<Vec<AiPluginRevokedKey>, String> {
    let registry = load_registry()?;
    Ok(registry.revoked_keys)
}

#[tauri::command]
pub fn get_ai_plugin_host_environment() -> Result<AiPluginHostEnvironment, String> {
    Ok(build_host_environment())
}

#[tauri::command]
pub async fn probe_ai_plugin_python_runtime(
    request: AiPluginPythonRuntimeProbeRequest,
) -> Result<AiPluginPythonRuntimeProbeResult, String> {
    probe_python_runtime(request).await
}

#[tauri::command]
pub fn register_ai_plugin_path(path: String) -> Result<AiPluginRegistry, String> {
    let mut registry = load_registry()?;
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(format!("Plugin path does not exist: {}", path));
    }

    let normalized = normalize_path(&path_buf);
    if !registry.registered_paths.iter().any(|p| p == &normalized) {
        registry.registered_paths.push(normalized);
        registry.registered_paths.sort();
        save_registry(&registry)?;
    }

    Ok(registry)
}

fn zip_entry_normalized_path(name: &str) -> Result<PathBuf, String> {
    if name.trim().is_empty() {
        return Err("Package entry path must not be empty".to_string());
    }

    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(format!("Package entry must be relative: {}", name));
    }

    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(format!(
                "Package entry must not escape the plugin root: {}",
                name
            ));
        }
        path.push(part);
    }

    if path.as_os_str().is_empty() {
        return Err(format!("Package entry path must not be empty: {}", name));
    }

    Ok(path)
}

fn zip_top_level_name(name: &str) -> Result<Option<String>, String> {
    let path = zip_entry_normalized_path(name)?;
    Ok(path
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .filter(|part| !part.trim().is_empty()))
}

fn safe_relative_path_from_str(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return None;
    }

    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return None;
        }
        path.push(part);
    }
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn plugin_model_relative_path(model: &Value) -> Option<PathBuf> {
    let path = model.get("path").and_then(Value::as_str)?;
    let relative = safe_relative_path_from_str(path)?;
    relative
        .strip_prefix("models")
        .ok()
        .map(PathBuf::from)
        .or(Some(relative))
}

fn plugin_model_file_summaries(
    manifest: &AiPluginManifest,
) -> Result<Vec<AiPluginModelFileSummary>, String> {
    let model_dir = plugin_model_dir(&manifest.id)?;
    let mut summaries = Vec::new();
    for model in &manifest.models {
        let Some(relative) = plugin_model_relative_path(model) else {
            continue;
        };
        let path = model_dir.join(relative);
        let id = model
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("model")
            .to_string();
        let name = model
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&id)
            .to_string();
        summaries.push(AiPluginModelFileSummary {
            id,
            name,
            required: model
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            exists: path.is_file(),
            path: normalize_path(&path),
            purpose: model
                .get("purpose")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
        });
    }
    Ok(summaries)
}

fn plugin_model_drop_dirs(manifest: &AiPluginManifest) -> Result<Vec<PathBuf>, String> {
    let model_dir = plugin_model_dir(&manifest.id)?;
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    for model in &manifest.models {
        let Some(relative) = plugin_model_relative_path(model) else {
            continue;
        };
        let target = model_dir.join(relative);
        let drop_dir = if target
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some()
        {
            target
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| model_dir.clone())
        } else {
            target
        };
        let key = normalize_path(&drop_dir);
        if seen.insert(key) {
            dirs.push(drop_dir);
        }
    }

    if dirs.is_empty() {
        dirs.push(model_dir);
    }
    Ok(dirs)
}

fn runtime_binding_display_dir(
    plugin_id: &str,
    profile: &PluginInstallProfile,
    binding: &PluginRuntimeBinding,
) -> Result<Option<PathBuf>, String> {
    if binding.scope.eq_ignore_ascii_case("shared") {
        let runtime_id = binding
            .id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or(&profile.id);
        return shared_runtime_root(runtime_id).map(Some);
    }

    if binding.scope.eq_ignore_ascii_case("external") {
        if let Some(root) = binding
            .root
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(Some(PathBuf::from(root)));
        }
        if let Some(python) = binding
            .python
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(Path::new(python).parent().map(PathBuf::from));
        }
        return Ok(None);
    }

    profile_runtime_dir(plugin_id, profile)
}

fn plugin_runtime_display_dirs(plugin_id: &str, manifest: &AiPluginManifest) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    for profile in &manifest.install_profiles {
        for binding in profile_runtime_bindings(profile) {
            let Ok(Some(dir)) = runtime_binding_display_dir(plugin_id, profile, &binding) else {
                continue;
            };
            let key = normalize_path(&dir);
            if seen.insert(key) {
                dirs.push(dir);
            }
        }
    }

    if dirs.is_empty() {
        if let Ok(dir) = plugin_runtime_root(plugin_id) {
            dirs.push(dir);
        }
    }
    dirs
}

fn prepare_installed_plugin_storage(manifest: &AiPluginManifest) -> Result<(), String> {
    let data_dir = plugin_data_dir(&manifest.id)?;
    let model_dir = plugin_model_dir(&manifest.id)?;
    let logs_dir = plugin_logs_dir(&manifest.id)?;
    let runtime_dir = plugin_runtime_root(&manifest.id)?;
    let cache_dir = plugin_cache_dir(&manifest.id)?;
    let output_dir = plugin_output_dir(&manifest.id)?;
    let config_path = plugin_config_path(&manifest.id)?;

    let drop_dirs = plugin_model_drop_dirs(manifest)?;
    for model in &manifest.models {
        let Some(relative) = plugin_model_relative_path(model) else {
            continue;
        };
        let target = model_dir.join(relative);
        let parent = if target
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some()
        {
            target.parent().map(PathBuf::from)
        } else {
            Some(target)
        };
        if let Some(parent) = parent {
            fs::create_dir_all(&parent).map_err(|e| {
                format!(
                    "Failed to create plugin model directory '{}': {}",
                    parent.display(),
                    e
                )
            })?;
        }
    }

    let mut readme = String::new();
    readme.push_str(&format!("PicAiPic plugin storage for {}\n\n", manifest.id));
    readme.push_str("Put model files under this plugin's models directory.\n\n");
    readme.push_str(&format!("Plugin data: {}\n", data_dir.display()));
    readme.push_str(&format!("Models: {}\n", model_dir.display()));
    readme.push_str(&format!("Logs: {}\n", logs_dir.display()));
    readme.push_str(&format!("Runtime envs: {}\n", runtime_dir.display()));
    readme.push_str(&format!("Task cache: {}\n", cache_dir.display()));
    readme.push_str(&format!("Outputs: {}\n", output_dir.display()));
    readme.push_str(&format!("Local config: {}\n\n", config_path.display()));
    readme.push_str("Expected models:\n");
    if manifest.models.is_empty() {
        readme.push_str("- This plugin did not declare model files.\n");
    } else {
        for model in &manifest.models {
            let id = model.get("id").and_then(Value::as_str).unwrap_or("model");
            let name = model.get("name").and_then(Value::as_str).unwrap_or(id);
            let required = model
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let required_label = if required { "required" } else { "optional" };
            if let Some(relative) = plugin_model_relative_path(model) {
                readme.push_str(&format!(
                    "- [{}] {}: {}\n",
                    required_label,
                    name,
                    model_dir.join(relative).display()
                ));
            }
        }
    }
    if !drop_dirs.is_empty() {
        readme.push_str("\nModel drop folders:\n");
        for dir in &drop_dirs {
            readme.push_str(&format!("- {}\n", dir.display()));
        }
    }
    fs::write(model_dir.join("README.txt"), &readme).map_err(|e| {
        format!(
            "Failed to write plugin model README for '{}': {}",
            manifest.id, e
        )
    })?;
    for dir in drop_dirs {
        if dir != model_dir {
            fs::create_dir_all(&dir).map_err(|e| {
                format!(
                    "Failed to create plugin model directory '{}': {}",
                    dir.display(),
                    e
                )
            })?;
            fs::write(dir.join("README.txt"), &readme).map_err(|e| {
                format!(
                    "Failed to write plugin model README for '{}': {}",
                    manifest.id, e
                )
            })?;
        }
    }

    Ok(())
}

/// Private storage roots that an install may create. Keep this list separate
/// from shared runtimes and external model bindings: those outlive one plugin.
fn plugin_private_storage_roots(plugin_id: &str) -> Result<Vec<PathBuf>, String> {
    let store = plugin_store_dir()?;
    Ok(plugin_private_storage_roots_in(&store, plugin_id))
}

fn plugin_private_storage_roots_in(store: &Path, plugin_id: &str) -> Vec<PathBuf> {
    [
        "plugin-data",
        "plugin-cache",
        "plugin-outputs",
        "plugin-runtimes",
    ]
    .into_iter()
    .map(|subdir| store.join(subdir).join(plugin_id))
    .collect()
}

/// Remove only storage roots this install created. Existing plugin data must
/// survive a failed code replacement.
fn cleanup_new_plugin_storage(roots: &[(PathBuf, bool)]) -> Vec<String> {
    roots
        .iter()
        .filter(|(_, existed)| !*existed)
        .filter_map(|(path, _)| match remove_dir_all_with_retries(path) {
            Ok(()) => None,
            Err(error) => Some(error),
        })
        .collect()
}

fn remove_dir_all_with_retries(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let mut last_error = None;
    for attempt in 0..=PLUGIN_DIRECTORY_REMOVE_RETRIES {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt < PLUGIN_DIRECTORY_REMOVE_RETRIES {
                    std::thread::sleep(Duration::from_millis(150 * (attempt as u64 + 1)));
                }
            }
        }
    }
    Err(format!(
        "{}: {}",
        path.display(),
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown remove error".to_string())
    ))
}

fn restore_directory_from_backup(destination: &Path, backup: Option<&Path>) -> Result<(), String> {
    if destination.exists() {
        remove_dir_all_with_retries(destination).map_err(|error| {
            format!(
                "Failed to remove incomplete plugin directory '{}': {}",
                destination.display(),
                error
            )
        })?;
    }
    if let Some(backup) = backup {
        fs::rename(backup, destination).map_err(|error| {
            format!(
                "Failed to restore previous plugin from '{}' to '{}': {}",
                backup.display(),
                destination.display(),
                error
            )
        })?;
    }
    Ok(())
}

/// Put the new directory in place without ever deleting the old one first.
/// The returned backup remains available until every post-install step commits.
fn replace_plugin_directory(
    staged_destination: &Path,
    destination: &Path,
    backup: &Path,
) -> Result<Option<PathBuf>, String> {
    let previous = if destination.exists() {
        fs::rename(destination, backup).map_err(|error| {
            format!(
                "Failed to stage existing plugin '{}' for replacement: {}",
                destination.display(),
                error
            )
        })?;
        Some(backup.to_path_buf())
    } else {
        None
    };

    if let Err(error) = fs::rename(staged_destination, destination) {
        let restore_error = restore_directory_from_backup(destination, previous.as_deref()).err();
        return Err(match restore_error {
            Some(restore_error) => format!(
                "Failed to move plugin into install directory '{}': {}; rollback also failed: {}",
                destination.display(),
                error,
                restore_error
            ),
            None => format!(
                "Failed to move plugin into install directory '{}': {}; previous plugin was restored",
                destination.display(),
                error
            ),
        });
    }

    Ok(previous)
}

struct StagedPluginDirectory {
    original: PathBuf,
    staged: PathBuf,
}

fn stage_plugin_directory_for_uninstall(
    path: &Path,
    root: &Path,
    plugin_id: &str,
) -> Result<Option<StagedPluginDirectory>, String> {
    if !path.exists() {
        return Ok(None);
    }
    if !path.is_dir() || !is_path_inside(path, root) {
        return Err(format!(
            "Refusing to stage plugin path outside managed storage: {}",
            path.display()
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Plugin path has no parent for uninstall staging: {}",
            path.display()
        )
    })?;
    let staged = parent.join(format!(".uninstalling-{}-{}", plugin_id, Uuid::new_v4()));
    fs::rename(path, &staged).map_err(|error| {
        format!(
            "Failed to stage plugin path '{}' for uninstall: {}",
            path.display(),
            error
        )
    })?;
    Ok(Some(StagedPluginDirectory {
        original: path.to_path_buf(),
        staged,
    }))
}

fn restore_staged_plugin_directories(staged: &[StagedPluginDirectory]) -> Vec<String> {
    let mut errors = Vec::new();
    for entry in staged.iter().rev() {
        if !entry.staged.exists() {
            continue;
        }
        if let Err(error) = fs::rename(&entry.staged, &entry.original) {
            errors.push(format!(
                "{} -> {}: {}",
                entry.staged.display(),
                entry.original.display(),
                error
            ));
        }
    }
    errors
}

fn remove_staged_plugin_directories(staged: Vec<StagedPluginDirectory>) -> Vec<String> {
    staged
        .into_iter()
        .filter_map(|entry| remove_dir_all_with_retries(&entry.staged).err())
        .collect()
}

struct PluginPackageSnapshot {
    path: PathBuf,
}

impl Drop for PluginPackageSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_plugin_package_snapshot(
    source: &Path,
    plugin_root: &Path,
) -> Result<PluginPackageSnapshot, String> {
    fs::create_dir_all(plugin_root)
        .map_err(|e| format!("Failed to create plugin directory: {}", e))?;
    let path = plugin_root.join(format!(".package-snapshot-{}.zip", Uuid::new_v4()));
    let mut input = fs::File::open(source).map_err(|e| {
        format!(
            "Failed to open plugin package '{}': {}",
            source.display(),
            e
        )
    })?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("Failed to create plugin package snapshot: {}", e))?;
    if let Err(error) = std::io::copy(&mut input, &mut output).and_then(|_| output.sync_all()) {
        let _ = fs::remove_file(&path);
        return Err(format!("Failed to snapshot plugin package: {}", error));
    }
    Ok(PluginPackageSnapshot { path })
}

fn read_plugin_package_file<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    entry_path: &str,
) -> Result<String, String> {
    let expected = entry_path.replace('\\', "/");
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read zip entry {}: {}", index, e))?;
        if entry.name().replace('\\', "/") != expected {
            continue;
        }

        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .map_err(|e| format!("Failed to read '{}': {}", entry_path, e))?;
        return Ok(content);
    }

    Err(format!("Package is missing '{}'", entry_path))
}

/// Result of checking a package's signature against the trust store.
#[derive(Debug)]
enum SignatureVerifyResult {
    /// Signature is valid and the publisher is already trusted.
    Verified,
    /// Signature is valid but the publisher is not yet trusted. The frontend
    /// should prompt the user to trust this publisher, then retry install.
    NeedsTrust {
        publisher: String,
        public_key: String,
    },
    /// No signature present. Allowed only in developer mode.
    UnsignedAllowed,
}

/// Check whether a package's Ed25519 signature is valid and whether the
/// publisher is in the user's trust store. In developer mode
/// (`PICAIPIC_ALLOW_UNSIGNED_PLUGINS` env), unsigned packages are allowed.
fn verify_package_signature(
    package_manifest: &AiPluginPackageManifest,
    publisher: &Option<String>,
    registry: &AiPluginRegistry,
) -> Result<SignatureVerifyResult, String> {
    use base64::Engine;
    use ed25519_dalek::{Verifier, VerifyingKey};

    let Some(signature) = package_manifest.signature.as_ref() else {
        // No signature. Allow only in developer mode.
        let dev_mode = std::env::var("PICAIPIC_ALLOW_UNSIGNED_PLUGINS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if dev_mode {
            return Ok(SignatureVerifyResult::UnsignedAllowed);
        }
        return Err(
            "Plugin package is not signed. Enable developer mode to allow unsigned plugins."
                .to_string(),
        );
    };

    if signature.algorithm != "ed25519" {
        return Err(format!(
            "Unsupported signature algorithm '{}'. Only 'ed25519' is supported.",
            signature.algorithm
        ));
    }

    // Reconstruct the signed content: the package manifest JSON without the
    // `signature` field. We re-serialize with a default (None) signature to
    // get the canonical bytes the signer signed.
    //
    // Canonicalization: serialize via `serde_json::Value` so that object keys
    // are emitted in lexicographic order (serde_json's default `Map` is a
    // BTreeMap when the `preserve_order` feature is off, which it is here).
    // The Python signer (`sign_plugin.py`) uses `json.dumps(sort_keys=True)`
    // with the same compact separators, so both sides produce byte-identical
    // output regardless of struct field declaration order or manifest file
    // key order. This is what makes the signature robust against reordering.
    let mut unsigned = package_manifest.clone();
    unsigned.signature = None;
    let unsigned_value = serde_json::to_value(&unsigned).map_err(|e| {
        format!(
            "Failed to serialize package manifest for verification: {}",
            e
        )
    })?;
    let signed_bytes = serde_json::to_vec(&unsigned_value).map_err(|e| {
        format!(
            "Failed to serialize package manifest for verification: {}",
            e
        )
    })?;

    // Decode public key.
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature.public_key.as_bytes())
        .map_err(|e| format!("Invalid signature public key (base64 decode failed): {}", e))?;
    if public_key_bytes.len() != 32 {
        return Err(format!(
            "Invalid Ed25519 public key length: expected 32 bytes, got {}",
            public_key_bytes.len()
        ));
    }
    let mut pk_array = [0u8; 32];
    pk_array.copy_from_slice(&public_key_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| format!("Invalid Ed25519 public key: {}", e))?;

    // Decode signature value.
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature.value.as_bytes())
        .map_err(|e| format!("Invalid signature value (base64 decode failed): {}", e))?;
    if sig_bytes.len() != 64 {
        return Err(format!(
            "Invalid Ed25519 signature length: expected 64 bytes, got {}",
            sig_bytes.len()
        ));
    }
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&sig_bytes);
    let sig = ed25519_dalek::Signature::from_bytes(&sig_array);

    // Verify.
    verifying_key
        .verify(&signed_bytes, &sig)
        .map_err(|e| format!("Package signature verification failed: {}", e))?;

    // Signature is valid. Local revoke list fails closed (even if still "trusted").
    if is_public_key_revoked(registry, &signature.public_key) {
        return Err(format!(
            "Package signing key has been revoked locally. Re-sign with a trusted key or remove the key from the revoke list. Key: {}",
            signature.public_key
        ));
    }

    // Trust store: any *active* key for this publisher may install.
    let publisher_name = publisher
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or("unknown");
    let trusted = registry.trusted_publishers.get(publisher_name);
    match trusted {
        Some(tp) if publisher_accepts_public_key(tp, &signature.public_key) => {
            Ok(SignatureVerifyResult::Verified)
        }
        _ => Ok(SignatureVerifyResult::NeedsTrust {
            publisher: publisher_name.to_string(),
            public_key: signature.public_key.clone(),
        }),
    }
}

fn validate_package_manifest_file_list<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    top_level: &str,
    package_manifest: &AiPluginPackageManifest,
) -> Result<(), String> {
    let mut expected = HashMap::new();
    for file in &package_manifest.files {
        if file.path.trim().is_empty() {
            return Err("Package manifest contains an empty file path".to_string());
        }
        if file.sha256.trim().is_empty() {
            return Err(format!(
                "Package manifest file '{}' is missing sha256",
                file.path
            ));
        }
        zip_entry_normalized_path(&file.path)?;
        expected.insert(file.path.replace('\\', "/"), file.clone());
    }

    if expected.is_empty() {
        return Err("Package manifest file list is empty".to_string());
    }

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read zip entry {}: {}", index, e))?;
        let name = entry.name().replace('\\', "/");
        if name.ends_with('/') {
            continue;
        }
        let relative = name
            .strip_prefix(&format!("{}/", top_level))
            .ok_or_else(|| {
                format!(
                    "Package entry is outside top-level plugin directory: {}",
                    name
                )
            })?
            .to_string();
        if relative == "picaipic.package.json" {
            continue;
        }
        let Some(expected_file) = expected.remove(&relative) else {
            return Err(format!(
                "Package contains file not declared in manifest: {}",
                relative
            ));
        };
        if expected_file.size != entry.size() {
            return Err(format!(
                "Package file size mismatch for '{}': manifest {}, zip {}",
                relative,
                expected_file.size,
                entry.size()
            ));
        }
        let mut hasher = Sha256::new();
        std::io::copy(&mut entry, &mut hasher)
            .map_err(|e| format!("Failed to hash package file '{}': {}", relative, e))?;
        let actual = format!("{:X}", hasher.finalize());
        if !actual.eq_ignore_ascii_case(&expected_file.sha256) {
            return Err(format!("Package file sha256 mismatch for '{}'", relative));
        }
    }

    if !expected.is_empty() {
        let missing = expected.keys().cloned().collect::<Vec<_>>().join(", ");
        return Err(format!(
            "Package manifest declares missing files: {}",
            missing
        ));
    }

    Ok(())
}

fn validate_package_unpacked_size_budget(
    package_manifest: &AiPluginPackageManifest,
) -> Result<(), String> {
    let mut total = 0u64;
    for file in &package_manifest.files {
        if file.size > PLUGIN_PACKAGE_MAX_FILE_BYTES {
            return Err(format!(
                "Plugin package file '{}' exceeds the {} GiB unpacked-file limit",
                file.path,
                PLUGIN_PACKAGE_MAX_FILE_BYTES / 1024 / 1024 / 1024
            ));
        }
        total = total
            .checked_add(file.size)
            .ok_or_else(|| "Plugin package declared file sizes overflow u64".to_string())?;
        if total > PLUGIN_PACKAGE_MAX_UNPACKED_BYTES {
            return Err(format!(
                "Plugin package exceeds the {} GiB unpacked-size limit",
                PLUGIN_PACKAGE_MAX_UNPACKED_BYTES / 1024 / 1024 / 1024
            ));
        }
    }
    Ok(())
}

fn unpack_plugin_package<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    destination: &Path,
) -> Result<(), String> {
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read zip entry {}: {}", index, e))?;
        let relative = zip_entry_normalized_path(entry.name())?;
        let output_path = destination.join(relative);
        if !output_path.starts_with(destination) {
            return Err(format!(
                "Refusing to unpack entry outside destination: {}",
                entry.name()
            ));
        }

        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(|e| {
                format!(
                    "Failed to create plugin package directory '{}': {}",
                    output_path.display(),
                    e
                )
            })?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create plugin package directory '{}': {}",
                    parent.display(),
                    e
                )
            })?;
        }
        let mut output = fs::File::create(&output_path)
            .map_err(|e| format!("Failed to create '{}': {}", output_path.display(), e))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|e| format!("Failed to unpack '{}': {}", output_path.display(), e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn install_ai_plugin_package(
    package_path: String,
) -> Result<AiPluginPackageInstallResult, String> {
    let _mutation_guard = PluginPackageMutationGuard::acquire()?;
    let zip_path = PathBuf::from(&package_path);
    if !zip_path.exists() {
        return Err(format!("Plugin package does not exist: {}", package_path));
    }
    if zip_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| !extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(true)
    {
        return Err("Plugin package must be a .zip file".to_string());
    }

    let plugin_root = plugin_home_dir()?;
    let snapshot = create_plugin_package_snapshot(&zip_path, &plugin_root)?;
    let file = fs::File::open(&snapshot.path).map_err(|e| {
        format!(
            "Failed to open plugin package snapshot '{}': {}",
            snapshot.path.display(),
            e
        )
    })?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read plugin package zip: {}", e))?;
    let mut top_levels = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read zip entry {}: {}", index, e))?;
        if let Some(top_level) = zip_top_level_name(entry.name())? {
            top_levels.insert(top_level);
        }
    }
    if top_levels.len() != 1 {
        return Err("Plugin package must contain exactly one top-level directory".to_string());
    }
    let top_level = top_levels
        .iter()
        .next()
        .cloned()
        .ok_or_else(|| "Plugin package is empty".to_string())?;

    let manifest_entry = format!("{}/{}", top_level, MANIFEST_FILE_NAME);
    let package_entry = format!("{}/picaipic.package.json", top_level);
    let manifest_content = read_plugin_package_file(&mut archive, &manifest_entry)?;
    let package_content = read_plugin_package_file(&mut archive, &package_entry)?;
    let manifest =
        serde_json::from_str::<AiPluginManifest>(manifest_content.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("Failed to parse package plugin manifest: {}", e))?;
    let package_manifest = serde_json::from_str::<AiPluginPackageManifest>(
        package_content.trim_start_matches('\u{feff}'),
    )
    .map_err(|e| format!("Failed to parse package manifest: {}", e))?;

    if package_manifest.schema_version != 1 {
        return Err(format!(
            "Unsupported package schemaVersion {}; expected 1",
            package_manifest.schema_version
        ));
    }
    if package_manifest.package_kind != "picaipic-plugin-package" {
        return Err(format!(
            "Unsupported packageKind '{}'",
            package_manifest.package_kind
        ));
    }
    if package_manifest.plugin_id != manifest.id {
        return Err(format!(
            "Package pluginId '{}' does not match manifest id '{}'",
            package_manifest.plugin_id, manifest.id
        ));
    }
    if package_manifest.version != manifest.version {
        return Err(format!(
            "Package version '{}' does not match manifest version '{}'",
            package_manifest.version, manifest.version
        ));
    }
    if top_level != manifest.id {
        return Err(format!(
            "Package top-level directory '{}' must match plugin id '{}'",
            top_level, manifest.id
        ));
    }

    validate_package_unpacked_size_budget(&package_manifest)?;
    validate_package_manifest_file_list(&mut archive, &top_level, &package_manifest)?;

    // Verify package signature and check the publisher trust store.
    let registry_for_sig = load_registry()?;
    let sig_result =
        verify_package_signature(&package_manifest, &manifest.publisher, &registry_for_sig)?;
    let (signature_verified, needs_trust) = match &sig_result {
        SignatureVerifyResult::Verified => (true, None),
        SignatureVerifyResult::UnsignedAllowed => (false, None),
        SignatureVerifyResult::NeedsTrust {
            publisher,
            public_key,
        } => (false, Some((publisher.clone(), public_key.clone()))),
    };
    if let Some((publisher, public_key)) = needs_trust {
        return Err(format!(
            "TRUST_REQUIRED:{}:{}:{}",
            publisher, public_key, manifest.id
        ));
    }

    let destination = plugin_root.join(&manifest.id);
    let staging_root = plugin_root.join(format!(".installing-{}-{}", manifest.id, Uuid::new_v4()));
    fs::create_dir_all(&staging_root)
        .map_err(|e| format!("Failed to create plugin staging directory: {}", e))?;

    let install_result = (|| -> Result<(AiPluginManifest, PluginValidationReport), String> {
        unpack_plugin_package(&mut archive, &staging_root)?;
        let staged_destination = staging_root.join(&manifest.id);
        let staged_manifest_path = staged_destination.join(MANIFEST_FILE_NAME);
        let staged_manifest = read_manifest(&staged_manifest_path)?;
        let validation = validate_manifest(&staged_manifest, &staged_destination);
        if !validation.valid {
            return Err(format!(
                "Installed plugin manifest is invalid: {}",
                validation.errors.join("; ")
            ));
        }

        if !is_path_inside(&staged_destination, &staging_root) {
            return Err(format!(
                "Refusing to install staged plugin outside staging directory: {}",
                staged_destination.display()
            ));
        }

        Ok((staged_manifest, validation))
    })();

    let (installed_manifest, validation) = match install_result {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(error);
        }
    };

    if destination.exists() && !is_path_inside(&destination, &plugin_root) {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(format!(
            "Refusing to replace plugin outside user plugin directory: {}",
            destination.display()
        ));
    }
    let staged_destination = staging_root.join(&manifest.id);
    let backup = plugin_root.join(format!(".replacing-{}-{}", manifest.id, Uuid::new_v4()));
    let previous = match replace_plugin_directory(&staged_destination, &destination, &backup) {
        Ok(previous) => previous,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(error);
        }
    };
    let _ = fs::remove_dir_all(&staging_root);

    let storage_roots = match plugin_private_storage_roots(&installed_manifest.id) {
        Ok(roots) => roots
            .into_iter()
            .map(|path| {
                let existed = path.exists();
                (path, existed)
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            let rollback_error =
                restore_directory_from_backup(&destination, previous.as_deref()).err();
            return Err(match rollback_error {
                Some(rollback_error) => format!(
                    "Failed to resolve plugin storage after install: {}; rollback also failed: {}",
                    error, rollback_error
                ),
                None => error,
            });
        }
    };

    if let Err(error) = prepare_installed_plugin_storage(&installed_manifest) {
        let storage_errors = cleanup_new_plugin_storage(&storage_roots);
        let rollback_error = restore_directory_from_backup(&destination, previous.as_deref()).err();
        return Err(match (rollback_error, storage_errors.is_empty()) {
            (Some(rollback_error), _) => format!(
                "Failed to prepare installed plugin storage: {}; rollback also failed: {}",
                error, rollback_error
            ),
            (None, false) => format!(
                "Failed to prepare installed plugin storage: {}; cleanup also failed: {}",
                error,
                storage_errors.join("; ")
            ),
            (None, true) => error,
        });
    }

    let storage = plugin_storage_summary(&installed_manifest.id, &destination);
    let model_files = match plugin_model_file_summaries(&installed_manifest) {
        Ok(model_files) => model_files,
        Err(error) => {
            let storage_errors = cleanup_new_plugin_storage(&storage_roots);
            let rollback_error =
                restore_directory_from_backup(&destination, previous.as_deref()).err();
            return Err(match (rollback_error, storage_errors.is_empty()) {
                (Some(rollback_error), _) => format!(
                    "Failed to inspect installed plugin models: {}; rollback also failed: {}",
                    error, rollback_error
                ),
                (None, false) => format!(
                    "Failed to inspect installed plugin models: {}; cleanup also failed: {}",
                    error,
                    storage_errors.join("; ")
                ),
                (None, true) => error,
            });
        }
    };

    let registry = match register_ai_plugin_path(normalize_path(&destination)) {
        Ok(registry) => registry,
        Err(error) => {
            let storage_errors = cleanup_new_plugin_storage(&storage_roots);
            let rollback_error =
                restore_directory_from_backup(&destination, previous.as_deref()).err();
            return Err(match (rollback_error, storage_errors.is_empty()) {
                (Some(rollback_error), _) => format!(
                    "Failed to register installed plugin: {}; rollback also failed: {}",
                    error, rollback_error
                ),
                (None, false) => format!(
                    "Failed to register installed plugin: {}; cleanup also failed: {}",
                    error,
                    storage_errors.join("; ")
                ),
                (None, true) => error,
            });
        }
    };

    if let Some(backup) = previous {
        if let Err(error) = remove_dir_all_with_retries(&backup) {
            eprintln!(
                "Installed plugin '{}' but could not remove replacement backup '{}': {}",
                installed_manifest.id,
                backup.display(),
                error
            );
        }
    }

    Ok(AiPluginPackageInstallResult {
        plugin_id: installed_manifest.id,
        version: installed_manifest.version,
        installed_path: normalize_path(&destination),
        registered_paths: registry.registered_paths,
        package_warnings: package_manifest.warnings,
        validation,
        storage,
        model_files,
        signature_verified,
        publisher: installed_manifest.publisher,
    })
}

#[tauri::command]
pub fn unregister_ai_plugin_path(path: String) -> Result<AiPluginRegistry, String> {
    let mut registry = load_registry()?;
    let normalized = normalize_path(&PathBuf::from(path));
    registry.registered_paths.retain(|p| p != &normalized);
    save_registry(&registry)?;
    Ok(registry)
}

fn clear_plugin_registry_state(registry: &mut AiPluginRegistry, plugin_id: &str) {
    registry
        .permission_grants
        .retain(|_, grant| grant.plugin_id != plugin_id);
    registry
        .profile_states
        .retain(|_, state| state.plugin_id != plugin_id);
    registry
        .setup_jobs
        .retain(|_, job| job.plugin_id != plugin_id);
    registry
        .runtime_probe_states
        .retain(|_, state| state.plugin_id != plugin_id);
    registry
        .task_states
        .retain(|_, state| state.plugin_id != plugin_id);
}

#[tauri::command]
pub async fn uninstall_ai_plugin(
    plugin_id: String,
    mode: Option<String>,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginUninstallResult, String> {
    let _mutation_guard = PluginPackageMutationGuard::acquire()?;
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err("Plugin id is required".to_string());
    }

    let mode = mode
        .as_deref()
        .map(|m| m.trim().to_ascii_lowercase())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "code_only".to_string());
    let purge_data = mode == "code_and_data";

    let plugin_root = plugin_home_dir()?;
    let target = plugin_root.join(&plugin_id);
    if !target.exists() {
        return Err(format!("Installed plugin was not found: {}", plugin_id));
    }
    if !target.is_dir() {
        return Err(format!(
            "Installed plugin path is not a directory: {}",
            target.display()
        ));
    }
    if !is_path_inside(&target, &plugin_root) {
        return Err(format!(
            "Refusing to uninstall plugin outside user plugin directory: {}",
            target.display()
        ));
    }

    let manifest_path = target.join(MANIFEST_FILE_NAME);
    let manifest = read_manifest(&manifest_path)?;
    if manifest.id != plugin_id {
        return Err(format!(
            "Installed plugin id mismatch: expected '{}', found '{}'",
            plugin_id, manifest.id
        ));
    }

    let _ = stop_ai_plugin_runtime(plugin_id.clone(), &state).await;

    let normalized = normalize_path(&target);
    let mut registry = load_registry()?;
    let store = plugin_store_dir()?;
    let mut staged_paths = Vec::new();

    match stage_plugin_directory_for_uninstall(&target, &plugin_root, &plugin_id) {
        Ok(Some(staged)) => staged_paths.push(staged),
        Ok(None) => return Err(format!("Installed plugin was not found: {}", plugin_id)),
        Err(error) => return Err(error),
    }

    // Keep data paths recoverable until registry persistence succeeds. Shared
    // runtimes remain outside this list and are never touched by uninstall.
    if purge_data {
        for path in plugin_private_storage_roots_in(&store, &plugin_id) {
            match stage_plugin_directory_for_uninstall(&path, &store, &plugin_id) {
                Ok(Some(staged)) => staged_paths.push(staged),
                Ok(None) => {}
                Err(error) => {
                    let restore_errors = restore_staged_plugin_directories(&staged_paths);
                    return Err(if restore_errors.is_empty() {
                        error
                    } else {
                        format!(
                            "{}; rollback also failed: {}",
                            error,
                            restore_errors.join("; ")
                        )
                    });
                }
            }
        }
    }

    registry.registered_paths.retain(|path| path != &normalized);
    clear_plugin_registry_state(&mut registry, &plugin_id);
    if let Err(error) = save_registry(&registry) {
        let restore_errors = restore_staged_plugin_directories(&staged_paths);
        return Err(if restore_errors.is_empty() {
            format!(
                "Failed to update plugin registry; installed files were restored: {}",
                error
            )
        } else {
            format!(
                "Failed to update plugin registry: {}; rollback also failed: {}",
                error,
                restore_errors.join("; ")
            )
        });
    }

    let mut removed_extra_paths = Vec::new();
    for entry in &staged_paths[1..] {
        removed_extra_paths.push(normalize_path(&entry.original));
    }
    let cleanup_errors = remove_staged_plugin_directories(staged_paths);
    if !cleanup_errors.is_empty() {
        eprintln!(
            "Plugin '{}' was unregistered, but uninstall cleanup could not remove staged paths: {}",
            plugin_id,
            cleanup_errors.join("; ")
        );
    }

    Ok(AiPluginUninstallResult {
        plugin_id,
        removed_path: normalized,
        registered_paths: registry.registered_paths,
        mode,
        removed_extra_paths,
    })
}

#[tauri::command]
pub fn validate_ai_plugin_manifest(path: String) -> Result<PluginManifestValidationResult, String> {
    let input = PathBuf::from(path);
    let manifest_path = if input.is_dir() {
        input.join(MANIFEST_FILE_NAME)
    } else {
        input
    };
    let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));

    match read_manifest(&manifest_path) {
        Ok(manifest) => {
            let validation = validate_manifest(&manifest, root);
            Ok(PluginManifestValidationResult {
                manifest_path: normalize_path(&manifest_path),
                manifest: Some(manifest),
                validation,
            })
        }
        Err(error) => Ok(PluginManifestValidationResult {
            manifest_path: normalize_path(&manifest_path),
            manifest: None,
            validation: PluginValidationReport {
                valid: false,
                errors: vec![error],
                warnings: Vec::new(),
            },
        }),
    }
}

/// Counts files under `dir` matching a relative glob pattern with `*` wildcards
/// (e.g. `experiments/pretrained_models/*.pth`). Supports a single `*` per path
/// segment; `**` is treated as a regular segment. Uses `walkdir` so it works on
/// all platforms without adding a glob dependency.
fn count_glob_matches(dir: &Path, pattern: &str) -> usize {
    let segments: Vec<&str> = pattern
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect();
    let mut search_root = dir.to_path_buf();
    let mut wildcard_segment: Option<(usize, &str)> = None;
    for (index, segment) in segments.iter().enumerate() {
        if segment.contains('*') {
            wildcard_segment = Some((index, segment));
            break;
        }
        search_root = search_root.join(segment);
    }
    let Some((seg_index, wildcard)) = wildcard_segment else {
        return 0;
    };
    let depth = segments.len() - seg_index - 1;
    let suffix_segments = &segments[seg_index + 1..];
    if !search_root.is_dir() {
        return 0;
    }
    let mut count = 0usize;
    for entry in walkdir::WalkDir::new(&search_root)
        .min_depth(1)
        .max_depth(depth + 1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&search_root)
            .ok()
            .and_then(|p| p.to_str());
        let Some(rel) = rel else { continue };
        let rel_segments: Vec<&str> = rel.split(['/', '\\']).collect();
        if rel_segments.len() != depth + 1 {
            continue;
        }
        if !match_simple_glob(wildcard, rel_segments[0]) {
            continue;
        }
        if suffix_segments
            .iter()
            .zip(rel_segments.iter().skip(1))
            .all(|(pat, name)| match_simple_glob(pat, name))
        {
            count += 1;
        }
    }
    count
}

/// Matches a single path segment against a pattern that may contain `*`
/// (matches any run of characters except the path separator).
fn match_simple_glob(pattern: &str, name: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == name;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return name == pattern;
    }
    let mut rest = name;
    for (index, part) in parts.iter().enumerate() {
        if index == 0 {
            if !rest.starts_with(part) {
                return false;
            }
            rest = &rest[part.len()..];
        } else if index == parts.len() - 1 {
            return rest.ends_with(part);
        } else if let Some(pos) = rest.find(part) {
            rest = &rest[pos + part.len()..];
        } else {
            return false;
        }
    }
    true
}

/// Validates a bound directory against a manifest model binding's declared
/// expected files and globs. Returns present/missing file lists and an `ok`
/// flag (true when no expected file is missing).
fn check_model_binding(binding: &PluginModelBinding, dir: &Path) -> AiPluginModelBindingCheck {
    let mut present = Vec::new();
    let mut missing = Vec::new();
    for expected in &binding.expected_files {
        let rel = expected.trim();
        if rel.is_empty() {
            continue;
        }
        let candidate = dir.join(rel);
        if candidate.is_file() {
            present.push(rel.to_string());
        } else {
            missing.push(rel.to_string());
        }
    }
    for pattern in &binding.expected_globs {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        let matched = count_glob_matches(dir, pattern);
        if matched > 0 {
            present.push(format!(
                "{} ({} match{})",
                pattern,
                matched,
                if matched == 1 { "" } else { "es" }
            ));
        } else {
            missing.push(pattern.to_string());
        }
    }
    AiPluginModelBindingCheck {
        binding_id: binding.id.clone(),
        dir: dir.display().to_string(),
        ok: missing.is_empty(),
        present_files: present,
        missing_files: missing,
    }
}

/// Builds the binding summary list for a plugin/profile, reading the persisted
/// directory from the registry and validating it on the fly.
fn model_binding_summaries(
    manifest: &AiPluginManifest,
    profile_id: &str,
) -> Result<Vec<AiPluginModelBindingSummary>, String> {
    let bindings = persisted_model_dir_bindings(&manifest.id, profile_id);
    let mut summaries = Vec::new();
    for binding in &manifest.model_bindings {
        let dir = bindings.get(&binding.id).cloned();
        let (present, missing) = match dir.as_deref().map(|d| d.trim()).filter(|d| !d.is_empty()) {
            Some(dir_str) => {
                let check = check_model_binding(binding, Path::new(dir_str));
                (check.present_files, check.missing_files)
            }
            None => (Vec::new(), Vec::new()),
        };
        summaries.push(AiPluginModelBindingSummary {
            id: binding.id.clone(),
            label: binding.label.clone(),
            env_var: binding.env_var.clone(),
            env_vars: binding.env_vars.clone(),
            layout: binding.layout.clone(),
            expected_files: binding.expected_files.clone(),
            expected_globs: binding.expected_globs.clone(),
            description: binding.description.clone(),
            dir,
            present_files: present,
            ok: missing.is_empty(),
            missing_files: missing,
        });
    }
    Ok(summaries)
}

#[tauri::command]
pub fn set_ai_plugin_model_dir_binding(
    plugin_id: String,
    profile_id: String,
    binding_id: String,
    dir_path: String,
) -> Result<AiPluginModelBindingCheck, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let binding = manifest
        .model_bindings
        .iter()
        .find(|b| b.id == binding_id)
        .ok_or_else(|| {
            format!(
                "Model binding '{}' not found for plugin '{}'",
                binding_id, plugin_id
            )
        })?;
    let dir = Path::new(&dir_path);
    if !dir.is_dir() {
        return Err(format!("Directory does not exist: {}", dir_path));
    }
    let canonical = dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve directory: {}", e))?;
    let mut registry = load_registry()?;
    let key = profile_state_key(&plugin_id, &profile_id);
    let state = registry
        .profile_states
        .entry(key)
        .or_insert_with(|| AiPluginProfileState {
            plugin_id: plugin_id.clone(),
            profile_id: profile_id.clone(),
            backend: String::new(),
            capability: String::new(),
            status: "notInstalled".to_string(),
            verified: false,
            updated_at: Utc::now().to_rfc3339(),
            setup_attempted: false,
            setup_job_id: None,
            duration_ms: None,
            error: None,
            result: None,
            runtime_binding: None,
            model_dir_bindings: HashMap::new(),
        });
    state
        .model_dir_bindings
        .insert(binding_id.clone(), canonical.display().to_string());
    state.updated_at = Utc::now().to_rfc3339();
    save_registry(&registry)?;
    Ok(check_model_binding(binding, &canonical))
}

#[tauri::command]
pub fn clear_ai_plugin_model_dir_binding(
    plugin_id: String,
    profile_id: String,
    binding_id: String,
) -> Result<(), String> {
    let mut registry = load_registry()?;
    let key = profile_state_key(&plugin_id, &profile_id);
    if let Some(state) = registry.profile_states.get_mut(&key) {
        state.model_dir_bindings.remove(&binding_id);
        state.updated_at = Utc::now().to_rfc3339();
    }
    save_registry(&registry)
}

/// Persist a confirmed switch from a shared (or external) runtime to a
/// plugin-private runtime for one install profile.
///
/// This does **not** run setup. After switching, the UI should re-run Setup so
/// dependencies install into `plugin-runtimes/<plugin-id>/<envDir>`, then Probe
/// and Smoke. Shared runtimes are left intact for other plugins.
#[tauri::command]
pub fn switch_ai_plugin_profile_to_private_runtime(
    plugin_id: String,
    profile_id: String,
) -> Result<AiPluginProfileState, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let profile = find_manifest_profile(&manifest, &plugin_id, &profile_id)?;
    let private_binding = plugin_private_runtime_binding(profile)?;
    let effective_profile = profile_with_runtime_binding(profile, Some(private_binding.clone()));
    // Ensure the private runtime root exists so path chips and setup preview
    // can resolve immediately after the switch.
    let _ = profile_runtime_dir(&plugin_id, &effective_profile)?;

    let mut registry = load_registry()?;
    let key = profile_state_key(&plugin_id, &profile_id);
    let previous = registry.profile_states.get(&key).cloned();
    let backend = previous
        .as_ref()
        .map(|state| state.backend.clone())
        .filter(|backend| !backend.trim().is_empty())
        .unwrap_or_else(|| profile.backend.clone());
    let capability = previous
        .as_ref()
        .map(|state| state.capability.clone())
        .filter(|capability| !capability.trim().is_empty())
        .unwrap_or_else(|| default_smoke_capability(&manifest));
    let model_dir_bindings = previous
        .as_ref()
        .map(|state| state.model_dir_bindings.clone())
        .unwrap_or_else(|| persisted_model_dir_bindings(&plugin_id, &profile_id));
    let now = Utc::now().to_rfc3339();
    let state = AiPluginProfileState {
        plugin_id: plugin_id.clone(),
        profile_id: profile_id.clone(),
        backend: backend.clone(),
        capability,
        // Switching isolation invalidates prior verification for the old binding.
        status: "needsVerify".to_string(),
        verified: false,
        updated_at: now,
        setup_attempted: previous
            .as_ref()
            .map(|state| state.setup_attempted)
            .unwrap_or(false),
        setup_job_id: previous.and_then(|state| state.setup_job_id),
        duration_ms: None,
        error: None,
        result: None,
        runtime_binding: Some(private_binding),
        model_dir_bindings,
    };
    registry.profile_states.insert(key, state.clone());
    save_registry(&registry)?;
    // Drop stale probe cache for the previous shared binding so the next probe
    // is binding-specific. Keep other profiles untouched.
    clear_profile_runtime_probe_states(&plugin_id, &profile_id, &backend)?;
    Ok(state)
}

#[tauri::command]
pub fn check_ai_plugin_model_bindings(
    plugin_id: String,
    binding_id: String,
    dir_path: String,
) -> Result<AiPluginModelBindingCheck, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let binding = manifest
        .model_bindings
        .iter()
        .find(|b| b.id == binding_id)
        .ok_or_else(|| {
            format!(
                "Model binding '{}' not found for plugin '{}'",
                binding_id, plugin_id
            )
        })?;
    let dir = Path::new(&dir_path);
    if !dir.is_dir() {
        return Err(format!("Directory does not exist: {}", dir_path));
    }
    let canonical = dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve directory: {}", e))?;
    Ok(check_model_binding(binding, &canonical))
}

/// Re-check the managed plugin model directory against the declared `models[]`
/// entries. Used by Settings "open model folder & validate" without changing
/// bindings.
#[tauri::command]
pub fn check_ai_plugin_model_files(
    plugin_id: String,
) -> Result<Vec<AiPluginModelFileSummary>, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    plugin_model_file_summaries(&manifest)
}

/// Copy user-selected model files into the managed plugin model directory.
///
/// Matching is filename-based against declared `models[].path` basenames so
/// users can import checkpoints without recreating nested folders manually.
/// Only paths that resolve inside `plugin-data/<id>/models` are written.
/// Unmatched selections are returned and not copied.
#[tauri::command]
pub fn import_ai_plugin_model_files(
    plugin_id: String,
    source_paths: Vec<String>,
) -> Result<AiPluginModelImportResult, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    if source_paths.is_empty() {
        return Err("No model files selected".to_string());
    }
    // Ensure drop folders / README exist before writing.
    prepare_installed_plugin_storage(&manifest)?;
    let model_dir = plugin_model_dir(&plugin_id)?;
    let model_dir_canon = model_dir.canonicalize().unwrap_or(model_dir.clone());

    // Map basename (lowercase) -> first declared model that needs that file.
    let mut by_basename: HashMap<String, (String, String, PathBuf)> = HashMap::new();
    for model in &manifest.models {
        let Some(relative) = plugin_model_relative_path(model) else {
            continue;
        };
        let Some(file_name) = relative.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let id = model
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("model")
            .to_string();
        let name = model
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&id)
            .to_string();
        by_basename
            .entry(file_name.to_ascii_lowercase())
            .or_insert((id, name, relative));
    }

    let mut imported = Vec::new();
    let mut unmatched = Vec::new();
    for source in source_paths {
        let source_path = PathBuf::from(&source);
        if !source_path.is_file() {
            unmatched.push(source);
            continue;
        }
        let Some(file_name) = source_path.file_name().and_then(|n| n.to_str()) else {
            unmatched.push(source);
            continue;
        };
        let Some((model_id, model_name, relative)) =
            by_basename.get(&file_name.to_ascii_lowercase())
        else {
            unmatched.push(source);
            continue;
        };
        let target = model_dir.join(relative);
        let target_parent = target.parent().unwrap_or(model_dir.as_path());
        fs::create_dir_all(target_parent).map_err(|e| {
            format!(
                "Failed to create model directory '{}': {}",
                target_parent.display(),
                e
            )
        })?;
        // Containment: refuse to write outside the managed model root.
        let parent_canon = target_parent
            .canonicalize()
            .unwrap_or(target_parent.to_path_buf());
        if !is_path_inside(&parent_canon, &model_dir_canon) && parent_canon != model_dir_canon {
            return Err(format!(
                "Refusing to import outside plugin model directory: {}",
                target.display()
            ));
        }
        fs::copy(&source_path, &target).map_err(|e| {
            format!(
                "Failed to copy '{}' to '{}': {}",
                source_path.display(),
                target.display(),
                e
            )
        })?;
        imported.push(AiPluginModelImportItem {
            model_id: model_id.clone(),
            model_name: model_name.clone(),
            source_path: normalize_path(&source_path),
            target_path: normalize_path(&target),
        });
    }

    Ok(AiPluginModelImportResult {
        plugin_id,
        model_dir: normalize_path(&model_dir),
        imported,
        unmatched,
        model_files: plugin_model_file_summaries(&manifest)?,
    })
}

#[tauri::command]
pub fn list_ai_plugins() -> Result<Vec<AiPluginSummary>, String> {
    let mut registry = load_registry()?;
    let manifest_paths = discover_manifest_paths()?;
    let mut duplicate_counts: HashMap<String, usize> = HashMap::new();
    let mut loaded = Vec::new();
    let mut registry_changed = false;

    for manifest_path in manifest_paths {
        match read_manifest(&manifest_path) {
            Ok(manifest) => {
                if !manifest.id.is_empty() {
                    *duplicate_counts.entry(manifest.id.clone()).or_default() += 1;
                }
                loaded.push((manifest_path, Some(manifest), None));
            }
            Err(error) => loaded.push((manifest_path, None, Some(error))),
        }
    }

    for (_, manifest, _) in &loaded {
        if let Some(manifest) = manifest {
            if !manifest.id.is_empty() {
                registry_changed |=
                    cleanup_plugin_task_cache_with_registry(&manifest.id, &mut registry)?;
            }
        }
    }
    if registry_changed {
        save_registry(&registry)?;
    }

    let profile_states = registry.profile_states;
    let permission_grants = registry.permission_grants;
    let setup_jobs = registry.setup_jobs;
    let runtime_probe_states = registry.runtime_probe_states;
    let task_states = registry.task_states;

    let mut summaries = Vec::new();
    for (manifest_path, manifest, error) in loaded {
        if let Some(manifest) = manifest {
            let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));
            let mut validation = validate_manifest(&manifest, root);
            if duplicate_counts.get(&manifest.id).copied().unwrap_or(0) > 1 {
                validation.valid = false;
                validation
                    .errors
                    .push(format!("Duplicate plugin id '{}'", manifest.id));
            }
            let permission_grant = permission_grants
                .get(&permission_grant_key(&manifest.id))
                .cloned();
            summaries.push(manifest_to_summary(
                &manifest_path,
                manifest,
                validation,
                permission_grant,
                &profile_states,
                &setup_jobs,
                &runtime_probe_states,
                &task_states,
            ));
        } else {
            let error = error.unwrap_or_else(|| "Failed to read manifest".to_string());
            let root_path = manifest_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default();
            summaries.push(AiPluginSummary {
                id: String::new(),
                name: String::new(),
                version: String::new(),
                publisher: None,
                path: normalize_path(&root_path),
                manifest_path: normalize_path(&manifest_path),
                platform_supported: false,
                validation: PluginValidationReport {
                    valid: false,
                    errors: vec![error],
                    warnings: Vec::new(),
                },
                permissions: AiPluginPermissionsSummary::default(),
                permission_grant: None,
                runtimes: Vec::new(),
                runtime: None,
                entry: None,
                install: None,
                storage: plugin_storage_summary("", &root_path),
                install_profiles: Vec::new(),
                smoke_test: None,
                capabilities: Vec::new(),
                contributes: PluginContributesSummary::default(),
                task_states: Vec::new(),
                model_bindings: Vec::new(),
                model_files: Vec::new(),
            });
        }
    }

    summaries.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| a.manifest_path.cmp(&b.manifest_path))
    });
    Ok(summaries)
}

#[tauri::command]
pub async fn get_ai_plugin_status(
    plugin_id: String,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginStatus, String> {
    get_ai_plugin_status_runtime(plugin_id, Some(&state)).await
}

async fn get_ai_plugin_status_runtime(
    plugin_id: String,
    state: Option<&AiPluginRuntimeState>,
) -> Result<AiPluginStatus, String> {
    let (_manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    let Some(entry) = manifest.entry else {
        return Ok(ai_plugin_status(
            plugin_id,
            false,
            false,
            None,
            None,
            Some("Plugin has no entry".to_string()),
        ));
    };

    if entry.kind != "local-http" {
        return Ok(ai_plugin_status(
            plugin_id,
            false,
            false,
            None,
            None,
            Some(format!(
                "Status probing is not supported for '{}'",
                entry.kind
            )),
        ));
    }

    let Some(base_url) = plugin_base_url(&plugin_id, &entry, state).await else {
        return Ok(ai_plugin_status(
            plugin_id,
            false,
            false,
            None,
            None,
            Some("Plugin has no baseUrl".to_string()),
        ));
    };

    let managed = runtime_base_url(state, &plugin_id).await.is_some();
    let token = if managed {
        match state {
            Some(s) => runtime_auth_token(s, &plugin_id).await,
            None => None,
        }
    } else {
        None
    };
    probe_ai_plugin_status(plugin_id, &entry, base_url, managed, token.as_deref()).await
}

async fn probe_ai_plugin_status(
    plugin_id: String,
    entry: &PluginEntry,
    base_url: String,
    managed: bool,
    token: Option<&str>,
) -> Result<AiPluginStatus, String> {
    let url = join_url(&base_url, &status_path_for(&entry));
    let client = plugin_http_client(token, 1200)?;

    match client.get(&url).send().await {
        Ok(response) => {
            let status_code = response.status();
            match response.json::<Value>().await {
                Ok(value)
                    if status_code.is_success() && !plugin_status_matches(&plugin_id, &value) =>
                {
                    Ok(ai_plugin_status(
                        plugin_id,
                        false,
                        managed,
                        Some(url),
                        Some(value),
                        Some("Status endpoint belongs to a different plugin".to_string()),
                    ))
                }
                Ok(value) if status_code.is_success() => Ok(ai_plugin_status(
                    plugin_id,
                    true,
                    managed,
                    Some(url),
                    Some(value),
                    None,
                )),
                Ok(value) => Ok(ai_plugin_status(
                    plugin_id,
                    false,
                    managed,
                    Some(url),
                    Some(value),
                    Some(format!("Status endpoint returned {}", status_code)),
                )),
                Err(error) => Ok(ai_plugin_status(
                    plugin_id,
                    false,
                    managed,
                    Some(url),
                    None,
                    Some(format!("Failed to parse status response: {}", error)),
                )),
            }
        }
        Err(error) => Ok(ai_plugin_status(
            plugin_id,
            false,
            managed,
            Some(url),
            None,
            Some(error.to_string()),
        )),
    }
}

fn plugin_status_matches(plugin_id: &str, value: &Value) -> bool {
    value
        .get("pluginId")
        .and_then(Value::as_str)
        .map(|id| id == plugin_id)
        .unwrap_or(true)
}

#[tauri::command]
pub async fn get_ai_plugin_diagnostics(
    plugin_id: String,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginDiagnostics, String> {
    get_ai_plugin_diagnostics_runtime(plugin_id, Some(&state)).await
}

async fn get_ai_plugin_diagnostics_runtime(
    plugin_id: String,
    state: Option<&AiPluginRuntimeState>,
) -> Result<AiPluginDiagnostics, String> {
    let (_manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    let Some(entry) = manifest.entry else {
        return Ok(AiPluginDiagnostics {
            plugin_id,
            reachable: false,
            url: None,
            diagnostics: None,
            error: Some("Plugin has no entry".to_string()),
        });
    };

    if entry.kind != "local-http" {
        return Ok(AiPluginDiagnostics {
            plugin_id,
            reachable: false,
            url: None,
            diagnostics: None,
            error: Some(format!(
                "Diagnostics are not supported for '{}'",
                entry.kind
            )),
        });
    }

    let Some(base_url) = plugin_base_url(&plugin_id, &entry, state).await else {
        return Ok(AiPluginDiagnostics {
            plugin_id,
            reachable: false,
            url: None,
            diagnostics: None,
            error: Some("Plugin has no baseUrl".to_string()),
        });
    };

    let url = join_url(&base_url, "/diagnostics");
    let token = match state {
        Some(s) => runtime_auth_token(s, &plugin_id).await,
        None => None,
    };
    let client = plugin_http_client(token.as_deref(), PLUGIN_DIAGNOSTICS_TIMEOUT_MS)?;

    match client.get(&url).send().await {
        Ok(response) => {
            let status_code = response.status();
            match response.json::<Value>().await {
                Ok(value) if status_code.is_success() => Ok(AiPluginDiagnostics {
                    plugin_id,
                    reachable: true,
                    url: Some(url),
                    diagnostics: Some(value),
                    error: None,
                }),
                Ok(value) => Ok(AiPluginDiagnostics {
                    plugin_id,
                    reachable: false,
                    url: Some(url),
                    diagnostics: Some(value),
                    error: Some(format!("Diagnostics endpoint returned {}", status_code)),
                }),
                Err(error) => Ok(AiPluginDiagnostics {
                    plugin_id,
                    reachable: false,
                    url: Some(url),
                    diagnostics: None,
                    error: Some(format!("Failed to parse diagnostics response: {}", error)),
                }),
            }
        }
        Err(error) => Ok(AiPluginDiagnostics {
            plugin_id,
            reachable: false,
            url: Some(url),
            diagnostics: None,
            error: Some(error.to_string()),
        }),
    }
}

#[tauri::command]
pub fn mark_ai_plugin_profile_setup_needed(
    plugin_id: String,
    request: AiPluginProfileSetupRequest,
) -> Result<AiPluginProfileState, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;

    let profile = find_manifest_profile(&manifest, &plugin_id, &request.profile_id)?;
    let capability = setup_capability(&manifest, request.capability)?;
    let runtime_binding = selected_runtime_binding(
        profile,
        request.runtime_binding_id.as_deref(),
        request.runtime_binding.clone(),
    )?;
    let effective_profile = profile_with_runtime_binding(profile, runtime_binding.clone());
    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let mut job = new_setup_job(
        &plugin_id,
        &effective_profile,
        &request.backend,
        &capability,
        root,
        "Preparing local runtime setup artifacts. No dependency installation will be executed yet.",
    );
    match prepare_profile_local_artifacts(root, &effective_profile, &mut job) {
        Ok(()) => {
            job.status = "needsVerify".to_string();
            job.progress = 100;
            job.updated_at = Utc::now().to_rfc3339();
            job.message =
                Some("Runtime setup artifacts are ready. Run Verify or Smoke next.".to_string());
            job.push_log(
                "Dependency installation is not implemented yet; no setup command was executed."
                    .to_string(),
            );
            job.push_log("Run Verify or Smoke to validate the existing runtime.".to_string());
        }
        Err(error) => {
            job.status = "failed".to_string();
            job.progress = 100;
            job.updated_at = Utc::now().to_rfc3339();
            job.message = Some("Runtime setup artifact preparation failed.".to_string());
            job.error = Some(error.clone());
            job.push_log(format!("Setup failed: {}", error));
        }
    }
    save_setup_job(job.clone())?;

    let state = AiPluginProfileState {
        plugin_id: plugin_id.clone(),
        profile_id: request.profile_id.clone(),
        backend: request.backend,
        capability,
        status: if job.status == "failed" {
            "failed".to_string()
        } else {
            "needsVerify".to_string()
        },
        verified: false,
        updated_at: job.updated_at.clone(),
        setup_attempted: true,
        setup_job_id: Some(job.id.clone()),
        duration_ms: None,
        error: job.error.clone(),
        result: None,
        runtime_binding,
        model_dir_bindings: persisted_model_dir_bindings(&plugin_id, &request.profile_id),
    };
    save_profile_state(state.clone())?;
    clear_profile_runtime_probe_states(&plugin_id, &state.profile_id, &state.backend)?;
    Ok(state)
}

#[tauri::command]
pub async fn cancel_ai_plugin_setup(
    job_id: String,
    cancel_state: tauri::State<'_, SetupCancellationState>,
) -> Result<(), String> {
    let registry = load_registry()?;
    let job = registry
        .setup_jobs
        .get(&job_id)
        .ok_or_else(|| format!("Setup job '{}' was not found", job_id))?;
    if job.status != "running" {
        return Err(format!(
            "Setup job '{}' cannot be cancelled from status '{}'",
            job_id, job.status
        ));
    }
    cancel_state.request_cancel(&job_id).await;
    Ok(())
}

#[tauri::command]
pub fn preview_ai_plugin_profile_setup_command(
    plugin_id: String,
    request: AiPluginProfileSetupRequest,
) -> Result<AiPluginSetupPreview, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let profile = find_manifest_profile(&manifest, &plugin_id, &request.profile_id)?;
    let capability = setup_capability(&manifest, request.capability)?;
    let runtime_binding = selected_runtime_binding(
        profile,
        request.runtime_binding_id.as_deref(),
        request.runtime_binding.clone(),
    )?;
    let effective_profile = profile_with_runtime_binding(profile, runtime_binding.clone());
    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let command = manifest
        .install
        .as_ref()
        .and_then(|install| install.command.as_deref())
        .ok_or_else(|| "Plugin has no setup command".to_string())?;

    Ok(build_setup_preview(
        &plugin_id,
        root,
        command,
        &effective_profile,
        &request.backend,
        &capability,
        runtime_binding,
        &manifest.model_bindings,
    ))
}

#[tauri::command]
pub async fn run_ai_plugin_profile_setup_command(
    plugin_id: String,
    request: AiPluginProfileSetupRequest,
    cancel_state: tauri::State<'_, SetupCancellationState>,
) -> Result<AiPluginProfileState, String> {
    if !request.allow_command_execution {
        return Err("Setup command execution requires explicit confirmation".to_string());
    }

    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let profile = find_manifest_profile(&manifest, &plugin_id, &request.profile_id)?;
    let capability = setup_capability(&manifest, request.capability)?;
    let runtime_binding = selected_runtime_binding(
        profile,
        request.runtime_binding_id.as_deref(),
        request.runtime_binding.clone(),
    )?;
    let effective_profile = profile_with_runtime_binding(profile, runtime_binding.clone());
    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let command = manifest
        .install
        .as_ref()
        .and_then(|install| install.command.as_deref())
        .ok_or_else(|| "Plugin has no setup command".to_string())?;

    let mut job = new_setup_job(
        &plugin_id,
        &effective_profile,
        &request.backend,
        &capability,
        root,
        "Running plugin setup command.",
    );
    save_setup_job(job.clone())?;
    save_profile_state(AiPluginProfileState {
        plugin_id: plugin_id.clone(),
        profile_id: request.profile_id.clone(),
        backend: request.backend.clone(),
        capability: capability.clone(),
        status: "running".to_string(),
        verified: false,
        updated_at: job.updated_at.clone(),
        setup_attempted: true,
        setup_job_id: Some(job.id.clone()),
        duration_ms: None,
        error: None,
        result: None,
        runtime_binding: runtime_binding.clone(),
        model_dir_bindings: persisted_model_dir_bindings(&plugin_id, &request.profile_id),
    })?;

    let status: String;
    let mut error: Option<String> = None;
    match prepare_profile_local_artifacts(root, &effective_profile, &mut job) {
        Ok(()) => match run_setup_command(
            root,
            command,
            &plugin_id,
            &effective_profile,
            &request.backend,
            &capability,
            &mut job,
            Some(&cancel_state),
            &manifest.model_bindings,
        )
        .await
        {
            Ok(SetupCommandOutcome::Completed) => {
                status = "needsVerify".to_string();
                job.status = "needsVerify".to_string();
                job.progress = 100;
                job.message =
                    Some("Setup command completed. Run Verify or Smoke next.".to_string());
                job.push_log("Setup command completed successfully.".to_string());
                job.push_log("Run Verify or Smoke to validate this profile.".to_string());
            }
            Ok(SetupCommandOutcome::Cancelled) => {
                status = "cancelled".to_string();
                job.status = "cancelled".to_string();
                job.progress = 100;
                job.message = Some("Setup command was cancelled.".to_string());
                job.error = None;
            }
            Err(command_error) => {
                status = "failed".to_string();
                error = Some(command_error.clone());
                job.status = "failed".to_string();
                job.progress = 100;
                job.message = Some("Setup command failed.".to_string());
                job.error = Some(command_error.clone());
                job.push_log(format!("Setup command failed: {}", command_error));
            }
        },
        Err(artifact_error) => {
            status = "failed".to_string();
            error = Some(artifact_error.clone());
            job.status = "failed".to_string();
            job.progress = 100;
            job.message = Some("Runtime setup artifact preparation failed.".to_string());
            job.error = Some(artifact_error.clone());
            job.push_log(format!("Setup failed: {}", artifact_error));
        }
    }
    job.updated_at = Utc::now().to_rfc3339();
    append_setup_log(&plugin_id, &job)?;
    save_setup_job(job.clone())?;

    let state = AiPluginProfileState {
        plugin_id: plugin_id.clone(),
        profile_id: request.profile_id.clone(),
        backend: request.backend,
        capability,
        status,
        verified: false,
        updated_at: job.updated_at.clone(),
        setup_attempted: true,
        setup_job_id: Some(job.id.clone()),
        duration_ms: None,
        error,
        result: None,
        runtime_binding,
        model_dir_bindings: persisted_model_dir_bindings(&plugin_id, &request.profile_id),
    };
    save_profile_state(state.clone())?;
    clear_profile_runtime_probe_states(&plugin_id, &state.profile_id, &state.backend)?;
    Ok(state)
}

#[tauri::command]
pub async fn smoke_test_ai_plugin(
    plugin_id: String,
    request: AiPluginSmokeTestRequest,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginSmokeTestResult, String> {
    let start = tokio::time::Instant::now();
    let start_status = start_ai_plugin(
        plugin_id.clone(),
        Some(AiPluginStartRequest {
            profile_id: Some(request.profile_id.clone()),
            backend: Some(request.backend.clone()),
            capability: Some(request.capability.clone()),
            runtime_binding_id: request.runtime_binding_id.clone(),
            runtime_binding: request.runtime_binding.clone(),
        }),
        state.clone(),
    )
    .await?;
    if !start_status.reachable {
        let startup_status = start_status.clone();
        return Ok(AiPluginSmokeTestResult {
            plugin_id,
            profile_id: request.profile_id,
            backend: request.backend,
            capability: request.capability,
            reachable: false,
            url: start_status.url.clone(),
            passed: false,
            duration_ms: Some(start.elapsed().as_millis() as u64),
            result: start_status.status.clone(),
            error: start_status
                .error
                .or_else(|| Some("Plugin did not start".to_string())),
            startup_status: Some(startup_status),
        });
    }

    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;

    let Some(entry) = manifest.entry.as_ref() else {
        return Err("Plugin has no entry".to_string());
    };
    if entry.kind != "local-http" {
        return Err(format!(
            "Smoke tests are not supported for '{}'",
            entry.kind
        ));
    }

    find_plugin_capability(&manifest, &request.capability)?;

    let profile = if request.profile_id.trim().is_empty() {
        None
    } else {
        Some(find_manifest_profile(
            &manifest,
            &plugin_id,
            &request.profile_id,
        )?)
    };
    let runtime_binding = match profile {
        Some(profile) => selected_runtime_binding(
            profile,
            request.runtime_binding_id.as_deref(),
            request.runtime_binding.clone(),
        )?,
        None => None,
    };

    let Some(base_url) = plugin_base_url(&plugin_id, entry, Some(&state)).await else {
        return Ok(AiPluginSmokeTestResult {
            plugin_id,
            profile_id: request.profile_id,
            backend: request.backend,
            capability: request.capability,
            reachable: false,
            url: None,
            passed: false,
            duration_ms: Some(start.elapsed().as_millis() as u64),
            result: None,
            error: Some("Plugin has no baseUrl".to_string()),
            startup_status: None,
        });
    };

    let url = join_url(&base_url, "/smoke-test");
    let timeout_ms = manifest
        .smoke_test
        .as_ref()
        .and_then(|smoke_test| smoke_test.timeout_ms)
        .unwrap_or(PLUGIN_SMOKE_TEST_TIMEOUT_MS);
    let client = plugin_http_client(
        runtime_auth_token(&state, &plugin_id).await.as_deref(),
        timeout_ms,
    )?;

    let payload = serde_json::json!({
        "profileId": request.profile_id,
        "backend": request.backend,
        "capability": request.capability,
        "runtimeBinding": runtime_binding.clone(),
    });

    match client.post(&url).json(&payload).send().await {
        Ok(response) => {
            let status_code = response.status();
            match response.json::<Value>().await {
                Ok(value) => {
                    let passed = status_code.is_success()
                        && value
                            .get("passed")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                    let duration_ms = value
                        .get("durationMs")
                        .and_then(Value::as_u64)
                        .or_else(|| Some(start.elapsed().as_millis() as u64));
                    let error = if passed {
                        None
                    } else {
                        value
                            .get("error")
                            .and_then(|error| {
                                error
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .or_else(|| error.as_str())
                            })
                            .map(|error| error.to_string())
                            .or_else(|| {
                                Some(format!("Smoke test endpoint returned {}", status_code))
                            })
                    };
                    let state = AiPluginProfileState {
                        plugin_id: plugin_id.clone(),
                        profile_id: request.profile_id.clone(),
                        backend: request.backend.clone(),
                        capability: request.capability.clone(),
                        status: if passed { "verified" } else { "failed" }.to_string(),
                        verified: passed,
                        updated_at: Utc::now().to_rfc3339(),
                        setup_attempted: true,
                        setup_job_id: load_registry().ok().and_then(|registry| {
                            registry
                                .profile_states
                                .get(&profile_state_key(&plugin_id, &request.profile_id))
                                .and_then(|state| state.setup_job_id.clone())
                        }),
                        duration_ms,
                        error: error.clone(),
                        result: Some(value.clone()),
                        runtime_binding: runtime_binding.clone(),
                        model_dir_bindings: persisted_model_dir_bindings(
                            &plugin_id,
                            &request.profile_id,
                        ),
                    };
                    save_profile_state(state)?;

                    Ok(AiPluginSmokeTestResult {
                        plugin_id,
                        profile_id: request.profile_id,
                        backend: request.backend,
                        capability: request.capability,
                        reachable: status_code.is_success(),
                        url: Some(url),
                        passed,
                        duration_ms,
                        result: Some(value),
                        error,
                        startup_status: None,
                    })
                }
                Err(error) => Ok(AiPluginSmokeTestResult {
                    plugin_id,
                    profile_id: request.profile_id,
                    backend: request.backend,
                    capability: request.capability,
                    reachable: false,
                    url: Some(url),
                    passed: false,
                    duration_ms: Some(start.elapsed().as_millis() as u64),
                    result: None,
                    error: Some(format!("Failed to parse smoke test response: {}", error)),
                    startup_status: None,
                }),
            }
        }
        Err(error) => Ok(AiPluginSmokeTestResult {
            plugin_id,
            profile_id: request.profile_id,
            backend: request.backend,
            capability: request.capability,
            reachable: false,
            url: Some(url),
            passed: false,
            duration_ms: Some(start.elapsed().as_millis() as u64),
            result: None,
            error: Some(error.to_string()),
            startup_status: None,
        }),
    }
}

fn read_log_tail(path: &Path) -> Result<AiPluginLogFile, String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Failed to read log metadata '{}': {}", path.display(), e))?;
    let bytes = metadata.len();
    let mut file = fs::File::open(path)
        .map_err(|e| format!("Failed to open log file '{}': {}", path.display(), e))?;
    let start = bytes.saturating_sub(PLUGIN_LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| format!("Failed to seek log file '{}': {}", path.display(), e))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read log file '{}': {}", path.display(), e))?;
    let content = String::from_utf8_lossy(&buffer).to_string();

    Ok(AiPluginLogFile {
        path: normalize_path(path),
        name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("plugin.log")
            .to_string(),
        bytes,
        content,
    })
}

#[tauri::command]
pub fn get_ai_plugin_logs(plugin_id: String) -> Result<AiPluginLogs, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;
    let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));
    let logs_dir = plugin_logs_dir(&plugin_id)?;

    if !logs_dir.exists() && !root.join("logs").exists() {
        return Ok(AiPluginLogs {
            plugin_id,
            files: Vec::new(),
            error: Some("Plugin logs directory does not exist".to_string()),
        });
    }

    let mut candidates = Vec::new();
    for dir in [logs_dir, root.join("logs")] {
        if !dir.exists() {
            continue;
        }
        let entries = fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read plugin logs directory: {}", e))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                candidates.push(path);
            }
        }
    }

    candidates.sort_by(|a, b| {
        let modified_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let modified_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        modified_b
            .cmp(&modified_a)
            .then_with(|| normalize_path(a).cmp(&normalize_path(b)))
    });
    candidates.truncate(3);

    let mut files = Vec::new();
    let mut errors = Vec::new();
    for path in candidates {
        match read_log_tail(&path) {
            Ok(file) => files.push(file),
            Err(error) => errors.push(error),
        }
    }

    Ok(AiPluginLogs {
        plugin_id,
        files,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
    })
}

#[tauri::command]
pub async fn start_ai_plugin(
    plugin_id: String,
    request: Option<AiPluginStartRequest>,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginStatus, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;

    let Some(entry) = manifest.entry.clone() else {
        return Err("Plugin has no entry".to_string());
    };
    if entry.kind != "local-http" {
        return Err(format!(
            "Starting plugins is not supported for '{}'",
            entry.kind
        ));
    }

    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let Some(command) = entry.start_command.as_deref() else {
        return Err("Plugin has no startCommand".to_string());
    };
    if !is_safe_relative_command(command) {
        return Err("startCommand must be a safe relative path".to_string());
    }

    let command_path = root.join(command);
    if !command_path.exists() {
        return Err(format!("startCommand '{}' does not exist", command));
    }
    let start_profile = resolve_start_profile(&manifest, &plugin_id, request.as_ref())?;
    let desired_signature = start_profile_signature(start_profile.as_ref());

    if let Ok(status) = get_ai_plugin_status_runtime(plugin_id.clone(), Some(&state)).await {
        if status.reachable && status.managed {
            let same_signature = {
                let processes = state.processes.lock().await;
                processes
                    .get(&plugin_id)
                    .map(|runtime| runtime.start_signature == desired_signature)
                    .unwrap_or(false)
            };
            if same_signature {
                return Ok(status);
            }
            let _ = stop_ai_plugin_runtime(plugin_id.clone(), &state).await;
        }
    }

    {
        let mut processes = state.processes.lock().await;
        if let Some(runtime) = processes.get_mut(&plugin_id) {
            match runtime.child.try_wait() {
                Ok(None) if runtime.start_signature == desired_signature => {
                    let base_url = runtime.base_url.clone();
                    drop(processes);
                    let start_log_path = plugin_logs_dir(&plugin_id)?.join("start.log");
                    return wait_for_plugin_ready(
                        plugin_id,
                        &entry,
                        base_url,
                        &state,
                        &start_log_path,
                    )
                    .await;
                }
                Ok(None) => {
                    drop(processes);
                    let _ = stop_ai_plugin_runtime(plugin_id.clone(), &state).await;
                }
                Ok(Some(_)) | Err(_) => {
                    // Child already exited or try_wait failed: drop the
                    // RunningPlugin (revokes sandbox ACLs via Drop).
                    drop(processes.remove(&plugin_id));
                }
            }
        }
    }

    let task_id = Uuid::new_v4().to_string();
    let data_dir = plugin_data_dir(&plugin_id)?;
    let cache_dir = plugin_cache_dir(&plugin_id)?;
    let task_dir = plugin_task_temp_dir(&plugin_id, &task_id)?;
    let output_dir = plugin_output_dir(&plugin_id)?;
    let model_dir = plugin_model_dir(&plugin_id)?;
    let config_path = plugin_config_path(&plugin_id)?;
    let runtime_dir = plugin_runtime_root(&plugin_id)?;
    let port = allocate_plugin_port(&plugin_id, &entry, &state).await?;
    let base_url = loopback_base_url(port);
    let logs_dir = plugin_logs_dir(&plugin_id)?;
    let start_log_path = logs_dir.join("start.log");
    let mut start_log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&start_log_path)
        .map_err(|e| format!("Failed to open plugin start log: {}", e))?;
    let _ = writeln!(
        start_log,
        "\n=== {} start {} ===\ncommand: {}\nroot: {}\nbaseUrl: {}\nport: {}\nsignature: {}\n",
        Utc::now().to_rfc3339(),
        plugin_id,
        normalize_path(&command_path),
        normalize_path(root),
        base_url,
        port,
        desired_signature.as_deref().unwrap_or("default")
    );
    let start_log_stdout = start_log
        .try_clone()
        .map_err(|e| format!("Failed to clone plugin start log for stdout: {}", e))?;
    let start_log_stderr = start_log
        .try_clone()
        .map_err(|e| format!("Failed to clone plugin start log for stderr: {}", e))?;

    let auth_token = generate_plugin_auth_token();

    // Apply process sandbox (deny-ACL write confinement) before spawn.
    // The handle's Drop revokes the ACLs when the RunningPlugin is torn down.
    // Writable roots use the Phase 1 allow-list (data/cache/outputs/runtimes/
    // code + shared runtimes + bound model dirs + task/output extras).
    let writable_dirs = plugin_writable_roots(
        &plugin_id,
        root,
        Some(&manifest),
        [
            data_dir.clone(),
            cache_dir.clone(),
            output_dir.clone(),
            model_dir.clone(),
            runtime_dir.clone(),
            task_dir.clone(),
        ],
    )?;
    let sandbox = match t_sandbox::apply_plugin_sandbox(&plugin_id, &writable_dirs).await {
        Ok(handle) => Some(handle),
        Err(e) => {
            // Sandbox failure is non-fatal: log and continue without confinement.
            let _ = writeln!(
                start_log,
                "sandbox: apply failed (continuing unsandboxed): {}",
                e
            );
            None
        }
    };
    if let Some(ref sandbox) = sandbox {
        let _ = writeln!(start_log, "sandbox: {}", sandbox.summary());
    }

    // Phase 3–5: network decision/apply + landlock/env log lines. Registry
    // failures deny network so enforcement never becomes fail-open.
    let runtime_network_granted = resolve_runtime_network_grant(
        plugin_runtime_network_granted(&plugin_id, &manifest),
        &plugin_id,
    );
    let network_sandbox =
        t_sandbox::apply_network_sandbox(&plugin_id, &command_path, runtime_network_granted);
    let _ = writeln!(start_log, "{}", network_sandbox.summary());
    for line in t_sandbox::experimental_confinement_log_lines(runtime_network_granted) {
        // Final network/landlock status is logged from apply_* helpers.
        if line.starts_with("network_os:") || line.starts_with("landlock:") {
            continue;
        }
        let _ = writeln!(start_log, "{}", line);
    }
    let network_policy = network_sandbox
        .status()
        .map(|s| s.policy_env_value())
        .unwrap_or("unrestricted");

    let mut cmd = Command::new(&command_path);
    cmd.current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(start_log_stdout))
        .stderr(Stdio::from(start_log_stderr))
        .kill_on_drop(false);

    // Phase 5: rebuild ambient env from allowlist BEFORE injecting PICAIPIC_*.
    // env_clear after injection would wipe host-provided plugin vars.
    let hygiene = t_sandbox::apply_env_hygiene(&mut cmd);
    let _ = writeln!(start_log, "env_hygiene: {}", hygiene.summary());
    // Phase 4: Linux Landlock (opt-in). RO plugin root + system prefixes; RW writable roots.
    // Writable roots already include shared/plugin runtimes; add plugin code root as RO extra.
    let landlock_ro = vec![root.to_path_buf()];
    let landlock = t_sandbox::apply_linux_landlock(&mut cmd, &writable_dirs, &landlock_ro);
    let _ = writeln!(start_log, "{}", landlock.summary());

    cmd.env("PICAIPIC_PLUGIN_ID", &plugin_id)
        .env("PICAIPIC_PLUGIN_ROOT", root)
        .env("PICAIPIC_PLUGIN_DATA_DIR", &data_dir)
        .env("PICAIPIC_PLUGIN_CACHE_DIR", &cache_dir)
        .env("PICAIPIC_PLUGIN_LOG_DIR", &logs_dir)
        .env("PICAIPIC_PLUGIN_MODEL_DIR", &model_dir)
        .env("PICAIPIC_PLUGIN_CONFIG_PATH", &config_path)
        .env("PICAIPIC_PLUGIN_RUNTIME_DIR", &runtime_dir)
        .env("PICAIPIC_TASK_TEMP_DIR", &task_dir)
        .env("PICAIPIC_OUTPUT_DIR", &output_dir)
        .env("PICAIPIC_PLUGIN_NETWORK_POLICY", network_policy);

    cmd.env("PICAIPIC_PLUGIN_PORT", port.to_string())
        .env("PICAIPIC_PLUGIN_BASE_URL", &base_url)
        .env("PICAIPIC_PLUGIN_AUTH_TOKEN", &auth_token);
    if let Some((profile, backend, capability)) = start_profile.as_ref() {
        for (key, value) in build_setup_environment(
            root,
            &plugin_id,
            profile,
            backend,
            capability,
            &manifest.model_bindings,
        )? {
            cmd.env(key, value);
        }
    }
    hide_command_window(&mut cmd);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start plugin '{}': {}", plugin_id, e))?;

    {
        let mut processes = state.processes.lock().await;
        processes.insert(
            plugin_id.clone(),
            RunningPlugin {
                child,
                port,
                base_url: base_url.clone(),
                start_signature: desired_signature,
                auth_token: auth_token.clone(),
                sandbox,
                network_sandbox: Some(network_sandbox),
            },
        );
    }

    wait_for_plugin_ready(plugin_id, &entry, base_url, &state, &start_log_path).await
}

async fn wait_for_plugin_ready(
    plugin_id: String,
    entry: &PluginEntry,
    base_url: String,
    state: &AiPluginRuntimeState,
    start_log_path: &Path,
) -> Result<AiPluginStatus, String> {
    let url = join_url(&base_url, &health_path_for(entry));
    let deadline = tokio::time::Instant::now() + Duration::from_millis(PLUGIN_STARTUP_TIMEOUT_MS);
    let client = plugin_http_client(None, 1200)?;

    let mut last_error: Option<String>;
    loop {
        match client.get(&url).send().await {
            Ok(response) => {
                let status_code = response.status();
                match response.json::<Value>().await {
                    Ok(value) if status_code.is_success() => {
                        if health_ready(entry, &value) {
                            let token = runtime_auth_token(state, &plugin_id).await;
                            return probe_ai_plugin_status(
                                plugin_id,
                                entry,
                                base_url,
                                true,
                                token.as_deref(),
                            )
                            .await;
                        }
                        last_error = Some("Plugin health endpoint is not ready".to_string());
                    }
                    Ok(value) => {
                        last_error = Some(format!(
                            "Health endpoint returned {}: {}",
                            status_code, value
                        ));
                    }
                    Err(error) => {
                        last_error = Some(format!("Failed to parse health response: {}", error));
                    }
                }
            }
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }

        if let Some(status) = take_exited_plugin_status(&plugin_id, state).await {
            return Ok(plugin_startup_failure_status(
                plugin_id,
                Some(url),
                Some(status),
                last_error,
                start_log_path,
            ));
        }

        if tokio::time::Instant::now() >= deadline {
            return Ok(plugin_startup_failure_status(
                plugin_id,
                Some(url),
                None,
                last_error.or_else(|| Some("Plugin did not become ready".to_string())),
                start_log_path,
            ));
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

async fn take_exited_plugin_status(
    plugin_id: &str,
    state: &AiPluginRuntimeState,
) -> Option<std::process::ExitStatus> {
    let mut processes = state.processes.lock().await;
    let runtime = processes.get_mut(plugin_id)?;
    match runtime.child.try_wait() {
        Ok(Some(status)) => {
            // Removing drops RunningPlugin, which drops the sandbox handle
            // and revokes the deny-ACLs applied at start.
            drop(processes.remove(plugin_id));
            Some(status)
        }
        Ok(None) => None,
        Err(_) => {
            drop(processes.remove(plugin_id));
            None
        }
    }
}

fn plugin_startup_failure_status(
    plugin_id: String,
    url: Option<String>,
    exit_status: Option<std::process::ExitStatus>,
    last_error: Option<String>,
    start_log_path: &Path,
) -> AiPluginStatus {
    let log_tail = if start_log_path.exists() {
        read_log_tail(start_log_path).ok()
    } else {
        None
    };
    let log_content = log_tail
        .as_ref()
        .map(|log| log.content.clone())
        .unwrap_or_default();
    let exit_description = exit_status
        .as_ref()
        .map(|status| status.to_string())
        .unwrap_or_else(|| "still running but health check timed out".to_string());
    let error = if exit_status.is_some() {
        format!(
            "Plugin start command exited before the health endpoint became ready ({})",
            exit_description
        )
    } else {
        last_error
            .clone()
            .unwrap_or_else(|| "Plugin did not become ready before startup timeout".to_string())
    };
    let mut status = ai_plugin_status(plugin_id, false, true, url, None, Some(error));
    status.error_code = Some(
        if exit_status.is_some() {
            "start_command_exited"
        } else {
            "startup_timeout"
        }
        .to_string(),
    );
    status.error_domain = Some("runtime".to_string());
    status.error_details = Some(serde_json::json!({
        "startLogPath": normalize_path(start_log_path),
        "exitStatus": exit_status.as_ref().map(|status| status.to_string()),
        "lastHealthError": last_error,
    }));
    status.log_tail = log_tail;
    status.advice = startup_failure_advice(&log_content);
    status
}

fn startup_failure_advice(log_content: &str) -> Vec<String> {
    let lower = log_content.to_lowercase();
    let mut advice = Vec::new();

    if lower.contains("syntax of the command is incorrect")
        || lower.contains("was unexpected at this time")
    {
        advice.push(
            "Check the plugin .bat syntax and make sure the file is saved with Windows CRLF line endings."
                .to_string(),
        );
    }
    if lower.contains("no module named")
        || lower.contains("modulenotfounderror")
        || lower.contains("importerror")
    {
        advice.push(
            "Check the selected Python runtime. It may be missing torch or another package required by this plugin."
                .to_string(),
        );
    }
    if lower.contains("torch") && (lower.contains("cuda") || lower.contains("rocm")) {
        advice.push(
            "Verify that the selected runtime binding matches this machine's GPU backend and PyTorch build."
                .to_string(),
        );
    }
    if lower.contains("model")
        || lower.contains(".pth")
        || lower.contains(".ckpt")
        || lower.contains("filenotfound")
        || lower.contains("no such file")
    {
        advice.push(
            "Check model/source paths and keep machine-specific overrides in the plugin .local.env file."
                .to_string(),
        );
    }

    advice.push("Open the plugin logs and inspect start.log for the command output.".to_string());
    advice.push("Run Probe, then Smoke again after fixing the runtime or .local.env.".to_string());

    advice.dedup();
    advice
}

fn health_ready(entry: &PluginEntry, value: &Value) -> bool {
    let Some(health) = &entry.health else {
        return true;
    };
    let Some(field) = health.ready_field.as_deref() else {
        return true;
    };
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

#[tauri::command]
pub async fn stop_ai_plugin(
    plugin_id: String,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginStatus, String> {
    stop_ai_plugin_runtime(plugin_id, &state).await
}

async fn stop_ai_plugin_runtime(
    plugin_id: String,
    state: &AiPluginRuntimeState,
) -> Result<AiPluginStatus, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));
    let entry = manifest.entry.clone();
    let tracked_runtime = {
        let processes = state.processes.lock().await;
        processes.get(&plugin_id).map(|runtime| {
            (
                runtime.port,
                runtime.base_url.clone(),
                runtime.auth_token.clone(),
            )
        })
    };

    if let Some(entry) = &entry {
        if let Some(command) = entry.stop_command.as_deref() {
            if !is_safe_relative_command(command) {
                return Err("stopCommand must be a safe relative path".to_string());
            }
            let command_path = root.join(command);
            if command_path.exists() {
                let mut cmd = Command::new(command_path);
                cmd.current_dir(root)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                if let Some((port, base_url, token)) = &tracked_runtime {
                    cmd.env("PICAIPIC_PLUGIN_PORT", port.to_string())
                        .env("PICAIPIC_PLUGIN_BASE_URL", base_url)
                        .env("PICAIPIC_PLUGIN_AUTH_TOKEN", token);
                }
                hide_command_window(&mut cmd);
                let _ = tokio::time::timeout(
                    Duration::from_millis(PLUGIN_PROCESS_KILL_TIMEOUT_MS),
                    cmd.output(),
                )
                .await;
            }
        }
    }

    let mut processes = state.processes.lock().await;
    let runtime = processes.remove(&plugin_id);
    drop(processes);

    let runtime_port = runtime.as_ref().map(|runtime| runtime.port);
    let runtime_base_url = runtime.as_ref().map(|runtime| runtime.base_url.clone());
    let had_managed_runtime = runtime.is_some();
    if let Some(mut runtime) = runtime {
        terminate_child_process_tree(&mut runtime.child).await;
    }

    if let Some(entry) = &entry {
        // Only reap a port the host actually spawned. Falling back to the manifest
        // port for an unmanaged plugin would kill unrelated processes that happen to
        // listen there, e.g. when uninstalling a plugin that was never started.
        if had_managed_runtime {
            if let Some(port) = runtime_port.or_else(|| plugin_port(entry)) {
                kill_processes_listening_on_port(port).await;
            }
        }
        return wait_for_plugin_stopped(plugin_id, entry, runtime_base_url).await;
    }

    get_ai_plugin_status_runtime(plugin_id, None).await
}

async fn terminate_child_process_tree(child: &mut Child) {
    #[cfg(target_os = "windows")]
    if let Some(pid) = child.id() {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_command_window(&mut cmd);
        let _ = tokio::time::timeout(
            Duration::from_millis(PLUGIN_PROCESS_KILL_TIMEOUT_MS),
            cmd.output(),
        )
        .await;
    }

    let _ = tokio::time::timeout(
        Duration::from_millis(PLUGIN_PROCESS_KILL_TIMEOUT_MS),
        async {
            let _ = child.kill().await;
            let _ = child.wait().await;
        },
    )
    .await;
}

async fn kill_processes_listening_on_port(port: u16) {
    #[cfg(target_os = "windows")]
    {
        let output = tokio::time::timeout(
            Duration::from_millis(PLUGIN_PROCESS_KILL_TIMEOUT_MS),
            Command::new("netstat").args(["-ano", "-p", "tcp"]).output(),
        )
        .await;
        let Ok(Ok(output)) = output else {
            return;
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let needle = format!(":{}", port);
        let mut pids = HashSet::new();
        for line in stdout.lines() {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() < 5 {
                continue;
            }
            if !columns[0].eq_ignore_ascii_case("TCP") {
                continue;
            }
            if !columns[1].ends_with(&needle) {
                continue;
            }
            if !columns[3].eq_ignore_ascii_case("LISTENING") {
                continue;
            }
            if let Ok(pid) = columns[4].parse::<u32>() {
                pids.insert(pid);
            }
        }

        for pid in pids {
            let mut cmd = Command::new("taskkill");
            cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            hide_command_window(&mut cmd);
            let _ = tokio::time::timeout(
                Duration::from_millis(PLUGIN_PROCESS_KILL_TIMEOUT_MS),
                cmd.output(),
            )
            .await;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = port;
    }
}

async fn wait_for_plugin_stopped(
    plugin_id: String,
    entry: &PluginEntry,
    base_url: Option<String>,
) -> Result<AiPluginStatus, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(3_000);
    let probe_status = |plugin_id: String| {
        let base_url = base_url.clone();
        async move {
            if let Some(base_url) = base_url {
                probe_ai_plugin_status(plugin_id, entry, base_url, true, None).await
            } else {
                Ok(ai_plugin_status(
                    plugin_id,
                    false,
                    false,
                    None,
                    None,
                    Some("Plugin has no managed runtime".to_string()),
                ))
            }
        }
    };

    let mut last_status = probe_status(plugin_id.clone()).await?;

    while last_status.reachable && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
        last_status = probe_status(plugin_id.clone()).await?;
    }

    if last_status.reachable {
        if let Some(port) = plugin_port(entry) {
            kill_processes_listening_on_port(port).await;
            tokio::time::sleep(Duration::from_millis(250)).await;
            last_status = probe_status(plugin_id).await?;
        }
    }

    Ok(last_status)
}

fn hide_command_window(cmd: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = cmd;
    }
}

#[tauri::command]
pub async fn invoke_ai_plugin_capability(
    plugin_id: String,
    capability_id: String,
    request: AiPluginInvokeRequest,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginInvokeResponse, String> {
    invoke_ai_plugin_capability_inner(plugin_id, capability_id, request, &state).await
}

async fn invoke_ai_plugin_capability_inner(
    plugin_id: String,
    capability_id: String,
    request: AiPluginInvokeRequest,
    state: &AiPluginRuntimeState,
) -> Result<AiPluginInvokeResponse, String> {
    let (manifest_path, manifest) = find_plugin_manifest(&plugin_id)?;
    ensure_valid_manifest(&manifest_path, &manifest)?;

    let Some(entry) = manifest.entry.as_ref() else {
        return Err("Plugin has no entry".to_string());
    };
    if entry.kind != "local-http" {
        return Err(format!(
            "Capability invocation is not supported for '{}'",
            entry.kind
        ));
    }

    let Some(base_url) = plugin_base_url(&plugin_id, entry, Some(state)).await else {
        return Err("Plugin has no baseUrl".to_string());
    };
    let token = resolve_plugin_auth_token(state, &plugin_id, entry).await?;

    let capability = find_plugin_capability(&manifest, &capability_id)?;
    ensure_runtime_probe_gate(
        &plugin_id,
        &capability_id,
        &manifest_path,
        &manifest,
        request.runtime.as_ref(),
    )?;
    cleanup_plugin_task_cache(&plugin_id)?;
    let task_id = request
        .task_id
        .clone()
        .filter(|task_id| !task_id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let output_dir_path = match request.output_dir.clone() {
        Some(output_dir) if !output_dir.trim().is_empty() => PathBuf::from(output_dir),
        _ => plugin_task_output_dir(&plugin_id, &task_id)?,
    };
    fs::create_dir_all(&output_dir_path)
        .map_err(|e| format!("Failed to create plugin output directory: {}", e))?;
    let output_dir = normalize_path(&output_dir_path);
    let task_dir = plugin_task_temp_dir(&plugin_id, &task_id)?;
    // Stage external input files into the plugin's readable area when the
    // sandbox is active. The rewritten paths are used for both the task
    // snapshot and the invoke payload so the plugin sees staged copies.
    let root = manifest_path
        .parent()
        .ok_or_else(|| "Plugin manifest has no parent directory".to_string())?;
    let writable_dirs = plugin_writable_roots(
        &plugin_id,
        root,
        Some(&manifest),
        [PathBuf::from(&output_dir), task_dir.clone()],
    )?;
    // Hardlink staging shares the source inode, so a plugin writing back to its
    // input would rewrite the user's photo. Only allow it when the manifest
    // declares `writeSourceFiles`, i.e. the user granted in-place source edits.
    let allow_hardlink = manifest
        .permissions
        .as_ref()
        .and_then(|value| value.get("writeSourceFiles"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let (inputs, staging_report) = stage_input_files_for_sandbox(
        &plugin_id,
        &task_id,
        &request.inputs,
        &writable_dirs,
        allow_hardlink,
    )?;
    let now = Utc::now().to_rfc3339();
    let result_policy = request
        .result_policy
        .clone()
        .unwrap_or_else(|| "copyIntoAlbum".to_string());
    let mut task_state = AiPluginTaskState {
        plugin_id: plugin_id.clone(),
        capability_id: capability_id.clone(),
        task_id: task_id.clone(),
        status: "queued".to_string(),
        created_at: now.clone(),
        updated_at: now,
        task_dir: normalize_path(&task_dir),
        output_dir: output_dir.clone(),
        result_policy: Some(result_policy.clone()),
        adopted: false,
        outputs: Vec::new(),
        progress: Some(0),
        message: Some(staging_report.queue_message()),
        error: None,
        error_code: None,
        error_domain: None,
        error_details: None,
        retryable: false,
        request_snapshot: Some(AiPluginInvokeRequestSnapshot {
            inputs: inputs.clone(),
            parameters: request.parameters.clone(),
            runtime: request.runtime.clone(),
            result_policy: Some(result_policy.clone()),
        }),
    };
    save_task_state(task_state.clone())?;

    let payload = serde_json::json!({
        "taskId": task_id,
        "capability": capability_id,
        "inputs": inputs,
        "parameters": request.parameters,
        "outputDir": output_dir,
        "runtime": request.runtime,
        "resultPolicy": result_policy,
    });

    let url = join_url(&base_url, &invoke_path_for(capability));
    let client = plugin_http_client(Some(&token), PLUGIN_INVOKE_TIMEOUT_MS)?;

    let response = match client.post(&url).json(&payload).send().await {
        Ok(response) => response,
        Err(error) => {
            apply_task_error(
                &mut task_state,
                "HTTP_ERROR",
                Some("transport"),
                format!("Failed to invoke plugin capability: {}", error),
                None,
            );
            save_task_state(task_state)?;
            return Err(format!("Failed to invoke plugin capability: {}", error));
        }
    };
    let status_code = response.status();
    let result = match response.json::<Value>().await {
        Ok(result) => result,
        Err(error) => {
            apply_task_error(
                &mut task_state,
                "RESPONSE_PARSE_FAILED",
                Some("transport"),
                format!("Failed to parse invoke response: {}", error),
                None,
            );
            save_task_state(task_state)?;
            return Err(format!("Failed to parse invoke response: {}", error));
        }
    };

    if !status_code.is_success() {
        apply_plugin_error(
            &mut task_state,
            &result,
            format!("Invoke endpoint returned {}", status_code),
        );
        task_state.outputs = result
            .get("outputs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        save_task_state(task_state)?;
        return Err(format!(
            "Invoke endpoint returned {}: {}",
            status_code, result
        ));
    }

    let final_result = if plugin_task_status(&result).is_some() {
        apply_plugin_task_result(&mut task_state, &result, &output_dir_path)?;
        save_task_state(task_state.clone())?;
        if is_plugin_task_terminal(&task_state.status) {
            result
        } else {
            spawn_plugin_task_poll(
                base_url.clone(),
                task_id.clone(),
                token.clone(),
                task_state.clone(),
                output_dir_path.clone(),
            );
            result
        }
    } else {
        if let Err(error) = validate_plugin_output_paths(&result, &output_dir_path) {
            apply_task_error(
                &mut task_state,
                "OUTPUT_VALIDATION_FAILED",
                Some("filesystem"),
                error.clone(),
                None,
            );
            save_task_state(task_state)?;
            return Err(error);
        }
        task_state.status = "succeeded".to_string();
        task_state.updated_at = Utc::now().to_rfc3339();
        task_state.outputs = result
            .get("outputs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        save_task_state(task_state.clone())?;
        result
    };

    if matches!(
        task_state.status.as_str(),
        "failed" | "error" | "cancelled" | "canceled"
    ) {
        let message = task_state.error.clone().unwrap_or_else(|| {
            format!(
                "Plugin task '{}' finished with status '{}'",
                task_id, task_state.status
            )
        });
        return Err(message);
    }

    Ok(AiPluginInvokeResponse {
        plugin_id,
        capability_id,
        task_id,
        url,
        result: final_result,
        task_state: Some(task_state),
    })
}

#[tauri::command]
pub fn adopt_ai_plugin_task_outputs(
    plugin_id: String,
    request: AiPluginTaskAdoptRequest,
) -> Result<AiPluginTaskState, String> {
    let mut registry = load_registry()?;
    let key = plugin_task_state_key(&plugin_id, &request.task_id);
    let mut state = registry
        .task_states
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("Plugin task '{}' was not found", request.task_id))?;
    state.status = "imported".to_string();
    state.adopted = true;
    state.updated_at = Utc::now().to_rfc3339();
    state.error = None;
    state.error_code = None;
    state.error_domain = None;
    state.error_details = None;
    state.retryable = false;

    if request.delete_task_dir {
        let task_dir = PathBuf::from(&state.task_dir);
        if task_dir.exists() {
            fs::remove_dir_all(&task_dir).map_err(|e| {
                format!(
                    "Failed to remove adopted plugin task directory '{}': {}",
                    task_dir.display(),
                    e
                )
            })?;
        }
    }

    registry.task_states.insert(key, state.clone());
    save_registry(&registry)?;
    Ok(state)
}

#[tauri::command]
pub fn discard_ai_plugin_task_outputs(
    plugin_id: String,
    request: AiPluginTaskAdoptRequest,
) -> Result<AiPluginTaskState, String> {
    let mut registry = load_registry()?;
    let key = plugin_task_state_key(&plugin_id, &request.task_id);
    let mut state = registry
        .task_states
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("Plugin task '{}' was not found", request.task_id))?;
    let original_status = state.status.to_ascii_lowercase();
    state.status = "discarded".to_string();
    state.adopted = false;
    state.updated_at = Utc::now().to_rfc3339();
    state.error = None;
    state.error_code = None;
    state.error_domain = None;
    state.error_details = None;
    state.retryable = false;

    if request.delete_task_dir {
        let task_dir = PathBuf::from(&state.task_dir);
        if task_dir.exists() {
            fs::remove_dir_all(&task_dir).map_err(|e| {
                format!(
                    "Failed to remove discarded plugin task directory '{}': {}",
                    task_dir.display(),
                    e
                )
            })?;
        }
    }

    if matches!(
        original_status.as_str(),
        "failed" | "error" | "cancelled" | "canceled" | "discarded"
    ) {
        registry.task_states.remove(&key);
    } else {
        registry.task_states.insert(key, state.clone());
    }
    save_registry(&registry)?;
    Ok(state)
}

#[tauri::command]
pub async fn retry_ai_plugin_task(
    plugin_id: String,
    task_id: String,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginInvokeResponse, String> {
    let registry = load_registry()?;
    let key = plugin_task_state_key(&plugin_id, &task_id);
    let task = registry
        .task_states
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("Plugin task '{}' was not found", task_id))?;
    if !task.retryable {
        return Err(format!("Plugin task '{}' is not marked retryable", task_id));
    }
    let snapshot = task
        .request_snapshot
        .ok_or_else(|| format!("Plugin task '{}' has no request snapshot", task_id))?;
    let request = AiPluginInvokeRequest {
        task_id: None,
        inputs: snapshot.inputs,
        parameters: snapshot.parameters,
        output_dir: None,
        runtime: snapshot.runtime,
        result_policy: snapshot.result_policy,
    };
    invoke_ai_plugin_capability_inner(plugin_id, task.capability_id, request, &state).await
}

#[tauri::command]
pub async fn cancel_ai_plugin_task(
    plugin_id: String,
    task_id: String,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginTaskState, String> {
    let mut registry = load_registry()?;
    let key = plugin_task_state_key(&plugin_id, &task_id);
    let mut task = registry
        .task_states
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("Plugin task '{}' was not found", task_id))?;
    if !matches!(task.status.as_str(), "queued" | "running" | "cancelling") {
        return Err(format!(
            "Plugin task '{}' cannot be cancelled from status '{}'",
            task_id, task.status
        ));
    }
    task.status = "cancelling".to_string();
    task.updated_at = Utc::now().to_rfc3339();
    registry.task_states.insert(key.clone(), task.clone());
    save_registry(&registry)?;

    let cancel_context = match find_plugin_manifest(&plugin_id) {
        Ok((_manifest_path, manifest)) => {
            if let Some(entry) = manifest.entry.as_ref() {
                if let Some(base_url) = plugin_base_url(&plugin_id, entry, Some(&state)).await {
                    let token = resolve_plugin_auth_token(&state, &plugin_id, entry).await?;
                    Ok((base_url, token))
                } else {
                    Err("Plugin has no baseUrl".to_string())
                }
            } else {
                Err("Plugin has no entry".to_string())
            }
        }
        Err(error) => Err(error),
    };

    let mut registry = load_registry()?;
    let mut task = registry
        .task_states
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("Plugin task '{}' was not found", task_id))?;
    match cancel_context {
        Ok((base_url, token)) => {
            match request_plugin_task_cancel(&base_url, &task_id, &token).await {
                Ok(()) => {
                    task.status = "cancelling".to_string();
                    task.updated_at = Utc::now().to_rfc3339();
                    task.message = Some("Cancel requested".to_string());
                    task.retryable = false;
                }
                Err(error) => {
                    task.status = "cancelled".to_string();
                    task.updated_at = Utc::now().to_rfc3339();
                    task.message = Some(
                        "Cancel endpoint unavailable; task marked cancelled locally.".to_string(),
                    );
                    task.error = Some(error);
                    task.error_code = Some("CANCEL_REQUEST_FAILED".to_string());
                    task.error_domain = Some("transport".to_string());
                    task.retryable = false;
                }
            }
        }
        Err(error) => {
            task.status = "cancelled".to_string();
            task.updated_at = Utc::now().to_rfc3339();
            task.message = Some("Plugin unavailable; task marked cancelled locally.".to_string());
            task.error = Some(error);
            task.error_code = Some("CANCEL_REQUEST_UNAVAILABLE".to_string());
            task.error_domain = Some("transport".to_string());
            task.retryable = false;
        }
    }
    registry.task_states.insert(key, task.clone());
    save_registry(&registry)?;
    Ok(task)
}

#[tauri::command]
pub async fn get_ai_plugin_task(
    plugin_id: String,
    task_id: String,
    state: tauri::State<'_, AiPluginRuntimeState>,
) -> Result<AiPluginTaskStatusResponse, String> {
    let registry = load_registry()?;
    let key = plugin_task_state_key(&plugin_id, &task_id);
    let mut task = registry
        .task_states
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("Plugin task '{}' was not found", task_id))?;
    drop(registry);

    let mut plugin_status = None;
    let mut plugin_status_error = None;
    if matches!(task.status.as_str(), "queued" | "running" | "cancelling") {
        match find_plugin_manifest(&plugin_id) {
            Ok((_manifest_path, manifest)) => {
                if let Some(entry) = manifest.entry.as_ref() {
                    if entry.kind == "local-http" {
                        if let Some(base_url) =
                            plugin_base_url(&plugin_id, entry, Some(&state)).await
                        {
                            // Fail closed the same way as capability invocation: a
                            // lost runtime token must not turn the task query into an
                            // anonymous request to whatever now owns the port.
                            match resolve_plugin_auth_token(&state, &plugin_id, entry).await {
                                Ok(token) => {
                                    let output_dir = PathBuf::from(&task.output_dir);
                                    match query_plugin_task_once(
                                        &base_url,
                                        &task_id,
                                        &token,
                                        &mut task,
                                        &output_dir,
                                    )
                                    .await
                                    {
                                        Ok(value) => plugin_status = Some(value),
                                        Err(error) => plugin_status_error = Some(error),
                                    }
                                }
                                Err(error) => plugin_status_error = Some(error),
                            }
                        } else {
                            plugin_status_error = Some("Plugin has no baseUrl".to_string());
                        }
                    }
                }
            }
            Err(error) => {
                plugin_status_error = Some(error);
            }
        }
    }

    Ok(AiPluginTaskStatusResponse {
        plugin_id,
        task_id,
        state: task,
        plugin_status,
        plugin_status_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_package_mutations_are_process_serialized() {
        let first = PluginPackageMutationGuard::acquire().unwrap();
        let second = PluginPackageMutationGuard::acquire().unwrap_err();
        assert!(second.contains("already in progress"), "{second}");
        drop(first);
        PluginPackageMutationGuard::acquire().unwrap();
    }

    #[test]
    fn new_plugin_store_defaults_to_app_data_independent_of_install_path() {
        let root = rollback_fixture_root("default-store-app-data");
        let app_data_store = root.join("app-data").join("picaipic-local");
        let install_store = root
            .join("Program Files")
            .join("PicAiPic")
            .join("picaipic-local");

        assert_eq!(
            default_plugin_store_dir_for_paths(&app_data_store, &install_store),
            app_data_store
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_legacy_plugin_store_remains_visible_until_migrated() {
        let root = rollback_fixture_root("default-store-legacy");
        let app_data_store = root.join("app-data").join("picaipic-local");
        let install_store = root.join("installed").join("picaipic-local");
        fs::create_dir_all(&install_store).unwrap();

        assert_eq!(
            default_plugin_store_dir_for_paths(&app_data_store, &install_store),
            install_store
        );

        fs::create_dir_all(&app_data_store).unwrap();
        assert_eq!(
            default_plugin_store_dir_for_paths(&app_data_store, &install_store),
            app_data_store
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn rollback_fixture_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("picaipic-plugin-{}-{}", name, Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn replace_plugin_directory_restores_previous_plugin_when_move_fails() {
        let root = rollback_fixture_root("replace-rollback");
        let destination = root.join("plugin");
        let staged = root.join("staged-plugin");
        let backup = root.join("backup-plugin");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("version.txt"), "old").unwrap();
        // This missing staged path forces the final move to fail after the
        // old directory was already staged as a backup.

        let error = replace_plugin_directory(&staged, &destination, &backup).unwrap_err();
        assert!(error.contains("previous plugin was restored"), "{error}");
        assert_eq!(
            fs::read_to_string(destination.join("version.txt")).unwrap(),
            "old"
        );
        assert!(!backup.exists(), "backup should be restored to destination");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staged_uninstall_directories_restore_without_registry_commit() {
        let root = rollback_fixture_root("uninstall-rollback");
        let store = root.join("store");
        let plugin_root = store.join("plugins");
        let target = plugin_root.join("plugin");
        let data = store.join("plugin-data").join("plugin");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&data).unwrap();
        fs::write(target.join("code.txt"), "code").unwrap();
        fs::write(data.join("state.txt"), "state").unwrap();

        let mut staged = vec![
            stage_plugin_directory_for_uninstall(&target, &plugin_root, "plugin")
                .unwrap()
                .unwrap(),
        ];
        staged.push(
            stage_plugin_directory_for_uninstall(&data, &store, "plugin")
                .unwrap()
                .unwrap(),
        );

        assert!(!target.exists());
        assert!(!data.exists());
        assert!(restore_staged_plugin_directories(&staged).is_empty());
        assert_eq!(fs::read_to_string(target.join("code.txt")).unwrap(), "code");
        assert_eq!(fs::read_to_string(data.join("state.txt")).unwrap(), "state");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_storage_roots_are_scoped_to_the_selected_store() {
        let root = rollback_fixture_root("private-roots");
        let roots = plugin_private_storage_roots_in(&root, "plugin");
        assert_eq!(roots.len(), 4);
        assert!(roots.iter().all(|path| is_path_inside(path, &root)));
        assert!(roots.contains(&root.join("plugin-data").join("plugin")));
        assert!(roots.contains(&root.join("plugin-runtimes").join("plugin")));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovery_ignores_interrupted_plugin_transaction_directories() {
        let root = rollback_fixture_root("transaction-discovery");
        let live = root.join("plugin");
        let backup = root.join(".replacing-plugin-id");
        let uninstalling = root.join(".uninstalling-plugin-id");
        for path in [&live, &backup, &uninstalling] {
            fs::create_dir_all(path).unwrap();
            fs::write(
                path.join(MANIFEST_FILE_NAME),
                r#"{"id":"plugin","name":"Plugin","version":"1.0.0"}"#,
            )
            .unwrap();
        }

        let mut manifests = Vec::new();
        let mut seen = HashSet::new();
        collect_child_manifests(&root, &mut manifests, &mut seen);
        assert_eq!(manifests, vec![live.join(MANIFEST_FILE_NAME)]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn is_path_inside_rejects_parent_dir_escape_when_missing() {
        let root = std::env::temp_dir().join(format!("picaipic-path-root-{}", Uuid::new_v4()));
        // Intentionally do not create `root` — exercise lexical fallback.
        let nested = root.join("plugin-a").join("backend");
        assert!(
            is_path_inside(&nested, &root),
            "nested path under missing root must still count as inside"
        );
        let escape = root.join("..").join("outside-payload");
        assert!(
            !is_path_inside(&escape, &root),
            "root/../outside must not pass containment"
        );
        let sibling_escape = root
            .join("plugin-a")
            .join("..")
            .join("..")
            .join("outside-payload");
        assert!(
            !is_path_inside(&sibling_escape, &root),
            "deep .. climb must not pass containment"
        );
    }

    #[test]
    fn is_path_inside_accepts_equal_and_child_when_present() {
        let root = std::env::temp_dir().join(format!("picaipic-path-exist-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("child")).unwrap();
        assert!(is_path_inside(&root, &root));
        assert!(is_path_inside(&root.join("child"), &root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn app_version_comparison_handles_common_versions() {
        assert_eq!(
            compare_app_versions("1.0.0", "0.2.4"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            compare_app_versions("1.0", "1.0.0"),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(
            compare_app_versions("v1.2.0-beta.1", "1.1.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(compare_app_versions("not-a-version", "1.0.0"), None);
    }

    #[test]
    fn plugin_private_runtime_binding_requires_env_dir() {
        let mut profile = PluginInstallProfile {
            id: "windows-cpu".to_string(),
            backend: "cpu".to_string(),
            label: Some("CPU".to_string()),
            support_level: Some("fallback".to_string()),
            derived_from: None,
            env_dir: None,
            requirements: Some("backend/requirements-cpu.txt".to_string()),
            runtime_binding: None,
            runtime_bindings: vec![],
            notes: None,
        };
        assert!(plugin_private_runtime_binding(&profile).is_err());

        profile.env_dir = Some(".venv-cpu".to_string());
        let binding = plugin_private_runtime_binding(&profile).expect("private binding");
        assert_eq!(binding.scope, "plugin");
        assert_eq!(binding.id.as_deref(), Some("plugin-private:windows-cpu"));
        assert_eq!(
            binding.requirements.as_deref(),
            Some("backend/requirements-cpu.txt")
        );
        assert!(binding.label.as_deref().unwrap_or("").contains("CPU"));
    }

    #[test]
    fn input_staging_rewrites_external_paths_and_counts_bytes() {
        let root = std::env::temp_dir().join(format!("picaipic-stage-ok-{}", Uuid::new_v4()));
        let external = root.join("library");
        let staging = root.join("staging");
        let writable = root.join("writable");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&writable).unwrap();

        let external_file = external.join("photo.jpg");
        let writable_file = writable.join("already-inside.bin");
        fs::write(&external_file, b"hello-stage").unwrap();
        fs::write(&writable_file, b"inside").unwrap();

        let mut inputs = serde_json::json!({
            "items": [
                { "path": external_file.to_string_lossy(), "role": "source" },
                { "path": writable_file.to_string_lossy(), "role": "mask" },
                { "path": external.join("missing.jpg").to_string_lossy(), "role": "gone" },
            ]
        });
        let mut report = InputStagingReport {
            enabled: true,
            staging_dir: Some(normalize_path(&staging)),
            ..InputStagingReport::default()
        };
        // `allow_hardlink = true` mirrors a manifest declaring `writeSourceFiles`.
        stage_paths_in_value(
            &mut inputs,
            &staging,
            &[writable.clone()],
            &mut report,
            true,
        )
        .expect("staging should succeed");

        assert_eq!(report.staged_files, 1);
        assert_eq!(report.staged_bytes, 11);
        assert_eq!(report.skipped_writable, 1);
        assert_eq!(report.skipped_missing, 1);
        // Same temp volume → hardlink preferred (Phase 2).
        assert_eq!(report.hardlinked_files, 1);
        assert_eq!(report.copied_files, 0);

        let staged_path = inputs["items"][0]["path"].as_str().unwrap();
        assert!(
            Path::new(staged_path).starts_with(&staging),
            "external path must be rewritten under staging: {}",
            staged_path
        );
        assert_eq!(
            inputs["items"][1]["path"].as_str().unwrap(),
            writable_file.to_string_lossy()
        );
        assert!(report.queue_message().contains("1 file"));
        assert!(report.queue_message().contains("hardlink"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn input_staging_hardlink_shares_inode_when_same_volume() {
        let root = std::env::temp_dir().join(format!("picaipic-stage-hl-{}", Uuid::new_v4()));
        let external = root.join("library");
        let staging = root.join("staging");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&staging).unwrap();
        let external_file = external.join("big.bin");
        fs::write(&external_file, b"0123456789abcdef").unwrap();

        let (staged, bytes, method) =
            stage_one_file(&external_file, &staging, true).expect("stage_one_file");
        assert_eq!(method, StageMaterializeMethod::Hardlink);
        assert_eq!(bytes, 16);
        assert!(staged.exists());
        // Content must match; on Unix hardlinks share inode (optional check).
        assert_eq!(fs::read(&staged).unwrap(), b"0123456789abcdef");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a = fs::metadata(&external_file).unwrap().ino();
            let b = fs::metadata(&staged).unwrap().ino();
            assert_eq!(a, b, "hardlink should share inode");
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn input_staging_copies_unless_source_writes_are_allowed() {
        let root = std::env::temp_dir().join(format!("picaipic-stage-copy-{}", Uuid::new_v4()));
        let external = root.join("library");
        let staging = root.join("staging");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&staging).unwrap();
        let external_file = external.join("photo.jpg");
        fs::write(&external_file, b"original-bytes").unwrap();

        let (staged, bytes, method) =
            stage_one_file(&external_file, &staging, false).expect("stage_one_file");
        assert_eq!(method, StageMaterializeMethod::Copy);
        assert_eq!(bytes, 14);
        assert_eq!(fs::read(&staged).unwrap(), b"original-bytes");

        // Simulate a plugin writing back to its input path: the library file must be
        // untouched because the staged copy has its own inode. Under the old
        // unconditional hardlink this silently rewrote the user's photo.
        fs::write(&staged, b"plugin-overwrote-its-input").unwrap();
        assert_eq!(
            fs::read(&external_file).unwrap(),
            b"original-bytes",
            "a staged copy must not alias the user's original media"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn input_staging_fails_closed_when_copy_cannot_complete() {
        let root = std::env::temp_dir().join(format!("picaipic-stage-fail-{}", Uuid::new_v4()));
        let external = root.join("library");
        // staging is intentionally a file, so create_dir is not used and copy fails.
        let staging_as_file = root.join("staging-not-a-dir");
        fs::create_dir_all(&external).unwrap();
        fs::write(&staging_as_file, b"block").unwrap();
        let external_file = external.join("photo.jpg");
        fs::write(&external_file, b"data").unwrap();

        let mut inputs = serde_json::json!({ "path": external_file.to_string_lossy() });
        let mut report = InputStagingReport {
            enabled: true,
            ..InputStagingReport::default()
        };
        let err = stage_paths_in_value(&mut inputs, &staging_as_file, &[], &mut report, false)
            .expect_err("staging into a non-directory must fail closed");
        assert!(
            err.contains("Failed to stage input"),
            "unexpected error: {}",
            err
        );
        // Original external path must remain only because the whole invoke aborts;
        // the helper itself may have partially rewritten — fail closed means Result::Err.
        assert_eq!(report.staged_files, 0);
        assert_eq!(report.hardlinked_files, 0);
        assert_eq!(report.copied_files, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn input_staging_disabled_queue_message() {
        let report = InputStagingReport::disabled();
        assert!(!report.enabled);
        assert!(report.queue_message().contains("disabled"));
    }

    /// Real layout proof: library-like source under the OS temp dir staged into
    /// a path under the crate (often a different Windows volume than `%TEMP%`).
    /// Same-volume → hardlink; cross-volume → copy. Always writes
    /// `staging-report.json` with hardlinked/copied counts.
    #[test]
    fn input_staging_real_layout_report_hardlink_or_copy() {
        let case_id = Uuid::new_v4();
        let external = std::env::temp_dir().join(format!("picaipic-lib-{}", case_id));
        // Stage under the crate target tree so Windows C: TEMP → D: workspace
        // reproduces the production library-vs-plugin-store volume split.
        let staging = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("stage-real-layout")
            .join(case_id.to_string())
            .join("inputs");
        fs::create_dir_all(&external).unwrap();
        fs::create_dir_all(&staging).unwrap();

        let external_file = external.join("library-photo.jpg");
        let payload = b"phase0-real-layout-library-bytes";
        fs::write(&external_file, payload).unwrap();

        let mut inputs = serde_json::json!({
            "items": [{ "path": external_file.to_string_lossy(), "role": "source" }]
        });
        let mut report = InputStagingReport {
            enabled: true,
            staging_dir: Some(normalize_path(&staging)),
            ..InputStagingReport::default()
        };
        stage_paths_in_value(&mut inputs, &staging, &[], &mut report, true).expect("staging");
        write_input_staging_report(&staging, &report);

        assert_eq!(report.staged_files, 1);
        assert_eq!(report.staged_bytes, payload.len() as u64);
        assert_eq!(
            report.hardlinked_files + report.copied_files,
            1,
            "exactly one materialize method"
        );
        assert!(
            report.hardlinked_files == 1 || report.copied_files == 1,
            "must hardlink or copy: hl={} copy={}",
            report.hardlinked_files,
            report.copied_files
        );

        let src_vol = external_file
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        let dst_vol = staging
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        if src_vol.is_some() && src_vol != dst_vol {
            // Cross-volume (typical Windows C: library / D: plugin store): copy only.
            assert_eq!(report.hardlinked_files, 0);
            assert_eq!(report.copied_files, 1);
            assert!(
                report.queue_message().contains("0 hardlink")
                    && report.queue_message().contains("1 copy"),
                "queue message: {}",
                report.queue_message()
            );
        }

        let report_path = staging.join("staging-report.json");
        assert!(report_path.is_file(), "staging-report.json must exist");
        let body = fs::read_to_string(&report_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["enabled"], true);
        assert_eq!(parsed["stagedFiles"], 1);
        assert_eq!(
            parsed["hardlinkedFiles"].as_u64().unwrap_or(0)
                + parsed["copiedFiles"].as_u64().unwrap_or(0),
            1
        );
        let staged_path = inputs["items"][0]["path"].as_str().unwrap();
        assert!(
            Path::new(staged_path).starts_with(&staging),
            "rewritten path under staging: {}",
            staged_path
        );
        assert_eq!(fs::read(staged_path).unwrap(), payload);

        let _ = fs::remove_dir_all(&external);
        let _ = fs::remove_dir_all(staging.parent().unwrap());
    }

    #[test]
    fn path_is_inside_any_matches_allow_list_roots() {
        let root = std::env::temp_dir().join(format!("picaipic-allow-{}", Uuid::new_v4()));
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(a.join("nested")).unwrap();
        fs::create_dir_all(&b).unwrap();
        let nested = a.join("nested").join("file.txt");
        fs::write(&nested, b"x").unwrap();
        assert!(path_is_inside_any(&nested, &[a.clone(), b.clone()]));
        assert!(!path_is_inside_any(&nested, &[b]));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_shared_runtime_roots_dedupes_by_id() {
        let profile = PluginInstallProfile {
            id: "windows-cpu".to_string(),
            backend: "cpu".to_string(),
            label: None,
            support_level: None,
            derived_from: None,
            env_dir: Some(".venv-cpu".to_string()),
            requirements: None,
            runtime_binding: Some(PluginRuntimeBinding {
                scope: "shared".to_string(),
                kind: Some("python".to_string()),
                id: Some("python312-cpu".to_string()),
                label: None,
                python: None,
                root: None,
                requirements: None,
                notes: None,
            }),
            runtime_bindings: vec![PluginRuntimeBinding {
                scope: "shared".to_string(),
                kind: Some("python".to_string()),
                id: Some("python312-cpu".to_string()),
                label: None,
                python: None,
                root: None,
                requirements: None,
                notes: None,
            }],
            notes: None,
        };
        let mut manifest: AiPluginManifest = serde_json::from_value(serde_json::json!({
            "id": "test-plugin",
            "name": "test",
            "version": "0.0.0"
        }))
        .expect("minimal manifest");
        manifest.install_profiles = vec![profile];
        let roots = collect_shared_runtime_roots(&manifest);
        assert_eq!(roots.len(), 1);
        assert!(
            roots[0]
                .to_string_lossy()
                .replace('\\', "/")
                .contains("shared-runtimes/python312-cpu")
                || roots[0]
                    .to_string_lossy()
                    .contains("shared-runtimes\\python312-cpu")
        );
    }

    fn test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn package_manifest_rejects_declared_missing_file() {
        let present = b"print('ok')";
        let zip_bytes = test_zip(&[("test-plugin/a.py", present)]);
        let mut archive = ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(present);
        let manifest = AiPluginPackageManifest {
            files: vec![
                AiPluginPackageFile {
                    path: "a.py".to_string(),
                    size: present.len() as u64,
                    sha256: format!("{:X}", hasher.finalize()),
                },
                AiPluginPackageFile {
                    path: "missing.py".to_string(),
                    size: 1,
                    sha256: "00".repeat(32),
                },
            ],
            ..AiPluginPackageManifest::default()
        };

        let error = validate_package_manifest_file_list(&mut archive, "test-plugin", &manifest)
            .unwrap_err();
        assert!(error.contains("missing.py"), "unexpected error: {error}");
    }

    #[test]
    fn package_manifest_rejects_excessive_unpacked_size() {
        let manifest = AiPluginPackageManifest {
            files: vec![AiPluginPackageFile {
                path: "model.bin".to_string(),
                size: PLUGIN_PACKAGE_MAX_FILE_BYTES + 1,
                sha256: "00".repeat(32),
            }],
            ..AiPluginPackageManifest::default()
        };
        let error = validate_package_unpacked_size_budget(&manifest).unwrap_err();
        assert!(
            error.contains("unpacked-file limit"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn package_snapshot_is_stable_when_source_is_replaced() {
        let root =
            std::env::temp_dir().join(format!("picaipic-package-snapshot-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("plugin.zip");
        fs::write(&source, test_zip(&[("test-plugin/value.txt", b"signed-a")])).unwrap();

        let snapshot = create_plugin_package_snapshot(&source, &root).unwrap();
        let mut archive = ZipArchive::new(fs::File::open(&snapshot.path).unwrap()).unwrap();
        fs::write(
            &source,
            test_zip(&[("test-plugin/value.txt", b"tampered-b")]),
        )
        .unwrap();

        let content = read_plugin_package_file(&mut archive, "test-plugin/value.txt").unwrap();
        assert_eq!(content, "signed-a");

        drop(archive);
        drop(snapshot);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_network_registry_error_fails_closed() {
        assert!(!resolve_runtime_network_grant(
            Err("registry unreadable".to_string()),
            "test-plugin"
        ));
        assert!(resolve_runtime_network_grant(Ok(true), "test-plugin"));
        assert!(!resolve_runtime_network_grant(Ok(false), "test-plugin"));
    }

    /// Build a minimal manifest matching the one signed by `sign_plugin.py`
    /// in the canonical-serialization test fixture. Field assignment order in
    /// Rust is irrelevant to the signature because verification re-serializes
    /// through `serde_json::Value` (BTreeMap → lexicographic key order).
    fn fixture_manifest() -> AiPluginPackageManifest {
        let mut m = AiPluginPackageManifest::default();
        m.schema_version = 1;
        m.package_kind = "picaipic-plugin-package".to_string();
        m.plugin_id = "test-plugin".to_string();
        m.version = "0.1.0".to_string();
        m.created_at = Some("2026-01-01T00:00:00Z".to_string());
        m.files = vec![AiPluginPackageFile {
            path: "a.py".to_string(),
            size: 100,
            sha256: "abc".to_string(),
        }];
        m.warnings = vec![];
        m
    }

    fn fixture_signature() -> AiPluginPackageSignature {
        AiPluginPackageSignature {
            algorithm: "ed25519".to_string(),
            public_key: "dSlXXhlQVJRMsE0BAoc2vmaC/G1PG1W6e4g0lgqNw7o="
                .to_string(),
            value: "O32+sFP0worKsRYH3mv9gPYBv4myxGagbTR7k8R9SST9dqcBML+EqjGiDVjPa6edd3Wbyn8XEYn0lYHGIBRJAw==".to_string(),
        }
    }

    fn empty_registry() -> AiPluginRegistry {
        AiPluginRegistry::default()
    }

    /// A signed package produced by `sign_plugin.py` (with `sort_keys=True`)
    /// must verify on the Rust side. This is the cross-language byte-level
    /// consistency check: if either side changes its canonical serialization,
    /// this test fails.
    #[test]
    fn signed_package_from_python_verifies() {
        let mut manifest = fixture_manifest();
        manifest.signature = Some(fixture_signature());
        let publisher = Some("test-author".to_string());
        let result = verify_package_signature(&manifest, &publisher, &empty_registry());
        match result {
            Ok(SignatureVerifyResult::NeedsTrust { .. }) | Ok(SignatureVerifyResult::Verified) => {
                // Signature verified; publisher just isn't trusted yet.
            }
            Ok(SignatureVerifyResult::UnsignedAllowed) => {
                panic!("expected signature to verify, but got UnsignedAllowed")
            }
            Err(e) => panic!("expected verification to pass, but got error: {}", e),
        }
    }

    /// The signature must be independent of struct field declaration order and
    /// of on-disk key order. Constructing the same manifest via a different
    /// code path (here: just re-asserting the same values, which is the only
    /// way to vary construction in Rust) must still verify. The real
    /// order-independence guarantee comes from `serde_json::Value` sorting keys
    /// during verification; this test guards against regressing that path back
    /// to `serde_json::to_vec(&struct)`.
    #[test]
    fn canonical_serialization_sorts_keys() {
        let mut m = fixture_manifest();
        m.signature = None;
        let value = serde_json::to_value(&m).unwrap();
        let bytes = serde_json::to_vec(&value).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        // createdAt must appear before files, which appears before packageKind,
        // etc. — i.e. lexicographic, NOT struct declaration order
        // (schemaVersion would come first if we used struct order).
        let created_at = s.find("\"createdAt\"").unwrap();
        let files = s.find("\"files\"").unwrap();
        let package_kind = s.find("\"packageKind\"").unwrap();
        let plugin_id = s.find("\"pluginId\"").unwrap();
        let schema_version = s.find("\"schemaVersion\"").unwrap();
        let version = s.find("\"version\"").unwrap();
        let warnings = s.find("\"warnings\"").unwrap();
        assert!(created_at < files);
        assert!(files < package_kind);
        assert!(package_kind < plugin_id);
        assert!(plugin_id < schema_version);
        assert!(schema_version < version);
        assert!(version < warnings);
    }

    /// A tampered signature (flipped bit) must fail verification. This guards
    /// against the verifier accidentally accepting arbitrary bytes.
    #[test]
    fn tampered_signature_is_rejected() {
        let mut manifest = fixture_manifest();
        let mut sig = fixture_signature();
        // Flip a character in the signature value.
        let mut chars: Vec<char> = sig.value.chars().collect();
        chars[0] = if chars[0] == 'O' { 'P' } else { 'O' };
        sig.value = chars.into_iter().collect();
        manifest.signature = Some(sig);
        let publisher = Some("test-author".to_string());
        let result = verify_package_signature(&manifest, &publisher, &empty_registry());
        assert!(matches!(result, Err(_)), "tampered signature should fail");
    }

    #[test]
    fn multi_key_trust_accepts_any_active_key() {
        let mut manifest = fixture_manifest();
        let sig = fixture_signature();
        let package_key = sig.public_key.clone();
        manifest.signature = Some(sig);
        let publisher = Some("test-author".to_string());

        let mut registry = empty_registry();
        trust_publisher_key_in_registry(
            &mut registry,
            "test-author".to_string(),
            "other-key-not-used=".to_string(),
        );
        trust_publisher_key_in_registry(
            &mut registry,
            "test-author".to_string(),
            package_key.clone(),
        );
        let tp = registry.trusted_publishers.get("test-author").unwrap();
        assert!(tp.keys.len() >= 2);
        assert!(publisher_accepts_public_key(tp, &package_key));

        let result = verify_package_signature(&manifest, &publisher, &registry);
        assert!(
            matches!(result, Ok(SignatureVerifyResult::Verified)),
            "expected Verified with multi-key trust, got {:?}",
            result
        );
    }

    #[test]
    fn revoked_key_is_rejected_even_if_trusted() {
        let mut manifest = fixture_manifest();
        let sig = fixture_signature();
        let package_key = sig.public_key.clone();
        manifest.signature = Some(sig);
        let publisher = Some("test-author".to_string());

        let mut registry = empty_registry();
        trust_publisher_key_in_registry(
            &mut registry,
            "test-author".to_string(),
            package_key.clone(),
        );
        registry.revoked_keys.push(AiPluginRevokedKey {
            public_key: package_key,
            revoked_at: "2026-07-20T00:00:00Z".to_string(),
            reason: Some("test".to_string()),
        });

        let result = verify_package_signature(&manifest, &publisher, &registry);
        assert!(
            matches!(result, Err(_)),
            "revoked key must fail closed, got {:?}",
            result
        );
    }

    #[test]
    fn legacy_single_key_publisher_normalizes() {
        let mut tp = AiPluginTrustedPublisher {
            publisher: "legacy".to_string(),
            public_key: "abcKey".to_string(),
            trusted_at: "t0".to_string(),
            keys: vec![],
        };
        normalize_trusted_publisher(&mut tp);
        assert_eq!(tp.keys.len(), 1);
        assert_eq!(tp.keys[0].public_key, "abcKey");
        assert!(publisher_accepts_public_key(&tp, "abcKey"));
        assert!(!publisher_accepts_public_key(&tp, "other"));
    }

    #[test]
    fn second_key_of_known_publisher_needs_trust() {
        let mut manifest = fixture_manifest();
        let sig = fixture_signature();
        manifest.signature = Some(sig);
        let publisher = Some("test-author".to_string());

        let mut registry = empty_registry();
        // Only trust a different key — package key still NeedsTrust.
        trust_publisher_key_in_registry(
            &mut registry,
            "test-author".to_string(),
            "old-key-only=".to_string(),
        );
        let result = verify_package_signature(&manifest, &publisher, &registry);
        assert!(
            matches!(result, Ok(SignatureVerifyResult::NeedsTrust { .. })),
            "new key of known publisher should need trust, got {:?}",
            result
        );
    }

    /// End-to-end verification of the real signed plugin zip packages in
    /// `dist/plugins/`. Marked `#[ignore]` because it depends on the repo's
    /// dist artifacts (which are rebuilt, not committed). Run with
    /// `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_signed_zips_verify() {
        use std::io::Read;
        let repo_root = env!("CARGO_MANIFEST_DIR");
        let zips = [
            "picai-nafnet-restore-0.1.0.zip",
            "picai-salut-color-0.1.0.zip",
        ];
        for name in zips {
            let path = format!("{}/../dist/plugins/{}", repo_root, name);
            let file = std::fs::File::open(&path).unwrap_or_else(|e| {
                panic!(
                    "could not open {}: {} (did you run package_plugin.ps1?)",
                    path, e
                )
            });
            let mut zip = zip::ZipArchive::new(file).expect("open zip");
            // Find the manifest entry (top-level dir / picaipic.package.json).
            let manifest_idx = (0..zip.len())
                .find(|i| {
                    zip.by_index(*i)
                        .map(|r| r.name().ends_with("picaipic.package.json"))
                        .unwrap_or(false)
                })
                .expect("manifest entry in zip");
            let mut buf = String::new();
            zip.by_index(manifest_idx)
                .unwrap()
                .read_to_string(&mut buf)
                .unwrap();
            let manifest: AiPluginPackageManifest =
                serde_json::from_str(&buf).expect("parse manifest");
            assert!(
                manifest.signature.is_some(),
                "{}: manifest has no signature",
                name
            );
            let publisher = Some("local".to_string());
            let result = verify_package_signature(&manifest, &publisher, &empty_registry());
            match result {
                Ok(SignatureVerifyResult::NeedsTrust { .. })
                | Ok(SignatureVerifyResult::Verified) => {}
                Ok(SignatureVerifyResult::UnsignedAllowed) => {
                    panic!("{}: unexpectedly got UnsignedAllowed", name)
                }
                Err(e) => panic!("{}: verification failed: {}", name, e),
            }
        }
    }
}
