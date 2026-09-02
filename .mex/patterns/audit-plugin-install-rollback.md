# Audit: AI Plugin Install / Uninstall / Runtime Rollback (t_plugin.rs)

- Scope: `src-tauri/src/t_plugin.rs` — package install (`install_ai_plugin_package` 6785),
  uninstall (`uninstall_ai_plugin` 7008), runtime stop (`stop_ai_plugin_runtime` 8909),
  task temp-dir lifecycle (`cleanup_*`). Trust-boundary checks (zip-slip, signature,
  input-staging) were covered by the earlier P-1/P-2/P-3 audit; this runbook focuses on
  execution safety and rollback consistency, i.e. what happens when an install/uninstall
  step *fails* midway.
- Severity legend: High / Medium / Low / Design-as-intended.
- Date: 2026-07-30. Findings re-verified and high-priority consistency fixes shipped.

## Trust-boundary controls confirmed SAFE (carried from P-series, re-verified here)
- Zip-slip: `unpack_plugin_package` (6746-6753) normalizes each entry and rejects any
  `output_path` that is not `starts_with(destination)`. Path traversal from a malicious
  package is blocked at unpack time.
- Integrity: `validate_package_manifest_file_list` (6643) verifies sha256 + size of every
  file and **rejects** files not declared in the manifest (6700) and **rejects** declared
  files missing from the package (6720). Strong tamper detection.
- Signature + trust: `verify_package_signature` + trust store; `TRUST_REQUIRED` early-return
  (6887-6892) happens **before** any staging dir is created, so no leftover staging on trust
  failure.
- Path containment on destructive ops: install-replace (6934, 6941), uninstall (7036), and
  `purge_data` extras (7077 `remove_existing_dir(&dir, &store)`) all use `is_path_inside`
  against the plugin root / store. Out-of-tree deletion is refused.
- Runtime stop awaits: `stop_ai_plugin_runtime` calls `wait_for_plugin_stopped` (8968);
  uninstall `await`s it (7052) before `remove_dir_all`, so the disk delete generally runs
  after the process has released its file handles.

## Findings

### FE-PLUG-1 — Concurrent package installs can overwrite registry updates — FIXED
The Settings window already prevented repeated clicks in one component instance, but the Rust
command had no process-wide package-mutation gate. Two direct/concurrent IPC installs could both
complete filesystem staging and then persist registry snapshots independently, allowing the later
save to drop the other install's registered path. Install and uninstall now acquire a fail-fast
RAII `PluginPackageMutationGuard` for the full filesystem + registry transaction. This serializes
install/install and install/uninstall operations across windows and direct IPC callers; trust-required
returns release the guard before the explicit trust-and-retry flow.

### PLUG-1 — Install does NON-ATOMIC in-place replace: old plugin lost if rename fails (Medium→High) — FIXED
`install_ai_plugin_package` order (6902-6959):
1. `6941` If `destination.exists()`, `fs::remove_dir_all(&destination)` deletes the **old**
   plugin **first**.
2. `6951-6957` `fs::rename(&staged_destination, &destination)`; on `Err` it deletes the
   staging root (`6952`) and returns `Err`.

Consequence: if the final `rename` fails, the old plugin is already gone (step 1) **and** the
new staged copy is deleted (6952). The user ends up with **neither** version installed.
In the normal same-volume case `rename` rarely fails, but the operation is not atomic and
offers no backup to roll back to. A cross-device/permission hiccup during rename = permanent
loss of the installed plugin.

Implemented: `replace_plugin_directory` now stages the previous directory as a same-parent
`.replacing-<id>-<uuid>` backup, moves the staged directory into place, and restores the backup
if that move fails. The backup is removed only after storage preparation and registry persistence
commit. Interrupted hidden transaction directories are ignored by normal plugin discovery.

### PLUG-2 — prepare/register failure after rename leaves a half-installed orphan dir (Medium) — FIXED
After the successful rename (6959), `staging_root` is already removed. Then:
- `6961` `prepare_installed_plugin_storage(&installed_manifest)?`
- `6965` `register_ai_plugin_path(normalize_path(&destination))?`

If either returns `Err`, the function returns immediately **without deleting the now-installed
`destination` directory**. Result: a plugin directory exists on disk but is **not registered**
in the registry → orphan that survives restart. No rollback deletes `destination`.

