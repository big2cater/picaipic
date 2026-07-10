---
name: agents
description: Always-loaded PicAiPic project anchor. Read this first for project identity, hard rules, commands, and MEX navigation.
last_updated: 2026-07-10
---

# PicAiPic

## What This Is
PicAiPic is a local-first Tauri desktop photo manager for browsing, indexing, searching, editing, and organizing large personal image and video libraries offline.

## Non-Negotiables
- Preserve the local-first privacy model: never introduce required cloud upload or remote processing for user media.
- Treat original media as user-owned source data; destructive file operations must remain explicit, guarded, and consistent with the existing trash/permanent-delete distinction.
- Keep frontend IPC wrappers in `src-vite/src/common/api.js` aligned with Rust commands registered in `src-tauri/src/main.rs`.
- Route SQLite schema changes through `src-tauri/src/t_migration.rs`; preserve per-library databases, WAL checkpointing, backup/restore, and storage migration safety.
- Do not weaken the AI plugin trust boundary: package signatures, publisher trust, permission grants, bearer-token auth, runtime conflict gates, and input staging are required behavior.
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

## After Every Task
After meaningful work, run GROW:
- Ground: state what behavior, command, dependency, or workflow actually changed.
- Record: update `.mex/ROUTER.md` and the relevant `.mex/context/` files.
- Orient: create or improve a `.mex/patterns/` runbook when the task can recur.
- Write: bump `last_updated` on changed scaffold files and use `mex log` when rationale, risk, or follow-up matters.

## Navigation
At the start of every session, read `.mex/ROUTER.md`. It routes architecture, stack, conventions, decisions, setup, plugin-runtime, and recurring-task guidance.
