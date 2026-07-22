<template>
  <ModalDialog :title="$t('photo_frame.title')" :width="920" :height="640" @cancel="onCancel">
    <div class="flex gap-3 text-sm select-none min-h-0 h-full">
      <!-- Left: files -->
      <div class="w-[180px] shrink-0 flex flex-col gap-2 min-h-0">
        <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
          {{ $t('photo_frame.photos') }}
          <span class="font-normal normal-case tracking-normal opacity-70">
            ({{ files.length }})
          </span>
        </div>
        <div class="flex-1 min-h-0 overflow-auto space-y-1.5">
          <button
            v-for="(f, idx) in files"
            :key="f.file_path + idx"
            type="button"
            class="w-full flex items-center gap-2 rounded-box border p-1.5 text-left transition-colors"
            :class="activeIndex === idx
              ? 'border-primary/50 bg-primary/10'
              : 'border-base-content/10 hover:bg-base-100/40'"
            @click="activeIndex = idx"
          >
            <div class="h-10 w-10 shrink-0 rounded overflow-hidden bg-base-content/5 border border-base-content/10">
              <img
                v-if="f.thumbnail"
                :src="f.thumbnail"
                class="h-full w-full object-cover"
                alt=""
              />
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-xs truncate">{{ f.name || fileBase(f.file_path) }}</div>
            </div>
          </button>
          <div v-if="!files.length" class="text-xs text-base-content/40 py-6 text-center">
            {{ $t('photo_frame.no_photos') }}
          </div>
        </div>
      </div>

      <!-- Center: preview -->
      <div class="flex-1 min-w-0 min-h-0 flex flex-col gap-2">
        <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
          {{ $t('photo_frame.preview') }}
        </div>
        <div
          class="flex-1 min-h-0 flex items-center justify-center rounded-box border border-base-content/10 bg-base-300/40 p-3 overflow-auto"
        >
          <div v-if="previewLoading" class="text-xs text-base-content/50">
            {{ $t('photo_frame.preview_loading') }}
          </div>
          <img
            v-else-if="previewUrl"
            :src="previewUrl"
            class="max-h-full max-w-full object-contain shadow-lg rounded-sm"
            alt=""
          />
          <div v-else class="text-xs text-base-content/40 text-center px-6">
            {{ previewError || $t('photo_frame.preview_hint') }}
          </div>
        </div>
        <div class="text-[11px] text-base-content/40 text-center truncate">
          {{ activeFileLabel }}
        </div>
      </div>

      <!-- Right: options -->
      <div class="w-[250px] shrink-0 flex flex-col gap-2 min-h-0 overflow-auto">
        <div class="space-y-1.5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('photo_frame.template') }}
          </div>
          <div class="flex flex-col gap-1.5">
            <button
              v-for="tpl in layoutTemplates"
              :key="tpl.id"
              type="button"
              class="px-3 py-1.5 rounded-box border text-sm text-left cursor-pointer transition-colors"
              :class="options.templateId === tpl.id
                ? 'border-primary bg-primary/15 text-primary'
                : 'border-base-content/15 hover:bg-base-100/40'"
              @click="selectTemplate(tpl.id)"
            >
              {{ $t(tpl.nameKey) }}
            </button>
          </div>
        </div>

        <div class="space-y-1.5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('photo_frame.presets') }}
          </div>
          <div class="flex flex-col gap-1.5">
            <button
              type="button"
              class="t-button-default text-xs w-full"
              :disabled="isRunning"
              @click="savePreset"
            >
              {{ $t('photo_frame.save_preset') }}
            </button>
            <select v-model="selectedPresetId" class="select select-bordered select-xs w-full">
              <option value="">
                {{ customPresets.length ? $t('photo_frame.pick_preset') : $t('photo_frame.no_presets') }}
              </option>
              <option v-for="p in customPresets" :key="p.id" :value="p.id">{{ p.name }}</option>
            </select>
            <div class="flex gap-1.5">
              <button
                type="button"
                class="t-button-default text-xs flex-1"
                :disabled="!selectedPresetId || isRunning"
                @click="loadPreset"
              >
                {{ $t('photo_frame.load_preset') }}
              </button>
              <button
                type="button"
                class="t-button-default text-xs flex-1"
                :disabled="!selectedPresetId || isRunning"
                @click="deletePreset"
              >
                {{ $t('photo_frame.delete_preset') }}
              </button>
            </div>
          </div>
        </div>

        <div class="space-y-1.5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('photo_frame.fields') }}
          </div>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showBrand" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_brand') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showModel" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_model') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showLens" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_lens') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showFocalLength" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_focal') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showAperture" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_aperture') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showShutter" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_shutter') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showISO" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_iso') }}
          </label>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showDateTime" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.field_datetime') }}
          </label>
        </div>

        <div class="space-y-1.5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('photo_frame.style') }}
          </div>
          <label class="flex flex-col gap-1 text-xs">
            <span class="opacity-60">{{ $t('photo_frame.bar_ratio') }}: {{ Math.round(options.barRatio * 100) }}%</span>
            <input
              v-model.number="options.barRatio"
              type="range"
              min="0.05"
              max="0.22"
              step="0.01"
              class="range range-xs range-primary"
            />
          </label>
          <label class="flex flex-col gap-1 text-xs">
            <span class="opacity-60">
              {{ isBlurLayout ? $t('photo_frame.pad_ratio') : $t('photo_frame.margin_ratio') }}:
              {{ Math.round(options.marginRatio * 100) }}%
            </span>
            <input
              v-model.number="options.marginRatio"
              type="range"
              :min="isBlurLayout ? 0.04 : 0"
              :max="isBlurLayout ? 0.20 : 0.12"
              step="0.005"
              class="range range-xs range-primary"
            />
          </label>
          <template v-if="isBlurLayout">
            <label class="flex flex-col gap-1 text-xs">
              <span class="opacity-60">{{ $t('photo_frame.blur_sigma') }}: {{ Math.round(options.blurSigma) }}</span>
              <input
                v-model.number="options.blurSigma"
                type="range"
                min="2"
                max="48"
                step="1"
                class="range range-xs range-primary"
              />
            </label>
            <label class="flex flex-col gap-1 text-xs">
              <span class="opacity-60">{{ $t('photo_frame.shadow_blur') }}: {{ Math.round(options.shadowBlur) }}</span>
              <input
                v-model.number="options.shadowBlur"
                type="range"
                min="2"
                max="40"
                step="1"
                class="range range-xs range-primary"
              />
            </label>
            <label class="flex flex-col gap-1 text-xs">
              <span class="opacity-60">
                {{ $t('photo_frame.shadow_offset') }}: {{ Math.round(options.shadowOffsetRatio * 100) }}%
              </span>
              <input
                v-model.number="options.shadowOffsetRatio"
                type="range"
                min="0"
                max="0.12"
                step="0.005"
                class="range range-xs range-primary"
              />
            </label>
            <label class="flex flex-col gap-1 text-xs">
              <span class="opacity-60">
                {{ $t('photo_frame.shadow_opacity') }}: {{ Math.round(options.shadowOpacity * 100) }}%
              </span>
              <input
                v-model.number="options.shadowOpacity"
                type="range"
                min="0.05"
                max="0.9"
                step="0.05"
                class="range range-xs range-primary"
              />
            </label>
          </template>
          <template v-else>
            <label class="flex items-center justify-between gap-2 text-xs">
              <span class="opacity-60">{{ $t('photo_frame.bg_color') }}</span>
              <input v-model="options.backgroundColor" type="color" class="h-7 w-10 cursor-pointer bg-transparent border-0" />
            </label>
          </template>
          <label class="flex items-center justify-between gap-2 text-xs">
            <span class="opacity-60">{{ $t('photo_frame.text_color') }}</span>
            <input v-model="options.textColor" type="color" class="h-7 w-10 cursor-pointer bg-transparent border-0" />
          </label>
          <label class="flex items-center justify-between gap-2 text-xs">
            <span class="opacity-60">{{ $t('photo_frame.secondary_color') }}</span>
            <input v-model="options.secondaryTextColor" type="color" class="h-7 w-10 cursor-pointer bg-transparent border-0" />
          </label>
        </div>

        <div class="space-y-1.5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('photo_frame.logo') }}
          </div>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="options.showLogo" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.show_logo') }}
          </label>
          <div class="flex gap-1">
            <input
              v-model="options.logoPath"
              type="text"
              class="input input-bordered input-xs flex-1 min-w-0"
              :placeholder="$t('photo_frame.logo_default_hint')"
              readonly
            />
            <button type="button" class="btn btn-xs btn-ghost" :disabled="!options.showLogo" @click="pickLogo">
              …
            </button>
            <button
              type="button"
              class="btn btn-xs btn-ghost"
              :disabled="!options.showLogo || !options.logoPath"
              :title="$t('photo_frame.clear_logo')"
              @click="clearLogo"
            >
              ×
            </button>
          </div>
          <div class="text-[10px] text-base-content/40 leading-snug">
            {{ $t('photo_frame.logo_default_hint') }}
          </div>
          <label class="flex flex-col gap-1 text-xs" :class="{ 'opacity-40': !options.showLogo }">
            <span class="opacity-60">{{ $t('photo_frame.logo_scale') }}: {{ Math.round(options.logoScale * 100) }}%</span>
            <input
              v-model.number="options.logoScale"
              type="range"
              min="0.04"
              max="0.18"
              step="0.01"
              class="range range-xs range-primary"
              :disabled="!options.showLogo"
            />
          </label>
          <select
            v-model="options.logoPosition"
            class="select select-bordered select-xs w-full"
            :disabled="!options.showLogo"
          >
            <option value="bar-center">{{ $t('photo_frame.logo_bar_center') }}</option>
            <option value="top-left">{{ $t('photo_frame.logo_top_left') }}</option>
            <option value="top-right">{{ $t('photo_frame.logo_top_right') }}</option>
          </select>
        </div>

        <div class="space-y-1.5">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('photo_frame.output') }}
          </div>
          <div class="flex gap-1">
            <input
              v-model="outputDir"
              type="text"
              class="input input-bordered input-xs flex-1 min-w-0"
              :placeholder="$t('photo_frame.output_dir')"
              readonly
            />
            <button type="button" class="btn btn-xs btn-ghost" @click="pickOutputDir">
              …
            </button>
          </div>
          <div class="flex gap-2">
            <select v-model="outputFormat" class="select select-bordered select-xs flex-1">
              <option value="jpg">JPEG</option>
              <option value="png">PNG</option>
            </select>
            <label v-if="outputFormat === 'jpg'" class="flex items-center gap-1 text-xs">
              <span class="opacity-50">Q</span>
              <input v-model.number="quality" type="number" min="40" max="100" class="input input-bordered input-xs w-14" />
            </label>
          </div>
          <select v-model="nameMode" class="select select-bordered select-xs w-full">
            <option value="original">{{ $t('photo_frame.name_original') }}</option>
            <option value="suffix">{{ $t('photo_frame.name_suffix') }}</option>
            <option value="prefix">{{ $t('photo_frame.name_prefix') }}</option>
            <option value="sequence">{{ $t('photo_frame.name_sequence') }}</option>
          </select>
          <label class="flex items-center gap-2 text-xs">
            <input v-model="importToLibrary" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('photo_frame.import_to_library') }}
          </label>
        </div>

        <div v-if="isRunning" class="space-y-1">
          <progress
            class="progress progress-primary w-full"
            :value="progress.current"
            :max="Math.max(progress.total, 1)"
          />
          <div class="text-[11px] text-base-content/50">
            {{ progress.current }}/{{ progress.total }} · {{ progress.status }}
          </div>
        </div>
        <p v-if="errorMessage" class="text-xs text-error">{{ errorMessage }}</p>

        <div class="mt-auto flex gap-2 pt-2">
          <button type="button" class="t-button-default flex-1" :disabled="isRunning" @click="onCancel">
            {{ $t('msgbox.cancel') }}
          </button>
          <button
            v-if="!isRunning"
            type="button"
            class="t-button-primary flex-1"
            :disabled="!canExport"
            @click="startExport"
          >
            {{ $t('photo_frame.export') }}
          </button>
          <button
            v-else
            type="button"
            class="t-button-default flex-1"
            @click="cancelRunning"
          >
            {{ $t('photo_frame.stop') }}
          </button>
        </div>
      </div>
    </div>
  </ModalDialog>

  <MessageBox
    v-if="showPresetNameBox"
    :title="$t('photo_frame.preset_name_title')"
    :message="$t('photo_frame.preset_name_prompt')"
    :showInput="true"
    :inputText="presetNameInput"
    :inputPlaceholder="$t('photo_frame.preset_name_prompt')"
    :OkText="$t('msgbox.ok')"
    :cancelText="$t('msgbox.cancel')"
    @ok="onPresetNameOk"
    @cancel="showPresetNameBox = false"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { ask, open as openDialog } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import ModalDialog from '@/components/ModalDialog.vue';
