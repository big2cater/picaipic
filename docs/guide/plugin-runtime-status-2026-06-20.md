# PicAiPic Plugin Runtime Status - 2026-06-22

This is the canonical clean handoff snapshot for the current AI plugin runtime
work. It supersedes older progress notes that still describe one-venv-per-plugin
as the default direction.

This note records the current AI plugin runtime state after the Setup / Verify /
Smoke work and the Probe UX enhancement. It is meant as a clean handoff snapshot
because `docs/guide/目前的开发情况.md` currently has encoding issues in this
workspace.

## Current Model

- Plugin packages are added by registering a plugin directory. The runtime
  profile `Setup` button does not install a plugin package.
- Runtime profiles use `Setup / Verify / Smoke` in Settings.
- `Setup` means preparing or recording the runtime environment for one profile,
  such as CUDA, ROCm, DirectML, or CPU.
- `Verify` starts the plugin and refreshes diagnostics.
- `Smoke` starts the plugin, calls `POST /smoke-test`, displays the structured
  result, and persists `verified` or `failed`.

## Runtime Environment Direction

Do not make one private virtual environment per plugin the default strategy.
That becomes too large and slow for AI plugins, especially when several plugins
need PyTorch, CUDA, ROCm, DirectML, OpenCV, diffusers, or similar packages.

The target model should separate three things:

- Plugin package: protocol adapter, manifest, small scripts, UI/menu
  contributions, and plugin-owned code.
- Runtime environment: reusable Python or native runtime that may be external,
  shared by multiple plugins, or private only when isolation is required.
- Runtime profile state: PicAiPic's record that a plugin profile is bound to a
  runtime and has passed, failed, or still needs smoke verification.

Runtime scopes should be explicit:

```text
external - an existing user or project runtime, such as SA-LUT's current ROCm venv
shared   - a PicAiPic-managed runtime pool reused by compatible plugins
plugin   - a plugin-private runtime, used only when isolation is necessary
```

`envDir` should remain optional and should not imply that every profile creates a
plugin-private venv. Future setup work should prefer binding a profile to an
existing external or shared runtime, then using Smoke as the proof that the
binding really works.

SA-LUT is the current example: it already has a working external Windows ROCm
runtime at `D:\ailab\20260610133133\backend\venv`. PicAiPic should be able to
record and preview that binding instead of duplicating the environment under
the plugin directory.

The manifest/profile shape now supports an optional `runtimeBinding` object and
a `runtimeBindings` candidate list:

```json
{
  "scope": "external",
  "kind": "python",
  "id": "salut-windows-rocm",
  "label": "Existing SA-LUT ROCm runtime",
  "python": "D:\\ailab\\20260610133133\\backend\\venv\\Scripts\\python.exe",
  "root": "D:\\ailab\\20260610133133\\backend",
  "requirements": "backend/requirements-rocm.txt"
}
```

`scope: "plugin"` remains available for private runtimes, but it is opt-in.
When both fields exist, `runtimeBinding` is treated as the default candidate and
`runtimeBindings` supplies additional choices.

Current implementation supports:

- single default `runtimeBinding`
- multiple `runtimeBindings` candidates
- a Settings runtime binding selector
- lightweight Python runtime discovery in Host AI environment
- common plugin-local venv discovery for `.venv`, `venv`, and `env`
- conda/venv discovery under common user, ProgramData, and Poetry cache roots
- on-demand Python runtime probes for torch, CUDA, ROCm, DirectML, ONNX Runtime,
  and selected-backend availability
- discovered Python runtimes as extra external candidates
- request-time `runtimeBinding` override for discovered runtimes not declared
  in the manifest
- persistence of the selected binding snapshot in `profileStates`
- persistence of Python runtime probe states in `runtimeProbeStates`
- runtime probe fingerprint and TTL staleness checks
- invocation preflight gate for Python-backed runtime profiles

## Persisted State

PicAiPic persists runtime profile state in `plugin-registry.json`.

