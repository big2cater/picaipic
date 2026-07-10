# PicAiPic AI Plugin Author Checklist

Date: 2026-06-30

Use this checklist before adding or releasing a PicAiPic AI plugin against the v1 local HTTP contract.

## Host interface status

The PicAiPic host interface is considered complete enough for v1 local HTTP plugins when the plugin fits this shape:

- plugin directory with `picaipic.plugin.json`
- local HTTP backend started by `entry.startCommand`
- `/health`, `/status`, `/invoke/{capabilityId}`, `/tasks/{taskId}`, optional `/tasks/{taskId}/events`, and `/tasks/{taskId}/cancel`
- runtime profile validated by Probe/Setup/Smoke
- outputs written under the host-provided `outputDir`
- host-owned import/adopt/discard

Do not request new host interface fields for algorithm quality or plugin implementation convenience. New host fields should be justified by a boundary problem that cannot be handled by the existing v1 contract.

Plugin buttons and menu actions should be declared through `contributes.menus[]`. Do not require PicAiPic body code to add a button for a specific plugin id. If a plugin needs a UI location that does not exist yet, propose a reusable placement name rather than a plugin-specific hard-coded button.

## 1. Directory and manifest

- [ ] Plugin root contains `picaipic.plugin.json`.
- [ ] Manifest JSON parses successfully.
- [ ] `schemaVersion`, `id`, `name`, `version`, `compatibility.pluginApi`, `entry.kind`, and `capabilities[]` are present.
- [ ] `entry.kind` is `local-http` for a local backend plugin.
- [ ] `entry.startCommand` is a safe relative path inside the plugin directory.
- [ ] `entry.stopCommand`, when present, is safe, idempotent, and returns quickly.
- [ ] `entry.defaultPort` is treated only as preferred; backend honors `PICAIPIC_PLUGIN_PORT`.
- [ ] Capabilities have stable `id`, `kind`, `inputs`, `outputs`, `parameters`, and `invoke` metadata.
- [ ] User-facing plugin buttons are declared in `contributes.menus[]`, not hard-coded in the PicAiPic body.
- [ ] Menu entries use existing placements when possible, such as `image.contextMenu` and `image.toolbar`.

## 2. Backend endpoints

- [ ] `GET /health` returns lightweight JSON readiness.
- [ ] `GET /status` returns useful runtime/model diagnostics.
- [ ] `POST /invoke/{capabilityId}` accepts host `taskId`, `inputs`, `parameters`, and `outputDir`.
- [ ] Long operations return async task state instead of blocking indefinitely.
- [ ] `GET /tasks/{taskId}` reports `queued`, `running`, `cancelling`, `succeeded`, `failed`, or `cancelled`.
- [ ] `POST /tasks/{taskId}/cancel` returns quickly and records cancellation intent.
- [ ] Optional `GET /tasks/{taskId}/events` supports cursor-style progress updates.

## 3. Task and output behavior

- [ ] Backend writes all generated outputs under the supplied `outputDir`.
- [ ] Successful terminal task includes `outputs[]` descriptors.
- [ ] Output descriptors include at least `id`, `kind`, `path`, and `mime` when applicable.
- [ ] Backend never asks PicAiPic to import/adopt paths outside `outputDir`.
- [ ] Progress and messages are updated at meaningful checkpoints.
- [ ] Slow model operations check cancellation at safe checkpoints where possible.

## 4. Errors

- [ ] Failures return structured error metadata where possible.
- [ ] Error has `code`, `domain`, `message`, and optional `details`.
- [ ] Use one of the v1 domains: `transport`, `plugin`, `runtime`, `device_backend`, `filesystem`, `task`, `host`.
- [ ] Dependency/model/device failures are distinguishable from invalid user input.
- [ ] Cancellation uses task-domain metadata such as `TASK_CANCELLED` when applicable.

## 5. Runtime/setup/smoke

- [ ] `installProfiles[]` describe realistic runtime choices.
- [ ] External runtime paths are intentional and documented when used.
- [ ] Smoke test uses representative minimum image/input sizes, not unrealistically tiny fixtures.
- [ ] Smoke test fails with clear JSON/details when runtime or model files are missing.
- [ ] Setup/probe/smoke can be run from Settings without manual terminal steps.

## 5.1 Windows batch files (.bat)

Windows plugin scripts (`start.bat`, `stop.bat`, `install.bat`) are executed
by `cmd.exe`. Two discipline rules prevent the "Smoke stuck on
`error sending request for url (http://127.0.0.1:PORT/health)`" failure mode
seen on 2026-07-01:

- [ ] **All `.bat` files use CRLF line endings.** `cmd.exe` rejects some
  LF-only batch files with `The syntax of the command is incorrect.`
  Non-Windows editors, copy/paste, and some linters silently save LF; verify
  before packaging. If unsure, run `file start.bat` and confirm it reports
  `with CRLF line terminators`.
