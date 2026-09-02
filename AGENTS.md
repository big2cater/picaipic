---
name: agents
description: Always-loaded PicAiPic project anchor. Read this first for project identity, hard rules, commands, and navigation.
last_updated: 2026-09-02
---

# PicAiPic

## What This Is
PicAiPic is a local-first Tauri desktop photo manager for browsing, indexing, searching, editing, and organizing large personal image and video libraries offline.
Current release line is **1.2.0** (public: Windows x64/ARM64 MSI + Linux amd64/arm64 deb/AppImage, in-app updater wired to GitHub Releases `latest.json`).

## Non-Negotiables
- Preserve the local-first privacy model: never introduce required cloud upload or remote processing for user media.
- Treat original media as user-owned source data; destructive file operations must remain explicit, guarded, and consistent with the existing trash/permanent-delete distinction.
- Keep frontend IPC wrappers in `src-vite/src/common/api.js` aligned with Rust commands registered in `src-tauri/src/main.rs`.
- Route SQLite schema changes through `src-tauri/src/t_migration.rs`; preserve per-library databases, WAL checkpointing, backup/restore, and storage migration safety.
- Do not weaken the AI plugin trust boundary: package signatures, publisher trust, permission grants, bearer-token auth, runtime conflict gates, and input staging are required behavior (the plugin UI can be hidden, the boundary stays).
- Protect performance for 10k-100k+ file libraries; scanning, thumbnail, database, and AI changes require representative large-library testing or an explicit limitation note.
- Never commit private signing keys, release secrets, user media, databases, model/runtime caches, or plugin-local environments.

## Commands
- Dev: `cargo tauri dev`
- Frontend build: `pnpm --dir src-vite build`
- Rust format: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- Rust check: `cargo check --manifest-path src-tauri/Cargo.toml`
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml`
- Plugin-host regression: `powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1`
- Plugin-host stress: `powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1 -IncludeStress -FastStress`
- Windows package: `.\scripts\package_windows.ps1 -CheckOnly` (preflight) or `.\build-exe.bat` (build)
- Docs site (local): `pnpm --dir docs build`; deployed from `docs/` on main via `deploy-docs.yml` (manual trigger).

## After Every Task
After meaningful work:
- Record what actually changed (behavior, command, dependency, workflow) in a commit message that states it plainly.
- Keep user-facing docs accurate: `docs/guide/` (getting-started, live-photo) and, when behavior ships, a new `docs/guide/release-notes/<version>.md` entry.
- If a change is worth remembering, note it in the release notes for the current line rather than scattered agent docs.

## Navigation
- Product state and recent history: `README.md` (EN) + `i18n/README.zh-CN.md`, plus per-version `docs/guide/release-notes/`.
- User-facing feature guides live in `docs/guide/`; the VitePress site renders `docs/` (`docs/index.md` homepage, `docs/.vitepress/config.mts` sidebar).
- ComfyUI integration status and runbook: `docs/comfyui-integration-status.md` is local-only; the backend module is `src-tauri/src/t_comfy.rs`, frontend conversion in `src-vite/src/common/comfyConvert.js`.
- Release/updater plumbing: `tauri.conf.json` updater endpoint + `latest.json` on GitHub Releases (four platforms; Windows entries are merged by `release-windows.yml`), `.github/workflows/release.yml` and `release-windows.yml`.