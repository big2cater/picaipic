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
 * CSS: grid / neon / skyline. Canvas: rain, particles, kana via pre-baked sprites
 * (no per-frame shadowBlur / createLinearGradient).
 */
import { onMounted, onUnmounted, ref, watch } from 'vue';

const props = withDefaults(
  defineProps<{
    animate?: boolean;
  }>(),
  { animate: true },
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

const GLYPHS =
  'アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン電脳都市夜雨霓虹未来記憶断片回路';

type RainDrop = { x: number; y: number; spd: number; scale: number; a: number };
type Particle = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  scale: number;
  a: number;
  sprite: number; // 0 magenta, 1 cyan
  life: number;
  maxLife: number;
};
type Glyph = {
  x: number;
  y: number;
  ch: string;
  spd: number;
  scale: number;
  a: number;
  sprite: number;
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
let ctx: CanvasRenderingContext2D | null = null;

// Pre-baked sprites (radial glow / rain streak / glyph cells)
let particleSprites: HTMLCanvasElement[] = [];
let rainSprite: HTMLCanvasElement | null = null;
const glyphSpriteCache = new Map<string, HTMLCanvasElement>();

function rand(a = 0, b = 1) {
  return a + Math.random() * (b - a);
}

function pickGlyph() {
  return GLYPHS[(Math.random() * GLYPHS.length) | 0] || 'ア';
}

function makeGlowSprite(hue: number, radius = 10): HTMLCanvasElement {
  const pad = radius * 3;
  const size = Math.ceil(pad * 2);
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d')!;
  const cx = size / 2;
  const grd = g.createRadialGradient(cx, cx, 0, cx, cx, pad);
  grd.addColorStop(0, `hsla(${hue}, 100%, 70%, 1)`);
  grd.addColorStop(0.35, `hsla(${hue}, 100%, 60%, 0.55)`);
  grd.addColorStop(1, `hsla(${hue}, 100%, 50%, 0)`);
  g.fillStyle = grd;
  g.beginPath();
  g.arc(cx, cx, pad, 0, Math.PI * 2);
  g.fill();
  return c;
}

function makeRainSprite(): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = 8;
  c.height = 40;
  const g = c.getContext('2d')!;
  const grd = g.createLinearGradient(4, 0, 1, 40);
  grd.addColorStop(0, 'rgba(180, 220, 255, 0)');
  grd.addColorStop(0.35, 'rgba(160, 210, 255, 0.85)');
  grd.addColorStop(1, 'rgba(0, 229, 255, 0)');
  g.strokeStyle = grd;
  g.lineWidth = 1.4;
  g.lineCap = 'round';
  g.beginPath();
  g.moveTo(5, 0);
  g.lineTo(2, 40);
  g.stroke();
  return c;
}

function makeGlyphSprite(ch: string, hue: number): HTMLCanvasElement {
  const key = `${ch}:${hue}`;
  const hit = glyphSpriteCache.get(key);
  if (hit) return hit;
  const size = 48;
  const c = document.createElement('canvas');
  c.width = c.height = size;
  const g = c.getContext('2d')!;
  g.font = '22px "Segoe UI", "Yu Gothic", "Meiryo", monospace';
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  // soft glow baked once
  g.shadowColor = `hsla(${hue}, 100%, 55%, 0.9)`;
  g.shadowBlur = 12;
  g.fillStyle = `hsla(${hue}, 100%, 72%, 1)`;
  g.fillText(ch, size / 2, size / 2 - 4);
  g.shadowBlur = 0;
  g.fillStyle = `hsla(${hue}, 90%, 55%, 0.28)`;
  g.fillText(ch, size / 2, size / 2 + 14);
  glyphSpriteCache.set(key, c);
  return c;
}

function bakeSprites() {
  particleSprites = [makeGlowSprite(318, 9), makeGlowSprite(188, 9)];
  rainSprite = makeRainSprite();
  glyphSpriteCache.clear();
  // warm common glyphs
  for (const ch of 'アイウエオカキクケコ電脳都市夜雨') {
    makeGlyphSprite(ch, 318);
    makeGlyphSprite(ch, 188);
  }
}

