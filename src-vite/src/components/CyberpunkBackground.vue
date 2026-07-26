<template>
  <div
    class="cp-backdrop pointer-events-none fixed inset-0 z-0 overflow-hidden"
    :class="{ 'cp-backdrop--motion': animate }"
    aria-hidden="true"
  >
    <div class="cp-backdrop__base" />
    <div class="cp-backdrop__skyline" />
    <div class="cp-backdrop__glow cp-backdrop__glow--magenta" />
    <div class="cp-backdrop__glow cp-backdrop__glow--cyan" />
    <div class="cp-backdrop__glow cp-backdrop__glow--violet" />
    <div class="cp-backdrop__grid" />
    <div class="cp-backdrop__horizon" />
    <canvas ref="canvasRef" class="cp-backdrop__fx" />
    <div class="cp-backdrop__scanlines" />
    <div v-if="animate" class="cp-backdrop__beam" />
    <div class="cp-backdrop__vignette" />
  </div>
</template>

<script setup lang="ts">
/**
 * Night-city cyberpunk ambient (always-on under Cyberpunk theme).
 * CSS: grid / neon / skyline. Canvas: rain, particles, floating kana.
 * Idle photo glitch remains PhotoGlitchLayer.
 */
import { onMounted, onUnmounted, ref, watch } from 'vue';

const props = withDefaults(
  defineProps<{
    /** false when prefers-reduced-motion: static scene, no rain/particles */
    animate?: boolean;
  }>(),
  { animate: true },
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

// Mixed katakana / sparse kanji — decorative only (not real sentences)
const GLYPHS =
  'アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン電脳都市夜雨霓虹未来記憶断片回路';

type RainDrop = { x: number; y: number; len: number; spd: number; thick: number; a: number };
type Particle = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  a: number;
  hue: number;
  life: number;
  maxLife: number;
};
type Glyph = {
  x: number;
  y: number;
  ch: string;
  spd: number;
  size: number;
  a: number;
  hue: number;
  blink: number;
};

let rain: RainDrop[] = [];
let particles: Particle[] = [];
let glyphs: Glyph[] = [];
let raf = 0;
let lastTs = 0;
let w = 0;
let h = 0;
let dpr = 1;
let running = false;

function rand(a = 0, b = 1) {
  return a + Math.random() * (b - a);
}

function pickGlyph() {
  return GLYPHS[(Math.random() * GLYPHS.length) | 0] || 'ア';
}

function resize() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const parent = canvas.parentElement;
  const rect = parent?.getBoundingClientRect() ?? { width: window.innerWidth, height: window.innerHeight };
  dpr = Math.min(window.devicePixelRatio || 1, 1.5);
  w = Math.max(1, Math.floor(rect.width));
  h = Math.max(1, Math.floor(rect.height));
  canvas.width = Math.floor(w * dpr);
  canvas.height = Math.floor(h * dpr);
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  const ctx = canvas.getContext('2d');
  if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  seedField(true);
}

function seedField(force = false) {
  const area = w * h;
  const rainN = Math.min(220, Math.max(80, Math.floor(area / 9000)));
  const partN = Math.min(90, Math.max(28, Math.floor(area / 22000)));
  const glyphN = Math.min(36, Math.max(12, Math.floor(area / 55000)));

  if (force || rain.length !== rainN) {
    rain = Array.from({ length: rainN }, () => makeRain(true));
  }
  if (force || particles.length !== partN) {
    particles = Array.from({ length: partN }, () => makeParticle(true));
  }
  if (force || glyphs.length !== glyphN) {
    glyphs = Array.from({ length: glyphN }, () => makeGlyph(true));
  }
}

function makeRain(scatter: boolean): RainDrop {
  return {
    x: rand(0, w),
    y: scatter ? rand(-h, h) : rand(-h * 0.2, 0),
    len: rand(12, 36),
    spd: rand(520, 980),
    thick: rand(0.7, 1.4),
    a: rand(0.12, 0.38),
  };
}

