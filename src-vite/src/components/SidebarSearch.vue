<template>
  <div class="px-1 pb-1">
    <div
      class="flex items-center gap-1 px-2 py-1 rounded-box bg-base-100/50 border border-base-content/10 focus-within:border-primary/50 transition-colors"
    >
      <IconSearch class="w-3.5 h-3.5 shrink-0 opacity-60" />
      <input
        ref="inputRef"
        :value="modelValue"
        type="text"
        class="grow min-w-0 bg-transparent text-sm outline-none placeholder:text-base-content/40"
        :placeholder="placeholder"
        @input="onInput"
        @keydown.esc.prevent="clear"
      />
      <button
        v-if="modelValue"
        type="button"
        class="shrink-0 opacity-60 hover:opacity-100"
        :title="$t('msgbox.cancel')"
        @click="clear"
      >
        <IconClose class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { IconClose, IconSearch } from '@/common/icons';

const props = defineProps<{
  modelValue: string;
  placeholder?: string;
  autofocus?: boolean;
}>();

const emit = defineEmits<{ 'update:modelValue': [string] }>();

const inputRef = ref<HTMLInputElement | null>(null);

function onInput(event: Event) {
  emit('update:modelValue', (event.target as HTMLInputElement).value);
}

function clear() {
  emit('update:modelValue', '');
  inputRef.value?.focus();
}

watch(
  () => props.autofocus,
  async (value) => {
    if (!value) return;
    await nextTick();
    inputRef.value?.focus();
  },
  { immediate: true }
);
</script>