Implemented: failed storage preparation, model-summary generation, or registry persistence removes
the newly installed code and only the private storage roots created by this install, then restores
the prior plugin backup. Existing private data remains untouched during an upgrade rollback.

### PLUG-3 — Unpack writes to staging with no size/disk-space ceiling (Low) — FIXED
`unpack_plugin_package` streams each entry with `io::copy` (or equivalent) with no
per-file size cap and no free-space pre-check. A package declaring a huge/loop file could
fill the plugin volume. Mitigated because it is confined to the staging dir (deleted on any
error), but a disk-full mid-unpack yields a confusing failure rather than a clean refuse.

Implemented: package manifests are rejected before unpack when one declared payload exceeds 2 GiB
or the declared unpacked total exceeds 4 GiB. The manifest-size and ZIP-entry-size equality check
ensures this budget covers the bytes extraction would write.

### PLUG-4 — Uninstall order "delete disk first, save registry last" (Medium→Low) — FIXED
`uninstall_ai_plugin` (7008):
1. `7054` `fs::remove_dir_all(&target)` — disk gone.
2. `7086` `save_registry(&registry)` — registry update happens **after**.

If `save_registry` fails (IO error, interrupted), the disk directory is already deleted but
the registry still lists the plugin → stale registry entry; next launch tries to load a
missing plugin. Lower blast radius than PLUG-1 (registry can be repaired) but still an
inconsistent state. Also note `uninstall` is permanent (no trash) — acceptable for an
explicit "uninstall" action, but it should at least be registry-last-or-atomic.

Implemented: uninstall stops runtime, stages code plus optional plugin-private data to hidden
same-parent `.uninstalling-<id>-<uuid>` paths, and writes the registry before final deletion. A
registry-save or data-staging failure restores every staged path. Registry writes now use a synced
temp file plus previous-file backup, so a failed replacement keeps the former valid registry.

### PLUG-5 — Windows busy-delete window not fully closed (Low / design) — FIXED
Although `uninstall` `await`s `stop_ai_plugin_runtime` (7052→8968), the final `remove_dir_all`
(7054) runs after `wait_for_plugin_stopped` returns. If a child spawns a detached grandchild
not covered by `terminate_child_process_tree`/`kill_processes_listening_on_port`, or the wait
times out, a handle on `target` may still be open on Windows → `remove_dir_all` leaves the
directory partially deleted. The same applies to `stop_command` using `cmd.output()` with a
`PLUGIN_PROCESS_KILL_TIMEOUT_MS` timeout (8945) that may return before the process truly exits.
Implemented: transactional cleanup and rollback removal use three short bounded backoff retries
after a stop attempt. A persistent error retains the hidden staged directory and is logged rather
than silently converting it into a visible or partially discovered plugin.

## Execution-side temp lifecycle (verified SAFE, for contrast)
- `cleanup_failed_plugin_task_dir` (5361) wipes the task dir on `failed`/`cancelled`.
- `cleanup_stale_plugin_tasks_in_registry` (5066) expires `succeeded` unadopted tasks after
  `PLUGIN_TASK_SUCCESS_TTL_SECS` and clears outputs.
- `cleanup_orphan_plugin_task_dirs` (5028) sweeps leftover task dirs/TTL `.tmp` files not
  present in the registry ledger.
- Net: task temp dirs are bounded and self-healing; no unbounded leak found.

## Regression coverage
- `plugin_package_mutations_are_process_serialized` proves a second concurrent package mutation is
  rejected and that dropping the first guard permits the next transaction.
- `replace_plugin_directory_restores_previous_plugin_when_move_fails` proves a failed final move
  leaves the old code in its normal destination.
- `staged_uninstall_directories_restore_without_registry_commit` proves code and data return after
  an uncommitted uninstall transaction.
- `discovery_ignores_interrupted_plugin_transaction_directories` prevents recoverable backups from
  being loaded as duplicate plugins after a crash.

All confirmed PLUG-1 through PLUG-5 findings are now closed. Disk free-space availability remains
an OS-level best-effort condition; the declared-size budget prevents a package from requesting an
unbounded extraction.

## Relation to earlier P-series
P-1/P-2/P-3 (trust boundary: signature, publisher trust, permission grants, bearer-token
auth, runtime conflict gates, input staging) remain valid and are **not** weakened by these
findings. PLUG-* are purely about install/uninstall rollback correctness, orthogonal to the
trust boundary.
