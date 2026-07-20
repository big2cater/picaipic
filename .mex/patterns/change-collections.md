---
name: change-collections
description: Virtual Collections (集合) tray, membership SQLite tables, and Content query source.
last_updated: 2026-07-19
---

# Change collections / 集合

## When to use

- Add/edit collection CRUD, membership, tray UI, or drag-drop into collections
- Change how Content loads a collection view

## Touchpoints

| Area | Path |
|------|------|
| Schema | `t_migration.rs` v7 + `ensure_collections_tables` |
| Host | `t_sqlite.rs` `ACollection`, `t_cmds.rs` collection commands, `main.rs` |
| IPC | `api.js` list/create/rename/delete/add/remove/clear/get_* |
| UI | `CollectionTray.vue`, `Home.vue` tray slot, Settings `showCollections` |
| Content | `querySource === 'collection'`, drag `data-collection-drop-*` |
| State | `libraryStore.collection.selectedId`, `activePane`, `config.collectionTray` |
| i18n | `collection.*`, `settings.general.show_collections` |

## Rules

- Max **10** collections per library (`ACollection::MAX_COLLECTIONS`).
- Membership is many-to-many; file delete cascades membership.
- Counts/lists exclude Apple Live companion videos (`live_photo_type != 2`).
- Definitions of which collection is selected live in **LibraryState**; rows live in **per-library SQLite**.
- Content: when `activePane === 'collection'` and `selectedId` set, use `get_collection_*` APIs with normal sort/type filters from `config.search`.
- Drag from grid → tray: create on empty tray (`data-collection-drop-new`) or add to `data-collection-drop-id`.

## Verify

- `cargo check` / `pnpm --dir src-vite build`
- Manual: create/rename/clear/delete collection; drag photos onto tray; open collection view; switch library isolation