import MessageBox from '@/components/MessageBox.vue';
import {
  cancelPhotoFrameExport,
  exportPhotoFrame,
  photoFramePreview,
} from '@/common/api';
import { config } from '@/common/config';
import {
  PHOTO_FRAME_TEMPLATES,
  applyTemplatePreset,
  clonePhotoFrameOptions,
  createDefaultPhotoFrameOptions,
  createPhotoFramePresetId,
  filterPhotoFrameImageFiles,
  isBlurFrameTemplate,
  normalizePhotoFrameOptions,
  normalizePhotoFramePresets,
  type PhotoFrameFileItem,
  type PhotoFrameOptions,
  type PhotoFramePreset,
  type PhotoFrameTemplateId,
} from '@/common/photoFrameTemplates';
import { useToast } from '@/common/toast';

const props = defineProps<{
  files: PhotoFrameFileItem[];
}>();

const emit = defineEmits<{
  cancel: [];
  done: [result: any];
}>();

const { t } = useI18n();
const toast = useToast();

const layoutTemplates = PHOTO_FRAME_TEMPLATES;
const files = ref<PhotoFrameFileItem[]>(
  filterPhotoFrameImageFiles(props.files || []).map((f) => ({
    ...f,
    file_path: String(f.file_path || ''),
  })),
);
const activeIndex = ref(0);

