<template>
  <ModalDialog :title="$t('batch.title')" :width="820" :height="640" @cancel="onCancel">
    <div class="flex flex-col gap-3 text-sm select-none min-h-0 h-full">
      <!-- Steps -->
      <div class="flex items-center gap-2 text-xs">
        <button
          v-for="s in 3"
          :key="s"
          type="button"
          class="px-2.5 py-1 rounded-box border transition-colors"
          :class="step === s
            ? 'border-primary bg-primary/15 text-primary'
            : 'border-base-content/15 text-base-content/50'"
          :disabled="isRunning"
          @click="step = s"
        >
          {{ s }}. {{ $t(`batch.step_${s}`) }}
        </button>
        <span class="ml-auto text-base-content/40">
          {{ $t('batch.file_count', { count: files.length }) }}
        </span>
      </div>

      <!-- Step 1: files -->
      <div v-if="step === 1" class="flex-1 min-h-0 flex flex-col gap-2">
        <div class="flex flex-wrap gap-2">
          <button type="button" class="t-button-default text-xs" :disabled="isRunning" @click="addFiles">
            {{ $t('batch.add_files') }}
          </button>
          <button type="button" class="t-button-default text-xs" :disabled="isRunning || files.length === 0" @click="clearFiles">
            {{ $t('batch.clear_files') }}
          </button>
        </div>
        <div class="flex-1 min-h-0 overflow-auto rounded-box border border-base-content/10 p-2">
          <div v-if="files.length === 0" class="text-xs text-base-content/40 py-8 text-center">
            {{ $t('batch.files_empty') }}
          </div>
          <div v-else class="grid grid-cols-[repeat(auto-fill,minmax(4.5rem,1fr))] gap-1.5">
            <div
              v-for="(f, idx) in files"
              :key="f.file_path + idx"
              class="relative h-16 overflow-hidden rounded-box border border-base-content/10 bg-base-content/5"
              :title="f.name || f.file_path"
            >
              <img v-if="f.thumbnail" :src="f.thumbnail" class="h-full w-full object-cover" loading="lazy" />
              <div v-else class="h-full w-full flex items-center justify-center text-[9px] opacity-40 p-1 text-center break-all">
                {{ f.name || '…' }}
              </div>
              <button
                type="button"
                class="absolute top-0.5 right-0.5 badge badge-xs badge-neutral cursor-pointer"
                @click="removeFile(idx)"
              >×</button>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 2: actions -->
      <div v-else-if="step === 2" class="flex-1 min-h-0 grid grid-cols-[1fr_1fr] gap-3">
        <div class="min-h-0 flex flex-col gap-2 border border-base-content/10 rounded-box p-2">
          <div class="flex items-center justify-between">
            <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
              {{ $t('batch.action_list') }}
            </div>
            <button type="button" class="t-button-default text-xs" :disabled="!actions.length" @click="actions = []">
              {{ $t('batch.clear_actions') }}
            </button>
          </div>
          <div class="flex-1 min-h-0 overflow-auto space-y-1.5">
            <div v-if="!actions.length" class="text-xs text-base-content/40 py-6 text-center">
              {{ $t('batch.actions_empty') }}
            </div>
            <div
              v-for="(action, idx) in actions"
              :key="action.id"
              class="rounded-box border border-base-content/10 p-2 space-y-1.5"
              :class="selectedActionId === action.id ? 'border-primary/50 bg-primary/5' : ''"
              @click="selectedActionId = action.id"
            >
              <div class="flex items-center gap-1">
                <span class="text-xs font-medium flex-1 min-w-0 truncate">
                  {{ idx + 1 }}. {{ describeAction(action, t, customRatios) }}
                </span>
                <button type="button" class="btn btn-ghost btn-xs" :disabled="idx === 0" @click.stop="moveAction(idx, -1)">↑</button>
                <button type="button" class="btn btn-ghost btn-xs" :disabled="idx === actions.length - 1" @click.stop="moveAction(idx, 1)">↓</button>
                <button type="button" class="btn btn-ghost btn-xs" @click.stop="removeAction(idx)">×</button>
              </div>
              <div v-if="selectedActionId === action.id" class="grid grid-cols-2 gap-1.5 text-xs">
                <template v-if="action.type === 'resize'">
                  <select v-model="action.mode" class="select select-bordered select-xs col-span-2">
                    <option value="longEdge">{{ $t('batch.resize_long') }}</option>
                    <option value="width">{{ $t('batch.resize_w') }}</option>
                    <option value="height">{{ $t('batch.resize_h') }}</option>
                    <option value="percent">{{ $t('batch.resize_percent') }}</option>
                    <option value="exact">{{ $t('batch.resize_exact') }}</option>
                  </select>
                  <input v-if="action.mode !== 'height' && action.mode !== 'percent'" v-model.number="action.width" type="number" min="1" class="input input-bordered input-xs" :placeholder="$t('batch.width')" />
                  <input v-if="action.mode === 'height' || action.mode === 'exact'" v-model.number="action.height" type="number" min="1" class="input input-bordered input-xs" :placeholder="$t('batch.height')" />
                  <input v-if="action.mode === 'percent'" v-model.number="action.percent" type="number" min="1" max="500" class="input input-bordered input-xs col-span-2" />
                </template>
                <template v-else-if="action.type === 'crop'">
                  <select v-model="action.presetId" class="select select-bordered select-xs col-span-2">
                    <option v-for="opt in cropOptions" :key="opt.id" :value="opt.id">
                      {{ opt.label || $t(`msgbox.image_editor.photo_sizes.${opt.labelKey}`) }}
                    </option>
                  </select>
                  <label class="flex items-center gap-1 col-span-2">
                    <input v-model="action.portrait" type="checkbox" class="checkbox checkbox-xs" />
                    {{ $t('batch.crop_portrait') }}
                  </label>
                  <label class="flex items-center gap-1 col-span-2">
                    <input v-model="action.applyTargetPixels" type="checkbox" class="checkbox checkbox-xs" />
                    {{ $t('batch.crop_apply_px') }}
                  </label>
                </template>
                <template v-else-if="action.type === 'rotate'">
                  <select v-model.number="action.degrees" class="select select-bordered select-xs col-span-2">
                    <option :value="90">90°</option>
                    <option :value="180">180°</option>
                    <option :value="270">270°</option>
                  </select>
                </template>
                <template v-else-if="action.type === 'flip'">
                  <select v-model="action.axis" class="select select-bordered select-xs col-span-2">
                    <option value="horizontal">{{ $t('batch.flip_h') }}</option>
                    <option value="vertical">{{ $t('batch.flip_v') }}</option>
                  </select>
                </template>
                <template v-else-if="action.type === 'filter'">
                  <select v-model="action.filter" class="select select-bordered select-xs col-span-2">
                    <option value="grayscale">{{ $t('batch.action_filter_grayscale') }}</option>
                    <option value="sepia">{{ $t('batch.action_filter_sepia') }}</option>
                    <option value="invert">{{ $t('batch.action_filter_invert') }}</option>
                  </select>
                </template>
                <template v-else-if="action.type === 'border'">
                  <input v-model.number="action.width" type="number" min="1" max="500" class="input input-bordered input-xs" :placeholder="$t('batch.border_width')" />
                  <input v-model="action.color" type="color" class="h-7 w-full cursor-pointer rounded border border-base-content/15 bg-transparent" />
                </template>
                <template v-else-if="action.type === 'expand'">
                  <input v-model.number="action.top" type="number" min="0" class="input input-bordered input-xs" :placeholder="$t('batch.expand_top')" />
                  <input v-model.number="action.bottom" type="number" min="0" class="input input-bordered input-xs" :placeholder="$t('batch.expand_bottom')" />
                  <input v-model.number="action.left" type="number" min="0" class="input input-bordered input-xs" :placeholder="$t('batch.expand_left')" />
                  <input v-model.number="action.right" type="number" min="0" class="input input-bordered input-xs" :placeholder="$t('batch.expand_right')" />
                  <div class="col-span-2 flex items-center gap-2">
                    <input v-model="action.color" type="color" class="h-7 w-12 cursor-pointer rounded border border-base-content/15 bg-transparent" />
                    <input v-model="action.color" type="text" class="input input-bordered input-xs flex-1" />
                  </div>
                </template>
                <template v-else-if="action.type === 'watermark'">
                  <div class="col-span-2 flex gap-1">
                    <input v-model="action.imagePath" type="text" class="input input-bordered input-xs flex-1 min-w-0" :placeholder="$t('batch.watermark_path')" readonly />
                    <button type="button" class="t-button-default text-[10px]" @click.stop="pickWatermark(action)">{{ $t('batch.browse') }}</button>
                  </div>
                  <select v-model="action.position" class="select select-bordered select-xs col-span-2">
                    <option v-for="pos in anchorOptions" :key="pos" :value="pos">{{ $t(`batch.pos_${pos.replace('-', '_')}`) }}</option>
                  </select>
                  <label class="col-span-2 text-[11px] opacity-60">{{ $t('batch.scale') }}: {{ action.scale }}%</label>
                  <input v-model.number="action.scale" type="range" min="5" max="80" class="range range-xs range-primary col-span-2" />
                  <label class="col-span-2 text-[11px] opacity-60">{{ $t('batch.opacity') }}: {{ action.opacity }}%</label>
                  <input v-model.number="action.opacity" type="range" min="5" max="100" class="range range-xs range-primary col-span-2" />
                  <input v-model.number="action.margin" type="number" min="0" class="input input-bordered input-xs col-span-2" :placeholder="$t('batch.margin')" />
                </template>
                <template v-else-if="action.type === 'text'">
                  <input v-model="action.text" type="text" maxlength="120" class="input input-bordered input-xs col-span-2" :placeholder="$t('batch.text_content')" />
                  <select v-model="action.position" class="select select-bordered select-xs col-span-2">
                    <option v-for="pos in anchorOptions" :key="pos" :value="pos">{{ $t(`batch.pos_${pos.replace('-', '_')}`) }}</option>
                  </select>
                  <input v-model.number="action.fontSize" type="number" min="8" max="400" class="input input-bordered input-xs" :placeholder="$t('batch.font_size')" />
                  <input v-model="action.color" type="color" class="h-7 w-full cursor-pointer rounded border border-base-content/15 bg-transparent" />
                  <label class="col-span-2 text-[11px] opacity-60">{{ $t('batch.opacity') }}: {{ action.opacity }}%</label>
                  <input v-model.number="action.opacity" type="range" min="5" max="100" class="range range-xs range-primary col-span-2" />
                  <input v-model.number="action.margin" type="number" min="0" class="input input-bordered input-xs col-span-2" :placeholder="$t('batch.margin')" />
                </template>
                <template v-else>
                  <input v-model.number="action.value" type="range" class="range range-xs range-primary col-span-2" :min="sliderMin(action)" :max="sliderMax(action)" :step="1" />
                  <span class="col-span-2 text-[11px] opacity-50">{{ action.value }}</span>
                </template>
              </div>
            </div>
          </div>
        </div>

        <div class="min-h-0 flex flex-col gap-2 border border-base-content/10 rounded-box p-2">
          <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
            {{ $t('batch.palette') }}
          </div>
          <div class="flex flex-wrap gap-1.5 content-start">
            <button
              v-for="p in palette"
              :key="p.type"
              type="button"
              class="t-button-default text-xs"
              @click="addAction(p.type)"
            >
              + {{ $t(`batch.type_${p.type}`) }}
            </button>
          </div>
          <div class="border-t border-base-content/10 pt-2 space-y-1.5">
            <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
              {{ $t('batch.one_click') }}
            </div>
            <div class="flex flex-wrap gap-1.5">
              <button type="button" class="t-button-default text-xs" :disabled="!actions.length" @click="saveTemplate">
                {{ $t('batch.save_template') }}
              </button>
              <select v-model="selectedTemplateId" class="select select-bordered select-xs flex-1 min-w-[8rem]">
                <option value="">{{ templates.length ? $t('batch.pick_template') : $t('batch.no_templates') }}</option>
                <option v-for="tpl in templates" :key="tpl.id" :value="tpl.id">{{ tpl.name }}</option>
              </select>
              <button type="button" class="t-button-default text-xs" :disabled="!selectedTemplateId" @click="loadTemplate">
                {{ $t('batch.load_template') }}
              </button>
              <button type="button" class="t-button-default text-xs" :disabled="!selectedTemplateId" @click="deleteTemplate">
                {{ $t('batch.delete_template') }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Step 3: output -->
      <div v-else class="flex-1 min-h-0 overflow-auto space-y-3">
        <div class="grid grid-cols-2 gap-3">
          <label class="form-control">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.output_mode') }}</span>
            <select v-model="outputMode" class="select select-bordered select-sm">
              <option value="saveAs">{{ $t('batch.output_save_as') }}</option>
              <option value="overwrite">{{ $t('batch.output_overwrite') }}</option>
            </select>
          </label>
          <label class="form-control">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.format') }}</span>
            <select v-model="outputFormat" class="select select-bordered select-sm">
              <option value="jpg">JPEG</option>
              <option value="png">PNG</option>
              <option value="webp">WebP</option>
            </select>
          </label>
          <label v-if="outputMode === 'saveAs'" class="form-control col-span-2">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.output_dir') }}</span>
            <div class="flex gap-2">
              <input v-model="outputDir" type="text" class="input input-bordered input-sm flex-1 min-w-0" readonly />
              <button type="button" class="t-button-default text-xs" @click="pickOutputDir">{{ $t('batch.browse') }}</button>
            </div>
          </label>
          <label class="form-control">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.name_mode') }}</span>
            <select v-model="nameMode" class="select select-bordered select-sm">
              <option value="original">{{ $t('batch.name_original') }}</option>
              <option value="prefix">{{ $t('batch.name_prefix') }}</option>
              <option value="suffix">{{ $t('batch.name_suffix') }}</option>
              <option value="sequence">{{ $t('batch.name_sequence') }}</option>
            </select>
          </label>
          <label class="form-control">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.overwrite_policy') }}</span>
            <select v-model="overwritePolicy" class="select select-bordered select-sm">
              <option value="skip">{{ $t('batch.policy_skip') }}</option>
              <option value="rename">{{ $t('batch.policy_rename') }}</option>
              <option value="overwrite">{{ $t('batch.policy_overwrite') }}</option>
            </select>
          </label>
          <label v-if="nameMode === 'prefix' || nameMode === 'sequence'" class="form-control">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.prefix') }}</span>
            <input v-model="namePrefix" type="text" class="input input-bordered input-sm" />
          </label>
          <label v-if="nameMode === 'suffix'" class="form-control">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.suffix') }}</span>
            <input v-model="nameSuffix" type="text" class="input input-bordered input-sm" />
          </label>
          <label v-if="outputFormat === 'jpg'" class="form-control col-span-2">
            <span class="label-text text-xs opacity-70 mb-1">{{ $t('batch.quality') }}: {{ quality }}</span>
            <input v-model.number="quality" type="range" min="40" max="100" step="1" class="range range-xs range-primary" />
          </label>
          <label class="flex items-start gap-2 col-span-2 pt-1">
            <input
              v-model="importToLibrary"
              type="checkbox"
              class="checkbox checkbox-xs checkbox-primary mt-0.5"
              :disabled="outputMode === 'overwrite'"
              @change="persistImportPref"
            />
            <span class="text-xs leading-snug">
              <span class="font-medium">{{ $t('batch.import_to_library') }}</span>
              <span class="block text-base-content/45 mt-0.5">
                {{ outputMode === 'overwrite' ? $t('batch.import_overwrite_hint') : $t('batch.import_to_library_hint') }}
              </span>
            </span>
          </label>
        </div>

        <div v-if="isRunning || lastResult" class="rounded-box border border-base-content/10 p-3 space-y-2">
          <div class="flex justify-between text-xs">
            <span>{{ progressLabel }}</span>
            <span>{{ progress.current }}/{{ progress.total }}</span>
          </div>
          <progress class="progress progress-primary w-full" :value="progress.current" :max="Math.max(1, progress.total)" />
          <div v-if="lastResult" class="text-xs text-base-content/60">
            {{ $t('batch.result_summary', lastResult) }}
          </div>
          <div v-if="lastResult?.errors?.length" class="max-h-24 overflow-auto text-[11px] text-error whitespace-pre-wrap">
            {{ lastResult.errors.slice(0, 20).join('\n') }}
          </div>
        </div>
      </div>

      <p v-if="errorMessage" class="text-error text-xs">{{ errorMessage }}</p>

      <div class="flex justify-between gap-2 pt-1 shrink-0">
        <button class="t-button-default" :disabled="isRunning || step === 1" @click="step = Math.max(1, step - 1)">
          {{ $t('batch.prev') }}
        </button>
        <div class="flex gap-2">
          <button class="t-button-default" :disabled="isRunning" @click="onCancel">{{ $t('msgbox.cancel') }}</button>
          <button
            v-if="step < 3"
            class="t-button-primary"
            :disabled="!canNext"
            @click="step += 1"
          >
            {{ $t('batch.next') }}
          </button>
          <button
            v-else-if="!isRunning"
            class="t-button-primary"
            :disabled="!canStart"
            @click="startBatch"
          >
            {{ $t('batch.start') }}
          </button>
          <button
            v-else
            class="t-button-default"
            @click="cancelRunning"
          >
            {{ $t('batch.stop') }}
          </button>
        </div>
      </div>
    </div>
  </ModalDialog>

  <MessageBox
    v-if="showTemplateNameBox"
    :title="$t('batch.save_template')"
    :message="$t('batch.template_name_prompt')"
    :showInput="true"
    :inputText="templateNameInput"
    :inputPlaceholder="$t('batch.template_name_prompt')"
    :OkText="$t('msgbox.ok')"
    :cancelText="$t('msgbox.cancel')"
    @ok="onTemplateNameOk"
    @cancel="showTemplateNameBox = false"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { ask, open as openDialog } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import ModalDialog from '@/components/ModalDialog.vue';
import MessageBox from '@/components/MessageBox.vue';
import { batchProcessImages, cancelBatchProcess } from '@/common/api';
import { config } from '@/common/config';
import {
  BATCH_ACTION_PALETTE,
  createBatchAction,
  createBatchTemplateId,
  cropPresetOptions,
  describeAction,
  filterBatchImageFiles,
  normalizeBatchTemplates,
  type BatchAction,
  type BatchActionTemplate,
  type BatchActionType,
  type BatchFileItem,
  type BatchNameMode,
  type BatchOutputMode,
  type BatchOverwritePolicy,
} from '@/common/batchProcess';
import { normalizeCustomCropRatios, type CustomCropRatio } from '@/common/photoSizePresets';
import { useToast } from '@/common/toast';

const props = defineProps<{
  files: BatchFileItem[];
}>();

const emit = defineEmits<{
  cancel: [];
  done: [result: any];
}>();

const { t } = useI18n();
const toast = useToast();

const step = ref(1);
const files = ref<BatchFileItem[]>(
  filterBatchImageFiles(props.files || []).map((f) => ({ ...f, file_path: String(f.file_path || '') })),
);
const actions = ref<BatchAction[]>([]);
const selectedActionId = ref('');
const palette = BATCH_ACTION_PALETTE;
const anchorOptions = [
  'bottom-right',
  'bottom-left',
  'top-right',
  'top-left',
  'center',
  'top',
  'bottom',
  'left',
  'right',
] as const;

const outputMode = ref<BatchOutputMode>('saveAs');
const outputDir = ref('');
const outputFormat = ref<'jpg' | 'png' | 'webp'>('jpg');
const nameMode = ref<BatchNameMode>('suffix');
const namePrefix = ref('out');
const nameSuffix = ref('edit');
const overwritePolicy = ref<BatchOverwritePolicy>('rename');
const quality = ref(90);

const isRunning = ref(false);
const errorMessage = ref('');
const progress = ref({ current: 0, total: 0, status: '', filePath: '', message: '' });
const lastResult = ref<any>(null);
const selectedTemplateId = ref('');
const templatesTick = ref(0);
const showTemplateNameBox = ref(false);
const templateNameInput = ref('');

let unlistenProgress: (() => void) | null = null;

const customRatios = computed<CustomCropRatio[]>(() =>
  normalizeCustomCropRatios((config as any).imageEditor?.customCropRatios),
);
const cropOptions = computed(() => cropPresetOptions(customRatios.value));

function ensureTemplatesConfig() {
  if (!(config as any).batchProcess || typeof (config as any).batchProcess !== 'object') {
    (config as any).batchProcess = { templates: [], importToLibrary: false };
  } else if (!Array.isArray((config as any).batchProcess.templates)) {
    (config as any).batchProcess.templates = [];
  } else {
    (config as any).batchProcess.templates = normalizeBatchTemplates((config as any).batchProcess.templates);
  }
  if (typeof (config as any).batchProcess.importToLibrary !== 'boolean') {
    (config as any).batchProcess.importToLibrary = false;
  }
}
ensureTemplatesConfig();

const importToLibrary = ref(Boolean((config as any).batchProcess?.importToLibrary));

function persistImportPref() {
  ensureTemplatesConfig();
  (config as any).batchProcess.importToLibrary = !!importToLibrary.value;
}

const templates = computed<BatchActionTemplate[]>(() => {
  templatesTick.value;
  ensureTemplatesConfig();
  return normalizeBatchTemplates((config as any).batchProcess?.templates);
});

const canNext = computed(() => {
  if (step.value === 1) return files.value.length > 0;
  if (step.value === 2) return actions.value.length > 0;
  return true;
});

const canStart = computed(() =>
  files.value.length > 0
  && actions.value.length > 0
  && (outputMode.value === 'overwrite' || !!outputDir.value),
);

const progressLabel = computed(() => {
  if (isRunning.value) return progress.value.filePath || t('batch.running');
  if (lastResult.value?.cancelled) return t('batch.cancelled');
  if (lastResult.value) return t('batch.finished');
  return '';
});

function sliderMin(action: BatchAction) {
  if (action.type === 'hue') return -180;
  if (action.type === 'brightness' || action.type === 'contrast') return -100;
  return 0;
}
function sliderMax(action: BatchAction) {
  if (action.type === 'saturation') return 200;
  if (action.type === 'hue') return 180;
  if (action.type === 'blur') return 20;
  return 100;
}

function addAction(type: BatchActionType) {
  const a = createBatchAction(type);
  actions.value.push(a);
  selectedActionId.value = a.id;
}
function removeAction(idx: number) {
  const [removed] = actions.value.splice(idx, 1);
  if (removed && selectedActionId.value === removed.id) selectedActionId.value = actions.value[0]?.id || '';
}
function moveAction(idx: number, dir: number) {
  const j = idx + dir;
  if (j < 0 || j >= actions.value.length) return;
  const arr = actions.value.slice();
  const tmp = arr[idx];
  arr[idx] = arr[j];
  arr[j] = tmp;
  actions.value = arr;
}

function removeFile(idx: number) {
  files.value.splice(idx, 1);
}
function clearFiles() {
  files.value = [];
}

async function addFiles() {
  const selected = await openDialog({
    multiple: true,
    filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp', 'heic', 'heif', 'tif', 'tiff', 'bmp'] }],
  });
  if (!selected) return;
  const list = Array.isArray(selected) ? selected : [selected];
  const existing = new Set(files.value.map((f) => f.file_path));
  for (const path of list) {
    const p = String(path);
    if (!p || existing.has(p)) continue;
    const name = p.split(/[/\\]/).pop() || p;
    files.value.push({ file_path: p, name, file_type: 1 });
    existing.add(p);
  }
}

async function pickOutputDir() {
  const selected = await openDialog({ directory: true, multiple: false });
  if (selected) outputDir.value = String(selected);
}

async function pickWatermark(action: BatchAction) {
  if (action.type !== 'watermark') return;
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
  });
  if (selected) action.imagePath = String(selected);
}

