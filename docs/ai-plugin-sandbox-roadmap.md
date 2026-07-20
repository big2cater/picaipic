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
| **3** | Network OS block (WFP / AppContainer / etc.) | **Opt-in spike** (2026-07-20): flag + cooperative policy env; Windows `netsh` outbound program block when no runtime grant (soft-fail → policy_only); Linux policy_only |
| **4** | Linux Landlock / seccomp | **Opt-in spike** (2026-07-20): `PICAIPIC_ENABLE_LINUX_LANDLOCK=1` → ABI probe + path ruleset + `pre_exec` restrict_self; soft-fail if missing; seccomp still not done |
| **5** | Env hygiene / strip | **Opt-in allowlist** (2026-07-20): `PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1` → `env_clear` + keep PATH/GPU/`PICAIPIC_*`; default still inherits |

Do **not** describe Phase 0–2 as a complete OS sandbox. Do **not** claim network or Landlock enforcement until Phase 3–4 backends land behind explicit opt-in flags. Scaffold flags must keep default behavior identical to Phase 0–2.

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

### Phase 3 — Network confinement (opt-in spike)

**Status (2026-07-20):** **Partial opt-in.** Permission prompts + `allowedDomains` remain primary UX policy.

When `PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX=1` and the plugin has **no** stored `runtime_network` grant:

| Platform | Behavior |
|----------|----------|
| Windows | Best-effort `netsh advfirewall` **outbound block** for the plugin start program path; rule name `PicAiPicPluginNetDeny-<pluginId>`; removed on plugin stop (handle Drop). Soft-fail if admin/API fails → `policy_only`. |
| Linux / other | No OS block yet → `policy_only` |

Always inject `PICAIPIC_PLUGIN_NETWORK_POLICY` = `unrestricted` \| `allow` \| `deny` for cooperative plugin behavior. Loopback health/invoke expected to keep working under firewall program rules (verify on target OS).

**Not claimed:** full WFP per-PID filters, AppContainer, domain allowlists at OS level.

**Recommendation:** keep default **off**. Elevate only for explicit testing; do not advertise “kernel enforced network sandbox” in release notes until matrix-validated.

### Phase 4 — Linux process sandbox (opt-in Landlock spike)

**Status (2026-07-20):** **Partial opt-in on Linux.** Flag `PICAIPIC_ENABLE_LINUX_LANDLOCK=1`:

1. Probe Landlock ABI (`landlock_create_ruleset` version query).
2. Create ruleset; add **RW** path-beneath rules for `plugin_writable_roots`; **RO** for plugin code root + common system prefixes (`/usr`, `/lib*`, `/dev`, `/proc`, `/sys`, `/etc`, `/tmp`, CUDA/ROCm prefixes when present).
3. Child `pre_exec`: `PR_SET_NO_NEW_PRIVS` + `landlock_restrict_self`.
4. Soft-fail on missing kernel/ABI/rule errors → plugin still starts; start.log records `applied` or `soft_fail`.

Non-Linux: `unsupported_os`. Seccomp still not implemented.

**Goal:** Optional Landlock (filesystem) and/or seccomp-bpf for plugin children on Linux.

Constraints:

- Must allow read of shared runtime, models, staged inputs; write only to plugin roots.
- Must not break ROCm/CUDA device nodes (`/dev/dri`, `/dev/kfd`, etc.) — RO `/dev` is intentional for this spike.
- Prefer **opt-in** flag first (`PICAIPIC_ENABLE_LINUX_LANDLOCK=1`), same lesson as Windows ACL.

**Verify (Linux):** flag on → start.log `landlock: applied (...)`; write outside writable roots fails; GPU/venv still start. Flag off → `not_enforced`.

### Phase 5 — Env hygiene (opt-in allowlist)

**Status (2026-07-20):** Implemented behind `PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1`.
Default remains **full host env inheritance**.

**Behavior when enabled (start + setup spawns):**
1. `cmd.env_clear()`
2. Re-inject allowlisted host vars (`t_sandbox::env_hygiene_base_allowlist` + `PICAIPIC_*` / `CUDA_*` / `HIP_*` / `HSA_*` prefixes)
3. Then inject host `PICAIPIC_*` / setup profile env (must happen **after** clear)

**Allowlist inventory (kept):** `PATH`/`PATHEXT`, Windows system roots, locale, `VIRTUAL_ENV`/`PYTHON*`, CUDA/ROCm/HIP discovery, `LD_LIBRARY_PATH`.  
**Dropped examples:** `AWS_*`, `OPENAI_API_KEY`, `GITHUB_TOKEN`, arbitrary user shell tokens.

**Verify:** start plugin with flag on → start.log `env_hygiene: allowlist applied (...)`; GPU/venv plugins still start; secrets absent from child env. Flag off → `env_hygiene: inherit`.

## Explicit non-goals (do not mix into UI work)

- Settings toggles for experimental ACL/Landlock before Phase 0–1 are stable.
- Bundling a container runtime (Docker/Podman) for plugins.
- macOS Seatbelt until macOS is a release target again.
- Automatic deletion of shared runtimes after sandbox policy changes.

## Kill switches (must keep)

| Variable | Effect |
|----------|--------|
| `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` | Skip staging and optional ACL apply; still clean stale deny ACEs on Windows start path when ACL code runs. Marks Phase 3–5 scaffolds as not_enforced in start.log. |
| `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` | Windows only: enable deny-W on sensitive dirs. |
| `PICAIPIC_SANDBOX_DENY_PATHS` | Extra semicolon-separated deny targets for ACL mode. |
| `PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX=1` | Phase 3: when no runtime network grant, try Windows outbound firewall block on start program; always set `PICAIPIC_PLUGIN_NETWORK_POLICY`; soft-fail → policy_only. |
| `PICAIPIC_ENABLE_LINUX_LANDLOCK=1` | Phase 4: Linux Landlock FS ruleset on plugin start (soft-fail if ABI missing). |
| `PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1` | Phase 5: `env_clear` + allowlist rebuild on plugin start/setup. Default off (inherit). |

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
