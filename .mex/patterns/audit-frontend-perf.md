# Frontend Performance Audit — PicAiPic

_Audit of frontend (src-vite) rendering / memory / IPC performance for 10k–100k file libraries.
Original read-only review dated 2026-07-29; re-verified and closed where actionable on 2026-07-30.
Companion to `audit-frontend-destructive.md`._

## 2026-07-30 re-verification

| ID | Current status | Resolution |
|----|----------------|------------|
| PE-1 | **Fixed** | `Content.vue` incrementally evicts off-screen `data:image/...` values outside a bounded visible/prefetch window while retaining the active item. Back-scroll first checks the shared thumbnail LRU. |
| PE-2 | **Fixed at the correct boundary** | `GridView.vue` owns the single plugin contribution computed and passes it to cards. Per-file menu computation remains because favorite/rating/type/live-photo state is file-specific. |
| PE-3 | **Verified low impact; no change** | Getter calls are O(1) and run only for the virtualized visible window. Wrapping the 10k–100k source list in normalized row objects would add allocation/reactivity cost for negligible gain. |
| PE-4 | **Stale / already fixed** | Current `utils.ts` has no idbKeyval thumbnail cache. It uses a process-local byte-accounted LRU capped at 96 MiB and clears it when the library id changes. |

Verification: `pnpm --dir src-vite build`; `git diff --check` for the touched frontend files.
Limitation: this closure proves bounded reference retention by code inspection/build, but did not capture
a before/after heap or RSS trace while scrolling a representative 100k-file library. Keep that measurement
as the next performance validation rather than treating the cap as a measured memory reduction.

## Scope
Focus: large-library rendering cost, thumbnail memory/IPC, virtualization correctness, listener/timer
lifecycle, and per-card reactive overhead. Verified against `Content.vue`, `GridView.vue`,
`VirtualScroll.vue`, `Thumbnail.vue`, `Calendar.vue`, `CalendarMonthly.vue`, `utils.ts`, `api.js`.
_dates: 2026-07-29_

## Findings

### PE-1 — Thumbnail base64 data URLs are never evicted within a folder session (Medium)
- `Content.vue` `getFileListThumb` (7648-7745) attaches a **base64 `thumbnail`** onto each `fileList`
  item as it scrolls into view (`file.thumbnail = thumb.url`, ~7732) and never clears it. There is no
  LRU / off-screen eviction.
- `preserveLoadedThumbnails` (7807-7870) only re-attaches previously-fetched thumbnails when the
  **same** folder is re-queried (it builds `thumbnailsById` from the old `fileList` before it is
  replaced); it does **not** evict. So within one large folder, every scrolled-in thumbnail stays in
  the reactive `fileList` for the life of the folder view.
- Impact: for a 10k–100k library, scrolling through a folder accumulates up to N base64 strings held
  in **Vue-reactive proxies**. Memory grows to tens–hundreds of MB and each proxy carries reactivity
  overhead. Worst at scroll-through of the largest folder. No correctness bug, but a real memory-scalability
  gap that contradicts the project's own "10k-100k+ performance" non-negotiable.
- Fix direction: add an LRU cap on retained thumbnails (e.g. keep last K + currently visible), evict
  off-screen `thumbnail` fields on scroll, or switch `thumbnail` to blob URLs with `revokeObjectURL`
  on eviction. Keep `pendingThumbnailKeys` dedup as-is.

### PE-2 — Every visible `Thumbnail` builds its own plugin-menu computed (Low→Medium)
- `Thumbnail.vue` calls `useFileMenuItems(props, ctx, store)` per card (474-491), which returns a
  computed `menuItems` derived from `pluginStore.getMenuItems(...)`. Each visible card is an independent
  subscriber; a single plugin-store mutation (install/uninstall/menu update) recomputes the menu across
  all ~visible cards (50–200). Results are not shared between cards.
- Impact: burst of recomputation during plugin changes and on mount of each card. Minor CPU, but
  avoidable. Could compute the package-scoped menu once at the grid level and pass down, or memoize by
  `file_type`/package.

