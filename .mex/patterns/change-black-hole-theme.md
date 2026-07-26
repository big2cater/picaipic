---
name: change-black-hole-theme
description: Black-hole theme menu, cosmos WebGL background, idle gravity PhotoVortex, chrome stacking, intensity.
last_updated: 2026-07-26
---

# Change black-hole idle theme / photo vortex

## When to use
- Theme menu Default / Retro / CMYK / Black hole / Cyberpunk (BH is id 3)
- Cosmos background, gravity idle timing, photo-area warp
- TitleBar / sidebar glass under black hole
- `dynamicThemeIntensity` or appearance lock under black hole

## Current product path (2026-07-26)

| Piece | Location | Notes |
|-------|----------|-------|
| Theme ids | `utils.ts` `THEME_ID`, `setTheme`, `isBlackHoleTheme` | Black hole pins `data-theme=dark` (with cyberpunk early-return) |
| Config | `configStore.js` `lightTheme`/`darkTheme`/`dynamicThemeIntensity` | Intensity 0/0.5/1/1.5 |
| Idle | `useIdle.ts` default **6000** ms; `Home.vue` `useIdle(6000)` | Was 15s |
| Gravity gate | `Home.vue` `gravityActive` | black hole + maximized + idle + !reducedMotion + !docHidden + empty inputStack + !library switch |
| Cosmos | `BlackHoleBackground.vue` | WebGL full-res ~30fps; Canvas2D fallback; `radii` emit only when Δ > 0.5px |
| **Photo effect** | `PhotoVortexLayer.vue` + `GridView.vue` | Freeze thumbs → UV spiral; **theme-gated mount**; ResizeObserver size cache (no per-frame reflow); lazy GL init |
| CSS warp (legacy) | `useGravityWarp.ts`, `blackHoleMath.ts`, `.bh-card` CSS | **Not driven** by GridView; keep for rollback |
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
- Claim CSS 6-layer warp is the live path without re-wiring GridView

## Design docs
- Baseline: `docs/guide/black-hole-idle-theme-design.md` (v1.4 + 2026-07-26 现状 table)
- CSS warp / intensity (historical + still useful for math): `docs/superpowers/specs/2026-07-25-black-hole-distortion-and-color-decouple-design.md`
- v1.3 idle plan (blackHoleMode boolean) **deleted** — do not reintroduce
- Sibling cyberpunk: `patterns/change-cyberpunk-theme.md`

## Verify
- Theme switch off black hole: cosmos unmounts, vortex component unmounts, no residual transforms/opacity on cards
- Settings window maximize does not set main `uiStore.isMaximized`
- Reduced motion: no cosmos / no vortex
- Manual QA: 6s idle vortex; chrome still interactive