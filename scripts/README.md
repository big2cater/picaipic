# Scripts

Utility scripts for the PicAiPic project.

## 1. download_models

Downloads AI model files (CLIP + InsightFace) required for the app to `src-tauri/resources/models/`. Already downloaded files are skipped automatically.

### Usage

**Linux:**
```bash
./scripts/download_models.sh
```

**Windows (PowerShell):**
```powershell
.\scripts\download_models.ps1
```

### Models Downloaded

| Model | Source | Purpose |
|-------|--------|---------|
| `tokenizer.json` | OpenAI CLIP ViT-B/32 | Text tokenization |
| `text_model.onnx` | CLIP ViT-B/32 (quantized) | Text embedding |
| `vision_model.onnx` | CLIP ViT-B/32 (quantized) | Image embedding |
| `det_500m.onnx` | InsightFace Buffalo-S | Face detection |
| `w600k_mbf.onnx` | InsightFace Buffalo-S | Face recognition |

## 2. download_ffmpeg_sidecar

Downloads FFmpeg and FFprobe sidecar binaries for the current platform into `src-tauri/resources/ffmpeg/`. This ensures the packaged app can include the sidecar binaries with releases built from source.

### Usage

**Linux:**
```bash
./scripts/download_ffmpeg_sidecar.sh
```

**Windows (PowerShell):**
```powershell
.\scripts\download_ffmpeg_sidecar.ps1
```

### Sidecar Files Downloaded

| Platform | Files |
|----------|-------|
| Linux x86_64 | `ffmpeg-x86_64-unknown-linux-gnu`, `ffprobe-x86_64-unknown-linux-gnu` |
| Linux aarch64 | `ffmpeg-aarch64-unknown-linux-gnu`, `ffprobe-aarch64-unknown-linux-gnu` |
| Windows x86_64 | `ffmpeg-x86_64-pc-windows-msvc.exe`, `ffprobe-x86_64-pc-windows-msvc.exe` |

## 3. plugin stress scripts

Mock-task stress scripts validate the local HTTP plugin task contract without
loading large AI models. They check queued/running/succeeded/cancelled states,
progress events, long-poll task events, and running-task cancellation.

```powershell
python scripts\stress_salut_http.py --tasks 6 --duration-ms 250 --cancel-every 3
python scripts\stress_nafnet_http.py --tasks 6 --duration-ms 250 --cancel-every 3
```

The scripts start the plugin HTTP server on a free local port, run mock work,
assert the expected terminal states, and terminate the server before exiting.

## 4. package_windows

Builds the Windows release app and installers in one command. This packages the
PicAiPic host app only; AI plugins under `plugins\` remain independent packages
that are registered and run through the plugin registry. The script checks local
tools, downloads missing host-bundled models/FFmpeg sidecars, installs frontend
dependencies if needed, disables updater signing artifacts for local builds, and
then runs Tauri.

From the project root:

```powershell
.\build-exe.bat
```

Or call the PowerShell script directly:

```powershell
.\scripts\package_windows.ps1
```

Useful options:

```powershell
.\scripts\package_windows.ps1 -Clean
.\scripts\package_windows.ps1 -CheckOnly
.\scripts\package_windows.ps1 -Bundle nsis
.\scripts\package_windows.ps1 -Bundle none
.\scripts\package_windows.ps1 -OpenOutput
```

Default outputs:

- `src-tauri\target\release\PicAiPic.exe`
- `src-tauri\target\release\bundle\nsis\PicAiPic_<version>_x64-setup.exe`
- `src-tauri\target\release\bundle\msi\PicAiPic_<version>_x64_en-US.msi`

## 5. check_plugin_host

Runs the host/plugin regression checks that are useful before freezing the v1
plugin contract or making a release build.

Default checks:

- both plugin manifests parse as JSON
- Rust formatting check
- Rust `cargo check`
- frontend production build
- SA-LUT and NAFNet backend Python compile checks

Usage:

```powershell
.\scripts\check_plugin_host.ps1
```

Include mock task stress tests for the async/local-HTTP plugin protocol:

```powershell
.\scripts\check_plugin_host.ps1 -IncludeStress
```

For a shorter stress pass:

```powershell
.\scripts\check_plugin_host.ps1 -IncludeStress -FastStress
```

## 6. package_plugin

Builds independent plugin zip packages. This is separate from the PicAiPic host
installer and should be used for directories under `plugins\`.

Package one plugin:

```powershell
.\scripts\package_plugin.ps1 .\plugins\picai-salut-color
```

Package all discovered plugin directories:

```powershell
.\scripts\package_plugin.ps1 -All
```

Or use the root one-click wrapper:

```powershell
.\package-plugins.bat
```

The script excludes runtime artifacts such as `logs\`, `tmp\`, `__pycache__\`,
virtual environments, `.pyc`, and `.log` files. It also emits warnings for
hard-coded absolute development paths so release candidates can tighten them
before distribution.

The script validates basic manifest shape before packaging: required plugin
metadata, compatibility, entry kind, capability fields, duplicate ids, and menu
contributions that reference missing capabilities.

Pass `-SignKeyFile <path>` to sign `picaipic.package.json` with an Ed25519
keypair during packaging (see section 7 below).

Default outputs:

- `dist\plugins\picai-salut-color-0.1.0.zip`
- `dist\plugins\picai-nafnet-restore-0.1.0.zip`

## 7. sign_plugin

Ed25519 keypair generation and manifest signing tool. Release packages must
be signed; unsigned packages are refused in release builds (developer mode
`PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1` bypasses this).

Generate a keypair (prints base64 private + public keys):

```bash
python scripts/sign_plugin.py generate-key
```

Sign a package manifest in-place (removes any existing `signature` field,
canonicalizes the JSON, signs, writes the `signature` object back):

```bash
python scripts/sign_plugin.py sign <picaipic.package.json> <private-key-base64>
```

`package_plugin.ps1 -SignKeyFile <key.txt>` invokes this automatically during
packaging. Requires the Python `cryptography` library.

## 8. sandbox_gpu_spike

A minimal spike that confirms the Windows deny-ACL sandbox mechanism does
**not** break ROCm/CUDA driver initialization — the central technical risk
for Approach C (process sandboxing) in the security hardening design.

It spawns a child Python process that imports `torch`, checks
`torch.cuda.is_available()`, and runs a GPU matmul, under an `icacls /deny`
write-restricted directory. Pass 1 runs without ACL; Pass 2 applies the
deny-ACL on the work directory. Both should report `cuda=True` and a
successful matmul, confirming GPU access survives the sandbox.

```bash
python scripts/sandbox_gpu_spike.py
```

Note: the child is killed after signaling completion rather than waited on,
because ROCm 7.2 + torch on Windows deadlocks in `DLL_PROCESS_DETACH` when a
CUDA-initialized subprocess exits. See `docs/ai-plugin-current-status.md`
(2026-07-04 entry) for details.
