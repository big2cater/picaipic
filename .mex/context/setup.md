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
last_updated: 2026-07-18
---

# Setup

## Prerequisites

- Node.js 20+ and pnpm (CI currently uses pnpm 9).
- Rust stable plus `cargo install tauri-cli --version "^2.0.0" --locked`.
- Git with recursive submodule support for native third-party libraries.
- Windows: Visual C++ build tools/runtime and PowerShell; Linux: WebKitGTK 4.1, GTK/appindicator, build tools, clang/nasm/pkg-config/autotools/cmake and related packages from README/CI.
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
- `.\scripts\package_windows.ps1 -CheckOnly` — Windows packaging preflight; `.\build-exe.bat` performs the build.
- Tag release (Linux draft + assets): push an annotated `v*` tag; workflow `.github/workflows/release.yml` builds Linux and uploads packages to the tag’s GitHub Release.
- Windows release assets: workflow_dispatch `.github/workflows/release-windows.yml` with `release_tag=vX.Y.Z` after the draft tag/release exists; updates `latest.json` with Windows entries.
- Current line: app versions **1.1.0**; draft release **v1.1.0** may exist unpublished on a private repo.

## Common Issues

**Tauri cannot reach the dev server:** keep Vite bound to `127.0.0.1:3580`; `tauri.conf.json` expects that exact URL.

**AI models or FFmpeg resources are missing:** rerun the platform download scripts before development/package builds.

**Windows AI startup reports missing runtime:** install the correct Microsoft Visual C++ Redistributable for x64/arm64 and restart.

**`cargo check` passes but release link fails:** inspect native MSVC/CRT compatibility for ONNX Runtime, LibRaw, and bundled C/C++ libraries; a check build does not prove final linker compatibility.

**CI fails only on CreateArtifact / artifact quota:** build may still have succeeded. Delete old Actions artifacts and/or rely on Release-asset upload paths; PR artifact upload is best-effort. See `patterns/release-build.md`.

**Plugin starts on a stale/default port or appears unmanaged:** ensure the backend honors host-injected `PICAIPIC_PLUGIN_PORT`, uses bearer auth, and terminate stale listeners before retesting.
