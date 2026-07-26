import { onUnmounted, type Ref, unref, watch, type WatchStopHandle } from 'vue';

type Target = EventTarget | null | undefined | Ref<EventTarget | null | undefined>;

/**
 * Add an event listener and remove it on unmount (and when the target ref changes).
 * Prefer this over manual add/remove pairs that are easy to leak on route/page churn.
 */
export function useEventListener(
  target: Target,
  type: string,
  handler: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions,
): () => void {
  let current: EventTarget | null = null;

  const cleanup = () => {
    if (current) {
      current.removeEventListener(type, handler, options as EventListenerOptions | boolean | undefined);
      current = null;
    }
  };

  const attach = (el: EventTarget | null | undefined) => {
    cleanup();
    if (!el) return;
    el.addEventListener(type, handler, options);
    current = el;
  };

  let stopWatch: WatchStopHandle | null = null;
  // Ref targets: re-bind when the element mounts/unmounts
  if (typeof target === 'object' && target !== null && 'value' in (target as object)) {
    stopWatch = watch(
      () => unref(target as Ref<EventTarget | null | undefined>),
      (el) => attach(el ?? null),
      { immediate: true, flush: 'post' },
    );
  } else {
    attach(target as EventTarget | null | undefined);
  }

  const stop = () => {
    stopWatch?.();
    stopWatch = null;
    cleanup();
  };

  onUnmounted(stop);
  return stop;
}
