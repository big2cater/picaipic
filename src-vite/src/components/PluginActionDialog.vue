<template>
  <div class="fixed inset-0 z-700 flex items-center justify-center bg-black/30 backdrop-blur-sm">
    <div class="w-[min(520px,calc(100vw-2rem))] max-h-[calc(100vh-2rem)] overflow-hidden rounded-box border border-base-content/20 bg-base-200 shadow-xl">
      <div class="flex items-center justify-between gap-3 border-b border-base-content/10 px-4 py-3">
        <div class="min-w-0">
          <div class="truncate text-sm font-semibold text-base-content/80">{{ title }}</div>
          <div class="truncate text-xs text-base-content/40">{{ plugin?.name || plugin?.id }}</div>
        </div>
        <button class="btn btn-xs btn-ghost" :disabled="busy" @click="$emit('cancel')">×</button>
      </div>

      <div class="max-h-[calc(100vh-10rem)] overflow-auto px-4 py-3 space-y-4">
        <div v-if="imageInputs.length > 0" class="space-y-2">
          <div class="text-[10px] uppercase tracking-widest text-base-content/30">Inputs</div>
          <div
            v-for="input in imageInputs"
            :key="input.id"
            class="flex items-center gap-2"
          >
            <div class="w-24 shrink-0 truncate text-xs text-base-content/50" :title="input.id">
              {{ inputLabel(input) }}
            </div>
            <button
              class="btn btn-xs btn-ghost min-w-0 flex-1 justify-start rounded-box border border-base-content/10 bg-base-100/60 text-base-content/60"
              :disabled="busy || input.locked"
              @click="chooseImageInput(input.id)"
            >
              <span class="truncate">{{ shortPath(inputValues[input.id]) || 'Choose image' }}</span>
            </button>
          </div>
        </div>

        <div v-if="parameterEntries.length > 0" class="space-y-2">
          <div class="text-[10px] uppercase tracking-widest text-base-content/30">Parameters</div>
          <label
            v-for="entry in parameterEntries"
            :key="entry.name"
            class="grid grid-cols-[6.5rem_minmax(0,1fr)] items-center gap-2"
          >
            <span class="truncate text-xs text-base-content/50" :title="entry.name">{{ entry.name }}</span>

            <select
              v-if="entry.schema.enum"
              v-model="parameterValues[entry.name]"
              class="select select-bordered select-xs w-full"
              :disabled="busy"
            >
              <option
                v-for="option in entry.schema.enum"
                :key="String(option)"
                :value="option"
              >
                {{ parameterOptionLabel(entry.name, option, entry.schema.enum) }}
              </option>
            </select>

            <input
              v-else-if="entry.schema.type === 'integer' || entry.schema.type === 'number'"
              v-model.number="parameterValues[entry.name]"
              class="input input-bordered input-xs w-full"
              type="number"
              :min="entry.schema.minimum"
              :max="entry.schema.maximum"
              :step="entry.schema.type === 'integer' ? 1 : 'any'"
              :disabled="busy"
            />

            <input
              v-else-if="entry.schema.type === 'boolean'"
              v-model="parameterValues[entry.name]"
              class="checkbox checkbox-sm"
              type="checkbox"
              :disabled="busy"
            />

            <input
              v-else
              v-model="parameterValues[entry.name]"
              class="input input-bordered input-xs w-full"
              type="text"
              :disabled="busy"
            />

            <div
              v-if="parameterHint(entry)"
              class="col-start-2 -mt-1 text-[11px] leading-4 text-base-content/30"
            >
              {{ parameterHint(entry) }}
            </div>
          </label>
        </div>

        <div v-if="error" class="space-y-2 rounded-box border border-error/20 bg-error/10 px-3 py-2 text-xs text-error">
          <div class="flex flex-wrap items-center gap-1.5">
            <span
              v-if="errorDomain"
              class="rounded-box border px-1.5 py-0.5 text-[10px] uppercase tracking-wide"
              :class="errorDomainClass"
            >
              {{ errorDomainLabel }}
            </span>
            <span
              v-if="errorCode"
              class="rounded-box border border-error/20 bg-error/10 px-1.5 py-0.5 text-[10px] font-mono"
            >
              {{ errorCode }}
            </span>
          </div>
          <div class="leading-5">{{ error }}</div>
          <details v-if="errorDetails" class="rounded-box bg-base-300/50 px-2 py-1 text-base-content/60">
            <summary class="cursor-pointer text-[11px] text-base-content/50">Error details</summary>
            <pre class="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-all text-[11px] leading-4">{{ formatJson(errorDetails) }}</pre>
          </details>
        </div>

        <div v-if="busy && activityLabel" class="space-y-1.5 rounded-box border border-base-content/10 bg-base-300/40 px-3 py-2">
          <div class="flex items-center justify-between gap-2">
            <span class="text-xs font-medium text-base-content/60">{{ activityLabel }}</span>
            <span v-if="taskProgressPercent > 0" class="text-xs text-base-content/40">{{ taskProgressPercent }}%</span>
          </div>
          <progress
            v-if="taskIsActive"
            class="progress progress-primary w-full"
            :value="taskProgressPercent"
            max="100"
          ></progress>
          <div v-if="taskMessage" class="text-[11px] leading-4 text-base-content/40 truncate" :title="taskMessage">
            {{ taskMessage }}
          </div>
        </div>

        <div v-if="diagnostics || logs?.length" class="space-y-2">
          <div class="text-[10px] uppercase tracking-widest text-base-content/30">Diagnostics</div>
          <pre
            v-if="diagnostics"
            class="max-h-40 overflow-auto rounded-box bg-base-300/70 p-2 text-[11px] leading-4 text-base-content/60 whitespace-pre-wrap break-all"
          >{{ formatJson(diagnostics) }}</pre>
          <div v-if="logs?.length" class="space-y-1">
            <details
              v-for="file in logs"
              :key="file.path || file.name"
              class="rounded-box bg-base-300/70 px-2 py-1"
            >
              <summary class="cursor-pointer text-xs text-base-content/50">
                {{ file.name || file.path }}
              </summary>
              <pre class="mt-1 max-h-36 overflow-auto text-[11px] leading-4 text-base-content/60 whitespace-pre-wrap break-all">{{ file.content || '(empty)' }}</pre>
            </details>
          </div>
        </div>
      </div>

      <div class="flex justify-end gap-2 border-t border-base-content/10 px-4 py-3">
        <button class="btn btn-xs btn-ghost" :disabled="busy" @click="$emit('cancel')">Cancel</button>
        <button
          v-if="busy && taskIsActive"
          class="btn btn-xs btn-warning btn-outline"
          @click="$emit('cancel-task')"
        >
          Cancel Task
        </button>
        <button class="btn btn-xs btn-primary" :disabled="busy" @click="submit">
          <span v-if="busy" class="loading loading-spinner loading-xs"></span>
          <span>Run</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import {
  AiPluginHostEnvironment,
  deviceOptionLabel,
  devicePreferenceHint,
} from '@/common/pluginRuntime';

