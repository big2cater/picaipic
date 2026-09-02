---
name: audit-plugin-trust-boundary
description: Code-audit findings for the AI plugin trust boundary in t_plugin.rs (signature, network permission fail-open, install TOCTOU). Read before changing install/verify/network-permission paths.
triggers:
  - plugin trust audit
  - package signature review
  - install TOCTOU
  - network permission fail-open
  - verify_package_signature change
edges:
  - target: context/plugin-runtime.md
    condition: when checking the expected trust-boundary contract
  - target: patterns/change-ai-plugin.md
    condition: when a finding requires a contract or implementation change
  - target: patterns/debug-plugin-runtime.md
    condition: when a trust failure manifests at install/runtime
last_updated: 2026-07-30
---

# Audit: AI plugin trust boundary (t_plugin.rs)

Static audit of `src-tauri/src/t_plugin.rs` covering the install/verify/network-permission
paths. Findings are severity-rated; verified-safe checks are listed so they are not
re-flagged in future audits.

Status (2026-07-30): P-1 was already fixed before this follow-up and now has regression
coverage. P-2 and P-3 are fixed and verified.

## Findings

| ID | Severity | Location | Summary |
|----|----------|----------|---------|
| P-1 | Resolved | `validate_package_manifest_file_list` | Leftover declared entries are rejected; missing-file regression coverage added. |
| P-2 | Resolved | `resolve_runtime_network_grant` | Registry read errors log and return `false`; network permission fails closed. |
| P-3 | Resolved | `install_ai_plugin_package` | The selected ZIP is copied to a UUID host-managed snapshot; manifest/hash/signature/trust checks and extraction reuse one archive opened from that snapshot. |

## P-1 — resolved before follow-up

Current code cross-checks ZIP entries against the manifest and rejects entries left in
`expected` after the scan. The original finding was stale by this follow-up.

Fix:

```rust
// after the for-loop over zip entries in validate_package_manifest_file_list
if !expected.is_empty() {
    return Err(format!(
        "Package manifest declares {} file(s) not present in zip",
        expected.len()
    ));
}
```

Regression coverage: `package_manifest_rejects_declared_missing_file`.

## P-2 — resolved: network permission fails closed

```rust
// ~L8617
let runtime_network_granted =
    plugin_runtime_network_granted(&plugin_id, &manifest).unwrap_or(true);
```

The start path routes the lookup through `resolve_runtime_network_grant`. Missing grants and
registry read failures both deny network; errors are logged without turning into an implicit
grant.

```rust
let runtime_network_granted = resolve_runtime_network_grant(
    plugin_runtime_network_granted(&plugin_id, &manifest),
    &plugin_id,
);
```

Regression coverage: `runtime_network_registry_error_fails_closed`.

## P-3 — resolved: source package is snapshotted once

`install_ai_plugin_package` opens the same `package_path` multiple times:

1. L6808 open to scan top-level dir, dropped at L6819.
2. L6832 `read_plugin_package_file` opens again to parse `manifest` / `package_manifest`.
3. L6873 `validate_package_manifest_file_list` opens again to cross-check entries.
4. L6878 `verify_package_signature` verifies using `package_manifest` from step 2.
5. L6903 `unpack_plugin_package` opens a **fourth** time to extract.

Between step 4 (verify) and step 5 (extract) an attacker can replace the file at
`package_path` with a structurally valid but unsigned/tampered package B. L6906
`validate_manifest` only checks manifest structure, so B is installed without re-verification.

The host creates `.package-snapshot-<uuid>.zip` under the managed plugin root with
`create_new`, copies and syncs the selected package, then opens that snapshot once. Package
reads, declared-file hash validation, signature/trust verification, and extraction reuse the
same archive. An RAII guard removes the snapshot on every return path.

Regression coverage: `package_snapshot_is_stable_when_source_is_replaced` replaces the
selected source ZIP after snapshot creation and confirms the archive still reads the signed
snapshot.

## Verified safe (do NOT re-flag)

- `verify_package_signature` (L6529) canonicalizes via `serde_json::Value` (BTreeMap sort),
  matching the Python signer's `json.dumps(sort_keys=True)`; checks 32B key / 64B signature
  and verifies with `ed25519_dalek`.
- `is_public_key_revoked` is checked before trust and is fail-closed (L6618).
- `NeedsTrust` result from `verify_package_signature` is intercepted and returns
  `Err("TRUST_REQUIRED:...")` at L6887, blocking install of untrusted packages outside dev mode.
- `zip_entry_normalized_path` (L6174) rejects any entry containing `..`, absolute paths, or
  drive letters; `unpack_plugin_package` additionally enforces
  `output_path.starts_with(destination)` (L6748). Zip-slip is prevented.
- `generate_plugin_auth_token` (L4777) uses `rand::thread_rng()` (CSPRNG) for 32 bytes.

## Verify

- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml` — 157 passed / 3 ignored
- `pnpm --dir src-vite build`
- `powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1`

## Update Scaffold

- [x] P-1/P-2/P-3 moved to resolved status and recorded in `change-ai-plugin.md`.
- [x] Runtime security invariants and router current state updated.
