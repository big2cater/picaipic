<template>
  <ModalDialog :title="$t('print_layout.title')" :width="980" :height="680" @cancel="onCancel">
    <div class="flex gap-3 text-sm select-none min-h-0 h-full">
      <!-- Left: photo bins -->
      <div class="w-[200px] shrink-0 flex flex-col gap-2 min-h-0">
        <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
          {{ $t('print_layout.add_photos') }}
        </div>
        <div class="flex-1 min-h-0 overflow-auto space-y-2">
          <div
            v-for="group in photoGroups"
            :key="group.photoId"
            class="rounded-box border border-base-content/10 p-2 space-y-1.5"
            :class="activePhotoId === group.photoId ? 'border-primary/50 bg-primary/5' : ''"
          >
            <div class="flex items-center justify-between gap-1">
              <button type="button" class="text-xs font-medium truncate text-left flex-1" @click="activePhotoId = group.photoId">
                {{ photoLabel(group.photoId) }}
              </button>
              <button type="button" class="btn btn-ghost btn-xs" @click="addFilesToGroup(group.photoId)">+ {{ $t('print_layout.add') }}</button>
            </div>
            <div class="grid grid-cols-3 gap-1">
              <div
                v-for="(f, idx) in group.files"
                :key="f.file_path + idx"
                class="relative h-12 rounded overflow-hidden border border-base-content/10 bg-base-content/5"
                :title="f.name || f.file_path"
              >
                <img v-if="f.thumbnail" :src="f.thumbnail" class="h-full w-full object-cover" />
                <button type="button" class="absolute top-0 right-0 badge badge-xs" @click="removeFromGroup(group.photoId, idx)">×</button>
              </div>
            </div>
            <div class="text-[10px] opacity-50">{{ group.files.length }} / {{ cellsForPhoto(group.photoId) }}</div>
          </div>
          <div v-if="!photoGroups.length" class="text-xs text-base-content/40 py-6 text-center">
            {{ $t('print_layout.pick_layout_first') }}
          </div>
        </div>
      </div>

      <!-- Center preview -->
      <div class="flex-1 min-w-0 min-h-0 flex flex-col gap-2">
        <div class="flex-1 min-h-0 flex items-center justify-center rounded-box border border-base-content/10 bg-base-300/40 p-3 overflow-auto">
          <div
            class="relative shadow-lg"
            :style="previewSheetStyle"
          >
            <div
              v-for="(cell, idx) in previewCells"
              :key="idx"
              class="absolute overflow-hidden"
              :style="cellStyle(cell)"
            >
              <img
                v-if="cell.file?.thumbnail || cell.file?.file_path"
                :src="cell.file.thumbnail || ''"
                class="h-full w-full object-cover"
              />
              <div v-else class="h-full w-full bg-base-content/5" />
            </div>
            <div
              v-if="!previewCells.length"
              class="absolute inset-0 flex items-center justify-center text-xs text-base-content/40 px-6 text-center"
            >
              {{ $t('print_layout.preview_hint') }}
            </div>
          </div>
        </div>
        <div class="text-[11px] text-base-content/40 text-center">
          {{ plan.paperPxW }}×{{ plan.paperPxH }}px · {{ dpi }} DPI · {{ plan.placed }}/{{ plan.capacity }}
          <span v-if="plan.utilization != null"> · {{ Math.round((plan.utilization || 0) * 100) }}%</span>
          <span v-if="plan.strategy && plan.strategy !== 'uniform'"> · {{ strategyLabel(plan.strategy) }}</span>
        </div>
      </div>

      <!-- Right: styles -->
      <div class="w-[240px] shrink-0 flex flex-col gap-2 min-h-0 overflow-auto">
        <div class="text-[11px] font-bold uppercase tracking-wider text-base-content/30">
          {{ $t('print_layout.styles') }}
        </div>
        <div class="rounded-box border border-base-content/10 max-h-[280px] overflow-auto">
          <button
            v-for="layout in layouts"
            :key="layout.id"
            type="button"
            class="w-full text-left px-2 py-1.5 text-xs border-b border-base-content/5 hover:bg-base-100/30"
            :class="selectedLayoutId === layout.id ? 'bg-primary/15 text-primary' : ''"
            @click="selectLayout(layout.id)"
          >
            {{ layoutLabel(layout) }}
          </button>
        </div>
        <div class="flex flex-wrap gap-1.5">
          <button type="button" class="t-button-default text-xs" @click="showCustomDialog = true">
            + {{ $t('print_layout.add_custom') }}
          </button>
          <button type="button" class="t-button-default text-xs" :disabled="!canDeleteLayout" @click="deleteSelectedLayout">
            {{ $t('print_layout.delete_layout') }}
          </button>
        </div>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('print_layout.dpi') }}</span>
          <input v-model.number="dpi" type="number" min="72" max="600" class="input input-bordered input-sm" />
        </label>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('print_layout.background') }}</span>
          <div class="flex gap-2 items-center">
            <input v-model="background" type="color" class="h-8 w-12 cursor-pointer rounded border border-base-content/15 bg-transparent" />
            <input v-model="background" type="text" class="input input-bordered input-sm flex-1" />
          </div>
        </label>
        <label class="flex items-center gap-2 text-xs">
          <input v-model="showGuides" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
          {{ $t('print_layout.show_guides') }}
        </label>
        <button type="button" class="t-button-default text-xs" @click="showPaperManage = true">
          {{ $t('print_layout.paper_manage') }}
        </button>
        <label class="flex items-center gap-2 text-xs">
          <input v-model="importToLibrary" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
          {{ $t('print_layout.import_to_library') }}
        </label>
        <p v-if="errorMessage" class="text-error text-xs">{{ errorMessage }}</p>
      </div>
    </div>

    <div class="flex justify-end gap-2 pt-3 shrink-0">
      <button class="t-button-default" :disabled="isProcessing" @click="onCancel">{{ $t('msgbox.cancel') }}</button>
      <button class="t-button-default" :disabled="isProcessing || !plan.cells.length" @click="doPrint">
        {{ isProcessing && processingMode === 'print' ? $t('print_layout.printing') : $t('print_layout.print') }}
      </button>
      <button class="t-button-primary" :disabled="isProcessing || !plan.cells.length" @click="doExport">
        {{ isProcessing && processingMode === 'export' ? $t('print_layout.exporting') : $t('print_layout.export') }}
      </button>
    </div>
  </ModalDialog>

  <!-- Paper manage -->
  <ModalDialog
    v-if="showPaperManage"
    :title="$t('print_layout.paper_manage_title')"
    :width="560"
    @cancel="showPaperManage = false"
  >
    <div class="space-y-2 text-sm select-none">
      <div class="overflow-auto max-h-[50vh] rounded-box border border-base-content/10">
        <table class="table table-xs w-full">
          <thead>
            <tr class="text-[10px] uppercase text-base-content/40">
              <th>{{ $t('print_layout.paper_name') }}</th>
              <th>{{ $t('print_layout.paper_inch') }}</th>
              <th>{{ $t('print_layout.paper_cm') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="p in papers"
              :key="p.id"
              class="cursor-pointer"
              :class="selectedPaperId === p.id ? 'bg-primary/15' : ''"
              @click="selectedPaperId = p.id"
            >
              <td>{{ paperLabel(p) }}</td>
              <td class="opacity-70">{{ p.inchW.toFixed(2) }} × {{ p.inchH.toFixed(2) }}</td>
              <td class="opacity-70">{{ (p.inchW * 2.54).toFixed(2) }} × {{ (p.inchH * 2.54).toFixed(2) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="flex justify-end gap-2">
        <button class="t-button-default" @click="showPaperManage = false">{{ $t('msgbox.cancel') }}</button>
        <button class="t-button-default" :disabled="!canDeletePaper" @click="deleteSelectedPaper">{{ $t('print_layout.delete_paper') }}</button>
        <button class="t-button-primary" @click="showAddPaperDialog = true">{{ $t('print_layout.add_paper') }}</button>
      </div>
    </div>
  </ModalDialog>

  <AddCustomPaperDialog
    v-if="showAddPaperDialog"
    :existing="papers"
    @cancel="showAddPaperDialog = false"
    @ok="onCustomPaperOk"
  />

  <!-- Custom layout -->
  <ModalDialog
    v-if="showCustomDialog"
    :title="$t('print_layout.custom_title')"
    :width="860"
    :height="560"
    @cancel="showCustomDialog = false"
  >
    <div class="flex gap-3 text-sm select-none min-h-0 h-full">
      <div class="flex-1 min-w-0 rounded-box border border-base-content/10 bg-base-300/30 flex items-center justify-center p-4">
        <div class="relative shadow" :style="customPreviewStyle">
          <div
            v-for="(cell, idx) in customPreviewCells"
            :key="idx"
            class="absolute border border-base-content/20 bg-base-100/80"
            :style="cellStyle(cell, customPreviewPlan.paperPxW, customPreviewPlan.paperPxH)"
          />
        </div>
      </div>
      <div class="w-[260px] shrink-0 space-y-2 overflow-auto">
        <div class="flex items-center justify-between">
          <span class="text-xs opacity-70">{{ $t('print_layout.paper_spec') }}</span>
          <button type="button" class="btn btn-ghost btn-xs" @click="showPaperManage = true">{{ $t('print_layout.paper_manage') }}</button>
        </div>
        <select v-model="customPaperId" class="select select-bordered select-sm w-full">
          <option v-for="p in papers" :key="p.id" :value="p.id">{{ paperLabel(p) }}</option>
        </select>
        <div class="flex gap-1">
          <button type="button" class="btn btn-sm flex-1" :class="customPaperOrientation==='landscape'?'btn-primary':''" @click="customPaperOrientation='landscape'">{{ $t('print_layout.landscape') }}</button>
          <button type="button" class="btn btn-sm flex-1" :class="customPaperOrientation==='portrait'?'btn-primary':''" @click="customPaperOrientation='portrait'">{{ $t('print_layout.portrait') }}</button>
        </div>

        <div class="text-xs opacity-70 pt-1">{{ $t('print_layout.photo_spec') }} 1</div>
        <select v-model="customSlot1Id" class="select select-bordered select-sm w-full">
          <option value="">{{ $t('print_layout.none') }}</option>
          <option v-for="p in photoPresets" :key="p.id" :value="p.id">{{ photoLabel(p.id) }}</option>
        </select>
        <div class="flex gap-1">
          <button type="button" class="btn btn-xs flex-1" :class="customSlot1Orient==='landscape'?'btn-primary':''" @click="customSlot1Orient='landscape'">{{ $t('print_layout.landscape') }}</button>
          <button type="button" class="btn btn-xs flex-1" :class="customSlot1Orient==='portrait'?'btn-primary':''" @click="customSlot1Orient='portrait'">{{ $t('print_layout.portrait') }}</button>
        </div>
        <label class="flex items-center gap-2 text-xs">
          {{ $t('print_layout.count') }}
          <input v-model.number="customSlot1Count" type="number" min="0" max="64" class="input input-bordered input-xs w-16" />
        </label>

        <div class="text-xs opacity-70 pt-1">{{ $t('print_layout.photo_spec') }} 2</div>
        <select v-model="customSlot2Id" class="select select-bordered select-sm w-full">
          <option value="">{{ $t('print_layout.none') }}</option>
          <option v-for="p in photoPresets" :key="p.id" :value="p.id">{{ photoLabel(p.id) }}</option>
        </select>
        <div class="flex gap-1">
          <button type="button" class="btn btn-xs flex-1" :class="customSlot2Orient==='landscape'?'btn-primary':''" @click="customSlot2Orient='landscape'">{{ $t('print_layout.landscape') }}</button>
          <button type="button" class="btn btn-xs flex-1" :class="customSlot2Orient==='portrait'?'btn-primary':''" @click="customSlot2Orient='portrait'">{{ $t('print_layout.portrait') }}</button>
        </div>
        <label class="flex items-center gap-2 text-xs">
          {{ $t('print_layout.count') }}
          <input v-model.number="customSlot2Count" type="number" min="0" max="64" class="input input-bordered input-xs w-16" />
        </label>

        <label class="flex items-center gap-2 text-xs">
          {{ $t('print_layout.gap_x') }}
          <input v-model.number="customGapX" type="number" min="0" step="0.05" class="input input-bordered input-xs w-20" />
          cm
        </label>
        <label class="flex items-center gap-2 text-xs">
          {{ $t('print_layout.gap_y') }}
          <input v-model.number="customGapY" type="number" min="0" step="0.05" class="input input-bordered input-xs w-20" />
          cm
        </label>

        <div class="text-xs opacity-70 pt-1">{{ $t('print_layout.pack_strategy') }}</div>
        <select v-model="customStrategy" class="select select-bordered select-sm w-full">
          <option value="auto">{{ $t('print_layout.strategy_auto') }}</option>
          <option value="h-bands">{{ $t('print_layout.strategy_h') }}</option>
          <option value="v-bands">{{ $t('print_layout.strategy_v') }}</option>
        </select>

        <label class="form-control w-full">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('print_layout.layout_name') }}</span>
          <input
            v-model="customLayoutName"
            type="text"
            maxlength="48"
            class="input input-bordered input-sm w-full"
            :placeholder="$t('print_layout.layout_custom_default')"
          />
        </label>

        <div class="flex justify-end gap-2 pt-2">
          <button class="t-button-default" @click="showCustomDialog = false">{{ $t('msgbox.cancel') }}</button>
          <button class="t-button-primary" @click="saveCustomLayout">{{ $t('msgbox.ok') }}</button>
        </div>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch, type CSSProperties } from 'vue';
import { useI18n } from 'vue-i18n';
import { open as openDialog, save } from '@tauri-apps/plugin-dialog';
import ModalDialog from '@/components/ModalDialog.vue';
import AddCustomPaperDialog from '@/components/AddCustomPaperDialog.vue';
import {
  cleanupStaleTempFiles,
  deleteTempFile,
  exportPrintLayout,
  tempFilePath,
} from '@/common/api';
import { config } from '@/common/config';
import { BUILTIN_PHOTO_SIZE_PRESETS } from '@/common/photoSizePresets';
import {
  allLayouts,
  allPapers,
  buildLayoutPlan,
  createCustomId,
  findLayout,
  findPaper,
  normalizePaperSizes,
  normalizePrintLayouts,
  type LayoutCell,
  type PackStrategy,
  type PaperOrientation,
  type PaperSizeSpec,
  type PhotoOrientation,
  type PrintLayoutPreset,
} from '@/common/printLayout';
import { useToast } from '@/common/toast';
import { getAssetSrc } from '@/common/utils';

type PrintFile = {
  file_path: string;
  name?: string;
  thumbnail?: string;
  file_type?: number | null;
};

export type PrintLayoutDonePayload = {
  path: string;
  importToLibrary: boolean;
};

const props = defineProps<{ files: PrintFile[] }>();
const emit = defineEmits<{ cancel: []; done: [PrintLayoutDonePayload] }>();
const { t } = useI18n();
const toast = useToast();

/** One-shot defaults only. Never call from computed — mutating pinia there freezes the UI. */
function ensureConfigOnce() {
  if (!(config as any).printLayout || typeof (config as any).printLayout !== 'object') {
    (config as any).printLayout = {
      customPapers: [],
      customLayouts: [],
      dpi: 300,
      background: '#ffffff',
      showGuides: true,
      importToLibrary: false,
    };
    return;
  }
  const pl = (config as any).printLayout;
  if (!Array.isArray(pl.customPapers)) pl.customPapers = [];
  if (!Array.isArray(pl.customLayouts)) pl.customLayouts = [];
  if (!pl.dpi) pl.dpi = 300;
  if (!pl.background) pl.background = '#ffffff';
  if (typeof pl.importToLibrary !== 'boolean') pl.importToLibrary = false;
}
ensureConfigOnce();

const customPapersTick = ref(0);
const customLayoutsTick = ref(0);
const customPapers = computed<PaperSizeSpec[]>(() => {
  customPapersTick.value;
  return normalizePaperSizes((config as any).printLayout?.customPapers);
});
const customLayouts = computed<PrintLayoutPreset[]>(() => {
  customLayoutsTick.value;
  return normalizePrintLayouts((config as any).printLayout?.customLayouts);
});

const papers = computed(() => allPapers(customPapers.value));
const layouts = computed(() => allLayouts(customLayouts.value));
const photoPresets = BUILTIN_PHOTO_SIZE_PRESETS;

const selectedLayoutId = ref(layouts.value[0]?.id || '');
const dpi = ref(Number((config as any).printLayout?.dpi) || 300);
const background = ref(String((config as any).printLayout?.background || '#ffffff'));
const showGuides = ref(Boolean((config as any).printLayout?.showGuides !== false));
const importToLibrary = ref(Boolean((config as any).printLayout?.importToLibrary));
const isProcessing = ref(false);
const processingMode = ref<'export' | 'print' | ''>('');
const errorMessage = ref('');
const showPaperManage = ref(false);
const showAddPaperDialog = ref(false);
const showCustomDialog = ref(false);
const selectedPaperId = ref('');
const activePhotoId = ref('');

// files assigned per photo size id
const filesByPhotoId = ref<Record<string, PrintFile[]>>({});

// seed from selection into first slot after layout select
function selectLayout(id: string) {
  selectedLayoutId.value = id;
  const layout = findLayout(id, customLayouts.value);
  if (!layout) return;
  // init empty groups
  const next: Record<string, PrintFile[]> = { ...filesByPhotoId.value };
  for (const s of layout.slots) {
    if (!next[s.photoId]) next[s.photoId] = [];
  }
  // if empty, dump selection into first slot
  const first = layout.slots[0]?.photoId;
  if (first && (!next[first] || next[first].length === 0) && props.files?.length) {
    next[first] = props.files.map((f) => ({ ...f, file_path: String(f.file_path || '') })).filter((f) => f.file_path);
  }
  filesByPhotoId.value = next;
  activePhotoId.value = first || '';
}
selectLayout(selectedLayoutId.value);

const selectedLayout = computed(() => findLayout(selectedLayoutId.value, customLayouts.value));
const selectedPaper = computed(() => {
  const layout = selectedLayout.value;
  if (!layout) return papers.value[0];
  return findPaper(layout.paperId, customPapers.value) || papers.value[0];
});

const plan = computed(() => {
  const layout = selectedLayout.value;
  const paper = selectedPaper.value;
  if (!layout || !paper) {
    return { paperPxW: 800, paperPxH: 600, cells: [] as LayoutCell[], placed: 0, capacity: 0 };
  }
  return buildLayoutPlan({
    paper,
    paperOrientation: layout.paperOrientation,
    slots: layout.slots,
    gapXcm: layout.gapXcm,
    gapYcm: layout.gapYcm,
    dpi: dpi.value,
  });
});

const photoGroups = computed(() => {
  const layout = selectedLayout.value;
  if (!layout) return [] as { photoId: string; files: PrintFile[] }[];
  const ids = [...new Set(layout.slots.map((s) => s.photoId))];
  return ids.map((photoId) => ({ photoId, files: filesByPhotoId.value[photoId] || [] }));
});

function cellsForPhoto(photoId: string) {
  return plan.value.cells.filter((c) => c.photoId === photoId).length;
}

const previewCells = computed(() => {
  const used: Record<string, number> = {};
  return plan.value.cells.map((cell) => {
    const list = filesByPhotoId.value[cell.photoId] || [];
    const idx = used[cell.photoId] || 0;
    used[cell.photoId] = idx + 1;
    // cycle files if fewer than cells
    const file = list.length ? list[idx % list.length] : null;
    return { ...cell, file };
  });
});

const previewSheetStyle = computed((): CSSProperties => {
  const maxW = 520;
  const maxH = 420;
  const w = plan.value.paperPxW || 1;
  const h = plan.value.paperPxH || 1;
  const scale = Math.min(maxW / w, maxH / h, 1);
  return {
    width: `${Math.round(w * scale)}px`,
    height: `${Math.round(h * scale)}px`,
    backgroundColor: background.value,
    outline: showGuides.value ? '1px solid rgba(128,128,128,0.35)' : undefined,
  };
});

function cellStyle(cell: LayoutCell, paperPxW?: number, paperPxH?: number): CSSProperties {
  const w = paperPxW || plan.value.paperPxW || 1;
  const h = paperPxH || plan.value.paperPxH || 1;
  return {
    left: `${(cell.x / w) * 100}%`,
    top: `${(cell.y / h) * 100}%`,
    width: `${(cell.w / w) * 100}%`,
    height: `${(cell.h / h) * 100}%`,
    boxShadow: showGuides.value ? 'inset 0 0 0 1px rgba(120,120,120,0.35)' : undefined,
  };
}

function paperLabel(p: PaperSizeSpec) {
  if (p.nameKey) return t(`print_layout.${p.nameKey}`);
  return p.name || p.id;
}
function photoLabel(id: string) {
  const p = photoPresets.find((x) => x.id === id);
  if (p) return t(`msgbox.image_editor.photo_sizes.${p.nameKey}`);
  return id;
}
function layoutLabel(layout: PrintLayoutPreset) {
  if (layout.nameKey) return t(`print_layout.${layout.nameKey}`);
  return layout.name || layout.id;
}
function strategyLabel(strategy?: string) {
  if (strategy === 'v-bands') return t('print_layout.strategy_v_short');
  if (strategy === 'h-bands') return t('print_layout.strategy_h_short');
  return '';
}

async function addFilesToGroup(photoId: string) {
  const selected = await openDialog({
    multiple: true,
    filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp', 'heic', 'tif', 'tiff'] }],
  });
  if (!selected) return;
  const list = Array.isArray(selected) ? selected : [selected];
  const cur = [...(filesByPhotoId.value[photoId] || [])];
  const existing = new Set(cur.map((f) => f.file_path));
  for (const path of list) {
    const p = String(path);
    if (!p || existing.has(p)) continue;
    cur.push({ file_path: p, name: p.split(/[/\\]/).pop() || p, file_type: 1 });
  }
  filesByPhotoId.value = { ...filesByPhotoId.value, [photoId]: cur };
}
function removeFromGroup(photoId: string, idx: number) {
  const cur = [...(filesByPhotoId.value[photoId] || [])];
  cur.splice(idx, 1);
  filesByPhotoId.value = { ...filesByPhotoId.value, [photoId]: cur };
}

const canDeleteLayout = computed(() => {
  const l = selectedLayout.value;
  return !!l && l.kind === 'custom';
});
const canDeletePaper = computed(() => {
  const p = papers.value.find((x) => x.id === selectedPaperId.value);
  return !!p && p.kind === 'custom';
});

function deleteSelectedLayout() {
  if (!canDeleteLayout.value) return;
  const id = selectedLayoutId.value;
  (config as any).printLayout.customLayouts = customLayouts.value.filter((l) => l.id !== id);
  customLayoutsTick.value += 1;
  selectedLayoutId.value = layouts.value[0]?.id || '';
  selectLayout(selectedLayoutId.value);
}
function deleteSelectedPaper() {
  if (!canDeletePaper.value) return;
  const id = selectedPaperId.value;
  (config as any).printLayout.customPapers = customPapers.value.filter((p) => p.id !== id);
  customPapersTick.value += 1;
  selectedPaperId.value = '';
}

function onCustomPaperOk(paper: PaperSizeSpec) {
  (config as any).printLayout.customPapers = [...customPapers.value, paper];
  customPapersTick.value += 1;
  selectedPaperId.value = paper.id;
  customPaperId.value = paper.id;
  showAddPaperDialog.value = false;
  toast.success(t('print_layout.paper_added'));
}

// custom layout form
const customPaperId = ref(papers.value[0]?.id || 'paper-3r');
const customPaperOrientation = ref<PaperOrientation>('landscape');
const customSlot1Id = ref('photo-1r');
const customSlot1Orient = ref<PhotoOrientation>('portrait');
const customSlot1Count = ref(8);
const customSlot2Id = ref('');
const customSlot2Orient = ref<PhotoOrientation>('portrait');
const customSlot2Count = ref(0);
const customGapX = ref(0.3);
const customGapY = ref(0.3);
const customStrategy = ref<PackStrategy>('auto');
const customLayoutName = ref('');

const customPreviewPlan = computed(() => {
  const paper = findPaper(customPaperId.value, customPapers.value) || papers.value[0];
  if (!paper) return { paperPxW: 600, paperPxH: 400, cells: [] as LayoutCell[], placed: 0, capacity: 0 };
  const slots = [] as PrintLayoutPreset['slots'];
  if (customSlot1Id.value) {
    slots.push({ photoId: customSlot1Id.value, orientation: customSlot1Orient.value, count: customSlot1Count.value });
  }
  if (customSlot2Id.value) {
    slots.push({ photoId: customSlot2Id.value, orientation: customSlot2Orient.value, count: customSlot2Count.value });
  }
  return buildLayoutPlan({
    paper,
    paperOrientation: customPaperOrientation.value,
    slots,
    gapXcm: customGapX.value,
    gapYcm: customGapY.value,
    dpi: dpi.value,
    strategy: customStrategy.value,
  });
});
const customPreviewCells = computed(() => customPreviewPlan.value.cells);
const customPreviewStyle = computed((): CSSProperties => {
  const maxW = 480;
  const maxH = 400;
  const w = customPreviewPlan.value.paperPxW || 1;
  const h = customPreviewPlan.value.paperPxH || 1;
  const scale = Math.min(maxW / w, maxH / h, 1);
  return {
    width: `${Math.round(w * scale)}px`,
    height: `${Math.round(h * scale)}px`,
    backgroundColor: background.value,
  };
});

function saveCustomLayout() {
  if (!customSlot1Id.value && !customSlot2Id.value) {
    toast.warning(t('print_layout.need_photo_spec'));
    return;
  }
  const slots: PrintLayoutPreset['slots'] = [];
  if (customSlot1Id.value) {
    slots.push({ photoId: customSlot1Id.value, orientation: customSlot1Orient.value, count: Math.max(0, customSlot1Count.value || 0) });
  }
  if (customSlot2Id.value) {
    slots.push({ photoId: customSlot2Id.value, orientation: customSlot2Orient.value, count: Math.max(0, customSlot2Count.value || 0) });
  }
  const layout: PrintLayoutPreset = {
    id: createCustomId('layout'),
    kind: 'custom',
    name: String(customLayoutName.value || '').trim() || t('print_layout.layout_custom_default'),
    paperId: customPaperId.value,
    paperOrientation: customPaperOrientation.value,
    gapXcm: Math.max(0, customGapX.value || 0),
    gapYcm: Math.max(0, customGapY.value || 0),
    slots,
  };
  (config as any).printLayout.customLayouts = [layout, ...customLayouts.value];
  customLayoutsTick.value += 1;
  showCustomDialog.value = false;
  customLayoutName.value = '';
  selectLayout(layout.id);
  toast.success(t('print_layout.layout_saved'));
}

function persistUiPrefs() {
  ensureConfigOnce();
  (config as any).printLayout.dpi = dpi.value;
  (config as any).printLayout.background = background.value;
  (config as any).printLayout.showGuides = showGuides.value;
  (config as any).printLayout.importToLibrary = importToLibrary.value;
}

function buildExportCells() {
  const used: Record<string, number> = {};
  return plan.value.cells.map((cell) => {
    const list = filesByPhotoId.value[cell.photoId] || [];
    const idx = used[cell.photoId] || 0;
    used[cell.photoId] = idx + 1;
    const file = list.length ? list[idx % list.length] : null;
    return {
      x: cell.x,
      y: cell.y,
      w: cell.w,
      h: cell.h,
      sourceFilePath: file?.file_path || '',
    };
  }).filter((c) => c.sourceFilePath);
}

function validateReady(): boolean {
  errorMessage.value = '';
  persistUiPrefs();
  if (!plan.value.cells.length) {
    errorMessage.value = t('print_layout.need_layout');
    return false;
  }
  const hasFile = Object.values(filesByPhotoId.value).some((arr) => arr.length > 0);
  if (!hasFile) {
    errorMessage.value = t('print_layout.need_photos');
    return false;
  }
  return true;
}

/**
 * Print uses a *print-sized* sheet (paper aspect, capped long edge), not full export DPI.
 * Waiting on full 300DPI composite before window.print() is why the dialog felt stuck.
 * Export still uses plan DPI. Host still downscales each source to cell pixels.
 */
type PrintSheetCache = {
  fingerprint: string;
  /** blob: URL (preferred) or file path */
  src: string;
  /** optional temp path if blob not used */
  path?: string;
  revoke?: () => void;
};
const printSheetCache = ref<PrintSheetCache | null>(null);
const pendingTempDeletes = new Set<string>();
let printDomCleanupTimer: ReturnType<typeof setTimeout> | null = null;
let prerenderTimer: ReturnType<typeof setTimeout> | null = null;
let prerenderInFlight: Promise<string | null> | null = null;
let prerenderGeneration = 0;

/** Long edge for OS print bitmap — ~200DPI on 6–8" paper, enough for dialog + home print. */
const PRINT_MAX_EDGE = 1800;

function isAppTempPath(path: string): boolean {
  const base = String(path || '').split(/[/\\]/).pop() || '';
  const lower = base.toLowerCase();
  return (
    lower.startsWith('print_layout_')
    || lower.startsWith('picaipic_')
    || lower.startsWith('picaipic-')
  );
}

function fileAssignmentKey(): string {
  // Stable shallow key — avoid deep watch thrashing object identity.
  const parts: string[] = [];
  for (const [photoId, list] of Object.entries(filesByPhotoId.value || {})) {
    parts.push(photoId);
    for (const f of list || []) {
      parts.push(String(f.file_path || ''));
    }
  }
  return parts.join('|');
}

function printFingerprint(cells: ReturnType<typeof buildExportCells>, paperW: number, paperH: number): string {
  return JSON.stringify({
    kind: 'print',
    w: paperW,
    h: paperH,
    bg: background.value || '#ffffff',
    guides: !!showGuides.value,
    layoutId: selectedLayoutId.value,
    files: fileAssignmentKey(),
    cells: cells.map((c) => `${c.x},${c.y},${c.w},${c.h},${c.sourceFilePath}`),
  });
}

/** Scale plan cells to print canvas (preserve layout, cap long edge). */
function buildPrintGeometry() {
  const fullW = Math.max(1, plan.value.paperPxW || 1);
  const fullH = Math.max(1, plan.value.paperPxH || 1);
  const scale = Math.min(1, PRINT_MAX_EDGE / Math.max(fullW, fullH));
  const paperWidth = Math.max(64, Math.round(fullW * scale));
  const paperHeight = Math.max(64, Math.round(fullH * scale));
  const cells = buildExportCells().map((c) => ({
    ...c,
    x: Math.max(0, Math.round(c.x * scale)),
    y: Math.max(0, Math.round(c.y * scale)),
    w: Math.max(2, Math.round(c.w * scale)),
    h: Math.max(2, Math.round(c.h * scale)),
  }));
  return { paperWidth, paperHeight, cells, scale };
}

async function safeDeleteTempPath(path: string | null | undefined) {
  const p = String(path || '').trim();
  if (!p || !isAppTempPath(p)) return;
  try {
    await deleteTempFile(p);
  } catch {
    /* ignore */
  }
}

function revokePrintCache() {
  const prev = printSheetCache.value;
  printSheetCache.value = null;
  if (prev?.revoke) {
    try {
      prev.revoke();
    } catch {
      /* ignore */
    }
  }
  if (prev?.path && isAppTempPath(prev.path)) {
    pendingTempDeletes.add(prev.path);
    window.setTimeout(() => {
      void safeDeleteTempPath(prev.path).finally(() => {
        if (prev.path) pendingTempDeletes.delete(prev.path);
      });
    }, 300);
  }
}

async function clearAllSheetTemps() {
  if (prerenderTimer) {
    clearTimeout(prerenderTimer);
    prerenderTimer = null;
  }
  if (printDomCleanupTimer) {
    clearTimeout(printDomCleanupTimer);
    printDomCleanupTimer = null;
  }
  prerenderGeneration += 1;
  prerenderInFlight = null;
  revokePrintCache();
  const paths = [...pendingTempDeletes];
  pendingTempDeletes.clear();
  await Promise.all(paths.map((p) => safeDeleteTempPath(p)));
  try {
    document.querySelector('.print-only')?.replaceChildren();
  } catch {
    /* ignore */
  }
}

async function renderSheetTo(
  dest: string,
  options?: {
    paperWidth?: number;
    paperHeight?: number;
    cells?: ReturnType<typeof buildExportCells>;
    quality?: number;
    showGuides?: boolean;
  },
) {
  const cells = options?.cells || buildExportCells();
  if (!cells.length) {
    throw new Error(t('print_layout.need_photos'));
  }
  await exportPrintLayout({
    destFilePath: dest,
    outputFormat: String(dest).toLowerCase().endsWith('.png') ? 'png' : 'jpg',
    quality: options?.quality ?? 92,
    paperWidth: options?.paperWidth ?? plan.value.paperPxW,
    paperHeight: options?.paperHeight ?? plan.value.paperPxH,
    background: background.value || '#ffffff',
    showGuides: options?.showGuides ?? showGuides.value,
    guideColor: '#cccccc',
    cells,
  });
  return dest;
}

/** Build (or reuse) a print-sized sheet; much faster than full export DPI. */
async function ensurePrintSheet(): Promise<string> {
  const geometry = buildPrintGeometry();
  if (!geometry.cells.length) {
    throw new Error(t('print_layout.need_photos'));
  }
  const fingerprint = printFingerprint(geometry.cells, geometry.paperWidth, geometry.paperHeight);
  const cached = printSheetCache.value;
  if (cached && cached.fingerprint === fingerprint && cached.src) {
    return cached.src;
  }
  if (prerenderInFlight) {
    const src = await prerenderInFlight;
    if (src && printSheetCache.value?.fingerprint === fingerprint) {
      return src;
    }
  }

  const gen = prerenderGeneration;
  const dest = await tempFilePath('print_layout', 'jpg');
  const work = (async () => {
    await renderSheetTo(String(dest), {
      paperWidth: geometry.paperWidth,
      paperHeight: geometry.paperHeight,
      cells: geometry.cells,
      quality: 85,
      showGuides: showGuides.value,
    });
    if (gen !== prerenderGeneration) {
      await safeDeleteTempPath(String(dest));
      return null;
    }
    // Prefer blob URL so window.print does not wait on asset-protocol disk re-read.
    let src = getAssetSrc(String(dest));
    let revoke: (() => void) | undefined;
    try {
      const bytes = await fetch(src).then((r) => r.arrayBuffer());
      const blob = new Blob([bytes], { type: 'image/jpeg' });
      const blobUrl = URL.createObjectURL(blob);
      revoke = () => URL.revokeObjectURL(blobUrl);
      src = blobUrl;
    } catch {
      // Fall back to asset URL if fetch fails.
    }
    revokePrintCache();
    printSheetCache.value = {
      fingerprint,
      src,
      path: String(dest),
      revoke,
    };
    return src;
  })();
  prerenderInFlight = work.catch(() => null);
  try {
    const src = await work;
    if (!src) return ensurePrintSheet();
    return src;
  } finally {
    prerenderInFlight = null;
  }
}

function schedulePrintPrerender() {
  if (prerenderTimer) {
    clearTimeout(prerenderTimer);
    prerenderTimer = null;
  }
  prerenderTimer = setTimeout(() => {
    prerenderTimer = null;
    if (isProcessing.value) return;
    const hasFile = Object.values(filesByPhotoId.value).some((arr) => arr.length > 0);
    if (!hasFile || !plan.value.cells.length) return;
    void ensurePrintSheet().catch(() => {
      /* ignore warm failures */
    });
  }, 200);
}

function waitForImageLoad(img: HTMLImageElement, timeoutMs = 12000): Promise<void> {
  if (img.complete && img.naturalWidth > 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      reject(new Error(t('print_layout.print_failed')));
    }, timeoutMs);
    img.addEventListener(
      'load',
      () => {
        window.clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
    img.addEventListener(
      'error',
      () => {
        window.clearTimeout(timer);
        reject(new Error(t('print_layout.print_failed')));
      },
      { once: true },
    );
  });
}

