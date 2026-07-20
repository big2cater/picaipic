<template>
  <ModalDialog :title="$t('collage.title')" :width="layoutMode === 'free' ? 860 : 760" @cancel="onCancel">
    <div class="flex flex-col gap-3 text-sm select-none min-h-0">
      <div class="text-xs text-base-content/40">
        {{ $t('collage.hint', { count: imageFiles.length }) }}
      </div>

      <!-- Mode -->
      <div class="space-y-1.5">
        <div class="text-[11px] font-bold uppercase tracking-[0.18em] text-base-content/30">
          {{ $t('collage.mode') }}
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="mode in modeButtons"
            :key="mode"
            type="button"
            class="px-3 py-1.5 rounded-box border text-sm cursor-pointer transition-colors"
            :class="layoutMode === mode
              ? 'border-primary bg-primary/15 text-primary'
              : 'border-base-content/15 hover:bg-base-100/40'"
            @click="setLayoutMode(mode)"
          >
            {{ $t(`collage.mode_${mode}`) }}
          </button>
        </div>
      </div>

      <!-- Layout presets (template / strip only) -->
      <div v-if="layoutMode !== 'free'" class="space-y-1.5">
        <div class="text-[11px] font-bold uppercase tracking-[0.18em] text-base-content/30">
          {{ layoutMode === 'strip' ? $t('collage.strip_direction') : $t('collage.template') }}
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="tpl in visibleLayouts"
            :key="tpl.id"
            type="button"
            class="px-3 py-1.5 rounded-box border text-sm cursor-pointer transition-colors"
            :class="templateId === tpl.id
              ? 'border-primary bg-primary/15 text-primary'
              : 'border-base-content/15 hover:bg-base-100/40'"
            @click="templateId = tpl.id"
          >
            {{ $t(`collage.template_${tpl.id.replace('-', '_')}`) }}
          </button>
        </div>
      </div>

      <!-- Free toolbar -->
      <div v-if="layoutMode === 'free'" class="flex flex-col gap-2">
        <div class="flex flex-wrap items-center gap-2">
          <button type="button" class="t-button-default text-xs" :disabled="!selectedFreeId" @click="rotateSelected(-15)">
            {{ $t('collage.rotate_ccw') }}
          </button>
          <button type="button" class="t-button-default text-xs" :disabled="!selectedFreeId" @click="rotateSelected(15)">
            {{ $t('collage.rotate_cw') }}
          </button>
          <button type="button" class="t-button-default text-xs" :disabled="!selectedFreeId" @click="bringSelectedFront">
            {{ $t('collage.bring_front') }}
          </button>
          <button type="button" class="t-button-default text-xs" :disabled="!selectedFreeId" @click="sendSelectedBack">
            {{ $t('collage.send_back') }}
          </button>
          <label class="flex items-center gap-1.5 text-xs text-base-content/60 ml-auto">
            <input v-model="snapEnabled" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
            {{ $t('collage.snap') }}
          </label>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <button
            type="button"
            class="t-button-default text-xs"
            :disabled="freeItems.length === 0"
            @click="saveFreeDraft"
          >
            {{ $t('collage.draft_save') }}
          </button>
          <select
            v-model="selectedDraftId"
            class="select select-bordered select-xs min-w-[10rem] flex-1"
            :disabled="freeDrafts.length === 0"
          >
            <option value="">{{ freeDrafts.length ? $t('collage.draft_pick') : $t('collage.draft_empty') }}</option>
            <option v-for="d in freeDrafts" :key="d.id" :value="d.id">
              {{ d.name }} ({{ draftMatchCount(d, imageFiles) }}/{{ d.items.length }})
            </option>
          </select>
          <button
            type="button"
            class="t-button-default text-xs"
            :disabled="!selectedDraftId"
            @click="loadSelectedDraft"
          >
            {{ $t('collage.draft_load') }}
          </button>
          <button
            type="button"
            class="t-button-default text-xs"
            :disabled="!selectedDraftId"
            @click="deleteSelectedDraft"
          >
            {{ $t('collage.draft_delete') }}
          </button>
        </div>
        <p v-if="draftMessage" class="text-[11px] text-base-content/45">{{ draftMessage }}</p>
      </div>

      <!-- Preview -->
      <div class="space-y-1.5">
        <div class="text-[11px] font-bold uppercase tracking-[0.18em] text-base-content/30">
          {{ $t('collage.preview') }}
        </div>

        <!-- Template / strip preview (absolute freeform cells, 光影-style) -->
        <div
          v-if="layoutMode !== 'free'"
          class="relative w-full max-w-[480px] mx-auto rounded-box overflow-hidden border border-base-content/10"
          :style="previewFrameStyle"
        >
          <div
            v-for="(cell, index) in freeformPreviewCells"
            :key="index"
            class="absolute overflow-hidden bg-base-content/5"
            :style="freeformCellStyle(cell.rect)"
          >
            <img
              v-if="cell.thumb"
              :src="cell.thumb"
              class="absolute inset-0 h-full w-full"
              :class="fillMode === 'contain' ? 'object-contain' : 'object-cover'"
              draggable="false"
              loading="lazy"
            />
            <div
              v-else
              class="absolute inset-0 flex items-center justify-center text-[10px] text-base-content/35"
            >
              {{ $t('collage.empty_cell') }}
            </div>
          </div>
        </div>

        <!-- Free canvas preview -->
        <div
          v-else
          ref="freeStageRef"
          class="relative w-full max-w-[520px] mx-auto aspect-square rounded-box overflow-hidden border border-base-content/10 touch-none"
          :style="{ backgroundColor: background }"
          @pointerdown.self="selectedFreeId = null"
        >
          <div
            v-for="item in freeItemsSorted"
            :key="item.id"
            class="absolute box-border"
            :class="selectedFreeId === item.id ? 'ring-2 ring-primary z-50' : 'ring-1 ring-base-content/10'"
            :style="freeItemStyle(item)"
            @pointerdown.stop="onFreePointerDown($event, item, 'move')"
          >
            <img
              v-if="item.thumb"
              :src="item.thumb"
              class="absolute inset-0 h-full w-full pointer-events-none"
              :class="fillMode === 'contain' ? 'object-contain' : 'object-cover'"
              draggable="false"
            />
            <div
              v-else
              class="absolute inset-0 flex items-center justify-center text-[10px] text-base-content/40 bg-base-content/5"
            >
              {{ item.name }}
            </div>
            <div
              v-if="selectedFreeId === item.id"
              class="absolute -right-1.5 -bottom-1.5 h-3.5 w-3.5 rounded-sm bg-primary cursor-se-resize"
              @pointerdown.stop="onFreePointerDown($event, item, 'resize')"
            />
          </div>
        </div>

        <div class="text-[11px] text-base-content/40 text-center">
          <template v-if="layoutMode === 'free'">
            {{ $t('collage.free_hint') }} · {{ freeItems.length }} · {{ outputSize.width }}×{{ outputSize.height }}
          </template>
          <template v-else>
            {{ $t('collage.using_n_of_m', { used: usedCount, total: cellCount }) }}
            · {{ outputSize.width }}×{{ outputSize.height }}
          </template>
        </div>
      </div>

      <!-- Options -->
      <div class="grid grid-cols-2 gap-3">
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.fill_mode') }}</span>
          <select v-model="fillMode" class="select select-bordered select-sm">
            <option value="cover">{{ $t('collage.fill_cover') }}</option>
            <option value="contain">{{ $t('collage.fill_contain') }}</option>
          </select>
        </label>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.format') }}</span>
          <select v-model="outputFormat" class="select select-bordered select-sm">
            <option value="jpg">JPEG</option>
            <option value="png">PNG</option>
          </select>
        </label>
        <label v-if="layoutMode !== 'free'" class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.gap') }}</span>
          <input v-model.number="gap" type="range" min="0" max="48" step="1" class="range range-xs range-primary" />
          <span class="text-[11px] opacity-50">{{ gap }} px</span>
        </label>
        <label v-if="layoutMode !== 'free'" class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.margin') }}</span>
          <input v-model.number="margin" type="range" min="0" max="64" step="1" class="range range-xs range-primary" />
          <span class="text-[11px] opacity-50">{{ margin }} px</span>
        </label>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.radius') }}</span>
          <input v-model.number="radius" type="range" min="0" max="48" step="1" class="range range-xs range-primary" />
          <span class="text-[11px] opacity-50">{{ radius }} px</span>
        </label>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.stroke') }}</span>
          <input v-model.number="strokeWidth" type="range" min="0" max="16" step="1" class="range range-xs range-primary" />
          <span class="text-[11px] opacity-50">{{ strokeWidth }} px</span>
        </label>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.background') }}</span>
          <div class="flex items-center gap-2">
            <input v-model="background" type="color" class="h-8 w-12 cursor-pointer rounded border border-base-content/15 bg-transparent" />
            <input v-model="background" type="text" maxlength="16" class="input input-bordered input-sm flex-1 min-w-0" />
          </div>
        </label>
        <label class="form-control">
          <span class="label-text text-xs opacity-70 mb-1">{{ $t('collage.stroke_color') }}</span>
          <div class="flex items-center gap-2">
            <input
              v-model="strokeColor"
              type="color"
              class="h-8 w-12 cursor-pointer rounded border border-base-content/15 bg-transparent"
              :disabled="strokeWidth <= 0"
            />
            <input
              v-model="strokeColor"
              type="text"
              maxlength="16"
              class="input input-bordered input-sm flex-1 min-w-0"
              :disabled="strokeWidth <= 0"
            />
          </div>
        </label>
      </div>

      <p v-if="errorMessage" class="text-error text-xs">{{ errorMessage }}</p>

      <div class="flex justify-end gap-2 pt-1">
        <button class="t-button-default" :disabled="isProcessing" @click="onCancel">
          {{ $t('msgbox.cancel') }}
        </button>
        <button class="t-button-primary" :disabled="isProcessing || exportDisabled" @click="doExport">
          {{ isProcessing ? $t('collage.exporting') : $t('collage.export') }}
        </button>
      </div>
    </div>
  </ModalDialog>

  <MessageBox
    v-if="showDraftNameBox"
    :title="$t('collage.draft_save')"
    :message="$t('collage.draft_name_prompt')"
    :showInput="true"
    :inputText="draftNameInput"
    :inputPlaceholder="$t('collage.draft_name_prompt')"
    :OkText="$t('msgbox.ok')"
    :cancelText="$t('msgbox.cancel')"
    @ok="onDraftNameOk"
    @cancel="showDraftNameBox = false"
  />
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, type CSSProperties } from 'vue';
import { useI18n } from 'vue-i18n';
import { ask, save } from '@tauri-apps/plugin-dialog';
import ModalDialog from '@/components/ModalDialog.vue';
import MessageBox from '@/components/MessageBox.vue';
import { exportCollage } from '@/common/api';
import { config } from '@/common/config';
import {
  COLLAGE_FREE_DRAFT_LIMIT,
  COLLAGE_STRIP_TEMPLATES,
  COLLAGE_TEMPLATES,
  bringFreeToFront,
  clampFreeItem,
  collageCellCount,
  collageCellRects,
  collageOutputSize,
  createFreeDraftId,
  draftMatchCount,
  filterCollageSourceFiles,
  freeSnapGuides,
  initFreeCollageItems,
  isFreeformTemplate,
  normalizeFreeDrafts,
  pickDefaultTemplateId,
  reindexFreeZ,
  restoreFreeItemsFromDraft,
  sendFreeToBack,
  serializeFreeDraftItems,
  snapFreeScalar,
  sortFreeByZ,
  type CollageCellRect,
  type CollageFillMode,
  type CollageMode,
  type CollageTemplateId,
  type FreeCollageDraft,
  type FreeCollageItem,
} from '@/common/collageTemplates';
import { useToast } from '@/common/toast';

