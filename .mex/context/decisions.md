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
last_updated: 2026-07-10
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