function makeParticle(scatter: boolean): Particle {
  const magenta = Math.random() > 0.45;
  return {
    x: rand(0, w),
    y: scatter ? rand(0, h) : h + rand(0, 40),
    vx: rand(-12, 12),
    vy: rand(-28, -8),
    r: rand(0.8, 2.6),
    a: rand(0.25, 0.85),
    hue: magenta ? rand(310, 330) : rand(180, 200),
    life: scatter ? rand(0, 1) : 0,
    maxLife: rand(3.5, 8),
  };
}

function makeGlyph(scatter: boolean): Glyph {
  const magenta = Math.random() > 0.5;
  return {
    x: rand(0, w),
    y: scatter ? rand(0, h) : h + rand(20, 80),
    ch: pickGlyph(),
    spd: rand(18, 48),
    size: rand(11, 20),
    a: rand(0.18, 0.55),
    hue: magenta ? 318 : 188,
    blink: rand(0, Math.PI * 2),
  };
}

function paint(ts: number) {
  if (!running) return;
  raf = requestAnimationFrame(paint);
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const dt = lastTs ? Math.min(0.05, (ts - lastTs) / 1000) : 0.016;
  lastTs = ts;

  ctx.clearRect(0, 0, w, h);

  // Rain
  ctx.lineCap = 'round';
  for (let i = 0; i < rain.length; i++) {
    const d = rain[i];
    d.y += d.spd * dt;
    d.x += 28 * dt; // slight wind
    if (d.y > h + d.len) {
      rain[i] = makeRain(false);
      continue;
    }
    if (d.x > w + 20) d.x = -10;
    const g = ctx.createLinearGradient(d.x, d.y, d.x - 2, d.y + d.len);
    g.addColorStop(0, `rgba(180, 220, 255, 0)`);
    g.addColorStop(0.4, `rgba(160, 210, 255, ${d.a})`);
    g.addColorStop(1, `rgba(0, 229, 255, 0)`);
    ctx.strokeStyle = g;
    ctx.lineWidth = d.thick;
    ctx.beginPath();
    ctx.moveTo(d.x, d.y);
    ctx.lineTo(d.x - 3, d.y + d.len);
    ctx.stroke();
  }

  // Floating neon particles / embers
  for (let i = 0; i < particles.length; i++) {
    const p = particles[i];
    p.life += dt;
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    p.vx += Math.sin(ts * 0.001 + i) * 6 * dt;
    if (p.life > p.maxLife || p.y < -20 || p.x < -30 || p.x > w + 30) {
      particles[i] = makeParticle(false);
      continue;
    }
    const fade = 1 - p.life / p.maxLife;
    const alpha = p.a * fade;
    ctx.beginPath();
    ctx.fillStyle = `hsla(${p.hue}, 100%, 65%, ${alpha})`;
    ctx.shadowColor = `hsla(${p.hue}, 100%, 60%, ${alpha})`;
    ctx.shadowBlur = 8;
    ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
    ctx.fill();
    ctx.shadowBlur = 0;
  }

  // Rising Japanese glyphs (matrix-ish columns feel without full matrix)
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (let i = 0; i < glyphs.length; i++) {
    const g = glyphs[i];
    g.y -= g.spd * dt;
    g.blink += dt * 3;
    if (g.y < -30) {
      glyphs[i] = makeGlyph(false);
      continue;
    }
    // occasional glyph swap = "data flicker"
    if (Math.random() < 0.008) g.ch = pickGlyph();
    const flicker = 0.65 + 0.35 * Math.sin(g.blink + i);
    const alpha = g.a * flicker;
    ctx.font = `${g.size}px "Segoe UI", "Yu Gothic", "Meiryo", monospace`;
    ctx.fillStyle = `hsla(${g.hue}, 100%, 70%, ${alpha})`;
    ctx.shadowColor = `hsla(${g.hue}, 100%, 55%, ${alpha * 0.9})`;
    ctx.shadowBlur = 10;
    ctx.fillText(g.ch, g.x, g.y);
    // trailing dim glyph
    ctx.shadowBlur = 0;
    ctx.fillStyle = `hsla(${g.hue}, 90%, 55%, ${alpha * 0.25})`;
    ctx.fillText(g.ch, g.x, g.y + g.size * 1.15);
  }
}

function start() {
  if (running || !props.animate) return;
  running = true;
  lastTs = 0;
  resize();
  raf = requestAnimationFrame(paint);
}

