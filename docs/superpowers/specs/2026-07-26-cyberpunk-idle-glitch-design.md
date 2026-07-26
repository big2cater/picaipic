# Cyberpunk idle glitch theme — design

- **Date:** 2026-07-26  
- **Status:** Draft for implementation (user-approved architecture + FX in brainstorm)  
- **Branch context:** Parallel to black-hole idle theme on `feat/black-hole-idle-theme` lineage  
- **Owner product choices:** trigger = maximize + 6s idle (same as BH); photo-area only; daily UI = dark + neon accents; continuous glitch until activity; approach = parallel `PhotoGlitchLayer` (not unified FX layer)

---

## 1. Goal

Add a fifth app theme **Cyberpunk** that:

1. Everyday: pins dark UI with light magenta/cyan neon chrome accents (no full-window glitch while working).  
2. Idle: after **main window maximized + ~6s idle**, freezes the **photo-area** thumbnail grid into a WebGL texture and runs a **continuous FragCoord-style glitch** until user activity / unmaximize / theme leave.  
3. Does **not** break the black-hole path (`PhotoVortexLayer` remains independent).

Non-goals for v1:

- Full-window chrome glitch overlay  
- CSS-only live-DOM glitch on virtualized cards  
- Disk persistence / intensity presets beyond existing `dynamicThemeIntensity`  
- New daisyUI theme pack beyond `data-theme=dark` + CSS accents  

---

## 2. Product contract

| Step | Behavior |
|------|----------|
| 1 | Settings theme menu: **Default / Retro / CMYK / Black hole / Cyberpunk** |
| 2 | Select **Cyberpunk** → `data-theme=dark`; dual-pin light/dark theme slots to cyberpunk id (same residual-slot hygiene as black hole) |
| 3 | **Maximize** main window only (`uiStore.isMaximized` from Home TitleBar — Settings maximize must not set this) |
| 4 | Idle **6s** (`useIdle(6000)`): no mouse/key/scroll/wheel/touch; document visible; empty `inputStack`; not library-switching; not `prefers-reduced-motion` |
| 5 | Photo area: capture visible thumbs → `PhotoGlitchLayer` continuous glitch; hide live grid after capture |
| 6 | Any activity / unmaximize / leave theme / reduced motion → clear layer, show grid |

Chrome (TitleBar, left rail, status) stays interactive; glitch layer is `pointer-events-none` and photo-region only.

---

## 3. Architecture

```
Home.vue
  useIdle(6000)
  blackHoleThemeOn / cyberpunkThemeOn
  bhFxActive  = BH && max && idle && !rm && !hidden && ...
  cpFxActive  = CP && max && idle && !rm && !hidden && ...
  provide('bhGravityActive', bhFxActive)   // existing name — keep for BH
  provide('cpGlitchActive', cpFxActive)    // new
  optional Cyberpunk ambient (CSS only; no second full-screen WebGL cosmos required for v1)

GridView.vue
  PhotoVortexLayer  when BH inject present / active
  PhotoGlitchLayer  when CP inject present / active   // NEW
  vortexHidesGrid || glitchHidesGrid → opacity-0 on VirtualScroll
```

**Theme ids** (`utils.ts`):

```ts
THEME_ID = {
  DEFAULT: 0,
  RETRO: 1,
  CMYK: 2,
  BLACK_HOLE: 3,
  CYBERPUNK: 4,
}
```

- `clampThemeId` max = 4  
- `isCyberpunkTheme(appearance, light, dark)` mirrors `isBlackHoleTheme`  
- `setTheme`: cyberpunk **and** black hole both force `data-theme=dark`  
- Settings dual-pin: selecting cyberpunk sets both light/dark slots to 4; leaving clears residual slot like BH  

**Mutual exclusion:** only one of BH / CP can be selected as current theme id, so both FX layers never run together. Still gate each layer on its own theme flag.

---

## 4. PhotoGlitchLayer

### 4.1 Lifecycle (mirror PhotoVortex)

| Event | Action |
|-------|--------|
| `active` false → true | `requestAnimationFrame` then `captureSource(sourceEl)` → upload texture → `emit('captured')` → start paint loop |
| paint loop | continuous; no `u_progress` absorb; time-driven glitch only |
| `active` true → false | cancel rAF, delete texture, `emit('cleared')` |
| unmount | same as clear |

Capture implementation: **copy the proven path from `PhotoVortexLayer.captureSource`** (draw decoded `<img>` rects into 2D canvas, DPR clamp ≤1.5, skip incomplete/cross-origin). Optional tiny shared helper only if duplication hurts; **not** a required refactor for v1.

### 4.2 Shader

Port user FragCoord shader to **WebGL1** mediump fragment (same constraints as vortex):

