<template>
  <div>
    <div 
      ref="triggerRef"
      class="relative inline-block"
      @mouseenter="showTooltip"
      @mouseleave="hideTooltip"
    >
      <button
        class="btn btn-ghost btn-square rounded-box border-0 focus:outline-none shadow-none flex flex-col"
        :class="[
          buttonClasses,
          {
            'btn-xs hover:bg-base-100/30': buttonSize === 'small',
            'btn-sm hover:bg-base-100/30': buttonSize === 'medium',
            'btn-lg hover:bg-base-100/30': buttonSize === 'large',
            'btn-disabled': disabled,
          }
        ]"
        :disabled="disabled"
        @click="$emit('click', $event)"
      >
        <component v-if="icon"
          :is="icon"
          :class="[
            iconClasses,
            {
              'w-4 h-4': buttonSize === 'small',
              'w-5 h-5': buttonSize === 'medium',
              'w-6 h-6': buttonSize === 'large',
              'text-primary': selected,
            }
          ]"
          :style="iconStyle"
        />
        <span 
          v-if="config.settings.showButtonText && text.length > 0"
          class='text-xs/2 whitespace-nowrap'
          :class="[
            textClasses,
            {
              'text-primary': selected,
            }
          ]"
        >
          {{ text }}
        </span>
      </button>
    </div>
    <Teleport to="body">
      <transition name="fade">
        <div
          v-if="isHovered && tooltipText && config.settings.showToolTip"
          ref="tooltipRef"
          class="fixed z-1000 px-2 py-1 text-xs leading-tight max-w-xs whitespace-normal text-center rounded-box bg-neutral text-neutral-content shadow-lg pointer-events-none"
          :style="tooltipStyle"
        >
          {{ tooltipText }}
        </div>
      </transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onBeforeUnmount, nextTick, watch, type CSSProperties } from 'vue';
import { config } from '@/common/config';
import type { Component } from 'vue';

const props = defineProps({
  buttonSize: {
    type: String,
    default: 'medium'         // 'small', 'medium'(default), 'large'
  },
  buttonClasses: {
    type: String,
    default: ''
  },
  icon: {
    type: Object as () => Component,
    required: false,
  },
  iconClasses: {
    type: [String, Array],
    default: ''
  },
  iconStyle: {
    type: Object,
    default: () => ({})
  },
  text: {
    type: String,
    default: ''
  },
  textClasses: {
    type: String,
    default: ''
  },
  disabled: {
    type: Boolean,
    default: false
  },
  selected: {
    type: Boolean,
    default: false
  },
  tooltip: {
    type: String,
    default: ''
  },
  shortcut: {
    type: String,
    default: ''
  },
  tooltipClasses: {
    type: String,
    default: 'tooltip-bottom'
  }
});

const emit = defineEmits(['click']);
const tooltipText = computed(() => {
  if (!props.shortcut) return props.tooltip;
  return props.tooltip ? `${props.tooltip} (${props.shortcut})` : props.shortcut;
});

const isHovered = ref(false);
const triggerRef = ref<HTMLElement | null>(null);
const tooltipRef = ref<HTMLElement | null>(null);
const tooltipStyle = ref<CSSProperties>({
  left: '0px',
  top: '0px',
});
let tooltipTimer: ReturnType<typeof setTimeout> | null = null;
let removeTooltipPositionListeners: (() => void) | null = null;

const clearTooltipTimer = () => {
  if (tooltipTimer) {
    clearTimeout(tooltipTimer);
    tooltipTimer = null;
  }
};

const removePositionListeners = () => {
  if (removeTooltipPositionListeners) {
    removeTooltipPositionListeners();
    removeTooltipPositionListeners = null;
  }
};

const updateTooltipPosition = async () => {
  await nextTick();

  if (!triggerRef.value || !tooltipRef.value) return;

  const triggerRect = triggerRef.value.getBoundingClientRect();
  const tooltipRect = tooltipRef.value.getBoundingClientRect();
  const gap = 6;
  const padding = 8;

  let left = triggerRect.left + (triggerRect.width - tooltipRect.width) / 2;
  left = Math.max(padding, Math.min(left, window.innerWidth - tooltipRect.width - padding));

  let top = triggerRect.bottom + gap;
  if (top + tooltipRect.height > window.innerHeight - padding) {
    top = triggerRect.top - tooltipRect.height - gap;
  }

  tooltipStyle.value = {
    left: `${Math.round(left)}px`,
    top: `${Math.round(top)}px`,
  };
};

const attachPositionListeners = () => {
  if (removeTooltipPositionListeners) return;

  const handler = () => {
    void updateTooltipPosition();
  };

  window.addEventListener('resize', handler);
  window.addEventListener('scroll', handler, true);
  removeTooltipPositionListeners = () => {
    window.removeEventListener('resize', handler);
    window.removeEventListener('scroll', handler, true);
  };
};

const showTooltip = () => {
  if (!tooltipText.value || !config.settings.showToolTip) return;

  clearTooltipTimer();
  isHovered.value = true;
  attachPositionListeners();
  void updateTooltipPosition();

  tooltipTimer = setTimeout(() => {
    isHovered.value = false;
    removePositionListeners();
    tooltipTimer = null;
  }, 3000);
};

const hideTooltip = () => {
  clearTooltipTimer();
  isHovered.value = false;
  removePositionListeners();
};

watch(isHovered, (visible) => {
  if (!visible) return;
  void updateTooltipPosition();
});

onBeforeUnmount(() => {
  clearTooltipTimer();
  removePositionListeners();
});
</script>
