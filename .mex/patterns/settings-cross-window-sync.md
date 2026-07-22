---
name: settings-cross-window-sync
description: Safe Pinia settings sync between main and settings webviews (emit/listen, hydrate gate, object setters).
last_updated: 2026-07-22
---

# Settings cross-window sync

## Context

Main and settings are **separate** Tauri webviews. Each boots `main.js` with its own Pinia + `listen('settings-*-changed')`. Settings.vue `watch`es pinia and `emit`s those events so the main window can apply changes.

This is fragile if:
1. Settings **opens** and watchers fire for every current value → event fan-out storm.
2. An **object** setting is deep-watched and the main/settings listener always **replaces** the object → infinite emit/apply loop (mediaBadges hang).

## Rules

1. **Hydrate gate** — `settingsHydrating` stays true until after `onMounted` work + `nextTick`. All cross-window settings emits go through `emitSettings()` which no-ops while hydrating. User-driven changes after that still sync.
2. **Scalars** — store setters assign primitives; Vue skips re-fire when equal. OK for simple watch → emit → set.
3. **Objects** (e.g. `grid.mediaBadges`) — setter **must no-op when logical value unchanged** (compare fields, do not assign a new object). Prefer emitting a plain snapshot, not shared identity.
4. **Never** mutate pinia inside a **computed getter** (side effects re-trigger watches).
5. **Do not** register the same `listen` twice in `main.js`.
6. Full shared-store (single pinia) is a larger redesign; keep emit/listen until then.

## Touchpoints

| Layer | Path |
|-------|------|
| Settings watches + hydrate | `src-vite/src/views/Settings.vue` (`settingsHydrating`, `emitSettings`) |
| Listeners | `src-vite/src/main.js` |
| Object setter example | `configStore.setGridMediaBadges` |
| Pattern history | `patterns/change-media-badges.md` |

## Verify

- Open Settings: main window should not thrash / re-apply every setting.
- Toggle media badges repeatedly: no freeze/loop.
- Change language/scale/collections: main updates after user action only.
