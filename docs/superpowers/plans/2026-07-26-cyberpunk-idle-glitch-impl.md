# Cyberpunk Idle Glitch Implementation Plan

> **Status update (2026-07-30): Completed.** The shipped implementation also
> includes changes beyond the original checklist: Home-owned native maximize
> synchronization, legacy intensity migration, GPU texture/viewport clamping,
> WebGL upload/context failure detection, and a CSS live-card glitch fallback.
> Current behavior is documented in
> `docs/guide/fx-theme-runtime-compatibility.md` and
> `.mex/patterns/change-cyberpunk-theme.md`. The unchecked boxes below are the
> historical execution plan, not remaining work.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add theme **Cyberpunk** (`THEME_ID=4`): dark + neon chrome daily; maximize + 6s idle freezes the photo grid into a continuous WebGL1 glitch layer until activity.

**Architecture:** Parallel to black-hole. Home provides pure `cpGlitchActive` (seven-way gate). GridView mounts new `PhotoGlitchLayer.vue` (capture + FragCoord-style glitch shader, WebGL1). Intensity gated only on GridView `:active`. No unify with `PhotoVortexLayer`. Spec: `docs/superpowers/specs/2026-07-26-cyberpunk-idle-glitch-design.md`.

**Tech Stack:** Vue 3 + Pinia + WebGL1 (`webgl` context) + daisyUI `data-theme=dark` + CSS neon accents.

---

## File map

| File | Responsibility |
|------|----------------|
| `src-vite/src/common/utils.ts` | `THEME_ID.CYBERPUNK=4`, clamp, `isCyberpunkTheme`, `setTheme` early-return with BH |
| `scripts/check_theme_ids.mjs` | Node smoke for pure theme id helpers (new) |
| `src-vite/src/stores/configStore.js` | `setLightTheme`/`setDarkTheme` max 4; comment |
| `src-vite/src/locales/zh.json` / `en.json` | Theme label + hints + appearance locked |
| `src-vite/src/views/Settings.vue` | Dual-pin BH+CP, appearance lock, hints, `isDynamicTheme` |
| `src-vite/src/views/Home.vue` | `cyberpunkThemeOn`, `cpFxActive`, provide, neon shell class |
| `src-vite/src/components/PhotoGlitchLayer.vue` | **New** capture + WebGL1 glitch loop |
| `src-vite/src/components/GridView.vue` | Mount glitch layer; intensity on `:active`; hide grid |
| `src-vite/src/assets/app.css` | `--cp-*` neon chrome helpers |
| `src-vite/src/components/StatusBar.vue` / `Content.vue` | Optional glass accents if they already special-case BH |
| `.mex/patterns/change-cyberpunk-theme.md` | Runbook |
| `.mex/ROUTER.md` | Current state bullet |

---

### Task 1: Theme IDs + pure helpers + smoke test

**Files:**
- Modify: `src-vite/src/common/utils.ts`
- Create: `scripts/check_theme_ids.mjs`
- Modify: `src-vite/src/stores/configStore.js`

- [ ] **Step 1: Write failing smoke script**

Create `scripts/check_theme_ids.mjs`:

```js
import assert from 'node:assert/strict';

// Keep in sync with src-vite/src/common/utils.ts THEME_ID / clamp / predicates
const THEME_ID = {
  DEFAULT: 0,
  RETRO: 1,
  CMYK: 2,
  BLACK_HOLE: 3,
  CYBERPUNK: 4,
};

function clampThemeId(themeId) {
  const n = Number(themeId);
  if (!Number.isFinite(n) || n < 0 || n > THEME_ID.CYBERPUNK) return THEME_ID.DEFAULT;
  return Math.floor(n);
}

function isBlackHoleTheme(appearance, lightTheme, darkTheme) {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.BLACK_HOLE;
}

function isCyberpunkTheme(appearance, lightTheme, darkTheme) {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.CYBERPUNK;
}

function forcesDarkDataTheme(themeId) {
  const id = clampThemeId(themeId);
  return id === THEME_ID.BLACK_HOLE || id === THEME_ID.CYBERPUNK;
}

assert.equal(clampThemeId(4), 4);
assert.equal(clampThemeId(5), 0);
assert.equal(clampThemeId(-1), 0);
assert.equal(isCyberpunkTheme(1, 0, 4), true);
assert.equal(isCyberpunkTheme(1, 0, 3), false);
assert.equal(isBlackHoleTheme(1, 0, 3), true);
assert.equal(forcesDarkDataTheme(3), true);
assert.equal(forcesDarkDataTheme(4), true);
assert.equal(forcesDarkDataTheme(1), false);
console.log('check_theme_ids: ok');
```

