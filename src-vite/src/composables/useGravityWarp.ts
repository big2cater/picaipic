import { watch, onUnmounted, unref, type Ref, type ComputedRef } from 'vue';
import { computeCardWarp, cardWarpCss, readPrimaryColor } from '@/common/blackHoleMath';
import { useConfigStore } from '@/stores/configStore';

export type RadiiValue = { R_event: number; R_inf: number };

type Anchor = { cx: number; cy: number };

function clearCard(el: HTMLElement) {
  el.style.transform = '';
  el.style.filter = '';
  el.style.opacity = '';
  el.style.willChange = '';
  el.style.transition = '';
  el.style.zIndex = '';
  el.style.pointerEvents = '';
  el.style.removeProperty('--bh-tear');
  el.style.removeProperty('--bh-tear-op');
  el.style.removeProperty('--bh-ring');
  el.style.removeProperty('--bh-ring-op');
  el.style.removeProperty('--bh-primary');
}

/**
 * Pin each card's LAYOUT center once (un-warped).
 * All spiral math is relative to this home so motion is a clean path
 * into the hole — not a feedback loop on live visual bounds.
 */
function captureAnchors(cards: HTMLElement[], into: Map<HTMLElement, Anchor>) {
  const need = cards.filter((el) => !into.has(el));
  if (!need.length) return;

  const saved = need.map((el) => ({
    transform: el.style.transform,
    filter: el.style.filter,
    opacity: el.style.opacity,
    transition: el.style.transition,
  }));

  for (const el of need) {
    el.style.transition = 'none';
    el.style.transform = 'none';
    el.style.filter = 'none';
    el.style.opacity = '1';
  }

  // one reflow for all new cards
  for (const el of need) {
    const rect = el.getBoundingClientRect();
    into.set(el, {
      cx: rect.left + rect.width / 2,
      cy: rect.top + rect.height / 2,
    });
  }

  need.forEach((el, i) => {
    el.style.transform = saved[i].transform;
    el.style.filter = saved[i].filter;
    el.style.opacity = saved[i].opacity;
    el.style.transition = saved[i].transition;
  });
}

export function useGravityWarp(options: {
  rootEl: Ref<HTMLElement | null>;
  gravityActive: Ref<boolean> | ComputedRef<boolean>;
  radii: Ref<RadiiValue> | ComputedRef<RadiiValue>;
}) {
  let timer: ReturnType<typeof setInterval> | null = null;
  let orbitPhase = 0;
  let lastOrbitTs = 0;
  const touched = new Set<HTMLElement>();
  /** fixed layout homes for the duration of this gravity session */
  const anchors = new Map<HTMLElement, Anchor>();
  let resizeObserver: ResizeObserver | null = null;
  let primaryColor = '124 92 255';
  let primaryColorTs = 0;

  function clearAll() {
    for (const el of touched) clearCard(el);
    touched.clear();
    anchors.clear();
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
      // Slow continuous spiral wind
      orbitPhase += dt * 0.07;
    }
    lastOrbitTs = now;

    const { R_event, R_inf } = unref(options.radii);
    if (!(R_inf > 0)) {
      clearAll();
      return;
    }

    const HX = window.innerWidth / 2;
    const HY = window.innerHeight / 2;
    const cardList = Array.from(root.querySelectorAll<HTMLElement>('.bh-card'));
    const live = new Set<HTMLElement>();
    const cardCount = cardList.length;
    const skipHeavy = cardCount > 80;

    const configStore = useConfigStore();
    const intensity = Number(configStore.settings.dynamicThemeIntensity) || 1;

    if (now - primaryColorTs > 500) {
      primaryColor = readPrimaryColor();
      primaryColorTs = now;
    }

    // Pin homes for any new cards (virtual-scroll recycle / first frame)
    captureAnchors(cardList, anchors);

    // Drop anchors for elements no longer in the DOM list (Set: O(n+m))
    const liveCards = new Set(cardList);
    for (const el of [...anchors.keys()]) {
      if (!liveCards.has(el)) anchors.delete(el);
    }

    cardList.forEach((el) => {
      const home = anchors.get(el);
      if (!home) return;

      const warp = computeCardWarp(
        home.cx, home.cy, HX, HY, R_event, R_inf, orbitPhase, 12, intensity, cardCount,
      );
      const css = cardWarpCss(warp);

      if (!warp.active) {
        if (touched.has(el)) {
          clearCard(el);
          touched.delete(el);
        }
        return;
      }

      // Longer easing so the spiral reads as continuous, not stepped
      el.style.transition = 'transform 160ms linear, filter 160ms linear, opacity 220ms ease-out';
      el.style.willChange = 'transform, filter, opacity';
      el.style.transform = css.transform;
      el.style.filter = css.filter || '';
      el.style.opacity = String(css.opacity);
      el.style.zIndex = String(1 + Math.round((1 - css.opacity) * 5));
      el.style.pointerEvents = css.opacity < 0.12 ? 'none' : '';

      if (!skipHeavy) {
        for (const [k, v] of Object.entries(css.vars)) {
          el.style.setProperty(k, v);
        }
      } else {
        el.style.setProperty('--bh-tear-op', '0');
        el.style.setProperty('--bh-ring-op', '0');
      }
      el.style.setProperty('--bh-primary', primaryColor);

      touched.add(el);
      live.add(el);
    });

    for (const el of [...touched]) {
      if (!live.has(el)) {
        clearCard(el);
        touched.delete(el);
      }
    }
  }

  function start() {
    if (timer != null) return;
    anchors.clear();
    lastOrbitTs = performance.now();
    tick();
    timer = setInterval(tick, 150);
  }

  function stop() {
    if (timer != null) {
      clearInterval(timer);
      timer = null;
    }
    clearAll();
  }

  function setupResizeObserver(rootEl: HTMLElement) {
    // Layout change invalidates homes — re-pin next tick
    resizeObserver = new ResizeObserver(() => {
      anchors.clear();
    });
    resizeObserver.observe(rootEl);
  }

  watch(
    () => unref(options.rootEl),
    (el) => {
      if (resizeObserver) {
        resizeObserver.disconnect();
        resizeObserver = null;
      }
      if (el) setupResizeObserver(el);
    },
    { immediate: true },
  );

  watch(
    () => unref(options.gravityActive),
    (on) => {
      if (on) start();
      else stop();
    },
    { immediate: true },
  );

  onUnmounted(() => {
    stop();
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
  });

  return { tick, clearAll, stop };
}
