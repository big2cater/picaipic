# Cyberpunk Theme Audit (CP-1 .. CP-4)

Scope: the **Cyberpunk** theme (`THEME_ID.CYBERPUNK = 4`) in `src-vite`. Read-only audit of
theme wiring, runtime FX lifecycle, and cross-view visual consistency. NOT a data-destructive path.

> Status (2026-07-30): CP-1 and CP-2 resolved; CP-3 and CP-4 confirmed intentional.

> Single source of truth for theme ids: `src-vite/src/common/utils.ts` `THEME_ID` / `clampThemeId`
> / `isCyberpunkTheme` / `isBlackHoleTheme` / `forcesDarkDataTheme` MUST stay in sync with
> `scripts/check_theme_ids.mjs` (asserts `THEME_ID`, clamp bounds, predicates). Verified in sync.

## How the Cyberpunk theme is wired (intentional, not a bug)
- daisyUI `themes:` in `src-vite/src/assets/app.css:5` is `light --default, dark, retro, coffee, cmyk`.
  **`cyberpunk` (and `black-hole`) are NOT registered as daisyUI schemes.**
- `setTheme(appearance, themeId)` (`utils.ts:90-102`): for `id === BLACK_HOLE || id === CYBERPUNK`
  it forces `document.documentElement.setAttribute('data-theme', 'dark')` and returns early.
  So Cyberpunk reuses the **dark** daisyUI variable set; its identity comes from:
  1. global `html.is-cyberpunk` neon chrome accents (`app.css`), and
  2. `CyberpunkBackground.vue` city-grid canvas ambient (`Home.vue:29`, `v-if="showCyberpunkBackdrop"`).
- `configStore.setLightTheme`/`setDarkTheme` clamp to `0..4` (configStore.js:247-255), matching
  `check_theme_ids.mjs` upper bound 4. No out-of-range id possible.

## Findings

### CP-1 — Resolved: neon chrome accents now render across all views
- `app.css:54-69` defines `.cp-shell` (neon `--fallback-p` on `.btn-primary`/`.bg-primary`, neon
  `.text-primary`). The class is applied **only** to the Home root container
  (`Home.vue:8`: `showCyberpunkChrome ? 'cp-shell' : ''`).
- A repo-wide search for `cp-shell` returns only `Home.vue:8` + `app.css`. ImageViewer, ImageEditor,
  Settings, ManageLibraries, etc. never add `.cp-shell`.
- Consequence: under the Cyberpunk theme, `Home` shows neon-magenta primary buttons / neon text,
  but every other view falls back to the plain daisyUI **dark** primary — the theme looks different
  per view. The user perceives "Cyberpunk" in the library but a normal dark UI when opening a photo.
- Resolution: `setTheme` now toggles `is-cyberpunk` on `<html>` and clears it for every other theme.
  Global CSS defines the actual daisyUI 5 `--color-primary` / `--color-primary-content` variables
  plus cyan `.text-primary`; the obsolete `--fallback-p` override and Home-only `.cp-shell` class
  were removed. ImageViewer, ImageEditor, Settings, and other routes now inherit the same chrome.

### CP-2 — Resolved: PhotoGlitchLayer stops scheduling when inactive
- `PhotoGlitchLayer.vue:302-323` `paint(ts)` does `raf = requestAnimationFrame(paint)` on its **first
  line**, then `if (!gl || !program || !hasTexture || !props.active) return;`. When `props.active`
  is false the loop keeps scheduling one frame per rAF tick (each tick just returns) until the
  `watch(active)` handler calls `endSession()` which `cancelAnimationFrame(raf)`.
- Functionally it stops (Vue `watch` flush precedes the next frame), but there is a 1–2 frame
  empty-spin window and the scheduling depends on watch timing rather than an explicit guard.
- Resolution: `paint` marks the consumed frame id as zero, checks GL/program/texture/active first,
  and schedules the next frame only when all rendering prerequisites remain valid. The watcher and
  unmount cleanup still cancel pending capture and paint frames.

### CP-3 — No dedicated Cyberpunk daisyUI palette (design note, NOT a bug)
- By design Cyberpunk is `dark` + `.cp-shell` + `CyberpunkBackground`. Any requirement for an
  independent Cyberpunk daisyUI color palette cannot be satisfied without registering a new scheme.
  Intentional (`utils.ts:92` comment `BH + CP: force dark`). Documented here to pre-empt false reports.

### CP-4 — City backdrop only mounted in Home (intentional, NOT a bug)
- `CyberpunkBackground` remains imported and rendered **only** in `Home.vue`. Sub-views (ImageViewer,
  ImageEditor, Settings) deliberately receive global neon chrome but no city canvas. This preserves
  clean photo inspection/editing surfaces and avoids an ambient animation/context in auxiliary views.

## Verified SAFE (do not regress)
- Theme-id consistency: `utils.ts` ↔ `check_theme_ids.mjs` ↔ `configStore` clamp (`0..4`).
- `CyberpunkBackground.onUnmounted` (333-341): `stop()` cancels rAF + clears canvas, `ro.disconnect()`,
  `particleSprites=[]`, `rainSprite=null`, `glyphSpriteCache.clear()`, `ctx=null`. Glyph cache key is
  `${ch}:${hue}` over a finite glyph set → bounded, no leak. `watch(animate)` start/stop correct.
- `PhotoGlitchLayer` GL lifecycle: `onUnmounted` → `disposeGl` deletes program/buffer/texture,
  disconnects source `ResizeObserver`. `captureSource` snapshot canvas is local and GC-eligible after
  `uploadTexture`. WebGL init guarded (falls back to `ready=false` if context/shader fails).
- `showCyberpunkChrome` / `showCyberpunkBackdrop` / `showFxShell` (Home.vue:358-361) all derive from
  `cyberpunkThemeOn`/`blackHoleThemeOn` → no backdrop/chrome desync within Home.
- Shader is mediump-safe (`mod(u_time,64.0)`, no `sin(dot)*43758`), avoids long-idle overflow.

## Verification (2026-07-30)
- `node scripts/check_theme_ids.mjs` — passed.
- `pnpm --dir src-vite build` — passed; compiled CSS contains `html.is-cyberpunk` with the daisyUI 5
  primary variables.
- Static lifecycle check: `PhotoGlitchLayer.paint` has no scheduling path before its active/render
  prerequisite guard.
- `git diff --check` — passed.