const props = defineProps<{
  plugin: any;
  capability: any;
  sourceFile: any;
  busy?: boolean;
  error?: string;
  errorCode?: string;
  errorDomain?: string;
  errorDetails?: any;
  diagnostics?: any;
  logs?: any[];
  hostEnvironment?: AiPluginHostEnvironment | null;
  stage?: string;
  taskStatus?: string;
  taskProgress?: number;
  taskMessage?: string;
}>();

const emit = defineEmits(['run', 'cancel', 'cancel-task']);

const inputValues = reactive<Record<string, string>>({});
const parameterValues = reactive<Record<string, any>>({});
const localError = ref('');
const error = computed(() => props.error || localError.value);
const errorCode = computed(() => String(props.errorCode || '').trim());
const errorDomain = computed(() => String(props.errorDomain || '').trim());
const errorDetails = computed(() => props.errorDetails || null);

const errorDomainLabel = computed(() => {
  const domain = errorDomain.value;
  const labels: Record<string, string> = {
    transport: 'Transport',
    plugin: 'Plugin',
    runtime: 'Runtime',
    device_backend: 'Device',
    filesystem: 'Filesystem',
    task: 'Task',
    host: 'Host',
  };
  return labels[domain] || domain;
});

const errorDomainClass = computed(() => {
  const domain = errorDomain.value;
  if (domain === 'device_backend' || domain === 'runtime') return 'border-warning/30 bg-warning/10 text-warning';
  if (domain === 'filesystem') return 'border-info/30 bg-info/10 text-info';
  if (domain === 'transport') return 'border-error/30 bg-error/10 text-error';
  return 'border-error/20 bg-error/10 text-error';
});

const title = computed(() => props.capability?.name || props.capability?.id || 'Plugin Action');
const inputs = computed(() => Array.isArray(props.capability?.inputs) ? props.capability.inputs : []);

