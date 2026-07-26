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
 * WebGL1 only — see design §4.2 port checklist.
 */
import { onMounted, onUnmounted, ref, watch } from 'vue';

const props = defineProps<{
  /** When true: capture once (if needed) and run continuous glitch */
  active: boolean;
  /** Element to snapshot (GridView root / photo area) */
  sourceEl: HTMLElement | null;
  /**
   * Effect strength (typically 0.5 / 1 / 1.5). Parent should skip mounting
   * active when intensity is 0 — bind raw Number (do not `|| 1`).
   */
  intensity?: number;
}>();

const emit = defineEmits<{
  /** Fired after a successful capture — parent may hide the live grid */
  captured: [];
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
let startMs = 0;
let hasTexture = false;

// Uniforms
let uRes: WebGLUniformLocation | null = null;
let uTime: WebGLUniformLocation | null = null;
let uTex: WebGLUniformLocation | null = null;
let uIntensity: WebGLUniformLocation | null = null;

const VERT = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`;

// WebGL1 port of FragCoord-style cyberpunk glitch (no #version 300 es)
const FRAG = `
precision mediump float;
varying vec2 v_uv;
uniform vec2 u_res;
uniform float u_time;
uniform sampler2D u_tex;
uniform float u_intensity;

float rand(vec2 co) {
  return fract(sin(dot(co.xy, vec2(12.9898, 78.233))) * 43758.5453);
}

mat2 Rot(float a) {
  float s = sin(a), c = cos(a);
  return mat2(c, -s, s, c);
}

void main() {
  vec2 uv = v_uv;
  float t = u_time;
  float intensity = max(u_intensity, 0.0);

  // Intermittent full-frame glitch flag from time
  float glitch = step(0.92, rand(vec2(floor(t * 8.0), 0.5)));
  float burst = step(0.96, rand(vec2(floor(t * 1.7), 7.3)));
  glitch = max(glitch, burst);

  // Line noise — horizontal displace (scaled by intensity)
  float lineY = floor(uv.y * (40.0 + 40.0 * intensity));
  float lineNoise = rand(vec2(lineY, floor(t * 12.0))) * 2.0 - 1.0;
  uv.x += lineNoise * 0.04 * intensity * glitch;

  // Block row displace
  float blockY = floor(uv.y * (8.0 + 8.0 * intensity));
  float blockNoise = rand(vec2(blockY, floor(t * 3.0))) * 2.0 - 1.0;
  float blockOn = step(0.7, rand(vec2(blockY, floor(t * 5.0))));
  uv.x += blockNoise * 0.12 * intensity * glitch * blockOn;

  // Small rotation under glitch
  vec2 center = vec2(0.5);
  vec2 fromC = uv - center;
  float ang = (rand(vec2(floor(t * 4.0), 1.2)) * 2.0 - 1.0) * 0.04 * intensity * glitch;
  fromC *= Rot(ang);
  uv = fromC + center;

  // RGB chromatic aberration (scaled by intensity)
  float ca = 0.008 * intensity * (1.0 + glitch * 2.5);
  float r = texture2D(u_tex, clamp(uv + vec2(ca, 0.0), 0.0, 1.0)).r;
  float g = texture2D(u_tex, clamp(uv, 0.0, 1.0)).g;
  float b = texture2D(u_tex, clamp(uv - vec2(ca, 0.0), 0.0, 1.0)).b;
  vec3 col = vec3(r, g, b);

  // Scanlines
  float scan = 0.92 + 0.08 * sin(uv.y * u_res.y * 1.5 + t * 8.0);
  col *= scan;

  // Cyan lift
  col.g = min(1.0, col.g + 0.03 * intensity);
  col.b = min(1.0, col.b + 0.05 * intensity);

  // Grain (scaled by intensity)
  float grain = (rand(uv * u_res + t) - 0.5) * 0.12 * intensity;
  col += grain;

  // Rare invert — slightly easier at higher intensity, still rare
  float invertThresh = 0.995 - intensity * 0.01;
  float invert = step(invertThresh, rand(vec2(floor(t * 2.0), 9.1))) * glitch;
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
  // WebGL1 only — must match PhotoVortexLayer (NOT webgl2)
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

function resizeToSource() {
  const canvas = canvasRef.value;
  const src = props.sourceEl;
  if (!canvas || !src || !gl) return;
  const rect = src.getBoundingClientRect();
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.max(1, Math.floor(rect.width * dpr));
  const h = Math.max(1, Math.floor(rect.height * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  canvas.style.width = `${rect.width}px`;
  canvas.style.height = `${rect.height}px`;
  gl.viewport(0, 0, w, h);
}

/**
 * Snapshot visible thumbnails in the photo area into a 2D canvas.
 * Uses already-decoded <img> elements (no extra network).
 * Diverges from vortex: drawn===0 returns null (abort — no emit captured).
 */
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
  // Dark cyber fill under transparent cards
  ctx.fillStyle = '#05060a';
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
    // Spec §4.1 / §8: abort empty capture — do NOT emit captured / hide grid
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

  resizeToSource();

  // Bind raw intensity Number — no `|| 1` (parent skips 0; 0||1 would mis-map)
  const intensity = Number(props.intensity);
  const intensityVal = Number.isFinite(intensity) ? intensity : 1;

  gl.useProgram(program);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  if (uRes) gl.uniform2f(uRes, gl.drawingBufferWidth, gl.drawingBufferHeight);
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

  // Wait one frame so layout/images are stable, then freeze
  requestAnimationFrame(() => {
    if (!props.active || !props.sourceEl) return;
    const snap = captureSource(props.sourceEl);
    if (!snap) return; // abort: no upload, no emit('captured')
    resizeToSource();
    if (!uploadTexture(snap)) return;

    startMs = performance.now();
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
  const canvas = canvasRef.value;
  if (canvas) initGl(canvas);
  if (props.active) beginSession();
});

onUnmounted(() => {
  endSession();
  if (gl && program) gl.deleteProgram(program);
  if (gl && buf) gl.deleteBuffer(buf);
  gl = null;
  program = null;
  buf = null;
});
</script>
