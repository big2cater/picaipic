<template>
  <canvas
    ref="canvasRef"
    class="pointer-events-none fixed inset-0 z-0"
    aria-hidden="true"
  />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { computeRadii } from '@/common/blackHoleMath';

const props = defineProps<{
  gravityActive: boolean;
  /** seconds of effective idle growth; treated as 0 when !gravityActive */
  effectiveElapsedSec: number;
}>();

const emit = defineEmits<{
  radii: [payload: { R_event: number; R_inf: number }];
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
let raf = 0;
let diskAngle = 0;
let lastTs = 0;
let paused = typeof document !== 'undefined' ? document.hidden : false;
let primaryColor = '#7c5cff';

/** Read theme primary; fall back to solid purple. Avoid broken canvas with oklch/color-mix. */
function readPrimary(): string {
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue('--color-primary')
    .trim();
  if (!v) return '#7c5cff';
  if (v.includes('(') || v.startsWith('#') || v.startsWith('rgb') || v.startsWith('hsl')) {
    return v;
  }
  // space-separated RGB components (e.g. "120 80 255")
  if (/^\d/.test(v)) {
    return `rgb(${v.split(/\s+/).slice(0, 3).join(',')})`;
  }
  return v;
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
  primaryColor = readPrimary();
}

function paint(ts: number) {
  raf = requestAnimationFrame(paint);

  // Skip heavy work while tab/window is hidden; keep rAF scheduled for clean resume.
  if (paused) return;

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
  // Background mode: freeze growth even if caller still passes elapsed.
  const elapsed = props.gravityActive ? props.effectiveElapsedSec : 0;
  const { R_event, R_inf } = computeRadii(elapsed, w, h);
  emit('radii', { R_event, R_inf });

  ctx.clearRect(0, 0, w, h);

  const p = primaryColor;
  // Slightly stronger glow while gravity is active.
  const glowBoost = props.gravityActive ? 1.15 : 1;

  // Accretion glow — radial gradient with solid colors + globalAlpha (no color-mix).
  const glowR = R_inf * 0.55;
  const glow = ctx.createRadialGradient(HX, HY, R_event * 0.9, HX, HY, glowR);
  glow.addColorStop(0, p);
  glow.addColorStop(0.35, p);
  glow.addColorStop(1, 'rgba(0,0,0,0)');

  ctx.save();
  ctx.globalAlpha = 0.45 * glowBoost;
  ctx.fillStyle = glow;
  ctx.beginPath();
  ctx.arc(HX, HY, glowR, 0, Math.PI * 2);
  ctx.fill();
  // Soft outer halo
  ctx.globalAlpha = 0.12 * glowBoost;
  ctx.beginPath();
  ctx.arc(HX, HY, glowR, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  // Spinning accretion ring hint
  ctx.save();
  ctx.translate(HX, HY);
  ctx.rotate(diskAngle);
  ctx.globalAlpha = 0.4;
  ctx.strokeStyle = p;
  ctx.lineWidth = Math.max(2, R_event * 0.08);
  ctx.beginPath();
  ctx.ellipse(0, 0, R_event * 1.35, R_event * 0.45, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();

  // Event horizon
  ctx.fillStyle = '#000';
  ctx.beginPath();
  ctx.arc(HX, HY, R_event, 0, Math.PI * 2);
  ctx.fill();

  // Thin Einstein ring
  ctx.save();
  ctx.globalAlpha = 0.75;
  ctx.strokeStyle = p;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(HX, HY, R_event * 1.08, 0, Math.PI * 2);
  ctx.stroke();
  // faint white highlight on the ring edge
  ctx.globalAlpha = 0.25;
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.arc(HX, HY, R_event * 1.08, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
}

function onVis() {
  paused = document.hidden;
  if (!paused) {
    lastTs = 0;
    primaryColor = readPrimary();
  }
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
</script>