function stop() {
  running = false;
  cancelAnimationFrame(raf);
  raf = 0;
  const canvas = canvasRef.value;
  const ctx = canvas?.getContext('2d');
  if (ctx && w && h) ctx.clearRect(0, 0, w, h);
}

let ro: ResizeObserver | null = null;

onMounted(() => {
  resize();
  ro = new ResizeObserver(() => {
    resize();
  });
  if (canvasRef.value?.parentElement) ro.observe(canvasRef.value.parentElement);
  window.addEventListener('resize', resize);
  if (props.animate) start();
});

onUnmounted(() => {
  stop();
  ro?.disconnect();
  ro = null;
  window.removeEventListener('resize', resize);
});

watch(
  () => props.animate,
  (on) => {
    if (on) start();
    else stop();
  },
);
</script>

<style scoped>
.cp-backdrop {
  --cp-magenta: #ff2bd6;
  --cp-cyan: #00e5ff;
  --cp-violet: #6b21ff;
  background: #03010a;
}

.cp-backdrop__base {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse 100% 80% at 50% 120%, rgba(90, 20, 160, 0.45) 0%, transparent 55%),
    radial-gradient(ellipse 70% 55% at 10% 15%, rgba(255, 43, 214, 0.22) 0%, transparent 52%),
    radial-gradient(ellipse 65% 50% at 90% 18%, rgba(0, 229, 255, 0.18) 0%, transparent 50%),
    linear-gradient(180deg, #12081f 0%, #070312 40%, #020008 100%);
}

