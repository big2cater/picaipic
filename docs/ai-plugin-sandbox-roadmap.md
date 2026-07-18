# AI Plugin Sandbox Roadmap

Status: design document (2026-07-17).  
Scope: process / filesystem / network confinement for host-managed AI plugins.  
Non-goal: ship a full OS sandbox in the same change as Settings/plugin UI work.

## Current baseline (v1 partial)

| Layer | Default | Platform | Notes |
|-------|---------|----------|-------|
| Input staging | **On** | Windows + Linux | Host materializes external inputs into `plugin-cache/<id>/tasks/<taskId>/inputs/` (same-volume **hardlink**, else **copy**), rewrites JSON `path` fields, writes `staging-report.json`. Disable: `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1`. |
| Deny-ACL write confinement | **Off** (opt-in) | Windows only | `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` applies non-recursive deny-W on Desktop/Documents/Pictures/Videos (+ `PICAIPIC_SANDBOX_DENY_PATHS`). Drop/stop revokes ACEs. |
| Loopback bearer auth | On | All | `PICAIPIC_PLUGIN_AUTH_TOKEN` on plugin HTTP (except `/health`). |
| Package signing / publisher trust | On (release) | All | Ed25519; unsigned rejected in release builds. |
| Path containment on host ops | On | All | adopt/uninstall/import refuse paths outside plugin store roots. |
| Network block | Not implemented | — | Permission flags are policy, not kernel enforcement. |
| Linux process sandbox | Not implemented | — | No seccomp / landlock / namespace yet. |
| Env sanitization | Not implemented | — | Inherited env kept so venv/GPU tooling works. |

Hard product constraints:

1. **Local-first privacy** — no required cloud path for media.
2. **GPU/CPU must keep working** for PyTorch CUDA/ROCm/DirectML/CPU profiles.
3. **Shared runtimes** under `shared-runtimes/<id>` must remain readable/writable for setup/start.
4. **Do not mutate user library ACLs by default** — that caused host UI access prompts.
5. **No auto-destructive cleanup** of shared runtimes.


## Phase board (status as of 2026-07-18)

| Phase | Scope | Status |
|-------|--------|--------|
| **0** | Cross-platform default staging; fail-closed; diagnostics + unit tests | **Done** |
| **1** | Host write allow-list `plugin_writable_roots` for staging skip + ACL exclusions | **Done** |
| **2** | Same-volume hardlink staging, copy fallback; report hardlink/copy counts | **Done (mainline)** |
| **2 leftovers** | Cache ref/range; smarter cross-volume policy | Open (optional) |
| **3** | Network OS block (WFP / AppContainer / etc.) | **Not done** — research only |
| **4** | Linux Landlock / seccomp | **Not done** — research only |
| **5** | Env hygiene / strip | **Not done** — deferred |

Do **not** describe Phase 0–2 as a complete OS sandbox. Do **not** claim network or Landlock enforcement until Phase 3–4 land behind explicit opt-in flags.

## Threat model (what we defend vs what we don't)

**In scope for hardening:**

- A compromised or malicious plugin package that still passed (or bypassed) install trust.
- Accidental writes into user Desktop/Documents while debugging plugins.
- Exfiltration of arbitrary library files via reading raw source paths on disk.
- Confused deputy: host rewriting paths into unsafe locations during adopt/import.

**Out of scope for v1–v1.x OS sandbox claims:**

- Kernel-level isolation equivalent to a container or browser site isolation.
- Stopping a plugin that already holds GPU/driver handles from using the GPU.
- Perfect protection against a plugin that the user granted setup-download + runtime-network permissions and then runs arbitrary Python.
- macOS Seatbelt (macOS is not a current release target).

## Design principles

1. **Prefer host-side path control over OS ACL mutation.** Staging and containment are the default; ACLs stay opt-in tests.
2. **Layer defenses.** Signing → permissions → loopback auth → staging → optional OS confinement.
3. **Fail open only for non-security-critical diagnostics; fail closed for writes outside roots.** Staging failures surface as task errors (fail closed; Phase 0 landed).
4. **Keep GPU/runtime paths unrestricted.** Never deny `plugin-runtimes`, `shared-runtimes`, or driver device interfaces.
5. **Ship in phases with explicit verification.** Each phase has a kill switch and a regression script target.

## Phase plan

### Phase 0 — Correctness of existing default (near-term, low risk)

**Goal:** Make the documented default true on all release platforms and harden staging failure modes.

| Item | Action | Risk |
|------|--------|------|
| Linux/mac input staging | `sandbox_enabled()` is not Windows-gated (staging only). | Low |
| Staging failure | If `stage_one_file` fails, return error to invoke instead of silently keeping the external path. | Medium (behavior change for broken FS) |
| Staging coverage | Document that only JSON fields named `path` are rewritten; nested non-`path` file refs need contract update if plugins use them. | Doc |
| Diagnostics | Log staging summary (count staged, bytes, skipped inside-writable) into task message or start log. | Low |
| Verification | SA-LUT/NAFNet invoke with library image outside store; confirm staged path under `plugin-cache/.../inputs/`. Linux + Windows. | — |

**Non-goals this phase:** network block, landlock, env scrubbing UI.

### Phase 1 — Write allow-list policy (host enforcement, still no OS sandbox)

**Goal:** Explicit allow-list of writable roots per plugin process, enforced where the host already controls I/O; optional advisory check for plugin-declared paths.

- Formalize writable roots: `plugin-data`, `plugin-cache`, `plugin-outputs`, plugin code dir, `plugin-runtimes/<id>`, selected `shared-runtimes/<id>`, bound model dirs (read or read-write as declared).
- Keep adopt/discard/uninstall containment as the source of truth for host mutations.
- Do **not** default to recursive deny-ACL trees.