- [ ] **Do not nest `%%A:~0,1%` substring `if` inside a `for /f` loop.** The
  pattern
  `for /f "usebackq tokens=1,* delims==" %%A in (...) do (if not "%%A"=="" if not "%%A:~0,1%"=="#" set "%%A=%%B")`
  is rejected by some `cmd.exe` builds. To skip comment lines in an env file,
  use the `eol=#` option on the `for /f` itself:
  `for /f "usebackq eol=# tokens=1,* delims==" %%A in (".local.env") do (if not "%%A"=="" set "%%A=%%B")`.
- [ ] `.local.env` is the supported mechanism for pointing the plugin at an
  already-installed external Python/PyTorch venv on this machine. It is
  excluded from release packages by `package_plugin.ps1`. Machine-specific
  absolute paths belong only in `.local.env`, never in `picaipic.plugin.json`
  or committed scripts.

## 6. Host regression pass

Before treating a plugin as compatible with v1, verify:

- [ ] `.\scripts\check_plugin_host.ps1` passes.
- [ ] `.\scripts\check_plugin_host.ps1 -IncludeStress` passes when the plugin participates in local HTTP async task behavior.

- [ ] Plugin appears in Settings > AI Plugins.
- [ ] Start works.
- [ ] Stop works.
- [ ] Stop changes the UI out of Running for the host-managed runtime.
- [ ] If the default port is occupied by a stale/unmanaged backend, Start can allocate a fresh managed port.
- [ ] Restart works.
- [ ] Refresh updates visible state.
- [ ] Probe Runtime works for the intended profile.
- [ ] Run Setup either succeeds or reports actionable failure.
- [ ] Smoke succeeds with representative input.
- [ ] Real image invocation opens `PluginActionDialog`.
- [ ] Dialog progress/stage updates during queued/running/importing/cancelling states.
- [ ] Cancel is available for long async tasks.
- [ ] Successful output can be adopt/import/discarded.
- [ ] Failure is shown as a bounded task failure, not a PicAiPic crash/hang.

## 7. Stop/lifecycle hardening

- [ ] `stopCommand` can be run multiple times safely.
- [ ] `stopCommand` attempts to clean stale backend processes belonging to this plugin.
- [ ] `stopCommand` honors `PICAIPIC_PLUGIN_PORT` when cleaning by port.
- [ ] The backend does not assume the manifest `defaultPort` is the only possible runtime port.
- [ ] A stale process on the default port does not prevent a new managed runtime from starting on another port.
- [ ] Plugin authors understand that host UI Running state is based on host-managed runtime state, not raw localhost reachability.

## 8. Security: signing and sandbox

PicAiPic enforces a three-layer security model (startup token, package
signing, process sandbox). Plugin authors should be aware of the latter two
when preparing a release.

### Package signing (release requirement)

- [ ] Release packages are signed with an Ed25519 keypair. Use
  `python scripts/sign_plugin.py generate-key` to create a keypair, then
  `.\scripts\package_plugin.ps1 <plugin> -SignKeyFile <key.txt>` to sign at
  packaging time.
- [ ] The `publisher` field in `picaipic.plugin.json` identifies the author.
  Users must trust the publisher (by public key) before the first install of
  a signed package; the host shows a consent dialog with the publisher name
  and public-key fingerprint.
- [ ] Unsigned packages are refused in release builds. They are allowed only
  in developer mode (`PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1`), for local
  iteration.
- [ ] Keep the private key out of the package and out of source control.

### Process confinement (transparent to plugin code)

On Windows the host confines task input access at runtime. This requires no
plugin-side changes, but authors should understand the constraints:

- [ ] When a task is invoked with input files that live outside the
  plugin's writable area (e.g. a user-selected source image), the host
  **copies** them into `plugin-cache/<id>/tasks/<taskId>/inputs/` and
  rewrites the `path` fields in the `inputs` payload to point at the staged
  copies. Read from the `path` values in the payload — do not assume the
  original on-disk location is reachable.
- [ ] Write outputs only to the host-provided directories exposed via
  `PICAIPIC_OUTPUT_DIR`, `PICAIPIC_TASK_TEMP_DIR`,
  `PICAIPIC_PLUGIN_DATA_DIR`, `PICAIPIC_PLUGIN_CACHE_DIR`, and
  `PICAIPIC_PLUGIN_MODEL_DIR`.
- [ ] GPU/CPU access is fully preserved — input staging does not break
  ROCm/CUDA/DirectML inference.
- [ ] `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` skips input staging and any
  optional ACL sandboxing (development escape hatch). Do not rely on this in
  release.
- [ ] The experimental Windows deny-ACL write-confinement path is opt-in via
  `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1`; plugin authors should not require
  it for correctness.

## 9. Current reference plugins

Validated reference paths:

- `D:\ailab\PicAiPic\plugins\picai-salut-color`
- `D:\ailab\PicAiPic\plugins\picai-nafnet-restore`

SA-LUT is the cleaner real image workflow sample. NAFNet is the heavyweight runtime stress-test sample; its model quality/performance quirks are plugin-owned and should not drive host contract changes unless they expose lifecycle, task, output, or UI boundary problems.
