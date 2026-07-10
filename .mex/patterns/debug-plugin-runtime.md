---
name: debug-plugin-runtime
description: Diagnose AI plugin discovery, install, setup, process, health, task, cancellation, or output failures by boundary.
triggers:
  - plugin failed
  - smoke failed
  - plugin not running
  - runtime conflict
  - plugin task stuck
edges:
  - target: context/plugin-runtime.md
    condition: when checking expected lifecycle and invariants
  - target: patterns/change-ai-plugin.md
    condition: when the diagnosis requires a contract or implementation change
last_updated: 2026-07-10
---

# Debug the Plugin Runtime

## Context
Do not jump directly to model code. Identify the first failed boundary using host diagnostics, plugin logs, registry/profile state, and the exact command/task response.

## Steps
1. **Discovery/manifest:** validate JSON, compatibility, ids, paths, capabilities, profiles, and contributions.
2. **Package/trust:** distinguish bad signature, unknown publisher (`TRUST_REQUIRED`), unsigned release package, and developer bypass state.
3. **Permission/setup:** confirm grants, setup-download domains, selected profile, injected requirements/env path, and setup log/job state.
4. **Probe/conflict:** inspect Python path, package versions, GPU backend, stale probe state, and blocking runtime conflicts.
5. **Process ownership:** inspect start/stop logs, host-selected port, PID/managed state, stale listeners, and whether stop kills the correct process tree.
6. **HTTP boundary:** test loopback health/status with the bearer token; verify base URL and ready fields.
7. **Task protocol:** trace queued → running → terminal state, progress events/long polling, cancellation, retry, bounded history, and terminal error payload.
8. **Inputs/outputs:** confirm input paths were staged, rewritten files exist, output paths remain within task/plugin output roots, and adoption/discard uses the correct task id.
9. Reproduce with the relevant stress script or a minimal mock task before loading large models.

## Gotchas
- A port responding does not prove the service is the host-managed process.
- GPU imports can succeed while the selected profile requirements still conflict.
- Windows ROCm/PyTorch processes may hang during DLL detach; test scripts may intentionally terminate after a completion signal.
- Dismissing an OS prompt in opt-in ACL tests may not be the actual plugin failure; default mode should not apply deny ACLs.
- `.local.env` and copied development plugin directories can mask packaged-runtime behavior.

## Verify
- [ ] Capture diagnostics/log evidence for the first failing boundary.
- [ ] Run `.\scripts\check_plugin_host.ps1`.
- [ ] Run fast stress when the HTTP/task path is involved.
- [ ] Re-test without unsigned/sandbox-disable escapes before calling release behavior fixed.
- [ ] Stop the plugin and confirm the listener/process is gone.

## Debug
If the failure crosses layers, reduce to: manifest parse → host start → authenticated `/health` → mock task → real model task. Fix and verify one boundary at a time.

## Update Scaffold
- [ ] Add newly learned failure signatures and commands to this pattern.
- [ ] Log unresolved release/security risks with `mex log --type risk`.