**Status (2026-07-17):** Host helper `plugin_writable_roots` is the single source of truth for start-time deny-ACL exclusions and invoke-time staging skip list. It always includes data/cache/outputs/plugin-runtimes/code root, plus manifest shared runtimes and persisted external model-dir bindings, plus call-site extras (task dir / task output). Output adoption still validates **only** the task output directory (stricter than the full allow-list).

### Phase 2 — Large input performance (zero-copy / hardlink where safe)

**Goal:** Reduce staging cost for large video / RAW without reopening arbitrary read.

Options (pick after spike):

1. **Hardlink** into staging when same volume and OS allows; fall back to copy.
2. **Ref + range** for Motion/Live extract paths already under app cache (already host-owned).
3. Keep full copy for cross-volume library folders.

Must preserve: plugin never receives a path outside allow-listed roots unless staging disabled for debug.

**Status (2026-07-17):** Option (1) landed in `stage_one_file`: try `fs::hard_link`, on any failure fall back to `fs::copy`. `InputStagingReport` records `hardlinkedFiles` / `copiedFiles` (and logical `stagedBytes`). Cross-volume still copies. Options (2)–(3) remain future optimizations.

### Phase 3 — Network confinement (high risk, separate design spike)

**Status (2026-07-18):** **Not implemented.** Permission prompts + `allowedDomains` packaging review only; no kernel/process network enforcement.

**Goal:** Enforce declared network permissions at OS/process level when possible.

Candidate mechanisms (Windows-first):

- Windows Filtering Platform (WFP) per-process rules (complex, admin rights questions).
- Restricted network capability / AppContainer (breaks many Python wheels / GPU).
- Userspace only: continue policy + no host proxy; accept that Python can still open sockets until OS layer exists.

**Recommendation:** treat full network block as **v2 research**. For v1.x, keep permission prompts + packaging `allowedDomains` review; do not claim kernel enforcement.

### Phase 4 — Linux process sandbox (high risk, separate design spike)

**Status (2026-07-18):** **Not implemented.** No Landlock/seccomp wiring in the host.

**Goal:** Optional Landlock (filesystem) and/or seccomp-bpf for plugin children on Linux.

Constraints:

- Must allow read of shared runtime, models, staged inputs; write only to plugin roots.
- Must not break ROCm/CUDA device nodes (`/dev/dri`, `/dev/kfd`, etc.).
- Prefer **opt-in** flag first (`PICAIPIC_ENABLE_LINUX_LANDLOCK=1`), same lesson as Windows ACL.

### Phase 5 — Env hygiene (deferred)

**Goal:** Reduce ambient credentials in plugin env without breaking venv.

- Allowlist host-injected `PICAIPIC_*` vars + computed `PATH` for selected runtime Python.
- Do not strip wholesale in v1; document residual risk.

## Explicit non-goals (do not mix into UI work)

- Settings toggles for experimental ACL/Landlock before Phase 0–1 are stable.
- Bundling a container runtime (Docker/Podman) for plugins.
- macOS Seatbelt until macOS is a release target again.
- Automatic deletion of shared runtimes after sandbox policy changes.

## Kill switches (must keep)

| Variable | Effect |
|----------|--------|
| `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` | Skip staging and optional ACL apply; still clean stale deny ACEs on Windows start path when ACL code runs. |
| `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` | Windows only: enable deny-W on sensitive dirs. |
| `PICAIPIC_SANDBOX_DENY_PATHS` | Extra semicolon-separated deny targets for ACL mode. |

Future opt-ins should follow the same pattern: default safe host-side control, experimental OS confinement behind explicit env flags.

## Verification matrix (per phase)

1. `scripts/check_plugin_host.ps1` (+ stress when task protocol changes).
2. Invoke SA-LUT/NAFNet on a library image **outside** the plugin store; assert staged input path.
3. Optional ACL mode (Windows): start log shows deny summary; plugin write to Desktop fails; GPU probe still works (`scripts/sandbox_gpu_spike.py` lineage).
4. Stop/restart/app exit: no residual deny ACEs on user dirs (Windows).
5. Linux release smoke: staging on by default; no ACL code path required.

## Decision summary

| Decision | Choice |
|----------|--------|
| Default confinement | Input staging on all supported platforms |
| OS write confinement | Windows deny-ACL **opt-in only** |
| Network OS block | Future / research (not v1 guarantee) |
| Linux Landlock/seccomp | Future opt-in spike (Phase 0–2 host path control done first) |
| Env strip | Deferred; inherit for GPU/venv |
| UI packaging | No sandbox settings panel; experimental OS modes stay env-flag only |

## Implementation status of this document's Phase 0

- [x] Document roadmap (this file).
- [x] Enable input staging on non-Windows by decoupling `sandbox_enabled()` from `cfg!(windows)`.
- [x] Fail closed when staging copy fails (invoke returns error instead of original path).
- [x] Staging diagnostics: task `message` summary + `inputs/staging-report.json`
      (`InputStagingReport`: staged_files, staged_bytes, hardlinked_files, copied_files, skipped_writable, skipped_missing).
- [x] Unit tests for rewrite/count, fail-closed, disabled message.
- [ ] End-to-end SA-LUT/NAFNet staged-path validation on Linux + Windows release builds
      (checklist: `docs/ai-plugin-sandbox-phase0-verify.md`).

## Related sources

- Implementation: `src-tauri/src/t_sandbox.rs`, `stage_input_files_for_sandbox` in `t_plugin.rs`.
- Contract: `docs/ai-plugin-contract-v1.md` (Process confinement).
- History: `docs/ai-plugin-security-hardening.md`.
- Product state: `.mex/context/plugin-runtime.md`, `.mex/ROUTER.md`.
