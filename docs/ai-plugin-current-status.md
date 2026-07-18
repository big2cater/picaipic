# PicAiPic AI Plugin Host - Current Status

Date: 2026-07-10

## 2026-07-10 v1.0.0 consistency, isolation, and performance pass

- **Active PicAiPic identity cleanup completed**: the main title bar, updater
  release-note URL, backup filename, VC++ dependency dialog, help menu, map
  fallback labels, AI download user agent, PR artifact names, Chinese README,
  and VitePress site configuration now use PicAiPic/current repository paths.
  Compatibility-sensitive internal `lap_*` cache/ABI identifiers and dated
  historical reports remain unchanged deliberately.
- **Cross-library thumbnail/preview isolation fixed**: `thumb://` and
  `preview://` now resolve the library id encoded in the URL, open that
  library's SQLite database through the shared connection pool, validate the
  id against configured libraries, and write generated thumbnail data to the
  matching database/cache. Delayed WebView requests can no longer resolve the
  same numeric file id in a newly selected library.
- **Host version compatibility enforced**: manifest validation now checks
  `minPicAiPicVersion`, optional `maxPicAiPicVersion`, and `pluginApi` major.
  Invalid version strings and incompatible hosts produce explicit validation
  errors. A Rust regression test covers version comparison.
- **Tooling/version metadata normalized**: Cargo, Tauri, frontend, and docs are
  aligned at `1.0.0`; pnpm is the sole JavaScript package manager and obsolete
  npm lockfiles were removed.
- **Home bundle split**: sidebar panels, Content, map, and library management
  are async components. The Home entry chunk dropped from about 527 KB to
  about 15 KB and the Vite chunk-size warning disappeared.

Verification completed:

```powershell
pnpm --dir src-vite build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml -- --skip real_signed_zips_verify
.\scripts\check_plugin_host.ps1
.\scripts\package_plugin.ps1 -All -FailOnWarnings
```

All checks passed: seven non-ignored Rust tests, both plugin manifests/backends,
frontend production build, Rust format/check, and strict packaging for SA-LUT
and NAFNet.

## 2026-07-08 First release pipeline + binary repo migration

- **First release `v1.0.0` (Draft) built end-to-end**: `release.yml` (Linux
  x86_64/aarch64) + `release-windows.yml` (Windows x64/arm64) both green.
  `latest.json` carries all four platforms with valid minisign signatures.
  Three cross-platform build blockers fixed: `beforeBuildCommand` hardcoded
  a Windows path, `third_party/` submodules were blocked by `.gitignore`
  so gitlinks never landed in commits, and `t_sandbox.rs` icacls calls
  lacked `#[cfg(target_os = "windows")]` guards. The release stays as Draft
  until feature completeness — see `docs/guide/picaipic-progress.md`.
- **Binary downloads migrated** from `julyx10/lap-binaries` to
  `big2cater/picaipic-binaries` (public). Ten assets re-uploaded under
  `ffmpeg-8.1` and `models` tags. `t_ai.rs` and
  `download_ffmpeg_sidecar.{ps1,sh}` now point at the new repo. The fork
  no longer depends on the upstream binary repository.

## 2026-07-07 Signature hardening, project rename, release build, trust flow validation

### Signature canonicalization fix (critical)

The Ed25519 package signature was fragile: Python signed with unsorted JSON
keys, Rust verified with struct field declaration order. Both happened to
match only because PowerShell's `[ordered]@{}` output order coincidentally
aligned with the Rust struct field order. Any field reorder would have
silently broken all signatures.

- Python `sign_plugin.py`: `json.dumps` now uses `sort_keys=True`.
- Rust `t_plugin.rs`: `verify_package_signature` serializes via
  `serde_json::Value` (default `Map` is `BTreeMap` → lexicographic key
  order) instead of `serde_json::to_vec(&struct)`.