- `profileStates`: latest state for each `pluginId + profileId`.
  Profile state now stores the selected `runtimeBinding` snapshot so Setup,
  Run setup, and Smoke can use the same runtime choice.
- `setupJobs`: latest setup job records with status, progress, message, error,
  and log lines.
- `runtimeProbeStates`: latest probe state for a selected
  `pluginId + profileId + backend + runtimeBinding`. Probe state stores the
  selected runtime binding, result payload, error, fingerprint, and staleness
  metadata.

Profile state flow:

```text
notInstalled -> needsVerify -> verified / failed
```

Only a passing smoke test may mark a profile `verified`. Diagnostics alone must
not mark a profile usable.

## Setup Job Skeleton

The Setup button currently creates a persisted setup job record and safe local
artifacts only.

It currently does:

- create or confirm the profile `envDir` inside the plugin root when declared
  for plugin-private runtimes
- check whether the declared requirements file exists
- write `logs/setup-<jobId>.log`
- persist job status, progress, message, error, and log lines

It does not yet:

- create virtual environments
- install Python requirements
- download model files
- install CUDA, ROCm, DirectML, or CPU runtime dependencies
- run plugin setup scripts
- mutate the system outside the plugin root and PicAiPic registry state

The job record already has the shape future installers need:

```json
{
  "id": "uuid",
  "pluginId": "picai-salut-color",
  "profileId": "windows-amd-rocm",
  "backend": "rocm",
  "capability": "color-transfer",
  "status": "needsVerify",
  "progress": 100,
  "message": "Runtime setup artifacts are ready. Run Verify or Smoke next.",
  "log": [
    "Created setup job record.",
    "Created or confirmed profile environment directory: ...",
    "Wrote setup log: ...",
    "Dependency installation is not implemented yet; no setup command was executed.",
    "Run Verify or Smoke to validate the existing runtime."
  ]
}
```

Settings shows the latest setup job under the profile row.

## Setup Command Runner

The backend now has a guarded command runner:

```text
run_ai_plugin_profile_setup_command
```

It requires `allowCommandExecution: true`. Without that explicit flag, it
returns an error and does not execute anything.

The runner:

- uses only the plugin manifest `install.command`
- requires the command path to be a safe relative path
- runs with the plugin root as the working directory
- accepts a full `runtimeBinding` override for discovered runtimes
- injects profile-aware environment variables such as
  `PICAIPIC_PLUGIN_PROFILE_ID`, `PICAIPIC_PLUGIN_BACKEND`,
  `PICAIPIC_PLUGIN_RUNTIME_SCOPE`, `PICAIPIC_PLUGIN_PYTHON`,
  `PICAIPIC_PLUGIN_ENV_DIR`, and `PICAIPIC_PLUGIN_REQUIREMENTS`
- captures stdout and stderr
- appends output to `logs/setup-<jobId>.log`
- finishes in `needsVerify` on success
- finishes in `failed` on command failure

The Settings UI now exposes a separate `Run setup` button when a plugin declares
`install.command`. This button:

- asks the backend for a setup preview before any execution
- sends the selected `runtimeBindingId` to preview and execution
- shows a warning confirmation dialog with plugin, profile, backend, and command
- shows the runtime scope, Python executable, resolved working directory,
  environment directory, requirements path, and preview warnings
- passes `allowCommandExecution: true` only after confirmation
- keeps the profile in `needsVerify` after command success
- still requires `Smoke` before the profile can become `verified`

The visible `Setup` button still only prepares safe local artifacts.

## SA-LUT Plugin

SA-LUT currently exposes:

```text
GET  /health
GET  /status
GET  /diagnostics
GET  /tasks/{taskId}
GET  /tasks/{taskId}/events
POST /smoke-test
POST /verify
POST /invoke/color-transfer
POST /invoke/export-lut
POST /tasks/{taskId}/cancel
```

The smoke test checks:

- Python executable and version
- torch availability
- requested backend availability
- CUDA / ROCm state through torch
- model file presence and Git LFS pointer detection
- model loading
- tiny input probe