/** Open system print as soon as the print-sized bitmap is in the DOM. */
async function printSheetViaWindow(src: string) {
  let host = document.querySelector('.print-only') as HTMLElement | null;
  if (!host) {
    host = document.createElement('div');
    host.className = 'print-only';
    document.body.appendChild(host);
  }
  const img = document.createElement('img');
  img.alt = '';
  img.src = src;
  host.replaceChildren(img);
  await waitForImageLoad(img);
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => {
      setTimeout(() => {
        try {
          window.print();
        } finally {
          resolve();
        }
      }, 16);
    });
  });
  if (printDomCleanupTimer) clearTimeout(printDomCleanupTimer);
  printDomCleanupTimer = setTimeout(() => {
    printDomCleanupTimer = null;
    try {
      if (host && host.contains(img)) host.replaceChildren();
    } catch {
      /* ignore */
    }
  }, 8000);
}

async function doExport() {
  if (!validateReady()) return;
  const dest = await save({
    title: t('print_layout.export'),
    defaultPath: `print_layout_${Date.now()}.jpg`,
    filters: [{ name: 'JPEG', extensions: ['jpg', 'jpeg'] }, { name: 'PNG', extensions: ['png'] }],
  });
  if (!dest) return;
  isProcessing.value = true;
  processingMode.value = 'export';
  try {
    // Export = full plan DPI (archive / lab). Independent of print-sized cache.
    await renderSheetTo(String(dest), {
      paperWidth: plan.value.paperPxW,
      paperHeight: plan.value.paperPxH,
      cells: buildExportCells(),
      quality: 92,
      showGuides: showGuides.value,
    });
    toast.success(t('print_layout.export_success'));
    emit('done', { path: String(dest), importToLibrary: importToLibrary.value });
  } catch (err: any) {
    const msg = typeof err === 'string' ? err : err?.message || t('print_layout.export_failed');
    errorMessage.value = msg;
    toast.error(msg);
  } finally {
    isProcessing.value = false;
    processingMode.value = '';
  }
}