type CollageFile = {
  id?: number;
  name?: string;
  file_path?: string;
  file_type?: number | null;
  thumbnail?: string;
};

const props = defineProps<{
  files: CollageFile[];
}>();

const emit = defineEmits<{
  cancel: [];
  done: [path: string];
}>();

const { t } = useI18n();
const toast = useToast();

const modeButtons: CollageMode[] = ['template', 'strip', 'free'];
const imageFiles = computed(() => filterCollageSourceFiles(props.files || []));
const layoutMode = ref<CollageMode>('template');
const templateId = ref<CollageTemplateId>(pickDefaultTemplateId(imageFiles.value.length));
const fillMode = ref<CollageFillMode>('cover');
const gap = ref(8);
const margin = ref(16);
const radius = ref(0);
const strokeWidth = ref(0);
const strokeColor = ref('#000000');
const background = ref('#ffffff');
const outputFormat = ref<'jpg' | 'png'>('jpg');
const isProcessing = ref(false);
const errorMessage = ref('');

// Free canvas state
const freeItems = ref<FreeCollageItem[]>(initFreeCollageItems(imageFiles.value));
const selectedFreeId = ref<string | null>(freeItems.value[0]?.id || null);
const snapEnabled = ref(true);
const freeStageRef = ref<HTMLElement | null>(null);
const selectedDraftId = ref('');
const draftMessage = ref('');
const showDraftNameBox = ref(false);
const draftNameInput = ref('');
const freeDraftsTick = ref(0);

