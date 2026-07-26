<template>
  <canvas
    ref="canvasRef"
    class="pointer-events-none fixed inset-0 z-0"
    aria-hidden="true"
  />
</template>

<script setup lang="ts">
/**
 * Cosmic black-hole ambient layer (design v1.4 + quality pass).
 * Prefer WebGL analytical fragment shader; fall back to Canvas2D.
 * No three.js. Photos are warped via CSS elsewhere — not here.
 */
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { computeRadii } from '@/common/blackHoleMath';

const props = defineProps<{
  gravityActive: boolean;
  effectiveElapsedSec: number;
  /** 0 light, 1 dark — tints cosmos */
  appearance?: number;
}>();

const emit = defineEmits<{
  radii: [payload: { R_event: number; R_inf: number }];
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);

type Backend = 'webgl' | '2d';
let backend: Backend = '2d';
let raf = 0;
let lastTs = 0;
let diskAngle = 0;
let paused = typeof document !== 'undefined' ? document.hidden : false;
let primaryColor = '#7c5cff';
let primaryRgb: [number, number, number] = [124, 92, 255];

// WebGL state
let gl: WebGLRenderingContext | null = null;
let program: WebGLProgram | null = null;
let uTime: WebGLUniformLocation | null = null;
let uRes: WebGLUniformLocation | null = null;
let uREvent: WebGLUniformLocation | null = null;
let uRInf: WebGLUniformLocation | null = null;
let uPrimary: WebGLUniformLocation | null = null;
let uAppearance: WebGLUniformLocation | null = null;
let uGravity: WebGLUniformLocation | null = null;
let uAngle: WebGLUniformLocation | null = null;

// Ambient background: slightly above the old 24fps so motion feels smoother
const FRAME_MS = 1000 / 30;
let lastFrameWall = 0;
// Avoid every-frame reactive storms: only emit when radii move > 0.5px
let lastEmittedREvent = Number.NaN;
let lastEmittedRInf = Number.NaN;

function emitRadiiIfChanged(R_event: number, R_inf: number) {
  if (
    Number.isFinite(lastEmittedREvent)
    && Number.isFinite(lastEmittedRInf)
    && Math.abs(R_event - lastEmittedREvent) < 0.5
    && Math.abs(R_inf - lastEmittedRInf) < 0.5
  ) {
    return;
  }
  lastEmittedREvent = R_event;
  lastEmittedRInf = R_inf;
  emit('radii', { R_event, R_inf });
}

// Was 0.5 — half-res upscale made stars/disk look like purple mush.
// 1.0 keeps crisp starfield; still cheap at ~30fps fullscreen quad.
const INTERNAL_SCALE = 1.0;

const VERT = `
attribute vec2 a_pos;
void main() {
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`;

