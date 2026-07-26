# Black Hole Idle Theme Implementation Plan

> ## ⚠️ [SUPERSEDED] — 不要再按本计划实现
>
> 本计划基线为 **v1.3**（依赖 `configStore.settings.blackHoleMode` 布尔开关）。但 v1.4 设计已**删除该开关**（改用主题菜单 `themeId===3` / 双钉 `lightTheme=darkTheme=3`），相关代码也已落地 v1.4。
> **照本计划新增 `configStore.settings.blackHoleMode` 持久字段，会直接撞上 `utils.ts` / `main.js` 的遗留迁移逻辑**（`migrateThemeSettings` 会把老 `blackHoleMode:true` 迁成 `themeId=3` 再清零），产生与 v1.4 主题-id 模型冲突的代码。
> 黑洞扭曲增强 + 配色解耦（**v1.5**）的 task-by-task 实施计划见：**`docs/superpowers/plans/2026-07-25-black-hole-distortion-decouple-impl.md`**（基线 v1.4）。
> 本文件仅保留作历史记录与 git 链接，不再作为实现依据。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an opt-in “black hole theme”: centered ambient canvas on the main Home window; when the **main** window is system-maximized and the user is idle 15s, slowly grow the hole and gravity-warp **only GridView** thumbnail outer roots (`.bh-card`), with instant clear/rebound on input or exit conditions.

**Architecture:** Pure frontend. `configStore.settings.blackHoleMode` (persisted) gates mounting. `uiStore.isMaximized` is written **only** by `TitleBar` when `viewName === 'Home'`, with mount init + resize/maximize listeners. `Home.vue` is the sole assembler of `gravityActive` (store + local `isSwitchingLibrary` + idle + reduced-motion + visibility) and provides it downward. `BlackHoleBackground` draws canvas; `useGravityWarp` throttles transforms on `.bh-card` under GridView. No Rust/AI/`data-theme` changes.

**Tech Stack:** Vue 3 + Pinia (`persist: true`) + Tauri 2 window API + Canvas 2D + CSS transitions. Spec: `docs/guide/black-hole-idle-theme-design.md` (v1.3).

**Note on tests:** `src-vite` has no Vitest/Jest. Pure math is verified with a small Node assert script; feature behavior is verified with the manual checklist in Task 10 / design §11.

---

## File map

| File | Responsibility |
|---|---|
| Create `src-vite/src/common/blackHoleMath.ts` | Pure lerp/smoothstep/growth/card transform helpers |
| Create `src-vite/src/composables/useIdle.ts` | 15s idle detection |
| Create `src-vite/src/composables/useGravityWarp.ts` | Throttled DOM transform apply/clear |
| Create `src-vite/src/components/BlackHoleBackground.vue` | Fixed canvas ambient + grow radii |
| Create `scripts/check_black_hole_math.mjs` | Node assert smoke for pure math |
| Modify `src-vite/src/stores/uiStore.js` | Add `isMaximized` + `setMaximized` |
| Modify `src-vite/src/stores/configStore.js` | `settings.blackHoleMode` + setter |
| Modify `src-vite/src/components/TitleBar.vue` | Home-only store sync + init + listeners |
| Modify `src-vite/src/views/Home.vue` | Mount background; assemble/provide `gravityActive` |
| Modify `src-vite/src/components/Content.vue` | Inject + pass `gravityActive` (and radii if needed) to GridView |
| Modify `src-vite/src/components/GridView.vue` | Host `useGravityWarp` |
| Modify `src-vite/src/components/Thumbnail.vue` | Outer root class `bh-card` |
| Modify `src-vite/src/views/Settings.vue` | Appearance toggle + emit |
| Modify `src-vite/src/main.js` | Cross-window settings listener |
| Modify `src-vite/src/locales/en.json` / `zh.json` | Copy keys |
| Optional MEX: `.mex/ROUTER.md` / log after ship | GROW |

**Do not modify as hosts:** `App.vue` (no black hole mount), `MediaViewer.vue` local maximize, Rust.

---

### Task 1: Pure math helpers + Node smoke

**Files:**
- Create: `src-vite/src/common/blackHoleMath.ts`
- Create: `scripts/check_black_hole_math.mjs`

- [ ] **Step 1: Add pure helpers**

Create `src-vite/src/common/blackHoleMath.ts`:

