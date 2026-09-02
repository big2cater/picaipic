---
name: change-cyberpunk-theme
description: Cyberpunk theme menu, dual-pin dark + neon chrome, idle photo glitch (PhotoGlitchLayer), intensity gate.
last_updated: 2026-07-30
---

# Change cyberpunk idle theme / photo glitch

## When to use
- Theme menu Default / Retro / CMYK / Black hole / **Cyberpunk**
- Neon chrome accents under cyberpunk (daily UI; no full-window glitch while working)
- Idle photo-area glitch timing / capture / continuous WebGL1 loop
- `dynamicThemeIntensity` or appearance lock under cyberpunk (shared FX-theme path with black hole)
- Home `cpFxActive` / provide `cpGlitchActive` / GridView intensity + **theme-gated mount**

## Current product path (2026-07-30)

| Piece | Location | Notes |
|-------|----------|-------|
| Theme ids | `utils.ts` `THEME_ID.CYBERPUNK=4`, `clampThemeId`, `isCyberpunkTheme`, `setTheme` | Cyberpunk pins `data-theme=dark` and toggles global `html.is-cyberpunk` |
| Config | `configStore.js` light/dark theme max **4**; `utils.ts` migration | Intensity 0/0.5/1/1.5; missing/invalid legacy intensity becomes 1 while explicit 0 stays off |
| Settings dual-pin | `Settings.vue` | BH **or** CP dual-pins light+dark slots; appearance locked under FX themes |
| Idle | `useIdle.ts` default **6000** ms; `Home.vue` `useIdle(6000)` | Shared with black hole |
| FX gate | `Home.vue` `cpFxActive` | Byte-for-byte mirror of `gravityActive` with `cyberpunkThemeOn`; Home directly queries/listens to its native maximize state; provide as `cpGlitchActive` |
| Daily ambient | `CyberpunkBackground.vue` | Night-city CSS + canvas rain/particles/kana via **pre-baked sprites** (no per-frame `shadowBlur` / gradients); `resize` → `seedField(false)` so drag-resize does not reseed/jump; static if reduced-motion |
| Chrome | `app.css` `html.is-cyberpunk`; Home glass rails; `StatusBar` / `Content` FX glass | Global daisyUI 5 magenta primary + cyan primary text across routes; city glass remains Home-only |
| **Photo effect** | `PhotoGlitchLayer.vue` + `GridView.vue` | Freeze thumbs → continuous glitch; **mediump-safe** hash + `mod(time)`; capture/canvas sizes clamp to GPU limits; upload/capture/context failures activate CSS card glitch without hiding photos |
| Mount gate | `GridView.vue` | `v-if="cyberpunkThemeOn && inject present"` — do **not** keep GL layer mounted on other themes |
| Intensity gate | `GridView.vue` `glitchLayerActive` | `cpGlitchActive && intensity > 0` on `:active` only — **not** inside `cpFxActive` |

## Glitch UX contract
1. Select theme **赛博朋克 / Cyberpunk**
2. **Maximize** main window (`uiStore.isMaximized` — Home owns native sync; Home TitleBar also refreshes it)
3. Idle **6s** without mouse/key/scroll/wheel/touch
4. Photo area only: freeze → `PhotoGlitchLayer` continuous glitch until activity
5. Any activity / unmaximize / leave theme / reduced motion → clear layer, show grid again

## Do not
- Mount glitch on Settings / ImageEditor / App root
- Mount `CyberpunkBackground` outside Home; global chrome does not imply a global city canvas
- Break black-hole path (`PhotoVortexLayer` / `gravityActive` remain independent)
- Hide grid on empty capture (no thumbs drawn → keep live grid)
- Fold intensity into the seven-way `cpFxActive` computed (gate intensity only at GridView layer active)
- Unify with `PhotoVortexLayer` in v1
- Use `fract(sin(dot)*43758)` style hash under **mediump** (overflows ±2^14 → frozen/NaN grain)
- Call `getBoundingClientRect` every paint frame (use ResizeObserver cache)
- Per-frame canvas `shadowBlur` / `createLinearGradient` for ambient rain/particles (bake sprites)
- `seedField(true)` on every ResizeObserver tick (use `seedField(false)` — only rebuild when count thresholds change)
- Leave untracked capture rAF from `beginSession` (store id; cancel in `endSession`)
- Schedule the next glitch paint before checking GL/program/texture/active prerequisites
- Hide the live grid after an empty/failed capture, or leave constrained GPUs with no photo effect; use the CSS fallback

## Design docs
- Spec: `docs/superpowers/specs/2026-07-26-cyberpunk-idle-glitch-design.md` (incl. WebGL1 port + mediump notes)
- Plan: `docs/superpowers/plans/2026-07-26-cyberpunk-idle-glitch-impl.md`
- Cross-machine/runtime guide: `docs/guide/fx-theme-runtime-compatibility.md`
- Sibling BH runbook: `patterns/change-black-hole-theme.md`

## Verify
- `node scripts/check_theme_ids.mjs` → `check_theme_ids: ok`
- Theme on: night-city ambient visible (rain/particles/kana when motion allowed); no multi-second jank
- Theme switch off cyberpunk: ambient + glitch **unmount**, grid visible, no residual FX
- Every route: `<html>` has `is-cyberpunk` only while theme id 4 is active; primary controls use the
  global magenta variable and `.text-primary` uses cyan
- Long idle glitch (minutes): grain/scan still animate (mediump-safe hash + `mod(time)`)
- High-DPI / low-limit GPU: capture and viewport buffers stay within `MAX_TEXTURE_SIZE` / `MAX_VIEWPORT_DIMS`
- WebGL unavailable, thumbnail capture blocked, upload error, or context loss: live cards show CSS translation/color/RGB-edge glitch
- Settings window maximize does not set main `uiStore.isMaximized`
- Restored/maximized window on another machine: Home's initial native query and resize listener keep `uiStore.isMaximized` correct even if TitleBar timing differs
- Legacy settings without `dynamicThemeIntensity`: startup migration writes intensity 1; an explicit saved 0 remains disabled
- Reduced motion: no glitch; ambient may stay static (no canvas loop)
- Intensity `0`: idle gate may still be true; glitch layer stays inactive
- Manual QA: maximize + 6s idle → photo glitch; activity clears; black hole path still works
