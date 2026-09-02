# Audit: Frontend Destructive / Consistency Path (src-vite)

> Status: re-verified 2026-07-30; FE-PLUG-1 and FE-IMPORT-1 fixed. FE-LIB-1/2 and
> FE-IMG-2 data-consistency risks were closed by backend exclusive rebinding and atomic edit
> metadata refresh. FE-C-3 remains the intentional `copy_file` transfer contract.

- Scope: Vue frontend destructive/consistency surfaces that call the audited backend commands
  (library switch/remove, file copy/move/delete, image edit, plugin install/uninstall).
  Mirrors the backend audit (`audit-*.md`); this runbook captures **frontend-side** gaps and
  how the frontend interacts with the backend findings (IMG-*, LIB-*, C-*, D-*, PLUG-*).
- Method: read-only static review of `src-vite/src` (api.js, Home.vue, Content.vue,
  ManageLibraries.vue, ImageEditor.vue, FileInfo.vue, Settings.vue). **No code changed.**
- Initial audit: 2026-07-29. Re-verified and fixed: 2026-07-30.

## Findings

### FE-LIB-1 — CLOSED BY BACKEND EXCLUSIVE REBINDING
- `ManageLibraries.vue` `doDeleteLibrary` (534) and `clickOk` (556) call `removeLibrary` /
  `switchLibrary` directly, with **no** `cancelIndexing` / `cancelFaceIndex` step.
- The delete button is disabled only on `lib.id === 'default' || isRenaming` (89/96) — **never**
  because a scan/import is in progress.
- Contrast: the main-path `Home.vue` `doSwitchLibrary` (744) *does* save libConfig, cancel
  indexing (755-760) and `cancelFaceIndex` (763) before `switchLibrary` (765).
- Re-verification: `switch_library` / `remove_library` now acquire exclusive media-scan/import
  rebinding guards. ManageLibraries can no longer redirect in-flight writes to another library.
  The missing frontend cancellation is now a UX difference, not a corruption path.

### FE-LIB-2 — CLOSED BY BACKEND EXCLUSIVE REBINDING
- `Home.vue` 744-765 cancels indexing only when `libConfig.index.status > 0 &&
  libConfig.index.albumQueue.length > 0` (755). If the scan is in another state, or a batch is
  already in flight with an empty queue, the cancel is skipped.
- Re-verification: cancellation completion is no longer the correctness boundary. The backend
  waits for conflicting full indexing, dedup, face indexing, and imports before rebinding.

### FE-IMG-2 — FIXED BY ATOMIC BACKEND EDIT REFRESH
- `ImageEditor.vue` `executeSave` (3299): on `editImage` success (3305) it calls
  `uiStore.updateFileVersion(fileInfo.value.file_path)` (3309) — this reloads the thumbnail from
  the file but **does not recompute hash / phash / dimensions / orientation** in the library.
- Re-verification: overwrite saves pass `fileId`; the backend stages the original, refreshes file
  metadata transactionally, invalidates thumbnail/embedding/hash state, and restores on failure.
  `editImage` now rethrows IPC errors so callers keep a real failure path. A corruption warning is
  no longer appropriate because the original is restored before failure is returned.

### FE-C-3 — NOT A DEFECT; EXPLICIT TRANSFER CONTRACT
- `Content.vue` `onCopyToFolder` (6698) and the drop handler (2153) both call `copyFile` then
  `addFileToDb(...)` to index the copy (6701 / 2155). This *compensates* for backend `copy_file`
  not inserting an `afiles` row (backend C-3), but it is a fragile **double-write**:
  - If backend C-3 is later fixed (copy_file inserts the row), the frontend `addFileToDb` will
    duplicate-insert → duplicate library record for the same file.
- Re-verification: `copy_file` intentionally remains a filesystem transfer primitive because it
  also supports destinations outside the current library. Shipped library-copy callers explicitly
  add the copied path to the DB and remove it if indexing fails. Do not remove this compensation
  unless the IPC contract itself is redesigned without double-indexing outside-library copies.

