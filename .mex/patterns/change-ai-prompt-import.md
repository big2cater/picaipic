---
name: change-ai-prompt-import
description: AI PNG/JPEG prompt import into empty comments during library scan.
last_updated: 2026-07-28
---

# Change AI prompt import (PNG + JPEG)

## When to use
- Change which tools/formats are parsed (A1111, NovelAI, InvokeAI, ComfyUI)
- Change empty-only comments fill policy or scan-time setting
- Extend import sources (more containers / XMP)

## Key files
| Layer | Path |
|-------|------|
| Parse + flag | `src-tauri/src/t_ai_prompt.rs` |
| Scan hook | `src-tauri/src/t_sqlite.rs` (`AFile::new`, `update_file_info`) |
| IPC | `src-tauri/src/t_cmds.rs` `set_import_ai_prompts` / `get_import_ai_prompts` |
| Register | `src-tauri/src/main.rs` |
| Setting | `src-vite/src/stores/configStore.js` `importAiPromptsToComments` |
| UI | `src-vite/src/views/Settings.vue` Library → Metadata |
| Sync | `Home.vue` boot + Settings watch + `main.js` event |
| API | `src-vite/src/common/api.js` `setImportAiPrompts` |

## Behaviour contract
1. **Default on** (`AtomicBool` + Pinia `importAiPromptsToComments: true`).
2. **PNG**: `tEXt` / `iTXt` / `zTXt` (`parameters`, `Comment`, `prompt`, `workflow`, …).
3. **JPEG**: EXIF `UserComment` (charset-aware), JPEG `COM` markers, fallback `ImageDescription` only when it looks like a prompt.
4. Fill **`comments` only when empty** — never overwrite user notes.
5. New inserts: `AFile::new` may set `comments` from prompt; `insert()` writes column.
6. Change re-scan: `update()` omits comments; empty→fill uses `update_column("comments", …)`.
7. Unchanged mtime files are **not** backfilled on normal rescan.
8. Bound PNG ancillary scan (≤4 MiB) and JPEG marker walk (≤2 MiB).
9. Truncate stored prompt to 4000 chars.
10. Prefer EXIF already opened in `AFile::new` (pass UserComment / ImageDescription into extractor).
11. JPEG prompt import reuses the pre-read header scan when it reaches SOS/EOI; only truncated/incomplete headers reopen the file for COM markers. EXIF `UserComment` is consumed directly before any marker scan.

## Verify
```bash
cargo test --manifest-path src-tauri/Cargo.toml t_ai_prompt
cargo check --manifest-path src-tauri/Cargo.toml
```
Manual: scan A1111 PNG/JPEG → FileInfo comments filled; edit comment → rescan keeps it; toggle off → new files not filled.