`export-lut` is still declared but not implemented.

The ROCm install profile now declares an external runtime binding to the
existing SA-LUT Windows ROCm venv. `install.bat` detects
`PICAIPIC_PLUGIN_RUNTIME_SCOPE=external` and verifies the external Python
instead of creating another plugin-local venv.

Setup, Run setup, and Smoke now pass the same selected runtime binding. Settings
shows a selector when a profile declares multiple candidate bindings. Smoke
persists the binding snapshot together with the final `verified` or `failed`
profile state.

Host environment discovery now includes lightweight Python runtime discovery.
PicAiPic probes Python executables declared by plugin external bindings,
plugin-local common venv folders, conda/venv folders under common user and
ProgramData roots, Poetry cached virtualenvs, and common PATH Python commands
with `--version`. Available discovered runtimes are shown in Settings and
proposed as extra external runtime binding candidates without modifying the
plugin manifest. Discovery is capped and intentionally avoids importing torch or
other heavy AI packages during the default Settings scan.

Each Python-backed runtime binding can be probed on demand from Settings. The
probe runs a short Python script against the selected interpreter, reports
Python and package versions, checks torch CUDA/ROCm/MPS state, checks
`torch_directml`, checks ONNX Runtime providers, and performs a tiny CUDA/ROCm
tensor probe when that backend is requested. Probe results are persisted in
`runtimeProbeStates`, not `profileStates`.

Probe cache invalidation is fingerprint based. The host stores Python executable
existence, size, mtime, inferred runtime root, `pyvenv.cfg` hash, requirements
hash, and runtime binding hash. On plugin listing and invocation preflight, the
host recomputes the fingerprint and marks the cached probe stale if the
fingerprint changed, the Python executable disappeared, or the TTL expired.
Current TTLs are 24 hours for external/shared runtimes and 7 days for
plugin-private runtimes. Setup and Run setup clear the affected profile's probe
cache.

Capability invocation now performs a runtime probe preflight gate for
Python-backed profiles. Missing, failed, or stale probe state blocks invocation
before the host sends the task to the plugin. Probe still does not mark a
profile `verified`; Smoke remains the only verification gate.

## Filesystem Handoff Status

Plugin tasks use loopback HTTP with JSON payloads and filesystem paths for large
image/LUT data. The default host output directory for invocation is now
task-scoped under:

```text
app-cache\plugins\<pluginId>\tasks\<taskId>\outputs
```

The host validates successful invoke responses before returning them to the
frontend. Every returned output path must canonicalize inside that task output
directory and must point to a non-empty regular file. This prevents plugins from
returning arbitrary external paths or half-created empty outputs as successful
results.

The host performs best-effort task cache cleanup before invocation:

- failed and cancelled task directories are removed best-effort
- orphan task directories older than 24 hours are removed
- orphan `.tmp` files older than 15 minutes are removed
- unadopted successful outputs older than 24 hours are marked `discarded` and
  removed from disk

SA-LUT output writes now use same-directory temporary files, flush/fsync, and
atomic replace before returning the final image path.

The host now persists a `taskStates` ledger in `plugin-registry.json`. Plugin
invoke records start as `queued`, then become `running`, `succeeded`, `failed`,
`cancelled`, or later `imported` / `discarded`. Successful task records store
returned output entries. After SA-LUT output is imported into the current album,
the frontend calls the host adoption command, which marks the task `imported`,
sets `adopted: true`, and deletes the task-scoped directory. Settings shows
recent task states and can explicitly discard unadopted successful tasks,
marking them `discarded` and deleting the task-scoped directory. Failed task
records persist structured error metadata: `errorCode`, `errorDomain`, optional
details, and a host-derived `retryable` flag. Retryable records store a safe
invoke request snapshot and can be retried from Settings. Retry creates a fresh
task id and task output directory instead of overwriting the failed task.

