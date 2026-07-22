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
last_updated: 2026-07-18
---

# Build Release Artifacts

## Context
App installers and plugin zips are separate products. App packaging bundles models/FFmpeg and updater metadata; plugin packaging validates manifests, excludes runtime artifacts, and optionally signs package manifests.

**Installer delivery (2026-07-18):** tagged app builds upload primarily to the **GitHub Release** for that tag. Actions artifacts are optional/best-effort because storage quota can fail after a green compile/bundle.

## Steps
1. Confirm target platform/architecture and clean source/submodule state; never include local keys, environments, caches, or test outputs.
2. Align versions across `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-vite/package.json`, and docs package when cutting a release line.
3. Install frontend dependencies with the lockfile and download required models/FFmpeg sidecars.
4. Run frontend build, Rust format/check/tests, and plugin-host checks before packaging.
5. Windows app local: use `scripts/package_windows.ps1 -CheckOnly`, then the packaging script/wrapper with the intended bundle target.
6. **Tagged Linux CI:** push annotated `v*` tag → `.github/workflows/release.yml` builds Linux targets and uploads deb/AppImage (and sigs) to a **draft** GitHub Release; finalize job writes/updates `latest.json` from release assets.
7. **Windows CI onto that tag:** workflow_dispatch `.github/workflows/release-windows.yml` with `release_tag=vX.Y.Z` → MSI + sigs to the same draft release; merge Windows platforms into `latest.json`.
8. Plugins: run `scripts/package_plugin.ps1 -All -FailOnWarnings`; for release, sign with the approved Ed25519 key and verify the real zip in the Rust path.
9. Inspect artifact names, architectures, sizes, signatures/updater JSON, and bundled resource paths on the **Release** page (not only Actions).
10. Smoke the installed/packaged application, including plugin trust/setup/start/smoke where the release contains plugin-host changes.
11. **Publish** the draft release only when the owner is ready (private repos may keep drafts indefinitely).

## Gotchas
- `cargo check` does not exercise final native linking or installer resource layout.
- Tauri updater signing and plugin Ed25519 signing are different key systems.
- `beforeBuildCommand` must work relative to the Tauri project on every platform; avoid machine-specific absolute paths.
- Native submodules must be real gitlinks and initialized recursively.
- Developer plugin directories or `.local.env` can make a packaged test pass for the wrong reason.
- **Actions artifact quota:** a red job with `Failed to CreateArtifact` often means the binary already built; fix quota or ignore best-effort upload steps. Prefer Release assets for durable installers.
- PR workflow `pr-build.yml` uses continue-on-error on artifact upload; do not treat missing PR artifacts as a failed product build.

## Verify
- [ ] Frontend build, Rust fmt/check/tests, and plugin-host regression pass.
- [ ] Required models and FFmpeg sidecars are present for each target architecture.
- [ ] Installer/updater/plugin signatures are produced and verified by their corresponding runtime.
- [ ] Draft/published GitHub Release lists expected MSI/deb/AppImage (+ sigs) and coherent `latest.json`.
- [ ] Packaged host opens, initializes DB/models, accesses a test library, and cleanly exits.
- [ ] Release plugin lifecycle is validated when affected.

## Debug
Classify failure as toolchain, native compile/link, missing resource, relative path, signature, installer layout, updater metadata, **artifact quota/upload**, or packaged runtime. Compare against the matching GitHub Actions job before changing build logic.

## Update Scaffold
- [ ] Update setup/stack/current state when platform, resource, or signing behavior changes.
- [ ] Record release blockers and key rotations with `mex log` without recording secrets.

## App icons

- Windows taskbar/exe icons come from `src-tauri/icons/icon.ico` (see `tauri.conf.json` `bundle.icon`).
- Canonical brand mark: repo-root `favicon1.ico`. Run `scripts/regenerate_app_icons.ps1`, then **clean rebuild** installers. Do not use frame `logo-pic.png` for app chrome.
