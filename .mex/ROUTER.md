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
- Tauri 2/Rust desktop host with Vue 3 frontend for Windows and Linux; first `v1.0.0` draft release pipeline has built Windows x64/arm64 and Linux x86_64/aarch64 artifacts.
- Folder-first multi-library browsing, SQLite metadata (schema v6), indexing/recovery, thumbnails, timeline/folder/location/camera/lens/tag/favorite/rating/face filters, deduplication, image editing, and broad image/RAW/video support.
- Rename/move disk↔DB consistency: `rename_file` / `rename_folder` roll back disk on DB failure (aligned with `move_file`); `edit_album` propagates name-column errors; dedup `get_files_by_sizes` reuses precomputed suspicious sizes via chunked `IN` binds.
- MediaViewer floating toolbar uses `props.file?.file_type` (null-safe); Live Photo playback guards against a cleared `props.file` before extract.
- Motion `content_id` parsing is centralized in `t_xmp::parse_motion_content_id` (shared by commands + export).
- Local AI search and face processing use bundled ONNX models; FFmpeg is bundled as a sidecar for video workflows.
- AI plugin host supports discovery, signed package install/trust, permissions, install profiles, shared/private/external Python runtimes, lifecycle control, async tasks, output adoption/discard, runtime-conflict detection, and two sample plugins.
- Plugin security A+B+C is present: startup bearer token, Ed25519 package signing/trust, and default input-file staging; Windows deny-ACL confinement remains explicit opt-in.
- Apple Live Photo (HEIC/JPEG+MOV) and Google Motion Photo (JPEG embedded MP4) detection, pairing, and long-press preview playback across backend scanning (EXIF ContentIdentifier, ffprobe, XMP `quick-xml`) and frontend MediaViewer/Thumbnail/FileInfo UI.
- Motion Photo extract cache: `app_cache_dir()/motion_cache/` with source-keyed reuse, size-based prune, startup purge of legacy OS-temp extracts; cleared with `clear_video_cache`.
- Live Photo export + conversion via `export_live_photo` + `LivePhotoExportDialog`: still / video / pair / to_motion / to_pair / set_keyframe (does not modify library originals).
- HEIC container-internal video (`live_photo_type=4`): libheif mime-item extract preferred, sequence track via ffmpeg demux fallback; long-press preview + export reuse motion_cache.
- Live Photo polish: confirmed JPEG keyframe overwrite of original still; album-level `rescan_live_photo_metadata` (no full reindex); FileInfo export button + album context-menu repair entry.
- Confirmed shared→plugin-private runtime switch (2026-07-17): Settings conflict block offers **Use private runtime**; host persists synthetic `plugin-private:<profileId>` binding, clears that profile's probe cache, marks `needsVerify`; shared runtimes untouched; Setup still user-driven.
- Model UX (2026-07-17): Settings storage shows managed model file presence; **Open & validate** rechecks managed model dir; **Import model files** copies selected checkpoints into `plugin-data/<id>/models` by basename; external model-dir binding also has open+validate.
- Sandbox Phase 0–2 **done** (2026-07-17/18): cross-platform staging + fail-closed + diagnostics; `plugin_writable_roots` allow-list; same-volume hardlink then copy (`hardlinkedFiles`/`copiedFiles`); adoption still task-output only. Phase 3–4 (network OS / Landlock) **not** implemented — `docs/ai-plugin-sandbox-roadmap.md`.
- Phase 0 Windows host-path acceptance (2026-07-18): `input_staging*` (5 tests) + `scripts/check_plugin_host.ps1` green; real-layout unit test writes the task staging report; this machine library on C: → plugin store on D: proves **copy** fallback (WinError 17 hardlink). Checklist: `docs/ai-plugin-sandbox-phase0-verify.md`.
- SA-LUT full E2E (2026-07-18): host-equivalent start (ROCm shared runtime + bearer) + stage 2 album JPGs from Downloads into the plugin task inputs dir (0 hardlink / 2 copy) + color-transfer invoke → PNG under the task outputs dir (~13MB). Task id phase0-e2e-de513e5c.
- Plugin smoke (2026-07-18): `scripts/check_plugin_host.ps1` + package preflight green; user confirmed GUI / release-shell smoke pass for this sandbox work.
- GitHub Actions build documentation, app releases, plugin packages, and VitePress documentation.
- Release line: app/docs versions bumped toward **v1.1.0** (Live Photo polish + sandbox Phase 0–2 + model/runtime UX).

**Not yet built / future work:**
- Built-in tools plan (2026-07-18): crop photo-size sub-menu + presets, collage/拼图 modes, batch wizard — see `docs/guide/builtin-tools-roadmap.md` (planned only; not implemented).
- Sandbox Phase 3–5 only: network OS block, Linux Landlock/seccomp, env hygiene, optional cache ref/range zero-copy — see `docs/ai-plugin-sandbox-roadmap.md`. Phase 0–2 host path control is done.
- Signing-key rotation/revocation design; keep release-executable plugin regression as a recurring check after host changes (latest sandbox smoke: 2026-07-18 pass).
- Broader HEIC sequence sample coverage (frame-decode re-encode path not implemented; ffmpeg demux may fail on unusual sequence brands).
- Broader automated coverage outside the plugin-host checks and current Rust unit tests.

**Known issues / active risks:**
- Packaged-plugin behavior must be checked in the release executable; dev-mode success alone does not prove installer/resource/runtime correctness.
- Release Rust builds can fail at local MSVC/CRT link time in native dependencies such as ONNX Runtime or LibRaw even when `cargo check` passes.
- AI plugin compatibility now enforces min/max PicAiPic versions and plugin API major; treat version-range changes as package compatibility changes.
- Protocol thumbnail/preview requests now resolve and cache against the library id encoded in the URL; future protocol work must preserve that isolation.
- pnpm is the sole JavaScript package manager, and host/frontend/docs versions are aligned at `1.1.0`.
- Historical docs, internal ABI/cache identifiers, and old source comments may still use `Lap`; user-visible active paths were corrected on 2026-07-10, while compatibility-sensitive internal identifiers are intentionally unchanged.
- ffprobe key name for `com.apple.quicktime.content.identifier` may vary across versions; the `first_exist` helper checks both dotted and underscored variants.

## Routing Table

| Task type | Load |
|-----------|------|
| Understand application flow or subsystem boundaries | `context/architecture.md` |
| Work with libraries, versions, native dependencies, or CI | `context/stack.md` |
| Write/review Rust, Vue, IPC, database, or UI code | `context/conventions.md` |
| Make or revisit a design choice | `context/decisions.md` |
| Set up, run, verify, or package | `context/setup.md` |
| Change AI plugin host, manifest, runtime, task, trust, or sandbox behavior | `context/plugin-runtime.md` |
| Change Live Photo / Motion Photo detection, pairing, or preview | `patterns/change-live-photo.md` |
| Perform any recurring task | `patterns/INDEX.md` |

## Behavioural Contract

1. **CONTEXT** — Read the routed context and matching pattern; use current code/docs as truth if memory conflicts.
2. **BUILD** — Keep changes focused and preserve the non-negotiables in `AGENTS.md`.
3. **VERIFY** — Run every applicable item in `context/conventions.md`; plugin changes also use `scripts/check_plugin_host.ps1`.
4. **DEBUG** — Use the matching debug pattern, reproduce at the narrowest boundary, then rerun verification.
5. **GROW** — Update current state/context/patterns, bump `last_updated`, and record material decisions, risks, or todos with `mex log`.
