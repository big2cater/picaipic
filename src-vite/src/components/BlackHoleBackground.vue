<template>
  <canvas
    ref="canvasRef"
    class="pointer-events-none fixed inset-0 z-0"
    aria-hidden="true"
  />
</template>

<script setup lang="ts">
/**
 * Cosmic black-hole ambient layer (design v1.4).
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

// Target ~24fps for ambient background
const FRAME_MS = 1000 / 24;
let lastFrameWall = 0;

// Internal render scale (performance switch)
const INTERNAL_SCALE = 0.5;

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

float starField(vec2 uv, float density) {
  vec2 g = floor(uv * density);
  vec2 f = fract(uv * density) - 0.5;
  float n = hash(g);
  float d = length(f);
  float s = smoothstep(0.03, 0.0, d) * step(0.97, n);
  return s * (0.6 + 0.4 * sin(u_time * 2.0 + n * 40.0));
}

void main() {
  vec2 frag = gl_FragCoord.xy;
  vec2 center = u_res * 0.5;
  vec2 p = frag - center;
  float dist = length(p);
  float angle = atan(p.y, p.x);

  // Cosmic base: dark navy vs slightly lifted indigo for light appearance
  vec3 deep = mix(vec3(0.04, 0.05, 0.12), vec3(0.01, 0.01, 0.03), u_appearance);
  vec3 base = deep;

  // Screen-space mild radial "lensing" for star sampling
  float lens = 1.0 + 0.08 * u_gravity * smoothstep(u_rInf, u_rEvent * 1.2, dist);
  vec2 suv = (center + p / lens) / max(u_res.y, 1.0);

  float stars = starField(suv + vec2(u_time * 0.003, 0.0), 90.0)
              + starField(suv * 1.7 - vec2(0.0, u_time * 0.002), 160.0) * 0.6;
  float starAmt = mix(0.45, 0.85, u_appearance);
  base += vec3(stars) * starAmt;

  // Soft nebula
  float neb = sin(suv.x * 6.0 + u_time * 0.05) * sin(suv.y * 4.0 - u_time * 0.04);
  neb = neb * 0.5 + 0.5;
  vec3 nebCol = mix(vec3(0.15, 0.18, 0.28), vec3(0.12, 0.05, 0.22), u_appearance);
  base = mix(base, nebCol, 0.08 + 0.06 * neb);

  // Accretion disk (ellipse in rotated frame)
  float ca = cos(u_angle);
  float sa = sin(u_angle);
  vec2 rp = vec2(p.x * ca + p.y * sa, -p.x * sa + p.y * ca);
  float er = length(vec2(rp.x / 1.35, rp.y / 0.42));
  float diskBand = smoothstep(u_rEvent * 0.95, u_rEvent * 1.05, er)
                 * (1.0 - smoothstep(u_rEvent * 1.55, u_rEvent * 2.1, er));
  // Pseudo-Doppler: left brighter
  float doppler = 0.55 + 0.45 * cos(angle - u_angle);
  vec3 diskWarm = mix(u_primary, vec3(1.0, 0.55, 0.25), 0.35);
  vec3 diskCool = mix(u_primary * 0.5, vec3(0.3, 0.45, 1.0), 0.4);
  vec3 diskCol = mix(diskCool, diskWarm, doppler);
  float diskBoost = mix(0.55, 0.9, u_appearance) * (1.0 + 0.25 * u_gravity);
  base += diskCol * diskBand * doppler * diskBoost;

  // Photon ring
  float ring = abs(dist - u_rEvent * 1.08);
  float ringGlow = exp(-ring * ring * 0.015) * (0.5 + 0.5 * u_gravity);
  base += mix(u_primary, vec3(1.0), 0.25) * ringGlow * mix(0.5, 0.85, u_appearance);

  // Soft halo out to influence radius
  float halo = 1.0 - smoothstep(u_rEvent, u_rInf * 0.55, dist);
  base += u_primary * halo * 0.08 * (1.0 + 0.3 * u_gravity);

  // Event horizon
  float hole = smoothstep(u_rEvent * 1.02, u_rEvent * 0.92, dist);
  base = mix(base, vec3(0.0), hole);

  // Vignette for light mode readability of UI edges
  float vig = smoothstep(1.2, 0.35, dist / max(u_res.y, 1.0));
  base *= mix(0.92 + 0.08 * vig, 1.0, u_appearance);

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
    canvas.getContext('webgl', { alpha: false, antialias: false, depth: false }) ||
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
  c.width = bw;
  c.height = bh;
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
  diskAngle += dt * 0.12;

  const w = window.innerWidth;
  const h = window.innerHeight;
  const HX = w / 2;
  const HY = h / 2;
  const elapsed = props.gravityActive ? props.effectiveElapsedSec : 0;
  const { R_event, R_inf } = computeRadii(elapsed, w, h);
  emit('radii', { R_event, R_inf });

  const dark = (props.appearance ?? 1) !== 0;
  ctx.fillStyle = dark ? '#03040a' : '#0a1020';
  ctx.fillRect(0, 0, w, h);

  // Stars
  const starCount = dark ? 180 : 100;
  for (let i = 0; i < starCount; i++) {
    const sx = (hash2(i, 1) * w);
    const sy = (hash2(i, 2) * h);
    const a = 0.3 + hash2(i, 3) * (dark ? 0.7 : 0.45);
    ctx.fillStyle = `rgba(255,255,255,${a})`;
    ctx.fillRect(sx, sy, 1 + (hash2(i, 4) > 0.9 ? 1 : 0), 1);
  }

  // Nebula
  const neb = ctx.createRadialGradient(HX * 0.7, HY * 1.1, 0, HX, HY, Math.max(w, h) * 0.6);
  const p = primaryColor;
  neb.addColorStop(0, dark ? 'rgba(40,20,80,0.25)' : 'rgba(60,80,120,0.15)');
  neb.addColorStop(0.5, 'rgba(0,0,0,0)');
  neb.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.fillStyle = neb;
  ctx.fillRect(0, 0, w, h);

  const glowBoost = props.gravityActive ? 1.2 : 1;
  const glowR = R_inf * 0.55;
  const glow = ctx.createRadialGradient(HX, HY, R_event * 0.9, HX, HY, glowR);
  glow.addColorStop(0, p);
  glow.addColorStop(0.4, p);
  glow.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.save();
  ctx.globalAlpha = (dark ? 0.4 : 0.28) * glowBoost;
  ctx.fillStyle = glow;
  ctx.beginPath();
  ctx.arc(HX, HY, glowR, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  ctx.save();
  ctx.translate(HX, HY);
  ctx.rotate(diskAngle);
  ctx.globalAlpha = dark ? 0.45 : 0.35;
  ctx.strokeStyle = p;
  ctx.lineWidth = Math.max(2, R_event * 0.08);
  ctx.beginPath();
  ctx.ellipse(0, 0, R_event * 1.35, R_event * 0.45, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();

  ctx.fillStyle = '#000';
  ctx.beginPath();
  ctx.arc(HX, HY, R_event, 0, Math.PI * 2);
  ctx.fill();

  ctx.save();
  ctx.globalAlpha = 0.75;
  ctx.strokeStyle = p;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(HX, HY, R_event * 1.08, 0, Math.PI * 2);
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
  emit('radii', { R_event, R_inf });

  const dt = lastTs ? Math.min(0.05, (ts - lastTs) / 1000) : 0.016;
  lastTs = ts;
  diskAngle += dt * 0.12;

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