function persistTemplates(list: BatchActionTemplate[]) {
  ensureTemplatesConfig();
  (config as any).batchProcess.templates = normalizeBatchTemplates(list);
  templatesTick.value += 1;
}

function saveTemplate() {
  if (!actions.value.length) return;
  // Tauri WebView often no-ops window.prompt; use in-app MessageBox instead.
  templateNameInput.value = t('batch.template_default_name');
  showTemplateNameBox.value = true;
}

function onTemplateNameOk(nameInput: string) {
  showTemplateNameBox.value = false;
  const tpl: BatchActionTemplate = {
    id: createBatchTemplateId(),
    name: String(nameInput || '').trim() || t('batch.template_default_name'),
    updatedAt: Date.now(),
    actions: JSON.parse(JSON.stringify(actions.value)),
  };
  persistTemplates([tpl, ...templates.value]);
  selectedTemplateId.value = tpl.id;
  toast.success(t('batch.template_saved'));
}

function loadTemplate() {
  const tpl = templates.value.find((x) => x.id === selectedTemplateId.value);
  if (!tpl) return;
  actions.value = JSON.parse(JSON.stringify(tpl.actions));
  selectedActionId.value = actions.value[0]?.id || '';
  toast.success(t('batch.template_loaded'));
}

async function deleteTemplate() {
  if (!selectedTemplateId.value) return;
  const confirmed = await ask(t('batch.template_delete_confirm'), {
    title: t('batch.delete_template'),
    kind: 'warning',
    okLabel: t('batch.delete_template'),
    cancelLabel: t('msgbox.cancel'),
  });
  if (!confirmed) return;
  persistTemplates(templates.value.filter((x) => x.id !== selectedTemplateId.value));
  selectedTemplateId.value = '';
}