```ts
export function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

export function smoothstep(t: number): number {
  const x = clamp(t, 0, 1);
  return x * x * (3 - 2 * x);
}

/** ~25s → ~95% when tau=8 */
export function growthK(elapsedSec: number, tau = 8): number {
  if (elapsedSec <= 0) return 0;
  return 1 - Math.exp(-elapsedSec / tau);
}

export type HoleRadii = {
  R_event: number;
  R_inf: number;
};

export function computeRadii(
  elapsedSec: number,
  vw: number,
  vh: number,
): HoleRadii {
  const m = Math.min(vw, vh);
  const R_event0 = 0.06 * m;
  const R_eventMax = 0.16 * m;
  const R_inf0 = 0.12 * m;
  const R_infMax = 0.92 * Math.hypot(vw, vh) / 2;
  const k = growthK(elapsedSec);
  return {
    R_event: lerp(R_event0, R_eventMax, k),
    R_inf: lerp(R_inf0, R_infMax, k),
  };
}

export type CardWarp = {
  tx: number;
  ty: number;
  scale: number;
  rotDeg: number;
  blur: number;
  active: boolean;
};

/**
 * Card center (cx,cy), hole center (HX,HY), radii, orbitPhase (radians), swirl (deg contribution).
 */
export function computeCardWarp(
  cx: number,
  cy: number,
  HX: number,
  HY: number,
  R_event: number,
  R_inf: number,
  orbitPhase: number,
  swirl = 12,
): CardWarp {
  const dx = cx - HX;
  const dy = cy - HY;
  const dist = Math.hypot(dx, dy);
  if (!(R_inf > 0) || dist > R_inf) {
    return { tx: 0, ty: 0, scale: 1, rotDeg: 0, blur: 0, active: false };
  }
  const angle = Math.atan2(dy, dx);
  const t = clamp((R_inf - dist) / R_inf, 0, 1);
  const s = smoothstep(t);
  const targetR = lerp(dist, R_event, s);
  const orbit = orbitPhase * (0.2 + 0.8 * t);
  const a2 = angle + orbit;
  const nx = HX + targetR * Math.cos(a2);
  const ny = HY + targetR * Math.sin(a2);
  const tx = nx - cx;
  const ty = ny - cy;
  const scale = lerp(1, 0.45, t);
  const rotDeg = ((a2 - angle) * 180) / Math.PI + swirl * t;
  const blur = t > 0.7 ? lerp(0, 3, (t - 0.7) / 0.3) : 0;
  return { tx, ty, scale, rotDeg, blur, active: true };
}

export function cardWarpCss(w: CardWarp): { transform: string; filter: string } {
  if (!w.active) return { transform: '', filter: '' };
  return {
    transform: `translate(${w.tx.toFixed(2)}px, ${w.ty.toFixed(2)}px) rotate(${w.rotDeg.toFixed(2)}deg) scale(${w.scale.toFixed(4)})`,
    filter: w.blur > 0.05 ? `blur(${w.blur.toFixed(2)}px)` : '',
  };
}
```

- [ ] **Step 2: Add Node smoke script (mirrors formulas; no TS import required)**

Create `scripts/check_black_hole_math.mjs`:

```js
import assert from 'node:assert/strict';

function clamp(n, min, max) { return Math.min(max, Math.max(min, n)); }
function lerp(a, b, t) { return a + (b - a) * t; }
function smoothstep(t) {
  const x = clamp(t, 0, 1);
  return x * x * (3 - 2 * x);
}
function growthK(elapsedSec, tau = 8) {
  if (elapsedSec <= 0) return 0;
  return 1 - Math.exp(-elapsedSec / tau);
}

assert.equal(lerp(0, 10, 0.5), 5);
assert.equal(smoothstep(0), 0);
assert.equal(smoothstep(1), 1);
assert.ok(growthK(0) === 0);
assert.ok(growthK(25) > 0.94 && growthK(25) < 0.96);

// far card inactive when dist > R_inf
const cx = 0, cy = 0, HX = 0, HY = 0, R_event = 50, R_inf = 100;
const distFar = 150;
assert.ok(distFar > R_inf);

// near card t high
const distNear = 10;
const t = clamp((R_inf - distNear) / R_inf, 0, 1);
assert.ok(t > 0.8);

console.log('check_black_hole_math: ok');
```

- [ ] **Step 3: Run smoke**

```bash
node scripts/check_black_hole_math.mjs
```

Expected: `check_black_hole_math: ok`

- [ ] **Step 4: Commit**

```bash
git add src-vite/src/common/blackHoleMath.ts scripts/check_black_hole_math.mjs
git commit -m "feat(black-hole): pure math helpers and smoke check"
```

---

### Task 2: Store fields (`isMaximized`, `blackHoleMode`)

