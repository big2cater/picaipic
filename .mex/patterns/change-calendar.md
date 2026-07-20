---
name: change-calendar
description: Runbook for calendar sidebar day/month selection, taken-date counts, and Content date-range queries.
last_updated: 2026-07-20
---

# Change calendar / 日历

## When to use

- Calendar dots empty / wrong counts
- Clicking a day or month shows “未找到文件” despite counts
- Calendar UI labels (month/day numbers vs photo counts)
- On this day / 历史上的今天 routing
- Sidebar index routing for calendar vs search/tags after Smart Albums

## Touchpoints

| Area | Path |
|------|------|
| Sidebar panel | `src-vite/src/components/Calendar.vue` |
| Month grid | `CalendarMonthly.vue` |
| Day grid | `CalendarDaily.vue` |
| Content routing | `Content.vue` `updateContent` branch `SIDEBAR.CALENDAR` |
| Date range helper | `src-vite/src/common/utils.ts` `getCalendarDateRange` |
| Sidebar absolute indices | `src-vite/src/common/constants.ts` `SIDEBAR` |
| Day counts IPC | `api.js` `getTakenDates` → `get_taken_dates` |
| Day counts SQL | `t_sqlite.rs` `Face`-adjacent `AFile::get_taken_dates` |
| File list date filter | `t_sqlite.rs` `build_search_query_parts` `start_date`/`end_date` |

## Absolute sidebar indices (do not hard-code elsewhere)

Order is `Home.vue` `buttons` array. Smart Albums sits at index **1** and shifts later entries:

| Const | Index | Panel |
|-------|-------|--------|
| `SIDEBAR.LIBRARY` | 0 | 相册 |
| `SIDEBAR.SMART` | 1 | 智能相册 |
| `SIDEBAR.FAVORITE` | 2 | 收藏 |
| `SIDEBAR.SEARCH` | 3 | 搜索 |
| `SIDEBAR.CALENDAR` | 4 | **日历** |
| `SIDEBAR.TAG` | 5 | 标签 |
| `SIDEBAR.PERSON` | 6 | 人物 |
| `SIDEBAR.LOCATION` | 7 | 地点 |
| `SIDEBAR.CAMERA` | 8 | 相机 |
| `SIDEBAR.MAP` | 9 | 地图 |

**Bug fixed 2026-07-20:** Content still treated calendar as `3` after Smart Albums insert → clicks updated `libConfig.calendar` but Content ran the search branch → empty list. Always use `SIDEBAR.*`.

## Rules

- Calendar dots and content list must use the **same** filters: file-type mask, folder search exclusion, Live companion hide (`live_photo_type != 2`).
- Date range filter compares **local calendar days** via `strftime('%Y-%m-%d', col, 'unixepoch', 'localtime')`, not raw unix bounds alone (avoids TZ/boundary empty lists).
- Pass `calendarSort: config.settings.calendarSort` into `getFileList` so the date column matches dots (taken / created / modified).
- On this day: `startDate: -1, endDate: -1`, force taken-date sort desc unless product changes.
- UI: month cells show **1–12**, day cells show **1–31**; photo count is hover `title` / density styling.
- Auto-select a year/month/day with photos when selection is null so Content is not left on empty title forever.
- Coerce QueryParams numerics (`startDate`, `calendarSort`, …) with `Number`/`Math.trunc` before IPC — HTML/pinia strings break Rust `i64`.
- Packaging does **not** clear user WebView/localStorage/DB; calendar bugs are code/index issues, not “need to clear cache”.

## Verify

- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: open 日历 → click a filled month/day → grid shows files
- Manual: switch 按月/按日 → cells show month/day numbers
- Manual: 历史上的今天 empty only when no files share today’s month-day
- Manual: change 设置 → 库 → 日历排序 and confirm dots + list stay aligned
