---
name: add-tauri-command
description: Add or change a Vue-to-Rust operation without drifting the Tauri IPC contract.
triggers:
  - add command
  - Tauri invoke
  - frontend backend API
  - IPC
edges:
  - target: context/architecture.md
    condition: when locating the command in the application flow
  - target: context/conventions.md
    condition: when checking naming, errors, events, and verification
last_updated: 2026-07-10
---

# Add or Change a Tauri Command

## Context
Load `context/architecture.md` and `context/conventions.md`. Identify the owning Rust module; general library/file operations usually live in `t_cmds.rs`, while domain commands remain in their domain module.

## Steps
1. Define or update the `#[tauri::command]` Rust function with explicit serializable arguments/return value and contextual `Result` errors.
2. Register it in the `tauri::generate_handler!` list in `src-tauri/src/main.rs`.
3. Add or update the wrapper in `src-vite/src/common/api.js`; keep the invoked snake_case name and camelCase payload keys aligned with Tauri serialization.
4. If work is long-running, add managed cancellation/progress state and a stable event name rather than blocking the UI.
5. Call the wrapper from the appropriate store/view/component; do not scatter direct `invoke` calls when the facade already owns the domain.
6. Add both English and Chinese UI/error text when user-visible behavior changes.

## Gotchas
- A Rust function compiling does not mean it is callable; missing `generate_handler!` registration fails at runtime.
- Payload key drift is easy because Rust arguments are snake_case identifiers while JavaScript object keys are commonly camelCase.
- Decide deliberately whether a wrapper returns a safe fallback or rethrows. Mutations/setup actions generally must surface actionable errors.
- Component-scoped event listeners must release their unlisten callbacks.

## Verify
- [ ] Search confirms the command name appears in the Rust declaration, `main.rs`, and `api.js`.
- [ ] `pnpm --dir src-vite build`
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] Exercise success, error, progress, and cancellation paths that apply.

## Debug
For “command not found,” check registration first. For missing arguments, compare the `invoke` payload keys with the Rust parameter names. For stalled UI, inspect whether the Rust command blocks and whether events/cancellation are actually emitted.

## Update Scaffold
- [ ] Update `context/architecture.md` if a new subsystem boundary was introduced.
- [ ] Update `context/plugin-runtime.md` if the command changes the plugin contract.
- [ ] Record compatibility-sensitive IPC decisions with `mex log --type decision`.
