import { ref, onMounted, onUnmounted, type Ref } from 'vue';

const ACTIVITY_EVENTS = ['mousemove', 'keydown', 'scroll', 'wheel', 'touchstart'] as const;

export function useIdle(ms = 15000): { idle: Ref<boolean> } {
  const idle = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  const clearTimer = () => {
    if (timer != null) {
      clearTimeout(timer);
      timer = null;
    }
  };

  const reset = () => {
    idle.value = false;
    clearTimer();
    timer = setTimeout(() => {
      idle.value = true;
    }, ms);
  };

  onMounted(() => {
    for (const e of ACTIVITY_EVENTS) {
      window.addEventListener(e, reset, { passive: true });
    }
    reset();
  });

  onUnmounted(() => {
    clearTimer();
    for (const e of ACTIVITY_EVENTS) {
      window.removeEventListener(e, reset);
    }
  });

  return { idle };
}