function ensureCollageDraftConfig() {
  const collage = (config as any).collage;
  if (!collage || typeof collage !== 'object') {
    (config as any).collage = { freeDrafts: [] };
  } else if (!Array.isArray(collage.freeDrafts)) {
    collage.freeDrafts = [];
  } else {
    collage.freeDrafts = normalizeFreeDrafts(collage.freeDrafts);
  }
}

ensureCollageDraftConfig();

const freeDrafts = computed<FreeCollageDraft[]>(() => {
  freeDraftsTick.value;
  ensureCollageDraftConfig();
  return normalizeFreeDrafts((config as any).collage?.freeDrafts);
});

type DragKind = 'move' | 'resize';
let dragKind: DragKind = 'move';
let dragItemId: string | null = null;
let dragStartClientX = 0;
let dragStartClientY = 0;
let dragOrigin: FreeCollageItem | null = null;
let dragRaf = 0;
let pendingClientX = 0;
let pendingClientY = 0;

const visibleLayouts = computed(() =>
  layoutMode.value === 'strip' ? COLLAGE_STRIP_TEMPLATES : COLLAGE_TEMPLATES,
);

const cellCount = computed(() => collageCellCount(templateId.value, imageFiles.value.length));
const outputSize = computed(() => {
  if (layoutMode.value === 'free') return { width: 2400, height: 2400 };
  return collageOutputSize(templateId.value, imageFiles.value.length);
});

