# PicAiPic AI Plugin Contract v1 Freeze Candidate

Status: **v1 freeze candidate** after real-plugin validation with `picai-salut-color` and `picai-nafnet-restore`.

Date: 2026-06-30

This document defines the host/plugin boundary for PicAiPic AI plugins. The contract is intentionally small: the host owns plugin lifecycle, task bookkeeping, output import/adoption/discard, and UI state; plugins own algorithm quality, runtime internals, models, and backend-specific behavior.

## Terminology

The words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are used in their RFC-style normative sense.

- **Host**: PicAiPic desktop application.
- **Plugin**: a directory containing `picaipic.plugin.json` and implementation files.
- **Local HTTP plugin**: a plugin launched by the host as a child process and controlled through localhost HTTP endpoints.
- **Task**: one invocation of one capability.
- **Terminal status**: `succeeded`, `failed`, or `cancelled`/`canceled`.

## v1 goals

- The host **MUST** discover plugins from registry paths and validate their manifests.
- The host **MUST** start/stop local HTTP plugin backends without blocking the main app indefinitely.
- The host **MUST** persist task state before invoking a plugin.
- The host **MUST** treat plugin crashes, hangs, invalid output, and slow runtime behavior as bounded task/plugin failures.
- The plugin **MUST** expose capability invocation through the declared transport.
- The plugin **MUST** write generated outputs under the provided `outputDir`.
- Algorithm quality, model architecture, and backend-specific optimization are plugin-owned and outside the v1 host contract.

## Manifest

Each plugin directory **MUST** contain `picaipic.plugin.json` at its root.

Required v1 fields:

- `schemaVersion`
- `id`
- `name`
- `version`
- `compatibility.pluginApi`
- `entry.kind`
- `capabilities[]`

Recommended fields used by current UI/runtime:

- `publisher`
- `homepage`
- `license`
- `platforms[]`
- `compatibility.minPicAiPicVersion`
- `permissions`
- `runtimes[]`
- `runtime`
- `hardware`
- `install`
- `installProfiles[]`
- `smokeTest`
- `models[]`
- `contributes.menus[]`

### Local HTTP entry

For local HTTP plugins:

- `entry.kind` **MUST** be `local-http`.
- `entry.startCommand` **MUST** be a safe relative command path inside the plugin directory.
- `entry.stopCommand` **MAY** be provided and **MUST** also be a safe relative command path when present.
- `entry.defaultPort` **MAY** declare a preferred port.
- `entry.health.path` **SHOULD** point to `/health`.
- `entry.status.path` **SHOULD** point to `/status`.

The host **MAY** assign a runtime port different from `entry.defaultPort`. It passes runtime address data through environment variables:

- `PICAIPIC_PLUGIN_PORT`
- `PICAIPIC_PLUGIN_BASE_URL`

A plugin **MUST NOT** assume `entry.defaultPort` is always available. It **SHOULD** bind to `PICAIPIC_PLUGIN_PORT` when provided.

### Timeout hints

v1 freezes timeout behavior as host policy, not manifest contract.

- `smokeTest.timeoutMs` is supported for smoke command/test execution.
- Generic manifest timeout hints such as `timeouts.startMs`, `timeouts.invokeMs`, `timeouts.taskMs`, and `timeouts.cancelMs` are deferred to v1.1 unless a future plugin requires them before release.
- Plugins **SHOULD** return quickly from cancel and diagnostic endpoints regardless of manifest timeout support.

## Local HTTP endpoints

### `GET /health`

Purpose: lightweight readiness check.

The plugin **MUST** return JSON. Recommended shape:

```json
{
  "pluginId": "example-plugin",
  "ready": true,
  "version": "0.1.0"
}
```

If `entry.health.readyField` is configured, the host **MAY** use that field as readiness signal. `/health` **SHOULD NOT** load large models or perform expensive work.

### `GET /status`

Purpose: diagnostics for runtime/backend/model/capability state.

The plugin **MUST** return JSON. Shape is plugin-defined; the host treats it as diagnostics. Plugins **SHOULD** include useful model/runtime availability data, but v1 does not standardize the full schema.

### `POST /invoke/{capabilityId}`

Purpose: invoke one capability.

The host sends JSON similar to:

```json
{
  "taskId": "uuid",
  "capability": "denoise",
  "inputs": {},
  "parameters": {},
  "outputDir": "absolute path",
  "runtime": {},
  "resultPolicy": "copyIntoAlbum"
}
```

Requirements:

- The plugin **MUST** use the provided `taskId` for task state.
- The plugin **MUST** write outputs only inside `outputDir`.
- The plugin **SHOULD** validate input paths and parameters before expensive work.
- The plugin **MAY** return synchronous terminal output for short operations.
- The plugin **SHOULD** return async task state for model inference or other long operations.

Async response shape:

```json
{
  "ok": true,
  "async": true,
  "pluginId": "example-plugin",
  "taskId": "uuid",
  "status": "queued",
  "poll": { "method": "GET", "path": "/tasks/uuid", "intervalMs": 1000 },
  "events": { "method": "GET", "path": "/tasks/uuid/events", "cursor": 0 }
}
```

### `GET /tasks/{taskId}`

Purpose: return current task state.

Recognized statuses:

- `queued`
- `running`
- `cancelling`
- `succeeded`
- `failed`
- `cancelled` / `canceled`

A running task **SHOULD** include progress when available:

```json
{
  "taskId": "uuid",
  "status": "running",
  "progress": 0.42,
  "message": "Restoring image"
}
```

Terminal success **MUST** include output descriptors under `outputs`.

### `GET /tasks/{taskId}/events`

Purpose: optional event/progress polling.

This endpoint is **OPTIONAL** but recommended for responsive UI updates. It **SHOULD** support cursor-style polling with query parameters:

- `after`
- `timeoutMs`

Events **MAY** contain direct task fields or a nested `state` object. The host must still be able to fall back to `GET /tasks/{taskId}`.

### `POST /tasks/{taskId}/cancel`

Purpose: best-effort cancellation.

Requirements:

- The endpoint **MUST** return quickly.
- The plugin **SHOULD** mark queued/running tasks as `cancelling` or terminal `cancelled`.
- If immediate interruption is impossible, the plugin **SHOULD** return `cancelling` and later finish as `cancelled`, `failed`, or `succeeded` according to what actually happened.
- Host-side forceful process cleanup remains host-owned and is not exposed as plugin HTTP contract.

## Task error shape

When a plugin or host task fails, structured error metadata **SHOULD** use:

```json
{
  "error": {
    "code": "DEVICE_OOM",
    "domain": "device_backend",
    "message": "GPU ran out of memory",
    "details": {}
  }
}
```

Recommended v1 domains:

- `transport`: HTTP/process communication failed or timed out.
- `plugin`: plugin application logic failed.
- `runtime`: language/runtime/dependency failure, e.g. Python or torch import failure.
- `device_backend`: GPU/accelerator/backend failure, e.g. CUDA/ROCm/OOM.
- `filesystem`: invalid path, missing file, permission, or output validation failure.
- `task`: task lifecycle issue, e.g. cancelled/not found/invalid transition.
- `host`: host-side policy or validation failure.

The host persists these fields as task `errorCode`, `errorDomain`, and `errorDetails`, then surfaces them in `PluginActionDialog`.

## Outputs

Each output descriptor **SHOULD** include:

```json
{
  "id": "result",
  "kind": "image",
  "path": "absolute path inside outputDir",
  "mime": "image/png",
  "sourceInputId": "source"
}
```

Requirements:

- `path` **MUST** point to a file inside the task `outputDir`.
- The host **MUST** validate output paths before import/adoption.
- The host **MUST NOT** import/adopt outputs that escape the task output directory.
- Adopt/import/discard is host-owned after valid output files exist; the plugin **MUST NOT** need to participate in those operations.

## Host lifecycle guarantees

- The host starts local HTTP plugins through `entry.startCommand`.
- The host hides plugin command windows on Windows.
- The host tracks child processes it starts and can stop/kill them.
- The host does not rely on slow OS-wide port enumeration during shutdown.
- If a declared/default port is occupied, the host may allocate a fresh port.
- Stop command, taskkill, child kill/wait, task polling, and cancel paths are bounded by host-side timeouts.

## Host task guarantees

- The host creates and persists task state before invoking the plugin.
- The host polls async tasks in the background.
- If a task exceeds host poll timeout, the host requests plugin cancellation and records a failed `TIMEOUT` state.
- A failed plugin task does not invalidate the host plugin contract.
- Failed/cancelled task temp directories may be cleaned by the host.

## UI expectations

Current v1 UI surfaces:

- Settings plugin controls: Start, Stop, Restart, Refresh.
- Runtime/setup/smoke controls from manifest metadata.
- `PluginActionDialog` stages: starting, invoking, queued, running, importing, cancelling, timed out, failed, completed.
- Structured error metadata: `errorCode`, `errorDomain`, `errorDetails`.
- Output decisions: adopt/import/discard.

## Lessons from real plugins

- SA-LUT validates real image color-transfer and representative smoke-test behavior.
- NAFNet validates heavyweight Python/PyTorch startup, progress, cancellation, slow tasks, backend instability, timeout/failure handling, and output validation.
- NAFNet denoise quality/performance is intentionally treated as plugin-owned; the host contract only requires bounded lifecycle and valid outputs.

## Non-goals for v1

- Standardizing algorithm quality metrics.
- Standardizing model installation/download formats beyond declared setup/runtime metadata.
- Requiring event streaming; `/tasks/{taskId}` polling is sufficient.
- Requiring plugins to support force-kill through HTTP.
- Freezing manifest timeout hints beyond `smokeTest.timeoutMs`.
