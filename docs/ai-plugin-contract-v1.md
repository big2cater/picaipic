# PicAiPic AI Plugin Contract v1

Status: **v1 frozen baseline** after real-plugin validation with `picai-salut-color` and `picai-nafnet-restore`.

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

### Privacy and network permissions

Plugins **SHOULD** declare privacy-relevant behavior through `permissions`.

Current v1 shape:

```json
{
  "permissions": {
    "readSelectedFiles": true,
    "writeOutputDir": true,
    "writeSourceFiles": false,
    "launchChildProcesses": true,
    "network": {
      "runtime": false,
      "setupDownloads": false,
      "uploadSelectedFiles": false,
      "uploadOutputs": false,
      "allowedDomains": []
    }
  }
}
```

Meaning:

- `network.setupDownloads`: setup/install may download packages, runtimes, or models.
- `network.runtime`: the plugin may access the network while running.
- `network.uploadSelectedFiles`: the plugin may upload user-selected inputs.
- `network.uploadOutputs`: the plugin may upload generated outputs.
- `network.allowedDomains`: declared remote domains for the above access.

For compatibility, older manifests may still use `permissions.network: false`.
Hosts may normalize that legacy value to `runtime=false` with all upload flags
false.

PicAiPic v1 currently provides **declaration + UI authorization + stored grant
state**, not a real operating-system-level network sandbox. A malicious plugin
process may still attempt network access outside its declaration. Therefore:

- user-content upload **MUST** be treated as a privacy boundary
- undeclared upload/network access is a host/plugin trust violation
- future hardening may add OS-level enforcement in a later version

### Menu and button contributions

Plugin action buttons **MUST** be contributed through manifest metadata, not hard-coded in the PicAiPic body for a specific plugin id.

The host owns generic placement surfaces such as:

- `image.contextMenu`
- `image.toolbar`

A plugin contributes actions by declaring `contributes.menus[]` entries with `id`, `label`, `capability`, `contexts[]`, `placements[]`, optional `icon`, and optional `order`.

The host **SHOULD** render these entries automatically and invoke the declared capability through the generic plugin action flow. The host **MUST NOT** require new frontend code for every new plugin that only needs existing placements, input kinds, output kinds, and parameter schema controls.

New host code is appropriate only when adding a reusable surface or primitive, such as a new placement, input kind, output kind, parameter control, or host-mediated action type. It is not appropriate for adding a single plugin-specific button.

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
- `PICAIPIC_PLUGIN_AUTH_TOKEN`

A plugin **MUST NOT** assume `entry.defaultPort` is always available. It **SHOULD** bind to `PICAIPIC_PLUGIN_PORT` when provided.

### Startup token authentication

The host generates a random auth token per plugin start and injects it as
`PICAIPIC_PLUGIN_AUTH_TOKEN`. The host sends `Authorization: Bearer <token>`
on every HTTP request to the plugin except `/health`.

- A plugin **SHOULD** read `PICAIPIC_PLUGIN_AUTH_TOKEN` and require
  `Authorization: Bearer <token>` on all endpoints except `/health`.
- `/health` **MUST** be publicly accessible (no auth) so the host can probe
  liveness including stale or externally-managed services.
- If `PICAIPIC_PLUGIN_AUTH_TOKEN` is empty or unset, auth **MUST NOT** be
  enforced (backward compatibility / developer mode).
- A request without a matching token **SHOULD** receive `401 Unauthorized`.

This prevents a user or external process from driving a plugin backend
independently by discovering its loopback port.

### Package signature verification

Plugin packages (zip files) **MAY** be signed with an Ed25519 signature. The
signature covers the canonical JSON of `picaipic.package.json` (compact,
object keys sorted lexicographically, with the `signature` field itself
omitted).

- `picaipic.package.json` **MAY** include a `signature` object:
  `{ algorithm: "ed25519", publicKey: "<base64>", value: "<base64>" }`.
- At install time, the host verifies the signature against the embedded
  `publicKey`. If verification fails, install is refused.
- If the signature is valid but the publisher (from `manifest.publisher`) is
  not in the user's trust store, the host returns a `TRUST_REQUIRED` error.
  The frontend prompts the user to trust the publisher; once trusted, install
  retries automatically.
- If no signature is present, install is refused unless developer mode is
  enabled (`PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1` env var).
- The host maintains a `trustedPublishers` map in the plugin registry, keyed
  by publisher name, storing the trusted public key and timestamp.

