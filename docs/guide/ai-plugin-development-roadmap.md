# PicAiPic AI Plugin Development Roadmap

Updated: 2026-07-10

Current v1 status: local HTTP discovery/lifecycle/tasks, signed packages and
publisher trust, shared/plugin/external runtime bindings, conflict detection,
uninstall modes, input staging, and manifest-driven external model directory
bindings are implemented. Remaining work is primarily release regression,
user-confirmed private-runtime fallback, signing-key rotation/revocation, and
stronger network/Linux isolation.

This document describes how to build PicAiPic's AI plugin system and how to
package upstream open-source AI projects as independent PicAiPic plugins.

The key rule is simple: PicAiPic is the host app, not a model bundle. Each
upstream project is wrapped as its own plugin.

## Target Architecture

```text
PicAiPic core
  - album and folder management
  - database and metadata
  - thumbnail and preview pipeline
  - lightweight editing
  - plugin discovery and task orchestration

PicAiPic plugin package
  - one upstream open-source project or model family
  - runtime requirements, model weights, logs, and status checks
  - may bind to an external, shared, or plugin-private runtime
  - standard PicAiPic manifest and invoke API
```

Example plugin packages:

```text
picai-salut-color       wraps D:\ailab\SA-LUT-main
picai-nafnet-restore    wraps D:\ailab\NAFNet
picai-iopaint-inpaint   wraps D:\ailab\IOPaint-main
picai-mobile-sam        wraps MobileSAM
picai-iat-exposure      wraps Illumination-Adaptive Transformer
picai-gpupixel-filter   wraps GPUPixel
```

`D:\ailab\20260610133133` can be used as reference code, but it should not be
treated as the plugin to ship. Its functions should be split and repackaged by
upstream project.

## Zip Plugin Storage Model

Zip plugin installs use a split storage layout. The package directory contains
replaceable plugin code only; persistent runtime and user data live under a
PicAiPic plugin store directory next to the installed executable by default.
This keeps large models and virtual environments out of the user's system
profile drive.

```text
<picai-install-dir>\picaipic-local\
  plugins\{plugin-id}\                  # installed zip code, replaceable
  plugin-data\{plugin-id}\              # persistent plugin-owned data
    config\plugin.local.json
    logs\
    models\
  plugin-runtimes\{plugin-id}\{envDir}\ # plugin-private venv/runtime
  shared-runtimes\{runtime-id}\         # shared heavy runtimes
  plugin-cache\{plugin-id}\             # disposable task cache
  plugin-outputs\{plugin-id}\           # durable generated outputs
```

Rules:

- Reinstalling or upgrading a zip plugin may replace only `plugins\{plugin-id}`.
- Models, local config, logs, and plugin-private runtimes must survive package
  reinstall.
- Setup commands install into the selected managed runtime path, not into the
  package code directory. Shared bindings use `shared-runtimes\{runtime-id}`;
  plugin-private bindings use `plugin-runtimes\{plugin-id}\{envDir}`.
- Heavy runtimes such as ROCm/CUDA/CPU PyTorch and DirectML should use
  `scope: shared` runtime bindings when compatible, so multiple plugins can
  reuse one managed runtime. Use plugin-private runtimes only when requirements
  conflict.
- Runtime conflict handling is now host-checked (implemented 2026-07-03). The
  host compares probe-reported package versions against the plugin's
  requirements specifiers; `version_mismatch` and `missing` conflicts
  hard-block capability invocation and the UI advises switching to a
  plugin-private runtime. Auto-switching the profile (with user confirmation)
  is still future work.
- Model files live under `plugin-data\{plugin-id}\models` unless the user
  explicitly binds an external model directory later.
- Uninstall offers a choice (implemented 2026-07-03): "code only" removes the
  installed code and registry state; "code + data & runtimes" additionally
  removes `plugin-data`, `plugin-cache`, `plugin-outputs`, and
  `plugin-runtimes` for the plugin id. `shared-runtimes` is never deleted.
- Settings > Plugins exposes "Plugin storage location" so users can point ZIP
  plugin data and runtimes at a large data drive. This changes the future store
  root only; existing plugins/runtimes are not moved automatically.
