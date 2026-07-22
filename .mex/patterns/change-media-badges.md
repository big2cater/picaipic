---
name: change-media-badges
description: Configurable thumbnail media-info badges (format, ISO, shutter, aperture, focal, exposure).
last_updated: 2026-07-22
---

# Change thumbnail media badges

## When to use
- Add/remove media badge kinds on thumbnails
- Change badge layout/priority or density caps
- Wire settings for overlay capture metadata

## Key files
| Layer | Path |
|-------|------|
| Settings state | `src-vite/src/stores/configStore.js` `settings.grid.mediaBadges` |
| Settings UI | `src-vite/src/views/Settings.vue` View tab |
| Sync | `src-vite/src/main.js` `settings-gridMediaBadges-changed` |
| Render | `src-vite/src/components/Thumbnail.vue` `mediaInfoBadges` |
| i18n | `src-vite/src/locales/en.json` / `zh.json` under `settings.view.media_*` |

## Behaviour contract
1. Default all badge flags **off** (non-intrusive).
2. Supported flags: `format`, `iso`, `shutter`, `aperture`, `focal`, `exposure`.
3. Render bottom-left overlays; cap at **4** badges per thumb to protect dense grids.
4. Prefer `format_label`; fallback extension / RAW.
5. Capture values use `formatCaptureSettingValue`.
6. Status badges (favorite/rating/tags) stay top-left; LIVE stays top-right.
7. Older persisted configs without `mediaBadges` are normalized on access (read-only fallback in Settings UI; do not mutate pinia inside a computed getter).
8. **`setGridMediaBadges` must no-op when the six flags are unchanged.** Settings deep-watches `mediaBadges` and emits; **both main and settings** load `main.js` and listen — replacing the object every time causes an infinite emit/apply loop (UI flicker / hang).

See also `patterns/settings-cross-window-sync.md` (hydrate gate + object equal-noop).

## Verify
```bash
pnpm --dir src-vite build
```
Manual: Settings → View → enable Format + ISO → grid shows badges; disable → gone; toggle repeatedly without freeze/thrash.
