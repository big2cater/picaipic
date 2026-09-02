---
name: change-ai-plugin
description: Add or modify an AI plugin, manifest field, host contract, runtime profile, capability, or package flow.
triggers:
  - add plugin
  - plugin capability
  - plugin manifest
  - runtime binding
  - plugin packaging
edges:
  - target: context/plugin-runtime.md
    condition: always before changing plugin behavior
  - target: context/conventions.md
    condition: when reviewing cross-layer contract and verification
  - target: patterns/debug-plugin-runtime.md
    condition: when the changed lifecycle or task flow fails
last_updated: 2026-08-29
---

# Change an AI Plugin or Host Contract

## Context
Read `context/plugin-runtime.md`, the v1 contract, author checklist, current-status doc, and one existing sample with the closest capability/runtime profile.

## Steps
1. Define the contract impact: manifest/schema, permission, profile/runtime binding, lifecycle, task protocol, output adoption, UI contribution, or packaging.
2. Update Rust host validation/state/commands first, preserving path containment, signature/trust, auth, permission, probe/conflict, and staging gates.
3. Update frontend API/store/settings/action UI and both locale files for every new field/state/action.
4. Update the plugin manifest, scripts/backend, health/status/task endpoints, cancellation, and host-injected environment handling.
5. Update contract/author/packaging/current-status documentation so plugin authors see the real rule.
6. Extend packaging validation and regression/stress coverage for the new invariant.
7. Package the plugin and, for release-sensitive changes, validate install/trust/setup/start/smoke/task/stop/uninstall in a packaged host.

## Gotchas
- Never default writable plugin storage under the executable directory. Windows per-machine installs may place it under `Program Files`; use app-local data while preserving an existing legacy install-adjacent store until the user migrates it.
- A valid installed manifest is not enough to expose contributed runtime menus. Gate actions on `reachable && managed`, and keep status synchronized between Settings and the main/viewer webviews.
- `entry.defaultPort` is not guaranteed; honor `PICAIPIC_PLUGIN_PORT` in start, backend, and stop cleanup.
- Stopping or uninstalling a plugin may only reap a port the host actually spawned. `kill_processes_listening_on_port` runs `taskkill /PID <pid> /T /F` against **every** PID listening there, so falling back to the manifest `defaultPort` for an unmanaged plugin kills unrelated user processes — e.g. uninstalling a plugin that was never started while the user runs something on that port. Gate port cleanup on a tracked runtime (`had_managed_runtime`); `wait_for_plugin_stopped` already probes only a managed `base_url`.
- Bearer auth goes through `resolve_plugin_auth_token`, which fails closed. A **managed** runtime always has a token the host generated, so a missing one means the tracked process exited (crash, reap) and the manifest port may now be answered by something else; the capability invoke, cancel, and task query are then refused instead of going out unauthenticated while still carrying staged input paths and the task payload. Externally managed services (a manifest `baseUrl` with no `startCommand`) have no host-issued token by design and remain the only path allowed to call without one.
- Plugin input staging materializes a **copy** unless the manifest declares `writeSourceFiles` (see `stage_one_file`). A hardlink shares the source inode, so an unconditional hardlink let a plugin that writes back to its input path silently rewrite the user's original photo.
- Release packages require canonical Ed25519 signing; do not test only with unsigned developer mode.
- Package install must operate on one host-managed snapshot/archive from manifest read through extraction. Reopening the user-selected path after signature verification reintroduces a TOCTOU window.
- Replace and uninstall are transactions, not direct deletes: preserve old code in a same-volume hidden backup until plugin registration commits; stage code/private storage on uninstall until registry cleanup persists. Do not scan hidden transaction directories as plugin candidates.
- Reject a package before extraction when its declared unpacked size exceeds the host budget; deletion after a runtime stop uses bounded retries for delayed Windows handle release.
- Runtime network permission lookup is fail-closed; registry errors must never become an implicit grant.
- Shared runtimes belong to multiple plugins and survive uninstall.
- A reachable service not tracked by the host is stale/external, not normally running.
- Capability inputs must use staged paths and outputs must stay in validated plugin/task roots until adoption.
- Changing an identifier or JSON field can break installed registry state and old packages.
- `scripts/check_plugin_host.ps1` validates manifests with **node**, not `ConvertFrom-Json`. Windows PowerShell 5.1 throws `ArgumentException` ("Invalid object passed in, ':' or '}' expected") on the valid SA-LUT/NAFNet manifests and has no `-Depth` parameter, which aborted the entire host check on Windows before any Rust/Python step ran.

## Verify
- [ ] `powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1`
- [ ] For task/protocol changes: add `-IncludeStress -FastStress`.
- [ ] `.\scripts\package_plugin.ps1 -All -FailOnWarnings`
- [ ] Signature/trust path tested without `PICAIPIC_ALLOW_UNSIGNED_PLUGINS` for release work.
- [ ] Start/stop/restart/smoke/cancel/retry/adopt/discard/uninstall paths that apply are exercised.
- [ ] English/Chinese UI and docs match implementation.

## Debug
Follow `debug-plugin-runtime.md`; separate discovery, trust, setup, runtime probe, process management, HTTP auth/health, task protocol, and output adoption instead of treating "plugin failed" as one boundary.

## Update Scaffold
- [ ] Update `context/plugin-runtime.md` and `ROUTER.md` current state.
- [ ] Record security/compatibility decisions in `context/decisions.md` and `mex log`.
- [ ] Update this pattern when a new recurring plugin gotcha is found.