**Files:**
- Modify: `src-vite/src/stores/uiStore.js`
- Modify: `src-vite/src/stores/configStore.js`
- Modify: `src-vite/src/main.js` (listener for settings sync)
- Modify: `src-vite/src/views/Settings.vue` (watch emit only if toggle lands same PR; else Task 8)

- [ ] **Step 1: uiStore — add field + action**

In `state: () => ({ ... })` add:

```js
isMaximized: false, // main Home window system maximize (black-hole gravity gate)
```

In `actions` add:

```js
setMaximized(value) {
  this.isMaximized = !!value;
},
```

- [ ] **Step 2: configStore — default + setter**

In `settings: {` near other general booleans (after `debugMode: false,` is fine) add:

```js
blackHoleMode: false, // opt-in black hole ambient + idle gravity (main grid only)
```

In `actions` near `setDebugMode` / general setters add:

```js
setBlackHoleMode(blackHoleMode) {
  this.settings.blackHoleMode = !!blackHoleMode;
},
```

`persist: true` already persists the whole store — no extra config.

- [ ] **Step 3: Cross-window settings plumbing**

In `src-vite/src/main.js`, after an existing settings listener block, add:

```js
listen('settings-blackHoleMode-changed', (event) => {
  config.setBlackHoleMode(event.payload)
})
```

(Exact import of `config` already exists in `main.js`.)

In `Settings.vue` (can land with Task 8 UI; if doing stores-only commit, skip watch until UI exists). Preferred: land watch with Task 8.

- [ ] **Step 4: Commit**

```bash
git add src-vite/src/stores/uiStore.js src-vite/src/stores/configStore.js src-vite/src/main.js
git commit -m "feat(black-hole): store flags for mode and main maximize"
```

---

### Task 3: TitleBar Home-only maximize sync (non-trivial)

**Files:**
- Modify: `src-vite/src/components/TitleBar.vue`

**Hard constraints (from design):**
- Write `uiStore.setMaximized` **only** when `props.viewName === 'Home'`.
- Settings (`viewName="Settings"`) / ImageEditor (`viewName="ImageEditor"`) must not touch the store.
- MediaViewer keeps its own local ref — do not change MediaViewer.
- Must add **mount init** + **window listeners** (not click-only).

- [ ] **Step 1: Import uiStore and lifecycle**

At top of script, ensure:

```js
import { ref, watch, onMounted, onUnmounted } from 'vue';
import { useUIStore } from '@/stores/uiStore';
```

```js
const uiStore = useUIStore();
const syncMaximizedToStore = props.viewName === 'Home';
let unlistenResize = null;
```

- [ ] **Step 2: Single apply path**

```js
function applyMaximizedState(maximized) {
  const next = !!maximized;
  isMaximized.value = next;
  if (syncMaximizedToStore) {
    uiStore.setMaximized(next);
  }
}

async function refreshMaximized() {
  try {
    const maximized = await appWindow.isMaximized();
    applyMaximizedState(maximized);
  } catch (e) {
    console.warn('TitleBar isMaximized failed', e);
  }
}
```

- [ ] **Step 3: Rewrite toggle to use apply path**

```js
const toggleMaximizeWindow = () => {
  appWindow.isMaximized().then((maximized) => {
    if (maximized) {
      applyMaximizedState(false);
      appWindow.unmaximize();
    } else {
      applyMaximizedState(true);
      appWindow.maximize();
    }
  });
};
```

- [ ] **Step 4: Mount init + listeners; unmount cleanup**

```js
onMounted(async () => {
  await refreshMaximized();
  try {
    // Tauri 2: onResized fires after maximize/restore via system UI too
    unlistenResize = await appWindow.onResized(() => {
      void refreshMaximized();
    });
  } catch (e) {
    console.warn('TitleBar onResized failed', e);
  }
});

onUnmounted(() => {
  if (typeof unlistenResize === 'function') {
    unlistenResize();
    unlistenResize = null;
  }
});
```

If `onResized` is unavailable in this API surface, fall back to:

```js
unlistenResize = await appWindow.listen('tauri://resize', () => {
  void refreshMaximized();
});
```

(Verify against `@tauri-apps/api/window` types already used by `getCurrentWindow`.)

- [ ] **Step 5: Manual check**

Run app (`cargo tauri dev` or existing dev flow):
1. Home: maximize button → `uiStore.isMaximized` true (Vue devtools / temporary log).
2. System restore → false.
3. Open Settings window, maximize Settings → Home store must **stay** unchanged.
4. ImageEditor maximize → Home store unchanged.

- [ ] **Step 6: Commit**