function ensurePhotoFrameConfig() {
  if (!(config as any).photoFrame || typeof (config as any).photoFrame !== 'object') {
    (config as any).photoFrame = {
      presets: [],
      importToLibrary: false,
      lastTemplateId: 'classic-white',
      lastPresetId: '',
    };
  }
  const pf = (config as any).photoFrame;
  if (!Array.isArray(pf.presets)) pf.presets = [];
  else pf.presets = normalizePhotoFramePresets(pf.presets);
  if (typeof pf.importToLibrary !== 'boolean') pf.importToLibrary = false;
  if (typeof pf.lastTemplateId !== 'string') pf.lastTemplateId = 'classic-white';
  if (typeof pf.lastPresetId !== 'string') pf.lastPresetId = '';
}
ensurePhotoFrameConfig();

const presetsTick = ref(0);
const selectedPresetId = ref(String((config as any).photoFrame?.lastPresetId || ''));
const showPresetNameBox = ref(false);
const presetNameInput = ref('');

const customPresets = computed<PhotoFramePreset[]>(() => {
  presetsTick.value;
  ensurePhotoFrameConfig();
  return normalizePhotoFramePresets((config as any).photoFrame?.presets);
});

const initialTemplate = (() => {
  const last = String((config as any).photoFrame?.lastTemplateId || 'classic-white');
  return (PHOTO_FRAME_TEMPLATES.some((x) => x.id === last) ? last : 'classic-white') as PhotoFrameTemplateId;
})();
const options = ref<PhotoFrameOptions>(createDefaultPhotoFrameOptions(initialTemplate));
// Restore last applied custom preset if still present.
if (selectedPresetId.value) {
  const found = customPresets.value.find((p) => p.id === selectedPresetId.value);
  if (found) options.value = clonePhotoFrameOptions(found.options);
}