async function doPrint() {
  if (!validateReady()) return;
  isProcessing.value = true;
  processingMode.value = 'print';
  toast.info(t('print_layout.printing'));
  try {
    const src = await ensurePrintSheet();
    await printSheetViaWindow(src);
    toast.success(t('print_layout.print_sent'));
    if (importToLibrary.value && printSheetCache.value?.path) {
      // Import uses the temp JPEG on disk (print-sized is OK for album copy).
      emit('done', { path: printSheetCache.value.path, importToLibrary: true });
    }
  } catch (err: any) {
    const msg = typeof err === 'string' ? err : err?.message || t('print_layout.print_failed');
    errorMessage.value = msg;
    toast.error(msg);
  } finally {
    isProcessing.value = false;
    processingMode.value = '';
  }
}

function onCancel() {
  if (isProcessing.value) return;
  void clearAllSheetTemps().finally(() => {
    emit('cancel');
  });
}

// Warm *print-sized* sheet only (fast). Fingerprint is shallow so deep object churn does not thrash.
watch(
  [
    selectedLayoutId,
    background,
    showGuides,
    () => plan.value.paperPxW,
    () => plan.value.paperPxH,
    () => plan.value.placed,
    () => fileAssignmentKey(),
  ],
  () => {
    prerenderGeneration += 1;
    revokePrintCache();
    schedulePrintPrerender();
  },
);

void cleanupStaleTempFiles(24 * 60 * 60);
schedulePrintPrerender();

onUnmounted(() => {
  void clearAllSheetTemps();
});
</script>