function resize() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const parent = canvas.parentElement;
  const rect = parent?.getBoundingClientRect() ?? {
    width: window.innerWidth,
    height: window.innerHeight,
  };
  dpr = Math.min(window.devicePixelRatio || 1, 1.5);
  w = Math.max(1, Math.floor(rect.width));
  h = Math.max(1, Math.floor(rect.height));
  canvas.width = Math.floor(w * dpr);
  canvas.height = Math.floor(h * dpr);
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  ctx = canvas.getContext('2d');
  if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  // force=false: only rebuild field when count thresholds change — avoids
  // restarting/jumping rain during continuous ResizeObserver while dragging window
  seedField(false);
}

function seedField(force = false) {
  const area = w * h;
  const rainN = Math.min(180, Math.max(70, Math.floor(area / 10000)));
  const partN = Math.min(70, Math.max(24, Math.floor(area / 26000)));
  const glyphN = Math.min(28, Math.max(10, Math.floor(area / 60000)));

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
    spd: rand(520, 980),
    scale: rand(0.7, 1.35),
    a: rand(0.18, 0.5),
  };
}

function makeParticle(scatter: boolean): Particle {
  return {
    x: rand(0, w),
    y: scatter ? rand(0, h) : h + rand(0, 40),
    vx: rand(-12, 12),
    vy: rand(-28, -8),
    scale: rand(0.35, 0.85),
    a: rand(0.3, 0.9),
    sprite: Math.random() > 0.45 ? 0 : 1,
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
    scale: rand(0.55, 0.95),
    a: rand(0.22, 0.6),
    sprite: magenta ? 0 : 1,
    blink: rand(0, Math.PI * 2),
  };
}

function paint(ts: number) {
  if (!running || !ctx) return;
  raf = requestAnimationFrame(paint);

  const dt = lastTs ? Math.min(0.05, (ts - lastTs) / 1000) : 0.016;
  lastTs = ts;

  ctx.clearRect(0, 0, w, h);

  // Rain — drawImage sprites only
  const rs = rainSprite;
  if (rs) {
    for (let i = 0; i < rain.length; i++) {
      const d = rain[i];
      d.y += d.spd * dt;
      d.x += 28 * dt;
      if (d.y > h + 40) {
        rain[i] = makeRain(false);
        continue;
      }
      if (d.x > w + 20) d.x = -10;
      ctx.globalAlpha = d.a;
      const dw = 6 * d.scale;
      const dh = 36 * d.scale;
      ctx.drawImage(rs, d.x - dw * 0.5, d.y, dw, dh);
    }
  }

  // Particles
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
    const spr = particleSprites[p.sprite];
    if (!spr) continue;
    const size = spr.width * p.scale * 0.55;
    ctx.globalAlpha = p.a * fade;
    ctx.drawImage(spr, p.x - size * 0.5, p.y - size * 0.5, size, size);
  }

  // Glyphs (baked glow)
  for (let i = 0; i < glyphs.length; i++) {
    const g = glyphs[i];
    g.y -= g.spd * dt;
    g.blink += dt * 3;
    if (g.y < -40) {
      glyphs[i] = makeGlyph(false);
      continue;
    }
    if (Math.random() < 0.008) g.ch = pickGlyph();
    const flicker = 0.65 + 0.35 * Math.sin(g.blink + i);
    const hue = g.sprite === 0 ? 318 : 188;
    const spr = makeGlyphSprite(g.ch, hue);
    const size = 48 * g.scale;
    ctx.globalAlpha = g.a * flicker;
    ctx.drawImage(spr, g.x - size * 0.5, g.y - size * 0.5, size, size);
  }

  ctx.globalAlpha = 1;
}

function start() {
  if (running || !props.animate) return;
  running = true;
  lastTs = 0;
  if (!particleSprites.length) bakeSprites();
  resize();
  raf = requestAnimationFrame(paint);
}

function stop() {
  running = false;
  cancelAnimationFrame(raf);
  raf = 0;
  if (ctx && w && h) ctx.clearRect(0, 0, w, h);
}

let ro: ResizeObserver | null = null;

onMounted(() => {
  bakeSprites();
  resize();
  ro = new ResizeObserver(() => {
    resize();
  });
  if (canvasRef.value?.parentElement) ro.observe(canvasRef.value.parentElement);
  // ResizeObserver covers window resize of the fixed full-viewport parent
  if (props.animate) start();
});

onUnmounted(() => {
  stop();
  ro?.disconnect();
  ro = null;
  particleSprites = [];
  rainSprite = null;
  glyphSpriteCache.clear();
  ctx = null;
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