// Analytical approximate black hole + cosmos (not geodesic raytrace)
const FRAG = `
precision mediump float;
uniform vec2 u_res;
uniform float u_time;
uniform float u_rEvent;
uniform float u_rInf;
uniform vec3 u_primary;
uniform float u_appearance; // 0 light, 1 dark
uniform float u_gravity;
uniform float u_angle;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float hash2(vec2 p) {
  return fract(sin(dot(p, vec2(269.5, 183.3))) * 43758.5453);
}

// Soft value noise for nebula (cheap, no texture)
float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) {
    v += a * vnoise(p);
    p = p * 2.03 + vec2(17.1, 9.7);
    a *= 0.5;
  }
  return v;
}

// Multi-size star cells
float starLayer(vec2 uv, float density, float size, float threshold) {
  vec2 g = floor(uv * density);
  vec2 f = fract(uv * density) - 0.5;
  float n = hash(g);
  float n2 = hash2(g);
  float d = length(f);
  float core = smoothstep(size, 0.0, d) * step(threshold, n);
  // faint cross diffraction on brighter stars
  float spike = 0.0;
  if (n > threshold + 0.015) {
    float sx = exp(-abs(f.x) * 90.0) * exp(-abs(f.y) * 14.0);
    float sy = exp(-abs(f.y) * 90.0) * exp(-abs(f.x) * 14.0);
    spike = (sx + sy) * 0.35 * (n - threshold);
  }
  float twinkle = 0.75 + 0.25 * sin(u_time * (1.5 + n2 * 3.0) + n * 40.0);
  return (core + spike) * twinkle;
}

void main() {
  vec2 frag = gl_FragCoord.xy;
  vec2 center = u_res * 0.5;
  vec2 p = frag - center;
  float dist = length(p);
  float angle = atan(p.y, p.x);
  float minDim = max(min(u_res.x, u_res.y), 1.0);

  // Cosmic base
  vec3 deepDark = vec3(0.004, 0.006, 0.018);
  vec3 deepLight = vec3(0.03, 0.04, 0.08);
  vec3 deep = mix(deepLight, deepDark, u_appearance);
  vec3 base = deep;

  // Mild gravity lens for background sampling
  float lensAmt = 0.05 + 0.12 * u_gravity;
  float lens = 1.0 + lensAmt * smoothstep(u_rInf * 0.9, u_rEvent * 1.15, dist);
  vec2 suv = (center + p / lens) / minDim;

  // Multi-layer starfield
  float stars =
      starLayer(suv + vec2(u_time * 0.002, 0.0), 70.0, 0.045, 0.965)
    + starLayer(suv * 1.6 - vec2(0.0, u_time * 0.0015), 140.0, 0.03, 0.975) * 0.75
    + starLayer(suv * 2.8 + vec2(u_time * 0.0008, u_time * 0.0005), 240.0, 0.02, 0.985) * 0.45;
  float starAmt = mix(0.55, 1.05, u_appearance);
  // slight cool tint
  base += vec3(0.92, 0.95, 1.0) * stars * starAmt;

  // Nebula / dust (low contrast, don't compete with photos)
  vec2 nuv = suv * 2.2 + vec2(u_time * 0.012, -u_time * 0.008);
  float neb = fbm(nuv);
  float neb2 = fbm(nuv * 1.7 + 3.1);
  vec3 nebA = mix(vec3(0.12, 0.18, 0.32), vec3(0.10, 0.04, 0.22), u_appearance);
  vec3 nebB = mix(vec3(0.08, 0.14, 0.22), u_primary * 0.35, u_appearance);
  vec3 nebCol = mix(nebA, nebB, neb2);
  float nebMask = smoothstep(0.35, 0.85, neb) * (0.10 + 0.10 * u_appearance);
  // keep nebula away from the hole so disk stays clean
  nebMask *= smoothstep(u_rEvent * 1.6, u_rEvent * 3.2, dist);
  base = mix(base, base + nebCol, nebMask);

  // ---- Accretion disk (inclined ellipse + spiral brightness) ----
  float ca = cos(u_angle);
  float sa = sin(u_angle);
  // slight fixed tilt so disk never looks like a flat circle
  float tilt = 0.55;
  vec2 rp = vec2(p.x * ca + p.y * sa, (-p.x * sa + p.y * ca) / tilt);
  float er = length(rp);
  float diskInner = u_rEvent * 1.05;
  float diskOuter = u_rEvent * 2.35;
  float diskBand = smoothstep(diskInner, diskInner + u_rEvent * 0.12, er)
                 * (1.0 - smoothstep(diskOuter * 0.78, diskOuter, er));

  // Spiral arms / clumpy brightness
  float diskAng = atan(rp.y, rp.x);
  float spiral = 0.55 + 0.45 * sin(diskAng * 3.0 - er * 0.045 + u_time * 0.8);
  spiral *= 0.7 + 0.3 * sin(diskAng * 7.0 + er * 0.02);

  // Pseudo-Doppler beaming (approaching side brighter)
  float doppler = 0.45 + 0.55 * cos(diskAng);

  vec3 diskWarm = mix(u_primary, vec3(1.0, 0.62, 0.28), 0.55);
  vec3 diskHot = mix(vec3(1.0, 0.92, 0.75), u_primary, 0.15);
  vec3 diskCool = mix(u_primary * 0.35, vec3(0.25, 0.35, 0.95), 0.55);
  vec3 diskCol = mix(diskCool, mix(diskWarm, diskHot, spiral), doppler);

  float diskBoost = mix(0.85, 1.25, u_appearance) * (1.0 + 0.35 * u_gravity);
  // brighter near ISCO-ish edge
  float edgeHot = exp(-pow((er - diskInner) / max(u_rEvent * 0.25, 1.0), 2.0));
  base += diskCol * diskBand * spiral * diskBoost * (0.75 + 0.85 * edgeHot);

  // Soft disk glow under the band
  float diskGlow = diskBand * 0.55;
  base += mix(diskWarm, u_primary, 0.4) * diskGlow * 0.35 * diskBoost;

  // ---- Photon ring (thin, bright Einstein ring) ----
  float ringR = u_rEvent * 1.06;
  float ring = abs(dist - ringR);
  float ringCore = exp(-ring * ring / max(u_rEvent * 0.012, 0.5));
  float ringSoft = exp(-ring * ring / max(u_rEvent * 0.08, 2.0));
  float ringAmt = mix(0.7, 1.15, u_appearance) * (0.65 + 0.45 * u_gravity);
  vec3 ringCol = mix(u_primary, vec3(1.0, 0.95, 0.88), 0.55);
  base += ringCol * ringCore * 1.4 * ringAmt;
  base += u_primary * ringSoft * 0.35 * ringAmt;

  // Soft purple/blue halo toward influence radius
  float halo = 1.0 - smoothstep(u_rEvent * 0.9, u_rInf * 0.5, dist);
  base += u_primary * halo * (0.06 + 0.05 * u_gravity);

  // Event horizon — pure black with a soft limb
  float hole = smoothstep(u_rEvent * 1.02, u_rEvent * 0.94, dist);
  base = mix(base, vec3(0.0), hole);
  // very subtle horizon rim (helps silhouette against disk)
  float limb = exp(-pow((dist - u_rEvent * 0.98) / max(u_rEvent * 0.03, 0.5), 2.0));
  base += u_primary * limb * 0.12 * (1.0 - hole);

  // Gentle vignette; stronger only in light appearance for UI edges
  float vig = smoothstep(1.25, 0.25, dist / minDim);
  base *= mix(0.88 + 0.12 * vig, 0.96 + 0.04 * vig, u_appearance);

  // Soft filmic clamp
  base = base / (1.0 + base * 0.35);

  gl_FragColor = vec4(base, 1.0);
}
`;

