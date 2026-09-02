---
name: setup
description: PicAiPic development, validation, packaging, and environment setup.
triggers:
  - setup
  - install
  - run
  - build
  - package
  - environment
edges:
  - target: context/stack.md
    condition: when tool or dependency versions matter
  - target: context/architecture.md
    condition: when diagnosing startup/resource flow
  - target: context/plugin-runtime.md
    condition: when provisioning or testing plugin runtimes
  - target: patterns/release-build.md
    condition: when producing app or plugin artifacts
last_updated: 2026-07-30
---

# Setup

## Prerequisites

- Node.js 20+ and pnpm (CI currently uses pnpm 9).
- Rust stable plus `cargo install tauri-cli --version "^2.0.0" --locked`.
- Git with recursive submodule support for native third-party libraries.
- Windows: Visual C++ build tools/runtime and PowerShell 7 (`pwsh`); Linux: WebKitGTK 4.1, GTK/appindicator, build tools, clang/nasm/pkg-config/autotools/cmake and related packages from README/CI.
- Python is required for plugin checks, signing tools, and sample AI plugin environments.

## First-time Setup

1. `git submodule update --init --recursive`
2. `pnpm --dir src-vite install`
3. Windows: `.\scripts\download_models.ps1` and `.\scripts\download_ffmpeg_sidecar.ps1`; Linux: `bash scripts/download_models.sh` and `bash scripts/download_ffmpeg_sidecar.sh`.
4. Install Tauri CLI if missing: `cargo install tauri-cli --version "^2.0.0" --locked`.
5. Start development from repository root: `cargo tauri dev` (Vite binds to `127.0.0.1:3580`).
6. Before plugin work, ensure Python and the sample plugin requirements/runtimes needed for the selected profile are available.

## Environment Variables

- No environment variable is required for ordinary host development after resources are downloaded.
- `APTABASE_KEY` (optional, compile time) — enables Aptabase telemetry; absent means no Aptabase plugin is registered.
- `HF_TOKEN` (optional/CI) — authenticated model download access.
- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (release only) — updater artifact signing; never commit values.
- `PICAIPIC_PLUGIN_STORE_DIR` (optional) — overrides the plugin store directory.
- `PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1` (developer only) — bypasses release plugin signature enforcement.
- `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` (developer/debug only) — disables input staging and optional ACL behavior.
- `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` (targeted Windows testing only) — enables deny-ACL write confinement.
- `PICAIPIC_AI_INTRA_THREADS` (optional, 1-64) — ONNX intra-op CPU threads; default is 2 and 4/8-thread runs must be judged by scan wall time, not CPU percentage.
- `PICAIPIC_SCAN_PHASE_PROFILE=1` (optional) — prints traversal/index/drain/thumbnail/embedding phase timings for a scan.
- With phase profiling, `index_metadata_*_seconds` further splits cold `AFile::new` work, including EXIF/header, binary TIFF, Motion XMP, and extraction paths; normal scans do not pay for these stage clocks. Use a newly copied media directory for cold comparisons and never delete/recreate an existing album just to profile it.
- `PICAIPIC_SCAN_SLOW_FILE_MS` (optional, 1-600000) — with phase profiling, emits only slow per-file index records above this millisecond threshold.
- `PICAIPIC_EMBED_FILE_TRACE=1` (optional targeted diagnosis) — logs per-file embedding source and success.
- `PICAIPIC_SANDBOX_DENY_PATHS` (optional targeted test) — adds directories to the Windows deny list.
- Plugin runtime variables such as `PICAIPIC_PLUGIN_PORT`, `PICAIPIC_PLUGIN_AUTH_TOKEN`, roots, output/task directories, profile/runtime ids, Python/env paths, and requirements paths are normally injected by the host, not configured globally by users.

## Common Commands

- `cargo tauri dev` — run the full desktop application with Vite hot reload.
- `pnpm --dir src-vite build` — production frontend build.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` — Rust formatting check.
- `cargo check --manifest-path src-tauri/Cargo.toml` — compile-check the Rust host and native bindings.
- `cargo test --manifest-path src-tauri/Cargo.toml` — run Rust unit tests.
- `.\scripts\check_plugin_host.ps1` — manifests, Rust fmt/check, frontend build, and Python compile checks.
- `.\scripts\check_plugin_host.ps1 -IncludeStress -FastStress` — add mock async/local-HTTP plugin task stress.
- `.\scripts\package_plugin.ps1 -All -FailOnWarnings` — validate/package all plugins; add signing options for release artifacts.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File .\scripts\package_windows.ps1 -CheckOnly` — Windows packaging preflight; add `-Clean` for a full local rebuild. Use PowerShell 7 for project terminal commands.
- `.\build-exe.bat` builds both NSIS and MSI by default and closes running PicAiPic processes from both build and installed locations first. Reinstalling an unchanged MSI product version can enter cached maintenance behavior, so uninstall or bump the version before validating MSI.
- Tag release (Linux draft + assets): push an annotated `v*` tag; workflow `.github/workflows/release.yml` builds Linux and uploads packages to the tag’s GitHub Release.
- Windows release assets: workflow_dispatch `.github/workflows/release-windows.yml` with `release_tag=vX.Y.Z` after the draft tag/release exists; updates `latest.json` with Windows entries.
- Current line: app versions **1.1.0**; draft release **v1.1.0** may exist unpublished on a private repo.

## Common Issues

**Tauri cannot reach the dev server:** keep Vite bound to `127.0.0.1:3580`; `tauri.conf.json` expects that exact URL.

**AI models or FFmpeg resources are missing:** rerun the platform download scripts before development/package builds.

**Windows icon regeneration fails with `os error 1224`:** current scripts generate under a temporary directory and hash-sync changed Windows/Linux/shared files only. If a genuinely changed destination is still locked, close PicAiPic and image preview windows, then rerun `build-exe.bat`.

**A newly built installer opens an older-looking UI:** first confirm every old PicAiPic process is closed before starting the installed app. MSI maintenance is also keyed by product version and may reuse cached state when, for example, another `1.1.0` MSI is run; uninstall/bump the version before validating MSI. The package script prints this warning whenever MSI output is requested.

**A dynamic theme background works but photos do not distort:** the ambient and photo layers are independent. Confirm main-window native maximize, 6 seconds idle, intensity above 0, Windows Animation effects enabled, and no modal/library switch. Photo WebGL buffers clamp to GPU limits; capture/GL failure should show the theme-specific CSS photo fallback. See `docs/guide/fx-theme-runtime-compatibility.md`.

**Local packaging reports `D:\ailab\src-vite` missing:** the generated Tauri override commands run from the repository root and must use `pnpm --dir src-vite ...`. Do not copy the base `tauri.conf.json` path (`../src-vite`), whose relative path is resolved under different config handling.

**Windows AI startup reports missing runtime:** install the correct Microsoft Visual C++ Redistributable for x64/arm64 and restart.

**`cargo check` passes but release link fails:** inspect native MSVC/CRT compatibility for ONNX Runtime, LibRaw, and bundled C/C++ libraries; a check build does not prove final linker compatibility.

**CI fails only on CreateArtifact / artifact quota:** build may still have succeeded. Delete old Actions artifacts and/or rely on Release-asset upload paths; PR artifact upload is best-effort. See `patterns/release-build.md`.

**Plugin starts on a stale/default port or appears unmanaged:** ensure the backend honors host-injected `PICAIPIC_PLUGIN_PORT`, uses bearer auth, and terminate stale listeners before retesting.