- Advanced users can still override the store root with
  `PICAIPIC_PLUGIN_STORE_DIR`, which wins over the saved Settings value.

The host injects these stable paths when it runs setup/start commands:

```text
PICAIPIC_PLUGIN_ROOT          # installed plugin code directory
PICAIPIC_PLUGIN_DATA_DIR      # persistent data root
PICAIPIC_PLUGIN_CACHE_DIR     # disposable cache root
PICAIPIC_PLUGIN_LOG_DIR       # persistent logs directory
PICAIPIC_PLUGIN_MODEL_DIR     # persistent model root
PICAIPIC_PLUGIN_CONFIG_PATH   # persistent local config file
PICAIPIC_PLUGIN_RUNTIME_DIR   # selected profile runtime root
PICAIPIC_PLUGIN_ENV_DIR       # selected absolute env path
PICAIPIC_PLUGIN_ENV_PATH      # same absolute env path for compatibility
```

Plugin code should prefer these variables over package-relative `models`,
`.venv`, or `logs` folders. Package-relative folders remain acceptable for local
development fixtures, but they are not the release storage contract.

Current runtime sharing rule:

- Shared runtime: use for large compatible stacks such as Python 3.12 +
  PyTorch ROCm 7.2 / CUDA 12.1 / CPU / DirectML.
- Plugin-private runtime: use when the plugin needs conflicting package
  versions, special native libraries, custom compiled extensions, or an older
  Python/numpy/OpenCV stack.
- External runtime: use for local development or advanced users pointing at an
  already-installed Python environment.
- Future work: add runtime package auditing and UI-assisted conflict detection
  so normal users do not need to understand these scopes.

## Current Status Note (2026-07-02)

The host/plugin interface is now beyond a documentation-only scaffold. The current focus is no longer to invent a new plugin architecture, but to harden the existing local-HTTP plugin loop and make it robust in packaged release builds.

Verified locally on this machine:

- `cargo check --manifest-path src-tauri\Cargo.toml` passed.
- `pnpm --dir src-vite build` passed.

The immediate next priorities are:

1. release-exe UI regression testing for Start / Restart / Smoke
2. ~~runtime dependency-conflict guidance for shared vs plugin-private runtimes~~ (done 2026-07-03)
3. ~~uninstall UX for "remove code only" vs "remove code + data + runtime"~~ (done 2026-07-03)
4. model import and external model directory binding

## Development Phases

### Phase 1: Host Plugin Skeleton

Goal: PicAiPic can discover plugins and explain whether they are usable.

Core work:

- Add a Rust plugin module, for example `src-tauri/src/t_plugin.rs`.
- Read plugin manifests from registered paths.
- Validate manifest structure and platform compatibility.
- Add Tauri commands:
  - `list_ai_plugins`
  - `validate_ai_plugin_manifest`
  - `get_ai_plugin_status`
- Store plugin registry entries in app config or a plugin config file.
- Add a settings panel that lists plugins, capabilities, status, and errors.

No AI inference is required in this phase.

### Phase 2: Local HTTP Task Runner

Goal: PicAiPic can start a plugin, call one capability, and receive output
files.

Core work:

- Start `local-http` plugins with `startCommand`.
- Allocate and pass `PICAIPIC_PLUGIN_PORT`.
- Poll `/health` and `/status`.
- Send normalized task JSON to `/invoke/{capability}`.
- Support cancellation and progress polling.
- Store task temp files in an app-managed task directory.
- Import successful outputs through the existing PicAiPic import/register flow.

The plugin declares its AI-specific dependency needs and owns model loading.
The actual Python/native runtime may be external, shared, or plugin-private,
and must be proven by Smoke before the profile is considered usable.

### Phase 3: First Real Plugin - SA-LUT

Goal: Package `D:\ailab\SA-LUT-main` as `picai-salut-color`.

Recommended plugin folder:

```text
D:\ailab\PicAiPic-plugins\picai-salut-color\
  picaipic.plugin.json
  README.md
  install.bat
  start.bat
  backend\
    main.py
    requirements.txt
    adapter\
      salut_adapter.py
```

Plugin API:

```text
GET  /health
GET  /status
POST /invoke/color-transfer
POST /invoke/export-lut
POST /tasks/{taskId}/cancel
```