```bash
git add src-vite/src/components/TitleBar.vue
git commit -m "feat(black-hole): sync main-window maximize into uiStore from Home TitleBar only"
```

---

### Task 4: `useIdle`

**Files:**
- Create: `src-vite/src/composables/useIdle.ts`

- [ ] **Step 1: Implement**

```ts
import { ref, onMounted, onUnmounted, type Ref } from 'vue';

const ACTIVITY_EVENTS = ['mousemove', 'keydown', 'scroll', 'wheel', 'touchstart'] as const;

export function useIdle(ms = 15000): { idle: Ref<boolean> } {
  const idle = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  const clearTimer = () => {
    if (timer != null) {
      clearTimeout(timer);
      timer = null;
    }
  };

  const reset = () => {
    idle.value = false;
    clearTimer();
    timer = setTimeout(() => {
      idle.value = true;
    }, ms);
  };

  onMounted(() => {
    for (const e of ACTIVITY_EVENTS) {
      window.addEventListener(e, reset, { passive: true });
    }
    reset();
  });

  onUnmounted(() => {
    clearTimer();
    for (const e of ACTIVITY_EVENTS) {
      window.removeEventListener(e, reset);
    }
  });

  return { idle };
}
```

- [ ] **Step 2: Commit**

```bash
git add src-vite/src/composables/useIdle.ts
git commit -m "feat(black-hole): useIdle 15s activity detector"
```

---

### Task 5: `BlackHoleBackground.vue` (background mode first)

**Files:**
- Create: `src-vite/src/components/BlackHoleBackground.vue`

- [ ] **Step 1: Component skeleton**

```vue
<template>
  <canvas
    ref="canvasRef"
    class="pointer-events-none fixed inset-0 z-0"
    aria-hidden="true"
  />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { computeRadii } from '@/common/blackHoleMath';

const props = defineProps<{
  gravityActive: boolean;
  /** seconds of effective idle growth; 0 in background mode */
  effectiveElapsedSec: number;
}>();

const emit = defineEmits<{
  radii: [payload: { R_event: number; R_inf: number }];
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
let raf = 0;
let diskAngle = 0;
let lastTs = 0;
let paused = document.hidden;

function primaryRgb(): string {
  const raw = getComputedStyle(document.documentElement)
    .getPropertyValue('--color-primary')
    .trim();
  // daisy may yield "oklch(...)" or "r g b" — fallback soft purple-blue
  return raw || '120 80 255';
}

function resize() {
  const c = canvasRef.value;
  if (!c) return;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = window.innerWidth;
  const h = window.innerHeight;
  c.width = Math.floor(w * dpr);
  c.height = Math.floor(h * dpr);
  c.style.width = `${w}px`;
  c.style.height = `${h}px`;
  const ctx = c.getContext('2d');
  if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function paint(ts: number) {
  if (paused) {
    raf = requestAnimationFrame(paint);
    return;
  }
  const c = canvasRef.value;
  if (!c) return;
  const ctx = c.getContext('2d');
  if (!ctx) return;
  const dt = lastTs ? Math.min(0.05, (ts - lastTs) / 1000) : 0.016;
  lastTs = ts;
  diskAngle += dt * 0.15;

  const w = window.innerWidth;
  const h = window.innerHeight;
  const HX = w / 2;
  const HY = h / 2;
  const elapsed = props.gravityActive ? props.effectiveElapsedSec : 0;
  const { R_event, R_inf } = computeRadii(elapsed, w, h);
  emit('radii', { R_event, R_inf });

  ctx.clearRect(0, 0, w, h);

  // accretion glow
  const glow = ctx.createRadialGradient(HX, HY, R_event * 0.9, HX, HY, R_inf * 0.55);
  const p = primaryRgb();
  glow.addColorStop(0, `color-mix(in oklab, ${cssColor(p)} 55%, transparent)`);
  glow.addColorStop(0.35, `color-mix(in oklab, ${cssColor(p)} 18%, transparent)`);
  glow.addColorStop(1, 'transparent');
  ctx.fillStyle = glow;
  ctx.beginPath();
  ctx.arc(HX, HY, R_inf * 0.55, 0, Math.PI * 2);
  ctx.fill();

  // spinning ring hint
  ctx.save();
  ctx.translate(HX, HY);
  ctx.rotate(diskAngle);
  ctx.strokeStyle = `color-mix(in oklab, ${cssColor(p)} 40%, transparent)`;
  ctx.lineWidth = Math.max(2, R_event * 0.08);
  ctx.beginPath();
  ctx.ellipse(0, 0, R_event * 1.35, R_event * 0.45, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();

  // event horizon
  ctx.fillStyle = '#000';
  ctx.beginPath();
  ctx.arc(HX, HY, R_event, 0, Math.PI * 2);
  ctx.fill();

  // einstein ring
  ctx.strokeStyle = `color-mix(in oklab, ${cssColor(p)} 70%, white 10%)`;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(HX, HY, R_event * 1.08, 0, Math.PI * 2);
  ctx.stroke();

  raf = requestAnimationFrame(paint);
}

function cssColor(p: string): string {
  // if already a full color function, use as-is; if "r g b", wrap
  if (p.includes('(')) return p;
  if (/^\d/.test(p)) return `rgb(${p.split(/\s+/).slice(0, 3).join(' ')})`;
  return p;
}

function onVis() {
  paused = document.hidden;
  if (!paused) lastTs = 0;
}

onMounted(() => {
  resize();
  window.addEventListener('resize', resize);
  document.addEventListener('visibilitychange', onVis);
  raf = requestAnimationFrame(paint);
});

onUnmounted(() => {
  cancelAnimationFrame(raf);
  window.removeEventListener('resize', resize);
  document.removeEventListener('visibilitychange', onVis);
});

watch(
  () => [props.gravityActive, props.effectiveElapsedSec],
  () => { /* paint loop reads props each frame */ },
);
</script>
```

