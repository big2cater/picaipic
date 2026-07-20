<template>
  <ModalDialog :title="$t('print_layout.add_paper_title')" :width="420" @cancel="emit('cancel')">
    <div class="flex flex-col gap-3 text-sm select-none">
      <label class="form-control w-full">
        <span class="label-text text-xs opacity-70 mb-1">{{ $t('print_layout.paper_name') }}</span>
        <input
          ref="nameInputRef"
          v-model="name"
          type="text"
          maxlength="40"
          class="input input-bordered input-sm w-full"
          :placeholder="$t('print_layout.paper_custom_default')"
          @keydown.enter.prevent="submit"
        />
      </label>

      <div class="flex gap-1">
        <button
          type="button"
          class="btn btn-sm flex-1"
          :class="unit === 'inch' ? 'btn-primary' : ''"
          @click="switchUnit('inch')"
        >
          {{ $t('print_layout.unit_inch') }}
        </button>
        <button
          type="button"
          class="btn btn-sm flex-1"
          :class="unit === 'cm' ? 'btn-primary' : ''"
          @click="switchUnit('cm')"
        >
          {{ $t('print_layout.unit_cm') }}
        </button>
      </div>

      <div class="grid grid-cols-2 gap-2">
        <label class="form-control w-full">
          <span class="label-text text-xs opacity-70 mb-1">
            {{ unit === 'inch' ? $t('print_layout.paper_inch_w') : $t('print_layout.paper_cm_w') }}
          </span>
          <input
            v-model.number="width"
            type="number"
            min="0.1"
            step="0.01"
            class="input input-bordered input-sm w-full"
            @keydown.enter.prevent="submit"
          />
        </label>
        <label class="form-control w-full">
          <span class="label-text text-xs opacity-70 mb-1">
            {{ unit === 'inch' ? $t('print_layout.paper_inch_h') : $t('print_layout.paper_cm_h') }}
          </span>
          <input
            v-model.number="height"
            type="number"
            min="0.1"
            step="0.01"
            class="input input-bordered input-sm w-full"
            @keydown.enter.prevent="submit"
          />
        </label>
      </div>

      <p class="text-[11px] opacity-50">
        {{ $t('print_layout.paper_preview_cm', {
          w: previewCm.w,
          h: previewCm.h,
        }) }}
        ·
        {{ $t('print_layout.paper_preview_inch', {
          w: previewInch.w,
          h: previewInch.h,
        }) }}
      </p>

      <p class="h-4 text-error text-xs">{{ errorMessage }}</p>

      <div class="flex justify-end gap-2">
        <button class="t-button-default" @click="emit('cancel')">{{ $t('msgbox.cancel') }}</button>
        <button class="t-button-primary" @click="submit">{{ $t('msgbox.ok') }}</button>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import {
  createCustomId,
  inchToCm,
  cmToInch,
  type PaperSizeSpec,
} from '@/common/printLayout';

const props = defineProps<{
  existing: PaperSizeSpec[];
}>();

const emit = defineEmits<{
  cancel: [];
  ok: [PaperSizeSpec];
}>();

const { t } = useI18n();
const name = ref(t('print_layout.paper_custom_default'));
const unit = ref<'inch' | 'cm'>('inch');
const width = ref(6);
const height = ref(4);
const errorMessage = ref('');
const nameInputRef = ref<HTMLInputElement | null>(null);

onMounted(() => {
  setTimeout(() => nameInputRef.value?.focus(), 50);
});

const previewInch = computed(() => {
  const w = Number(width.value) || 0;
  const h = Number(height.value) || 0;
  if (unit.value === 'inch') {
    return { w: w.toFixed(2), h: h.toFixed(2) };
  }
  return { w: cmToInch(w).toFixed(2), h: cmToInch(h).toFixed(2) };
});

const previewCm = computed(() => {
  const w = Number(width.value) || 0;
  const h = Number(height.value) || 0;
  if (unit.value === 'cm') {
    return { w: w.toFixed(2), h: h.toFixed(2) };
  }
  return { w: inchToCm(w).toFixed(2), h: inchToCm(h).toFixed(2) };
});

function switchUnit(next: 'inch' | 'cm') {
  if (next === unit.value) return;
  const w = Number(width.value) || 0;
  const h = Number(height.value) || 0;
  if (next === 'cm' && unit.value === 'inch') {
    width.value = Number(inchToCm(w).toFixed(2));
    height.value = Number(inchToCm(h).toFixed(2));
  } else if (next === 'inch' && unit.value === 'cm') {
    width.value = Number(cmToInch(w).toFixed(3));
    height.value = Number(cmToInch(h).toFixed(3));
  }
  unit.value = next;
}

function submit() {
  errorMessage.value = '';
  let inchW = Number(width.value);
  let inchH = Number(height.value);
  if (!(inchW > 0) || !(inchH > 0)) {
    errorMessage.value = t('print_layout.invalid_size');
    return;
  }
  if (unit.value === 'cm') {
    inchW = cmToInch(inchW);
    inchH = cmToInch(inchH);
  }
  if (inchW < 0.2 || inchH < 0.2 || inchW > 40 || inchH > 40) {
    errorMessage.value = t('print_layout.invalid_size_range');
    return;
  }

  const label = String(name.value || '').trim() || t('print_layout.paper_custom_default');
  const duplicate = props.existing.some(
    (p) =>
      p.kind === 'custom'
      && Math.abs(p.inchW - inchW) < 1e-3
      && Math.abs(p.inchH - inchH) < 1e-3
      && (p.name || '') === label,
  );
  if (duplicate) {
    errorMessage.value = t('print_layout.paper_duplicate');
    return;
  }

  emit('ok', {
    id: createCustomId('paper'),
    kind: 'custom',
    name: label,
    inchW: Number(inchW.toFixed(4)),
    inchH: Number(inchH.toFixed(4)),
  });
}
</script>