- [ ] **Step 2: Run smoke (expect FAIL until utils updated if you import from utils; with inlined script this PASSes alone)**

Run: `node scripts/check_theme_ids.mjs`  
Expected: `check_theme_ids: ok`

- [ ] **Step 3: Implement utils.ts theme API**

In `src-vite/src/common/utils.ts` replace the theme block with:

```ts
/** Theme menu ids: Default / Retro / CMYK / Black hole / Cyberpunk */
export const THEME_ID = {
  DEFAULT: 0,
  RETRO: 1,
  CMYK: 2,
  BLACK_HOLE: 3,
  CYBERPUNK: 4,
} as const;

const LIGHT_THEMES = ['light', 'retro', 'cmyk', 'light'] as const;
const DARK_THEMES = ['dark', 'coffee', 'cmyk', 'dark'] as const;

export function clampThemeId(themeId: number | undefined | null): number {
  const n = Number(themeId);
  if (!Number.isFinite(n) || n < 0 || n > THEME_ID.CYBERPUNK) return THEME_ID.DEFAULT;
  return Math.floor(n);
}

export function isBlackHoleTheme(
  appearance: number,
  lightTheme: number,
  darkTheme: number,
): boolean {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.BLACK_HOLE;
}

export function isCyberpunkTheme(
  appearance: number,
  lightTheme: number,
  darkTheme: number,
): boolean {
  const id = appearance === 0 ? lightTheme : darkTheme;
  return clampThemeId(id) === THEME_ID.CYBERPUNK;
}

// migrateThemeSettings: keep blackHoleMode migration; clamp already uses new max

export function setTheme(appearance: number, themeId: number) {
  const id = clampThemeId(themeId);
  // BH + CP: force dark; do NOT index LIGHT/DARK arrays (length 4 only)
  if (id === THEME_ID.BLACK_HOLE || id === THEME_ID.CYBERPUNK) {
    document.documentElement.setAttribute('data-theme', 'dark');
    return;
  }
  const theme =
    appearance === 0
      ? LIGHT_THEMES[id] || 'light'
      : DARK_THEMES[id] || 'dark';
  document.documentElement.setAttribute('data-theme', theme);
}
```

- [ ] **Step 4: Bump configStore clamps to 4**

In `src-vite/src/stores/configStore.js`:

```js
// Theme index: 0 default, 1 retro, 2 cmyk, 3 black hole, 4 cyberpunk
// setLightTheme / setDarkTheme:
this.settings.lightTheme = (Number.isFinite(n) && n >= 0 && n <= 4) ? Math.floor(n) : 0;
this.settings.darkTheme = (Number.isFinite(n) && n >= 0 && n <= 4) ? Math.floor(n) : 0;
```

- [ ] **Step 5: Re-run smoke + commit**

```bash
node scripts/check_theme_ids.mjs
git add scripts/check_theme_ids.mjs src-vite/src/common/utils.ts src-vite/src/stores/configStore.js
git commit -m "feat(cyberpunk): THEME_ID=4, clamp, isCyberpunkTheme, setTheme dark pin"
```

---

### Task 2: i18n + Settings dual-pin + intensity UI

**Files:**
- Modify: `src-vite/src/locales/zh.json`
- Modify: `src-vite/src/locales/en.json`
- Modify: `src-vite/src/views/Settings.vue`

- [ ] **Step 1: Append theme labels and strings**

`theme_options_light` / `theme_options_dark` both append `"赛博朋克"` / `"Cyberpunk"` as 5th entry (index 4).

Add:

```json
"cyberpunk_theme_hint": "… maximize + ~6s idle → photo glitch (reversible)",
"cyberpunk_theme_reduced_motion": "…",
"cyberpunk_appearance_locked": "…"
```

(zh/en parallel to black_hole_*).

- [ ] **Step 2: Dual-pin BH + CP residual clear**

