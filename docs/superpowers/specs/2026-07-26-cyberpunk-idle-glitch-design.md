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
| 4 | Idle gate (see **§2.1**): `useIdle(6000)` only tracks input silence; **full** `cpFxActive` is a Home `computed` that mirrors BH’s seven-way AND |
| 5 | Photo area: capture visible thumbs → `PhotoGlitchLayer` continuous glitch; hide live grid after capture |
| 6 | Any activity / unmaximize / leave theme / reduced motion → clear layer, show grid |

Chrome (TitleBar, left rail, status) stays interactive; glitch layer is `pointer-events-none` and photo-region only.

### 2.1 Idle / FX gate split (do not over-attribute to `useIdle`)

`useIdle.ts` **only** listens to `mousemove | keydown | scroll | wheel | touchstart` and flips `idle` after 6s without those events. It does **not** know about maximize, visibility, dialogs, library switch, or reduced motion.

`cpFxActive` **must** be a byte-for-byte mirror of `Home.vue` `gravityActive`, with only the theme predicate swapped:

```ts
// gravityActive today (BH):
blackHoleThemeOn
  && uiStore.isMaximized
  && idle.value
  && !reducedMotion.value
  && !docHidden.value
  && uiStore.inputStack.length === 0
  && !isSwitchingLibrary.value

// cpFxActive (required):
cyberpunkThemeOn
  && uiStore.isMaximized
  && idle.value
  && !reducedMotion.value
  && !docHidden.value
  && uiStore.inputStack.length === 0
  && !isSwitchingLibrary.value
```

If any of the six non-theme gates are omitted, glitch can fire during library switch, modal `inputStack`, background tab, or reduced-motion users.

**Intensity is not part of `cpFxActive`.** Keep the seven-way computed pure (theme + maximize + idle + rm + hidden + inputStack + library). Apply intensity at the **layer mount / `:active` binding** in `GridView` (or equivalent):

```ts
// provide stays pure:
provide('cpGlitchActive', cpFxActive)

// GridView — read intensity *inside* computed (pinia/settings reactivity):
const glitchLayerActive = computed(() => {
  const intensity = Number(config.settings.dynamicThemeIntensity)
  return !!unref(cpGlitchActive) && Number.isFinite(intensity) && intensity > 0
})
// <PhotoGlitchLayer :active="glitchLayerActive" ... />
// v-if can stay on inject presence; :active carries intensity 0 skip
```

This preserves §2.1 as a BH mirror and still enforces §4.3 / §8 “intensity 0 → no glitch layer / no freeze”.

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

- `clampThemeId` max = **4** (today max is `BLACK_HOLE` / 3 — must bump)  
- `isCyberpunkTheme(appearance, light, dark)` mirrors `isBlackHoleTheme`  
- **`setTheme` dark pin (required, avoids array OOB):**  
  `LIGHT_THEMES` / `DARK_THEMES` today are **length 4** (indices 0–3). Id `4` would be `undefined` and only survive via `|| 'dark'`. **Do not** append a fifth daisyUI pack (non-goal). Instead extend the existing early-return:

  ```ts
  if (id === THEME_ID.BLACK_HOLE || id === THEME_ID.CYBERPUNK) {
    document.documentElement.setAttribute('data-theme', 'dark');
    return;
  }
  // then LIGHT_THEMES[id] / DARK_THEMES[id] for 0..2 only
  ```

- **Dual-pin:** Settings selecting cyberpunk sets both `lightTheme`/`darkTheme` slots to `4` and clears residual BH slot the same way BH clears residual CP; **do not** invent a new `cyberpunkMode` boolean (reuse dual-pin + existing `blackHoleMode` migration only for legacy BH)  
- Settings dual-pin residual-slot hygiene mirrors BH  

**Mutual exclusion:** only one of BH / CP can be selected as current theme id, so both FX layers never run together. Still gate each layer on its own theme flag.

---

## 4. PhotoGlitchLayer

### 4.1 Lifecycle (mirror PhotoVortex, with one intentional divergence)

| Event | Action |
|-------|--------|
| `active` false → true | `requestAnimationFrame` then `captureSource(sourceEl)` → **if null, stop (no emit)** → else upload texture → `emit('captured')` → start paint loop |
| paint loop | continuous; no `u_progress` absorb; time-driven glitch only |
| `active` true → false | cancel rAF, delete texture, `emit('cleared')` |
| unmount | same as clear |

**Capture:** copy the drawing path from `PhotoVortexLayer.captureSource` (decoded `<img>` rects → 2D canvas, DPR clamp ≤1.5, skip incomplete/cross-origin) **with one required change vs vortex:**

```ts
// PhotoVortex today (do NOT copy this branch as-is):
if (drawn === 0) {
  console.warn(...);
  return out; // solid dark canvas — still emits captured / hides grid
}

// PhotoGlitchLayer MUST instead:
if (drawn === 0) {
  console.warn('PhotoGlitchLayer: no thumbnails drawn into capture');
  return null; // beginSession aborts: no upload, no emit('captured'), grid stays visible
}
```

Rationale: glitch without photo content is a frozen black plate that incorrectly hides the live grid (§8). Vortex’s “dark fallback is better than crash” does **not** apply here.

Optional shared helper later is fine; **not** required for v1. If shared, glitch path still needs the `drawn === 0 → null` policy (parameter or wrapper).

### 4.2 Shader (WebGL1 only — prototype is often WebGL2)

**Runtime context must match `PhotoVortexLayer`:**

```ts
canvas.getContext('webgl', { alpha: false, antialias: false, premultipliedAlpha: false })
// → WebGLRenderingContext (WebGL1), NOT webgl2
```

