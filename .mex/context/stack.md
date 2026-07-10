---
name: stack
description: PicAiPic technologies, important libraries, versions, and exclusions.
triggers:
  - dependency
  - library
  - version
  - framework
  - native build
edges:
  - target: context/architecture.md
    condition: when locating a technology in the application flow
  - target: context/conventions.md
    condition: when using the stack consistently
  - target: context/setup.md
    condition: when installing or troubleshooting toolchains
  - target: context/plugin-runtime.md
    condition: when Python, PyTorch, or plugin runtime dependencies are involved
last_updated: 2026-07-10
---

# Stack

## Core Technologies

- **Rust edition 2024 + Tauri 2** — native desktop host, filesystem/database/media/AI work, command IPC, packaging, and updater integration.
- **Vue 3.5 + Vite 8** — frontend SPA in `src-vite`; Vue code uses Composition API and `<script setup>`.
- **JavaScript/TypeScript 6** — most frontend application code is JavaScript, with shared typed helpers/composables in TypeScript.
- **Pinia 3 + persisted-state plugin** — frontend application and preference state.
- **SQLite via rusqlite 0.32.1 (bundled)** — one local metadata database per library.
- **Tailwind CSS 4 + daisyUI 5** — styling and component conventions.
- **Python plugin runtimes** — independent local AI plugin processes, typically Python 3.12/PyTorch profiles selected by manifest/runtime binding.

## Key Libraries

- **Tauri plugins** — dialog, filesystem, OS, process, shell, updater, and window-state integrations; use these rather than ad-hoc frontend OS access.
- **ONNX Runtime (`ort = 2.0.0-rc.10`) + ndarray/tokenizers** — bundled local CLIP and InsightFace inference.
- **LibRaw/libheif/libjpeg-turbo/jxl-oxide/Rust image** — native and Rust image decoding; do not replace casually because format coverage and release linking are sensitive.
- **FFmpeg/FFprobe sidecars** — video probing, compatibility conversion, and thumbnails; downloaded by project scripts and bundled as resources.
- **Leaflet + leaflet.heat** — map and GPS heatmap UI.
- **Video.js 8** — frontend video playback UI.
- **reqwest with rustls** — Rust HTTP without an OpenSSL runtime dependency.
- **Ed25519 (`ed25519-dalek`) and SHA-2** — plugin package verification; updater uses Tauri/minisign signing separately.
- **VitePress** — documentation site under `docs/`.

## What We Deliberately Do NOT Use

- No cloud-first media storage or mandatory remote AI service; photos and inference remain local by default.
- No ORM; persistence uses explicit `rusqlite` queries and versioned migration code.
- No direct database/filesystem access from Vue; privileged operations cross Tauri commands.
- No heavy frontend state framework beyond Pinia; preserve existing store/component organization.
- No macOS release pipeline in current scope.
- No unsigned plugins in release builds; `PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1` is a developer-only bypass.

## Version Constraints

- Development requires Node.js 20+, pnpm 9 (the sole committed JavaScript lockfile format), Rust stable, and Tauri CLI `^2.0.0`.
- Vite dev server is fixed to `127.0.0.1:3580`; Tauri `devUrl` depends on it.
- Cargo, Tauri, frontend, and docs package metadata are aligned at product version `1.0.0`.
- `ort` is pinned exactly to `2.0.0-rc.10`; native runtime changes can affect model downloads, copied DLLs, and release linking.
- CI uses pnpm 9 and builds Windows x64/arm64 plus Linux x86_64/aarch64.