const freeformPreviewCells = computed(() => {
  const list = imageFiles.value;
  const rects = collageCellRects(templateId.value, list.length);
  return rects.map((rect, i) => {
    const file = list[i];
    return {
      rect,
      thumb: file?.thumbnail || '',
      name: file?.name || file?.file_path || '',
    };
  });
});

const usedCount = computed(() => Math.min(imageFiles.value.length, cellCount.value));
const freeItemsSorted = computed(() => sortFreeByZ(freeItems.value));
const exportDisabled = computed(() =>
  layoutMode.value === 'free' ? freeItems.value.length === 0 : usedCount.value === 0,
);

const previewFrameStyle = computed((): CSSProperties => {
  const { width, height } = outputSize.value;
  const aspect = width / Math.max(1, height);
  return {
    backgroundColor: background.value,
    aspectRatio: String(aspect),
    maxHeight: '360px',
  };
});

function freeformCellStyle(rect: CollageCellRect): CSSProperties {
  const style: CSSProperties = {
    left: `${rect.x * 100}%`,
    top: `${rect.y * 100}%`,
    width: `${rect.w * 100}%`,
    height: `${rect.h * 100}%`,
  };
  if (radius.value > 0) style.borderRadius = `${Math.min(radius.value, 24)}px`;
  if (strokeWidth.value > 0) style.boxShadow = `inset 0 0 0 ${strokeWidth.value}px ${strokeColor.value}`;
  return style;
}

function freeItemStyle(item: FreeCollageItem): CSSProperties {
  return {
    left: `${item.x * 100}%`,
    top: `${item.y * 100}%`,
    width: `${item.w * 100}%`,
    height: `${item.h * 100}%`,
    transform: item.rotate ? `rotate(${item.rotate}deg)` : undefined,
    transformOrigin: 'center center',
    zIndex: item.z,
    borderRadius: radius.value > 0 ? `${Math.min(radius.value, 20)}px` : undefined,
    overflow: 'hidden',
    boxShadow: strokeWidth.value > 0 ? `inset 0 0 0 ${strokeWidth.value}px ${strokeColor.value}` : undefined,
    cursor: selectedFreeId.value === item.id ? 'grab' : 'pointer',
  };
}