const outputDir = ref('');
const outputFormat = ref<'jpg' | 'png'>('jpg');
const quality = ref(90);
const nameMode = ref<'original' | 'suffix' | 'prefix' | 'sequence'>('suffix');
const importToLibrary = ref(Boolean((config as any).photoFrame?.importToLibrary));

const previewUrl = ref('');
const previewLoading = ref(false);
const previewError = ref('');
let previewTimer: ReturnType<typeof setTimeout> | null = null;
let previewGeneration = 0;
let previewObjectUrl: string | null = null;

const isRunning = ref(false);
const errorMessage = ref('');
const progress = ref({ current: 0, total: 0, status: '', filePath: '', message: '' });
let unlistenProgress: (() => void) | null = null;

const activeFile = computed(() => files.value[activeIndex.value] || null);
const activeFileLabel = computed(() => {
  const f = activeFile.value;
  if (!f) return '';
  return f.name || fileBase(f.file_path);
});
const canExport = computed(
  () => files.value.length > 0 && !!outputDir.value.trim() && !isRunning.value,
);
const isBlurLayout = computed(() => isBlurFrameTemplate(options.value.templateId));

function fileBase(path: string) {
  const s = String(path || '');
  const i = Math.max(s.lastIndexOf('/'), s.lastIndexOf('\\'));
  return i >= 0 ? s.slice(i + 1) : s;
}