Capabilities:

```text
image.color.transfer
image.color.lut.export
```

Inputs:

```text
source image
reference/style image
optional parameters: intensity, saturation, contrast, tone preservation
```

Outputs:

```text
processed image
optional .cube LUT
sidecar JSON recipe/debug info
```

PicAiPic should not know SA-LUT internals. It only knows the manifest,
capabilities, status, and task response.

### Phase 4: Plugin UI Integration

Goal: Users can run the SA-LUT plugin from PicAiPic.

Core work:

- Add an AI tools entry point in the image viewer or editor.
- Render controls from the plugin parameter schema.
- Let users choose source/reference images.
- Run the task and show progress.
- Preview plugin output.
- Import/save result into the current album.
- Show diagnostics if the plugin fails.

The first UI can be plain and narrow. Avoid building a broad plugin marketplace
before one plugin works end to end.

### Phase 5: Additional Plugins

After SA-LUT works end to end, package the next upstream projects one at a time:

```text
picai-nafnet-restore    denoise / deblur / JPEG artifact removal
picai-iopaint-inpaint   mask-based inpainting
picai-mobile-sam        segmentation masks
picai-iat-exposure      exposure calibration
picai-gpupixel-filter   beauty/filter pipeline
```

Each plugin should prove:

- manifest validation
- install script
- status and environment detection
- device fallback
- one successful task
- output import
- diagnostics on failure

## Why SA-LUT First

SA-LUT is a good first plugin because its boundary is clean:

- two image inputs
- one image output
- optional LUT output
- model dependency is heavy enough to justify plugin packaging
- user workflow is easy to see in PicAiPic

NAFNet is also a strong candidate, but SA-LUT exercises multi-input tasks and
LUT sidecar output, which helps validate the plugin API early.

## What Not To Do First

- Do not move large model code into PicAiPic core.
- Do not directly embed `D:\ailab\20260610133133` as one plugin.
- Do not give plugin web pages Tauri permissions.
- Do not write plugin outputs directly into the database.
- Do not build a marketplace before local plugin discovery works.
- Do not implement every capability kind before one plugin works end to end.
- Do not put plugin-private venvs, downloaded models, local config, or logs
  inside the installed zip code directory.

## First Concrete Milestone

The first milestone is:

```text
PicAiPic discovers picai-salut-color, validates its manifest, starts its local
service, shows status, runs color transfer on two selected images, and imports
the output image into the current album.
```

That milestone proves the whole architecture without forcing all AI features to
be solved at once.

## Next Development Priorities

1. Run a release executable UI pass for the new shared runtime setup flow:
   install zip, grant setup downloads, run setup, Probe, Smoke, uninstall, and
   reinstall. Include shutdown cleanup, Smoke progress feedback, privacy
   authorization copy, and shared-runtime badge color in this pass. Also
   verify the new uninstall mode dialog (code-only vs code-and-data) and the
   runtime conflict warning block in the probe card.
2. ~~Add runtime scope/path visibility in Settings so users can see whether a
   profile is using shared, plugin-private, or external Python, and where that
   environment lives on disk.~~ (done 2026-07-02)
3. ~~Add runtime package auditing for key dependencies (`python`, `torch`,
   `torchvision`, `numpy`, `opencv-python`, `rawpy`, etc.) and surface version
   conflicts.~~ (done 2026-07-03: conflict detection compares probe versions
   against requirements specifiers and blocks invocation on mismatch)
4. Add a UI escape hatch to use a plugin-private runtime when a shared runtime
   is incompatible. (partially done: the host detects conflicts and advises;
   one-click auto-switch to plugin-private is still future work)
5. ~~Add a Settings UI for model import/external model directory binding.~~
   (done 2026-07-08: manifest `modelBindings[]`, validation, persistence,
   Settings controls, and host environment injection)
6. ~~Add uninstall choices for code-only vs code + data + runtime.~~ (done
   2026-07-03)
7. Keep package validation strict, but show install-time network warnings as
   review choices instead of hard blocks.
8. Run a release executable UI pass after every packaging/runtime change:
   install zip, setup selected profile, start, smoke test, uninstall, reinstall.
