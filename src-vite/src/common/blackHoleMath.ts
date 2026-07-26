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

/** v1.5 — extended 6-layer CardWarp (plus legacy fields for backward compat) */
export type CardWarp = {
  tx: number;
  ty: number;
  scale: number;
  rotDeg: number;
  blur: number;
  active: boolean;
  /** 1 = fully visible, 0 = swallowed into the hole */
  opacity: number;

  stretchX: number;
  stretchY: number;
  /** radial axis (radians) for anisotropic stretch */
  radialAxis: number;
  dispPx: number;
  hueShift: number;
  useDispersion: boolean;
  tear: number;
  tearOp: number;
  ring: number;
  ringOp: number;
};

/**
 * Spiral-absorb warp from a FIXED layout home position (cx,cy).
 *
 * Owner intent: every photo is slowly pulled into the center in a neat
 * spiral, increasingly flattened near the hole, then gently fades out
 * only at the very end — not a sudden pop.
 *
 * Important: (cx,cy) must be the un-warped layout center (pinned once
 * when gravity starts). Do NOT pass live visual centers.
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
  intensity = 1,
  visibleCardCount = 0,
): CardWarp {
  const dx = cx - HX;
  const dy = cy - HY;
  const homeDist = Math.hypot(dx, dy);
  const homeAngle = Math.atan2(dy, dx);

  const inactive: CardWarp = {
    tx: 0, ty: 0, scale: 1, rotDeg: 0, blur: 0, active: false, opacity: 1,
    stretchX: 1, stretchY: 1, radialAxis: 0,
    dispPx: 0, hueShift: 0, useDispersion: false,
    tear: 0, tearOp: 0, ring: 0, ringOp: 0,
  };

  if (!(R_inf > 0) || homeDist > R_inf) return inactive;

  const I = clamp(Number(intensity) || 0, 0, 1.5);
  if (I <= 0) return inactive;

  // Soft influence 0 at R_inf edge → 1 at hole center (home-based, stable)
  const u = clamp((R_inf - homeDist) / Math.max(R_inf, 1), 0, 1);
  // Gentle ease so cards ease into the spiral instead of snapping
  const fall = Math.pow(smoothstep(u), 1.35) * I;
  if (fall <= 0.001) return inactive;

  // Spiral radius: shrink homeDist → 0 as fall → 1
  // Ease-in so early motion is slow drift, late motion accelerates
  const collapse = Math.pow(fall, 1.1);
  const r = homeDist * (1 - collapse);

  // Neat spiral: angle advances with global phase * depth
  // Outer cards barely turn; inner cards complete more of a wind
  const spin = orbitPhase * (0.55 + 1.6 * fall) + (swirl * 0.015) * fall;
  const theta = homeAngle + spin;

  const nx = HX + r * Math.cos(theta);
  const ny = HY + r * Math.sin(theta);
  const tx = nx - cx;
  const ty = ny - cy;

  // Size: slow shrink, only tiny near the end
  const scale = lerp(1, 0.12, Math.pow(collapse, 1.05));

  // Mild face-turn along the spiral (not wild tumbling)
  const orbitDeg = ((theta - homeAngle) * 180) / Math.PI;
  const rotDeg = orbitDeg * 0.35;

  // Flatten more only in the inner half of the fall
  const crush = Math.pow(clamp((fall - 0.25) / 0.75, 0, 1), 1.4);
  const radialAxis = theta;
  const stretchX = lerp(1, 1.7, crush);  // elongate toward hole
  const stretchY = lerp(1, 0.38, crush); // squash sideways

  const blur = lerp(0, 3.5, crush);
  const dispPx = lerp(0, 3.2, crush);
  const hueShift = lerp(0, 8, crush);
  const useDispersion =
    crush > 0.25
    && I > 0
    && !(I >= 1.5 && visibleCardCount > 40);

  const tear = lerp(0, 4, crush);
  const tearOp = crush > 0.2 ? crush * 0.4 : 0;
  const ring = lerp(0, 0.55, crush);
  const ringOp = clamp(crush * 0.65, 0, 1);

  // Fade ONLY in the final approach past the photon ring — long smooth tail
  // r >> R_event: fully visible; r → 0: gone
  const fadeStart = Math.max(R_event * 1.8, homeDist * 0.22, 20);
  const fadeEnd = Math.max(R_event * 0.15, 3);
  let opacity = 1;
  if (r < fadeStart) {
    const ft = clamp((r - fadeEnd) / Math.max(fadeStart - fadeEnd, 1), 0, 1);
    // smoothstep for soft dissolve (no pop)
    opacity = smoothstep(ft);
  }
  // never hard-cut while still large on screen
  if (scale > 0.35) opacity = Math.max(opacity, 0.55);

  return {
    tx, ty, scale, rotDeg, blur, active: true, opacity,
    stretchX, stretchY, radialAxis,
    dispPx, hueShift, useDispersion,
    tear, tearOp, ring, ringOp,
  };
}

export type WarpCss = {
  transform: string;
  filter: string;
  opacity: number;
  vars: Record<string, string>;
};

export function cardWarpCss(w: CardWarp): WarpCss {
  const empty: WarpCss = { transform: '', filter: '', opacity: 1, vars: {} };
  if (!w.active) return empty;

  const useStretch =
    Math.abs(w.stretchX - 1) > 0.005 || Math.abs(w.stretchY - 1) > 0.005;
  const rx = w.radialAxis.toFixed(6);
  let transform =
    `translate(${w.tx.toFixed(2)}px, ${w.ty.toFixed(2)}px) `
    + `rotate(${w.rotDeg.toFixed(2)}deg) scale(${w.scale.toFixed(4)})`;
  if (useStretch) {
    transform +=
      ` rotate(${rx}rad) scale(${w.stretchX.toFixed(4)},${w.stretchY.toFixed(4)}) rotate(-${rx}rad)`;
  }

  const filterParts: string[] = [];
  if (w.blur > 0.05) filterParts.push(`blur(${w.blur.toFixed(2)}px)`);
  if (Math.abs(w.hueShift) > 0.2) filterParts.push(`hue-rotate(${w.hueShift.toFixed(2)}deg)`);
  if (w.useDispersion && w.dispPx > 0.2) {
    filterParts.push(`drop-shadow(${w.dispPx.toFixed(2)}px 0 0 rgba(255,80,80,0.28))`);
    filterParts.push(`drop-shadow(-${w.dispPx.toFixed(2)}px 0 0 rgba(80,220,255,0.28))`);
  }

  return {
    transform,
    filter: filterParts.join(' '),
    opacity: clamp(w.opacity, 0, 1),
    vars: {
      '--bh-tear': `${w.tear.toFixed(2)}px`,
      '--bh-tear-op': String(Math.max(0, Math.min(1, w.tearOp))),
      '--bh-ring': String(Math.max(0, Math.min(1, w.ring))),
      '--bh-ring-op': String(Math.max(0, Math.min(1, w.ringOp))),
    },
  };
}

export function readPrimaryColor(): string {
  if (typeof document === 'undefined') return '124 92 255';
  const raw = getComputedStyle(document.documentElement)
    .getPropertyValue('--color-primary')
    .trim();
  return raw || '124 92 255';
}
