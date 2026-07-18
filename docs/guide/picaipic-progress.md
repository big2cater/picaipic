# PicAiPic Progress

Updated: 2026-07-18

## 2026-07-18 merge + v1.1.0 line

- Merged Live Photo polish (#1) and plugin sandbox Phase 0–2 + private runtime + model UX (#2) to `main`.
- Windows/Linux PR builds green after Actions artifact-quota hardening (`pr-build.yml` best-effort upload).
- App version aligned to **1.1.0** for the next signed multi-arch release draft.


This document records the current implementation status for turning the existing
PicAiPic codebase into PicAiPic: a Windows x64 local album app with lightweight
built-in functions and independently registered AI plugins.

For detailed plugin runtime status, use:

- `docs/guide/plugin-runtime-status-2026-06-20.md`
- `docs/guide/ai-plugin-interface.md`
- `docs/guide/ai-plugin-development-roadmap.md`

## 2026-07-17 Live Photo / Motion Photo + reliability fixes

### Live Photo / Motion Photo (schema v6)

- Apple Live Photo: HEIC/JPEG still + companion MOV paired by EXIF ContentIdentifier
  (`Tag(Context::Tiff, 0x0011)`) and ffprobe `com.apple.quicktime.content.identifier`
  (dotted and underscored key variants). Stem-based same-folder fallback when UUID is missing.
- Google Motion Photo: single JPEG with embedded MP4; XMP parsed in `t_xmp.rs` (`quick-xml`);
  `content_id` stores `motion:<offset>:<length>`.
- HEIC-internal video (`live_photo_type=4`): detect/extract via libheif items/sequences with
  ffmpeg demux fallback on Windows/Linux (not macOS product target).
- DB columns on `afiles`: `content_id`, `paired_file_id`, `live_photo_type`
  (0=none, 1=Apple image, 2=Apple video, 3=Motion Photo, 4=HEIC-internal).
  Migration and open-time repair via `ensure_live_photo_columns`.
- Motion extract cache: `app_cache_dir()/motion_cache/` with source-keyed reuse, size-based
  prune, startup purge of legacy OS-temp extracts; cleared with `clear_video_cache`.
- Preview: MediaViewer 400ms long-press plays paired MOV or extracted motion video; LIVE badge
  on Thumbnail; FileInfo type labels; i18n en/zh.
- Export/convert (`export_live_photo` + `LivePhotoExportDialog`): still / video / pair /
  to_motion / to_pair / set_keyframe.
- Shared parser: `t_xmp::parse_motion_content_id` is the single source of truth for
  `motion:<offset>:<length>` (used by `t_cmds` and `t_live_photo`).
- **Polish (same day):**
  - Optional **confirmed** JPEG keyframe overwrite of the library still
    (`overwrite_original`; staged promote; Motion Photo keeps trailer; HEIC not supported).
  - Album-level `rescan_live_photo_metadata` repairs type `0`/`4` without full reindex, then
    re-pairs; AlbumList context menu + FileInfo export entry.
  - User guide: `docs/guide/live-photo.md`.

Runbook: `.mex/patterns/change-live-photo.md`.

### Reliability / consistency fixes

- `rename_file` / `rename_folder`: if disk rename succeeds and DB update fails, roll disk
  back to the old name (aligned with `move_file` rollback).
- `edit_album`: name-column errors propagate (no longer swallowed with `let _ =`).
- Dedup `get_files_by_sizes`: reuses precomputed suspicious sizes via chunked `IN` binds
  instead of a redundant full-table `GROUP BY`.
- MediaViewer: null-safe `props.file?.file_type` on floating toolbar; Live Photo playback
  guards when `props.file` is cleared mid long-press.
- `getBuildTime`: drop double semicolon; treat `0` with `!= null`.

### Verification (this pass)

- `cargo check --manifest-path src-tauri/Cargo.toml` passed.
- `pnpm --dir src-vite build` passed for the Live Photo polish UI pass.
- Full plugin-host regression not re-run in this pass; run
  `scripts/check_plugin_host.ps1` before release.

### Still open

- Broader HEIC sequence sample coverage; unusual sequence brands may fail ffmpeg demux.
- Sandbox **Phase 3–5 only** (Phase 0–2 done): network OS block, Linux Landlock/seccomp,
  env hygiene, optional cache ref/range zero-copy — `docs/ai-plugin-sandbox-roadmap.md`.
- Signing-key rotation/revocation design.
- Release-executable plugin regression after host/plugin changes.
- Manual SA-LUT/NAFNet staged-path checklist on release builds
  (`docs/ai-plugin-sandbox-phase0-verify.md`).

### 2026-07-17 shared→plugin-private confirmed switch

- Settings probe conflict block now offers **Use private runtime** when blocking
  conflicts exist and the profile still uses a non-private binding.
- User confirmation persists a synthetic `scope: "plugin"` binding via
  `switch_ai_plugin_profile_to_private_runtime`, clears that profile's probe
  cache, and marks the profile `needsVerify` without touching shared runtimes.
- After the switch, the user still re-runs Setup → Probe → Smoke for the private
  env under `plugin-runtimes/<plugin-id>/<envDir>`.

### 2026-07-17 model UX reinforcement

- `list_ai_plugins` now includes `modelFiles` presence under the managed model
  directory (`plugin-data/<id>/models`).
- Settings storage panel shows declared model files and offers:
  - **Open & validate** → `check_ai_plugin_model_files` + reveal model dir
  - **Import model files** → `import_ai_plugin_model_files` copies selected
    files by basename into declared model paths (containment-checked)
- External model-dir binding rows also open+validate the bound directory.

### 2026-07-17 sandbox Phase 0 (design + small correctness fixes)

- Roadmap: `docs/ai-plugin-sandbox-roadmap.md` (phased; no Settings sandbox panel).
- Input staging default is **platform-agnostic** (was Windows-gated).
- Staging copy failures **fail closed** (no silent fallthrough to original paths).
- Diagnostics: task queue message + `plugin-cache/.../inputs/staging-report.json`
  with staged file/byte counts and skip counters.
- Unit tests cover rewrite, fail-closed, and disabled messaging.
- Manual SA-LUT/NAFNet checklist: `docs/ai-plugin-sandbox-phase0-verify.md`.
- Network/Linux OS sandbox remain future opt-in research spikes.

### 2026-07-17 sandbox Phase 1 (host write allow-list)

- Single helper `plugin_writable_roots`: data/cache/outputs/plugin-runtimes/code
  + manifest shared runtimes + persisted model-dir bindings + call-site extras
  (task dir / task output).
- Used by invoke-time staging skip list and start-time optional deny-ACL
  exclusions (no Settings UI; no OS allow-list enforcement beyond existing ACL opt-in).
- Output adoption remains stricter: paths must stay under the **task output**
  directory only.

### 2026-07-17 sandbox Phase 2 (same-volume hardlink staging) — **done**

- Phase 2 **mainline complete**: `stage_one_file` tries hardlink first, then copy.
- Not full universal zero-copy: cross-volume still copies; cache ref/range not implemented.
- Staging report + task message include `hardlinkedFiles` / `copiedFiles`.
- Unit tests cover hardlink path on same temp volume; fail-closed still enforced.
- **Next sandbox work is Phase 3/4 research only** (do not ship as default).


## 2026-07-10 v1.0.0 stabilization pass

- Completed the active Lap → PicAiPic migration in UI text, updater/repository
  links, backup naming, dependency dialogs, help labels, CI artifact names,
  Chinese documentation, and VitePress configuration.
- Fixed cross-library thumbnail and preview isolation. Protocol URLs now select
  the encoded library's validated database and cache rather than relying on
  whichever library is current when an asynchronous request finishes.
- Enforced plugin host compatibility ranges (`minPicAiPicVersion`, optional
  `maxPicAiPicVersion`) alongside the v1 plugin API major gate.
- Standardized JavaScript tooling on pnpm, removed npm lockfiles, and aligned
  Cargo/Tauri/frontend/docs metadata at `1.0.0`.
- Split Home's heavy panels and Content into async chunks. The Home entry
  chunk dropped from about 527 KB to about 15 KB.
- Added `docs/guide/release-notes/v1.0.0.md` and moved the website's current
  release links to v1.0.0/current GitHub repository paths.

Verification: frontend production build, Rust format/check, seven non-ignored
Rust tests, `scripts/check_plugin_host.ps1`, and strict packaging for both
reference plugins all passed.

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
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm --dir src-vite build
cargo test --manifest-path src-tauri/Cargo.toml -- --skip real_signed_zips_verify
.\scripts\check_plugin_host.ps1
.\scripts\package_plugin.ps1 -All -FailOnWarnings
python -m json.tool plugins\picai-salut-color\picaipic.plugin.json
python -m py_compile plugins\picai-salut-color\backend\main.py plugins\picai-salut-color\backend\salut_adapter.py
python scripts\stress_salut_async.py --tasks 8 --duration-ms 300 --cancel-every 3
python scripts\stress_salut_http.py --tasks 6 --duration-ms 250 --cancel-every 3
python -m json.tool plugins\picai-nafnet-restore\picaipic.plugin.json
python -m py_compile plugins\picai-nafnet-restore\backend\main.py plugins\picai-nafnet-restore\backend\nafnet_adapter.py plugins\picai-nafnet-restore\backend\denoiser.py
python scripts\stress_nafnet_http.py --tasks 4 --duration-ms 120 --cancel-every 2
```

## 2026-07-08 Plugin-level external model directory binding

- **Manifest-declared `modelBindings[]`**: new top-level manifest field. Each
  binding declares an `envVar` (e.g. `SALUT_MODEL_DIR`, `NAFNET_SOURCE_DIR`),
  optional extra `envVars`, a `layout` (`"files"` or `"sourceTree"`), and
  `expectedFiles`/`expectedGlobs` for validation. The host reads the manifest
  and injects the user-selected directory as the declared env var into the
  plugin process — no host-side hardcoded plugin-id→envVar mapping. New
  plugins add `modelBindings[]` to their manifest with zero backend changes.
- **`AiPluginProfileState.model_dir_bindings`**: per-profile persisted binding
  map (key = binding id, value = directory absolute path). `#[serde(default)]`
  keeps old registries forward-compatible. Setup/smoke flows that reconstruct
  profile state preserve existing bindings via `persisted_model_dir_bindings`.
- **`build_setup_environment` injection**: after the default
  `PICAIPIC_PLUGIN_MODEL_DIR`, the host injects each binding's `envVar` (and
  extra `envVars`) from the persisted map. Bindings without a persisted
  directory are skipped so the plugin falls back to its default resolution.
  This mirrors the runtime-binding precedence — `.local.env` still wins for
  developers because `start.bat`'s `for /f` loop runs after host injection.