Cancellation is now part of the task contract. The host exposes
`cancel_ai_plugin_task`, records `cancelling` / `cancelled` / failed cancel
states, and calls plugin `POST /tasks/{taskId}/cancel`. SA-LUT implements a
best-effort cancel registry and checks cancellation around model load, image
read, inference boundaries, output encode/write, and finalization. This is not
a hard interrupt for a model call already inside a blocking inference operation.

SA-LUT `POST /invoke/color-transfer` now returns quickly with `202 Accepted`,
`queued`, and task tracking information. Real work runs in a single active
background worker queue to avoid unbounded GPU concurrency. The plugin exposes
`GET /tasks/{taskId}/events?after={seq}&timeoutMs={ms}` as a long-poll event
stream; the host consumes events first and falls back to `GET /tasks/{taskId}`
for older plugins. The plugin keeps a strict task state machine and event log.
Failed or cancelled host task directories are cleaned best-effort; successful
outputs remain until import/adopt/discard. Unadopted successful outputs are
expired by host-side TTL cleanup and marked `discarded`.
Settings shows recent plugin tasks with status badges, progress, output counts,
retry/cancel/discard actions, and a `Cleaned` label for discarded or expired
outputs.

The SA-LUT adapter also supports `parameters.mockTask: true`, which runs a
cancellable sleep-based mock task without loading the model. This is used by the
async stress scripts to validate queueing, event delivery, and cancellation
without touching GPU state.

## Verified Checks

The latest implementation was checked with:

```text
cargo check
cargo fmt
cd src-vite && npm run build
python -m json.tool plugins\picai-salut-color\picaipic.plugin.json
python -m py_compile plugins\picai-salut-color\backend\main.py plugins\picai-salut-color\backend\salut_adapter.py
python scripts\stress_salut_async.py --tasks 8 --duration-ms 300 --cancel-every 3
python scripts\stress_salut_http.py --tasks 6 --duration-ms 250 --cancel-every 3
```

## Probe UX Enhancement

The on-demand Probe action has been enhanced in three areas: grouped detail
display, multi-binding cached state, and structured failure remediation.

### Grouped Detail Display

The probe result card in Settings previously showed a flat 2-column grid of
up to 8 fields. It now shows grouped sections with all data collected by the
probe script:

- **General**: target backend, duration, binding label
- **Python**: version, platform, executable path
- **torch**: version, CUDA version, HIP version, device count, MPS availability
- **Backends**: per-backend row (cuda, rocm, directml, mps, openvino, cpu) with
  available marker, version, device count, and tensor probe result
- **ONNX Runtime**: version and provider list
- **Packages**: torch, torchDirectML, onnxruntime availability and errors

Each detail item carries a `tone` — `ok` (green), `bad` (red), or `neutral` —
so users can scan the result visually. The old `runtimeProbeDetails()` function
is retained for `formatRuntimeProbeResult()` which builds the `title` attribute
tooltip.

### Multi-Binding Cached State

The backend `list_ai_plugins` response now includes a `runtimeProbeStates`
array on each `PluginInstallProfileSummary`, containing all persisted probe
states for that plugin+profile pair across all bindings. The old single
`runtimeProbeState` field is retained for backward compatibility.

The frontend `profileRuntimeProbeResult()` function matches the currently
selected binding to a probe state by Python path first, then by binding id,
using a new `matchProbeStateByBinding()` helper. This means switching the
runtime binding selector immediately shows the correct cached probe result
without re-probing.

The binding selector dropdown appends a status marker to each option label:
`✓` for passed, `✗` for failed, `⟳` for stale, and no marker for not-probed.
A new `bindingProbeStatus()` function checks live probe results first, then
falls back to persisted backend states.

### Structured Failure Remediation

The `runtimeProbeAdvice()` function now returns `ProbeAdvice[]` where each item
is `{ text: string; kind: 'action' | 'diagnostic' }` instead of a flat
`string[]`. Action items are rendered with a `→` prefix and primary color;
diagnostic items use a muted style.