Replace `currentTheme` setter residual logic so **both** special themes dual-pin and clear either residual:

```ts
set(value: number) {
  const v = Number(value);
  if (v === THEME_ID.BLACK_HOLE || v === THEME_ID.CYBERPUNK) {
    config.settings.lightTheme = v;
    config.settings.darkTheme = v;
    return;
  }
  const special = (id: number) =>
    id === THEME_ID.BLACK_HOLE || id === THEME_ID.CYBERPUNK;
  if (config.settings.appearance === 0) {
    config.settings.lightTheme = v;
    if (special(Number(config.settings.darkTheme))) config.settings.darkTheme = v;
  } else {
    config.settings.darkTheme = v;
    if (special(Number(config.settings.lightTheme))) config.settings.lightTheme = v;
  }
}
```

- [ ] **Step 3: Appearance lock + hints + dynamic intensity**

```ts
import { …, isBlackHoleTheme, isCyberpunkTheme, THEME_ID } from '@/common/utils';

const isBlackHole = computed(() => isBlackHoleTheme(...));
const isCyberpunk = computed(() => isCyberpunkTheme(...));
const isFxTheme = computed(() => isBlackHole.value || isCyberpunk.value);
const isDynamicTheme = isFxTheme; // intensity under BH and CP

// appearance select: :disabled="isFxTheme"
// locked text: isBlackHole ? black_hole_appearance_locked : cyberpunk_appearance_locked
// hint block: currentTheme === 3 → BH hint; === 4 → CP hint
```

Template sketch:

```vue
<select ... :disabled="isFxTheme" :class="{ 'opacity-50 cursor-not-allowed': isFxTheme }">
...
<span v-if="isFxTheme" class="...">
  {{ isBlackHole ? $t('settings.general.black_hole_appearance_locked') : $t('settings.general.cyberpunk_appearance_locked') }}
</span>
...
<div v-if="currentTheme === THEME_ID.BLACK_HOLE" class="...">{{ $t('...black_hole_theme_hint') }}</div>
<div v-else-if="currentTheme === THEME_ID.CYBERPUNK" class="...">{{ $t('...cyberpunk_theme_hint') }}</div>
```

- [ ] **Step 4: Commit**

```bash
git add src-vite/src/locales/zh.json src-vite/src/locales/en.json src-vite/src/views/Settings.vue
git commit -m "feat(cyberpunk): settings menu, dual-pin, intensity under FX themes"
```

---

### Task 3: Home gate + provide + chrome class

**Files:**
- Modify: `src-vite/src/views/Home.vue`
- Modify: `src-vite/src/assets/app.css` (minimal vars if needed for shell)

- [ ] **Step 1: Import + theme computed**

```ts
import { isBlackHoleTheme, isCyberpunkTheme } from '@/common/utils';

const cyberpunkThemeOn = computed(() =>
  isCyberpunkTheme(
    Number(config.settings.appearance),
    Number(config.settings.lightTheme),
    Number(config.settings.darkTheme),
  ),
);

// cpFxActive: byte-for-byte gravityActive with cyberpunkThemeOn
const cpFxActive = computed(() =>
  cyberpunkThemeOn.value
  && !!uiStore.isMaximized
  && idle.value
  && !reducedMotion.value
  && !docHidden.value
  && uiStore.inputStack.length === 0
  && !isSwitchingLibrary.value
);

provide('cpGlitchActive', cpFxActive);
// keep existing provide('bhGravityActive', gravityActive)
```

- [ ] **Step 2: Neon chrome class on shell (no transparent cosmos required)**

```ts
const showCyberpunkChrome = computed(() => cyberpunkThemeOn.value);
```

Root div classes: add `cp-shell` when `showCyberpunkChrome`.

Left panel glass when cyberpunk (solid dark + magenta edge), e.g.:

```js
...(showCyberpunkChrome
  ? {
      background: 'rgba(8, 6, 14, 0.88)',
      boxShadow: 'inset 0 0 0 1px rgba(255, 43, 214, 0.35)',
    }
  : {}),
```

Do **not** force `bg-transparent` unless you add a backdrop layer (spec v1: solid dark + accents is enough).

- [ ] **Step 3: Commit**

```bash
git add src-vite/src/views/Home.vue
git commit -m "feat(cyberpunk): Home cpFxActive provide and neon shell chrome"
```