Tune z-index in Home so canvas sits above `bg-base-300` shell but under interactive panes (`z-0` may need `z-[1]` under content `z-10` — adjust when wiring Home).

- [ ] **Step 2: Commit**

```bash
git add src-vite/src/components/BlackHoleBackground.vue
git commit -m "feat(black-hole): canvas ambient BlackHoleBackground"
```

---

### Task 6: `useGravityWarp`

**Files:**
- Create: `src-vite/src/composables/useGravityWarp.ts`

- [ ] **Step 1: Implement**

```ts
import { watch, onUnmounted, type Ref } from 'vue';
import { computeCardWarp, cardWarpCss } from '@/common/blackHoleMath';

export type RadiiRef = Ref<{ R_event: number; R_inf: number }>;

export function useGravityWarp(options: {
  rootEl: Ref<HTMLElement | null>;
  gravityActive: Ref<boolean>;
  radii: RadiiRef;
}) {
  let timer: ReturnType<typeof setInterval> | null = null;
  let orbitPhase = 0;
  let lastOrbitTs = 0;
  const touched = new Set<HTMLElement>();

  function clearAll() {
    for (const el of touched) {
      el.style.transform = '';
      el.style.filter = '';
      el.style.willChange = '';
    }
    touched.clear();
    orbitPhase = 0;
    lastOrbitTs = 0;
  }

  function tick() {
    if (!options.gravityActive.value) {
      clearAll();
      return;
    }
    const root = options.rootEl.value;
    if (!root) return;
    const now = performance.now();
    if (lastOrbitTs) {
      const dt = Math.min(0.2, (now - lastOrbitTs) / 1000);
      orbitPhase += dt * 0.15;
    }
    lastOrbitTs = now;

    const { R_event, R_inf } = options.radii.value;
    const HX = window.innerWidth / 2;
    const HY = window.innerHeight / 2;
    const cards = root.querySelectorAll<HTMLElement>('.bh-card');
    const live = new Set<HTMLElement>();

    cards.forEach((el) => {
      const rect = el.getBoundingClientRect();
      const cx = rect.left + rect.width / 2;
      const cy = rect.top + rect.height / 2;
      const warp = computeCardWarp(cx, cy, HX, HY, R_event, R_inf, orbitPhase);
      const css = cardWarpCss(warp);
      if (!warp.active) {
        if (touched.has(el)) {
          el.style.transform = '';
          el.style.filter = '';
          el.style.willChange = '';
          touched.delete(el);
        }
        return;
      }
      el.style.willChange = 'transform';
      el.style.transform = css.transform;
      el.style.filter = css.filter || '';
      touched.add(el);
      live.add(el);
    });

    for (const el of [...touched]) {
      if (!live.has(el)) {
        el.style.transform = '';
        el.style.filter = '';
        el.style.willChange = '';
        touched.delete(el);
      }
    }
  }

  function start() {
    if (timer != null) return;
    lastOrbitTs = performance.now();
    tick();
    timer = setInterval(tick, 120);
  }

  function stop() {
    if (timer != null) {
      clearInterval(timer);
      timer = null;
    }
    clearAll();
  }

  watch(
    () => options.gravityActive.value,
    (on) => {
      if (on) start();
      else stop();
    },
    { immediate: true },
  );

  onUnmounted(stop);

  return { tick, clearAll, stop };
}
```

