<template>
  <div
    v-if="show"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    @click.self="close"
  >
    <div class="w-[460px] max-w-[92vw] rounded-box bg-base-200 border border-base-content/10 shadow-xl p-4 space-y-3">
      <div class="text-sm font-semibold text-base-content/80">{{ $t('comfy.run_title') }}</div>

      <div class="text-xs text-base-content/50">
        <span v-if="fileCount > 1">{{ $t('comfy.file_count', { count: fileCount }) }}</span>
        <span v-else class="block truncate">{{ files[0]?.file_path || '' }}</span>
      </div>

      <div class="space-y-1">
        <label class="text-xs text-base-content/50">{{ $t('comfy.select_saved_workflow') }}</label>
        <select
          v-model="selectedId"
          class="select select-sm select-bordered w-full"
          :disabled="busy"
        >
          <option v-for="wf in workflows" :key="wf.id" :value="wf.id">{{ wf.name }}</option>
        </select>
      </div>

      <div v-if="busy" class="space-y-1.5">
        <div class="flex items-center gap-2 text-xs text-base-content/60">
          <span class="loading loading-spinner loading-xs"></span>
          <span class="truncate">{{ progressLabel }}</span>
        </div>
        <progress
          v-if="fileCount > 1"
          class="progress progress-primary w-full h-1"
          :value="completedCount"
          :max="fileCount"
        ></progress>
      </div>

      <div v-if="error" class="text-xs text-error/80 break-all">{{ error }}</div>
      <div v-else-if="success" class="text-xs text-base-content/60">{{ success }}</div>

      <div class="flex justify-end gap-2 pt-1">
        <button
          v-if="busy"
          class="btn btn-sm btn-ghost text-error"
          :disabled="cancelling"
          @click="cancel"
        >
          {{ cancelling ? $t('comfy.cancelling') : $t('comfy.cancel_run') }}
        </button>
        <button v-else class="btn btn-sm btn-ghost" @click="close">
          {{ $t('msgbox.cancel') }}
        </button>
        <button
          class="btn btn-sm btn-primary"
          :disabled="busy || !selectedId || fileCount === 0"
          @click="run"
        >
          {{ $t('comfy.run_action') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useToast } from '@/common/toast';
import { config } from '@/common/config';
import {
  comfyUploadImage,
  comfyRunWorkflow,
  comfyCancelRun,
  comfyDownloadOutput,
  importFile,
  tempFilePath,
  deleteTempFile,
} from '@/common/api';

const props = defineProps<{
  show: boolean;
  files: any[];
  /// Where the finished images land: `{ folderId, folderPath, albumId }`.
  destination: { folderId: number; folderPath?: string; albumId: number } | null;
}>();
const emit = defineEmits<{
  close: [];
  imported: [payload: { albumId: number; fileIds: string[] }];
}>();

const { t } = useI18n();
const toast = useToast();

const files = computed(() => props.files || []);
const fileCount = computed(() => files.value.length);

const selectedId = ref('');
const busy = ref(false);
const stage = ref('');
const error = ref('');
const success = ref('');
const completedCount = ref(0);
const cancelling = ref(false);

/// Set by the user; checked between files and after each await.
let cancelled = false;
/// Id of the run in flight, so `cancel` can name it. Plain variable: only read imperatively.
let currentPromptId = '';
/// Serial for the readable import names (`comfy_<workflow>_<n>`), reset per run.
let importSerial = 0;

/// Derive a safe, readable filename stem from a workflow name. The backend sanitizes
/// again, but keeping the UI copy clean avoids surprising renames after the fact.
function readableWorkflowStem(name: string) {
  const cleaned = String(name || '')
    .replace(/[\\/:*?"<>|]/g, '_')
    .replace(/\s+/g, ' ')
    .trim();
  return (cleaned || 'workflow').slice(0, 40);
}

const workflows = computed(() => config.comfy?.workflows || []);

const stageLabel = computed(() => {
  switch (stage.value) {
    case 'uploading':
      return t('comfy.stage_uploading');
    case 'running':
      return t('comfy.stage_running');
    case 'downloading':
      return t('comfy.stage_downloading');
    case 'importing':
      return t('comfy.stage_importing');
    case 'cooldown':
      return t('comfy.stage_cooldown');
    default:
      return '';
  }
});

/// Wait that stays interruptible: being made to sit out a cooldown after asking to stop
/// would be pointless, so the wait is checked in small slices.
function wait(ms: number) {
  return new Promise<void>((resolve) => {
    const step = 100;
    let elapsed = 0;
    const timer = setInterval(() => {
      elapsed += step;
      if (elapsed >= ms || cancelled) {
        clearInterval(timer);
        resolve();
      }
    }, step);
  });
}

/// Breath between images so VRAM is genuinely free before the next run claims it.
async function cooldown() {
  const seconds = Math.max(0, Number(config.comfy?.cooldownSecs || 0));
  if (seconds === 0) return;
  stage.value = 'cooldown';
  await wait(seconds * 1000);
}

const progressLabel = computed(() => {
  const position = fileCount.value > 1
    ? t('comfy.progress', { current: Math.min(completedCount.value + 1, fileCount.value), total: fileCount.value })
    : '';
  return [position, stageLabel.value].filter(Boolean).join(' · ');
});

// `immediate` matters: the parent mounts this with v-if, so `show` is already true on
// setup and a plain watcher would never fire — leaving the workflow unselected and the
// run button permanently disabled.
watch(
  () => props.show,
  (visible) => {
    if (!visible) return;
    error.value = '';
    success.value = '';
    stage.value = '';
    completedCount.value = 0;
    cancelling.value = false;
    cancelled = false;
    currentPromptId = '';
    const list = workflows.value;
    selectedId.value = list.length > 0 ? String(list[0].id) : '';
  },
  { immediate: true }
);

/// Overwrite every loader's filename with the freshly uploaded one. Loaders are recognised
/// by the same shape the import validator uses (a string `image` input).
function injectImage(workflow: any, name: string) {
  const loaders = Object.values(workflow || {}).filter(
    (node: any) => typeof node?.inputs?.image === 'string'
  );
  if (loaders.length === 0) return false;
  for (const node of loaders) {
    (node as any).inputs.image = name;
  }
  return true;
}

/// True when `err` is the backend reporting the cancel we asked for.
function isCancellation(err: unknown) {
  return cancelled && String((err as Error)?.message || err || '').trim() === 'cancelled';
}

async function processOne(file: any, workflowTemplate: any, serverUrl: string, baseName: string) {
  const promptId = crypto.randomUUID();
  currentPromptId = promptId;

  stage.value = 'uploading';
  const uploaded = await comfyUploadImage(serverUrl, file?.file_path || '');
  if (cancelled) return [];

  const workflow = JSON.parse(JSON.stringify(workflowTemplate));
  if (!injectImage(workflow, uploaded?.name || '')) {
    throw new Error(t('comfy.no_image_input'));
  }

  // Blocks until ComfyUI finishes; the backend polls /history for us.
  stage.value = 'running';
  const result = await comfyRunWorkflow(serverUrl, workflow, promptId);
  if (cancelled) return [];

  stage.value = 'downloading';
  const fileIds: string[] = [];
  const tempPaths: string[] = [];
  try {
    for (const image of result?.images || []) {
      if (cancelled) return fileIds;
      const extension = String(image?.filename || '').split('.').pop() || 'png';
      // Must use an app-owned prefix: delete_temp_file rejects anything that is not
      // picaipic_/print_layout_, and cleanup_stale_temp_files only sweeps those, so a
      // bespoke prefix would leave every output leaked in the temp directory.
      const dest = await tempFilePath('picaipic', extension);
      tempPaths.push(dest);
      await comfyDownloadOutput(serverUrl, image, dest);

      stage.value = 'importing';
      importSerial += 1;
      // Import under a readable workflow-derived name, not the staging temp name.
      const imported = await importFile(
        dest,
        props.destination!.folderId,
        props.destination!.folderPath,
        `${baseName}_${importSerial}.${extension}`
      );
      if (imported?.id) fileIds.push(String(imported.id));
    }
  } finally {
    // Best effort; cleanup_stale_temp_files is the backstop for anything missed.
    for (const path of tempPaths) {
      try {
        await deleteTempFile(path);
      } catch {
        /* ignore */
      }
    }
  }
  return fileIds;
}

async function run() {
  const workflowEntry = workflows.value.find((wf: any) => String(wf.id) === String(selectedId.value));
  if (!workflowEntry) return;
  if (!props.destination) {
    error.value = t('comfy.need_album');
    return;
  }

  busy.value = true;
  error.value = '';
  success.value = '';
  completedCount.value = 0;
  cancelled = false;
  cancelling.value = false;

  const serverUrl = config.comfy?.serverUrl || '';
  const baseName = readableWorkflowStem(workflowEntry.name);
  importSerial = 0;
  const importedIds: string[] = [];
  let failed = 0;
  let failures: string[] = [];

  try {
    // Strictly one at a time: a consumer GPU queueing several runs only makes them all
    // slower and risks VRAM exhaustion, and it keeps a cancel meaningful.
    for (let index = 0; index < files.value.length; index += 1) {
      if (cancelled) break;
      try {
        const ids = await processOne(files.value[index], workflowEntry.workflow || {}, serverUrl, baseName);
        importedIds.push(...ids);
      } catch (err: any) {
        if (isCancellation(err)) break;
        failed += 1;
        failures.push(String(err?.message || err || ''));
      }
      if (!cancelled) completedCount.value += 1;
      // Wait before the next image, but never after the last one.
      if (!cancelled && index < files.value.length - 1) {
        await cooldown();
      }
    }

    if (cancelled) {
      // Keep whatever finished before the cancel: those images are already in the library.
      if (importedIds.length > 0) {
        emit('imported', { albumId: props.destination.albumId, fileIds: importedIds });
      }
      toast.info(t('comfy.run_cancelled'));
      close();
      return;
    }

    if (importedIds.length === 0) {
      error.value = failures[0] || t('comfy.no_output');
      return;
    }

    success.value = failed > 0
      ? t('comfy.run_partial', { ok: importedIds.length, failed })
      : t('comfy.run_success', { count: importedIds.length });
    if (failed > 0) toast.warning(success.value);
    else toast.success(success.value);

    emit('imported', { albumId: props.destination.albumId, fileIds: importedIds });
  } finally {
    busy.value = false;
    stage.value = '';
    cancelling.value = false;
    currentPromptId = '';
  }
}

async function cancel() {
  if (cancelling.value) return;
  cancelling.value = true;
  cancelled = true;
  try {
    // The backend bails out of its polling loop and interrupts ComfyUI, which is what
    // releases the still-pending comfyRunWorkflow promise.
    if (currentPromptId) {
      await comfyCancelRun(config.comfy?.serverUrl || '', currentPromptId);
    }
  } catch {
    /* the run may have finished on its own; the flag already stops us */
  }
}

function close() {
  if (busy.value) return;
  emit('close');
}
</script>
