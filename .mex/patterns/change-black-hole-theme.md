---
name: change-black-hole-theme
description: Black-hole theme menu, cosmos WebGL background, idle gravity PhotoVortex, chrome stacking, intensity.
last_updated: 2026-07-30
---

# Change black-hole idle theme / photo vortex

## When to use
- Theme menu Default / Retro / CMYK / Black hole / Cyberpunk (BH is id 3)
- Cosmos background, gravity idle timing, photo-area warp
- TitleBar / sidebar glass under black hole
- `dynamicThemeIntensity` or appearance lock under black hole

## Current product path (2026-07-30)

| Piece | Location | Notes |
|-------|----------|-------|
| Theme ids | `utils.ts` `THEME_ID`, `setTheme`, `isBlackHoleTheme` | Black hole pins `data-theme=dark` (with cyberpunk early-return) |
| Config | `configStore.js` `lightTheme`/`darkTheme`/`dynamicThemeIntensity`; `utils.ts` migration | Intensity 0/0.5/1/1.5; missing/invalid legacy intensity becomes 1 while explicit 0 stays off |
| Idle | `useIdle.ts` default **6000** ms; `Home.vue` `useIdle(6000)` | Was 15s |
| Gravity gate | `Home.vue` `gravityActive` | black hole + native-window maximized + idle + !reducedMotion + !docHidden + empty inputStack + !library switch; Home queries/listens to its own Tauri window state |
| Cosmos | `BlackHoleBackground.vue` | WebGL full-res ~30fps; Canvas2D fallback; `radii` emit only when Δ > 0.5px |
| **Photo effect** | `PhotoVortexLayer.vue` + `GridView.vue` | Freeze thumbs → UV spiral; **theme-gated mount**; capture/canvas dimensions clamp to GPU texture/viewport limits; upload errors and context loss fail visibly instead of accepting a black texture |
| CSS warp fallback | `useGravityWarp.ts`, `blackHoleMath.ts`, `.bh-card` CSS | GridView activates per-card spiral when WebGL init, thumbnail capture, texture upload, or context retention fails; normal WebGL capture keeps it off |
| Chrome | `Home.vue` / `Content.vue` / `StatusBar.vue` | TitleBar z-50; left z-20; translucent bars under BH |
| i18n | `black_hole_theme_hint` etc. | Mentions ~6s idle |

## Gravity UX contract
1. Select theme **黑洞**
2. **Maximize** main window
3. Idle **6s** without mouse/key/scroll/wheel/touch
4. Photo area only: freeze → WebGL absorb (cinematic τ≈12s in layer)
5. Any activity / unmaximize → clear vortex, show grid again

## Do not
- Mount cosmos on Settings / ImageEditor / App root
- Let card `z-index` escape content stacking (left chrome must stay clickable)
- Walk ancestors setting `overflow: visible` during gravity (broke sidebar)
- Run WebGL and CSS photo warps at the same time; CSS is failure fallback only
- Leave a one-shot capture rAF untracked, or keep the Home growth rAF alive while `gravityActive` is false

## Design docs
- Baseline: `docs/guide/black-hole-idle-theme-design.md` (v1.4 + 2026-07-26 现状 table)
- Cross-machine/runtime guide: `docs/guide/fx-theme-runtime-compatibility.md`
- CSS warp / intensity (historical + still useful for math): `docs/superpowers/specs/2026-07-25-black-hole-distortion-and-color-decouple-design.md`
- v1.3 idle plan (blackHoleMode boolean) **deleted** — do not reintroduce
- Sibling cyberpunk: `patterns/change-cyberpunk-theme.md`

## Verify
- Theme switch off black hole: cosmos unmounts, vortex component unmounts, no residual transforms/opacity on cards
- Settings window maximize does not set main `uiStore.isMaximized`
- Reduced motion: no cosmos / no vortex
- High-DPI / low-limit GPU: capture and viewport buffers stay within `MAX_TEXTURE_SIZE` / `MAX_VIEWPORT_DIMS`
- WebGL unavailable, tainted protocol capture, upload error, or context loss: live thumbnails remain visible and CSS card warp starts
- Restored/maximized window on another machine: Home's initial native query and resize listener keep `uiStore.isMaximized` correct even if TitleBar timing differs
- Legacy settings without `dynamicThemeIntensity`: startup migration writes intensity 1; an explicit saved 0 remains disabled
- Manual QA: 6s idle vortex; chrome still interactive
