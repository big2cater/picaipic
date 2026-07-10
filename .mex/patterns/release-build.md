---
name: release-build
description: Build or validate PicAiPic application and AI plugin release artifacts without missing native resources or signing requirements.
triggers:
  - release
  - package app
  - build installer
  - package plugin
  - updater signing
edges:
  - target: context/setup.md
    condition: when preparing toolchains, resources, or commands
  - target: context/stack.md
    condition: when native/version constraints cause failures
  - target: context/plugin-runtime.md
    condition: when producing plugin packages
last_updated: 2026-07-10
---

# Build Release Artifacts

## Context
App installers and plugin zips are separate products. App packaging bundles models/FFmpeg and updater metadata; plugin packaging validates manifests, excludes runtime artifacts, and optionally signs package manifests.

## Steps
1. Confirm target platform/architecture and clean source/submodule state; never include local keys, environments, caches, or test outputs.
2. Install frontend dependencies with the lockfile and download required models/FFmpeg sidecars.
3. Run frontend build, Rust format/check/tests, and plugin-host checks before packaging.
4. Windows app: use `scripts/package_windows.ps1 -CheckOnly`, then the packaging script/wrapper with the intended bundle target.
5. Linux/app CI: follow `.github/workflows/release.yml` dependencies, resource download, target, and bundle arguments.
6. Plugins: run `scripts/package_plugin.ps1 -All -FailOnWarnings`; for release, sign with the approved Ed25519 key and verify the real zip in the Rust path.
7. Inspect artifact names, architectures, sizes, signatures/updater JSON, and bundled resource paths.
8. Smoke the installed/packaged application, including plugin trust/setup/start/smoke where the release contains plugin-host changes.

## Gotchas
- `cargo check` does not exercise final native linking or installer resource layout.
- Tauri updater signing and plugin Ed25519 signing are different key systems.
- `beforeBuildCommand` must work relative to the Tauri project on every platform; avoid machine-specific absolute paths.
- Native submodules must be real gitlinks and initialized recursively.
- Developer plugin directories or `.local.env` can make a packaged test pass for the wrong reason.

## Verify
- [ ] Frontend build, Rust fmt/check/tests, and plugin-host regression pass.
- [ ] Required models and FFmpeg sidecars are present for each target architecture.
- [ ] Installer/updater/plugin signatures are produced and verified by their corresponding runtime.
- [ ] Packaged host opens, initializes DB/models, accesses a test library, and cleanly exits.
- [ ] Release plugin lifecycle is validated when affected.

## Debug
Classify failure as toolchain, native compile/link, missing resource, relative path, signature, installer layout, updater metadata, or packaged runtime. Compare against the matching GitHub Actions job before changing build logic.

## Update Scaffold
- [ ] Update setup/stack/current state when platform, resource, or signing behavior changes.
- [ ] Record release blockers and key rotations with `mex log` without recording secrets.
