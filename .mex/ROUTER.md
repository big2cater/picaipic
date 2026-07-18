---
name: router
description: PicAiPic session bootstrap, current state, and context routing hub.
edges:
  - target: context/architecture.md
    condition: when understanding application flow or changing a major subsystem
  - target: context/stack.md
    condition: when working with dependencies, versions, or build tooling
  - target: context/conventions.md
    condition: when writing or reviewing Rust, Vue, IPC, database, or UI code
  - target: context/decisions.md
    condition: when a non-obvious design choice or historical rationale matters
  - target: context/setup.md
    condition: when running, validating, packaging, or troubleshooting the development environment
  - target: context/plugin-runtime.md
    condition: when touching AI plugins, manifests, runtimes, permissions, tasks, packaging, or sandboxing
  - target: patterns/INDEX.md
    condition: before any implementation or diagnosis task
last_updated: 2026-07-18
---

# Session Bootstrap

Read root `AGENTS.md`, then this file, then the routed context and matching pattern before changing code.

## Current Project State

**Working:**
- Tauri 2/Rust desktop host with Vue 3 frontend for Windows and Linux.
- **v1.1.0** app/docs versions aligned; tag `v1.1.0` has a **private draft** multi-arch release (Linux deb/AppImage + Windows x64/arm64 MSI + updater latest JSON on the Release assets). Not published; keep private until the owner decides.
- Release CI publishes installers to **GitHub Release assets** (not Actions artifact storage) after quota failures; PR builds use best-effort artifact upload — see `patterns/release-build.md`.
- Folder-first multi-library browsing, SQLite metadata (schema v6), indexing/recovery, thumbnails, timeline/folder/location/camera/lens/tag/favorite/rating/face filters, deduplication, image editing, and broad image/RAW/video support.
- Rename/move disk↔DB consistency: `rename_file` / `rename_folder` roll back disk on DB failure (aligned with `move_file`); `edit_album` propagates name-column errors; dedup `get_files_by_sizes` reuses precomputed suspicious sizes via chunked `IN` binds.
- Local AI search and face processing use bundled ONNX models; FFmpeg is bundled as a sidecar for video workflows.
- AI plugin host: discovery, signed package install/trust, permissions, install profiles, shared/private/external Python runtimes, lifecycle, async tasks, output adopt/discard, runtime-conflict detection, two sample plugins.
- Plugin security A+B+C: bearer token, Ed25519 package signing/trust, default input-file staging; Windows deny-ACL opt-in only.
- Sandbox **Phase 0–2 done**: cross-platform staging, fail-closed, diagnostics, `plugin_writable_roots`, same-volume hardlink→copy. Phase 3–4 (network OS / Landlock) **not** implemented — `docs/ai-plugin-sandbox-roadmap.md`.
- Live Photo / Motion Photo: detect/pair/preview/export; HEIC-internal video; keyframe overwrite (JPEG); album rescan; user guide `docs/guide/live-photo.md`.
- Confirmed shared→plugin-private runtime switch + managed model open/validate/import (Settings).
- Merged to main (2026-07-18): Live Photo polish (#1), sandbox Phase 0–2 + runtime/model UX (#2).

**Not yet built / future work:**
- Built-in tools plan (planned only): crop photo-size sub-menu, collage/拼图, batch wizard — **`docs/guide/builtin-tools-roadmap.md`** (order A→B→C).
- Sandbox Phase 3–5: network OS block, Linux Landlock/seccomp, env hygiene, optional cache ref/range — roadmap doc above.
- Signing-key rotation/revocation design; recurring release-exe plugin regression after host changes.
- Broader HEIC sequence sample coverage; broader automated coverage outside plugin-host + current Rust unit tests.
- Publish v1.1.0 draft release (owner decision; repo remains private for now).

**Known issues / active risks:**
- Packaged-plugin behavior must be checked in the release executable; dev-mode success alone does not prove installer/resource/runtime correctness.
- GitHub Actions **artifact storage quota** can fail uploads even when builds succeed; prefer Release assets for installers; PR upload is best-effort.
- Release Rust builds can fail at local MSVC/CRT link time in native deps (ONNX/LibRaw) even when `cargo check` passes.
- AI plugin compatibility enforces min/max PicAiPic versions and plugin API major.
- Protocol thumbnail/preview resolve against library id in the URL; preserve isolation.
- pnpm is the sole JS package manager; host/frontend/docs versions aligned at **1.1.0**.
- Historical internal identifiers may still use `Lap`; user-visible paths corrected.
- ffprobe ContentIdentifier key name may vary; `first_exist` checks dotted and underscored variants.

## Routing Table

| Task type | Load |
|-----------|------|
| Understand application flow or subsystem boundaries | `context/architecture.md` |
| Work with libraries, versions, native dependencies, or CI | `context/stack.md` |
| Write/review Rust, Vue, IPC, database, or UI code | `context/conventions.md` |
| Make or revisit a design choice | `context/decisions.md` |
| Set up, run, verify, or package | `context/setup.md` |
| Change AI plugin host, manifest, runtime, task, trust, or sandbox | `context/plugin-runtime.md` |
| Change Live Photo / Motion Photo detection, pairing, or preview | `patterns/change-live-photo.md` |
| Plan or implement built-in crop presets, collage, or batch tools | `docs/guide/builtin-tools-roadmap.md` then `patterns/INDEX.md` |
| Build/release installers or plugin packages | `patterns/release-build.md` |
| Perform any recurring task | `patterns/INDEX.md` |

## Behavioural Contract

1. **CONTEXT** — Read the routed context and matching pattern; use current code/docs as truth if memory conflicts.
2. **BUILD** — Keep changes focused and preserve the non-negotiables in `AGENTS.md`.
3. **VERIFY** — Run every applicable item in `context/conventions.md`; plugin changes also use `scripts/check_plugin_host.ps1`.
4. **DEBUG** — Use the matching debug pattern, reproduce at the narrowest boundary, then rerun verification.
5. **GROW** — Update current state/context/patterns, bump `last_updated`, and record material decisions, risks, or todos with `mex log`.