### FE-D-UI — Batch delete uses real deleted ids (VERIFIED SAFE, contrast)
- `Content.vue` `onTrashFile` (6780): non-dedup multi-select `batchDeleteFiles` (6843) removes
  only `result.deletedFileIds` (6850-6861) and `throw`s if `!result` (6847) or if failed with
  zero deletions (6854). Single-select path `deleteFileAlways` (6920) awaits success *before*
  `removeFromFileList` (6868-6869). → **Not optimistic**: a backend failure does not hide the
  file. This is a good mitigation for backend D-1 (disk/DB order). Caveat: if backend D-1 leaves
  a dangling `afiles` row (file in trash, row alive), the next `refresh-content` re-shows it —
  that is the backend bug surfacing, not a frontend defect.

### FE-PLUG-1 — FIXED: Plugin install concurrency guard (Medium)
- `Settings.vue` `chooseAiPluginPackage` (5147) guards with `if (isLoadingAiPlugins.value) return`
  (5148), but `isLoadingAiPlugins` is only set in the **panel-list loader** (4814/4852), never in
  the install path (`installAiPluginPackageWithTrust` 5165 / `installAiPluginPackage` api.js 1723).
- Fix: `isInstallingAiPlugin` is set before opening the package picker and cleared in `finally`.
  The install action remains disabled through trust confirmation, retry, installation, panel reload,
  and model prompting. The backend also independently provides transactional install rollback.

### FE-PLUG-2 / FE-PLUG-3 — Install/uninstall failure surfacing (Low, mostly backend-led)
- Install failure is surfaced correctly: `installAiPluginPackage` (api.js 1723) **re-throws** (not
  swallowed), so `installAiPluginPackageWithTrust` catches (5179) and shows `toast.error` — no false
  "success". Trust-required flow is correct (trust then retry at 5202).
- **FE-PLUG-3**: when a backend **PLUG-2** half-install leaves an orphan plugin *directory* on disk
  (registry never registered it), the frontend only shows the error and cannot clean it (it isn't in
  the registry). Root cause is backend PLUG-2; frontend could at least warn "restart may be needed".
- **FE-PLUG-2**: `uninstallInstalledAiPlugin` (5208) does **not** optimistically remove the plugin on
  failure (list update is inside `try`), so a backend **PLUG-4** registry/disk inconsistency is not
  masked — good. The failure is shown via `toast.error` (5235) + `finally` resets loading.

### FE-IMPORT-1 — FIXED: Drag-drop reports partial failure (Medium→Low)
- `Content.vue` `domDrop` (4045) imports dropped files in a loop. For the filesystem-path branch
  (4075-4082) and the bytes branch (4093-4104), each `importFile` / `importFileBytes` failure is caught
  and only `console.error(...)` — it is **not** counted, and no partial-failure toast is shown:
  ```4075:4082:src-vite/src/components/Content.vue
  for (const filePath of [...new Set(filePaths)]) {
    try {
      const file = await importFile(filePath, folderId, folderPath);
      if (file) imported++;
    } catch (err) {
      console.error('Failed to import dropped file:', filePath, err);
    }
  }
  if (imported > 0) {
    await refreshImportedFiles(albumId);
    toast.success(t('msgbox.drop_import.success', { count: imported }));
    return;   // <-- returns on ANY success, failed files get NO user-visible signal
  }
  ```
- Impact: drop 10 files, 7 succeed + 3 fail → user sees "imported 7" and assumes all 10 landed. The 3
  failures are invisible (only in devtools console). The user may later believe missing files were
  imported. Source files are not destroyed (import is a copy), so this is **data-visibility**, not
  data-loss — but it pairs with backend **C-4** (batch import silent drop) in misleading the user.
- Fix: both filesystem-path and bytes fallback branches count thrown, falsy, invalid, empty, and
  oversized items. Any partial success uses `msgbox.drop_import.partial`; complete success retains
  the success toast, while zero-success behavior still falls through to URL handling/no-files.
