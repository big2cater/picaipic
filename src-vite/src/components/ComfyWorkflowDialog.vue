<template>
  <div
    v-if="show"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="close"
  >
    <div class="w-[540px] max-w-[92vw] rounded-box bg-base-200 border border-base-content/10 shadow-xl p-4 space-y-3">
      <div class="text-sm font-semibold text-base-content/80">{{ $t('comfy.import_title') }}</div>

      <div class="space-y-1">
        <label class="text-xs text-base-content/50">{{ $t('comfy.workflow_name') }}</label>
        <input
          v-model="name"
          type="text"
          class="input input-sm input-bordered w-full"
          :placeholder="$t('comfy.workflow_name_placeholder')"
        />
        <div v-if="willOverwrite" class="text-xs text-warning/80">
          {{ $t('comfy.will_overwrite', { name: name.trim() }) }}
        </div>
      </div>

      <div class="space-y-1">
        <div class="flex items-center justify-between gap-2">
          <label class="text-xs text-base-content/50">{{ $t('comfy.workflow_json') }}</label>
          <button type="button" class="btn btn-xs btn-ghost" @click="pickFile">
            {{ $t('comfy.select_file') }}
          </button>
        </div>
        <textarea
          v-model="text"
          rows="12"
          spellcheck="false"
          class="textarea textarea-bordered w-full font-mono text-xs leading-relaxed"
          :placeholder="jsonPlaceholder"
        ></textarea>
        <div class="text-xs text-base-content/40">{{ $t('comfy.workflow_json_hint') }}</div>
      </div>

      <input
        ref="fileInputRef"
        type="file"
        accept=".json,application/json"
        class="hidden"
        @change="onFilePicked"
      />

      <div v-if="error" class="text-xs text-error/80 break-all">{{ error }}</div>
      <div v-else-if="summary" class="text-xs text-base-content/60">{{ summary }}</div>

      <div
        v-if="uiDetected"
        class="flex items-center justify-between gap-2 rounded-box bg-base-100/50 px-2.5 py-2"
      >
        <span class="text-xs text-base-content/60">{{ $t('comfy.ui_convert_hint') }}</span>
        <button
          type="button"
          class="btn btn-xs btn-primary shrink-0 gap-1.5"
          :disabled="isConverting"
          @click="convertUi"
        >
          <span v-if="isConverting" class="loading loading-spinner loading-xs"></span>
          {{ isConverting ? $t('comfy.converting') : $t('comfy.convert_and_import') }}
        </button>
      </div>

      <div class="flex justify-end gap-2 pt-1">
        <button class="btn btn-sm btn-ghost" @click="close">{{ $t('msgbox.cancel') }}</button>
        <button class="btn btn-sm btn-primary" :disabled="!canSave" @click="save">
          {{ $t('comfy.save_workflow') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { config } from '@/common/config';
import { comfyObjectInfo } from '@/common/api';
import { isUiWorkflow, normalizeApiWorkflow, convertUiToApi } from '@/common/comfyConvert';

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{
  close: [];
  save: [payload: { name: string; workflow: any }];
}>();

const { t } = useI18n();

const name = ref('');
const text = ref('');
const error = ref('');
const summary = ref('');
const parsed = ref<any>(null);
const uiDetected = ref(false);
const isConverting = ref(false);

const jsonPlaceholder = '{\n  "3": { "class_type": "LoadImage", "inputs": { "image": "example.png" } }\n}';

watch(
  () => props.show,
  (visible) => {
    if (!visible) return;
    name.value = '';
    text.value = '';
    error.value = '';
    summary.value = '';
    parsed.value = null;
    uiDetected.value = false;
  }
);

/// Re-check on every keystroke so an unusable workflow is reported before saving,
/// not after ComfyUI rejects it.
watch(text, (value) => {
  error.value = '';
  summary.value = '';
  parsed.value = null;
  uiDetected.value = false;
  if (!value.trim()) return;

  let obj: any;
  try {
    obj = JSON.parse(value);
  } catch {
    error.value = t('comfy.invalid_json');
    return;
  }

  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) {
    error.value = t('comfy.not_api_format');
    return;
  }

  // A UI-format graph describes the same workflow, but as a canvas whose widget values are
  // positional. Offer a conversion instead of rejecting it with the generic message.
  if (isUiWorkflow(obj)) {
    uiDetected.value = true;
    error.value = t('comfy.ui_format_detected');
    return;
  }
  // Strip canvas-only keys (nodes/links) and rebuild the keyed form, so a valid export that
  // happens to carry them still imports, and nothing stray reaches the server.
  const api = normalizeApiWorkflow(obj);
  if (!api) {
    error.value = t('comfy.not_api_format');
    return;
  }
  const entries = Object.entries(api);
  if (entries.length === 0) {
    error.value = t('comfy.empty_workflow');
    return;
  }

  // ComfyUI's convention: a node reading a file out of the server's input directory takes it
  // as a string `image` input. Matching on that shape instead of on `class_type` keeps custom
  // loaders such as LoadImageAutoMP working, and it is the same field the runner overwrites
  // with the uploaded photo.
  const loaders = entries.filter(
    ([, node]: [string, any]) => typeof node?.inputs?.image === 'string'
  );
  const savers = entries.filter(
    ([, node]: [string, any]) =>
      String(node.class_type) === 'SaveImage' || String(node.class_type) === 'SaveImageWebsocket'
  );
  if (loaders.length === 0) {
    error.value = t('comfy.no_load_image');
    return;
  }
  if (savers.length === 0) {
    error.value = t('comfy.no_save_image');
    return;
  }

  parsed.value = obj;
  summary.value = t('comfy.workflow_summary', {
    nodes: entries.length,
    inputs: loaders.length,
  });
});

const canSave = computed(() => name.value.trim().length > 0 && parsed.value !== null);

/// Same name as a saved workflow means saving will replace it rather than create a duplicate.
const willOverwrite = computed(() => {
  const trimmed = name.value.trim();
  return (
    trimmed.length > 0 &&
    (config.comfy?.workflows || []).some((wf: any) => wf.name === trimmed)
  );
});

function describeConvertError(err: unknown) {
  const raw = String((err as Error)?.message || err || '');
  const separator = raw.indexOf(':');
  const key = separator === -1 ? raw : raw.slice(0, separator);
  const detail = separator === -1 ? '' : raw.slice(separator + 1).trim();
  switch (key) {
    case 'ui_no_object_info':
      return t('comfy.convert_need_server');
    case 'ui_bypass_unsupported':
      return t('comfy.convert_bypass');
    case 'ui_primitive_unsupported':
      return t('comfy.convert_primitive');
    case 'ui_no_nodes':
      return t('comfy.convert_no_nodes');
    case 'ui_missing_nodes':
      return t('comfy.convert_missing_nodes', { types: detail });
    default:
      return t('comfy.convert_failed', { detail: raw });
  }
}

/// Rewrites the pasted UI-format graph in place; the `text` watcher then re-runs the
/// ordinary API-format validation against the converted result.
async function convertUi() {
  if (isConverting.value) return;
  isConverting.value = true;
  error.value = '';
  try {
    const objectInfo = await comfyObjectInfo(config.comfy?.serverUrl || '');
    const { api } = convertUiToApi(JSON.parse(text.value), objectInfo);
    text.value = JSON.stringify(api, null, 2);
  } catch (err) {
    error.value = describeConvertError(err);
  } finally {
    isConverting.value = false;
  }
}

const fileInputRef = ref<HTMLInputElement | null>(null);

function pickFile() {
  fileInputRef.value?.click();
}

/// Reads through the browser File API rather than the Tauri fs plugin: fs scope only covers
/// $HOME/$DOWNLOAD/..., so a workflow kept on another drive would otherwise be rejected.
async function onFilePicked(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    text.value = await file.text();
    // Pre-fill the name from the filename unless the user already typed one.
    if (!name.value.trim()) {
      name.value = file.name.replace(/\.json$/i, '');
    }
  } catch {
    error.value = t('comfy.read_file_failed');
  } finally {
    // Reset so picking the same file again still fires change.
    input.value = '';
  }
}

function close() {
  emit('close');
}

function save() {
  if (!canSave.value) return;
  emit('save', { name: name.value.trim(), workflow: parsed.value });
}
</script>
