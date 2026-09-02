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
After meaningful work, run GROW (local, `.mex/` is not tracked — it stays on this machine):
- Ground: state what behavior, command, dependency, or workflow actually changed.
- Record: update `.mex/ROUTER.md` and the relevant `.mex/context/` files.
- Orient: create or improve a `.mex/patterns/` runbook when the task can recur.
- Write: bump `last_updated` on changed scaffold files; use `mex log` when rationale, risk, or follow-up matters. Keep user-facing docs (`docs/guide/`, release notes) accurate in the same pass.

## Navigation
At the start of every session, read `.mex/ROUTER.md` (local-only; restore from git history via `git archive <pre-cleanup-commit> .mex | tar -x -C .` if missing). It routes architecture, stack, conventions, decisions, setup, plugin-runtime, and recurring-task guidance.
- User-facing docs: `docs/guide/` (getting-started / live-photo) + per-version `docs/guide/release-notes/`; the VitePress site renders `docs/` (homepage `docs/index.md`, sidebar `docs/.vitepress/config.mts`).
- ComfyUI integration: backend `src-tauri/src/t_comfy.rs`, UI-format conversion in `src-vite/src/common/comfyConvert.js`, dialogs `ComfyWorkflowDialog.vue` / `ComfyRunDialog.vue`.
- Release/updater plumbing: `tauri.conf.json` updater endpoint → `/releases/latest/download/latest.json` (four platforms; Windows entries merged by `release-windows.yml`), `.github/workflows/release.yml` + `release-windows.yml`.
- Release/updater plumbing: `tauri.conf.json` updater endpoint + `latest.json` on GitHub Releases (four platforms; Windows entries are merged by `release-windows.yml`), `.github/workflows/release.yml` and `release-windows.yml`.