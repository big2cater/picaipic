# PicAiPic AI Plugin Security Hardening - Design Doc

Date: 2026-07-03 (updated 2026-07-10)

Status: **A + B implemented. C partially implemented (v1: input staging by
default on Windows; experimental deny-ACL write confinement is explicit
opt-in; network blocking + cross-platform sandboxing = future).** See
"Implementation status (2026-07-10)" below.

This document records the current security posture of the PicAiPic AI plugin
host and proposes three hardening approaches. The goal is to decide which
direction to pursue before touching code, because these changes affect the v1
plugin contract freeze.

## Why this matters

PicAiPic plugins run as independent OS processes (local HTTP backends) spawned
by the host. Once a plugin is allowed to execute, it runs with the full
privileges of the PicAiPic process, which means the privileges of the logged-in
user. A malicious or tampered plugin can:

- read, write, or delete arbitrary files on the machine
- open arbitrary network connections (exfiltrate data, download payloads)
- bind to arbitrary ports and serve content independently of the host
- spawn additional processes

The current permission system in the manifest is a **consent and display
surface**, not a runtime enforcement layer.

## Current security posture

Investigated in `src-tauri/src/t_plugin.rs` (2026-07-03). Summary:

| Concern | Declared in manifest? | Enforced at runtime? |
|---|---|---|
| Network access (`permissions.network.*`) | Yes | **No** - no firewall, no socket interception |
| `allowedDomains` | Yes | **No** - never matched against traffic |
| `launchChildProcesses` | Yes | **No** - plugin can spawn freely |
| Filesystem confinement (`writeOutputDir` etc.) | Yes | **No** for runtime I/O; only the host's own delete/extract calls are path-guarded |
| Arbitrary file deletion by plugin | - | **Not prevented** |
| Port binding (`defaultPort`) | Yes | Host assigns a loopback port, but **does not prevent** the plugin binding elsewhere |
| Loopback-only base URL | Yes | **Validated** for the declared URL, but not enforced on plugin-initiated sockets |
| Process isolation | - | **None** (only `CREATE_NO_WINDOW` cosmetic flag) |
| Environment variable sandboxing | - | **None** - env vars are path hints; inherited env (`PATH`, `USERPROFILE`, ...) is not stripped |
| Package SHA-256 integrity | `AiPluginPackageFile.sha256` | **Yes** - per-file hash check on zip install |
| Package signature / trust | - | **None** - no signature, public key, or trust field exists |
| Command path escape (`..` / absolute) | - | **Yes** via `is_safe_relative_command` - guards which binary runs, not what it does |
| Zip-slip on install | - | **Yes** via `zip_entry_normalized_path` + `is_path_inside` |
| Output-path containment | - | **Yes, post-hoc** - `validate_plugin_output_paths` rejects outputs outside task dir, but only for declared result paths |

### What the host does protect

- **Install-time integrity**: zip entries are hash-checked against
  `picaipic.package.json`; zip-slip is blocked.
- **Host's own operations**: `remove_dir_all`, staging, and extraction are
  guarded by `is_path_inside` against the store root.
- **Declared output validation**: plugin task outputs must be inside the
  host-provided task output directory.
- **Command path safety**: `startCommand` / `stopCommand` / install commands
  must be relative and `..`-free.
- **Loopback base URL**: the declared `baseUrl` must be `127.0.0.1` or
  `localhost`.

### What the host does NOT protect

- Plugin runtime I/O (arbitrary file read/write/delete).
- Plugin network access (arbitrary outbound/inbound sockets).
- Plugin process spawning.
- Plugin binding to ports other than the assigned one.
- Package authenticity (SHA-256 proves integrity against transport corruption,
  not trust, because the manifest and hashes travel together in the same zip).

## Proposed approaches

Three approaches are on the table. They are complementary, not mutually
exclusive, but each has very different cost and impact.

### Approach A: Startup token + loopback binding enforcement