function readPrimary(): string {
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue('--color-primary')
    .trim();
  if (!v) return '#7c5cff';
  if (v.includes('(') || v.startsWith('#') || v.startsWith('rgb') || v.startsWith('hsl')) {
    return v;
  }
  if (/^\d/.test(v)) {
    return `rgb(${v.split(/\s+/).slice(0, 3).join(',')})`;
  }
  return v;
}

function parseRgb(css: string): [number, number, number] {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 1;
  const ctx = canvas.getContext('2d');
  if (!ctx) return [124, 92, 255];
  ctx.fillStyle = '#000';
  ctx.fillStyle = css;
  ctx.fillRect(0, 0, 1, 1);
  const d = ctx.getImageData(0, 0, 1, 1).data;
  return [d[0], d[1], d[2]];
}

function refreshPrimary() {
  primaryColor = readPrimary();
  primaryRgb = parseRgb(primaryColor);
}

function compile(glCtx: WebGLRenderingContext, type: number, src: string): WebGLShader | null {
  const sh = glCtx.createShader(type);
  if (!sh) return null;
  glCtx.shaderSource(sh, src);
  glCtx.compileShader(sh);
  if (!glCtx.getShaderParameter(sh, glCtx.COMPILE_STATUS)) {
    console.warn('BlackHoleBackground shader compile failed', glCtx.getShaderInfoLog(sh));
    glCtx.deleteShader(sh);
    return null;
  }
  return sh;
}

