import assert from 'node:assert/strict';

// Inlined from src-vite/src/common/blackHoleMath.ts (keep in sync)

function clamp(n, min, max) {
  return Math.min(max, Math.max(min, n));
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function smoothstep(t) {
  const x = clamp(t, 0, 1);
  return x * x * (3 - 2 * x);
}

/** ~25s → ~95% when tau=8 */
function growthK(elapsedSec, tau = 8) {
  if (elapsedSec <= 0) return 0;
  return 1 - Math.exp(-elapsedSec / tau);
}

function computeRadii(elapsedSec, vw, vh) {
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

function computeCardWarp(cx, cy, HX, HY, R_event, R_inf, orbitPhase, swirl = 12) {
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

function cardWarpCss(w) {
  if (!w.active) return { transform: '', filter: '' };
  return {
    transform: `translate(${w.tx.toFixed(2)}px, ${w.ty.toFixed(2)}px) rotate(${w.rotDeg.toFixed(2)}deg) scale(${w.scale.toFixed(4)})`,
    filter: w.blur > 0.05 ? `blur(${w.blur.toFixed(2)}px)` : '',
  };
}

// --- unit helpers ---
assert.equal(lerp(0, 10, 0.5), 5);
assert.equal(smoothstep(0), 0);
assert.equal(smoothstep(1), 1);
assert.equal(growthK(0), 0);
assert.ok(growthK(25) > 0.94 && growthK(25) < 0.96);

// --- computeRadii ---
const r0 = computeRadii(0, 1000, 1000);
assert.equal(r0.R_event, 0.06 * 1000);
assert.equal(r0.R_inf, 0.12 * 1000);

// large elapsed approaches max radii (k → 1)
const rLarge = computeRadii(1e6, 1000, 1000);
const m = 1000;
const R_eventMax = 0.16 * m;
const R_infMax = 0.92 * Math.hypot(1000, 1000) / 2;
assert.ok(Math.abs(rLarge.R_event - R_eventMax) < 1e-6);
assert.ok(Math.abs(rLarge.R_inf - R_infMax) < 1e-6);
assert.ok(rLarge.R_event > r0.R_event);
assert.ok(rLarge.R_inf > r0.R_inf);

// --- far card: dist > R_inf → inactive, empty transform ---
const HX = 0;
const HY = 0;
const R_event = 50;
const R_inf = 100;
const far = computeCardWarp(150, 0, HX, HY, R_event, R_inf, 0);
assert.equal(far.active, false);
assert.equal(far.scale, 1);
const farCss = cardWarpCss(far);
assert.equal(farCss.transform, '');
assert.equal(farCss.filter, '');

// --- near card: active, scale < 1, non-empty transform ---
const near = computeCardWarp(10, 0, HX, HY, R_event, R_inf, 0.5);
assert.equal(near.active, true);
assert.ok(near.scale < 1);
const nearCss = cardWarpCss(near);
assert.ok(nearCss.transform.length > 0);
assert.ok(nearCss.transform.includes('translate'));
assert.ok(nearCss.transform.includes('scale'));

console.log('check_black_hole_math: ok');