**Goal**: prevent a plugin backend from being used independently of the host.
A user (or malware) that discovers the plugin port cannot drive it by opening a
browser or curl.

**How it works**:

1. The host generates a cryptographically random token (e.g. 32 bytes, base64)
   per plugin start.
2. The token is injected as `PICAIPIC_PLUGIN_AUTH_TOKEN` into the plugin
   process environment.
3. The plugin backend reads the token on startup and requires it as a
   `Authorization: Bearer <token>` header (or a custom `X-Picaipic-Token`
   header) on every request.
4. The host sends this token on all `/health`, `/status`, `/invoke/*`,
   `/tasks/*` calls.
5. Requests without a matching token get `401 Unauthorized`.
6. The token is never written to disk; it lives only in the process
   environment and the host's in-memory runtime state.

**What it solves**:

- A stale or independently-running plugin backend cannot be driven by anyone
  who discovers the port.
- The host can distinguish its own calls from foreign requests.

**What it does NOT solve**:

- A plugin can still read/write/delete arbitrary files.
- A plugin can still open arbitrary network connections.
- A plugin can still spawn processes.
- The token is in the process environment, so a sufficiently privileged
  attacker on the same machine can read it. This is acceptable because the
  threat model is "casual browser access", not "local privilege escalation".

**Contract impact**:

- The v1 contract should add: "local HTTP plugins SHOULD accept an
  `Authorization: Bearer <token>` header from `PICAIPIC_PLUGIN_AUTH_TOKEN` and
  reject requests without it."
- Sample plugins (SA-LUT, NAFNet) need a small `main.py` change to read the
  token and add a FastAPI middleware/dependency.
- The host's HTTP client wrapper (used for all plugin calls) needs to attach
  the header automatically.

**Effort**: Medium. Host-side token generation + header injection + contract
note. Plugin-side middleware in both sample backends.

### Approach B: Plugin package signature verification

**Goal**: ensure only trusted (signed) plugin packages can be installed, so a
tampered package cannot get in even if someone replaces files inside a zip.

**How it works**:

1. Each plugin author generates their own Ed25519 keypair. The private key is
   held by the author; the **public key** travels inside the package as part
   of the `signature` object written into `picaipic.package.json` (see step
   3). `picaipic.plugin.json` carries only a `publisher` string (author
   name); the public key is not stored there.
2. The host maintains a **trust store** (in `plugin-registry.json` under the
   app data dir) of known publisher public keys. When a user installs a
   plugin from a new publisher for the first time, the host shows the
   publisher identity and public key and asks the user to trust it (one-time
   consent, similar to installing an app from an unknown developer).
3. At packaging time (`package_plugin.ps1 -SignKeyFile <key.txt>`), after
   `picaipic.package.json` is generated with file hashes, that file (with the
   `signature` field omitted) is canonicalized to compact, sorted-keys JSON
   and signed with the author's private key. The resulting `signature`
   object (`algorithm`, `publicKey`, `value`) is written back into
   `picaipic.package.json` itself — there is no separate signature file.
4. At install time, the host reads `picaipic.package.json`, re-serializes
   the manifest with `signature` set to `None` through `serde_json::Value`
   (which emits object keys in lexicographic order, matching the signer's
   `sort_keys=True`), verifies the Ed25519 signature against the public key
   embedded in the `signature` object, checks whether that key is in the
   user's trust store, then proceeds with the existing SHA-256 per-file
   checks.
5. If signature verification fails or the publisher key is not trusted,
   install is refused with a clear error.

**Developer mode**:

- The env var `PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1` enables developer mode,
  which allows installing **unsigned** plugins without signature
  verification. It is intentionally **not** exposed as a Settings UI toggle —
  giving end users a one-click "allow unsigned plugins" switch would defeat
  the signing model. The env var is aimed at plugin authors (local
  development) and CI (batch testing), not end users.
- Release builds intended for end users ship with developer mode off by
  default, and there is no in-app affordance to turn it on.