/* Soft far skyline silhouettes */
.cp-backdrop__skyline {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 38%;
  background:
    linear-gradient(180deg, transparent 0%, rgba(2, 0, 8, 0.55) 100%),
    repeating-linear-gradient(
      90deg,
      transparent 0 18px,
      rgba(255, 43, 214, 0.04) 18px 19px,
      transparent 19px 40px,
      rgba(0, 229, 255, 0.03) 40px 41px
    );
  opacity: 0.9;
  mask-image: linear-gradient(180deg, transparent 0%, #000 35%);
  -webkit-mask-image: linear-gradient(180deg, transparent 0%, #000 35%);
}

.cp-backdrop__skyline::before,
.cp-backdrop__skyline::after {
  content: '';
  position: absolute;
  bottom: 0;
  width: 100%;
  height: 100%;
  background-repeat: no-repeat;
  background-position: bottom center;
  background-size: 100% 100%;
  opacity: 0.55;
}

/* blocky building silhouettes via layered gradients */
.cp-backdrop__skyline::before {
  background-image:
    linear-gradient(rgba(12, 6, 24, 0.95), rgba(12, 6, 24, 0.95)),
    linear-gradient(rgba(10, 5, 20, 0.9), rgba(10, 5, 20, 0.9)),
    linear-gradient(rgba(14, 8, 28, 0.92), rgba(14, 8, 28, 0.92)),
    linear-gradient(rgba(8, 4, 18, 0.95), rgba(8, 4, 18, 0.95)),
    linear-gradient(rgba(16, 8, 30, 0.9), rgba(16, 8, 30, 0.9)),
    linear-gradient(rgba(10, 6, 22, 0.92), rgba(10, 6, 22, 0.92));
  background-size:
    7% 42%, 5% 58%, 9% 48%, 6% 70%, 8% 52%, 5% 62%;
  background-position:
    4% bottom, 14% bottom, 24% bottom, 38% bottom, 55% bottom, 72% bottom;
}

.cp-backdrop__skyline::after {
  background-image:
    linear-gradient(rgba(12, 6, 24, 0.95), rgba(12, 6, 24, 0.95)),
    linear-gradient(rgba(14, 8, 28, 0.9), rgba(14, 8, 28, 0.9)),
    linear-gradient(rgba(8, 4, 18, 0.95), rgba(8, 4, 18, 0.95)),
    linear-gradient(rgba(16, 8, 30, 0.9), rgba(16, 8, 30, 0.9));
  background-size:
    6% 64%, 10% 46%, 5% 72%, 8% 54%;
  background-position:
    82% bottom, 90% bottom, 62% bottom, 48% bottom;
  opacity: 0.5;
}

.cp-backdrop__glow {
  position: absolute;
  border-radius: 50%;
  filter: blur(52px);
  opacity: 0.55;
}

.cp-backdrop__glow--magenta {
  width: min(58vw, 560px);
  height: min(58vw, 560px);
  left: -10%;
  bottom: 0%;
  background: radial-gradient(circle, rgba(255, 43, 214, 0.6) 0%, transparent 70%);
}

.cp-backdrop__glow--cyan {
  width: min(52vw, 480px);
  height: min(52vw, 480px);
  right: -8%;
  top: 4%;
  background: radial-gradient(circle, rgba(0, 229, 255, 0.5) 0%, transparent 70%);
}

.cp-backdrop__glow--violet {
  width: min(70vw, 640px);
  height: min(40vw, 360px);
  left: 20%;
  bottom: 8%;
  background: radial-gradient(circle, rgba(107, 33, 255, 0.4) 0%, transparent 70%);
  opacity: 0.4;
}

.cp-backdrop__grid {
  position: absolute;
  inset: -15% 0 0 0;
  background-image:
    linear-gradient(rgba(0, 229, 255, 0.09) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 43, 214, 0.07) 1px, transparent 1px);
  background-size: 44px 44px;
  transform: perspective(480px) rotateX(60deg) translateY(-6%);
  transform-origin: center top;
  mask-image: linear-gradient(180deg, transparent 0%, #000 22%, #000 68%, transparent 100%);
  -webkit-mask-image: linear-gradient(180deg, transparent 0%, #000 22%, #000 68%, transparent 100%);
  opacity: 0.9;
}

.cp-backdrop__horizon {
  position: absolute;
  left: -10%;
  right: -10%;
  top: 40%;
  height: 2px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 43, 214, 0.2) 12%,
    rgba(0, 229, 255, 0.95) 50%,
    rgba(255, 43, 214, 0.25) 88%,
    transparent 100%
  );
  box-shadow:
    0 0 20px rgba(0, 229, 255, 0.65),
    0 0 56px rgba(255, 43, 214, 0.3);
}

.cp-backdrop__fx {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
}

.cp-backdrop__scanlines {
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(
    0deg,
    transparent 0px,
    transparent 2px,
    rgba(0, 0, 0, 0.2) 2px,
    rgba(0, 0, 0, 0.2) 3px
  );
  opacity: 0.32;
  mix-blend-mode: multiply;
}

.cp-backdrop__beam {
  position: absolute;
  left: 0;
  right: 0;
  height: 26%;
  background: linear-gradient(
    180deg,
    transparent 0%,
    rgba(0, 229, 255, 0.045) 42%,
    rgba(255, 43, 214, 0.06) 52%,
    transparent 100%
  );
  animation: cp-beam 10s linear infinite;
  opacity: 0.65;
}

.cp-backdrop__vignette {
  position: absolute;
  inset: 0;
  background: radial-gradient(ellipse 78% 72% at 50% 42%, transparent 38%, rgba(1, 0, 6, 0.82) 100%);
}

.cp-backdrop--motion .cp-backdrop__glow--magenta {
  animation: cp-pulse-m 7s ease-in-out infinite;
}

.cp-backdrop--motion .cp-backdrop__glow--cyan {
  animation: cp-pulse-c 8.5s ease-in-out infinite;
}

@keyframes cp-beam {
  0% { transform: translateY(-45%); }
  100% { transform: translateY(300%); }
}

@keyframes cp-pulse-m {
  0%, 100% { opacity: 0.4; transform: scale(1); }
  50% { opacity: 0.75; transform: scale(1.07); }
}

@keyframes cp-pulse-c {
  0%, 100% { opacity: 0.35; transform: scale(1.02); }
  50% { opacity: 0.7; transform: scale(1); }
}

@media (prefers-reduced-motion: reduce) {
  .cp-backdrop__beam {
    animation: none;
    display: none;
  }
  .cp-backdrop--motion .cp-backdrop__glow--magenta,
  .cp-backdrop--motion .cp-backdrop__glow--cyan {
    animation: none;
  }
}
</style>
