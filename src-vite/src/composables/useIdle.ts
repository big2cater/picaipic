import { ref, onMounted, onUnmounted, type Ref } from 'vue';
import { useEventListener } from '@/composables/useEventListener';

const ACTIVITY_EVENTS = ['mousemove', 'keydown', 'scroll', 'wheel', 'touchstart'] as const;

export function useIdle(ms = 6000): { idle: Ref<boolean> } {
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

  for (const e of ACTIVITY_EVENTS) {
    useEventListener(window, e, reset, { passive: true });
  }

  onMounted(() => {
    reset();
  });

  onUnmounted(() => {
    clearTimer();
  });

  return { idle };
}