---

### Task 4: PhotoGlitchLayer component

**Files:**
- Create: `src-vite/src/components/PhotoGlitchLayer.vue`

- [ ] **Step 1: Scaffold component API**

Props/emits match vortex:

```ts
const props = defineProps<{
  active: boolean;
  sourceEl: HTMLElement | null;
  intensity?: number; // 0.5–1.5 when active; parent skips 0
}>();
const emit = defineEmits<{ captured: []; cleared: [] }>();
```

- [ ] **Step 2: WebGL1 init + VERT (copy vortex)**

```ts
canvas.getContext('webgl', { alpha: false, antialias: false, premultipliedAlpha: false })
// NOT webgl2
```

Vertex shader:

```glsl
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
```

- [ ] **Step 3: FRAG — port user glitch to WebGL1**

Preamble required:

```glsl
precision mediump float;
varying vec2 v_uv;
uniform vec2 u_res;
uniform float u_time;
uniform sampler2D u_tex;
uniform float u_intensity;
```

Port rules (must):

- `texture(...)` → `texture2D(...)`
- `fragColor =` → `gl_FragColor =`
- no `#version 300 es`
- start from `vec2 uv = v_uv` (not `gl_FragCoord` for sampling base)
- scale displace / CA / grain by `max(u_intensity, 0.0)`
- sample only `u_tex` (captured thumbs)

Core effect structure (adapt numbers freely, keep behavior):

```glsl
// rand, Rot as in prototype
// glitch = step(...);
// lineNoise displace uv.x *= intensity
// block displace *= intensity
// small Rot under glitch
// CA: r/g/b sample with ca * intensity
// scanlines, cyan lift, grain * intensity
// rare invert
gl_FragColor = vec4(col, 1.0);
```

- [ ] **Step 4: captureSource with zero-draw abort**

Copy vortex draw loop, but:

```ts
if (drawn === 0) {
  console.warn('PhotoGlitchLayer: no thumbnails drawn into capture');
  return null;
}
return out;
```

`beginSession`:

```ts
requestAnimationFrame(() => {
  if (!props.active || !props.sourceEl) return;
  const snap = captureSource(props.sourceEl);
  if (!snap) return; // do NOT emit captured
  if (!uploadTexture(snap)) return;
  startMs = performance.now();
  ready.value = true;
  emit('captured');
  cancelAnimationFrame(raf);
  raf = requestAnimationFrame(paint);
});
```

- [ ] **Step 5: paint loop**

Continuous while `props.active && hasTexture`:

```ts
gl.uniform2f(uRes, gl.drawingBufferWidth, gl.drawingBufferHeight);
gl.uniform1f(uTime, ts * 0.001);
gl.uniform1f(uIntensity, Number(props.intensity) || 1);
// bind tex unit 0
gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
```

- [ ] **Step 6: endSession / unmount** — mirror vortex (`emit('cleared')`, delete tex/program)

- [ ] **Step 7: Commit**

```bash
git add src-vite/src/components/PhotoGlitchLayer.vue
git commit -m "feat(cyberpunk): PhotoGlitchLayer WebGL1 capture + continuous glitch"
```

---

### Task 5: GridView wire-up

**Files:**
- Modify: `src-vite/src/components/GridView.vue`

- [ ] **Step 1: Import + inject**

```ts
import PhotoGlitchLayer from '@/components/PhotoGlitchLayer.vue';

const cpGlitchActive = inject<Ref<boolean> | ComputedRef<boolean> | null>('cpGlitchActive', null);
const glitchEnabled = computed(() => cpGlitchActive != null);
const glitchHidesGrid = ref(false);

const glitchLayerActive = computed(() => {
  const intensity = Number(config.settings.dynamicThemeIntensity);
  return !!unref(cpGlitchActive) && Number.isFinite(intensity) && intensity > 0;
});

const glitchIntensity = computed(() => {
  const n = Number(config.settings.dynamicThemeIntensity);
  return Number.isFinite(n) ? n : 1;
});
```

- [ ] **Step 2: Template**

```vue
<PhotoGlitchLayer
  v-if="glitchEnabled"
  class="z-20"
  :active="glitchLayerActive"
  :source-el="containerRef"
  :intensity="glitchIntensity"
  @captured="onGlitchCaptured"
  @cleared="onGlitchCleared"
/>
```