- `AiPluginPackageManifest`: `signature` and `created_at` fields gained
  `skip_serializing_if = "Option::is_none"` so `None` omits the key
  (matching Python's `data.pop("signature")`), not `"signature":null`.
- Fixed `sign_plugin.py generate-key` bug: `PublicFormat.Raw` was passed
  where `PrivateFormat.Raw` was needed — the command could not run at all.

Tests added (`t_plugin::tests`):
- `signed_package_from_python_verifies` — cross-language byte-level check
- `canonical_serialization_sorts_keys` — regression guard
- `tampered_signature_is_rejected` — flipped bit must fail
- `real_signed_zips_verify` (`#[ignore]`) — reads real dist zips

### Project renamed Lap → PicAiPic

- `productName`, `identifier` (`com.julyx10.lap` → `com.big2cater.picaipic`),
  Cargo.toml, index.html title, SettingsAbout.vue, t_menu.rs / t_config.rs
  fallbacks all updated. App data dir is now
  `%LOCALAPPDATA%\com.big2cater.picaipic`.
- All user-facing docs (release notes, CONTRIBUTING, PRIVACY, getting-started,
  introduction, vitepress theme) and .github issue templates updated.
- Historical regression docs (dated 2026-06-30 etc.) kept as-is.

### Updater signing key rotated

The old updater pubkey in `tauri.conf.json` belonged to upstream julyx10;
the matching private key was never available to this fork, so signed
updates were impossible.

- Generated a new minisign keypair. Public key written to
  `tauri.conf.json` `plugins.updater.pubkey`; private key kept locally as
  `picaipic-updater-key.key` (gitignored).
- Updater endpoint moved from `julyx10/lap` to `big2cater/picaipic`.
- `package_windows.ps1` auto-loads the key from the repo root if
  `TAURI_SIGNING_PRIVATE_KEY` is not set in the environment.
- Removed `--no-sign` from build args (it skipped updater signing too).
- Removed `createUpdaterArtifacts = $false` override in `New-LocalTauriConfig`.

Verified: release build produces `PicAiPic.exe` (43.85 MB), NSIS installer
(183.77 MB), and `.sig` (420 bytes, minisign format).

### macOS support removed

AI plugins are incompatible with macOS (the confinement implementation is
Windows-oriented and there is no macOS Seatbelt integration). Removed:

- `src-tauri/tauri.macos.conf.json`
- `src-tauri/infoplist/` (11 `.lproj` dirs)
- `.github/workflows/release-homebrew.yml`
- macOS matrix entries and steps from `release.yml` and `pr-build.yml`
- macOS install sections from README, getting-started, introduction

Kept: Rust `#[cfg(target_os = "macos")]` branches (harmless, preserves
cross-compile structure). Platform scope is now Windows + Linux.

### Languages trimmed: 9 → 2

Dropped 7 locales (de/es/fr/ja/ko/pt/ru) and their i18n READMEs. Only `en`
and `zh` remain in `main.js`, `Settings.vue` language picker, and `i18n/`.
Frontend `index.js` bundle: 635 KB → 378 KB (−40%).

### End-to-end trust flow validated

Installed a signed `picai-salut-color` zip in dev mode:
1. `install_ai_plugin_package` → `verify_package_signature` → `NeedsTrust`
2. Frontend parsed `TRUST_REQUIRED:local:e7Ccs...:picai-salut-color`
3. Consent dialog showed publisher `local` + public key fingerprint
4. User clicked "Trust publisher" → `trust_publisher` wrote to
   `plugin-registry.json` `trustedPublishers.local`
5. Retry install → `Verified` → unpack + register → success

`plugin-registry.json` confirmed: `publicKey` matches the release signing
key exactly. Dev-mode bypass (`PICAIPIC_ALLOW_UNSIGNED_PLUGINS`) was **not**
set — the real signature verification path was exercised.

### Two plugin packages signed

Both `picai-salut-color-0.1.0.zip` and `picai-nafnet-restore-0.1.0.zip`
are signed with the release key (publisher `local`, pubkey
`e7CcsIlYh0E2PiX5htlprQoVVT7ljZIiS/455iNIpe8=`). Both verify on the Rust
side (`real_signed_zips_verify` test).

---


## 2026-07-17/18 sandbox Phase 0–2 (host path control)

Status summary (see `docs/ai-plugin-sandbox-roadmap.md` for full board):

- **Done:** cross-platform default input staging; fail-closed materialize errors;
  task message + `staging-report.json`; `plugin_writable_roots` allow-list;
  same-volume hardlink with copy fallback (`hardlinkedFiles` / `copiedFiles`).
- **Not done:** network OS block (Phase 3); Linux Landlock/seccomp (Phase 4);
  env strip (Phase 5); universal zero-copy across volumes / cache ref-range.
- Windows deny-ACL remains opt-in (`PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1`).
- Do not claim a complete OS sandbox in release notes.

## 2026-07-10 sandbox policy update — input staging default, deny-ACL opt-in

The Windows deny-ACL write-confinement path caused confusing host/UI behavior
when a plugin was running: it temporarily changed ACLs on real user
directories, so file/directory access prompts could appear even though the
plugin workflow could continue after dismissal. That made the default security
mechanism feel ineffective and disruptive.

### What changed

- **Default behavior**: plugin task inputs are still staged into
  `plugin-cache/<id>/tasks/<taskId>/inputs/` and payload paths are rewritten.
  Plugins read the staged copies and do not need raw access to source-image
  locations.
- **Deny-ACL mode is now explicit opt-in**:
  `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` enables the old Windows
  `icacls /deny <user>:(W) /L` write-confinement path for targeted testing.
- **Disable switch preserved**:
  `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` skips both input staging and optional
  ACL handling for plugin development/debugging.
- **Stale ACL cleanup**: normal startup best-effort removes deny ACEs left by
  older builds or crashed runs, then continues without re-applying them unless
  opt-in ACL mode is enabled.

### Verification

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` passed.
- `cargo check --manifest-path src-tauri/Cargo.toml` passed.
- `cargo build --release` failed locally at MSVC/CRT link time in
  `libort_sys`/`LibRaw` dependencies; this was not caused by the sandbox code
  change.

## 2026-07-04 Approach C sandbox — deny-ACL write confinement + input staging

Current note (2026-07-10): this section records the original implementation.
The deny-ACL path remains in the codebase but is no longer enabled by default;
see the 2026-07-10 update above for current behavior.

This pass landed the v1 scope of Approach C (process sandboxing): the last
of the three security-hardening approaches blocking the v1 contract freeze.
A (startup token) and B (package signing) were already implemented.

### What landed

- **New `src-tauri/src/t_sandbox.rs` module**: `SandboxHandle` applies a
  non-recursive deny-write ACE (`icacls /deny <user>:(W) /L`) on sensitive
  user directories (Desktop, Documents, Pictures, Videos under
  `%USERPROFILE%`, plus `PICAIPIC_SANDBOX_DENY_PATHS`) before the plugin
  process is spawned. The handle's `Drop` revokes the ACEs
  (`icacls /remove:d`) — tied to the `RunningPlugin` lifetime.
- **Spawn integration** (`t_plugin.rs`): `start_ai_plugin_runtime` applies
  the sandbox before `Command::spawn()` and stores the handle in the new
  `RunningPlugin.sandbox` field. Spawn failure drops the handle (RAII
  revoke). The start log records `sandbox: applied deny-W on N paths`.
- **Removal-site audit**: all three process-removal paths
  (`stop_ai_plugin_runtime`, `take_exited_plugin_status` crash detection,
  and the start-path immediate-exit branch) now drop the `RunningPlugin`
  explicitly so the sandbox ACLs are revoked. `stop_ai_plugin_runtime`
  revokes after `terminate_child_process_tree`.
- **Input staging** (`t_plugin.rs`): `invoke_ai_plugin_capability_inner`
  now calls `stage_input_files_for_sandbox` before constructing the payload.
  It recursively walks `inputs` JSON, copies any external `path` field into
  `plugin-cache/<id>/tasks/<taskId>/inputs/`, and rewrites the path. The
  plugin reads staged copies — it never needs raw disk access to the
  originals. Skipped when the sandbox is off.
- **Disable switch**: `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` skips both the
  ACL application and input staging (dev/debug escape hatch).
- **Idempotent cleanup**: apply pre-revokes any leftover ACE from a prior
  crashed run before re-applying.

### GPU access confirmation

`sandbox_gpu_spike.py` (v4) confirmed that deny-ACL on a directory does
**not** break ROCm/CUDA driver initialization — a sandboxed child process
successfully imported torch, reported `cuda=True`, and ran a GPU matmul
in ~1.8s. This was the central technical risk called out in the security
hardening doc.

### Spike hang fix (prerequisite)

The original `sandbox_gpu_spike.py` timed out at 300s. Root cause: ROCm
7.2 + torch on Windows deadlocks in `DLL_PROCESS_DETACH` when a
CUDA-initialized subprocess exits. Fix: spike v4 uses `Popen` + a
done-signal file + `terminate()` instead of `subprocess.run` (which waits
for exit). This is unrelated to the sandbox itself but documented here
because the spike was the gate for C.

### Out of scope (future)

- Network blocking (WFP or restricted token) — v1 relies on signature +
  token + loopback binding; documented as future.
- macOS Seatbelt / Linux seccomp.
- Strict allow-list write confinement (needs restricted token, GPU risk).
- Zero-copy handling of large video inputs.

## 2026-07-03 runtime conflict detection + uninstall mode pass

This pass implemented the two highest-priority items from the previous status:
runtime conflict detection and the code-only vs code-and-data uninstall choice.

### Runtime conflict detection

The host now compares the package versions reported by a probe against the
version specifiers declared in the plugin's requirements file, so environment
drift in a shared runtime is caught before it produces a confusing import or
ABI error.

What changed:

- New `RequirementSpec` and `RuntimeConflict` structs in `t_plugin.rs`.
- `parse_requirements_file` reads a requirements file and extracts package
  name + PEP 440 specifier pairs, skipping comments, option flags, and bare
  URL lines (e.g. ROCm direct wheels).
- `parse_version`, `compare_versions`, and `spec_satisfied` form a minimal
  hand-written PEP 440 comparator supporting `==`, `!=`, `>=`, `<=`, `>`, `<`,
  `~=`, and bare versions (treated as `==`). No `semver` crate dependency was
  added. Local version segments (`+rocm7.2.1`) are stripped before comparison.
- `normalize_package_name` maps import names to pip names (`cv2` to
  `opencv-python`, `skimage` to `scikit-image`, etc.) so probe results and
  requirements line up.
- `detect_runtime_conflicts` compares each declared spec against the probe
  `result.packages[name]` block. It produces three kinds: `version_mismatch`
  and `missing` are blocking; `unprobed` (declared but not inspected by the
  probe, e.g. NAFNet's `timm`/`skimage`) is informational.
- `PluginInstallProfileSummary` gained a `runtimeConflicts` field, populated
  in `manifest_to_summary` only when a non-stale, passed probe state exists.
- `ensure_runtime_probe_gate` now hard-blocks capability invocation when any
  `version_mismatch` or `missing` conflict is present, returning an actionable
  error that advises switching to a plugin-private runtime or re-running setup.
- The Settings probe card renders a warning block above the advice list:
  blocking conflicts use `text-warning` with a `⚠` marker, `unprobed` items use
  neutral grey with a `○` marker, and a `→ Switch to a plugin-private runtime,
  or re-run Setup` advice line appears when blocking conflicts exist.

Boundary:

- Confirmed one-click switch to plugin-private is implemented (2026-07-17).
  When blocking conflicts exist, Settings shows **Use private runtime**. After
  user confirmation, `switch_ai_plugin_profile_to_private_runtime` persists a
  synthetic `scope: "plugin"` binding (`plugin-private:<profileId>`) on the
  profile state, clears that profile's probe cache, and marks the profile
  `needsVerify`. Shared runtimes are never modified. The host still does **not**
  switch without confirmation and does **not** auto-run Setup.
- The probe script was **not** extended to inspect NAFNet-only packages
  (`timm`, `scikit-image`, `addict`, `pyyaml`). They surface as `unprobed` and
  do not block.
- `verify_setup.py` is unchanged. Conflict detection is a runtime check;
  `verify_setup` remains the install-time check.

Under normal conditions (both sample plugins pin `==` versions that match a
correctly provisioned runtime) `runtimeConflicts` is empty and nothing blocks.
Conflicts only appear when the runtime drifts, e.g. someone runs
`pip install "numpy>=2"` in a shared runtime that pins `numpy==1.26.4`.

### Uninstall mode: code only vs code + data/runtimes

Uninstall now offers a choice instead of always deleting only the code
directory.

What changed:

- `AiPluginUninstallResult` gained `mode` and `removedExtraPaths` fields.
- New `remove_existing_dir` helper deletes a directory if it exists without
  triggering `create_dir_all` (unlike `plugin_data_dir` and friends, which
  create on access). It guards every deletion with `is_path_inside` against
  the store root.
- `uninstall_ai_plugin` takes an optional `mode` parameter (defaults to
  `code_only`):
  - `code_only`: identical to the previous behavior — deletes only
    `plugins/<id>`.
  - `code_and_data`: additionally removes `plugin-data/<id>`,
    `plugin-cache/<id>`, `plugin-outputs/<id>`, and `plugin-runtimes/<id>`.
    `shared-runtimes/*` is **never** deleted because other plugins may share
    it.
- `clear_plugin_registry_state` still clears all registry state in both modes;
  the difference is purely on-disk.
- New `UninstallModeDialog.vue` component (modeled on `FileConflictDialog.vue`)
  presents two option cards: "Code only" (neutral) and "Code + data &
  runtimes" (destructive `bg-error` styling), with a clear hint that shared
  runtimes are kept.
- `Settings.vue` replaced the native `ask()` binary dialog with a
  `requestUninstallMode` → Promise → `resolveUninstallMode` flow using the
  in-app `ModalDialog`. Success toast text differs by mode.
- `api.js` `uninstallAiPlugin(pluginId, mode)` passes the mode through.
- i18n strings added under `msgbox.uninstall.*` in English and Chinese, plus
  `pluginText` fallback keys for the two success messages.

Verification:

```powershell
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --manifest-path src-tauri\Cargo.toml
pnpm --dir src-vite build
.\scripts\check_plugin_host.ps1 -SkipFrontendBuild -SkipCargoCheck -SkipCargoFmt
```

All checks passed. The default `code_only` path is byte-for-byte equivalent to
the previous uninstall behavior; only users who explicitly choose
`code_and_data` get the extra deletion.

## 2026-07-02 release UI/lifecycle polish pass

This pass closed several issues found during the packaged-plugin UI test after
shared ROCm setup succeeded.

What changed:

- App shutdown cleanup is stronger. `AiPluginRuntimeState::stop_all()` now
  scans discovered plugin manifests in addition to currently tracked runtime
  processes, so shutdown attempts to stop installed plugins even if the process
  registry missed them.
- Windows stale-listener cleanup no longer uses slow `Get-NetTCPConnection`.
  The host and both sample plugins now use `netstat -ano` plus `taskkill` as
  the port-based fallback.
- Run setup progress visibility was fixed by saving the running setup job id to
  profile state before the install command blocks on pip output.
- Smoke now shows an in-progress UI row while the synchronous smoke request is
  running. It is an indeterminate progress indicator; real percentage progress
  would require a future smoke/task event channel.
- The privacy `Revoke` button is now labelled as authorization revocation and
  its confirmation explains that it clears saved setup-download/network grants,
  not plugin files or runtimes.
- Runtime binding badges now color-code scope: shared runtimes are green,
  plugin-private runtimes are blue, and external runtimes are yellow.
- Reopening PicAiPic while a stale plugin backend is still listening no longer
  shows the plugin as normal `Running`. The Settings UI and plugin menu now
  require `reachable=true && managed=true` for a plugin to count as running.
  `reachable=true && managed=false` is shown as a stale/external service.

Verification:

```powershell
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --manifest-path src-tauri\Cargo.toml
pnpm --dir src-vite build
.\scripts\package_plugin.ps1 -All -FailOnWarnings
```

Local verification on this machine has now completed successfully:

- `cargo check --manifest-path src-tauri\Cargo.toml` passed.
- `pnpm --dir src-vite build` passed.

The only Rust issue found during verification was a small type mismatch in
`src-tauri/src/t_plugin.rs` around runtime binding scope handling; it has been
fixed locally.

The release-exe UI validation pass has now been completed for the current
workflow. The next work is to add runtime-conflict guidance for shared vs
plugin-private bindings, then improve uninstall behavior so users can choose
between removing code only versus removing code plus data and runtimes, and
finally add model import plus external model directory binding support.

## 2026-07-02 shared runtime setup pass

Run setup now creates or reuses PicAiPic-managed shared Python environments for
the bundled PyTorch sample plugins instead of relying on plugin-private venvs
or a machine-local `.local.env`.

What changed:

- `picai-salut-color` profile requirements now install the matching runtime:
  ROCm `python312-rocm72-torch291`, CUDA `python312-cuda121-torch231`, CPU
  `python312-cpu-torch231`, and DirectML `python312-directml`.
- `picai-nafnet-restore` now declares setup downloads and the same dependency
  domains as SA-LUT, so Settings can request explicit setup download
  permission before running the install script.
- Both plugin install scripts install from `PICAIPIC_PLUGIN_REQUIREMENTS_PATH`
  into `PICAIPIC_PLUGIN_ENV_DIR`, which points at
  `picaipic-local\shared-runtimes\<runtime-id>` for shared bindings.
- Both plugins now run `backend\verify_setup.py` after installation to prove
  the selected venv can import the expected Python modules and to print torch
  CUDA/HIP availability into the setup log.
- `package_plugin.ps1` no longer treats a manifest-declared setup download
  with explicit `allowedDomains` as a package warning, and network scanning is
  limited to executable/script/code files rather than README text.

Important boundary:

- Shared and plugin-private runtime paths are implemented.
- The sample plugins currently choose shared profiles in their manifests.
- A plugin can opt into a private runtime by using `runtimeBinding.scope:
  "plugin"` with an `envDir`; the host will inject
  `PICAIPIC_PLUGIN_ENV_DIR` under
  `picaipic-local\plugin-runtimes\<plugin-id>\<envDir>`.
- Automatic dependency-conflict detection is not implemented yet. The host
  does not currently inspect requirements, compare installed package versions,
  or switch a profile from shared to plugin-private without user confirmation.
  Confirmed one-click switch is available in Settings after blocking conflicts.

Next validation is a packaged-app UI pass: rebuild plugin zips, install them
from Settings, grant setup download permission, run setup for the target
profile, then Probe/Smoke.

## 2026-07-02 SA-LUT packaged runtime and RAW input pass

The packaged `picai-salut-color` zip no longer depends on the local
`D:\ailab\20260610133133\backend` source tree for SA-LUT imports.

What changed:

- `plugins\picai-salut-color\backend\engine` now includes the SA-LUT runtime
  subset used by the adapter: `device_manager.py` and `engine\salut\*.py`.
- `start.bat` now defaults `SALUT_SOURCE_DIR` to the packaged backend only when
  `backend\engine` exists, and no longer points source lookup at an arbitrary
  shared runtime root.
- Settings storage buttons now open real model/runtime drop folders:
  `modelDirs` points at manifest-derived folders such as
  `plugin-data\picai-salut-color\models\salut`; `runtimeDirs` points at shared
  runtime roots such as `shared-runtimes\python312-rocm72-torch291`.
- SA-LUT image loading now supports RAW files through `rawpy` for formats such
  as `.RW2`, `.CR2`, `.CR3`, `.NEF`, `.DNG`, `.ARW`, `.ORF`, and `.RAF`.
  OpenCV remains an IO/resize/channel-conversion/output-encoding dependency;
  the actual color transfer is still SA-LUT model inference through
  `SALUTInference`.
- `backend\requirements*.txt` now declare `numpy`, `opencv-python`, `rawpy`,
  and profile-specific torch runtime wheels. PyTorch remains profile-owned and
  goes into the shared runtime directory, not into the plugin code package.

Local verification on this machine:

- Existing ROCm venv:
  `D:\ailab\20260610133133\backend\venv\Scripts\python.exe`
- Runtime check: `torch 2.9.1+rocm7.2.1`, HIP `7.2.53211-158bd99533`,
  `AMD Radeon RX 7900 XT`.
- RAW decode check:
  `C:\Users\a7925\Desktop\新建文件夹 (6)\P1013608.RW2` decodes to
  `(4344, 5784, 3) uint8`.
- Current installed release plugin has a machine-local `.local.env` pointing at
  that venv for immediate testing. The file is intentionally excluded from zip
  packages.

Remaining product gap:

- Run setup can now populate `shared-runtimes\python312-rocm72-torch291`.
  The remaining work is release-exe UI validation: install the zip, grant setup
  download permission, run setup, then Probe/Smoke and verify the resulting
  messages are understandable for normal users.

## 2026-07-02 Startup failure actionable UX

The startup failure UX from the 2026-07-01 regression is now implemented in
the host and Settings UI.

What changed:

- `start_ai_plugin` now watches the managed start process while waiting for
  `/health`. If the start command exits before the backend becomes ready, the
  host returns a structured runtime failure instead of only surfacing the last
  health request error.
- Startup failures now include `errorCode`, `errorDomain`, `errorDetails`,
  `logTail`, and `advice` on `AiPluginStatus`.
- Settings renders a warning panel for runtime startup failures with the
  structured code/domain, actionable advice, and the tail of `logs/start.log`.
- The advice covers the failure classes that caused the real regression:
  `.bat` syntax/CRLF issues, missing Python modules such as `torch`,
  backend/runtime mismatch, and missing model/source paths that should be
  handled through `.local.env`.

Verification:

```powershell
pnpm --dir src-vite build
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --manifest-path src-tauri\Cargo.toml
.\scripts\check_plugin_host.ps1 -SkipFrontendBuild -SkipCargoCheck -SkipCargoFmt
```

Next validation is a release executable UI pass: rebuild the app, install the
generated plugin zips, and verify Start / Restart / Smoke failure and success
states inside Settings.

## 2026-07-01 Plugin startup regression fix

A real Settings > Smoke regression was found and closed on 2026-07-01.

Symptom: clicking Smoke on `picai-salut-color` returned
`error sending request for url (http://127.0.0.1:8011/health)`. The root cause
was not the smoke step itself, but `start.bat` exiting before the backend could
listen.

Two independent defects were fixed:

1. **`.bat` line endings.** All plugin `.bat` files (`start.bat`, `stop.bat`,
   `install.bat` for both plugins) had been saved with Unix LF line endings.
   `cmd.exe` rejects some LF batch files with
   `The syntax of the command is incorrect.` All plugin `.bat` files were
   converted to CRLF. There is no `.gitattributes` because this tree is not a
   git repository; the discipline is "never re-save `.bat` files as LF".
2. **`.local.env` parse syntax.** The `for /f` loop that loads `.local.env` in
   both `start.bat` files used
   `if not "%%A"=="" if not "%%A:~0,1%"=="#" set "%%A=%%B"`. The nested
   `%%A:~0,1%` substring comparison inside an `if` is rejected by `cmd.exe` on
   this machine. It was replaced with
   `for /f "usebackq eol=# tokens=1,* delims==" %%A in (".local.env") do (
   if not "%%A"=="" set "%%A=%%B" )`, using `eol=#` to skip comment lines
   instead of a runtime substring check. `picai-nafnet-restore` had the same
   pattern and was fixed the same way; it had never been spawned before (no
   `start.log`), so the bug was latent there.

After the `start.bat` fixes, the SA-LUT backend started but Smoke still failed
with `No module named 'torch'`, because `start.bat` picked the empty
`.venv\Scripts\python.exe` (created by an earlier setup attempt) instead of the
machine's real ROCm PyTorch venv at `D:\ailab\20260610133133\backend\venv`. A
`.local.env` file was added to `plugins/picai-salut-color` pointing
`PICAIPIC_SALUT_PYTHON`, `SALUT_SOURCE_DIR`, and `SALUT_MODEL_DIR` at that
external runtime. Smoke then passed.

Key decisions recorded from this session:

- `.local.env` is the supported mechanism for "reuse an already-installed
  Python/PyTorch runtime on this machine" and is the right answer for
  developers with an existing ROCm/CUDA venv. It is excluded from release
  plugin packages by `package_plugin.ps1`.
- `runtimeBinding.scope: "external"` with a non-empty `python` path in a
  release manifest is still forbidden (machine-specific paths must stay in
  `.local.env`, never in manifests/scripts). Adding an `external` binding
  with an empty `python` field to rocm/cuda profiles is a possible future
  enhancement so Settings can show "point at an existing venv" for users who
  have one, but it is not done yet.
- The failure mode exposed here — `start.bat` exits, Smoke surfaces only
   `error sending request for url` with no guidance — was implemented as
   structured startup failure UX on 2026-07-02.

## Summary

The AI plugin host is now beyond documentation-only scaffolding. It has been
validated with two real local HTTP plugins:

- `picai-salut-color`
- `picai-nafnet-restore`

The current phase is **v1 host/plugin contract hardening and freeze**, not adding a third plugin.

Short answer on interface maturity: **the core host plugin interface is basically complete for v1 local HTTP plugins**, but it is not a final "all future plugin types" interface. The host/body side now covers the important v1 boundary:

- discovery and manifest validation
- registry-backed plugin paths
- local HTTP start/stop/restart/status
- managed-runtime tracking separate from stale localhost reachability
- runtime profile, binding, probe, setup, run-setup, and smoke gates
- async invoke/task polling/event polling/cancel
- structured task errors
- output path validation
- host-owned import/adopt/discard
- Settings and `PluginActionDialog` UI for current lifecycle states

What remains before calling v1 frozen is regression and packaging discipline, not a large new interface feature. Any new host API should now require a concrete boundary-breaking finding from SA-LUT, NAFNet, or a future plugin packaging pass.

## What works

### Plugin package install/uninstall

The release app now supports the independent plugin package loop:

- build plugin zips with `.\package-plugins.bat` or `.\scripts\package_plugin.ps1 -All`
- install a generated zip from Settings > AI Plugins
- unpack the package under the user plugin directory
- register the unpacked plugin path
- refresh and show the installed plugin cards
- uninstall the installed package copy from the plugin card
- remove the user plugin directory copy and unregister the path

Manual verification on the updated executable confirmed that installing and
uninstalling the generated plugin zips works from the Settings plugin tab.
Uninstall is intentionally limited to installed package copies under the user
plugin directory; development plugin directories remain user-owned source files
and are only unregistered, not deleted.

### Plugin discovery

The debug registry contains real plugin paths:

- `D:\ailab\PicAiPic\plugins\picai-salut-color`
- `D:\ailab\PicAiPic\plugins\picai-nafnet-restore`

Settings > AI Plugins can discover and display both plugins.

### Runtime/profile validation

For NAFNet:

- manifest JSON validates
- Python backend compiles
- `start.bat` starts the local HTTP backend
- `/health` works
- `/status` works
- Probe Runtime / Run Setup / Smoke have been exercised

### Async task lifecycle

The host and plugins have exercised:

- `queued`
- `running`
- progress updates
- events polling
- cancel request
- `cancelled`
- `succeeded`
- `failed`
- output adoption/import/discard

### Build checks

These checks passed after current host/UI/plugin changes:

```powershell
cargo check --manifest-path D:\ailab\PicAiPic\src-tauri\Cargo.toml
cd D:\ailab\PicAiPic\src-vite
npm run build
```

The project now has a single regression entry point for the common host/plugin checks:

```powershell
.\scripts\check_plugin_host.ps1
```

For mock async/local-HTTP task stress checks:

```powershell
.\scripts\check_plugin_host.ps1 -IncludeStress
```

Latest scripted checks from this handoff:

- `.\scripts\check_plugin_host.ps1` passed.
- `.\scripts\check_plugin_host.ps1 -IncludeStress -FastStress -SkipFrontendBuild -SkipCargoCheck -SkipCargoFmt` passed.
- The stress pass covered SA-LUT async mock tasks, SA-LUT HTTP mock tasks, and NAFNet HTTP mock tasks, including progress events and cancellation.

## Important decisions

### Algorithm quality is plugin-owned

NAFNet exposed real-world backend problems:

- large image denoise can be too slow
- ROCm/PyTorch can hit large tensor/indexing limits
- naive tiled denoise can create color/exposure artifacts
- plugin code can hang or crash independently of host logic

These are plugin implementation issues. They should not block the host plugin contract.

### Host must be resilient to bad plugins

The host must tolerate plugins that are:

- slow
- unstable
- stuck
- returning invalid output
- unable to cancel immediately
- using heavyweight child processes

A plugin failure must become a bounded task failure, not a PicAiPic process failure.

### NAFNet denoise is experimental-fast by default

`picai-nafnet-restore` keeps NAFNet for experimentation, but default denoise now uses a fast OpenCV path. This keeps the plugin useful as an interface validation plugin without making product UX depend on NAFNet SIDD quality/performance.

### Manifest timeout hints are deferred

For v1, generic timeout hints are host policy rather than manifest contract. `smokeTest.timeoutMs` remains supported. Broader fields such as `timeouts.startMs`, `timeouts.invokeMs`, `timeouts.taskMs`, and `timeouts.cancelMs` are deferred to v1.1 unless a concrete plugin forces the issue.

## Host hardening completed

### Error domain UI

`PluginActionDialog` now displays structured error metadata when available:

- `errorCode`
- `errorDomain`
- `errorDetails`

Known UI domains include:

- `transport`
- `plugin`
- `runtime`
- `device_backend`
- `filesystem`
- `task`
- `host`

When a task fails before structured task metadata is available, the UI infers a best-effort domain from the error message.

### Stage display UI

`PluginActionDialog` now separates the important operation stages:

- starting
- invoking
- queued
- running
- importing
- cancelling
- timed out
- failed
- completed

### Backend lifecycle

- local HTTP plugins are started by `entry.startCommand`
- host passes `PICAIPIC_PLUGIN_PORT` and `PICAIPIC_PLUGIN_BASE_URL`
- if the default port is occupied, host can allocate a fresh port
- runtime status now distinguishes host-managed plugin processes from unmanaged/stale localhost services
- Settings has Start / Stop / Restart / Refresh controls

### Shutdown safety

- removed slow `Get-NetTCPConnection` port enumeration from stop path
- child process tree kill has a timeout
- stopCommand execution has a timeout
- Stop no longer reports Running just because an old backend is still reachable on a manifest/default port
- SA-LUT and NAFNet stop scripts now attempt to clean stale Python backends by script path and plugin port

### Task safety

- async task state is persisted before invoking plugin
- polling is bounded by timeout
- timeout triggers best-effort `/tasks/{taskId}/cancel`
- failed/cancelled output temp dirs are cleaned
- host validates output paths before import/adoption

## Current docs

- `D:\ailab\PicAiPic\docs\ai-plugin-contract-v1.md` — frozen v1 baseline contract with MUST/SHOULD/MAY language.
- `D:\ailab\PicAiPic\docs\ai-plugin-author-checklist.md` — checklist for future plugin authors and regression passes.
- `D:\ailab\PicAiPic\docs\ai-plugin-e2e-regression-2026-06-30.md` — E2E regression notes from SA-LUT and NAFNet.
- `D:\ailab\PicAiPic\docs\ai-plugin-ui-verification-2026-06-30.md` — dev/release UI launch and Settings verification notes.
- `D:\ailab\PicAiPic\docs\ai-plugin-stop-state-fix-2026-06-30.md` — stop/running state fix notes.
- `D:\ailab\PicAiPic\docs\release-build-2026-06-30.md` — release build output paths and hashes.

## Current contract direction

Freeze v1 around:

- manifest shape
- local HTTP lifecycle
- `/health`
- `/status`
- `/invoke/{capabilityId}`
- `/tasks/{taskId}`
- optional `/tasks/{taskId}/events`
- `/tasks/{taskId}/cancel`
- structured task errors
- output descriptors and path validation
- host-owned adopt/import/discard

## Interface completeness

### Complete enough for v1

The PicAiPic body-side plugin interface is complete enough for local HTTP AI plugins that:

- are discovered from a plugin directory containing `picaipic.plugin.json`
- expose `/health`, `/status`, `/invoke/{capabilityId}`, `/tasks/{taskId}`, optional `/tasks/{taskId}/events`, and `/tasks/{taskId}/cancel`
- receive file paths and write outputs into a host-provided task output directory
- rely on PicAiPic for import/adoption/discard instead of touching the library database
- use runtime profiles and Smoke to prove whether the selected Python/native runtime works

This is the supported v1 target. SA-LUT validates a real image workflow and NAFNet validates heavyweight runtime/lifecycle stress.

### Not complete, by design

The following are intentionally not part of the frozen v1 body interface yet:

- plugin marketplace or remote plugin installation
- plugin package signatures, updates, and remote distribution
- richer shared runtime pool management UI
- full model download/acquisition automation
- cloud/API-key plugin credential storage
- plugin web UIs with their own trusted surfaces
- non-local transports beyond the current local HTTP contract
- generic manifest timeout hints beyond `smokeTest.timeoutMs`
- `export-lut` business logic for SA-LUT
- quality/performance guarantees for individual algorithms

These should stay deferred unless a v1 freeze regression exposes a host boundary problem.

## Next work

1. Treat `docs\ai-plugin-contract-v1.md` as the current baseline. Only reopen it for boundary-breaking findings, not for algorithm quality issues.
2. Run the scripted regression before each release or contract edit:

   ```powershell
   .\scripts\check_plugin_host.ps1
   .\scripts\check_plugin_host.ps1 -IncludeStress
   ```

3. Do one final **release executable** plugin regression pass after each host/package change:
   - Settings > AI Plugins shows SA-LUT and NAFNet.
   - Install package and Uninstall package both work for the generated zips.
   - uninstall now offers code-only vs code-and-data; verify both modes and
     confirm `shared-runtimes` survives a code-and-data uninstall.
   - runtime conflict warnings appear in the probe card when drift is
     introduced (e.g. `pip install "numpy>=2"` in a shared runtime) and
     capability invocation is blocked until resolved.
   - runtime binding selector, Probe, Setup, Run setup, and Smoke remain understandable.
   - Run setup and Smoke both show immediate in-progress feedback.
   - Start / Stop / Restart / Refresh behave correctly.
   - Restart button behaves correctly.
   - closing the main app stops installed plugin backends and leaves no stale Python listener on the plugin ports.
   - if a stale backend still exists, Settings shows `Stale service` rather than `Running`, and plugin menu entries do not treat it as available.
   - privacy authorization controls are understandable (`Revoke authorization` means clearing saved setup/network grants).
   - shared runtime bindings are visually distinct from plugin-private and external runtimes.
   - `PluginActionDialog` shows stage/error/output controls correctly.
   - successful plugin output can be imported/adopted/discarded.
   - failed/cancelled plugin tasks remain bounded task failures, not app hangs.
4. Keep NAFNet as a complex sample/stress plugin, not a quality benchmark.
5. Runtime conflict detection is implemented (2026-07-03). Confirmed
   shared→plugin-private switch is implemented (2026-07-17):
   - Settings conflict block offers **Use private runtime** after user confirm.
   - Host command: `switch_ai_plugin_profile_to_private_runtime`.
   - Remaining optional work: extend the probe script to inspect NAFNet-only
     packages (`timm`, `scikit-image`, `addict`, `pyyaml`) so they move from
     `unprobed` to real checks.
6. Add first-class runtime management UI:
   - show shared/plugin/external scope, actual venv path, and disk location
   - show key package versions for `python`, `torch`, `torchvision`, `numpy`, `opencv-python`, and plugin-specific requirements
7. Uninstall mode is implemented (2026-07-03). Remaining work:
   - consider scanning registered plugins to determine whether a
     `shared-runtime` is unreferenced before offering to delete it in a future
     "deep clean" mode (currently shared runtimes are never deleted).
8. Decide duplicate-source UX for development directories vs installed package copies of the same plugin id.
9. Defer a third plugin until SA-LUT and NAFNet both pass the release UI pass, package install/uninstall, strict packaging, and runtime/model setup checks.
10. Defer SA-LUT `export-lut` implementation until after v1 host contract freeze, or mark it clearly unavailable in status/UI while it remains declared but unimplemented.
11. ~~Add model import / external model directory binding.~~ **Completed
    2026-07-08:** manifest-declared `modelBindings[]`, Settings binding UI,
    validation, persistence, and environment injection are implemented.
    **Reinforced 2026-07-17:** managed model-file presence in `list_ai_plugins`
    (`modelFiles`), Settings **Open & validate**
    (`check_ai_plugin_model_files`), and basename **Import model files** into
    `plugin-data/<id>/models` (`import_ai_plugin_model_files`).
12. Keep the host/plugin version gate covered whenever compatibility fields or
    product versioning changes.
13. Keep thumbnail/preview protocol database access library-scoped; add a
    multi-library regression fixture when the database test harness expands.

## Packaging boundary

The Windows app package script builds the PicAiPic host only:

```powershell
.\build-exe.bat
```

It does not bundle `plugins\picai-salut-color` or `plugins\picai-nafnet-restore` into the host installer. AI plugins remain independent packages registered through the plugin registry. This separation is intentional and should remain true for v1.

Independent plugin packages are built with:

```powershell
.\package-plugins.bat
```

or:

```powershell
.\scripts\package_plugin.ps1 -All
```

Current package outputs:

- `D:\ailab\PicAiPic\dist\plugins\picai-salut-color-0.1.0.zip`
- `D:\ailab\PicAiPic\dist\plugins\picai-nafnet-restore-0.1.0.zip`

The packaging script now validates required manifest shape, capability ids, menu contribution ids, and menu-to-capability references. It excludes runtime artifacts such as logs, temp files, caches, and virtual environments, then generates `picaipic.package.json` with file hashes and warnings.

The PicAiPic host can now install and uninstall these zip packages from Settings > AI Plugins. The install flow validates exactly one top-level plugin directory, reads both `picaipic.plugin.json` and `picaipic.package.json`, verifies declared file sizes and SHA256 hashes, unpacks into the user plugin directory, registers the unpacked path, and refreshes the plugin list. The uninstall flow accepts a plugin id, stops any host-managed runtime best-effort, verifies the target is inside the user plugin directory, removes that installed package copy, unregisters it, and clears stored setup/probe/task state for that plugin id. Development plugin directories outside the user plugin directory are not deleted by uninstall.

Current generated reference packages pass strict packaging with zero package
warnings. Development-machine paths should stay in `.local.env` and must not be
committed into release manifests, scripts, or backend source files.

## Stop/running state fix 2026-06-30

Stop for `picai-nafnet-restore` exposed an important host boundary issue: the UI previously treated any reachable service on the manifest/default port as `Running`, even if that service was an old backend not currently started or tracked by PicAiPic.

The host now distinguishes:

- `reachable=true, managed=true`: PicAiPic owns/tracks this runtime; UI may show Running.
- `reachable=true, managed=false`: a stale or external localhost service is reachable; UI must not show this as the current plugin Running.

Additional hardening:

- Stop no longer falls back to probing the default manifest port after the tracked runtime is removed.
- Start can allocate a fresh port if the default port is occupied by a stale backend.
- NAFNet and SA-LUT stop scripts now attempt stale Python backend cleanup by script path and plugin port.

On this machine, one old Python backend on `127.0.0.1:8012` returned `Access is denied` even for `taskkill /T /F`, so the host-side `managed` distinction is required even when OS cleanup is imperfect.

## E2E regression 2026-06-30

Regression notes are recorded in:

- `D:\ailab\PicAiPic\docs\ai-plugin-e2e-regression-2026-06-30.md`

Summary:

- NAFNet health/status/smoke/fast-denoise/cancel/failure tests passed.
- SA-LUT health/status/color-transfer passed.
- SA-LUT smoke initially failed because the synthetic smoke input was too small for model padding; fixed by changing smoke input from 16x16 to 128x128.
- After the smoke fix, SA-LUT smoke passes.
