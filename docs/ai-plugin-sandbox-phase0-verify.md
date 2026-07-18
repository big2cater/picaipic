# Sandbox Phase 0 verification checklist

Use this after a host build that includes Phase 0 staging changes
(cross-platform default staging, fail-closed copy errors, staging diagnostics).
Phase 1 (allow-list) and Phase 2 (hardlink→copy) land on the same host path
and are covered by the same checks where noted.

## Automated (must pass)

```powershell
cargo test --manifest-path src-tauri/Cargo.toml input_staging -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml
powershell -ExecutionPolicy Bypass -File .\scripts\check_plugin_host.ps1
```

Expected unit tests:

- `input_staging_rewrites_external_paths_and_counts_bytes`
- `input_staging_hardlink_shares_inode_when_same_volume`
- `input_staging_fails_closed_when_copy_cannot_complete`
- `input_staging_disabled_queue_message`
- `input_staging_real_layout_report_hardlink_or_copy`  
  (library-like source under OS temp staged into `src-tauri/target/.../inputs`;
  cross-volume → copy + `staging-report.json`; same-volume → hardlink)

### Last automated run (2026-07-18, Windows)

| Check | Result |
|-------|--------|
| `cargo test … input_staging` | **5 passed** |
| `cargo check --manifest-path src-tauri/Cargo.toml` | **pass** (via plugin-host script) |
| `scripts/check_plugin_host.ps1` | **All selected checks passed** (fmt, cargo check, frontend build, SA-LUT/NAFNet py_compile) |

## Manual / release-exe (Windows and Linux)

### A. Staging on by default

1. Install SA-LUT (or NAFNet) package; bind models if needed; Run setup + Smoke for a working profile.
2. Start the plugin.
3. Invoke color-transfer / denoise on a **library image outside** the plugin store
   (normal album photo is fine).
4. Open the latest task under Settings → plugin Recent tasks, or inspect the task dir:
   - `plugin-cache/<plugin-id>/tasks/<taskId>/inputs/`
   - Expect a staged copy/hardlink of the source image **and** `staging-report.json`.
5. Task message should look like:
   - `Queued (staging: 1 file(s), N bytes, 1 hardlink, 0 copy, 0 already writable)`  
     (hardlink when library and plugin store share a volume; otherwise copy counts rise)
     or include skipped counts when applicable.
6. Open `staging-report.json` and confirm `hardlinkedFiles` / `copiedFiles` make sense
   for the disk layout (same drive → hardlink preferred).
7. Confirm the plugin completed successfully using the staged input.

### A′. Host-path materialize proof (no full plugin invoke)

When a full SA-LUT/NAFNet UI invoke is not convenient, prove the **same host
staging path** the invoke uses:

1. Source must be a real library-style absolute path **outside** the plugin store.
2. Destination under  
   `…/picaipic-local/plugin-cache/<plugin-id>/tasks/<taskId>/inputs/`.
3. Host preference: `hard_link` then `copy` (`stage_one_file`).
4. Expect `staging-report.json` with camelCase counts.

#### Windows evidence (2026-07-18, this machine)

| Item | Value |
|------|--------|
| Library source | `C:\Users\a7925\Downloads\3333.jpg` (album folder; outside plugin store) |
| Plugin store / cache | `D:\ailab\PicAiPic\src-tauri\target\release\picaipic-local\plugin-cache\…` |
| Volume layout | **Cross-volume** (`C:` → `D:`) |
| Expected method | **copy** (hardlink fails with WinError 17 / different drive) |
| Observed | hardlink refused; full copy of 3 186 124 bytes into task `inputs/` |
| Report | `plugin-cache/picai-salut-color/tasks/phase0-verify-*/inputs/staging-report.json` with `hardlinkedFiles: 0`, `copiedFiles: 1` |
| Unit coverage | `input_staging_real_layout_report_hardlink_or_copy` (TEMP → crate `target`, same preference order + report file) |

This closes the **staged-path / hardlink-or-copy / report** bar for Windows host
behavior.

#### Windows SA-LUT full start + color-transfer (2026-07-18)

Host-equivalent path (same env/staging/HTTP contract as the Tauri host; not
driven through the GUI IPC this run):

| Item | Value |
|------|--------|
| Profile / runtime | `windows-amd-rocm` / `shared-runtimes/python312-rocm72-torch291` |
| GPU | AMD Radeon RX 7900 XT (torch `2.9.1+rocm7.2.1`, CUDA/HIP available) |
| Listen | `127.0.0.1:18011` with bearer token |
| Library sources (outside store) | `C:\Users\a7925\Downloads\3333.jpg`, `4444.jpg` |
| Staging | 2 files, 6 310 648 bytes, **0 hardlink / 2 copy** (C:→D:) |
| Report | `plugin-cache/picai-salut-color/tasks/phase0-e2e-de513e5c-…/inputs/staging-report.json` |
| Invoke payload | only staged paths under `…/tasks/…/inputs/` (no raw `C:\…` paths) |
| HTTP | `POST /invoke/color-transfer` → **202** queued |
| Output | `…/outputs/3333-salut-phase0-e.png` (**13 293 617** bytes, PNG magic OK) |
| Summary | `…/tasks/phase0-e2e-de513e5c-…/e2e_summary.json` |

Notes: long-poll after the result file already existed later saw HTTP 502 (plugin
process became unresponsive); **model output was already written successfully**.
GUI/Tauri IPC invoke remains a nice-to-have recheck if the release shell wiring
is under test.

### B. Staging disable switch

1. Set `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` and restart the host.
2. Invoke again.
3. Task message should be `Queued (input staging disabled)`.
4. No requirement to create staged copies under `inputs/` for external paths
   (dev/debug path only).

### C. Fail-closed (optional chaos)

1. With staging enabled, make the task staging parent unwritable if you can safely
   simulate (or rely on unit test).
2. Invoke must **fail** with a staging error; payload must not proceed with the
   original external path.

### D. Optional Windows ACL mode (not Phase 0 default)

1. Only if testing ACL: `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1`.
2. Start log shows deny summary; GPU probe still works.
3. Stop/restart cleans deny ACEs.

## Pass criteria for Phase 0

- [x] Design doc: `docs/ai-plugin-sandbox-roadmap.md`
- [x] Staging not Windows-gated
- [x] Staging copy failures fail closed
- [x] Staging diagnostics in task message + `staging-report.json`
- [x] Unit tests for rewrite / fail-closed / disabled message
- [x] Unit test for hardlink preference + real-layout report (`hardlink` or `copy`)
- [x] Windows host-path staged materialize (library outside store → plugin-cache `inputs/` + report; cross-volume → copy)
- [x] Windows SA-LUT full start + color-transfer on staged library paths (ROCm GPU; PNG output under task `outputs/`)
- [ ] Manual staged-path check on Linux build (if available)
- [ ] Optional: same invoke via Tauri GUI / `invoke_ai_plugin_capability` IPC (release shell wiring)

Phase 1 (allow-list) and Phase 2 (hardlink mainline) have also landed in code.
This checklist remains valid for manual staged-path proof. Phase 3 network OS
block and Phase 4 Landlock stay out of this checklist.