function persistPresets(list: PhotoFramePreset[]) {
  ensurePhotoFrameConfig();
  (config as any).photoFrame.presets = normalizePhotoFramePresets(list);
  presetsTick.value += 1;
}

function persistFramePrefs() {
  ensurePhotoFrameConfig();
  (config as any).photoFrame.importToLibrary = !!importToLibrary.value;
  (config as any).photoFrame.lastTemplateId = options.value.templateId;
  (config as any).photoFrame.lastPresetId = selectedPresetId.value || '';
}

function selectTemplate(id: PhotoFrameTemplateId) {
  options.value = applyTemplatePreset(options.value, id);
  selectedPresetId.value = '';
  persistFramePrefs();
}

function savePreset() {
  presetNameInput.value = t('photo_frame.preset_default_name');
  showPresetNameBox.value = true;
}

function onPresetNameOk(nameInput: string) {
  showPresetNameBox.value = false;
  const name = String(nameInput || '').trim() || t('photo_frame.preset_default_name');
  const tpl: PhotoFramePreset = {
    id: createPhotoFramePresetId(),
    name,
    updatedAt: Date.now(),
    options: clonePhotoFrameOptions(normalizePhotoFrameOptions(options.value)),
  };
  persistPresets([tpl, ...customPresets.value]);
  selectedPresetId.value = tpl.id;
  persistFramePrefs();
  toast.success(t('photo_frame.preset_saved'));
}

function loadPreset() {
  const tpl = customPresets.value.find((x) => x.id === selectedPresetId.value);
  if (!tpl) return;
  options.value = clonePhotoFrameOptions(tpl.options);
  persistFramePrefs();
  toast.success(t('photo_frame.preset_loaded'));
}

async function deletePreset() {
  if (!selectedPresetId.value) return;
  const current = customPresets.value.find((x) => x.id === selectedPresetId.value);
  const confirmed = await ask(
    t('photo_frame.preset_delete_confirm', { name: current?.name || '' }),
    {
      title: t('photo_frame.delete_preset'),
      kind: 'warning',
      okLabel: t('photo_frame.delete_preset'),
      cancelLabel: t('msgbox.cancel'),
    },
  );
  if (!confirmed) return;
  persistPresets(customPresets.value.filter((x) => x.id !== selectedPresetId.value));
  selectedPresetId.value = '';
  persistFramePrefs();
}

async function pickLogo() {
  const file = await openDialog({
    multiple: false,
    filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    title: t('photo_frame.pick_logo'),
  });
  if (file) {
    options.value.logoPath = String(file);
    options.value.showLogo = true;
  }
}

function clearLogo() {
  options.value.logoPath = '';
  options.value.showLogo = true;
}

function revokePreviewUrl() {
  if (previewObjectUrl) {
    URL.revokeObjectURL(previewObjectUrl);
    previewObjectUrl = null;
  }
  previewUrl.value = '';
}

function bytesToObjectUrl(bytes: Uint8Array | number[] | ArrayBuffer): string {
  let arr: Uint8Array;
  if (bytes instanceof ArrayBuffer) {
    arr = new Uint8Array(bytes);
  } else if (bytes instanceof Uint8Array) {
    arr = bytes;
  } else {
    arr = new Uint8Array(bytes);
  }
  // Fresh buffer so Blob typing accepts the payload from Tauri invoke.
  const copy = new Uint8Array(arr.byteLength);
  copy.set(arr);
  const blob = new Blob([copy.buffer], { type: 'image/jpeg' });
  return URL.createObjectURL(blob);
}