VirtualScroll hide class:

```ts
'opacity-0 pointer-events-none': vortexHidesGrid || glitchHidesGrid,
```

Handlers:

```ts
function onGlitchCaptured() { glitchHidesGrid.value = true; }
function onGlitchCleared() { glitchHidesGrid.value = false; }
watch(glitchLayerActive, (on) => { if (!on) glitchHidesGrid.value = false; });
```

- [ ] **Step 3: Commit**

```bash
git add src-vite/src/components/GridView.vue
git commit -m "feat(cyberpunk): GridView PhotoGlitchLayer + intensity active gate"
```

---

### Task 6: CSS neon + StatusBar/Content polish (light)

**Files:**
- Modify: `src-vite/src/assets/app.css`
- Modify: `src-vite/src/components/StatusBar.vue` if it special-cases BH glass
- Modify: `src-vite/src/components/Content.vue` only if BH-only chrome would look wrong under CP

- [ ] **Step 1: CSS variables**

```css
.cp-shell {
  --cp-magenta: #ff2bd6;
  --cp-cyan: #00e5ff;
}
.cp-shell .cp-rail-accent {
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--cp-magenta) 40%, transparent);
}
```

Wire Home left panel / StatusBar with `cp-shell` / existing inline styles from Task 3.

- [ ] **Step 2: StatusBar** — if `isBlackHoleTheme` gates translucency, extend with `isCyberpunkTheme` OR `isFxTheme` helper so CP gets similar glass (cyan bottom edge optional).

- [ ] **Step 3: Commit**

```bash
git add src-vite/src/assets/app.css src-vite/src/components/StatusBar.vue src-vite/src/components/Content.vue
git commit -m "feat(cyberpunk): neon chrome CSS and status/content accents"
```

---

### Task 7: MEX + docs + verification

**Files:**
- Create: `.mex/patterns/change-cyberpunk-theme.md`
- Modify: `.mex/ROUTER.md`
- Modify: `docs/guide/picaipic-progress.md` (one bullet)

- [ ] **Step 1: Pattern runbook** (mirror change-black-hole-theme structure: when to use, files, contract, do-nots, verify)

- [ ] **Step 2: Automated checks**

```bash
node scripts/check_theme_ids.mjs
# Expected: check_theme_ids: ok

# Optional frontend typecheck (may show pre-existing tsconfig deprecations only)
# pnpm --dir src-vite exec tsc --noEmit
```

- [ ] **Step 3: Manual QA checklist**

- [ ] Theme menu shows Cyberpunk (index 4)
- [ ] Select CP → dark chrome + neon accents; appearance locked
- [ ] Intensity control visible; 0 → never freezes grid
- [ ] Maximize + 6s idle → photo glitch continuous
- [ ] Move mouse / unmaximize → grid back
- [ ] Reduced motion → no glitch
- [ ] Empty library / no thumbs → grid **not** hidden
- [ ] Black hole path still works
- [ ] Settings window maximize does **not** trigger main glitch

- [ ] **Step 4: Commit**

```bash
git add .mex/patterns/change-cyberpunk-theme.md .mex/ROUTER.md docs/guide/picaipic-progress.md
git commit -m "docs(mex): cyberpunk theme runbook and progress note"
```

---

## Spec coverage (self-check)

| Spec section | Task |
|--------------|------|
| THEME_ID=4, clamp, isCyberpunk, setTheme early-return | T1 |
| Dual-pin, residual clear BH+CP, i18n, intensity UI | T2 |
| §2.1 seven-way `cpFxActive` pure provide | T3 |
| Neon chrome daily | T3 + T6 |
| PhotoGlitchLayer WebGL1 port + continuous loop | T4 |
| drawn===0 → null abort | T4 |
| GridView intensity on `:active`, hide grid | T5 |
| u_intensity bind without `\|\| 1` | T4 (bind raw number; layer only active when >0) |
| reduced motion / library switch / modal | T3 gates |
| mex/docs/QA | T7 |

No intentional placeholders. No unified PhotoFx refactor (YAGNI).

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-26-cyberpunk-idle-glitch-impl.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — this session, executing-plans with checkpoints  

Which approach?