function setLayoutMode(mode: CollageMode) {
  layoutMode.value = mode;
  if (mode === 'strip') {
    if (!String(templateId.value).startsWith('strip-')) templateId.value = 'strip-h';
  } else if (mode === 'template') {
    if (String(templateId.value).startsWith('strip-') || templateId.value === 'free') {
      templateId.value = pickDefaultTemplateId(imageFiles.value.length);
    }
  } else {
    templateId.value = 'free';
    if (freeItems.value.length === 0) {
      freeItems.value = initFreeCollageItems(imageFiles.value);
    }
    selectedFreeId.value = freeItems.value[0]?.id || null;
  }
}

function updateFreeItem(id: string, patch: Partial<FreeCollageItem>) {
  freeItems.value = freeItems.value.map((it) =>
    it.id === id ? clampFreeItem({ ...it, ...patch }) : it,
  );
}

function onFreePointerDown(event: PointerEvent, item: FreeCollageItem, kind: DragKind) {
  if (event.button !== 0) return;
  selectedFreeId.value = item.id;
  freeItems.value = bringFreeToFront(freeItems.value, item.id);
  dragKind = kind;
  dragItemId = item.id;
  dragStartClientX = event.clientX;
  dragStartClientY = event.clientY;
  dragOrigin = { ...item };
  pendingClientX = event.clientX;
  pendingClientY = event.clientY;
  (event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId);
  window.addEventListener('pointermove', onFreePointerMove);
  window.addEventListener('pointerup', onFreePointerUp);
  window.addEventListener('pointercancel', onFreePointerUp);
}

function onFreePointerMove(event: PointerEvent) {
  pendingClientX = event.clientX;
  pendingClientY = event.clientY;
  if (dragRaf) return;
  dragRaf = requestAnimationFrame(() => {
    dragRaf = 0;
    applyFreeDrag(pendingClientX, pendingClientY);
  });
}

function onFreePointerUp() {
  if (dragRaf) {
    cancelAnimationFrame(dragRaf);
    dragRaf = 0;
    applyFreeDrag(pendingClientX, pendingClientY);
  }
  dragItemId = null;
  dragOrigin = null;
  window.removeEventListener('pointermove', onFreePointerMove);
  window.removeEventListener('pointerup', onFreePointerUp);
  window.removeEventListener('pointercancel', onFreePointerUp);
}

function applyFreeDrag(clientX: number, clientY: number) {
  if (!dragItemId || !dragOrigin || !freeStageRef.value) return;
  const rect = freeStageRef.value.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;
  const dx = (clientX - dragStartClientX) / rect.width;
  const dy = (clientY - dragStartClientY) / rect.height;
  const guides = freeSnapGuides(freeItems.value, dragItemId);

  if (dragKind === 'move') {
    let x = dragOrigin.x + dx;
    let y = dragOrigin.y + dy;
    if (snapEnabled.value) {
      const sx = snapFreeScalar(x, guides.x);
      const sy = snapFreeScalar(y, guides.y);
      const sx2 = snapFreeScalar(x + dragOrigin.w, guides.x);
      const sy2 = snapFreeScalar(y + dragOrigin.h, guides.y);
      if (sx.snapped) x = sx.value;
      else if (sx2.snapped) x = sx2.value - dragOrigin.w;
      if (sy.snapped) y = sy.value;
      else if (sy2.snapped) y = sy2.value - dragOrigin.h;
    }
    updateFreeItem(dragItemId, { x, y });
  } else {
    let w = dragOrigin.w + dx;
    let h = dragOrigin.h + dy;
    if (snapEnabled.value) {
      w = snapFreeScalar(dragOrigin.x + w, guides.x).value - dragOrigin.x;
      h = snapFreeScalar(dragOrigin.y + h, guides.y).value - dragOrigin.y;
    }
    updateFreeItem(dragItemId, { w, h });
  }
}