- **Three Tauri commands**: `set_ai_plugin_model_dir_binding` (validate dir,
  persist, return check result), `clear_ai_plugin_model_dir_binding`,
  `check_ai_plugin_model_bindings` (validate without persisting). All
  registered in `main.rs`.
- **`list_ai_plugins` summary**: `AiPluginSummary.model_bindings` carries the
  manifest declarations; each `PluginInstallProfileSummary.modelBindingChecks`
  carries the live validation (present/missing files, `ok` flag) for that
  profile's persisted bindings.
- **Settings UI**: each profile row shows a model-binding card when the plugin
  declares `modelBindings`. Each binding shows a status chip (ready/missing/
  not-bound), the bound directory path, and Bind/Change/Open/Clear buttons.
  Directory picker uses the established `openDialog({ directory: true })`
  idiom; Open reuses `revealPath`.
- **SA-LUT manifest** declares `salut-model-dir` (`envVar: SALUT_MODEL_DIR`,
  `expectedFiles: [vgg_normalised.pth, epoch=100-step=4127466.ckpt.state.pt]`).
  **NAFNet manifest** declares `nafnet-source-dir` (`envVar: NAFNET_SOURCE_DIR`,
  `layout: sourceTree`, `expectedGlobs: [experiments/pretrained_models/*.pth]`).
