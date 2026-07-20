<template>
  <ModalDialog :title="$t('msgbox.image_editor.photo_size_manage_title')" :width="560" @cancel="emit('cancel')">
    <div class="flex flex-col gap-3 text-sm select-none min-h-0">
      <div class="text-xs text-base-content/40">
        {{ $t('msgbox.image_editor.photo_size_manage_hint') }}
      </div>

      <div class="overflow-auto max-h-[50vh] rounded-box border border-base-content/10">
        <table class="table table-xs w-full">
          <thead class="sticky top-0 bg-base-200 z-10">
            <tr class="text-[10px] uppercase tracking-wide text-base-content/40">
              <th>{{ $t('msgbox.image_editor.photo_size_col_name') }}</th>
              <th>{{ $t('msgbox.image_editor.photo_size_col_inch') }}</th>
              <th>{{ $t('msgbox.image_editor.photo_size_col_cm') }}</th>
              <th>{{ $t('msgbox.image_editor.photo_size_col_px') }}</th>
              <th>DPI</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="preset in photoPresets" :key="preset.id">
              <td class="font-medium">{{ $t(`msgbox.image_editor.photo_sizes.${preset.nameKey}`) }}</td>
              <td class="opacity-70">{{ formatPair(preset.inchW, preset.inchH) }}</td>
              <td class="opacity-70">{{ formatPair(preset.cmW, preset.cmH) }}</td>
              <td class="opacity-70">{{ preset.pxW }} × {{ preset.pxH }}</td>
              <td class="opacity-70">{{ preset.dpi }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="space-y-2">
        <div class="text-[11px] font-bold uppercase tracking-[0.18em] text-base-content/30">
          {{ $t('msgbox.image_editor.custom_ratios_title') }}
        </div>

        <div v-if="localCustomRatios.length === 0" class="text-xs text-base-content/40">
          {{ $t('msgbox.image_editor.custom_ratios_empty') }}
        </div>

        <div
          v-for="item in localCustomRatios"
          :key="item.id"
          class="flex items-center gap-2 rounded-box border border-base-content/10 px-2 py-1.5"
        >
          <div class="flex-1 min-w-0">
            <div class="truncate font-medium">{{ item.name }}</div>
            <div class="text-xs opacity-50">{{ formatRatioLabel(item.ratioW, item.ratioH) }}</div>
          </div>
          <TButton
            buttonSize="small"
            :icon="IconTrash"
            :tooltip="$t('msgbox.image_editor.delete_custom_ratio')"
            @click="removeCustom(item.id)"
          />
        </div>
      </div>

      <div class="flex justify-end gap-2 pt-1">
        <button class="t-button-default" @click="emit('cancel')">
          {{ $t('msgbox.image_editor.close') }}
        </button>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import ModalDialog from '@/components/ModalDialog.vue';
import TButton from '@/components/TButton.vue';
import { IconTrash } from '@/common/icons';
import {
  BUILTIN_PHOTO_SIZE_PRESETS,
  formatRatioLabel,
  type CustomCropRatio,
} from '@/common/photoSizePresets';

const props = defineProps<{
  customRatios: CustomCropRatio[];
}>();

const emit = defineEmits<{
  cancel: [];
  'update:customRatios': [CustomCropRatio[]];
}>();

const photoPresets = BUILTIN_PHOTO_SIZE_PRESETS;
const localCustomRatios = ref<CustomCropRatio[]>([...props.customRatios]);

watch(
  () => props.customRatios,
  (value) => {
    localCustomRatios.value = [...value];
  },
  { deep: true },
);

function formatPair(a: number, b: number) {
  return `${a} × ${b}`;
}

function removeCustom(id: string) {
  localCustomRatios.value = localCustomRatios.value.filter((item) => item.id !== id);
  emit('update:customRatios', [...localCustomRatios.value]);
}
</script>