The advice engine now covers twelve failure scenarios:

1. Stale cache — `fingerprint_changed`, `python_missing`, `ttl_expired`
2. Available runtime — next step is Smoke
3. Tensor probe explicitly failed — takes priority over "available", shows
   error text and OOM hint
4. torch not installed — install action
5. torch import error (installed but crashed) — diagnostic with error text
6. ONNX Runtime not installed — install action for ONNX-dependent backends
7. DirectML not installed — install action
8. DirectML initialization failure — diagnostic
9. GPU device count zero — diagnostic about driver or GPU occupancy
10. Probe script timeout — action to try CPU
11. No binding selected — action to select a runtime
12. Unknown fallback — action to open diagnostics

New i18n keys were added to `fallback`, `en.json`, and `zh.json` for all new
advice messages and detail group labels. Other locales fall back to English.

## Plugin Action Dialog Progress

The PluginActionDialog previously showed only a spinner during task execution.
It now displays real-time task progress and supports cancellation.

Content.vue's `waitForPluginTaskOutput()` polling loop updates
`taskStatus`, `taskProgress`, and `taskMessage` on every
`getAiPluginTask` poll. The dialog renders a status label (Queued, Running,
Cancelling, Done, Failed, Cancelled), a progress bar (0–100%), and the
plugin's progress message text. A "Cancel Task" button appears when the task
is in an active state and calls the existing `cancel_ai_plugin_task` backend
command.

## Setup Command Streaming And Cancellation

The Run setup command previously executed as a black box. stdout and stderr
were collected only after the command finished, the UI showed a loading
spinner with no progress, and there was no way to cancel a long-running
install.

### Backend Changes

`run_setup_command()` now spawns the child process and reads stdout and stderr
line by line using `tokio::io::BufReader`. Every 5 lines the job state is
saved to the registry via `save_setup_job()`, so the frontend can poll
`list_ai_plugins` and see the log grow in real time.

A new `SetupCancellationState` global state tracks cancel requests by job id.
The command loop checks the cancel flag on each iteration and kills the child
process via `child.kill().await` if cancellation is requested. A new
`cancel_ai_plugin_setup` Tauri command exposes this to the frontend. The
command is registered in `main.rs` alongside `SetupCancellationState::new()`.

`run_ai_plugin_profile_setup_command` signature now accepts
`cancel_state: tauri::State<'_, SetupCancellationState>` and passes it
through to `run_setup_command()`.

### Frontend Changes

Settings.vue's `runAiPluginProfileSetup()` now starts a 2-second polling
loop (`setInterval`) that calls `loadAiPluginPanel(false)` to refresh the
plugin list while the setup command runs. The polling stops when the command
completes or errors.

The setup job display area in Settings now shows:
- A progress bar (`<progress>`) when the job status is `running`
- A scrollable log area (`max-h-32 overflow-auto`) instead of a flat block
- A "Cancel" button that appears when the job is running, calling
  `cancelAiPluginSetup(jobId)` which invokes the new
  `cancel_ai_plugin_setup` Tauri command

The cancel button finds the running job id from the current plugin list by
matching the plugin id and profile id tracked in `aiPluginSetupRunningFor`.

New i18n keys `runSetupCancelled` and `cancelSetup` were added to `fallback`,
`en.json`, and `zh.json`.

## Next Good Step

The async task pipe, Probe UX hardening, plugin action dialog progress, setup
streaming/cancellation, and a second plugin package are now complete. The next
good step is validating the second plugin through the full host UI flow:

- use `plugins/picai-nafnet-restore` to validate registered discovery, runtime
  Probe, Setup, Smoke, and real invoke/adopt/discard from PicAiPic
- keep the runtime probe preflight gate mandatory before invoke
- keep plugin-private `envDir` as an opt-in fallback, not the default
- continue using `Smoke` as the only gate to `verified`
- continue avoiding concrete `export-lut` business logic until SA-LUT and
  NAFNet both validate the runtime and task UX from the host UI