- Validation: `cargo fmt --check`, `cargo check` (zero warnings), `pnpm build`,
  `python -m json.tool` (both manifests), `python -m py_compile` (both
  backends) all pass.

## 2026-07-07 Project rename, signing hardening, release build

- **Signature canonicalization fix**: the Ed25519 package signature was
  fragile — Python signed with unsorted JSON keys, Rust verified with
  struct field order. Both sides now use lexicographic key ordering
  (Python `sort_keys=True`, Rust `serde_json::Value` with BTreeMap), so
  the signature is field-order independent. Also fixed `Option::None`
  serialization mismatch (`skip_serializing_if` on `signature` and
  `created_at` fields). Unit tests cover cross-language consistency,
  key-order independence, and tamper rejection. See `t_plugin.rs` tests.
- **Project identity renamed from Lap to PicAiPic**: `productName`,
  `identifier` (`com.julyx10.lap` → `com.big2cater.picaipic`), Cargo.toml,
  window title, fallback URLs, and all user-facing docs updated. This
  changes the app data directory (`%LOCALAPPDATA%\com.big2cater.picaipic`),
  so prior Lap-era local data is not visible to the new identity.
- **Updater signing key rotated**: the old updater pubkey in
  `tauri.conf.json` belonged to upstream julyx10; the matching private key
  was never available to this fork. Generated a new minisign keypair;
  public key is in `tauri.conf.json`, private key is gitignored locally.
  Updater endpoint moved from `julyx10/lap` to `big2cater/picaipic`.
- **macOS support removed**: AI plugins are incompatible with macOS
  (plugin confinement is Windows-oriented; no macOS Seatbelt). Deleted
  `tauri.macos.conf.json`, `infoplist/`, homebrew workflow, and macOS
  matrix entries from release/pr-build workflows. Rust `cfg(macos)`
  branches kept intact (harmless, preserves structure). Platform scope is
  now Windows + Linux only.
- **Languages trimmed to en + zh**: dropped 7 locales (de/es/fr/ja/ko/pt/ru)
  and their i18n READMEs. Frontend bundle reduced ~40%.