- [ ] **Step 2: Commit**

```bash
git add src-vite/src/composables/useGravityWarp.ts
git commit -m "feat(black-hole): useGravityWarp throttled card transforms"
```

---

### Task 7: Thumbnail `.bh-card` on **outer** root

**Files:**
- Modify: `src-vite/src/components/Thumbnail.vue` (template root `div` ~line 2)

- [ ] **Step 1: Add class to outer root only**

On the **outermost** root `div` (the one with `border-2 rounded-box` and `transition-all ease-in-out duration-300`), add `'bh-card'` to the `:class` array.

**Do not** add to `ref="containerRef"` inner div.

Example:

```vue
:class="[
  'bh-card border-2 rounded-box flex flex-col items-center cursor-pointer group',
  isTransitionDisabled ? 'transition-none' : 'transition-all ease-in-out duration-300 ',
  // ...rest unchanged
]"
```

- [ ] **Step 2: Commit**

```bash
git add src-vite/src/components/Thumbnail.vue
git commit -m "feat(black-hole): mark Thumbnail outer root as bh-card"
```

---

### Task 8: Settings UI + i18n + emit

**Files:**
- Modify: `src-vite/src/views/Settings.vue` (appearance section ~lines 46–76)
- Modify: `src-vite/src/locales/zh.json`
- Modify: `src-vite/src/locales/en.json`
- Modify: `src-vite/src/views/Settings.vue` watches (~5500)

- [ ] **Step 1: i18n keys under `settings.general`**

`zh.json` (near `font_size` / appearance keys):

```json
"black_hole_theme": "黑洞主题",
"black_hole_theme_desc": "窗口最大化且空闲时，引力会聚拢主网格照片（可随时回弹）",
"black_hole_theme_hint": "平时仅作居中氛围背景；最大化后发呆约 15 秒释放引力",
"black_hole_theme_reduced_motion": "系统已开启「减少动态效果」，此特效不会运行"
```

`en.json`:

```json
"black_hole_theme": "Black hole theme",
"black_hole_theme_desc": "When maximized and idle, gravity gathers main-grid photos (always reversible)",
"black_hole_theme_hint": "Ambient centered background until ~15s idle while maximized",
"black_hole_theme_reduced_motion": "Reduced motion is on; this effect stays off"
```

- [ ] **Step 2: Appearance section toggle**

Inside the appearance `rounded-box` after font size row, add:

```vue
<div class="flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
  <div class="min-w-0 flex flex-col gap-0.5 text-sm leading-5">
    <div>{{ $t('settings.general.black_hole_theme') }}</div>
    <div class="text-xs text-base-content/30">{{ $t('settings.general.black_hole_theme_desc') }}</div>
    <div class="text-xs text-base-content/30">{{ $t('settings.general.black_hole_theme_hint') }}</div>
  </div>
  <input
    type="checkbox"
    class="toggle toggle-primary toggle-sm shrink-0"
    v-model="config.settings.blackHoleMode"
  />
</div>
```

- [ ] **Step 3: Watch + emit**

```js
watch(() => config.settings.blackHoleMode, (newValue) => {
  emitSettings('settings-blackHoleMode-changed', newValue);
});
```

Confirm Task 2 already registered `main.js` listener.

- [ ] **Step 4: Manual — toggle persists across restart**

- [ ] **Step 5: Commit**

```bash
git add src-vite/src/views/Settings.vue src-vite/src/locales/zh.json src-vite/src/locales/en.json src-vite/src/main.js
git commit -m "feat(black-hole): settings toggle and i18n"
```

---

### Task 9: Wire Home → Content → GridView

**Files:**
- Modify: `src-vite/src/views/Home.vue`
- Modify: `src-vite/src/components/Content.vue`
- Modify: `src-vite/src/components/GridView.vue`

#### 9A — Home assembles `gravityActive` and mounts background

- [ ] **Step 1: Imports and state in Home script**