The brainstorm / FragCoord prototype may use GLSL ES 3.00 (`texture()`, `out vec4 fragColor`, injected `u_time`). **Pasting that source into `PhotoGlitchLayer` will fail to compile** under WebGL1. Implementers **must** apply this port checklist:

| WebGL2 / prototype | WebGL1 app (`PhotoGlitchLayer.FRAG`) |
|--------------------|--------------------------------------|
| `#version 300 es` | **delete** (WebGL1 default) |
| `texture(u_tex1, uv)` | `texture2D(u_tex, uv)` |
| `fragColor = vec4(...)` | `gl_FragColor = vec4(...)` |
| `out vec4 fragColor` | **delete** |
| `u_resolution` | **`u_res`** (match vortex JS `getUniformLocation`) |
| `u_tex1` | **`u_tex`** |
| `u_time` | `u_time` (keep; bind from JS `performance.now()` like vortex) |
| *(missing)* | **`uniform float u_intensity`** (new — prototype has hard-coded strength) |
| `gl_FragCoord.xy / u_resolution` | Prefer **`v_uv`** from vertex (same as vortex). “FragCoord-style” is the *look*, not a requirement to sample `gl_FragCoord` (avoids DPR / `UNPACK_FLIP_Y_WEBGL` pitfalls) |

**Required GLSL preamble (fragment):**

```glsl
precision mediump float;
varying vec2 v_uv;
uniform vec2 u_res;
uniform float u_time;
uniform sampler2D u_tex;
uniform float u_intensity;
```

**Vertex (copy vortex):**

```glsl
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
```

**Effects** (all scaled by `u_intensity` where noted): full-frame glitch flag, horizontal line displace, block row displace, small rotation under glitch, RGB chromatic aberration, scanlines, cyan lift, grain, rare invert.

**Do not** fetch external `picsum` — `u_tex` is only the captured library thumbs.

**JS uniforms (must bind every frame / on start):**

| Uniform | Type | Source |
|---------|------|--------|
| `u_res` | `2f` | `drawingBufferWidth/Height` |
| `u_time` | `1f` | seconds (`ts * 0.001` or elapsed) |
| `u_tex` | `1i` | texture unit 0 |
| `u_intensity` | `1f` | `Number(config.settings.dynamicThemeIntensity)` — **do not** use `\|\| 1` (`Number(0)\|\|1 === 1` would mis-map intentional zero; layer already unmounted when intensity is 0, so the uniform is only bound for &gt;0 values) |

Prototype has **no** `u_intensity`: add `getUniformLocation(program, 'u_intensity')` + `uniform1f` on the paint path; in GLSL multiply line/block displace, CA, and grain by `u_intensity` (and optionally ease invert rarity).

### 4.3 Intensity mapping

`dynamicThemeIntensity` already 0 / 0.5 / 1 / 1.5:

| Control | Scale |
|---------|--------|
| line/block displace amplitude | × intensity |
| CA amount | × intensity |
| grain | × intensity |
| invert probability threshold | slightly easier at higher intensity (still rare) |
| **0** | **Do not mount / do not capture** glitch layer (`intensity === 0` → treat as inactive for FX). Avoid a frozen-but-static photo plate. Chrome neon accents may still show. |

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
| `utils.ts` | THEME_ID.CYBERPUNK=4, clamp max 4, isCyberpunkTheme, **setTheme early-return with BH** (do not index LIGHT/DARK arrays for id 4), no new daisyUI entries |
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
| WebGL unavailable / shader compile fail | no glitch layer (log once); theme chrome still works; **never** leave grid hidden without a successful capture |
| Capture draws 0 images | **abort** capture; do **not** emit `captured` / do **not** hide grid |
| Virtual scroll during freeze | grid hidden after capture — user cannot scroll photos until activity clears (same as BH) |
| Switch BH ↔ CP while idle | leave theme clears old layer; new theme must re-satisfy idle window |
| Reduced motion | no PhotoGlitchLayer; no pulse animations on chrome |
| Library switch / modal stack / hidden tab | `cpFxActive` false via the six non-theme gates in §2.1 |
| `dynamicThemeIntensity === 0` | skip glitch layer entirely |

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

## 12. Spec errata (2026-07-26 review pass)

Verified against `PhotoVortexLayer.vue` (WebGL1), `utils.ts` theme arrays, `useIdle.ts`, `Home.vue` `gravityActive`. All of the following are **in force** for implementation:

| Severity | Finding | Spec fix |
|----------|---------|----------|
| 🔴 | Prototype often WebGL2; app is WebGL1 — paste would fail compile | §4.2 port table + preamble |
| 🟠 | `LIGHT_THEMES`/`DARK_THEMES` length 4 → id 4 OOB without early-return | §3 `setTheme` early-return with BH |
| 🟠 | `useIdle` is input-only; other gates live in Home computed | §2.1 seven-way `cpFxActive` |
| 🟡 | Prototype lacks `u_intensity` | §4.2 JS bind + GLSL multiply; §4.3 skip at 0 |
| 🟢 | Prefer `v_uv` over raw `gl_FragCoord` | §4.2 |
| 🟢 | No new `cyberpunkMode` flag | §3 dual-pin only |
| 🟠 | §4.1 “copy captureSource” vs §8 zero-draw abort conflict (vortex returns dark canvas) | §4.1: glitch **must** `return null` when `drawn===0` |
| 🟡 | intensity 0 gate vs “byte-for-byte” cpFxActive | §2.1: intensity on GridView `:active`, not inside provide |
| ⚪ | `Number(x) \|\| 1` maps 0→1 | §4.2: bind raw `Number(...)` only |
