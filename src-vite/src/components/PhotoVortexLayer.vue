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
 * FragCoord-style photo-area UV black-hole warp.
 * Freezes a snapshot of the grid, then slowly spirals it into a hole
 * (cinematic, photo-region only — not app chrome).
 */
import { onMounted, onUnmounted, ref, watch } from 'vue';

const props = defineProps<{
  /** When true: capture once (if needed) and run absorb animation */
  active: boolean;
  /** Element to snapshot (GridView root / photo area) */
  sourceEl: HTMLElement | null;
  /** Optional primary tint for the photon ring */
  primaryRgb?: [number, number, number];
}>();

const emit = defineEmits<{
  /** Fired after a successful capture — parent may hide the live grid */
  captured: [];
  /** Fired when the WebGL snapshot path cannot run; parent may use CSS fallback */
  failed: [];
  /** Fired when layer is fully cleared */
  cleared: [];
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
const ready = ref(false);

let gl: WebGLRenderingContext | null = null;
let program: WebGLProgram | null = null;
let tex: WebGLTexture | null = null;
let buf: WebGLBuffer | null = null;
let raf = 0;
/** Pending one-shot capture rAF from beginSession (not the paint loop). */
let captureRaf = 0;
let startMs = 0;
let hasTexture = false;
let failed = false;
let maxTextureSize = 4096;
let maxViewportWidth = 4096;
let maxViewportHeight = 4096;

// Uniforms
let uRes: WebGLUniformLocation | null = null;
let uTime: WebGLUniformLocation | null = null;
let uProgress: WebGLUniformLocation | null = null;
let uTex: WebGLUniformLocation | null = null;
let uRing: WebGLUniformLocation | null = null;

// Cinematic absorb: ~95% by ~22s (slow movie feel)
const TAU_SEC = 12;
// Hole radius in fit-space grows with progress
const R0 = 0.04;
const R1 = 0.62;
// Twist strength (FragCoord demo uses 0.4 — slightly softer for photos)
const TWIST = 0.32;

const VERT = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`;

// Based on https://fragcoord.xyz/s/uq4op5g3 — UV rotation lens + hole + ring
const FRAG = `
precision mediump float;
varying vec2 v_uv;
uniform vec2 u_res;
uniform float u_time;
uniform float u_progress;
uniform sampler2D u_tex;
uniform vec3 u_ring;

mat2 rot(float a) {
  float s = sin(a), c = cos(a);
  return mat2(c, -s, s, c);
}

void main() {
  // Screen UV [0,1] and aspect-correct fit coords (FragCoord demo)
  vec2 uv = v_uv;
  float m = max(min(u_res.x, u_res.y), 1.0);
  // fit = 2 * (frag - 0.5*res) / min(res)
  vec2 fit = 2.0 * (uv - 0.5) * (u_res / m);

  // Hole radius grows slowly with cinematic progress
  float r = mix(${R0.toFixed(3)}, ${R1.toFixed(3)}, u_progress);
  float r2 = r + 0.18;

  float d0 = length(fit);
  float d = d0 - r;
  // Demo: c = 0.4 / d ; soften singularity
  float c = ${TWIST.toFixed(3)} / max(abs(d), 0.02) * sign(d + 1e-5);

  // Spiral-sample the frozen photo (demo: fit *= Rot(c))
  vec2 warpedFit = fit * rot(c);
  // Inverse of fit mapping → texture UV
  vec2 suv = warpedFit * (m / u_res) * 0.5 + 0.5;

  float inTex = step(0.001, suv.x) * step(suv.x, 0.999)
              * step(0.001, suv.y) * step(suv.y, 0.999);
  vec3 tex_col = texture2D(u_tex, clamp(suv, 0.0, 1.0)).rgb * inTex;

  float d_ball = d0 - r2;
  float out_ball = step(0.0, d_ball);
  float in_ball = 1.0 - out_ball;

  // Kill photo inside the hole
  tex_col *= out_ball;

  // Soft interior glow + photon ring
  float ball_light = 0.08 / max(r2 - d0, 0.001);
  tex_col += in_ball * ball_light * u_ring * 0.55;

  float l = 0.09 / max(d_ball, 0.0008) * out_ball;
  float ringPeak = exp(-pow(d_ball * 14.0, 2.0));
  tex_col += (l * 0.65 + ringPeak * 1.1) * u_ring;

  // Vignette into cosmos
  float vig = smoothstep(1.4, 0.5, d0);
  tex_col *= mix(0.7, 1.0, vig);

  // Late-stage dim of remaining fringe
  tex_col *= mix(1.0, 0.2, u_progress * u_progress);

  gl_FragColor = vec4(tex_col, 1.0);
}
`;

function compile(glCtx: WebGLRenderingContext, type: number, src: string): WebGLShader | null {
  const sh = glCtx.createShader(type);
  if (!sh) return null;
  glCtx.shaderSource(sh, src);
  glCtx.compileShader(sh);
  if (!glCtx.getShaderParameter(sh, glCtx.COMPILE_STATUS)) {
    console.warn('PhotoVortexLayer shader compile failed', glCtx.getShaderInfoLog(sh));
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
  maxTextureSize = Math.max(1, Number(gl.getParameter(gl.MAX_TEXTURE_SIZE)) || 4096);
  const viewportDims = gl.getParameter(gl.MAX_VIEWPORT_DIMS) as Int32Array | number[] | null;
  maxViewportWidth = Math.max(1, Number(viewportDims?.[0]) || maxTextureSize);
  maxViewportHeight = Math.max(1, Number(viewportDims?.[1]) || maxTextureSize);

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
    console.warn('PhotoVortexLayer link failed', gl.getProgramInfoLog(prog));
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
  uProgress = gl.getUniformLocation(program, 'u_progress');
  uTex = gl.getUniformLocation(program, 'u_tex');
  uRing = gl.getUniformLocation(program, 'u_ring');

  return true;
}

// Cached CSS size — measure only on ResizeObserver / beginSession (not every paint)
let cachedCssW = 0;
let cachedCssH = 0;
let bufW = 0;
let bufH = 0;
let sourceRo: ResizeObserver | null = null;
let observedEl: HTMLElement | null = null;

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
  const wantedDpr = Math.min(window.devicePixelRatio || 1, 2);
  const scale = Math.min(
    wantedDpr,
    maxViewportWidth / Math.max(cachedCssW, 1),
    maxViewportHeight / Math.max(cachedCssH, 1),
  );
  const w = Math.max(1, Math.floor(cachedCssW * scale));
  const h = Math.max(1, Math.floor(cachedCssH * scale));
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

/**
 * Snapshot visible thumbnails in the photo area into a 2D canvas.
 * Uses already-decoded <img> elements (no extra network).
 */
function captureSource(el: HTMLElement): HTMLCanvasElement | null {
  const rect = el.getBoundingClientRect();
  if (rect.width < 2 || rect.height < 2) return null;

  const wantedDpr = Math.min(window.devicePixelRatio || 1, 1.5);
  const dpr = Math.min(
    wantedDpr,
    maxTextureSize / Math.max(rect.width, 1),
    maxTextureSize / Math.max(rect.height, 1),
  );
  const out = document.createElement('canvas');
  out.width = Math.max(1, Math.floor(rect.width * dpr));
  out.height = Math.max(1, Math.floor(rect.height * dpr));
  const ctx = out.getContext('2d');
  if (!ctx) return null;

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  // Match dark cosmos under transparent cards
  ctx.fillStyle = '#03040a';
  ctx.fillRect(0, 0, rect.width, rect.height);

  const imgs = el.querySelectorAll('img');
  let drawn = 0;
  imgs.forEach((img) => {
    if (!(img instanceof HTMLImageElement)) return;
    if (!img.complete || img.naturalWidth < 1) return;
    const ir = img.getBoundingClientRect();
    // Skip offscreen
    if (ir.right < rect.left || ir.left > rect.right || ir.bottom < rect.top || ir.top > rect.bottom) {
      return;
    }
    const x = ir.left - rect.left;
    const y = ir.top - rect.top;
    try {
      // Rounded clip roughly matching thumbnail cards
      ctx.save();
      const rr = 8;
      const w = ir.width;
      const h = ir.height;
      ctx.beginPath();
      ctx.moveTo(x + rr, y);
      ctx.arcTo(x + w, y, x + w, y + h, rr);
      ctx.arcTo(x + w, y + h, x, y + h, rr);
      ctx.arcTo(x, y + h, x, y, rr);
      ctx.arcTo(x, y, x + w, y, rr);
      ctx.closePath();
      ctx.clip();
      ctx.drawImage(img, x, y, w, h);
      ctx.restore();
      drawn++;
    } catch (e) {
      // cross-origin / protocol taint — skip
    }
  });

  if (drawn === 0) {
    console.warn('PhotoVortexLayer: no thumbnails drawn into capture');
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
  for (let i = 0; i < 8 && gl.getError() !== gl.NO_ERROR; i++) {
    // Clear bounded stale errors so this upload's result is authoritative.
  }
  try {
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
  } catch (error) {
    console.warn('PhotoVortexLayer texture upload failed', error);
    return false;
  }
  const uploadError = gl.getError();
  if (uploadError !== gl.NO_ERROR) {
    console.warn('PhotoVortexLayer texture upload GL error', uploadError);
    return false;
  }
  hasTexture = true;
  return true;
}

function failSession(reason: string) {
  if (failed) return;
  failed = true;
  ready.value = false;
  hasTexture = false;
  cancelAnimationFrame(raf);
  raf = 0;
  console.warn(`PhotoVortexLayer: ${reason}; using CSS fallback`);
  emit('failed');
}

function progressAt(elapsedSec: number): number {
  // 1 - e^(-t/tau) — slow start, asymptotic approach
  return 1 - Math.exp(-elapsedSec / TAU_SEC);
}

function paint(ts: number) {
  raf = requestAnimationFrame(paint);
  if (!gl || !program || !hasTexture || !props.active) return;

  // No per-frame getBoundingClientRect — ResizeObserver updates size
  if (bufW < 1 || bufH < 1) applyCanvasSize();
  const elapsed = startMs ? (ts - startMs) / 1000 : 0;
  const progress = progressAt(elapsed);
  const rgb = props.primaryRgb || [180, 80, 200];

  gl.useProgram(program);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  if (uRes) gl.uniform2f(uRes, bufW, bufH);
  if (uTime) gl.uniform1f(uTime, ts * 0.001);
  if (uProgress) gl.uniform1f(uProgress, progress);
  if (uRing) gl.uniform3f(uRing, rgb[0] / 255, rgb[1] / 255, rgb[2] / 255);
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
      failSession('WebGL unavailable');
      return;
    }
  }

  ensureSourceObserver();

  // Wait one frame so layout/images are stable, then freeze
  if (captureRaf) {
    cancelAnimationFrame(captureRaf);
    captureRaf = 0;
  }
  captureRaf = requestAnimationFrame(() => {
    captureRaf = 0;
    if (!props.active || !props.sourceEl) return;
    const snap = captureSource(props.sourceEl);
    if (!snap) {
      failSession('thumbnail capture unavailable');
      return;
    }
    measureSource();
    applyCanvasSize();
    if (!uploadTexture(snap)) {
      failSession('texture upload unavailable');
      return;
    }

    startMs = performance.now();
    failed = false;
    ready.value = true;
    emit('captured');

    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(paint);
  });
}

function endSession() {
  if (captureRaf) {
    cancelAnimationFrame(captureRaf);
    captureRaf = 0;
  }
  cancelAnimationFrame(raf);
  raf = 0;
  ready.value = false;
  failed = false;
  hasTexture = false;
  startMs = 0;
  if (gl && tex) {
    gl.deleteTexture(tex);
    tex = null;
  }
  emit('cleared');
}

watch(
  () => props.active,
  (on, was) => {
    if (on && !was) beginSession();
    else if (!on && was) endSession();
  },
);

onMounted(() => {
  // Lazy GL: only init on first active session (saves a context when unmounted by theme gate)
  ensureSourceObserver();
  canvasRef.value?.addEventListener('webglcontextlost', onContextLost);
  if (props.active) beginSession();
});

function onContextLost(event: Event) {
  event.preventDefault();
  gl = null;
  program = null;
  tex = null;
  buf = null;
  failSession('WebGL context lost');
}

onUnmounted(() => {
  canvasRef.value?.removeEventListener('webglcontextlost', onContextLost);
  endSession();
  sourceRo?.disconnect();
  sourceRo = null;
  observedEl = null;
  if (gl && program) gl.deleteProgram(program);
  if (gl && buf) gl.deleteBuffer(buf);
  gl = null;
  program = null;
  buf = null;
});
</script>