- Inputs: `u_res` (drawing buffer), `u_time` (seconds), `u_tex` (frozen photo), `u_intensity` (0–1.5 from settings)  
- Effects (scale by intensity): random full-frame glitch flag, horizontal line displace, block row displace, small rotation under glitch, RGB chromatic aberration, scanlines, cyan lift, grain, rare invert  
- Output: `gl_FragColor`  
- Sampling: `texture2D`  
- **Do not** use external `picsum` URL — texture is always the captured library thumbs  

Vertex: full-screen triangle strip + `v_uv` like vortex.

### 4.3 Intensity mapping

`dynamicThemeIntensity` already 0 / 0.5 / 1 / 1.5:

| Control | Scale |
|---------|--------|
| line/block displace amplitude | × intensity |
| CA amount | × intensity |
| grain | × intensity |
| invert probability threshold | slightly easier at higher intensity (still rare) |
| 0 | capture optional still allowed but effect nearly static (or skip layer if intensity===0 — **prefer skip glitch layer when intensity is 0**, chrome accents may remain) |

### 4.4 Grid hide

Same as vortex: after `captured`, set hide flag; on `cleared` or inactive, clear flag. Do not drive CSS `useGravityWarp` for cyberpunk.

---

## 5. Chrome / CSS

When `cyberpunkThemeOn && !reducedMotion` (static accents allowed under reduced motion without pulse):

- Home shell: transparent html/body optional only if a neon backdrop is used; **v1 may keep solid dark** and only accent glass rails  
- Left panel / status: existing glass under BH patterns reused with **magenta edge + cyan wash** CSS variables, e.g. `--cp-magenta`, `--cp-cyan`  
- TitleBar: `z-50`, no glitch  
- Appearance control: locked / hint copy like BH (“color mode locked under Cyberpunk”)  

No second analytical cosmos WebGL required for v1 (unlike BH). Optional subtle CSS grid/scan on photo **chrome border** only — not full photo-area CSS glitch.

---

## 6. i18n & settings

- Theme option label: `cyberpunk` / `赛博朋克`  
- Hint: maximized + ~6s idle → photo glitch (reversible)  
- Reduced-motion message parallel to BH  
- Intensity control remains shared; show under cyberpunk as under BH  

---

## 7. Files (expected touch list)

| File | Change |
|------|--------|
| `utils.ts` | THEME_ID.CYBERPUNK, clamp, isCyberpunkTheme, setTheme dark pin, migrate clamp |
| `configStore.js` | comment theme index range |
| `Settings.vue` | menu option, dual-pin, appearance lock, hints |
| `Home.vue` | cyberpunkThemeOn, cpFxActive, provide, optional shell/chrome class |
| `GridView.vue` | mount PhotoGlitchLayer, hide grid on capture |
| `PhotoGlitchLayer.vue` | **new** |
| `app.css` | neon chrome vars/classes |
| `en.json` / `zh.json` | strings |
| `.mex/patterns/change-black-hole-theme.md` or new `change-cyberpunk-theme.md` | runbook |
| `docs/guide/...` progress note | brief |

---

## 8. Error / edge cases

| Case | Behavior |
|------|----------|
| WebGL unavailable | no glitch layer (log once); theme chrome still works |
| Capture draws 0 images | keep dark clear; still may show scan/grain only or abort capture (prefer abort + no hide grid) |
| Virtual scroll during freeze | grid hidden after capture — user cannot scroll photos until activity clears (same as BH) |
| Switch BH ↔ CP while idle | leave theme clears old layer; new theme must re-satisfy idle window |
| Reduced motion | no PhotoGlitchLayer; no pulse animations |
| Library switch | cpFxActive false via existing switching flag |

---

## 9. Testing

- Unit/smoke: theme clamp includes 4; isCyberpunkTheme true only for id 4  
- Manual: CP theme chrome accents; max+6s glitch; activity clears; unmaximize clears; leave theme clears; Settings maximize does not trigger; reduced motion off; intensity 0 vs 1.5 visible difference  
- Regression: BH path still works unchanged  

---

## 10. Implementation order (for plan skill)

1. Theme id + setTheme + Settings dual-pin + i18n  
2. Home gate + provide + chrome CSS  
3. PhotoGlitchLayer (capture + shader + loop)  
4. GridView wire-up + hide grid  
5. Intensity + reduced motion + mex/docs  
6. Manual QA checklist  

---

## 11. Open items (resolved in brainstorm)

| Item | Decision |
|------|----------|
| Maximize required? | **Yes** (option A) |
| Effect region | **Photo area only** |
| Daily look | **Dark + neon accents** |
| Timeline | **Continuous until activity** |
| Code structure | **Parallel PhotoGlitchLayer (A)** |