async function runPreview() {
  const f = activeFile.value;
  if (!f?.file_path) {
    revokePreviewUrl();
    previewError.value = '';
    return;
  }
  const gen = ++previewGeneration;
  previewLoading.value = true;
  previewError.value = '';
  try {
    const bytes = await photoFramePreview({
      sourceFilePath: f.file_path,
      // Host clamps to ≤2048 and caches decode/EXIF/logo; keep UI preview light.
      maxEdge: 1000,
      options: { ...options.value },
    });
    if (gen !== previewGeneration) return;
    revokePreviewUrl();
    previewObjectUrl = bytesToObjectUrl(bytes as any);
    previewUrl.value = previewObjectUrl;
  } catch (err: any) {
    if (gen !== previewGeneration) return;
    revokePreviewUrl();
    previewError.value =
      typeof err === 'string' ? err : err?.message || t('photo_frame.preview_failed');
  } finally {
    if (gen === previewGeneration) previewLoading.value = false;
  }
}

function schedulePreview() {
  if (previewTimer) clearTimeout(previewTimer);
  previewTimer = setTimeout(() => {
    previewTimer = null;
    void runPreview();
  }, 300);
}

async function pickOutputDir() {
  const dir = await openDialog({
    directory: true,
    multiple: false,
    title: t('photo_frame.pick_output_dir'),
  });
  if (dir) outputDir.value = String(dir);
}

async function startExport() {
  errorMessage.value = '';
  if (files.value.length === 0) {
    errorMessage.value = t('photo_frame.need_photos');
    return;
  }
  if (!outputDir.value.trim()) {
    errorMessage.value = t('photo_frame.need_output_dir');
    return;
  }
  persistFramePrefs();
  isRunning.value = true;
  progress.value = {
    current: 0,
    total: files.value.length,
    status: 'start',
    filePath: '',
    message: '',
  };
  try {
    const result = await exportPhotoFrame({
      files: files.value.map((f) => ({ sourceFilePath: f.file_path })),
      options: normalizePhotoFrameOptions(options.value),
      outputDir: outputDir.value,
      outputFormat: outputFormat.value,
      quality: quality.value,
      nameMode: nameMode.value,
      prefix: 'frame',
      suffix: 'frame',
      overwritePolicy: 'rename',
    });
    if (result?.cancelled) toast.warning(t('photo_frame.cancelled'));
    else toast.success(t('photo_frame.finished', {
      ok: result?.succeeded ?? 0,
      fail: result?.failed ?? 0,
    }));
    emit('done', {
      ...result,
      importToLibrary: !!importToLibrary.value,
      outputMode: 'saveAs',
      sourceFiles: files.value.map((f) => ({
        id: f.id,
        file_path: f.file_path,
      })),
    });
  } catch (err: any) {
    const msg = typeof err === 'string' ? err : err?.message || t('photo_frame.failed');
    errorMessage.value = msg;
    toast.error(msg);
  } finally {
    isRunning.value = false;
  }
}

async function cancelRunning() {
  try {
    await cancelPhotoFrameExport();
  } catch {
    /* ignore */
  }
}

function onCancel() {
  if (isRunning.value) {
    void cancelRunning();
    return;
  }
  emit('cancel');
}

watch(
  [activeIndex, options],
  () => schedulePreview(),
  { deep: true },
);

watch(importToLibrary, () => {
  persistFramePrefs();
});

onMounted(async () => {
  unlistenProgress = await listen('photo-frame-progress', (event: any) => {
    const p = event?.payload || {};
    progress.value = {
      current: Number(p.current || 0),
      total: Number(p.total || files.value.length),
      status: String(p.status || ''),
      filePath: String(p.filePath || ''),
      message: String(p.message || ''),
    };
  });
  schedulePreview();
});

onUnmounted(() => {
  if (previewTimer) clearTimeout(previewTimer);
  if (unlistenProgress) unlistenProgress();
  if (isRunning.value) void cancelPhotoFrameExport();
  revokePreviewUrl();
});
</script>