```ts
import { computed, provide, ref, onMounted, onUnmounted } from 'vue';
import { useConfigStore } from '@/stores/configStore'; // if not already via config proxy
import BlackHoleBackground from '@/components/BlackHoleBackground.vue';
import { useIdle } from '@/composables/useIdle';

// Prefer existing `config` from `@/common/config` if that is the pinia instance used in Home.
const { idle } = useIdle(15000);
const reducedMotion = ref(false);
const docHidden = ref(typeof document !== 'undefined' ? document.hidden : false);
const effectiveElapsedSec = ref(0);
const holeRadii = ref({ R_event: 0, R_inf: 0 });
let growthRaf = 0;
let growthAnchor = 0; // performance.now when gravity became true (adjusted for pauses)
let growthAccum = 0;  // seconds accumulated while visible

const gravityActive = computed(() =>
  !!config.settings.blackHoleMode
  && !!uiStore.isMaximized
  && idle.value
  && !reducedMotion.value
  && !docHidden.value
  && uiStore.inputStack.length === 0
  && !isSwitchingLibrary.value
);

const showBlackHole = computed(
  () => !!config.settings.blackHoleMode && !reducedMotion.value,
);

provide('bhGravityActive', gravityActive);
provide('bhRadii', holeRadii);
```

- [ ] **Step 2: Growth clock (effective elapsed only while gravityActive && !hidden)**

```ts
function onVis() {
  docHidden.value = document.hidden;
}

function growthLoop(ts: number) {
  if (gravityActive.value && !docHidden.value) {
    if (!growthAnchor) growthAnchor = ts;
    // recompute from accum + (ts - segmentStart) — simple approach:
    effectiveElapsedSec.value = growthAccum + (ts - growthAnchor) / 1000;
  }
  growthRaf = requestAnimationFrame(growthLoop);
}

watch(gravityActive, (on, was) => {
  if (on && !was) {
    growthAccum = 0;
    growthAnchor = performance.now();
    effectiveElapsedSec.value = 0;
  } else if (!on) {
    growthAccum = 0;
    growthAnchor = 0;
    effectiveElapsedSec.value = 0;
  }
});

watch(docHidden, (hidden) => {
  if (hidden) {
    if (gravityActive.value && growthAnchor) {
      growthAccum += (performance.now() - growthAnchor) / 1000;
      growthAnchor = 0;
    }
  } else if (gravityActive.value) {
    growthAnchor = performance.now();
  }
});
```

On mount: `matchMedia('(prefers-reduced-motion: reduce)')`, listen change + `visibilitychange`; start `growthLoop`. On unmount: cleanup.

- [ ] **Step 3: Template — mount background under shell**

Inside the outermost Home root `div`, early child (after switching overlay is fine):

```vue
<BlackHoleBackground
  v-if="showBlackHole"
  :gravity-active="gravityActive"
  :effective-elapsed-sec="effectiveElapsedSec"
  @radii="holeRadii = $event"
/>
```

Ensure main content panes keep `relative z-10` (left pane already `z-10`) so controls stay clickable; canvas `pointer-events-none`.

#### 9B — Content inject + prop to GridView

- [ ] **Step 4: Content.vue**

```ts
import { inject, type Ref, computed } from 'vue';

const bhGravityActive = inject<Ref<boolean> | undefined>('bhGravityActive', undefined);
const bhRadii = inject<Ref<{ R_event: number; R_inf: number }> | undefined>('bhRadii', undefined);

const gridGravityActive = computed(() => !!bhGravityActive?.value);
```

On `<GridView ...>` add:

```vue
:gravity-active="gridGravityActive"
:bh-radii="bhRadii?.value ?? { R_event: 0, R_inf: 0 }"
```

(If inject default undefined when not under Home — Content only used from Home today — safe.)

Better: pass refs via computed objects:

```vue
:gravity-active="!!bhGravityActive?.value"
```

For radii, either inject inside GridView too (simpler — **preferred** to avoid prop drilling Content):

**Preferred simplification:** GridView `inject('bhGravityActive')` and `inject('bhRadii')` directly; Content unchanged except ensure it stays under Home provide tree (it is). Then **skip Content edits**.

Use inject-in-GridView to reduce Content.vue risk (Content is ~8k lines).

#### 9C — GridView hosts warp

- [ ] **Step 5: GridView script**

```ts
import { inject, ref, type Ref, computed } from 'vue';
import { useGravityWarp } from '@/composables/useGravityWarp';

const bhGravityActive = inject<Ref<boolean> | null>('bhGravityActive', null);
const bhRadii = inject<Ref<{ R_event: number; R_inf: number }> | null>('bhRadii', null);

const gravityActiveRef = computed(() => !!bhGravityActive?.value);
// useGravityWarp wants Ref<boolean> — wrap:
import { toRef, customRef } from 'vue';
// simplest: local computed ref pattern
const gravityActiveForWarp = computed(() => !!bhGravityActive?.value) as unknown as Ref<boolean>;
// Better implement useGravityWarp to accept MaybeRefOrGetter — for plan, pass:
const falseRef = ref(false);
const radiiFallback = ref({ R_event: 0, R_inf: 0 });

useGravityWarp({
  rootEl: containerRef, // existing root ref on GridView outer div
  gravityActive: bhGravityActive ?? falseRef,
  radii: bhRadii ?? radiiFallback,
});
```