**What it solves**:

- Tamper detection that survives "attacker replaces both files and hashes",
  because the attacker cannot re-sign without the author's private key.
- A trust chain: only packages signed by a publisher the user has trusted can
  install.
- Per-author key ownership: authors are not dependent on the PicAiPic
  maintainer to publish signed plugins.

**What it does NOT solve**:

- A plugin that is signed but malicious is still fully trusted. Signing proves
  origin, not intent.
- The plugin still runs with full privileges after install (until approach C
  is implemented).

**Contract impact**:

- The Ed25519 public key and signature are embedded in
  `picaipic.package.json` under a `signature` object (`algorithm`,
  `publicKey`, `value`), covering the canonical (sorted-keys, compact) JSON
  of that same file with the `signature` field omitted. There is no separate
  signature file.
- `picaipic.plugin.json` carries a `publisher` string (author name); the
  public key is **not** stored there.
- The v1 contract should document the signing requirement, the trust store
  consent flow, and the developer mode env-var bypass.
- `package_plugin.ps1` takes a `-SignKeyFile <path>` option pointing at a
  text file whose first line is the base64 Ed25519 private key.
- The host needs trust store management (add/remove/list trusted publishers)
  and a first-install consent UI.

**Effort**: Medium-high. Keypair management, signing tooling, host-side
verification, developer bypass UX, key rotation story.

### Approach C: Process sandboxing (Windows Job Object + restricted token)

**Goal**: confine plugin processes so they can only access their designated
directories, the assigned loopback port, and nothing else.

**How it works**:

