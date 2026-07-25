import { watch, onUnmounted, unref, type Ref, type ComputedRef } from 'vue';
import { computeCardWarp, cardWarpCss } from '@/common/blackHoleMath';

export type RadiiValue = { R_event: number; R_inf: number };

export function useGravityWarp(options: {
  rootEl: Ref<HTMLElement | null>;
  gravityActive: Ref<boolean> | ComputedRef<boolean>;
  radii: Ref<RadiiValue> | ComputedRef<RadiiValue>;
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
    if (!unref(options.gravityActive)) {
      clearAll();
      return;
    }
    const root = unref(options.rootEl);
    if (!root) return;
    const now = performance.now();
    if (lastOrbitTs) {
      const dt = Math.min(0.2, (now - lastOrbitTs) / 1000);
      orbitPhase += dt * 0.15;
    }
    lastOrbitTs = now;

    const { R_event, R_inf } = unref(options.radii);
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
    () => unref(options.gravityActive),
    (on) => {
      if (on) start();
      else stop();
    },
    { immediate: true },
  );

  onUnmounted(stop);

  return { tick, clearAll, stop };
}
