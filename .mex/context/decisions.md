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
last_updated: 2026-07-17
---

# Decisions

## Decision Log

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
