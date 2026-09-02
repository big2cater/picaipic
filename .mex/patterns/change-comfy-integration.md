---
name: change-comfy-integration
description: ComfyUI server integration — workflow import/UI→API conversion, run/batch/cancel pipeline, and result import. Live status: docs/comfyui-integration-status.md.
last_updated: 2026-09-02
---

# Change ComfyUI integration

## When to use

- Changing workflow import, UI→API conversion, or format detection
- Changing the run/batch pipeline, cancel, cooldown, or result import
- Diagnosing "workflow corrupted / values shifted", "not API format", or stale-workflow picks

## Design contract

- PicAiPic never hosts the ComfyUI process; it talks HTTP to a user-managed server at
  `config.comfy.serverUrl` (default `http://127.0.0.1:8188`). No plugin trust boundary
  applies — this is a separate module from `t_plugin.rs`.
- Everything the server does stays local; results are imported into the **currently open
  album** (`resolveCurrentAlbumImportDestination`), and the entry points refuse up front
  when no album is open.
- Cancelling keeps already-finished images: the backend polls `/history`, the cancel flag is
  observed at the poll interval (~700 ms), and `comfy_run_workflow` returns `cancelled` as a
  distinct error the UI does not surface as a failure.

## Format detection contract (the load-bearing part)

`common/comfyConvert.js` distinguishes three shapes; do not weaken it:

1. **UI format** — has a `nodes` array whose entries carry `widgets_values`. Detection
   (`isUiWorkflow`) must require `widgets_values`: testing only for a `nodes`/`links` array
   misclassifies API-format exports that keep a `nodes` key, and running the converter on
   already-API JSON silently shifts every widget value (symptom: `cfg: 959948902156062.0`).
2. **API format, keyed** — `{ nodeId: { class_type, inputs } }`. Passed through as-is.
3. **API format + stray canvas keys** — the same keyed document with a `nodes`/`links`
   array on top (some ComfyUI versions export this). `normalizeApiWorkflow` rebuilds the
   keyed form from the `nodes` entries (recognised by `class_type`, which UI nodes never
   have) and never lets `nodes`/`links` reach `/prompt`, where an unexpected top-level key
   is mistaken for a node id.

Never send a raw document that still contains `nodes`/`links` to `POST /prompt`. Never
"fix" a converted value by hand — if `/object_info` widget ordering is wrong (custom packs
registering frontend-only widgets), the workflow must be exported via `Save (API Format)`
and skip the converter.

## Touchpoints

| Area | Path |
|------|------|
| Backend commands (test/object_info/upload/run/cancel/free/download) | `src-tauri/src/t_comfy.rs` |
| Import naming + sanitize (`import_file_as`, `sanitize_import_name`) | `src-tauri/src/t_utils.rs` |
| `import_file` command (`target_name` param) | `src-tauri/src/t_cmds.rs` |
| Command registration | `src-tauri/src/main.rs` (invoke_handler) |
| IPC wrappers | `src-vite/src/common/api.js` |
| UI→API conversion + format detection | `src-vite/src/common/comfyConvert.js` |
| Import/validation dialog | `src-vite/src/components/ComfyWorkflowDialog.vue` |
| Run/batch/cancel dialog | `src-vite/src/components/ComfyRunDialog.vue` |
| Workflow store + upsert | `src-vite/src/views/Settings.vue` (`saveComfyWorkflow`) |
| Single-selection menu | `src-vite/src/common/fileMenu.ts` (action `comfy-run`) |
| Multi-selection menu + dispatch | `src-vite/src/components/Content.vue` |
| Config defaults | `src-vite/src/stores/configStore.js` (`config.comfy`) |

## Import/run pipeline notes

- **Workflow save is an upsert by name** — re-importing a corrected file replaces the
  existing entry (id kept) so the run dialog's first-entry default never points at stale
  data. The dialog shows an overwrite hint when the name matches.
- Saved workflows are API-format JSON in `config.comfy.workflows`; the file picker reads
  through the browser File API (fs scope would reject other drives).
- Run is strictly serial; `config.comfy.cooldownSecs` (default 2) pauses between images so
  ComfyUI can release VRAM. The cooldown wait is interruptible (100 ms slices).
- Outputs download to `tempFilePath('picaipic', ext)` (must keep an app prefix —
  `delete_temp_file`/`cleanup_stale_temp_files` reject everything else), import via
  `importFile(dest, folderId, folderPath, targetName)`, then best-effort `deleteTempFile`.
- **Imports use readable names**: `targetName` is `comfy_<workflow name>_<n>.<ext>`, so the
  album does not fill with `picaipic_<uuid>` staging names. The backend
  `t_utils::import_file_as` + `sanitize_import_name` are the authority (final path
  component only, illegal chars replaced, Windows reserved names tamed, `(1)` collision
  suffix); the frontend stem sanitization is cosmetic. Do not bypass either.
- **Download intermediates keep an app prefix and self-clean.** `comfy_download_output`
  stages into `picaipic-comfy-<uuid>.tmp` next to the destination (same-volume rename is
  atomic) and removes it when the final rename fails. A `.`-prefixed name would escape
  `is_allowed_temp_basename` and the stale-temp sweep — never introduce one.
- **`/prompt` submission must check the HTTP status** before parsing JSON: ComfyUI answers
  200 + `node_errors` for rejected workflows (report those), and 4xx with an error body for
  malformed prompts (report status + text). Do not let the body parse stand in for either.
- Only node-output `images` arrays are collected; animated/video outputs are skipped.
- **After a run, Content.vue jumps to the first result**: `revealImportedFile` (loaded
  list → else `get_query_file_position` + chunk prefetch → `selectedItemIndex`, GridView
  scrolls). It waits for the content reload to settle first (`updateContent` does not
  await its list reload) and only jumps while still on the importing album. This helper
  is shared with the dedup panel **and the other new-file producers** (batch saveAs,
  photo frame, print layout, plugin results from grid dialog and viewer) — keep it
  generic; do not fork per tool.
- Loaders are recognised by shape (`inputs.image` is a string), not by `class_type`, so
  custom loaders like `LoadImageAutoMP` work; `injectImage` overwrites every loader.

## Verification

```powershell
pnpm --dir src-vite build
# converter shape fixtures (plain API, API+nodes, UI, UI-without-widgets_values, garbage)
node --input-type=module -e "import('./src-vite/src/common/comfyConvert.js').then(m => { ... })"
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml   # t_comfy.rs has no unit tests; run covers regressions elsewhere
```

End-to-end (needs a real server): import the previously offending export and confirm no
"UI format" prompt and no `not_api_format` error; run one image to `SaveImage` output.