function initWebGL(canvas: HTMLCanvasElement): boolean {
  const ctx =
    canvas.getContext('webgl', { alpha: false, antialias: false, depth: false, powerPreference: 'low-power' }) ||
    (canvas.getContext('experimental-webgl', { alpha: false, antialias: false }) as WebGLRenderingContext | null);
  if (!ctx) return false;
  gl = ctx;

  const vs = compile(gl, gl.VERTEX_SHADER, VERT);
  const fs = compile(gl, gl.FRAGMENT_SHADER, FRAG);
  if (!vs || !fs) {
    gl = null;
    return false;
  }
  const prog = gl.createProgram();
  if (!prog) {
    gl = null;
    return false;
  }
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    console.warn('BlackHoleBackground program link failed', gl.getProgramInfoLog(prog));
    gl = null;
    return false;
  }
  program = prog;
  gl.useProgram(program);

  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(program, 'a_pos');
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);

  uTime = gl.getUniformLocation(program, 'u_time');
  uRes = gl.getUniformLocation(program, 'u_res');
  uREvent = gl.getUniformLocation(program, 'u_rEvent');
  uRInf = gl.getUniformLocation(program, 'u_rInf');
  uPrimary = gl.getUniformLocation(program, 'u_primary');
  uAppearance = gl.getUniformLocation(program, 'u_appearance');
  uGravity = gl.getUniformLocation(program, 'u_gravity');
  uAngle = gl.getUniformLocation(program, 'u_angle');

  return true;
}

function resize() {
  const c = canvasRef.value;
  if (!c) return;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = window.innerWidth;
  const h = window.innerHeight;
  const scale = backend === 'webgl' ? INTERNAL_SCALE : 1;
  const bw = Math.max(1, Math.floor(w * dpr * scale));
  const bh = Math.max(1, Math.floor(h * dpr * scale));
  if (c.width !== bw || c.height !== bh) {
    c.width = bw;
    c.height = bh;
  }
  c.style.width = `${w}px`;
  c.style.height = `${h}px`;
  if (backend === '2d') {
    const ctx = c.getContext('2d');
    if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  } else if (gl) {
    gl.viewport(0, 0, bw, bh);
  }
  refreshPrimary();
}

