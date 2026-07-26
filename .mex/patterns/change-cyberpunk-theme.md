---
name: change-cyberpunk-theme
description: Cyberpunk theme menu, dual-pin dark + neon chrome, idle photo glitch (PhotoGlitchLayer), intensity gate.
last_updated: 2026-07-26
---

# Change cyberpunk idle theme / photo glitch

## When to use
- Theme menu Default / Retro / CMYK / Black hole / **Cyberpunk**
- Neon chrome accents under cyberpunk (daily UI; no full-window glitch while working)
- Idle photo-area glitch timing / capture / continuous WebGL1 loop
- `dynamicThemeIntensity` or appearance lock under cyberpunk (shared FX-theme path with black hole)
- Home `cpFxActive` / provide `cpGlitchActive` / GridView intensity gate

## Current product path (2026-07-26)

| Piece | Location | Notes |
|-------|----------|-------|
| Theme ids | `utils.ts` `THEME_ID.CYBERPUNK=4`, `clampThemeId`, `isCyberpunkTheme`, `setTheme` | Cyberpunk pins `data-theme=dark` (same early-return as black hole) |
| Config | `configStore.js` light/dark theme max **4** | Intensity 0/0.5/1/1.5 via `dynamicThemeIntensity` |
| Settings dual-pin | `Settings.vue` | BH **or** CP dual-pins light+dark slots; appearance locked under FX themes |
| Idle | `useIdle.ts` default **6000** ms; `Home.vue` `useIdle(6000)` | Shared with black hole |
| FX gate | `Home.vue` `cpFxActive` | Byte-for-byte mirror of `gravityActive` with `cyberpunkThemeOn`; provide as `cpGlitchActive` |
| Chrome | `Home.vue` `showCyberpunkChrome` / `cp-shell` | Neon accents; TitleBar stays interactive |
| **Photo effect** | `PhotoGlitchLayer.vue` + `GridView.vue` | Freeze visible thumbs → continuous FragCoord-style glitch (WebGL1); hide grid after capture |
| Intensity gate | `GridView.vue` `glitchLayerActive` | `cpGlitchActive && intensity > 0` only on layer `:active` — **not** inside `cpFxActive` |

## Glitch UX contract
1. Select theme **赛博朋克 / Cyberpunk**
2. **Maximize** main window (`uiStore.isMaximized` — Home TitleBar only)
3. Idle **6s** without mouse/key/scroll/wheel/touch
4. Photo area only: freeze → `PhotoGlitchLayer` continuous glitch until activity
5. Any activity / unmaximize / leave theme / reduced motion → clear layer, show grid again

## Do not
- Mount glitch on Settings / ImageEditor / App root
- Break black-hole path (`PhotoVortexLayer` / `gravityActive` remain independent)
- Hide grid on empty capture (no thumbs drawn → keep live grid)
- Fold intensity into the seven-way `cpFxActive` computed (gate intensity only at GridView layer active)
- Unify with `PhotoVortexLayer` in v1

## Design docs
- Spec: `docs/superpowers/specs/2026-07-26-cyberpunk-idle-glitch-design.md`
- Plan: `docs/superpowers/plans/2026-07-26-cyberpunk-idle-glitch-impl.md`
- Sibling BH runbook: `patterns/change-black-hole-theme.md`

## Verify
- `node scripts/check_theme_ids.mjs` → `check_theme_ids: ok`
- Theme switch off cyberpunk: glitch unmounts, grid visible, no residual FX
- Settings window maximize does not set main `uiStore.isMaximized`
- Reduced motion: no glitch
- Intensity `0`: idle gate may still be true; layer stays inactive
- Manual QA: maximize + 6s idle → photo glitch; activity clears; black hole path still works