function rotateSelected(delta: number) {
  if (!selectedFreeId.value) return;
  const cur = freeItems.value.find((it) => it.id === selectedFreeId.value);
  if (!cur) return;
  updateFreeItem(cur.id, { rotate: cur.rotate + delta });
}

function bringSelectedFront() {
  if (!selectedFreeId.value) return;
  freeItems.value = reindexFreeZ(bringFreeToFront(freeItems.value, selectedFreeId.value));
}

function sendSelectedBack() {
  if (!selectedFreeId.value) return;
  freeItems.value = reindexFreeZ(sendFreeToBack(freeItems.value, selectedFreeId.value));
}

function persistFreeDrafts(list: FreeCollageDraft[]) {
  ensureCollageDraftConfig();
  (config as any).collage.freeDrafts = normalizeFreeDrafts(list).slice(0, COLLAGE_FREE_DRAFT_LIMIT);
  freeDraftsTick.value += 1;
}

function defaultDraftName(): string {
  const stamp = new Date().toLocaleString();
  return t('collage.draft_default_name', { time: stamp });
}

function saveFreeDraft() {
  draftMessage.value = '';
  if (freeItems.value.length === 0) {
    draftMessage.value = t('collage.draft_need_items');
    return;
  }
  // Tauri WebView often no-ops window.prompt; use in-app MessageBox instead.
  draftNameInput.value = defaultDraftName();
  showDraftNameBox.value = true;
}

function onDraftNameOk(nameInput: string) {
  showDraftNameBox.value = false;
  const name = String(nameInput || '').trim() || defaultDraftName();

  const draft: FreeCollageDraft = {
    id: createFreeDraftId(),
    name,
    updatedAt: Date.now(),
    fillMode: fillMode.value,
    radius: Math.max(0, Math.round(radius.value)),
    strokeWidth: Math.max(0, Math.round(strokeWidth.value)),
    strokeColor: strokeColor.value || '#000000',
    background: background.value || '#ffffff',
    outputFormat: outputFormat.value,
    snapEnabled: snapEnabled.value,
    items: serializeFreeDraftItems(freeItems.value),
  };

  const next = [draft, ...freeDrafts.value.filter((d) => d.id !== draft.id)];
  persistFreeDrafts(next);
  selectedDraftId.value = draft.id;
  draftMessage.value = t('collage.draft_saved', { name });
  toast.success(t('collage.draft_saved', { name }));
}

function loadSelectedDraft() {
  draftMessage.value = '';
  const draft = freeDrafts.value.find((d) => d.id === selectedDraftId.value);
  if (!draft) {
    draftMessage.value = t('collage.draft_missing');
    return;
  }
  const restored = restoreFreeItemsFromDraft(draft.items, imageFiles.value);
  if (restored.length === 0) {
    draftMessage.value = t('collage.draft_no_match');
    toast.warning(t('collage.draft_no_match'));
    return;
  }
  freeItems.value = restored;
  selectedFreeId.value = restored[0]?.id || null;
  fillMode.value = draft.fillMode;
  radius.value = draft.radius;
  strokeWidth.value = draft.strokeWidth;
  strokeColor.value = draft.strokeColor;
  background.value = draft.background;
  outputFormat.value = draft.outputFormat;
  snapEnabled.value = draft.snapEnabled;
  layoutMode.value = 'free';
  templateId.value = 'free';
  const dropped = draft.items.length - restored.length;
  draftMessage.value = dropped > 0
    ? t('collage.draft_loaded_partial', { used: restored.length, total: draft.items.length })
    : t('collage.draft_loaded', { count: restored.length });
  toast.success(draftMessage.value);
}

async function deleteSelectedDraft() {
  draftMessage.value = '';
  const id = selectedDraftId.value;
  if (!id) return;
  const draft = freeDrafts.value.find((d) => d.id === id);
  if (!draft) return;
  const confirmed = await ask(t('collage.draft_delete_confirm', { name: draft.name }), {
    title: t('collage.draft_delete'),
    kind: 'warning',
    okLabel: t('collage.draft_delete'),
    cancelLabel: t('msgbox.cancel'),
  });
  if (!confirmed) return;
  persistFreeDrafts(freeDrafts.value.filter((d) => d.id !== id));
  selectedDraftId.value = '';
  draftMessage.value = t('collage.draft_deleted');
}