function paint2d(ts: number) {
  const c = canvasRef.value;
  if (!c) return;
  const ctx = c.getContext('2d');
  if (!ctx) return;

  const dt = lastTs ? Math.min(0.05, (ts - lastTs) / 1000) : 0.016;
  lastTs = ts;
  diskAngle += dt * 0.14;

  const w = window.innerWidth;
  const h = window.innerHeight;
  const HX = w / 2;
  const HY = h / 2;
  const elapsed = props.gravityActive ? props.effectiveElapsedSec : 0;
  const { R_event, R_inf } = computeRadii(elapsed, w, h);
  emitRadiiIfChanged(R_event, R_inf);

  const dark = (props.appearance ?? 1) !== 0;
  ctx.fillStyle = dark ? '#02030a' : '#0a1020';
  ctx.fillRect(0, 0, w, h);

  // Dense starfield
  const starCount = dark ? 320 : 180;
  for (let i = 0; i < starCount; i++) {
    const sx = hash2(i, 1) * w;
    const sy = hash2(i, 2) * h;
    const a = 0.25 + hash2(i, 3) * (dark ? 0.8 : 0.5);
    const sz = hash2(i, 4) > 0.92 ? 2 : 1;
    ctx.fillStyle = `rgba(230,235,255,${a})`;
    ctx.fillRect(sx, sy, sz, sz);
  }

  // Nebula blobs
  const neb = ctx.createRadialGradient(HX * 0.65, HY * 1.15, 0, HX, HY, Math.max(w, h) * 0.65);
  neb.addColorStop(0, dark ? 'rgba(55,25,110,0.28)' : 'rgba(60,90,140,0.16)');
  neb.addColorStop(0.45, dark ? 'rgba(20,10,50,0.12)' : 'rgba(40,60,100,0.06)');
  neb.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.fillStyle = neb;
  ctx.fillRect(0, 0, w, h);

  const p = primaryColor;
  const glowBoost = props.gravityActive ? 1.25 : 1;
  const glowR = Math.max(R_inf * 0.55, R_event * 2.2);
  const glow = ctx.createRadialGradient(HX, HY, R_event * 0.85, HX, HY, glowR);
  glow.addColorStop(0, p);
  glow.addColorStop(0.35, p);
  glow.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.save();
  ctx.globalAlpha = (dark ? 0.42 : 0.3) * glowBoost;
  ctx.fillStyle = glow;
  ctx.beginPath();
  ctx.arc(HX, HY, glowR, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  // Soft filled disk body under the ring stroke
  ctx.save();
  ctx.translate(HX, HY);
  ctx.rotate(diskAngle);
  ctx.scale(1, 0.42);
  const diskGrad = ctx.createRadialGradient(0, 0, R_event * 0.95, 0, 0, R_event * 2.2);
  diskGrad.addColorStop(0, 'rgba(0,0,0,0)');
  diskGrad.addColorStop(0.35, p);
  diskGrad.addColorStop(0.7, p);
  diskGrad.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.globalAlpha = (dark ? 0.55 : 0.4) * glowBoost;
  ctx.fillStyle = diskGrad;
  ctx.beginPath();
  ctx.arc(0, 0, R_event * 2.2, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalAlpha = dark ? 0.7 : 0.55;
  ctx.strokeStyle = '#fff6e8';
  ctx.lineWidth = Math.max(1.5, R_event * 0.05);
  ctx.beginPath();
  ctx.arc(0, 0, R_event * 1.35, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();

  // Event horizon
  ctx.fillStyle = '#000';
  ctx.beginPath();
  ctx.arc(HX, HY, R_event, 0, Math.PI * 2);
  ctx.fill();

  // Photon ring
  ctx.save();
  ctx.globalAlpha = 0.9;
  ctx.strokeStyle = p;
  ctx.lineWidth = Math.max(1.5, R_event * 0.04);
  ctx.beginPath();
  ctx.arc(HX, HY, R_event * 1.06, 0, Math.PI * 2);
  ctx.stroke();
  ctx.globalAlpha = 0.35;
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.arc(HX, HY, R_event * 1.06, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
}

function hash2(i: number, s: number): number {
  const x = Math.sin(i * 12.9898 + s * 78.233) * 43758.5453;
  return x - Math.floor(x);
}

function paintWebGL(ts: number) {
  if (!gl || !program) return;
  const w = window.innerWidth;
  const h = window.innerHeight;
  const elapsed = props.gravityActive ? props.effectiveElapsedSec : 0;
  const { R_event, R_inf } = computeRadii(elapsed, w, h);
  emitRadiiIfChanged(R_event, R_inf);

  const dt = lastTs ? Math.min(0.05, (ts - lastTs) / 1000) : 0.016;
  lastTs = ts;
  diskAngle += dt * 0.14;

  // Radii are in CSS pixels; internal buffer is scaled — pass buffer-space radii
  const scale = INTERNAL_SCALE * Math.min(window.devicePixelRatio || 1, 2);
  const bw = gl.drawingBufferWidth;
  const bh = gl.drawingBufferHeight;

  gl.useProgram(program);
  if (uTime) gl.uniform1f(uTime, ts * 0.001);
  if (uRes) gl.uniform2f(uRes, bw, bh);
  if (uREvent) gl.uniform1f(uREvent, R_event * scale);
  if (uRInf) gl.uniform1f(uRInf, R_inf * scale);
  if (uPrimary) {
    gl.uniform3f(uPrimary, primaryRgb[0] / 255, primaryRgb[1] / 255, primaryRgb[2] / 255);
  }
  if (uAppearance) gl.uniform1f(uAppearance, (props.appearance ?? 1) !== 0 ? 1 : 0);
  if (uGravity) gl.uniform1f(uGravity, props.gravityActive ? 1 : 0);
  if (uAngle) gl.uniform1f(uAngle, diskAngle);

  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

function paint(ts: number) {
  raf = requestAnimationFrame(paint);
  if (paused) return;
  if (ts - lastFrameWall < FRAME_MS) return;
  lastFrameWall = ts;

  if (backend === 'webgl') paintWebGL(ts);
  else paint2d(ts);
}

function onVis() {
  paused = document.hidden;
  if (!paused) {
    lastTs = 0;
    refreshPrimary();
  }
}

onMounted(() => {
  const c = canvasRef.value;
  if (!c) return;
  refreshPrimary();
  if (initWebGL(c)) {
    backend = 'webgl';
  } else {
    backend = '2d';
    gl = null;
    program = null;
  }
  resize();
  window.addEventListener('resize', resize);
  document.addEventListener('visibilitychange', onVis);
  raf = requestAnimationFrame(paint);
});

onUnmounted(() => {
  cancelAnimationFrame(raf);
  window.removeEventListener('resize', resize);
  document.removeEventListener('visibilitychange', onVis);
  if (gl && program) {
    gl.deleteProgram(program);
  }
  gl = null;
  program = null;
});

watch(
  () => props.appearance,
  () => refreshPrimary(),
);
</script>
