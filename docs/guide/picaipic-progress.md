# PicAiPic Progress

Updated: 2026-07-04

This document records the current implementation status for turning the existing
PicAiPic codebase into PicAiPic: a Windows x64 local album app with lightweight
built-in functions and independently registered AI plugins.

For detailed plugin runtime status, use:

- `docs/guide/plugin-runtime-status-2026-06-20.md`
- `docs/guide/ai-plugin-interface.md`
- `docs/guide/ai-plugin-development-roadmap.md`

## Product Direction

PicAiPic is the main application body. The original source code's lightweight
album, browsing, editing, search, face, deduplication, and media features should
remain stable unless a later task explicitly changes them.

AI capabilities are not bundled into one large built-in system. Each upstream
open-source project should be wrapped as an independent PicAiPic plugin. Plugin
packages are registered by adding or dropping a plugin directory; runtime setup
is a separate profile-level workflow.

`D:\ailab\20260610133133` is reference material and a source pool for future
wrapping work. It should not be mounted into PicAiPic as one big plugin.

## Current AI Plugin Host

Implemented backend pieces:

- `src-tauri/src/t_plugin.rs`
- Tauri command registration in `src-tauri/src/main.rs`
- frontend API wrappers in `src-vite/src/common/api.js`
- plugin Settings UI in `src-vite/src/views/Settings.vue`
- menu/capability integration through `pluginStore`, file menus, media viewers,
  and `PluginActionDialog`

Current host capabilities:

- reads and validates `picaipic.plugin.json`
- maintains `plugin-registry.json`
- registers and unregisters plugin directories
- discovers plugins from app data, ProgramData, registered paths, and
  `PICAIPIC_PLUGIN_PATHS`
- lists plugins, capabilities, runtime profiles, menu contributions, setup
  state, diagnostics, logs, and validation warnings
- starts and stops `local-http` plugins
- invokes plugin capabilities through normalized HTTP payloads
- passes plugin runtime environment variables such as `PICAIPIC_PLUGIN_ROOT`,
  `PICAIPIC_PLUGIN_PORT`, `PICAIPIC_OUTPUT_DIR`, and runtime binding variables

## Runtime Profiles

Runtime profiles use three user-facing actions in Settings:

- `Setup`: records or prepares safe local runtime setup artifacts.
- `Run setup`: executes the plugin-declared setup command only after backend
  preview and explicit user confirmation.
- `Smoke`: starts the plugin, calls `POST /smoke-test`, displays structured
  results, and is the only action that can mark a profile `verified`.

The profile state flow is:

```text
notInstalled -> needsVerify -> verified / failed
```

Diagnostics alone do not mark a profile usable.

## Runtime Binding Direction

Do not make one private virtual environment per plugin the default strategy.
That becomes too large for AI plugins. Runtime environments are modeled as
bindings:

```text
external - existing user/project runtime
shared   - future PicAiPic-managed reusable runtime pool
plugin   - plugin-private runtime, used only when isolation is required
```

Profiles can declare:

- a default `runtimeBinding`
- additional `runtimeBindings`
- plugin-private `envDir` only when needed

Settings shows a runtime binding selector when multiple candidates exist. The
selected binding is passed to Setup, Run setup, and Smoke, and is persisted as a
snapshot in `profileStates`.

Host AI environment discovery now performs lightweight Python runtime discovery:

- Python paths declared by external runtime bindings
- plugin-local common environment folders such as `.venv`, `venv`, and `env`
- conda/venv folders under common user, ProgramData, and Poetry cache locations
- PATH commands such as `python`, `py`, `python3`

Discovered Python runtimes are shown in Settings and proposed as external
runtime candidates without changing the plugin manifest. Discovery is capped and
only runs a cheap `--version` probe so opening Settings does not import heavy AI
packages.

Settings also has an on-demand `Probe` action beside Python-backed runtime
bindings. Probe runs only when requested and checks the selected Python for
Python version, torch, CUDA, ROCm, DirectML, ONNX Runtime, and backend
availability hints. Probe results are persisted in `runtimeProbeStates` with a
runtime fingerprint. The host marks cached probe results stale when Python,
`pyvenv.cfg`, requirements, runtime binding, or TTL changes. Capability
invocation now performs a runtime probe preflight gate for Python-backed
profiles. Smoke remains the only action that can mark a runtime profile
`verified`.

## Probe UX Enhancement

The on-demand Probe action now provides richer detail, multi-binding cached
state display, and structured failure remediation. Three areas were improved:

Probe result detail is now grouped instead of a flat key-value list. The
Settings probe card shows five groups: General (target, duration, binding),
Python (version, platform, executable), torch (version, CUDA version, HIP
version, device count, MPS availability), Backends (per-backend available
state with device count, version, and tensor probe result), ONNX Runtime
(version and providers), and Packages (torch, torchDirectML, onnxruntime
availability and errors). Each item carries a tone — ok, bad, or neutral —
rendered with color cues so users can scan the result at a glance.

Multi-binding cached state display lets users see the probe status of every
runtime binding without probing each one. The backend `list_ai_plugins`
response now includes a `runtimeProbeStates` array on each install profile,
containing all persisted probe states for that plugin+profile pair. The
runtime binding selector in Settings appends a status marker to each option:
`✓` for passed, `✗` for failed, `⟳` for stale, and no marker for not-probed.
The frontend matches probe states to bindings by Python path first, then by
binding id, so switching the binding selector shows the correct cached result
immediately.

Failure remediation advice is now structured as `action` or `diagnostic`
items instead of a flat string list. Action items are rendered with a `→`
prefix and primary color; diagnostic items use a muted style. The advice
engine covers twelve failure scenarios: stale cache (three sub-reasons),
available runtime, torch not installed, torch import error, ONNX Runtime
missing, DirectML not installed, DirectML initialization failure, GPU device
count zero, tensor probe failure (OOM and non-OOM), probe timeout, no binding
selected, and unknown fallback. Tensor probe failure takes priority over the
"available → Smoke" path so a failed GPU tensor test is never hidden behind a
green checkmark.

## Plugin Action Dialog Progress

The PluginActionDialog (shown when a user triggers a plugin capability from the
image context menu or toolbar) previously showed only a spinner during the
entire task. It now shows real-time task progress and supports cancellation.

The dialog receives `taskStatus`, `taskProgress`, and `taskMessage` props from
the parent Content component. The `waitForPluginTaskOutput()` polling loop
updates these fields on every `getAiPluginTask` poll, so the dialog reflects
the current task state (queued, running, cancelling), a progress bar
(0–100%), and the plugin's progress message text. A "Cancel Task" button
appears when the task is in an active state and calls the existing
`cancel_ai_plugin_task` backend command.

## Setup Command Streaming And Cancellation

The Run setup command previously executed as a black box: stdout and stderr
were collected only after the command finished, the UI showed a loading
spinner with no progress, and there was no way to cancel a long-running
install.

The backend `run_setup_command()` now spawns the child process and reads
stdout and stderr line by line. Every 5 lines the job state is saved to the
registry, so the frontend can poll `list_ai_plugins` and see the log grow in
real time. A new `SetupCancellationState` global state tracks cancel requests
by job id. The command loop checks the cancel flag on each iteration and
kills the child process if cancellation is requested. A new
`cancel_ai_plugin_setup` Tauri command exposes this to the frontend.

The Settings UI now runs a 2-second polling loop during setup execution to
refresh the plugin list and show the live job status, progress bar, and
scrolling log output (max-height with overflow). A "Cancel" button appears
next to the progress percentage when the job is running. The setup job log
area is now scrollable instead of rendering the full log as a single block.

## Filesystem Handoff

Plugin invocation still uses loopback HTTP with JSON control payloads and file
paths for large image/LUT data. Host-created default outputs are now
task-scoped:

```text
app-cache\plugins\<pluginId>\tasks\<taskId>\outputs
```

The host validates returned output paths before accepting a successful invoke
response:

- returned paths must stay inside the task output directory
- returned files must exist and be non-empty regular files

The host also performs ledger-aware task cache cleanup. Failed and cancelled
task directories are removed best-effort. Orphan task directories are removed
after 24 hours, orphan `.tmp` files after 15 minutes, and unadopted successful
outputs are expired after 24 hours, marked `discarded`, and removed from disk.
SA-LUT image output now writes to a same-directory temporary file, flushes and
fsyncs it, then commits with atomic replace before returning the final path.

Plugin invocation now records a host-owned `taskStates` ledger in
`plugin-registry.json`. Tasks move through `queued`, `running`, `cancelling`,
`succeeded`, `failed`, `cancelled`, and later `imported` / `discarded` states.
Successful invoke responses store returned outputs in the ledger. After SA-LUT
output is imported into the current album, the frontend marks the task adopted
and asks the host to delete the task directory. Settings shows recent plugin
tasks and can explicitly discard unadopted successful tasks, marking them
`discarded` and deleting their task directory. Failed task states persist
structured error metadata: `errorCode`, `errorDomain`, optional details, and a
host-derived `retryable` flag. Retryable tasks store a safe invoke request
snapshot and can be retried from Settings. Retry creates a fresh task id and
task output directory instead of overwriting the failed task.

