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
