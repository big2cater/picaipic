---
name: change-library-shortcuts
description: Library panel quick entries (All / Favorites / On this day) and view-adaptive date grouping in Content/GridView.
last_updated: 2026-07-19
---

# Library shortcuts + adaptive date grouping

## When to use
- Add or change Library sidebar quick entries (全部 / 收藏 / 今日)
- Route Content queries from `libConfig.library.item`
- Adjust view-adaptive date grouping (day/month/none) for grid headers

## Touchpoints
| Area | Files |
|------|--------|
| Quick entry ids | `src-vite/src/common/constants.ts` (`LIB_ITEM`, `DATE_GROUP`) |
| State | `libraryStore.library.item`; Rust `LibraryQuickState` in `t_config.rs` |
| Sidebar UI | `Library.vue` (keeps `AlbumList` below shortcuts) |
| Clear on album click | `useAlbumSelection.ts` → `library.item = all-files` |
| Query routing | `Content.vue` `updateContent` when `sidebarIndex === 0` && `album.id === 0` |
| Date grouping | `Content.vue` `effectiveDateGrouping` → `GridView` prop `dateGrouping` |
| i18n | `library.all_files` / `library.favorites` / `library.on_this_day` |

## Behavior
1. **全部** → all files query; selected when `library.item === all-files` and `album.id === 0`
2. **收藏** → `isFavorite: true` library-wide (does not switch Favorite sidebar)
3. **今日** → `startDate/endDate = -1` (on-this-day across years) + taken-desc sort; force **day** date groups
4. Clicking an album/folder resets `library.item` to `all-files`
5. Calendar year → month groups; month → day groups; day → none; search/person/smart-tag → none; else Settings `grid.dateGrouping`

## Non-goals (this slice)
- Full lap 0.3 Library rewrite (ratings tree / subjects inside Library panel)
- Server-side `groupBy` / YEAR grouping (still client timeline day/month headers)

## Verify
- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Manual: Library → 全部/收藏/今日 titles + counts; open album clears quick selection; 今日 shows day headers when time-sorted