Signing tooling:

```bash
# Generate an Ed25519 keypair (prints base64 private + public keys)
python scripts/sign_plugin.py generate-key

# Sign a package manifest in-place
python scripts/sign_plugin.py sign <picaipic.package.json> <private-key-base64>
```

The `package_plugin.ps1` script accepts `-SignKeyFile <path>` to sign the
manifest automatically during packaging.

### Process confinement (v1, Windows)

The host **MAY** confine plugin processes so they cannot **write** to
sensitive user directories. This is a runtime enforcement layered on top
of the signature/trust checks above; plugins do not need to opt in and are
not generally aware of it.

- **Write confinement (deny-ACL)**: before spawning a plugin process, the
  host applies a non-recursive deny-write ACE (`icacls /deny <user>:(W) /L`)
  on sensitive user directories — `Desktop`, `Documents`, `Pictures`,
  `Videos` under `%USERPROFILE%`, plus any extra paths listed in the
  `PICAIPIC_SANDBOX_DENY_PATHS` env var (semicolon-separated). The plugin
  can still **read** these directories; only writes are blocked.
- **Writable directories**: the plugin may write to its
  `plugin-data/<id>`, `plugin-cache/<id>`, `plugin-outputs/<id>`, plugin
  code directory, `plugin-runtimes/<id>`, and `shared-runtimes/<id>`.
  These are never denied.
- **Authorized reads via input staging**: when a task is invoked with
  input files that live outside the plugin's writable area (e.g. a
  user-selected source image), the host **copies** those files into
  `plugin-cache/<id>/tasks/<taskId>/inputs/` before invoking, and rewrites
  the `path` fields in the `inputs` payload to point at the staged copies.
  The plugin reads from the staged paths; it never needs raw access to the
  original locations.
- **GPU/CPU access is fully preserved**: the deny-ACL approach was
  confirmed (via `scripts/sandbox_gpu_spike.py`) not to break ROCm/CUDA
  driver initialization — sandboxed plugins can still run GPU inference.
- **Disable switch**: set `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` to skip
  sandboxing entirely (useful for plugin development/debugging).
- **Scope of v1**: write confinement + input staging only. Network
  blocking, macOS Seatbelt, and Linux seccomp are future work.

ACLs are revoked (`icacls /remove:d`) when the plugin process is torn down
— on stop, restart, crash-detection, and app shutdown. Revocation is
idempotent, so a leftover ACE from a crashed prior run is cleaned up
safely on the next apply.

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
- The host distinguishes managed runtimes it started from unmanaged/stale localhost services.
- The UI **SHOULD** show a plugin as Running only when the reachable runtime is host-managed.
- The host does not rely on slow OS-wide port enumeration during shutdown.
- If a declared/default port is occupied, the host may allocate a fresh port.
- Stop command, taskkill, child kill/wait, task polling, and cancel paths are bounded by host-side timeouts.

### Managed runtime state

For local HTTP plugins, reachability alone is not the same as "running in PicAiPic".

The host may expose status metadata equivalent to:

```json
{
  "pluginId": "example-plugin",
  "reachable": true,
  "managed": true,
  "url": "http://127.0.0.1:18123/status"
}
```

Meaning:

- `reachable=true, managed=true`: the host currently owns/tracks this runtime.
- `reachable=true, managed=false`: a matching localhost service is reachable, but it is not the current host-managed runtime.
- `reachable=false`: no usable runtime is reachable at the probed URL.

The host **MUST NOT** treat unmanaged reachability as proof that Stop failed. The host **MAY** allocate a fresh runtime port when a stale unmanaged listener occupies the declared/default port.

Plugins with `entry.stopCommand` **SHOULD** make that command best-effort idempotent and may clean stale backend processes by script path or assigned port. Failure to clean an operating-system-protected stale process must not make the host UI report the current managed runtime as Running.

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
- NAFNet exposed the need to separate managed host runtime state from stale default-port reachability.
- NAFNet denoise quality/performance is intentionally treated as plugin-owned; the host contract only requires bounded lifecycle and valid outputs.

## Non-goals for v1

- Standardizing algorithm quality metrics.
- Standardizing model installation/download formats beyond declared setup/runtime metadata.
- Requiring event streaming; `/tasks/{taskId}` polling is sufficient.
- Requiring plugins to support force-kill through HTTP.
- Freezing manifest timeout hints beyond `smokeTest.timeoutMs`.