- Note: `importFile` (api.js 1113) re-throws on backend error, and returns falsy on a non-throwing
  backend result (e.g. a skip/duplicate). The drop loop treats *any* falsy as "not imported", so a
  backend "skipped" also increments nothing and is silently dropped — same silent-loss shape.

### FE-MOVE — Move refresh consistency (VERIFIED SAFE, contrast)
- `Content.vue` move path (6453-6501) only removes moved rows on a **truthy** `movedFile` return
  (`successfulMoves` / `successIds`, 6461-6473), and after the loop refreshes affected albums +
  total count (`refreshAffectedAlbums` + `refreshLibraryTotalCount`, 6493-6494). Single-select mode
  (6483) likewise only updates on `movedFile`. No optimistic removal on failure.
- Thumbnail cache is **fileId-keyed** (`utils.ts` `getThumbnailCacheKey(fileId, ...)` 461); `move_file`
  preserves the `file_id`, so cached thumbnails stay valid after a move. No stale-thumbnail bug.
- Minor: a backend "skip" (conflict policy) returns falsy and is counted as a failure
  (6468-6470, 6476-6478), so a deliberate skip shows an error toast — cosmetic mislabel, not data loss.
- Conclusion: move path is robust; no new finding. (Mirrors backend C-1 being the gap, not the UI.)

### FE-STORAGE — DB storage-location migration guards (VERIFIED SAFE, contrast)
- `Settings.vue` `selectDbStorageDir` (5736) **and** `restoreDefaultDbStorageDir` (5775) both guard with
  `libConfig.index.status === 1` (5737, 5776) and `isFaceIndexing()` (5742, 5781) before allowing a
  location change — the two paths are **symmetric** (early hypothesis of an asymmetric guard was wrong).
- `chooseDbStorageDir` (5751) / `confirmResetDbStorageDir` (5790) set `isChangingDbStorage` (5763/5794)
  and reset in `finally` (5771/5802); the change/reset buttons are `:disabled="isChangingDbStorage"`
  (434/443) — concurrent migration is prevented.
- `changeDbStorageDir` / `resetDbStorageDir` (api.js 50/59) re-throw on backend error, surfaced via
  `toast.error` (5769/5800). No false "success".
- Re-verification: backend config mutation serialization plus exclusive library rebinding prevent
  concurrent storage migration and library switch/delete from crossing database ownership. The
  storage-migration UI remains robust; no new frontend gap.

## Frontend→backend finding map
| Frontend | Backend | Relation |
|----------|---------|----------|
| FE-LIB-1 / FE-LIB-2 | LIB-2 / LIB-1 | Closed by backend exclusive rebinding; frontend cancellation is UX-only |
| FE-IMG-2 | IMG-2 (+IMG-1) | Closed by staged save plus transactional metadata/cache refresh; IPC errors rethrow |
| FE-C-3 | C-3 | Intentional transfer primitive plus explicit library indexing/orphan cleanup |
| FE-D-UI | D-1 | Frontend correctly avoids optimistic delete (safe contrast) |
| FE-PLUG | PLUG-1 / PLUG-2 | Install concurrency guarded; failure surfacing verified; backend rollback is transactional |
| FE-IMPORT-1 | C-4 | Fixed: drag-drop reports imported and failed counts on partial success |
| FE-MOVE | C-1 | UI move path is safe (fileId-keyed thumbs, truthy-only update, album refresh); backend C-1 is the gap |
| FE-STORAGE | S-1..S-7 | UI migration path is safe (symmetric index/face guards, concurrency lock, re-throw); no new gap |

## Current conclusion
All correctness findings in this frontend audit are closed or reclassified. Retain the explicit
`copy_file` + `addFileToDb` library-copy contract, and rely on backend exclusive rebinding rather
than frontend cancellation timing for library isolation.
