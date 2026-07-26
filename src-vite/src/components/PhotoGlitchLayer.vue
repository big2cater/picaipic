<template>
  <canvas
    ref="canvasRef"
    class="absolute inset-0 w-full h-full pointer-events-none"
    :class="ready ? 'opacity-100' : 'opacity-0'"
    aria-hidden="true"
  />
</template>

<script setup lang="ts">
/**
 * Continuous FragCoord-style photo-area glitch (cyberpunk idle FX).
 * Freezes a snapshot of the grid, then runs time-driven glitch until inactive.
 * WebGL1 only — mediump-safe hash (no sin*43758 overflow).
 */
import { onMounted, onUnmounted, ref, watch } from 'vue';

const props = defineProps<{
  active: boolean;
  sourceEl: HTMLElement | null;
  /** Effect strength (0.5 / 1 / 1.5). Parent skips active when intensity is 0. */
  intensity?: number;
}>();

const emit = defineEmits<{
  captured: [];
  cleared: [];
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
const ready = ref(false);

let gl: WebGLRenderingContext | null = null;
let program: WebGLProgram | null = null;
let tex: WebGLTexture | null = null;
let buf: WebGLBuffer | null = null;
let raf = 0;
let hasTexture = false;

let uRes: WebGLUniformLocation | null = null;
let uTime: WebGLUniformLocation | null = null;
let uTex: WebGLUniformLocation | null = null;
let uIntensity: WebGLUniformLocation | null = null;

// Cached CSS size of sourceEl — measure only on ResizeObserver / beginSession
let cachedCssW = 0;
let cachedCssH = 0;
let bufW = 0;
let bufH = 0;
let sourceRo: ResizeObserver | null = null;
let observedEl: HTMLElement | null = null;

const VERT = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`;

// mediump-safe: no sin(dot)*43758 (overflows ±2^14). Time wrapped with mod.
const FRAG = `
precision mediump float;
varying vec2 v_uv;
uniform vec2 u_res;
uniform float u_time;
uniform sampler2D u_tex;
uniform float u_intensity;

// mediump-safe hash — keep intermediates in ~[0,1]
float rand(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

mat2 Rot(float a) {
  float s = sin(a), c = cos(a);
  return mat2(c, -s, s, c);
}

void main() {
  vec2 uv = v_uv;
  float intensity = max(u_intensity, 0.0);
  // Periodic time — avoids large sin/rand arguments after long idle
  float tt = mod(u_time, 64.0);

  float glitch = step(0.92, rand(vec2(floor(tt * 8.0), 0.5)));
  float burst = step(0.96, rand(vec2(floor(tt * 1.7), 7.3)));
  glitch = max(glitch, burst);

  float lineY = floor(uv.y * (40.0 + 40.0 * intensity));
  float lineNoise = rand(vec2(lineY, floor(tt * 12.0))) * 2.0 - 1.0;
  uv.x += lineNoise * 0.04 * intensity * glitch;

  float blockY = floor(uv.y * (8.0 + 8.0 * intensity));
  float blockNoise = rand(vec2(blockY, floor(tt * 3.0))) * 2.0 - 1.0;
  float blockOn = step(0.7, rand(vec2(blockY, floor(tt * 5.0))));
  uv.x += blockNoise * 0.12 * intensity * glitch * blockOn;

  vec2 center = vec2(0.5);
  vec2 fromC = uv - center;
  float ang = (rand(vec2(floor(tt * 4.0), 1.2)) * 2.0 - 1.0) * 0.04 * intensity * glitch;
  fromC *= Rot(ang);
  uv = fromC + center;

  float ca = 0.008 * intensity * (1.0 + glitch * 2.5);
  float r = texture2D(u_tex, clamp(uv + vec2(ca, 0.0), 0.0, 1.0)).r;
  float g = texture2D(u_tex, clamp(uv, 0.0, 1.0)).g;
  float b = texture2D(u_tex, clamp(uv - vec2(ca, 0.0), 0.0, 1.0)).b;
  vec3 col = vec3(r, g, b);

  // Fixed spatial freq + modded phase (avoid large sin args)
  float scan = 0.92 + 0.08 * sin(mod(uv.y * 220.0 + tt * 8.0, 6.2831853));
  col *= scan;

  col.g = min(1.0, col.g + 0.03 * intensity);
  col.b = min(1.0, col.b + 0.05 * intensity);

  float grain = (rand(uv * 240.0 + fract(tt)) - 0.5) * 0.12 * intensity;
  col += grain;

  float invertThresh = 0.995 - intensity * 0.01;
  float invert = step(invertThresh, rand(vec2(floor(tt * 2.0), 9.1))) * glitch;
  col = mix(col, 1.0 - col, invert);

  gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
`;

function compile(glCtx: WebGLRenderingContext, type: number, src: string): WebGLShader | null {
  const sh = glCtx.createShader(type);
  if (!sh) return null;
  glCtx.shaderSource(sh, src);
  glCtx.compileShader(sh);
  if (!glCtx.getShaderParameter(sh, glCtx.COMPILE_STATUS)) {
    console.warn('PhotoGlitchLayer shader compile failed', glCtx.getShaderInfoLog(sh));
    glCtx.deleteShader(sh);
    return null;
  }
  return sh;
}

function initGl(canvas: HTMLCanvasElement): boolean {
  const ctx =
    canvas.getContext('webgl', { alpha: false, antialias: false, premultipliedAlpha: false }) ||
    (canvas.getContext('experimental-webgl', { alpha: false }) as WebGLRenderingContext | null);
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
    console.warn('PhotoGlitchLayer link failed', gl.getProgramInfoLog(prog));
    gl = null;
    return false;
  }
  program = prog;
  gl.useProgram(program);

  buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const loc = gl.getAttribLocation(program, 'a_pos');
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);

  uRes = gl.getUniformLocation(program, 'u_res');
  uTime = gl.getUniformLocation(program, 'u_time');
  uTex = gl.getUniformLocation(program, 'u_tex');
  uIntensity = gl.getUniformLocation(program, 'u_intensity');

  return true;
}

function measureSource() {
  const src = props.sourceEl;
  if (!src) return;
  const rect = src.getBoundingClientRect();
  cachedCssW = rect.width;
  cachedCssH = rect.height;
}

function applyCanvasSize() {
  const canvas = canvasRef.value;
  if (!canvas || !gl) return;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.floor(cachedCssW * dpr));
  const h = Math.max(1, Math.floor(cachedCssH * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
    bufW = w;
    bufH = h;
  }
  canvas.style.width = `${cachedCssW}px`;
  canvas.style.height = `${cachedCssH}px`;
  gl.viewport(0, 0, w, h);
}

function ensureSourceObserver() {
  const src = props.sourceEl;
  if (!src || typeof ResizeObserver === 'undefined') return;
  if (observedEl === src && sourceRo) return;
  sourceRo?.disconnect();
  observedEl = src;
  sourceRo = new ResizeObserver(() => {
    measureSource();
    if (hasTexture && props.active) applyCanvasSize();
  });
  sourceRo.observe(src);
}

function captureSource(el: HTMLElement): HTMLCanvasElement | null {
  const rect = el.getBoundingClientRect();
  if (rect.width < 2 || rect.height < 2) return null;

  const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
  const out = document.createElement('canvas');
  out.width = Math.max(1, Math.floor(rect.width * dpr));
  out.height = Math.max(1, Math.floor(rect.height * dpr));
  const ctx = out.getContext('2d');
  if (!ctx) return null;

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = '#05060a';
  ctx.fillRect(0, 0, rect.width, rect.height);

  const imgs = el.querySelectorAll('img');
  let drawn = 0;
  imgs.forEach((img) => {
    if (!(img instanceof HTMLImageElement)) return;
    if (!img.complete || img.naturalWidth < 1) return;
    const ir = img.getBoundingClientRect();
    if (ir.right < rect.left || ir.left > rect.right || ir.bottom < rect.top || ir.top > rect.bottom) {
      return;
    }
    const x = ir.left - rect.left;
    const y = ir.top - rect.top;
    try {
      ctx.save();
      const rr = 8;
      const iw = ir.width;
      const ih = ir.height;
      ctx.beginPath();
      ctx.moveTo(x + rr, y);
      ctx.arcTo(x + iw, y, x + iw, y + ih, rr);
      ctx.arcTo(x + iw, y + ih, x, y + ih, rr);
      ctx.arcTo(x, y + ih, x, y, rr);
      ctx.arcTo(x, y, x + iw, y, rr);
      ctx.closePath();
      ctx.clip();
      ctx.drawImage(img, x, y, iw, ih);
      ctx.restore();
      drawn++;
    } catch {
      // taint
    }
  });

  if (drawn === 0) {
    console.warn('PhotoGlitchLayer: no thumbnails drawn into capture');
    return null;
  }
  return out;
}

function uploadTexture(source: HTMLCanvasElement) {
  if (!gl || !program) return false;
  if (tex) {
    gl.deleteTexture(tex);
    tex = null;
  }
  tex = gl.createTexture();
  if (!tex) return false;
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 1);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
  hasTexture = true;
  return true;
}

function paint(ts: number) {
  raf = requestAnimationFrame(paint);
  if (!gl || !program || !hasTexture || !props.active) return;

  // Size only changes on ResizeObserver / beginSession — no per-frame reflow
  if (bufW < 1 || bufH < 1) applyCanvasSize();

  const intensity = Number(props.intensity);
  const intensityVal = Number.isFinite(intensity) ? intensity : 1;

  gl.useProgram(program);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  if (uRes) gl.uniform2f(uRes, bufW, bufH);
  if (uTime) gl.uniform1f(uTime, ts * 0.001);
  if (uIntensity) gl.uniform1f(uIntensity, intensityVal);
  if (uTex) {
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.uniform1i(uTex, 0);
  }
  gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
}

function beginSession() {
  const src = props.sourceEl;
  const canvas = canvasRef.value;
  if (!src || !canvas) return;

  if (!gl) {
    if (!initGl(canvas)) {
      console.warn('PhotoGlitchLayer: WebGL unavailable');
      return;
    }
  }

  ensureSourceObserver();

  requestAnimationFrame(() => {
    if (!props.active || !props.sourceEl) return;
    const snap = captureSource(props.sourceEl);
    if (!snap) return;
    measureSource();
    applyCanvasSize();
    if (!uploadTexture(snap)) return;

    ready.value = true;
    emit('captured');

    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(paint);
  });
}

function endSession() {
  cancelAnimationFrame(raf);
  raf = 0;
  ready.value = false;
  hasTexture = false;
  if (gl && tex) {
    gl.deleteTexture(tex);
    tex = null;
  }
  emit('cleared');
}

function disposeGl() {
  endSession();
  if (gl && program) gl.deleteProgram(program);
  if (gl && buf) gl.deleteBuffer(buf);
  gl = null;
  program = null;
  buf = null;
  sourceRo?.disconnect();
  sourceRo = null;
  observedEl = null;
}

watch(
  () => props.active,
  (on, was) => {
    if (on && !was) beginSession();
    else if (!on && was) endSession();
  },
);

watch(
  () => props.sourceEl,
  () => {
    ensureSourceObserver();
    measureSource();
  },
);

onMounted(() => {
  // Lazy GL init on first active session — avoid idle context cost
  ensureSourceObserver();
  if (props.active) beginSession();
});

onUnmounted(() => {
  disposeGl();
});
</script>