### PE-3 — Per-slot template calls several computed getters repeatedly (Low)
- `GridView.vue` slot template invokes `getFileItem(item)`, `getFileIndex(item, index)`,
  `getDateGroupIndex(...)`, plus badge computeds (`mediaInfoBadges`, `statusBadges`, `menuItems`)
  **multiple times per card per render** (lines 198-259). Each is O(1) but multiplied across all
  visible cards on every scroll-frame reactive flush.
- Impact: small, but normalizing a single `row` object per visible item (computed once) would remove
  the redundant lookups. Optional cleanup, not a blocker.

### PE-4 — `getThumbnailDataUrl` caches every thumbnail as base64 in idbKeyval (Low)
- `utils.ts` `getThumbnailDataUrl` (356-381) reads/writes a per-file base64 thumbnail cache in idbKeyval
  (IndexedDB/localStorage-backed). For 100k files this on-disk cache can grow to GBs. Read is async per
  card but deduped via `pendingThumbnailKeys` (7733), so it is throttled, not unbounded per session.
- Impact: disk growth over time; no per-session memory leak. Recommend a cache-size cap / TTL or a
  backend-side thumbnail cache instead of re-encoding every frontend request.

This finding describes the 2026-07-29 snapshot and is not true of the re-verified current code.

### Non-issues verified (do NOT regress)
- **Grid virtualization is real**: `VirtualScroll.vue` uses binary-search geometry lookup, rAF-coalesced
  scroll, a single `ResizeObserver`, and `contain: layout paint style`; `GridView` `renderItems` slices
  only the visible window + 8-row buffer (138-195). No full-list DOM.
- **Metadata is paged**: `getFileList` seeds lightweight placeholders (`isPlaceholder: true`) for the
  full count, then `getQueryFiles(params, chunkStart, chunkSize)` fills real rows in chunks (4696, 4808).
  100k placeholders are cheap objects; real-metadata memory is bounded by loaded chunks.
- **Calendar is bounded**: `CalendarMonthly.vue` renders 12 month-number cells per year, not per-day;
  multi-decade libraries still produce only ~years×12 nodes. Not virtualized but cheap.
- **FilmStrip is a single preview**, not a strip of all thumbnails: `Content.vue:270-296` renders one
  `MediaViewer` for `fileList[selectedItemIndex]` — no all-thumbnails render loop.
- **Scroll fetch is gated**: `handleVisibleRangeUpdate` only re-fetches when `lastVisibleRange` actually
  changes; `onUpdate` (GridView 852) computes the visible slice cheaply.
- **FX lifecycle clean**: `CyberpunkBackground` / `PhotoGlitchLayer` cancel rAF / dispose GL / disconnect
  observers on unmount (see `audit-cyberpunk-theme.md`).
- **Idle/debounce hygiene**: `useIdleCallback` batches non-urgent work; `tagSearch` and keybindings are
  debounced.

## Frontend→backend performance map
| ID | Severity | Area | Note |
|----|----------|------|------|
| PE-1 | Medium | Thumbnail memory | No LRU eviction within folder; contradicts 10k-100k perf goal |
| PE-2 | Low→Med | Per-card menu computed | Share/memoize plugin menu at grid level |
| PE-3 | Low | Per-slot getters | Normalize a `row` object per visible item |
| PE-4 | Low | Thumbnail disk cache | idbKeyval base64 cache can grow to GBs; needs cap/TTL |

## Recommendations
1. **PE-1** — Add LRU eviction for `file.thumbnail` within a folder: keep visible + small look-behind
   window, drop the rest; or convert to blob URLs with `revokeObjectURL`. Highest-impact perf fix.
2. **PE-2** — Compute plugin context menu once per grid (keyed by `file_type`/package) and inject,
   instead of `useFileMenuItems` per card.
3. **PE-3** — Build a single normalized `row` per visible item in `renderItems` to avoid repeated
   getter calls in the slot template.
4. **PE-4** — Cap/TTL the idbKeyval thumbnail cache, or rely on a backend thumbnail cache keyed by
   file id + mtime so frontend never re-encodes.
