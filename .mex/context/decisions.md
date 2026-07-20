---
name: decisions
description: Active PicAiPic architectural decisions and their rationale.
triggers:
  - decision
  - rationale
  - why
  - alternative
  - historical choice
edges:
  - target: context/architecture.md
    condition: when a decision affects subsystem boundaries
  - target: context/stack.md
    condition: when a decision constrains technology choice
  - target: context/plugin-runtime.md
    condition: when a decision concerns plugin security or lifecycle
  - target: context/setup.md
    condition: when a decision affects release or platform workflow
last_updated: 2026-07-20
---

# Decisions

## Decision Log

### Gap triage 2026-07-20: cancel G1/G7/G8/G9; ship G2/G6/G10–G13; sandbox 3–5 opt-in only
**Date:** 2026-07-20  
**Status:** Active  
**Decision:** Do **not** implement G1 (collage-in-batch), G7 (`export-lut`), G8 (face GPU EP), or G9 (whole-library empty-comment backfill). Ship batch library import (G2), multi-key trust + local revoke (G6), FileInfo Live hover (G10), print magazine pack (G11), export-only DPI UI (G12), and system-print UX copy (G13) without host `print_file`. Sandbox Phase 3–5 remain **opt-in env flags**, default confinement stays Phase 0–2.  
**Reasoning:** Owner priority is product polish and trust/security foundation without expanding AI plugin business surface (LUT export / GPU face) or expensive full-library backfill. Print device selection stays in the OS dialog for local-first simplicity.  
**Alternatives considered:** Native printer/tray host API (rejected for v1); default-on Landlock/network sandbox (rejected until GPU matrix); whole-library prompt backfill (rejected for large-library cost).  
**Consequences:** Docs/ROUTER list cancelled IDs explicitly; scan-time empty-only prompt import remains the only path. See `docs/guide/目前的开发情况.md`, `patterns/change-print-layout.md`, `patterns/change-live-photo.md`.

### Import AI generation prompts into empty comments only (PNG + JPEG)
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** During library scan, import A1111/NovelAI/InvokeAI/ComfyUI prompts from PNG text chunks and JPEG UserComment/COM into `afiles.comments` **only when empty**. Default **on**. No full-library backfill of unchanged files in v1.
**Reasoning:** Matches lap #197 UX and PicAiPic’s AI-oriented library without overwriting user notes; scan-time is local-first and free of cloud upload.
**Alternatives considered:** Always overwriting comments (rejected: destroys user data); JPEG-only or PNG-only first (PNG first shipped, JPEG extended same day); mtime-unchanged rescan fill (rejected for large-library cost).
**Consequences:** `t_ai_prompt.rs` + Settings Library toggle; re-scan merge must preserve non-empty comments and use `update_column` because `AFile::update` omits comments. Optional backfill remains future work. See `patterns/change-ai-prompt-import.md`.

### Configurable thumbnail media badges default off
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** Media-info overlays (format/ISO/shutter/aperture/focal/exposure) are **opt-in** flags under `settings.grid.mediaBadges`, cap **4** badges per thumb.
**Reasoning:** Dense 10k–100k grids need low visual noise by default; power users can enable capture metadata like lap #174.
**Alternatives considered:** Always-on capture strip (rejected for clutter); reusing only bottom labels without overlays (incomplete vs upstream badge UX).
**Consequences:** Cross-window sync via settings events; older Pinia configs normalize missing `mediaBadges`. See `patterns/change-media-badges.md`.