Confirm GridView already has `containerRef` on the outer `div.w-full.h-full` (template line ~3 `ref="containerRef"`). Use that as query root so only this grid’s `.bh-card` match.

If `inject` default null typing is awkward, normalize:

```ts
const gravityActiveLocal = computed(() => !!(bhGravityActive && bhGravityActive.value));
// change useGravityWarp to accept `() => boolean` OR watch computed — adjust composable:

// In useGravityWarp options:
// gravityActive: Ref<boolean> | ComputedRef<boolean>
```

Update `useGravityWarp` watch source to `() => unref(options.gravityActive)`.

- [ ] **Step 6: Smoke in dev**

1. Settings: enable black hole → Home shows centered glow, photos still.
2. Maximize + wait 15s → cards drift; opacity stays 1.
3. Move mouse → rebound.
4. Open rename/`inputStack` dialog mid-gravity → clear.
5. Settings window: no canvas.
6. Image viewer window: no canvas on that route.

- [ ] **Step 7: Commit**

```bash
git add src-vite/src/views/Home.vue src-vite/src/components/GridView.vue src-vite/src/composables/useGravityWarp.ts
git commit -m "feat(black-hole): wire Home gravityActive provide and GridView warp"
```

---

### Task 10: Guards polish + full manual checklist

**Files:** possibly small fixes only.

- [ ] **Step 1: reduced-motion**

Home mount:

```ts
const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
const applyMq = () => { reducedMotion.value = mq.matches; };
applyMq();
mq.addEventListener?.('change', applyMq);
// onUnmounted remove
```

When `reducedMotion`, `showBlackHole` false → unmount canvas; `gravityActive` false.

- [ ] **Step 2: Run math smoke again**

```bash
node scripts/check_black_hole_math.mjs
```

- [ ] **Step 3: Manual checklist (design §11)**

| # | Expect |
|---|---|
| 1 Default off | no canvas |
| 2 On, not maximized | ambient only |
| 3 Max + activity | no gravity |
| 4 Max + 15s idle | grow + orbit cards |
| 5 Input | instant clear |
| 6 Unmaximize (button + system) | clear; store false |
| 6b Settings maximize | main store unchanged |
| 7 inputStack dialog | gravity off |
| 7b library switch overlay | gravity off |
| 8 ImageViewer route/window | no hole there |
| 9 Settings route | no hole |
| 10 Filmstrip | strip GridView cards can warp; MediaViewer preview not |
| 11 Virtual scroll | no stuck transforms on recycled nodes |
| 12 reduced-motion | fully off |
| 13 document.hidden | pause growth |
| 14 disable setting | unmount + clear |

- [ ] **Step 4: GROW (project rule)**

- Update `.mex/ROUTER.md` Current Project State bullet for black-hole idle theme.
- `mex log` decision/note if available.
- Bump `last_updated` on touched mex files.

- [ ] **Step 5: Final commit**

```bash
git add -A
git status
git commit -m "feat(black-hole): guards, mex note, idle theme complete"
```

---

## Spec coverage (self-review)

| Spec item | Task |
|---|---|
| opt-in `blackHoleMode` persist | 2, 8 |
| `uiStore.isMaximized` new | 2 |
| TitleBar Home-only + init + resize | 3 |
| useIdle 15s | 4 |
| BlackHoleBackground canvas primary tint | 5 |
| growth curve / radii | 1, 5, 9 |
| useGravityWarp 120ms, outer `.bh-card` | 6, 7 |
| gravityActive assembled in Home only | 9 |
| isSwitchingLibrary local | 9 |
| inject/consume not re-query store in warp | 6, 9 |
| no App.vue host | 9 |
| GridView host not Content | 9 |
| reduced-motion whole off | 9, 10 |
| Settings appearance + i18n | 8 |
| cross-window settings event | 2, 8 |
| MediaViewer maximize untouched | 3 (explicit non-touch) |
| performance budget / no new deps | all |

**Placeholders:** none intentional.  
**Type names:** `computeRadii`, `computeCardWarp`, `useIdle`, `useGravityWarp`, `setMaximized`, `setBlackHoleMode`, provide keys `bhGravityActive` / `bhRadii` consistent across tasks.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-25-black-hole-idle-theme.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — this session with executing-plans and checkpoints  

Which approach?
