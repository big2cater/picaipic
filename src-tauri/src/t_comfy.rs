/**
 * ComfyUI server integration.
 *
 * Talks to a ComfyUI server the user already runs (desktop build, 一键包, or a
 * remote box) and submits **API-format** workflows with a library image as input.
 * This is deliberately a separate module from `t_plugin.rs`: the plugin host owns
 * untrusted third-party code, while this only talks HTTP to a service the user
 * pointed us at, so none of the plugin trust boundary applies here.
 *
 * Progress uses polling of `/history/{prompt_id}` rather than a WebSocket, which
 * keeps the dependency footprint at `reqwest`. `POST /prompt` also returns
 * `node_errors`, surfaced immediately so a missing model or node is reported
 * instead of hanging until the timeout.
 */
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// ComfyUI's default listen address; the desktop build and most 一键包 use it too.
pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8188";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(700);
/// Generous: upscalers/restorers on CPU can take many minutes.
const RUN_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyServerInfo {
    pub reachable: bool,
    pub version: Option<String>,
    pub device: Option<String>,
}

/// A file already uploaded into ComfyUI's input directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUploadedImage {
    pub name: String,
    #[serde(default)]
    pub subfolder: String,
    #[serde(default = "default_upload_type", rename = "type")]
    pub image_type: String,
}

/// One image produced by a finished workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyOutputImage {
    pub filename: String,
    #[serde(default)]
    pub subfolder: String,
    #[serde(default = "default_output_type", rename = "type")]
    pub image_type: String,
    /// Node that produced it, so a workflow with several outputs is navigable.
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyRunResult {
    pub prompt_id: String,
    pub images: Vec<ComfyOutputImage>,
}

fn default_upload_type() -> String {
    "input".to_string()
}

fn default_output_type() -> String {
    "output".to_string()
}