### Viewer canvas background is app chrome, not global theme
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** Image/quick-viewer media area supports theme/black/white/gray/checker via `mediaViewer.backgroundMode`; cycle with **B**; does **not** change app-wide appearance theme.
**Reasoning:** Checking transparency and edge contrast needs a local canvas override (lap #173) without breaking daisyUI theme preference.
**Alternatives considered:** Only dark/light app theme (insufficient for checker/transparent review); per-window non-persisted mode (worse UX).
**Consequences:** Paint media area only; sync Settings/toolbar/shortcut. See `patterns/change-viewer-background.md`.

### AI search uses same file-type bitmask as library queries
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** `ImageSearchParams.search_file_type` reuses library mask (0 all / 1 image / 2 video / 4 raw) and filters vector-search SQL before cosine scoring; search results show Visual/Similar/Filename section headers.
**Reasoning:** Users expect the same Image/RAW/Video control in AI search as in albums; filtering candidates early avoids scoring irrelevant embeddings.
**Alternatives considered:** Frontend-only post-filter (wastes embedding work / wrong limit); separate AI-only type enum (duplicate UX).
**Consequences:** Toolbar filter enabled in search-like views; similar-from-file temp mode re-runs on filter change. See `patterns/change-ai-search-filters.md`.

### Ship built-in crop/collage/batch as host tools, not AI plugins
**Date:** 2026-07-18
**Status:** Active (Phase A/B/C1/C2 + print layout shipped; C3 optional)
**Decision:** Photo-size crop presets, collage/拼图, multi-image batch processing, and print layout are **host-built-in** features. Plan and phased scope live in `docs/guide/builtin-tools-roadmap.md` (A crop → B collage → C batch → print). Do not route v1 of these through the AI plugin runtime.
**Reasoning:** Geometry, export, and deterministic batch work fit the local editor/library host; plugins remain for heavy/model-specific AI. Reference UX is 光影魔术手-style, not full parity.
**Alternatives considered:** Implementing batch only as plugins was rejected for v1 (worse offline UX and trust surface for basic resize/crop). Full “magic hand” parity in one release was rejected as scope risk.
**Consequences:** Keep originals safe (explicit overwrite); share crop presets between editor and batch; C3 collage-as-batch-step remains optional.

### Prefer GitHub Release assets over Actions artifacts for installers
**Date:** 2026-07-18
**Status:** Active
**Decision:** Multi-arch app installers for tagged releases upload primarily to the **GitHub Release** for that tag. Actions `upload-artifact` for PR/release builds is **best-effort** (continue-on-error, short retention) and must not fail a successful compile/bundle.
**Reasoning:** Free-tier Actions artifact storage quota repeatedly failed CI after successful Tauri builds (`CreateArtifact: Artifact storage quota has been hit`).
**Alternatives considered:** Only deleting old artifacts manually was insufficient as a sole fix; keeping hard-fail upload re-blocked release drafts.
**Consequences:** `release.yml` / `release-windows.yml` / `pr-build.yml` follow this policy. `latest.json` is assembled from release assets. Operators may still prune stale Actions artifacts occasionally.

### Keep media local and use a folder-first workflow
**Date:** 2024-08-08
**Status:** Active
**Decision:** PicAiPic works directly with user-selected folders and performs search/AI locally without required upload.
**Reasoning:** Privacy, no library lock-in, offline usability, and suitability for large personal collections are core product promises.
**Alternatives considered:** Cloud-managed libraries and mandatory import into proprietary storage were rejected because they violate privacy and ownership goals.
**Consequences:** Original files remain external source data; uninstall/database cleanup must not delete them, and features must tolerate filesystem changes.

### Use Tauri/Rust for privileged work and Vue for UI
**Date:** 2024-08-08
**Status:** Active
**Decision:** Vue renders the desktop UI while Rust owns filesystem, database, decoding, AI, and process operations behind Tauri commands.
**Reasoning:** The split combines a productive UI stack with native performance, broad media integration, and a controlled privilege boundary.
**Alternatives considered:** A browser-only app cannot safely access local libraries/native codecs; putting privileged work in JavaScript would weaken control and performance.
**Consequences:** IPC contracts must stay synchronized, and long-running Rust operations need events/cancellation to keep the UI responsive.

### Store metadata in per-library SQLite databases
**Date:** 2026-01-15
**Status:** Active
**Decision:** Each configured library has a local SQLite database, with versioned migrations and optional custom database storage.
**Reasoning:** SQLite is embedded, offline, portable, and sufficient for large-library metadata without operating a service.
**Alternatives considered:** A server database adds deployment complexity; one global database increases coupling between independent libraries.
**Consequences:** Schema changes require forward migrations, storage moves require WAL checkpoint/copy safeguards, and backup/restore covers multiple library DBs.

### Run AI extensions as signed, permissioned local HTTP plugins
**Date:** 2026-07-04
**Status:** Active
**Decision:** AI plugins run as host-managed loopback processes with signed packages, publisher trust, explicit permissions, bearer-token authentication, runtime profiles, and staged inputs.
**Reasoning:** Independent Python/PyTorch stacks need isolation and lifecycle control without embedding every model dependency in the host.
**Alternatives considered:** In-process plugins risk dependency/ABI conflicts; unrestricted subprocesses lack a defensible trust boundary.
**Consequences:** Contract changes are cross-cutting; release builds reject unsigned packages, input paths are rewritten to staged copies, and runtime drift can block invocation.

### Make Windows deny-ACL plugin confinement opt-in
**Date:** 2026-07-10
**Status:** Active
**Decision:** Input staging is the default v1 confinement; the Windows `icacls` deny-write path is enabled only with `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1`.
**Reasoning:** Applying ACLs to real user directories caused disruptive prompts and confusing host behavior; staging provides a safer default without mutating user directory ACLs.
**Alternatives considered:** Default deny-ACL was implemented and tested but rejected as the normal path; network blocking and restricted-token approaches remain future work.
**Consequences:** Do not describe v1 as a complete OS sandbox. Preserve stale-ACL cleanup and the development disable switch.


### Treat sandbox hardening as a phased host design, not a UI bundle
**Date:** 2026-07-17
**Status:** Active
**Decision:** Further sandbox work follows `docs/ai-plugin-sandbox-roadmap.md`: Phase 0 correctness (cross-platform staging, fail-closed staging errors), then host write allow-list clarity, then optional large-file hardlink/zero-copy, with network OS block and Linux Landlock as separate opt-in research spikes. Do not ship experimental ACL/Landlock toggles in Settings until Phase 0–1 are verified.
**Reasoning:** OS confinement is high-risk, platform-specific, and GPU-sensitive; mixing it with product UI work caused scope and safety confusion. Host-side path control already provides the defensible default.
**Alternatives considered:** Default OS network/process isolation was deferred because it breaks common Python/GPU stacks and needs admin/capability spikes. Env scrubbing remains deferred to preserve venv activation.
**Consequences:** Product docs must not claim kernel network sandboxing. Linux release builds use the same input-staging default as Windows. Experimental OS modes stay behind explicit env flags.

### Support Windows and Linux releases, not macOS
**Date:** 2026-07-07
**Status:** Active
**Decision:** Current release scope is Windows x64/arm64 and Linux x86_64/aarch64.
**Reasoning:** The AI plugin confinement/runtime implementation is Windows-oriented and no macOS Seatbelt integration exists.
**Alternatives considered:** Keeping macOS packaging was rejected until plugin security and runtime support can meet the contract.
**Consequences:** CI/release docs must not claim macOS support; remaining conditional Rust branches are not proof of a supported target.

### Support both Apple Live Photo and Google Motion Photo with long-press preview
**Date:** 2026-07-13
**Status:** Active
**Decision:** PicAiPic simultaneously supports Apple Live Photo (HEIC/JPEG + MOV paired by ContentIdentifier UUID) and Google Motion Photo (JPEG with embedded MP4 offset in XMP `Container:Directory`). Paired MOV files remain visible as independent videos in the library but are linked to their companion image via `paired_file_id`. Users long-press an image in MediaViewer to play the paired video and release to return to the static image.
**Reasoning:** Live Photo and Motion Photo are the two dominant formats for hybrid still+motion captures; supporting both covers the majority of consumer device ecosystems (iPhone + Pixel/Samsung). Keeping the MOV visible as an independent video preserves user expectations that all imported files are browseable, while the link enables the Live Photo interaction. Long-press mirrors iOS native behavior and is the most intuitive gesture.
**Alternatives considered:** Integrating the external `live-photo-conv` Vala/GTK/GStreamer project was rejected because it only handles Android Motion Photo (not Apple), requires GStreamer + GObject dependencies incompatible with the Tauri stack, and duplicates capability the project already has (libheif + FFmpeg + EXIF). Hidden-then-linked MOV files were rejected as they break the expectation that all imported files are visible. Click-to-play button was rejected as less intuitive than long-press.
**Consequences:** DB schema is at v6 with `content_id`, `paired_file_id`, `live_photo_type` columns on `afiles`. `t_xmp.rs` module depends on `quick-xml`. HEIC container-internal video track extraction (some Apple Live Photos embed video in HEIC rather than separate MOV) is deferred to a future iteration. File-name stem pair fallback handles exported photos that lost ContentIdentifier metadata but requires same-folder + same-stem naming convention.

### Rollback disk renames when DB metadata updates fail
**Date:** 2026-07-17
**Status:** Active
**Decision:** After a successful filesystem rename (file or root folder), any subsequent SQLite metadata update failure must best-effort rename the path back to the original name before returning failure to the frontend. Partial multi-column DB writes (for example `name_pinyin` then `name`) also restore earlier columns when a later step fails.
**Reasoning:** The UI treats a failed rename as no-op, but an unreverted disk rename leaves `afiles.name` / virtual `file_path` pointing at a missing path and breaks open/thumbnail/reindex. `move_file` already rolled back disk on DB failure; rename must match that invariant.
**Alternatives considered:** Returning success after disk rename while logging DB failure was rejected because the list would show the old name and subsequent operations would target the wrong path. Leaving the new disk name and repairing only DB was rejected because the command already returned failure to the client.
**Consequences:** `rename_file` and `rename_folder` call `t_utils::rename_*` again with the original basename on DB error. If rollback itself fails, log a critical message; full two-phase rename transactions remain a future hardening option.
