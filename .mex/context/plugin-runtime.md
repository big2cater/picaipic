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
last_updated: 2026-07-10
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

## Security Invariants

- Package JSON canonicalization must match between Python signing and Rust verification; keys are sorted and the signature field is omitted from signed bytes.
- Permissions distinguish reading selected files, writing output directories/source files, process launch, setup download, runtime network, and upload behavior.
- Backends bind to loopback, accept the host-selected port, and require `PICAIPIC_PLUGIN_AUTH_TOKEN` bearer authentication.
- Default confinement stages/copies external input paths into task storage. Do not pass raw source paths as a shortcut.
- Windows deny-ACL confinement is opt-in; stale deny ACEs are cleaned best-effort. Network blocking/Linux sandboxing are not implemented v1 guarantees.
- All delete/adopt/uninstall paths require canonical containment checks. `code_and_data` may delete plugin-private data/runtimes but must retain shared runtimes.

## Runtime Rules

- Profiles describe backend/support level, requirements, hardware, and binding. Sample runtimes cover CUDA, ROCm, CPU, and for SA-LUT DirectML.
- Runtime probes and requirement comparison detect missing/version-mismatched packages. Blocking conflicts prevent capability invocation and advise private runtime or setup repair.
- Do not auto-switch runtime scope without user confirmation. External runtimes must be probed and represented as external binding state.
- `entry.defaultPort` is preferred only; start/stop scripts and backend must honor `PICAIPIC_PLUGIN_PORT`.
- Long-running work uses async task states/events and cancellation; keep task history bounded and avoid blocking UI-only progress assumptions.

## Sources of Truth

- Host implementation: `src-tauri/src/t_plugin.rs`, `src-tauri/src/t_sandbox.rs`.
- Frontend: `src-vite/src/stores/pluginStore.js`, `src-vite/src/common/pluginRuntime.ts`, plugin sections/components in Settings and Content.
- Contract/docs: `docs/ai-plugin-contract-v1.md`, `docs/ai-plugin-author-checklist.md`, `docs/ai-plugin-packaging-v1.md`, `docs/ai-plugin-current-status.md`.
- Samples: `plugins/picai-salut-color`, `plugins/picai-nafnet-restore`.
- Verification: `scripts/check_plugin_host.ps1`, stress scripts, and `scripts/package_plugin.ps1`.