function hostActions() {
  return actions.value.map((a) => {
    const base: any = { ...a };
    delete base.id;
    if (a.type === 'crop') {
      // Host resolves known preset ids; custom ratios need explicit ratio_w/h
      const custom = customRatios.value.find((c) => c.id === a.presetId);
      if (custom) {
        base.ratio_w = custom.ratioW;
        base.ratio_h = custom.ratioH;
      }
    }
    if (a.type === 'saturation') {
      // keep percent 0-200 for host
      base.value = a.value;
    }
    return base;
  });
}

async function startBatch() {
  errorMessage.value = '';
  lastResult.value = null;
  if (files.value.length === 0 || actions.value.length === 0) {
    errorMessage.value = t('batch.need_files_actions');
    return;
  }
  if (outputMode.value === 'saveAs' && !outputDir.value) {
    errorMessage.value = t('batch.need_output_dir');
    return;
  }
  if (outputMode.value === 'overwrite') {
    // Critical: window.confirm is unreliable in Tauri WebView; a no-op can
    // yield undefined and skip this guard, overwriting originals silently.
    const confirmed = await ask(t('batch.overwrite_confirm', { count: files.value.length }), {
      title: t('batch.title'),
      kind: 'warning',
      okLabel: t('batch.start'),
      cancelLabel: t('msgbox.cancel'),
    });
    if (!confirmed) return;
  }

  isRunning.value = true;
  progress.value = { current: 0, total: files.value.length, status: 'start', filePath: '', message: '' };
  try {
    const result = await batchProcessImages({
      files: files.value.map((f) => ({
        sourceFilePath: f.file_path,
        orientation: Number(f.e_orientation ?? f.orientation ?? 1) || 1,
      })),
      actions: hostActions(),
      outputDir: outputMode.value === 'saveAs' ? outputDir.value : null,
      outputMode: outputMode.value,
      outputFormat: outputFormat.value,
      quality: quality.value,
      nameMode: nameMode.value,
      prefix: namePrefix.value,
      suffix: nameSuffix.value,
      overwritePolicy: outputMode.value === 'overwrite' ? 'overwrite' : overwritePolicy.value,
    });
    lastResult.value = result;
    if (result?.cancelled) toast.warning(t('batch.cancelled'));
    else toast.success(t('batch.finished'));
    persistImportPref();
    emit('done', {
      ...result,
      importToLibrary: !!importToLibrary.value,
      outputMode: outputMode.value,
      // For overwrite refresh: library ids from the source selection when present.
      sourceFiles: files.value.map((f) => ({
        id: f.id,
        file_path: f.file_path,
      })),
    });
  } catch (err: any) {
    const msg = typeof err === 'string' ? err : err?.message || t('batch.failed');
    errorMessage.value = msg;
    toast.error(msg);
  } finally {
    isRunning.value = false;
  }
}

async function cancelRunning() {
  try {
    await cancelBatchProcess();
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

onMounted(async () => {
  unlistenProgress = await listen('batch-process-progress', (event: any) => {
    const p = event?.payload || {};
    progress.value = {
      current: Number(p.current || 0),
      total: Number(p.total || files.value.length),
      status: String(p.status || ''),
      filePath: String(p.filePath || ''),
      message: String(p.message || ''),
    };
    if (p.result) lastResult.value = p.result;
  });
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (isRunning.value) void cancelBatchProcess();
});
</script>
