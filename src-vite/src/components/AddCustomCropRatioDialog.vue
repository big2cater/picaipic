<template>
  <ModalDialog :title="$t('msgbox.image_editor.add_custom_ratio_title')" :width="400" @cancel="emit('cancel')">
    <div class="flex flex-col gap-3 text-sm select-none">
      <label class="form-control w-full">
        <span class="label-text text-xs opacity-70 mb-1">{{ $t('msgbox.image_editor.add_custom_ratio_name') }}</span>
        <input
          v-model="name"
          type="text"
          maxlength="40"
          class="input input-bordered input-sm w-full"
          :placeholder="$t('msgbox.image_editor.add_custom_ratio_name_placeholder')"
        />
      </label>

      <label class="form-control w-full">
        <span class="label-text text-xs opacity-70 mb-1">{{ $t('msgbox.image_editor.add_custom_ratio_value') }}</span>
        <input
          ref="ratioInputRef"
          v-model="ratioText"
          type="text"
          maxlength="32"
          class="input input-bordered input-sm w-full"
          :placeholder="$t('msgbox.image_editor.add_custom_ratio_placeholder')"
          @keydown.enter.prevent="submit"
        />
      </label>

      <p class="h-4 text-error text-xs">{{ errorMessage }}</p>

      <div class="flex justify-end gap-2">
        <button class="t-button-default" @click="emit('cancel')">{{ $t('msgbox.cancel') }}</button>
        <button class="t-button-primary" @click="submit">{{ $t('msgbox.ok') }}</button>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import {
  createCustomRatioId,
  formatRatioLabel,
  parseRatioParts,
  type CustomCropRatio,
} from '@/common/photoSizePresets';

const props = defineProps<{
  existing: CustomCropRatio[];
}>();

const emit = defineEmits<{
  cancel: [];
  ok: [CustomCropRatio];
}>();

const { locale, messages } = useI18n();
const localeMsg = () => (messages.value[locale.value] as any).msgbox.image_editor;

const name = ref('');
const ratioText = ref('');
const errorMessage = ref('');
const ratioInputRef = ref<HTMLInputElement | null>(null);

onMounted(() => {
  setTimeout(() => ratioInputRef.value?.focus(), 50);
});

function ratiosEqual(aW: number, aH: number, bW: number, bH: number) {
  return Math.abs(aW / aH - bW / bH) < 1e-6;
}

function submit() {
  const parsed = parseRatioParts(ratioText.value);
  if (!parsed) {
    errorMessage.value = localeMsg().add_custom_ratio_invalid;
    return;
  }

  const duplicate = props.existing.some((item) =>
    ratiosEqual(item.ratioW, item.ratioH, parsed.ratioW, parsed.ratioH),
  );
  if (duplicate) {
    errorMessage.value = localeMsg().add_custom_ratio_duplicate;
    return;
  }

  const label = formatRatioLabel(parsed.ratioW, parsed.ratioH);
  emit('ok', {
    id: createCustomRatioId(),
    name: name.value.trim() || label,
    ratioW: parsed.ratioW,
    ratioH: parsed.ratioH,
  });
}
</script>