Cancellation is part of the task contract. The host exposes
`cancel_ai_plugin_task`, records `cancelling` / `cancelled` / failed cancel
states, and calls plugin `POST /tasks/{taskId}/cancel`. SA-LUT implements a
best-effort cancel registry and checks cancellation around model load, image
read, inference boundaries, output encode/write, and finalization. This is not
a hard interrupt for a model call already inside a blocking inference operation.

SA-LUT `color-transfer` now uses async invoke. `POST /invoke/color-transfer`
returns `202 Accepted` with a `taskId`, initial `queued` status, and task
tracking endpoints. Work runs in a plugin-side background worker with one active
task by default. The plugin exposes `GET /tasks/{taskId}/events` as a long-poll
event stream; the host consumes events first and falls back to
`GET /tasks/{taskId}` for older plugins. Failed and cancelled task directories
are cleaned best-effort; successful outputs remain for import/adopt/discard and
are later expired by host-side TTL cleanup if they were never adopted.
Settings now shows recent plugin tasks with clearer status badges, progress,
output counts, retry/cancel/discard actions, and a `Cleaned` label for expired
or discarded outputs.

## SA-LUT Plugin

Current local plugin:

```text
plugins/picai-salut-color
```

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

`color-transfer` is wired through the local HTTP plugin wrapper.
`export-lut` is declared but still not implemented.

The ROCm profile declares the existing external Windows SA-LUT runtime:

```text
D:\ailab\20260610133133\backend\venv\Scripts\python.exe
```

This avoids duplicating a large PyTorch/ROCm environment under the plugin
directory. A plugin-private ROCm fallback binding is also declared, but it is
not the default.

## Verification Commands

Latest checks used during this work:

```text
cargo check
cargo fmt
cd src-vite && npm run build
python -m json.tool plugins\picai-salut-color\picaipic.plugin.json
python -m py_compile plugins\picai-salut-color\backend\main.py plugins\picai-salut-color\backend\salut_adapter.py
python scripts\stress_salut_async.py --tasks 8 --duration-ms 300 --cancel-every 3
python scripts\stress_salut_http.py --tasks 6 --duration-ms 250 --cancel-every 3
python -m json.tool plugins\picai-nafnet-restore\picaipic.plugin.json
python -m py_compile plugins\picai-nafnet-restore\backend\main.py plugins\picai-nafnet-restore\backend\nafnet_adapter.py plugins\picai-nafnet-restore\backend\denoiser.py
python scripts\stress_nafnet_http.py --tasks 4 --duration-ms 120 --cancel-every 2
```

## Next Work

- `picai-nafnet-restore` is already wired as the second plugin; both SA-LUT
  and NAFNet have passed discovery, runtime Probe, Setup, Smoke, and real
  image invoke/adopt/discard from the host UI. Keep Smoke as the final
  verification gate for every runtime profile.
- Runtime conflict detection is implemented (2026-07-03): the host compares
  probe-reported package versions against the plugin's requirements specifiers
  and hard-blocks capability invocation on `version_mismatch`/`missing`
  conflicts, advising a switch to a plugin-private runtime. Auto-switching the
  profile is still future work.
- Uninstall mode is implemented (2026-07-03): users choose between "code only"
  (delete the plugin code package) and "code + data & runtimes" (also remove
  plugin-data, plugin-cache, plugin-outputs, and plugin-private runtimes).
  Shared runtimes are never deleted.
- Security hardening A+B+C is landed (2026-07-04): startup bearer-token auth
  (A), Ed25519 package signing with a user-managed trust store (B), and v1
  process sandboxing (C). The sandbox applies a non-recursive deny-write ACE
  via `icacls` on sensitive user directories before spawning the plugin
  process, copies external input files into a staged readable area before
  invoke, and preserves full GPU/CPU access (spike-confirmed). ACLs are
  revoked on stop/crash/shutdown. Network blocking, macOS Seatbelt, and
  Linux seccomp are future work. See `docs/ai-plugin-security-hardening.md`.
- Next priority: end-to-end validation of the sandboxed plugin lifecycle with
  SA-LUT/NAFNet (GPU usable under deny-ACL, write confinement effective, ACL
  revocation leaves no residue), then model import / external model directory
  binding, so users with model files already on disk do not need to hand-edit
  `.local.env`.
- Avoid concrete `export-lut` business logic until runtime binding confidence
  is stable across both SA-LUT and NAFNet.
