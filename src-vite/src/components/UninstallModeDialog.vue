<template>
  <ModalDialog :title="$t('msgbox.uninstall.title')" :width="460" @cancel="resolve('code_only')">
    <p class="text-sm whitespace-pre-line wrap-break-word">
      {{ $t('msgbox.uninstall.message', { plugin, path }) }}
    </p>

    <div class="mt-4 space-y-3">
      <!-- Code only -->
      <button
        class="w-full text-left px-3 py-2 rounded-box border border-base-content/10 hover:bg-base-100 cursor-pointer transition-colors"
        @click="resolve('code_only')"
      >
        <div class="text-sm font-medium">{{ $t('msgbox.uninstall.code_only') }}</div>
        <div class="text-xs text-base-content/50 mt-0.5">{{ $t('msgbox.uninstall.code_only_hint') }}</div>
      </button>

      <!-- Code + data & runtimes -->
      <button
        class="w-full text-left px-3 py-2 rounded-box border border-error/20 bg-error/5 hover:bg-error/10 cursor-pointer transition-colors"
        @click="resolve('code_and_data')"
      >
        <div class="text-sm font-medium text-error">{{ $t('msgbox.uninstall.code_and_data') }}</div>
        <div class="text-xs text-base-content/50 mt-0.5">{{ $t('msgbox.uninstall.code_and_data_hint') }}</div>
      </button>
    </div>

    <div class="mt-5 flex justify-end gap-2">
      <button
        class="px-3 py-1 rounded-box hover:bg-base-100 cursor-pointer"
        @click="resolve('cancel')"
      >
        {{ $t('msgbox.cancel') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { useUIStore } from '@/stores/uiStore';
import ModalDialog from '@/components/ModalDialog.vue';

type UninstallMode = 'code_only' | 'code_and_data' | 'cancel';

defineProps({
  plugin: {
    type: String,
    required: true,
  },
  path: {
    type: String,
    required: true,
  },
});

const emit = defineEmits<{
  resolve: [result: { mode: UninstallMode }];
}>();

const uiStore = useUIStore();

onMounted(() => uiStore.pushInputHandler('UninstallModeDialog'));
onUnmounted(() => uiStore.removeInputHandler('UninstallModeDialog'));

function resolve(mode: UninstallMode) {
  emit('resolve', { mode });
}
</script>
