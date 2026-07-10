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
last_updated: 2026-07-10
---

# Session Bootstrap

Read root `AGENTS.md`, then this file, then the routed context and matching pattern before changing code.

## Current Project State

**Working:**
- Tauri 2/Rust desktop host with Vue 3 frontend for Windows and Linux; first `v1.0.0` draft release pipeline has built Windows x64/arm64 and Linux x86_64/aarch64 artifacts.
- Folder-first multi-library browsing, SQLite metadata, indexing/recovery, thumbnails, timeline/folder/location/camera/lens/tag/favorite/rating/face filters, deduplication, image editing, and broad image/RAW/video support.
- Local AI search and face processing use bundled ONNX models; FFmpeg is bundled as a sidecar for video workflows.
- AI plugin host supports discovery, signed package install/trust, permissions, install profiles, shared/private/external Python runtimes, lifecycle control, async tasks, output adoption/discard, runtime-conflict detection, and two sample plugins.
- Plugin security A+B+C is present: startup bearer token, Ed25519 package signing/trust, and default input-file staging; Windows deny-ACL confinement remains explicit opt-in.
- GitHub Actions build documentation, app releases, plugin packages, and VitePress documentation.

**Not yet built / future work:**
- One-click confirmation-driven switch from a conflicting shared runtime to a plugin-private runtime.
- Complete model import and external model-directory binding UX across plugin workflows.
- Network confinement and Linux process sandboxing; strict write allow-listing and zero-copy large-video staging remain future security/performance work.
- Broader automated coverage outside the plugin-host checks and current Rust unit tests.

**Known issues / active risks:**
- Packaged-plugin behavior must be checked in the release executable; dev-mode success alone does not prove installer/resource/runtime correctness.
- Release Rust builds can fail at local MSVC/CRT link time in native dependencies such as ONNX Runtime or LibRaw even when `cargo check` passes.
- AI plugin compatibility now enforces min/max PicAiPic versions and plugin API major; treat version-range changes as package compatibility changes.
- Protocol thumbnail/preview requests now resolve and cache against the library id encoded in the URL; future protocol work must preserve that isolation.
- pnpm is the sole JavaScript package manager, and host/frontend/docs versions are aligned at `1.0.0`.
- Historical docs, internal ABI/cache identifiers, and old source comments may still use `Lap`; user-visible active paths were corrected on 2026-07-10, while compatibility-sensitive internal identifiers are intentionally unchanged.

## Routing Table

| Task type | Load |
|-----------|------|
| Understand application flow or subsystem boundaries | `context/architecture.md` |
| Work with libraries, versions, native dependencies, or CI | `context/stack.md` |
| Write/review Rust, Vue, IPC, database, or UI code | `context/conventions.md` |
| Make or revisit a design choice | `context/decisions.md` |
| Set up, run, verify, or package | `context/setup.md` |
| Change AI plugin host, manifest, runtime, task, trust, or sandbox behavior | `context/plugin-runtime.md` |
| Perform any recurring task | `patterns/INDEX.md` |

## Behavioural Contract

1. **CONTEXT** — Read the routed context and matching pattern; use current code/docs as truth if memory conflicts.
2. **BUILD** — Keep changes focused and preserve the non-negotiables in `AGENTS.md`.
3. **VERIFY** — Run every applicable item in `context/conventions.md`; plugin changes also use `scripts/check_plugin_host.ps1`.
4. **DEBUG** — Use the matching debug pattern, reproduce at the narrowest boundary, then rerun verification.
5. **GROW** — Update current state/context/patterns, bump `last_updated`, and record material decisions, risks, or todos with `mex log`.