- **Release build verified**: `package_windows.ps1` produces
  `PicAiPic.exe`, NSIS installer, and `.sig` updater signature. The
  script auto-loads the updater key from `picaipic-updater-key.key`.
- **End-to-end trust flow validated**: installing a signed plugin zip
  triggers the `TRUST_REQUIRED` consent dialog, user confirms, publisher
  is written to `plugin-registry.json`, and install completes. Verified
  with the real salut-color plugin package.
- **Two plugin packages signed**: both `picai-salut-color` and
  `picai-nafnet-restore` zips are signed with the release key
  (publisher `local`, pubkey `e7Ccs...pe8=`).
- **Dev server IPv4 fix**: Vite v8 binds IPv6 by default; Tauri devUrl
  resolves to IPv4. Set `server.host = '127.0.0.1'` in `vite.config.js`.

## 2026-07-10 Plugin sandbox policy update

- **Default confinement changed to input staging only**: external image inputs
  are copied into `plugin-cache/<id>/tasks/<taskId>/inputs/` before invoke and
  payload `path` values are rewritten. Plugins read staged copies instead of
  raw source-image paths.
- **Windows deny-ACL write confinement is now opt-in**:
  `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` enables the old
  `icacls /deny <user>:(W) /L` path. It is no longer default because it mutates real user
  directory ACLs while plugins run and can trigger confusing host UI access
  prompts.
- **Development escape hatch preserved**:
  `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` skips both input staging and optional
  ACL handling.
- **Stale ACL cleanup added**: default plugin startup best-effort removes
  old deny ACEs left by previous builds or crashed runs, then continues
  without re-applying them unless opt-in ACL mode is set.
- **Verification**: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  and `cargo check --manifest-path src-tauri/Cargo.toml` passed. Local
  `cargo build --release` still fails at MSVC/CRT link time in existing
  `libort_sys`/`LibRaw` dependencies, unrelated to the sandbox code.

## Next Work

- ~~Write a `latest.json` generation script and create the first GitHub
  release (v0.2.4) with the NSIS installer + `.sig` + `latest.json`, so
  the in-app auto-updater has a real endpoint to check.~~ **Completed
  (2026-07-08):** first release `v1.0.0` (Draft) built end-to-end via
  `release.yml` + `release-windows.yml`. `latest.json` carries all four
  platforms (linux-x86_64, linux-aarch64, windows-x86_64, windows-aarch64)
  with valid signatures. Three CI build blockers were fixed along the way:
  `beforeBuildCommand` hardcoded a Windows absolute path (broke Linux CI),
  `third_party/` submodules were blocked by `.gitignore` so gitlinks never
  landed in commits (broke Rust `build.rs`), and `t_sandbox.rs` icacls
  calls lacked `#[cfg(target_os = "windows")]` guards (broke Linux
  compilation). The release stays as Draft until feature completeness.
- ~~Migrate AI model / ffmpeg binary downloads from `julyx10/lap-binaries`
  to a `big2cater/picaipic-binaries` release, so the fork does not depend
  on the upstream binary repo.~~ **Completed (2026-07-08):** ten binaries
  (8 ffmpeg/ffprobe sidecars for Windows x64/arm64 + Linux x86_64/aarch64,
  plus `text_model.onnx` and `tokenizer.json`) re-uploaded to
  `big2cater/picaipic-binaries` under `ffmpeg-8.1` and `models` release
  tags. `t_ai.rs` and `download_ffmpeg_sidecar.{ps1,sh}` now point at the
  new repo. The `picaipic-binaries` repo was set to public so anonymous
  release-asset downloads work at runtime.
- Design plugin signing key rotation (security-hardening open question 3):
  if an author's private key is compromised, there is currently no
  revocation/rotation path in the trust store.
- Migrate prior Lap-era local data (`com.julyx10.lap.debug` directory)
  to the new `com.big2cater.picaipic.debug` path, so existing dev-time
  plugin installs and config carry over.
- ~~Model import / external model directory binding support, so users with
  model files already on disk do not need to hand-edit `.local.env`.~~
  **Completed (2026-07-08):** plugin-level external model directory binding
  landed. Manifest `modelBindings[]` declares the env var + expected files;
  Settings UI lets users pick a directory; host injects it into the plugin
  process and validates file presence. Optional **bulk file import into the
  plugin model directory** remains a UX nice-to-have, not a blocker.
- Avoid concrete `export-lut` business logic until runtime binding
  confidence is stable across both SA-LUT and NAFNet.
- Add user-confirmed one-click fallback from a conflicting shared runtime to a
  plugin-private runtime (detect + text advice + manual private selection already work).
- Design publisher signing-key rotation/revocation and continue release-exe
  plugin regression after host/package changes.
- Strengthen network confinement and Linux process isolation without breaking
  GPU/runtime compatibility (beyond default input staging / opt-in Windows ACL).