function onCancel() {
  if (isProcessing.value) return;
  emit('cancel');
}

function defaultExportName(): string {
  const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-');
  const tag = layoutMode.value === 'free' ? 'free' : templateId.value;
  return `collage_${tag}_${stamp}.${outputFormat.value === 'png' ? 'png' : 'jpg'}`;
}

async function doExport() {
  errorMessage.value = '';

  if (layoutMode.value === 'free') {
    if (freeItems.value.length === 0) {
      errorMessage.value = t('collage.need_images');
      return;
    }
  } else {
    const sources = imageFiles.value
      .slice(0, cellCount.value)
      .map((f) => String(f.file_path || ''))
      .filter(Boolean);
    if (sources.length === 0) {
      errorMessage.value = t('collage.need_images');
      return;
    }
  }

  const destPath = await save({
    title: t('collage.export'),
    defaultPath: defaultExportName(),
    filters: [
      outputFormat.value === 'png'
        ? { name: 'PNG', extensions: ['png'] }
        : { name: 'JPEG', extensions: ['jpg', 'jpeg'] },
    ],
  });
  if (!destPath) return;

  isProcessing.value = true;
  try {
    if (layoutMode.value === 'free') {
      const size = { width: 2400, height: 2400 };
      const sorted = sortFreeByZ(freeItems.value);
      await exportCollage({
        sourceFilePaths: sorted.map((it) => it.filePath),
        destFilePath: destPath,
        outputFormat: outputFormat.value === 'png' ? 'png' : 'jpg',
        quality: 90,
        template: 'free',
        outputWidth: size.width,
        outputHeight: size.height,
        gap: 0,
        margin: 0,
        background: background.value || '#ffffff',
        fillMode: fillMode.value,
        radius: Math.max(0, Math.round(radius.value)),
        strokeWidth: Math.max(0, Math.round(strokeWidth.value)),
        strokeColor: strokeColor.value || '#000000',
        items: sorted.map((it) => ({
          filePath: it.filePath,
          x: it.x,
          y: it.y,
          w: it.w,
          h: it.h,
          rotate: it.rotate,
          z: it.z,
        })),
      });
    } else {
      const sources = imageFiles.value
        .slice(0, cellCount.value)
        .map((f) => String(f.file_path || ''))
        .filter(Boolean);
      const size = collageOutputSize(templateId.value, sources.length);
      const rects = collageCellRects(templateId.value, sources.length);
      // Freeform magazine templates bake margins/gaps into cell geometry.
      // Equal grids still honor UI gap/margin sliders via host grid path when no cells payload.
      const useCells = isFreeformTemplate(templateId.value) || layoutMode.value === 'strip' || rects.length > 0;
      await exportCollage({
        sourceFilePaths: sources,
        destFilePath: destPath,
        outputFormat: outputFormat.value === 'png' ? 'png' : 'jpg',
        quality: 90,
        template: useCells ? 'cells' : templateId.value,
        outputWidth: size.width,
        outputHeight: size.height,
        gap: useCells ? 0 : Math.max(0, Math.round(gap.value)),
        margin: useCells ? 0 : Math.max(0, Math.round(margin.value)),
        background: background.value || '#ffffff',
        fillMode: fillMode.value,
        radius: Math.max(0, Math.round(radius.value)),
        strokeWidth: Math.max(0, Math.round(strokeWidth.value)),
        strokeColor: strokeColor.value || '#000000',
        cells: useCells
          ? rects.map((r) => ({ x: r.x, y: r.y, w: r.w, h: r.h }))
          : undefined,
      });
    }
    toast.success(t('collage.export_success'));
    emit('done', destPath);
  } catch (err: any) {
    const msg = typeof err === 'string' ? err : err?.message || t('collage.export_failed');
    errorMessage.value = msg;
    toast.error(msg);
  } finally {
    isProcessing.value = false;
  }
}

onUnmounted(() => {
  window.removeEventListener('pointermove', onFreePointerMove);
  window.removeEventListener('pointerup', onFreePointerUp);
  window.removeEventListener('pointercancel', onFreePointerUp);
  if (dragRaf) cancelAnimationFrame(dragRaf);
});
</script>