1. The host spawns each plugin process inside a Windows Job Object with:
   - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` so the plugin dies when the host
     exits (already desired, currently not enforced).
   - Restrict UI / clipboard access.
2. The host creates a restricted process token that:
   - Preserves **full GPU and CPU access** - ROCm/CUDA/DirectML device files
     and driver APIs must remain reachable. This is a hard requirement; the
     sandbox must not break AI inference.
   - **Write confinement**: the plugin may only **write** to
     `plugin-data/<id>`, `plugin-cache/<id>`, `plugin-outputs/<id>`, the
     plugin code directory, and `shared-runtimes/<id>` /
     `plugin-runtimes/<id>` (read+write for venv management during setup).
     Writes to any other path are denied.
   - **Read authorization**: the plugin may **read** the directories above
     plus any file the host explicitly authorizes for the current task. When
     the user selects a source/style image from anywhere on the machine
     (e.g. SA-LUT color transfer), the host injects that file path as an
     authorized read target. The plugin can read it, but cannot scan the
     rest of the disk.
   - Blocks network access (either via restricted token + no-network SID, or
     via WFP filter on the process).
3. The plugin process inherits this restricted token and cannot escape.

**What it solves**:

- The strongest runtime confinement: a plugin literally cannot touch files or
  network outside its allowance, regardless of what its code does.
- This is the only approach that addresses "a plugin can delete arbitrary
  files" and "a plugin can exfiltrate data" at runtime.

**What it does NOT solve (easily)**:

- Cross-platform: Job Objects and restricted tokens are Windows-specific.
  macOS would need `sandbox-exec` / Seatbelt; Linux would need seccomp /
  namespaces. This is a large portability burden.
- Shared runtimes: a plugin running in a sandbox needs access to
  `shared-runtimes/<id>`, which is outside `plugin-data/<id>`. The ACL allow
  list must include it, which widens the surface.
- GPU access: ROCm/CUDA/DirectML must remain fully accessible. This is a hard
  requirement, not a "nice to have". The sandbox design must preserve device
  file and driver API access. This rules out the simplest restricted-token
  approaches (e.g. AppContainer low-integrity) and likely requires a more
  nuanced ACL approach that denies filesystem/network while keeping GPU
  device handles open. A focused spike is still needed to confirm the exact
  mechanism on Windows.
- Debugging: a sandboxed plugin is much harder to troubleshoot.

**Contract impact**:

- Minimal contract change (the plugin does not need to know it is sandboxed),
  but the host's spawn path changes significantly.
- The v1 contract should document that "the host MAY confine plugin processes
  to their declared directories and the assigned port" so authors are not
  surprised.

**Effort**: High. Win32 Job Object + restricted token APIs from Rust
(`windows` crate), ACL construction, GPU access investigation, cross-platform
strategy, extensive testing. This is a multi-week effort.

## Comparison

| | A: Startup token | B: Package signing | C: Process sandbox |
|---|---|---|---|
| Prevents independent use of plugin port | **Yes** | No | Yes (indirectly) |
| Prevents file system abuse | No | No | **Yes** |
| Prevents network exfiltration | No | No | **Yes** |
| Prevents tampered package install | No | **Yes** | No |
| Prevents unsigned plugin install | No | **Yes** | No |
| Breaks GPU inference risk | None | None | **High** |
| Cross-platform effort | Low (just env var + HTTP header) | Low (crypto is portable) | **High** (OS-specific) |
| Contract impact | Small (add token header) | Small (add signature field) | Minimal (host-side only) |
| Effort | Medium | Medium-high | High |

## What "packaging plugins as exe" would and would not solve

The question of packaging plugins as standalone executables (e.g. via
PyInstaller) was considered and rejected:

- **Code protection**: PyInstaller bundles bytecode, which is trivially
  decompilable. It does not meaningfully protect Python source. PyTorch model
  weights cannot be hidden regardless.
- **Independence from host**: an exe can still be run independently; it does
  not inherit any trust relationship with the host.
- **Sandboxing**: an exe runs with the same privileges as a `.bat`-launched
  Python process. No isolation is gained.
- **Ecosystem friction**: forcing plugin authors to build exes adds a heavy
  packaging step, breaks source-level debugging, and bloats package size
  (PyTorch + CUDA wheels inside an exe are enormous).
- **The real problems** (independent port access, tamper detection, runtime
  confinement) are better solved by approaches A, B, and C above, which keep
  plugins as readable Python + `.bat`/`.py` launchers.

## Recommendation

1. **Start with A (startup token)** - it is the best cost/benefit ratio and
   directly addresses the "cannot run independently of host" requirement. It
   also lays groundwork for B (the host's HTTP client wrapper is the right
   place to enforce auth).
2. **Then B (package signing)** - adds tamper resistance and a per-author
   trust chain. Each plugin author holds their own keypair; the host keeps a
   user-managed trust store; developer mode allows unsigned plugins for local
   development.
3. **Then C (process sandbox)** - the v1 contract freeze waits for all three.
   The sandbox must preserve full GPU/CPU access, confine writes to
   host-designated directories, and allow reads to host-authorized files
   (e.g. user-selected source/style images) without granting blanket disk
   read. A GPU-access spike should run before full implementation to confirm
   the Windows mechanism.

## Decisions resolved (2026-07-03)

1. **Signing keys**: each plugin author holds their own Ed25519 keypair. The
   host maintains a user-managed trust store of publisher public keys, not a
   single embedded key. (See approach B above.)
2. **Developer mode**: yes, unsigned plugins are allowed during local
   development, gated by the `PICAIPIC_ALLOW_UNSIGNED_PLUGINS=1` env var only
   (no Settings UI — intentionally, so end users cannot trivially disable
   the signing model). Release builds default to unsigned-not-allowed.
3. **Sandbox file access model**: "read authorized files + write confined
   directories". The plugin may write only to `plugin-data/<id>`,
   `plugin-cache/<id>`, `plugin-outputs/<id>`, plugin code dir, and
   runtime dirs. It may read those plus any file the host explicitly
   authorizes for the current task (e.g. the source/style image the user
   selected in PicAiPic). The plugin cannot scan the rest of the disk.
4. **Sandbox GPU/CPU**: full access is a hard requirement. The sandbox must
   not break ROCm/CUDA/DirectML/CPU inference. The ACL/token design must
   preserve device file and driver API access.
5. **v1 contract freeze**: waits for A + B + C to all be implemented. The
   contract is not frozen until the security model is in place.

## Open questions

1. ~~For approach C, what is the exact Windows mechanism that denies
   filesystem/network while preserving GPU device access?~~ **UPDATED
   (2026-07-10)**: the practical v1 default is input staging, not global
   directory ACL mutation. The deny-ACL path (`icacls /deny <user>:(W) /L`)
   remains available behind `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` for
   explicit testing, and the spike confirmed it does **not** break ROCm/CUDA
   driver initialization. It is not default because it changes real user
   directory ACLs while a plugin is running and can interfere with host UI
   file/directory access prompts. v1 does not block network; that remains
   future work.
2. ~~For approach C, how does the host inject "authorized read" file paths
   into the sandbox at runtime?~~ **RESOLVED (2026-07-04)**: option (c) —
   pre-copy input files into `plugin-cache/<id>/tasks/<taskId>/inputs/`
   and rewrite the `path` fields in the invoke payload to point at the
   staged copies. Chosen for simplicity (no ACE manipulation, no broker
   protocol change). Perf cost is acceptable for typical photos; large
   videos are a future optimization.
3. For approach B, what is the key rotation story if an author's private key
   is compromised?
4. ~~For approach B, does the trust store live in the plugin registry JSON,
   or in a separate OS keychain/credential store?~~ **RESOLVED
   (2026-07-04)**: the trust store lives in the plugin registry JSON
   (`plugin-registry.json` under the app data dir) as a
   `trusted_publishers` map keyed by publisher name. OS keychain integration
   is deferred (the JSON file is adequate for v1; a single-user, single-file
   trust store does not warrant the complexity of per-platform credential
   store bindings).
5. Should the host strip inherited environment variables (`PATH`,
   `USERPROFILE`, etc.) from plugin processes in approach C, or construct a
   minimal env from scratch (which would break venv activation)? **v1
   decision: leave env inherited** — stripping `PATH`/`USERPROFILE` breaks
   venv activation and runtime probing. Input staging confines normal task
   source-file access; env sanitization is deferred.

## Implementation status (2026-07-10)

- **A (startup token)**: implemented — `PICAIPIC_PLUGIN_AUTH_TOKEN` bearer
  auth on all plugin endpoints except `/health`.
- **B (package signing)**: implemented — Ed25519 signing tool
  (`scripts/sign_plugin.py`), host verification at install, user-managed
  trust store, `TRUST_REQUIRED` consent flow, developer-mode bypass.
- **C (process confinement)**: **v1 partial** — input file staging into the
  plugin task directory before invoke is the default path. The prior Windows
  deny-ACL write confinement code remains in `src-tauri/src/t_sandbox.rs`,
  but is explicit opt-in via `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1`; default
  startup best-effort removes stale deny ACEs from older builds. Network
  blocking, macOS Seatbelt, and Linux seccomp are future work. GPU access is
  preserved.

The v1 contract freeze requires A + B + C validated with SA-LUT and
NAFNet. A and B are validated; C's practical v1 scope (input staging, with
optional deny-ACL testing) is implemented and pending end-to-end validation.

## Implementation order

Once the design is approved:

1. Implement A (startup token) - host token generation, HTTP header injection,
   plugin-side middleware in both sample backends, contract note.
2. Implement B (package signing) - Ed25519 keypair generation tooling,
   `package_plugin.ps1 -SignKeyFile`, `signature` object embedded in
   `picaipic.package.json`, host trust store + install-time verification +
   first-install consent UI + developer mode env-var bypass.
3. Spike C's GPU access question on Windows (restricted token vs deny-ACL
   vs broker), then implement full sandbox with the read-authorized /
   write-confined model.
4. Freeze v1 contract after all three are validated with SA-LUT and NAFNet.
