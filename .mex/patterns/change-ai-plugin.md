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
last_updated: 2026-07-10
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
- `entry.defaultPort` is not guaranteed; honor `PICAIPIC_PLUGIN_PORT` in start, backend, and stop cleanup.
- Release packages require canonical Ed25519 signing; do not test only with unsigned developer mode.
- Shared runtimes belong to multiple plugins and survive uninstall.
- A reachable service not tracked by the host is stale/external, not normally running.
- Capability inputs must use staged paths and outputs must stay in validated plugin/task roots until adoption.
- Changing an identifier or JSON field can break installed registry state and old packages.

## Verify
- [ ] `powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1`
- [ ] For task/protocol changes: add `-IncludeStress -FastStress`.
- [ ] `.\scripts\package_plugin.ps1 -All -FailOnWarnings`
- [ ] Signature/trust path tested without `PICAIPIC_ALLOW_UNSIGNED_PLUGINS` for release work.
- [ ] Start/stop/restart/smoke/cancel/retry/adopt/discard/uninstall paths that apply are exercised.
- [ ] English/Chinese UI and docs match implementation.

## Debug
Follow `debug-plugin-runtime.md`; separate discovery, trust, setup, runtime probe, process management, HTTP auth/health, task protocol, and output adoption instead of treating “plugin failed” as one boundary.

## Update Scaffold
- [ ] Update `context/plugin-runtime.md` and `ROUTER.md` current state.
- [ ] Record security/compatibility decisions in `context/decisions.md` and `mex log`.
- [ ] Update this pattern when a new recurring plugin gotcha is found.
