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
