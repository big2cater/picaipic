<template>
  <div
    class="cp-backdrop pointer-events-none fixed inset-0 z-0 overflow-hidden"
    :class="{ 'cp-backdrop--motion': animate }"
    aria-hidden="true"
  >
    <div class="cp-backdrop__base" />
    <div class="cp-backdrop__glow cp-backdrop__glow--magenta" />
    <div class="cp-backdrop__glow cp-backdrop__glow--cyan" />
    <div class="cp-backdrop__grid" />
    <div class="cp-backdrop__horizon" />
    <div class="cp-backdrop__scanlines" />
    <div v-if="animate" class="cp-backdrop__beam" />
    <div class="cp-backdrop__vignette" />
  </div>
</template>

<script setup lang="ts">
/**
 * Ambient cyberpunk chrome backdrop (always-on under Cyberpunk theme).
 * CSS-only — no WebGL. Idle photo glitch is PhotoGlitchLayer.
 */
defineProps<{
  /** false when prefers-reduced-motion: keep static neon, no beam */
  animate?: boolean;
}>();
</script>

<style scoped>
.cp-backdrop {
  --cp-magenta: #ff2bd6;
  --cp-cyan: #00e5ff;
  --cp-violet: #6b21ff;
  background: #05030a;
}

.cp-backdrop__base {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse 90% 70% at 50% 110%, rgba(107, 33, 255, 0.35) 0%, transparent 55%),
    radial-gradient(ellipse 60% 50% at 15% 20%, rgba(255, 43, 214, 0.18) 0%, transparent 50%),
    radial-gradient(ellipse 55% 45% at 85% 25%, rgba(0, 229, 255, 0.16) 0%, transparent 48%),
    linear-gradient(180deg, #0a0614 0%, #05030a 45%, #020108 100%);
}

.cp-backdrop__glow {
  position: absolute;
  border-radius: 50%;
  filter: blur(48px);
  opacity: 0.55;
}

.cp-backdrop__glow--magenta {
  width: min(55vw, 520px);
  height: min(55vw, 520px);
  left: -8%;
  bottom: 5%;
  background: radial-gradient(circle, rgba(255, 43, 214, 0.55) 0%, transparent 70%);
}

.cp-backdrop__glow--cyan {
  width: min(50vw, 460px);
  height: min(50vw, 460px);
  right: -6%;
  top: 8%;
  background: radial-gradient(circle, rgba(0, 229, 255, 0.45) 0%, transparent 70%);
}

.cp-backdrop__grid {
  position: absolute;
  inset: -20% 0 0 0;
  background-image:
    linear-gradient(rgba(0, 229, 255, 0.07) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 43, 214, 0.06) 1px, transparent 1px);
  background-size: 48px 48px;
  transform: perspective(520px) rotateX(58deg) translateY(-8%);
  transform-origin: center top;
  mask-image: linear-gradient(180deg, transparent 0%, #000 28%, #000 72%, transparent 100%);
  -webkit-mask-image: linear-gradient(180deg, transparent 0%, #000 28%, #000 72%, transparent 100%);
  opacity: 0.85;
}

.cp-backdrop__horizon {
  position: absolute;
  left: -10%;
  right: -10%;
  top: 42%;
  height: 2px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 43, 214, 0.15) 15%,
    rgba(0, 229, 255, 0.85) 50%,
    rgba(255, 43, 214, 0.2) 85%,
    transparent 100%
  );
  box-shadow:
    0 0 18px rgba(0, 229, 255, 0.55),
    0 0 48px rgba(255, 43, 214, 0.25);
  opacity: 0.9;
}

.cp-backdrop__scanlines {
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(
    0deg,
    transparent 0px,
    transparent 2px,
    rgba(0, 0, 0, 0.18) 2px,
    rgba(0, 0, 0, 0.18) 3px
  );
  opacity: 0.35;
  mix-blend-mode: multiply;
}

.cp-backdrop__beam {
  position: absolute;
  left: 0;
  right: 0;
  height: 28%;
  background: linear-gradient(
    180deg,
    transparent 0%,
    rgba(0, 229, 255, 0.04) 40%,
    rgba(255, 43, 214, 0.06) 50%,
    transparent 100%
  );
  animation: cp-beam 9s linear infinite;
  opacity: 0.7;
}

.cp-backdrop__vignette {
  position: absolute;
  inset: 0;
  background: radial-gradient(ellipse 75% 70% at 50% 45%, transparent 40%, rgba(2, 1, 8, 0.75) 100%);
  pointer-events: none;
}

.cp-backdrop--motion .cp-backdrop__glow--magenta {
  animation: cp-pulse-m 7s ease-in-out infinite;
}

.cp-backdrop--motion .cp-backdrop__glow--cyan {
  animation: cp-pulse-c 8.5s ease-in-out infinite;
}

@keyframes cp-beam {
  0% { transform: translateY(-40%); }
  100% { transform: translateY(280%); }
}

@keyframes cp-pulse-m {
  0%, 100% { opacity: 0.4; transform: scale(1); }
  50% { opacity: 0.7; transform: scale(1.06); }
}

@keyframes cp-pulse-c {
  0%, 100% { opacity: 0.35; transform: scale(1.02); }
  50% { opacity: 0.65; transform: scale(1); }
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