const stageLabel = computed(() => {
  const stage = String(props.stage || '').toLowerCase();
  const labels: Record<string, string> = {
    starting: 'Starting plugin',
    invoking: 'Invoking capability',
    queued: 'Queued',
    running: 'Running',
    importing: 'Importing result',
    cancelling: 'Cancelling',
    timedout: 'Timed out',
    failed: 'Failed',
    completed: 'Completed',
  };
  return labels[stage] || '';
});

const taskStatusLabel = computed(() => {
  const status = String(props.taskStatus || '').toLowerCase();
  const labels: Record<string, string> = {
    queued: 'Queued',
    running: 'Running',
    cancelling: 'Cancelling',
    succeeded: 'Done',
    failed: 'Failed',
    cancelled: 'Cancelled',
    canceled: 'Cancelled',
  };
  return labels[status] || status || '';
});

const activityLabel = computed(() => stageLabel.value || taskStatusLabel.value);

const taskIsActive = computed(() => {
  const stage = String(props.stage || '').toLowerCase();
  const status = String(props.taskStatus || '').toLowerCase();
  return ['starting', 'invoking', 'queued', 'running', 'importing', 'cancelling'].includes(stage)
    || ['queued', 'running', 'cancelling'].includes(status);
});

const taskProgressPercent = computed(() => Math.max(0, Math.min(100, Number(props.taskProgress || 0))));
const imageInputs = computed(() =>
  inputs.value
    .filter((input: any) => input?.kind === 'image')
    .map((input: any, index: number) => ({
      ...input,
      locked: index === 0 && Boolean(props.sourceFile?.file_path),
    }))
);
const parameterSchema = computed(() => props.capability?.parameters?.properties || {});
const parameterEntries = computed(() =>
  Object.entries(parameterSchema.value).map(([name, schema]) => ({ name, schema: schema as any }))
);

watch(
  () => props.capability,
  () => resetForm(),
  { immediate: true }
);

function resetForm() {
  localError.value = '';
  for (const key of Object.keys(inputValues)) delete inputValues[key];
  for (const key of Object.keys(parameterValues)) delete parameterValues[key];

  const firstImageInput = imageInputs.value[0];
  if (firstImageInput?.id && props.sourceFile?.file_path) {
    inputValues[firstImageInput.id] = props.sourceFile.file_path;
  }

  for (const entry of parameterEntries.value) {
    const schema = entry.schema || {};
    if (schema.default !== undefined) {
      parameterValues[entry.name] = schema.default;
    } else if (schema.enum?.length) {
      parameterValues[entry.name] = schema.enum[0];
    } else if (schema.type === 'boolean') {
      parameterValues[entry.name] = false;
    } else if (schema.type === 'integer' || schema.type === 'number') {
      parameterValues[entry.name] = schema.minimum ?? 0;
    } else {
      parameterValues[entry.name] = '';
    }
  }
}

function inputLabel(input: any) {
  return input?.name || input?.id || 'image';
}

function shortPath(path: string) {
  if (!path) return '';
  const normalized = String(path).replaceAll('\\', '/');
  return normalized.split('/').pop() || path;
}

function parameterOptionLabel(name: string, option: any, options: any[]) {
  if (String(name).toLowerCase() !== 'device') return String(option);
  return deviceOptionLabel(String(option), props.hostEnvironment, options.map((item) => String(item)));
}

function parameterHint(entry: any) {
  if (String(entry?.name || '').toLowerCase() !== 'device') return '';
  if (!Array.isArray(entry?.schema?.enum)) return '';
  return devicePreferenceHint(props.hostEnvironment, entry.schema.enum.map((item: any) => String(item)));
}

async function chooseImageInput(inputId: string) {
  const selected = await openDialog({
    title: 'Choose image',
    multiple: false,
    filters: [
      { name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp', 'tif', 'tiff'] },
    ],
  });
  if (!selected || Array.isArray(selected)) return;
  inputValues[inputId] = selected;
}

function submit() {
  localError.value = '';
  const missing = inputs.value.find((input: any) =>
    input?.required && input?.kind === 'image' && !inputValues[input.id]
  );
  if (missing) {
    localError.value = `Missing input: ${missing.id}`;
    return;
  }

  const payloadInputs: Record<string, any> = {};
  for (const [id, path] of Object.entries(inputValues)) {
    if (path) payloadInputs[id] = { path };
  }

  emit('run', {
    inputs: payloadInputs,
    parameters: { ...parameterValues },
  });
}

function formatJson(value: any) {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
</script>
