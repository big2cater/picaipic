---
name: plugin-runtime
description: AI plugin host contract, lifecycle, storage, security, runtime profiles, and verification.
triggers:
  - plugin
  - manifest
  - runtime profile
  - sandbox
  - signed package
  - plugin task
edges:
  - target: context/architecture.md
    condition: when tracing host/frontend/plugin data flow
  - target: context/conventions.md
    condition: when implementing or reviewing contract changes
  - target: patterns/change-ai-plugin.md
    condition: when adding or changing plugin behavior
  - target: patterns/debug-plugin-runtime.md
    condition: when setup, start, health, smoke, task, or output behavior fails
last_updated: 2026-07-18
---


# AI Plugin Runtime

## Contract and Flow

1. The host discovers a plugin from the managed store or registered development path and validates `picaipic.plugin.json` schema, compatibility, permissions, entry, profiles, capabilities, and contributions.
   Compatibility validation enforces `minPicAiPicVersion`, optional `maxPicAiPicVersion`, and the plugin API major version.
2. Packaged installs verify the Ed25519 signature and publisher trust. Release builds reject unsigned packages; developer bypass is explicit.
3. The user grants declared permissions and chooses/probes an install profile. Runtime bindings may be shared, plugin-private, or external.
4. The host injects plugin/profile/runtime/storage variables, selects a free loopback port, creates a bearer token, applies default input staging policy, and starts the process.
5. Health/status prove reachability; `reachable && managed` is the definition of a normally running plugin. A reachable untracked service is stale/external, not healthy managed state.
6. Capability invocation creates a task, stages input files under the task cache, rewrites payload paths, calls the local HTTP API, tracks progress/events/cancellation, and records outputs.
7. Outputs remain in plugin output/task storage until the user adopts or discards them. Adoption must validate paths and integrate through host-controlled file/library operations.

## Storage Boundaries

- `plugins/<id>` — installed code.
- `plugin-data/<id>` — persistent plugin settings/data.
- `plugin-cache/<id>` — disposable cache and task staging.
- `plugin-outputs/<id>` — generated outputs awaiting host/user handling.
- `plugin-runtimes/<id>` — plugin-private runtimes.
- `shared-runtimes/<runtime-id>` — shared environments; never delete during one plugin's uninstall.
- Model roots and external bindings are separate persistent resources and require explicit binding/validation.
- Model UX (2026-07-17): `list_ai_plugins` exposes `modelFiles` presence under the managed model dir. Settings can open+validate that dir (`check_ai_plugin_model_files`) and import selected files by basename into `plugin-data/<id>/models` (`import_ai_plugin_model_files`, path-containment enforced). External `modelBindings` still support bind/open+validate/clear.

## Security Invariants

- Package JSON canonicalization must match between Python signing and Rust verification; keys are sorted and the signature field is omitted from signed bytes.
- Permissions distinguish reading selected files, writing output directories/source files, process launch, setup download, runtime network, and upload behavior.
- Backends bind to loopback, accept the host-selected port, and require `PICAIPIC_PLUGIN_AUTH_TOKEN` bearer authentication.
- Default confinement stages/copies external input paths into task storage on all supported platforms (`sandbox_enabled()` is not Windows-gated). Staging failures fail closed. Diagnostics: task message + `inputs/staging-report.json` (counts/bytes/hardlink vs copy/skips). Staging prefers same-volume hardlink, then copy. Only JSON fields named `path` are rewritten. Do not pass raw source paths as a shortcut.
- Writable roots allow-list (Phase 1): `plugin_writable_roots` centralizes data/cache/outputs/plugin-runtimes/code + shared runtimes + bound model dirs + task extras; used for staging skip + optional ACL exclusions. Adoption still requires task-output containment only.
- Windows deny-ACL confinement is opt-in; stale deny ACEs are cleaned best-effort.
- Sandbox roadmap status: **Phase 0–2 done** (cross-platform staging, fail-closed, diagnostics, `plugin_writable_roots`, same-volume hardlink→copy). **Phase 3 network OS block and Phase 4 Linux Landlock are not implemented** (v1 does not claim them). Roadmap: `docs/ai-plugin-sandbox-roadmap.md`.
- Phase 0 verification (2026-07-18): automated `input_staging*` + plugin-host script; Windows cross-volume host-path proof; SA-LUT full start+color-transfer on staged album paths (ROCm, PNG output); user-confirmed GUI/release-shell smoke pass. Checklist: `docs/ai-plugin-sandbox-phase0-verify.md`.
- All delete/adopt/uninstall paths require canonical containment checks. `code_and_data` may delete plugin-private data/runtimes but must retain shared runtimes.

## Runtime Rules

- Profiles describe backend/support level, requirements, hardware, and binding. Sample runtimes cover CUDA, ROCm, CPU, and for SA-LUT DirectML.
- Runtime probes and requirement comparison detect missing/version-mismatched packages. Blocking conflicts prevent capability invocation and advise private runtime or setup repair.
- Confirmed shared→plugin-private switch (2026-07-17): Settings shows **Use private runtime** on blocking conflicts; host command `switch_ai_plugin_profile_to_private_runtime` persists a synthetic `scope: "plugin"` binding (`plugin-private:<profileId>`), clears that profile's probe cache, and marks `needsVerify`. Shared runtimes are never modified. Do not switch without user confirmation, and do not auto-run Setup after the switch.
- External runtimes must be probed and represented as external binding state.
- `entry.defaultPort` is preferred only; start/stop scripts and backend must honor `PICAIPIC_PLUGIN_PORT`.
- Long-running work uses async task states/events and cancellation; keep task history bounded and avoid blocking UI-only progress assumptions.

## Sources of Truth

- Host implementation: `src-tauri/src/t_plugin.rs`, `src-tauri/src/t_sandbox.rs`.
- Frontend: `src-vite/src/stores/pluginStore.js`, `src-vite/src/common/pluginRuntime.ts`, plugin sections/components in Settings and Content.
- Contract/docs: `docs/ai-plugin-contract-v1.md`, `docs/ai-plugin-author-checklist.md`, `docs/ai-plugin-packaging-v1.md`, `docs/ai-plugin-current-status.md`.
- Samples: `plugins/picai-salut-color`, `plugins/picai-nafnet-restore`.
- Verification: `scripts/check_plugin_host.ps1`, stress scripts, `scripts/package_plugin.ps1`, and `docs/ai-plugin-sandbox-phase0-verify.md` (staging path acceptance).