fn normalize_server_url(server_url: &str) -> String {
    let trimmed = server_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_SERVER_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Check whether a ComfyUI server is reachable, and report version/device.
#[tauri::command]
pub async fn comfy_test_connection(server_url: &str) -> Result<ComfyServerInfo, String> {
    let base = normalize_server_url(server_url);
    let client = http_client()?;

    let response = client
        .get(format!("{base}/system_stats"))
        .send()
        .await
        .map_err(|e| format!("Cannot reach ComfyUI at '{base}': {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "ComfyUI at '{base}' returned HTTP {}",
            response.status()
        ));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("ComfyUI returned invalid JSON: {e}"))?;

    let device = body
        .get("devices")
        .and_then(|devices| devices.get(0))
        .and_then(|device| device.get("name"))
        .and_then(|name| name.as_str())
        .map(|name| name.to_string());

    let version = body
        .get("system")
        .and_then(|system| system.get("comfyui_version"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    Ok(ComfyServerInfo {
        reachable: true,
        version,
        device,
    })
}

/// Fetch the node definitions ComfyUI exposes, reduced to just the widget layout.
///
/// The UI format stores widget values as a positional array (`widgets_values`), so turning
/// it into the API format requires knowing which node input each position maps to. Only
/// that ordering is kept, which shrinks a multi-megabyte `/object_info` down to a few KB
/// before it crosses the IPC boundary.
#[tauri::command]
pub async fn comfy_object_info(server_url: &str) -> Result<Value, String> {
    let base = normalize_server_url(server_url);
    let client = http_client()?;

    let response = client
        .get(format!("{base}/object_info"))
        .send()
        .await
        .map_err(|e| format!("Cannot reach ComfyUI at '{base}': {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "ComfyUI at '{base}' returned HTTP {}",
            response.status()
        ));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("ComfyUI returned invalid JSON: {e}"))?;

    Ok(distill_object_info(&body))
}

/// Widget-typed inputs carry a primitive or enum spec; everything else is a link slot.
fn is_widget_input(spec: &Value) -> bool {
    match spec.get(0) {
        Some(Value::Array(_)) => true,
        Some(Value::String(name)) => {
            matches!(name.as_str(), "INT" | "FLOAT" | "STRING" | "BOOLEAN")
        }
        _ => false,
    }
}

fn has_control_widget(spec: &Value) -> bool {
    spec.get(1)
        .and_then(|options| options.get("control_after_generate"))
        .and_then(|flag| flag.as_bool())
        .unwrap_or(false)
}

fn collect_widgets(input: &Value) -> Vec<Value> {
    let mut widgets = Vec::new();
    // `required` first, then `optional`: the order the ComfyUI frontend creates widgets in,
    // which is exactly the order `widgets_values` is serialized in.
    for section_key in ["required", "optional"] {
        let Some(section) = input.get(section_key).and_then(|s| s.as_object()) else {
            continue;
        };
        for (name, spec) in section {
            if !is_widget_input(spec) {
                continue;
            }
            widgets.push(serde_json::json!({
                "name": name,
                // A randomizable widget is followed by an extra "control_after_generate"
                // value that has no counterpart in the API format, so it must be skipped.
                "control": has_control_widget(spec),
            }));
        }
    }
    widgets
}

fn distill_object_info(body: &Value) -> Value {
    let mut out = serde_json::Map::new();
    let Some(root) = body.as_object() else {
        return Value::Object(out);
    };
    for (node_type, def) in root {
        let widgets = def.get("input").map(collect_widgets).unwrap_or_default();
        out.insert(node_type.clone(), serde_json::json!({ "widgets": widgets }));
    }
    Value::Object(out)
}

/// Upload a library image into ComfyUI's input directory.
#[tauri::command]
pub async fn comfy_upload_image(
    server_url: &str,
    file_path: &str,
) -> Result<ComfyUploadedImage, String> {
    let base = normalize_server_url(server_url);
    let path = Path::new(file_path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Invalid image path: {file_path}"))?;

    let bytes =
        std::fs::read(path).map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(file_name.to_string())
        .mime_str("application/octet-stream")
        .map_err(|e| format!("Failed to build the upload part: {e}"))?;
    let form = reqwest::multipart::Form::new()
        .part("image", part)
        .text("overwrite", "true");

    let client = http_client()?;
    let response = client
        .post(format!("{base}/upload/image"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Failed to upload image to ComfyUI: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!(
            "ComfyUI rejected the image upload (HTTP {status}): {detail}"
        ));
    }

    response
        .json::<ComfyUploadedImage>()
        .await
        .map_err(|e| format!("ComfyUI returned an unexpected upload response: {e}"))
}

/// Prompt ids the user asked to stop. `comfy_run_workflow` blocks until ComfyUI finishes,
/// so a cancel request necessarily arrives on a separate invoke and has to be recorded
/// here for that polling loop to observe.
static CANCELLED_RUNS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Returned when the user cancels, kept distinct from a real failure so the UI does not
/// surface it as an error.
pub const CANCELLED: &str = "cancelled";

fn mark_cancelled(prompt_id: &str) {
    if let Ok(mut set) = CANCELLED_RUNS.lock() {
        set.insert(prompt_id.to_string());
    }
}

/// Takes (rather than reads) the flag so a stale cancel cannot abort the next run.
fn take_cancelled(prompt_id: &str) -> bool {
    match CANCELLED_RUNS.lock() {
        Ok(mut set) => set.remove(prompt_id),
        Err(_) => false,
    }
}

/// Submit an API-format workflow and wait for it to finish.
///
/// `prompt_id` comes from the caller so it can cancel before this returns; when omitted one
/// is generated, which is what a fire-and-forget caller wants.
#[tauri::command]
pub async fn comfy_run_workflow(
    server_url: &str,
    workflow: Value,
    prompt_id: Option<String>,
) -> Result<ComfyRunResult, String> {
    let base = normalize_server_url(server_url);
    let client = http_client()?;
    let prompt_id = prompt_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let payload = serde_json::json!({
        "prompt": workflow,
        "prompt_id": prompt_id,
        "client_id": uuid::Uuid::new_v4().to_string(),
    });

    let response = client
        .post(format!("{base}/prompt"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to submit workflow to ComfyUI: {e}"))?;

    // A non-2xx answer (bad prompt, wrong format) usually carries a usable JSON body;
    // report the status up front instead of letting the body parse stand in for it.
    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!(
            "ComfyUI rejected the workflow (HTTP {status}): {detail}"
        ));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("ComfyUI returned an invalid /prompt response: {e}"))?;

    // ComfyUI answers 200 + node_errors for a *rejected* workflow (missing model,
    // unknown node, bad link). Report it now instead of polling until the timeout.
    if let Some(errors) = body.get("node_errors") {
        let has_errors = errors
            .as_object()
            .map(|map| !map.is_empty())
            .unwrap_or(false);
        if has_errors {
            return Err(format!("ComfyUI rejected the workflow: {errors}"));
        }
    }
    if let Some(error) = body.get("error") {
        return Err(format!("ComfyUI refused the workflow: {error}"));
    }

    let prompt_id = body
        .get("prompt_id")
        .and_then(|id| id.as_str())
        .map(|id| id.to_string())
        .unwrap_or(prompt_id);

    let images = wait_for_outputs(&base, &prompt_id).await?;
    Ok(ComfyRunResult { prompt_id, images })
}

/// Ask ComfyUI to unload models and release VRAM.
///
/// ComfyUI keeps models resident between runs, which is normally what you want, but after a
/// heavy workflow (or before starting one) it can be worth reclaiming the memory. Models are
/// reloaded on demand, so this is safe to call at any time.
#[tauri::command]
pub async fn comfy_free_memory(server_url: &str) -> Result<(), String> {
    let base = normalize_server_url(server_url);
    let client = http_client()?;

    let response = client
        .post(format!("{base}/free"))
        .json(&serde_json::json!({
            "unload_models": true,
            "free_memory": true,
        }))
        .send()
        .await
        .map_err(|e| format!("Cannot reach ComfyUI at '{base}': {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "ComfyUI at '{base}' returned HTTP {}",
            response.status()
        ));
    }
    Ok(())
}

/// Stop a workflow that is currently running.
///
/// Records the id for the polling loop to pick up, and also calls `/interrupt` so ComfyUI
/// abandons the job now instead of spending GPU time on a result nobody will collect. A
/// failed interrupt (the run may have just finished) is not an error: the flag alone is
/// enough to stop our side.
#[tauri::command]
pub async fn comfy_cancel_run(server_url: &str, prompt_id: &str) -> Result<(), String> {
    mark_cancelled(prompt_id);

    let base = normalize_server_url(server_url);
    if let Ok(client) = http_client() {
        let _ = client.post(format!("{base}/interrupt")).send().await;
    }
    Ok(())
}

/// Poll `/history/{prompt_id}` until the entry appears (or we give up).
async fn wait_for_outputs(base: &str, prompt_id: &str) -> Result<Vec<ComfyOutputImage>, String> {
    let client = http_client()?;
    let deadline = std::time::Instant::now() + RUN_TIMEOUT;

    loop {
        // A cancel lands in CANCELLED_RUNS while this loop still owns the run.
        if take_cancelled(prompt_id) {
            return Err(CANCELLED.to_string());
        }

        let response = client
            .get(format!("{base}/history/{prompt_id}"))
            .send()
            .await
            .map_err(|e| format!("Failed to query ComfyUI history: {e}"))?;

        if response.status().is_success() {
            let history: Value = response.json().await.unwrap_or(Value::Null);
            if let Some(entry) = history.get(prompt_id) {
                return Ok(collect_output_images(entry));
            }
        }

        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {}s waiting for ComfyUI to finish this workflow",
                RUN_TIMEOUT.as_secs()
            ));
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn collect_output_images(entry: &Value) -> Vec<ComfyOutputImage> {
    let mut images = Vec::new();
    let Some(outputs) = entry.get("outputs").and_then(|o| o.as_object()) else {
        return images;
    };

    for (node_id, node_output) in outputs {
        let Some(list) = node_output.get("images").and_then(|i| i.as_array()) else {
            continue;
        };
        for image in list {
            let Some(filename) = image.get("filename").and_then(|f| f.as_str()) else {
                continue;
            };
            images.push(ComfyOutputImage {
                filename: filename.to_string(),
                subfolder: image
                    .get("subfolder")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string(),
                image_type: image
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("output")
                    .to_string(),
                node_id: node_id.clone(),
            });
        }
    }
    images
}

/// Download one output image to a local path, via a temp file + rename so a
/// partial download never looks like a finished result.
#[tauri::command]
pub async fn comfy_download_output(
    server_url: &str,
    image: ComfyOutputImage,
    dest_path: &str,
) -> Result<(), String> {
    let base = normalize_server_url(server_url);
    let client = http_client()?;

    let response = client
        .get(format!("{base}/view"))
        .query(&[
            ("filename", image.filename.as_str()),
            ("subfolder", image.subfolder.as_str()),
            ("type", image.image_type.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Failed to download output from ComfyUI: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "ComfyUI returned HTTP {} while downloading '{}'",
            response.status(),
            image.filename
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read output bytes: {e}"))?;

    let dest = Path::new(dest_path);
    let temp_path = temp_output_path(dest);
    std::fs::write(&temp_path, &bytes)
        .map_err(|e| format!("Failed to write '{}': {e}", temp_path.display()))?;
    if let Err(error) = std::fs::rename(&temp_path, dest) {
        // The intermediate file keeps an app-owned prefix so the stale-temp sweep can
        // clean it up, but remove it here too: a failed download should never leave a
        // partial copy behind.
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to finalize '{}': {error}", dest.display()));
    }
    Ok(())
}

/// Intermediate path for a download: same directory as the destination (so the final
/// rename is atomic on the same volume) and an app-owned prefix, so a crashed or failed
/// download leaves a file the stale-temp sweep can still clean up.
fn temp_output_path(dest: &Path) -> PathBuf {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("picaipic-comfy-{}.tmp", uuid::Uuid::new_v4()))
}
