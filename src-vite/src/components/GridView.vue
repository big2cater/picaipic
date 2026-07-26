<template>
  <div
    ref="containerRef"
    class="relative w-full h-full" 
    :class="{ 
      'pointer-events-none': uiStore.inputStack.length > 0,
    }"
    @wheel="onWheel"
    @dragstart.capture.prevent
  >
    <!-- FragCoord-style photo-area vortex (WebGL UV warp). CSS card warp disabled while this is primary. -->
    <PhotoVortexLayer
      v-if="vortexEnabled"
      class="z-20"
      :active="vortexActive"
      :source-el="containerRef"
      :primary-rgb="vortexPrimaryRgb"
      @captured="onVortexCaptured"
      @cleared="onVortexCleared"
    />
    <PhotoGlitchLayer
      v-if="glitchEnabled"
      class="z-20"
      :active="glitchLayerActive"
      :source-el="containerRef"
      :intensity="glitchIntensity"
      @captured="onGlitchCaptured"
      @cleared="onGlitchCleared"
    />

    <VirtualScroll
      v-if="fileList.length > 0"
      ref="scroller"
      class="w-full h-full no-scrollbar"
      :class="{
        'pt-12': !config.settings.grid.showFilmStrip,
        'pb-8': !config.settings.grid.showFilmStrip && config.settings.showStatusBar,
        'pb-1': !config.settings.grid.showFilmStrip && !config.settings.showStatusBar,
        // Hide live grid after freeze-frame capture so only the warped layer shows
        'opacity-0 pointer-events-none': vortexHidesGrid || glitchHidesGrid,
      }"
      :items="renderItems"
      :direction="config.settings.grid.showFilmStrip && config.settings.grid.previewPosition < 2 ? 'horizontal' : 'vertical'"
      :grid-items="config.settings.grid.showFilmStrip ? 1 : columnCount"
      :item-size="config.settings.grid.showFilmStrip ? (config.settings.grid.previewPosition < 2 ? filmStripItemSize : itemHeight) : itemHeight"
      :item-secondary-size="!config.settings.grid.showFilmStrip ? itemWidth : (config.settings.grid.previewPosition >= 2 ? itemWidth : undefined)"
      :key="`${config.settings.grid.showFilmStrip}-${dateGroupingEnabled}-${sectionHeaderEnabled}-${props.sectionLabel || ''}`"
      :geometry="virtualScrollGeometry"
      :content-height="virtualScrollContentHeight"
      :transition="isLayoutTransitioning"
      key-field="id"
      :emit-update="true"
      :buffer="8"
      v-slot="{ item, index }"
      @update="onUpdate"
      @scroll="onScroll"
    >
      <div
        v-if="isDateHeader(item)"
        class="w-full h-full flex items-center gap-1 px-1 text-base-content/70 select-none group"
        :class="{ 'cursor-pointer hover:text-base-content': selectMode }"
        @click="selectMode && toggleDateGroupSelection(item)"
      >
        <input
          v-if="selectMode"
          type="checkbox"
          class="checkbox checkbox-sm border-base-content/30 group-hover:border-base-content/70"
          :checked="getDateGroupSelectionState(item).allSelected"
          :indeterminate.prop="getDateGroupSelectionState(item).partialSelected"
          @click.stop
          @change="(event) => toggleDateGroupSelection(item, (event.target as HTMLInputElement).checked)"
        />
        <component
          :is="item?.isSectionHeader ? IconSearch : (effectiveDateGroupingMode === 1 ? IconCalendarDay : IconCalendarMonth)"
          v-if="!selectMode"
          class="w-5 h-5"
        />
        <span>{{ item.label }}</span>
        <span class="text-base-content/30 text-xs">({{ (item.endIndex - item.startIndex).toLocaleString() }})</span>
      </div>
      <div
        v-else
        class="w-full h-full flex items-center justify-center overflow-hidden"
        @pointerdown="onItemPointerDown($event, getFileIndex(item, index))"
      >
        <Thumbnail
          v-if="getFileItem(item) && !getFileItem(item).isPlaceholder"
          :id="'item-' + getFileIndex(item, index)"
          :file="getFileItem(item)"
          :is-selected="selectMode ? Boolean(getFileItem(item).isSelected) : getFileIndex(item, index) === selectedItemIndex"
          :is-active="getFileIndex(item, index) === selectedItemIndex"
          :select-mode="selectMode"
          @clicked="(modifiers) => $emit('item-clicked', getFileIndex(item, index), modifiers)"
          @dblclicked="(modifiers) => $emit('item-dblclicked', getFileIndex(item, index), modifiers)"
          @select-toggled="(shiftKey) => $emit('item-select-toggled', getFileIndex(item, index), shiftKey)"
          @action="(action) => $emit('item-action', { action, index: getFileIndex(item, index) })"
          @select-contextmenu="(payload) => $emit('select-contextmenu', { ...payload, index: getFileIndex(item, index) })"
        />
        <div v-else class="w-full h-full bg-base-200/70"></div>
      </div>
    </VirtualScroll>
    <!-- Empty State / Loading -->
    <div v-else class="absolute inset-0 flex flex-col items-center justify-center">
      <div class="text-base-content/30 flex flex-col items-center gap-2 text-center px-4">
        <template v-if="showDelayedLoading">
          <span class="loading loading-dots loading-lg text-primary"></span>
          <span>{{ $t('tooltip.loading') }}</span>
        </template>
        <template v-else-if="!contentReady" />
        <template v-else-if="showFolderFiles && folderExcluded">
          <span>{{ $t('tooltip.not_found.folder_excluded') }}</span>
          <span class="text-xs">{{ $t('tooltip.not_found.folder_excluded_hint') }}</span>
        </template>
        <template v-else-if="showFolderFiles">
          <span>{{ $t('tooltip.not_found.folder_files') }}</span>
          <span class="text-xs">{{ $t('tooltip.not_found.folder_files_hint') }}</span>
        </template>
        <span v-else>{{ emptyMessage || $t('tooltip.not_found.files') }}</span>
      </div>
    </div>

  </div>

</template>

<script setup lang="ts">

import { watch, ref, onMounted, onBeforeUnmount, computed, nextTick, inject, unref, type Ref, type ComputedRef } from 'vue';
import { useI18n } from 'vue-i18n';
import { useUIStore } from '@/stores/uiStore';
import { config } from '@/common/config';
import { formatDate } from '@/common/utils';
import Thumbnail from '@/components/Thumbnail.vue';
import VirtualScroll from '@/components/VirtualScroll.vue';
import PhotoVortexLayer from '@/components/PhotoVortexLayer.vue';
import PhotoGlitchLayer from '@/components/PhotoGlitchLayer.vue';
import { calculateJustifiedLayout, calculateLinearRowLayout, calculateLinearColumnLayout, calculateMasonryLayout, type Geometry } from '@/common/layout';
import { IconCalendarDay, IconCalendarMonth, IconSearch } from '@/common/icons';
import { readPrimaryColor, type RadiiValue } from '@/common/blackHoleMath';
import { isBlackHoleTheme, isCyberpunkTheme } from '@/common/utils';
// CSS per-card warp kept available but NOT driven while WebGL vortex is primary
// import { useGravityWarp } from '@/composables/useGravityWarp';

const props = withDefaults(defineProps<{
  selectedItemIndex: number;
  fileList: any[];
  timelineData?: any[];
  sortType?: number;
  /** Effective date grouping mode: 0 none, 1 day, 2 month. When omitted, falls back to settings. */
  dateGrouping?: number | null;
  /** Optional single section header for search/similar result sets (when date grouping is off). */
  sectionLabel?: string | null;
  showFolderFiles?: boolean;
  folderExcluded?: boolean;
  selectMode?: boolean;
  contentReady?: boolean;
  emptyMessage?: string;
  layoutVersion?: number;
}>(), {
  selectedItemIndex: -1,
  timelineData: () => [],
  sortType: 0,
  dateGrouping: null,
  sectionLabel: null,
  showFolderFiles: false,
  folderExcluded: false,
  selectMode: false,
  contentReady: false,
  emptyMessage: '',
  layoutVersion: 0,
});

const emit = defineEmits([
  'item-clicked',
  'item-dblclicked',
  'item-select-toggled',
  'item-action',
  'select-contextmenu',
  'date-group-select',
  'request-scroll',
  'visible-range-update',
  'scroll',
  'layout-update',
  'item-drag-start',
  'item-drag',
  'item-drag-end',
]);

const uiStore = useUIStore();
const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[locale.value] as any);
const containerRef = ref<HTMLElement | null>(null);
const bhGravityActive = inject<Ref<boolean> | ComputedRef<boolean> | null>('bhGravityActive', null);
const cpGlitchActive = inject<Ref<boolean> | ComputedRef<boolean> | null>('cpGlitchActive', null);
const bhRadii = inject<Ref<RadiiValue> | ComputedRef<RadiiValue> | null>('bhRadii', null);
const gravityActiveFallback = ref(false);
const radiiFallback = ref<RadiiValue>({ R_event: 0, R_inf: 0 });

// Theme-gate mount so we don't keep two idle WebGL contexts alive at once.
// Provide is always present from Home; inject != null alone is not enough.
const blackHoleThemeOn = computed(() =>
  isBlackHoleTheme(
    Number(config.settings.appearance),
    Number(config.settings.lightTheme),
    Number(config.settings.darkTheme),
  ),
);
const cyberpunkThemeOn = computed(() =>
  isCyberpunkTheme(
    Number(config.settings.appearance),
    Number(config.settings.lightTheme),
    Number(config.settings.darkTheme),
  ),
);

// WebGL FragCoord-style vortex (photo area only). CSS card warp disabled as primary path.
const vortexEnabled = computed(() => blackHoleThemeOn.value && bhGravityActive != null);
const vortexActive = computed(() => blackHoleThemeOn.value && !!unref(bhGravityActive));
const vortexHidesGrid = ref(false);
const vortexPrimaryRgb = ref<[number, number, number]>([180, 80, 200]);

// Cyberpunk continuous glitch layer (photo area only)
const glitchEnabled = computed(() => cyberpunkThemeOn.value && cpGlitchActive != null);
const glitchHidesGrid = ref(false);
const glitchLayerActive = computed(() => {
  const intensity = Number(config.settings.dynamicThemeIntensity);
  return (
    cyberpunkThemeOn.value
    && !!unref(cpGlitchActive)
    && Number.isFinite(intensity)
    && intensity > 0
  );
});
const glitchIntensity = computed(() => {
  const n = Number(config.settings.dynamicThemeIntensity);
  return Number.isFinite(n) ? n : 1;
});

function parsePrimaryRgb(): [number, number, number] {
  const raw = readPrimaryColor();
  // DaisyUI may expose "r g b" components or a css color; try both
  const parts = raw.split(/[\s,]+/).map(Number).filter((n) => Number.isFinite(n));
  if (parts.length >= 3) return [parts[0], parts[1], parts[2]];
  try {
    const c = document.createElement('canvas');
    c.width = c.height = 1;
    const ctx = c.getContext('2d');
    if (!ctx) return [180, 80, 200];
    ctx.fillStyle = '#000';
    ctx.fillStyle = raw.startsWith('#') || raw.includes('(') ? raw : `rgb(${raw})`;
    ctx.fillRect(0, 0, 1, 1);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return [d[0], d[1], d[2]];
  } catch {
    return [180, 80, 200];
  }
}

function onVortexCaptured() {
  vortexHidesGrid.value = true;
}

function onVortexCleared() {
  vortexHidesGrid.value = false;
}

function onGlitchCaptured() {
  glitchHidesGrid.value = true;
}

function onGlitchCleared() {
  glitchHidesGrid.value = false;
}

watch(vortexActive, (on) => {
  if (on) vortexPrimaryRgb.value = parsePrimaryRgb();
  else vortexHidesGrid.value = false;
});

watch(glitchLayerActive, (on) => {
  if (!on) glitchHidesGrid.value = false;
});

// Silence unused inject fallbacks (kept for future CSS-warp toggle)
void gravityActiveFallback;
void radiiFallback;
void bhRadii;

const scroller = ref<any>(null);
const columnCount = ref(4);
const containerWidth = ref(0);
const headerHeight = 48;
let pendingPointerDrag: {
  pointerId: number;
  index: number;
  startX: number;
  startY: number;
  hotspotXRatio: number;
  hotspotYRatio: number;
  active: boolean;
} | null = null;

function clearPointerDragListeners() {
  document.removeEventListener('pointermove', onDocumentPointerMove, true);
  document.removeEventListener('pointerup', onDocumentPointerUp, true);
  document.removeEventListener('pointercancel', onDocumentPointerUp, true);
}

function onItemPointerDown(event: PointerEvent, index: number) {
  const target = event.target as HTMLElement;
  if (
    event.button !== 0
    || event.pointerType === 'touch'
    || target.closest('button, input, a, [role="button"]')
    || !props.fileList[index]
    || props.fileList[index].isPlaceholder
  ) return;
  const itemElement = document.getElementById(`item-${index}`);
  const itemRect = itemElement?.getBoundingClientRect();
  pendingPointerDrag = {
    pointerId: event.pointerId,
    index,
    startX: event.clientX,
    startY: event.clientY,
    hotspotXRatio: itemRect?.width
      ? Math.max(0, Math.min(1, (event.clientX - itemRect.left) / itemRect.width))
      : 0.5,
    hotspotYRatio: itemRect?.height
      ? Math.max(0, Math.min(1, (event.clientY - itemRect.top) / itemRect.height))
      : 0.5,
    active: false,
  };
  document.addEventListener('pointermove', onDocumentPointerMove, true);
  document.addEventListener('pointerup', onDocumentPointerUp, true);
  document.addEventListener('pointercancel', onDocumentPointerUp, true);
}

function onDocumentPointerMove(event: PointerEvent) {
  const drag = pendingPointerDrag;
  if (!drag || event.pointerId !== drag.pointerId) return;
  if (!drag.active) {
    if (Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) < 6) return;
    drag.active = true;
    document.documentElement.style.userSelect = 'none';
    document.documentElement.style.webkitUserSelect = 'none';
    emit('item-drag-start', {
      event,
      index: drag.index,
      hotspotXRatio: drag.hotspotXRatio,
      hotspotYRatio: drag.hotspotYRatio,
    });
  }
  event.preventDefault();
  emit('item-drag', event);
}

function onDocumentPointerUp(event: PointerEvent) {
  const drag = pendingPointerDrag;
  if (!drag || event.pointerId !== drag.pointerId) return;
  if (drag.active) {
    event.preventDefault();
    event.stopPropagation();
    emit('item-drag-end', event);
  }
  pendingPointerDrag = null;
  document.documentElement.style.userSelect = '';
  document.documentElement.style.webkitUserSelect = '';
  clearPointerDragListeners();
}

function isGeometryGridStyle(style: number) {
  return style === 2 || style === 3;
}

const isTimeSort = computed(() => [0, 1, 2].includes(Number(props.sortType)));
/** Prefer Content-provided view-adaptive grouping; fall back to Settings. */
const effectiveDateGroupingMode = computed(() => {
  if (props.dateGrouping != null) return Number(props.dateGrouping || 0);
  return Number(config.settings.grid.dateGrouping || 0);
});
const dateGroupingEnabled = computed(() =>
  !config.settings.grid.showFilmStrip &&
  isTimeSort.value &&
  effectiveDateGroupingMode.value > 0 &&
  props.timelineData.length > 0
);

function formatDateGroupLabel(marker: any, mode: number) {
  const year = Number(marker.year || 0);
  const month = Number(marker.month || 0);
  const date = Number(marker.date || 1);
  if (!year || !month) return '';

  if (mode === 1) {
    return formatDate(year, month, date, localeMsg.value.format.date_long);
  }
  return formatDate(year, month, 1, localeMsg.value.format.month);
}

const dateGroupMarkers = computed(() => {
  if (!dateGroupingEnabled.value) return [];
  const mode = effectiveDateGroupingMode.value;
  const seen = new Set<string>();
  const markers: any[] = [];

  for (const marker of props.timelineData) {
    const position = Number(marker.position);
    if (!Number.isFinite(position) || position < 0 || position >= props.fileList.length) continue;
    const year = Number(marker.year || 0);
    const month = Number(marker.month || 0);
    const date = Number(marker.date || 0);
    if (!year || !month || (mode === 1 && !date)) continue;
    const key = mode === 1 ? `${year}-${month}-${date}` : `${year}-${month}`;
    if (seen.has(key)) continue;
    seen.add(key);
    markers.push({
      ...marker,
      key,
      position,
      label: formatDateGroupLabel(marker, mode),
    });
  }

  return markers.sort((a, b) => a.position - b.position);
});

const sectionHeaderEnabled = computed(() => {
  if (dateGroupingEnabled.value) return false;
  if (config.settings.grid.showFilmStrip) return false;
  const label = String(props.sectionLabel || '').trim();
  return !!label && props.fileList.length > 0;
});

/** True when renderItems uses header + file wrappers (date groups or search section). */
const hasHeaderItems = computed(() => dateGroupingEnabled.value || sectionHeaderEnabled.value);

/** Build header+file list and fileIndex→displayIndex in one pass (B3/B4). */
const renderLayout = computed(() => {
  const fileIndexMap = new Map<number, number>();

  // Single section header for AI / similar / filename search result sets.
  if (sectionHeaderEnabled.value) {
    const label = String(props.sectionLabel || '').trim();
    const items: any[] = [{
      id: `section-header-${label}`,
      isDateHeader: true,
      isSectionHeader: true,
      label,
      fileIndex: 0,
      startIndex: 0,
      endIndex: props.fileList.length,
    }];
    props.fileList.forEach((file, fileIndex) => {
      fileIndexMap.set(fileIndex, items.length);
      items.push({
        id: `section-file-${file?.id ?? fileIndex}-${fileIndex}`,
        isDateFile: true,
        file,
        fileIndex,
      });
    });
    return { items, fileIndexMap };
  }

  if (!dateGroupingEnabled.value) {
    return { items: props.fileList, fileIndexMap };
  }

  const markersByPosition = new Map<number, any[]>();
  dateGroupMarkers.value.forEach(marker => {
    if (!markersByPosition.has(marker.position)) markersByPosition.set(marker.position, []);
    markersByPosition.get(marker.position)!.push(marker);
  });

  const markerEndIndex = new Map<string, number>();
  dateGroupMarkers.value.forEach((marker, i) => {
    const nextMarker = dateGroupMarkers.value[i + 1];
    markerEndIndex.set(marker.key, nextMarker ? nextMarker.position : props.fileList.length);
  });

  const items: any[] = [];
  props.fileList.forEach((file, fileIndex) => {
    const markers = markersByPosition.get(fileIndex) || [];
    markers.forEach(marker => {
      items.push({
        id: `date-header-${marker.key}-${marker.position}`,
        isDateHeader: true,
        label: marker.label,
        fileIndex,
        startIndex: marker.position,
        endIndex: markerEndIndex.get(marker.key) ?? props.fileList.length,
      });
    });
    fileIndexMap.set(fileIndex, items.length);
    items.push({
      id: `date-file-${file?.id ?? fileIndex}-${fileIndex}`,
      isDateFile: true,
      file,
      fileIndex,
    });
  });

  return { items, fileIndexMap };
});

const renderItems = computed(() => renderLayout.value.items);

const fileIndexToDisplayIndex = computed(() => renderLayout.value.fileIndexMap);

/** Precomputed selection state per date-header key — avoids O(group) scans in template. */
const dateGroupSelectionState = computed(() => {
  const map = new Map<string, { allSelected: boolean; partialSelected: boolean }>();
  if (!props.selectMode || !hasHeaderItems.value) return map;

  for (const item of renderItems.value) {
    if (!item?.isDateHeader || item.isSectionHeader) continue;
    const key = String(item.id ?? item.label ?? '');
    const startIndex = Number(item.startIndex ?? 0);
    const endIndex = Number(item.endIndex ?? startIndex);
    const fileCount = Math.max(0, endIndex - startIndex);
    if (fileCount === 0) {
      map.set(key, { allSelected: false, partialSelected: false });
      continue;
    }
    let selectedCount = 0;
    for (let index = startIndex; index < endIndex; index++) {
      if (props.fileList[index]?.isSelected) selectedCount++;
    }
    map.set(key, {
      allSelected: selectedCount === fileCount,
      partialSelected: selectedCount > 0 && selectedCount < fileCount,
    });
  }
  return map;
});

// Layout Geometry Calculation
const groupedLayoutGeometryResult = computed(() => {
  if (!hasHeaderItems.value || renderItems.value.length === 0 || containerWidth.value <= 0) {
    return { boxes: [], contentSize: 0 };
  }

  const { style, size, showFilmStrip } = config.settings.grid;
  const boxes: Geometry[] = new Array(renderItems.value.length);

  if (showFilmStrip) return { boxes: [], contentSize: 0 };

  if (!isGeometryGridStyle(style)) {
    let y = 0;
    let col = 0;

    renderItems.value.forEach((item, displayIndex) => {
      if (item?.isDateHeader) {
        if (col > 0) {
          y += itemHeight.value;
          col = 0;
        }
        boxes[displayIndex] = { x: 0, y, width: containerWidth.value, height: headerHeight };
        y += headerHeight;
        return;
      }

      boxes[displayIndex] = {
        x: col * itemWidth.value,
        y,
        width: itemWidth.value,
        height: itemHeight.value,
      };
      col += 1;
      if (col >= columnCount.value) {
        y += itemHeight.value;
        col = 0;
      }
    });

    if (col > 0) y += itemHeight.value;
    return { boxes, contentSize: y };
  }

  let y = 0;
  let groupFiles: any[] = [];
  let groupDisplayIndices: number[] = [];

  const flushGroup = () => {
    if (groupFiles.length === 0) return;
    const result = config.settings.grid.style === 3
      ? calculateMasonryLayout(groupFiles, containerWidth.value, size, 0)
      : calculateJustifiedLayout(groupFiles, containerWidth.value, size, 0);
    result.boxes.forEach((box, index) => {
      boxes[groupDisplayIndices[index]] = {
        ...box,
        y: box.y + y,
      };
    });
    y += result.containerHeight;
    groupFiles = [];
    groupDisplayIndices = [];
  };

  renderItems.value.forEach((item, displayIndex) => {
    if (item?.isDateHeader) {
      flushGroup();
      boxes[displayIndex] = { x: 0, y, width: containerWidth.value, height: headerHeight };
      y += headerHeight;
      return;
    }

    groupFiles.push(item.file);
    groupDisplayIndices.push(displayIndex);
  });
  flushGroup();

  return { boxes, contentSize: y };
});

const layoutGeometryResult = computed(() => {
  if (props.fileList.length === 0) {
    return { boxes: [], contentSize: 0 };
  }

  const { style, size, showFilmStrip } = config.settings.grid;

  if (hasHeaderItems.value) {
    return groupedLayoutGeometryResult.value;
  }

  if (showFilmStrip) {
    if (isGeometryGridStyle(style)) {
      const isVertical = config.settings.grid.previewPosition >= 2;
      if (isVertical) {
        if (containerWidth.value <= 0) return { boxes: [], contentSize: 0 };
        const result = calculateLinearColumnLayout(props.fileList, containerWidth.value, 0);
        return { boxes: result.boxes, contentSize: result.containerHeight };
      }
      const result = calculateLinearRowLayout(props.fileList, size, 0);
      return { boxes: result.boxes, contentSize: result.containerWidth };
    }
  } else {
    if (style === 2 && containerWidth.value > 0) {
      const result = calculateJustifiedLayout(props.fileList, containerWidth.value, size, 0);
      return { boxes: result.boxes, contentSize: result.containerHeight };
    }
    else if (style === 3 && containerWidth.value > 0) {
      const result = calculateMasonryLayout(props.fileList, containerWidth.value, size, 0);
      return { boxes: result.boxes, contentSize: result.containerHeight };
    }
  }
  return { boxes: [], contentSize: 0 };
});

const layoutGeometry = computed(() => layoutGeometryResult.value.boxes);
const layoutContentHeight = computed(() => layoutGeometryResult.value.contentSize);
const usesGeometryLayout = computed(() =>
  hasHeaderItems.value ||
  isGeometryGridStyle(config.settings.grid.style)
);
const virtualScrollGeometry = computed(() =>
  usesGeometryLayout.value ? layoutGeometry.value : undefined
);
const virtualScrollContentHeight = computed(() =>
  usesGeometryLayout.value ? layoutContentHeight.value : undefined
);

const isLayoutTransitioning = ref(false);
const startGridSize = ref(0);
let layoutTransitionTimer: ReturnType<typeof setTimeout> | null = null;
let layoutAnchorVersion = 0;
let isInitialLayout = true;

const gap = 8; // Gap between items
const isVerticalFilmstrip = computed(() => config.settings.grid.showFilmStrip && config.settings.grid.previewPosition >= 2);

// item width and height(including gap)
const itemWidth = computed(() => {
  const { style, size } = config.settings.grid;
  if (isVerticalFilmstrip.value && containerWidth.value > 0) {
    return containerWidth.value;
  }
  if (style === 0) return size + 20; // size + padding(4*2) + border(2*2) + gap(8)
  return size;
});

const itemHeight = computed(() => {
  const { style, size } = config.settings.grid;
  
  if (style === 0) {
    let labelHeight = 0;
    if (config.settings.grid.labelPrimary > 0) labelHeight += 18;   // text-sm
    if (config.settings.grid.labelSecondary > 0) labelHeight += 16; // text-xs
    
    if (isVerticalFilmstrip.value && containerWidth.value > 0) {
      return containerWidth.value + 12 + labelHeight; // Narrower padding in filmstrip
    }
    return size + 20 + labelHeight; // size + padding/border/gap(20) + labels
  }
  if (style === 1) return itemWidth.value + gap * 0.5;
  
  if (isVerticalFilmstrip.value && containerWidth.value > 0) {
    return containerWidth.value;
  }
  return size;
});

const filmStripItemSize = computed(() => {
  return itemWidth.value;
});

let resizeObserver: ResizeObserver | null = null;
const showDelayedLoading = ref(false);
let loadingDelayTimer: ReturnType<typeof setTimeout> | null = null;

function updateColumnCount() {
  if (containerRef.value) {
    containerWidth.value = containerRef.value.clientWidth;
    if (itemWidth.value > 0) {
      columnCount.value = Math.max(1, Math.floor(containerWidth.value / itemWidth.value));
    }
  }
}

function updateLayout() {
  updateColumnCount();
  emit('layout-update', { height: layoutContentHeight.value });
}

watch(() => [config.settings.grid.size, config.settings.grid.style, config.settings.grid.showFilmStrip, config.settings.grid.dateGrouping, props.dateGrouping, props.sortType], async () => {
  if (isInitialLayout) {
    isInitialLayout = false;
    updateColumnCount();
    return;
  }

  const anchorVersion = ++layoutAnchorVersion;
  isLayoutTransitioning.value = true;
  updateColumnCount();

  await nextTick();
  if (anchorVersion !== layoutAnchorVersion) return;

  centerItem(props.selectedItemIndex);

  if (layoutTransitionTimer) clearTimeout(layoutTransitionTimer);
  layoutTransitionTimer = setTimeout(() => {
    layoutTransitionTimer = null;
    isLayoutTransitioning.value = false;
  }, 500);
});

watch(() => props.fileList, () => {
  updateLayout();
});

watch(() => props.timelineData, () => {
  updateLayout();
});

watch(() => props.layoutVersion, () => {
  updateLayout();
});

watch(
  () => props.contentReady,
  (ready) => {
    if (loadingDelayTimer) {
      clearTimeout(loadingDelayTimer);
      loadingDelayTimer = null;
    }

    if (ready) {
      showDelayedLoading.value = false;
      return;
    }

    showDelayedLoading.value = false;
    loadingDelayTimer = setTimeout(() => {
      loadingDelayTimer = null;
      if (!props.contentReady) {
        showDelayedLoading.value = true;
      }
    }, 500);
  },
  { immediate: true }
);

watch(layoutContentHeight, (newHeight) => {
  emit('layout-update', { height: newHeight });
});

watch(() => props.selectedItemIndex, (newValue) => {
  if (newValue !== -1) {
    scrollToItem(newValue);
  }
});

onMounted(() => {
  if (containerRef.value) {
    resizeObserver = new ResizeObserver(() => {
      // updateColumnCount(); // merged into updateLayout
      updateLayout();
      if (props.selectedItemIndex !== -1) {
        scrollToItem(props.selectedItemIndex);
      }
    });
    resizeObserver.observe(containerRef.value);
    updateLayout();

    // gesture events for macOS touchpad pinch
    containerRef.value.addEventListener('gesturestart', onGestureStart as any);
    containerRef.value.addEventListener('gesturechange', onGestureChange as any);
  }
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown);
  clearPointerDragListeners();
  pendingPointerDrag = null;
  document.documentElement.style.userSelect = '';
  document.documentElement.style.webkitUserSelect = '';
  if (loadingDelayTimer) {
    clearTimeout(loadingDelayTimer);
    loadingDelayTimer = null;
  }
  if (layoutTransitionTimer) {
    clearTimeout(layoutTransitionTimer);
    layoutTransitionTimer = null;
  }
  if (containerRef.value) {
    containerRef.value.removeEventListener('gesturestart', onGestureStart as any);
    containerRef.value.removeEventListener('gesturechange', onGestureChange as any);
  }
  if (resizeObserver) {
    resizeObserver.disconnect();
  }
});

function onGestureStart(e: any) {
  e.preventDefault();
  startGridSize.value = config.settings.grid.size;
}

function onGestureChange(e: any) {
  e.preventDefault();
  if (startGridSize.value > 0) {
    let newSize = Math.round(startGridSize.value * e.scale);
    // Clamp between 120 and 360
    newSize = Math.max(120, Math.min(360, newSize));
    config.settings.grid.size = newSize;
  }
}

function onUpdate(startIndex: number, endIndex: number) {
  if (hasHeaderItems.value) {
    const visibleFiles = renderItems.value
      .slice(startIndex, endIndex)
      .filter(item => item?.isDateFile)
      .map(item => item.fileIndex);
    if (visibleFiles.length === 0) {
      const fallback = getNearestFileIndexFromDisplayIndex(startIndex);
      emit('visible-range-update', { startIndex: fallback, endIndex: fallback + 1 });
      return;
    }
    emit('visible-range-update', {
      startIndex: Math.min(...visibleFiles),
      endIndex: Math.max(...visibleFiles) + 1,
    });
    return;
  }
  emit('visible-range-update', { startIndex, endIndex });
}

function onScroll(e: Event) {
  emit('scroll', e);
}

function onWheel(e: WheelEvent) {
  if (config.settings.grid.showFilmStrip && scroller.value) {
    const isHorizontal = config.settings.grid.previewPosition < 2;
    if (isHorizontal) {
      // If it's a vertical scroll (deltaY) and no horizontal scroll (deltaX),
      // translate it to horizontal scroll
      if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
        scroller.value.$el.scrollLeft += e.deltaY;
        e.preventDefault(); // Prevent default vertical scrolling behavior if any
      }
    }
  }
}

function onKeyDown(e: KeyboardEvent) {
  // Prevent default scrolling for arrow keys and spacebar
  if (['ArrowUp', 'ArrowDown', 'Space', ' '].includes(e.key)) {
    // Allow default behavior if typing in an input
    const target = e.target as HTMLElement;
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      return;
    }
    e.preventDefault();
  }
}

function scrollToItem(index: number, center = false) {
  if (!scroller.value) return;
  
  const el = scroller.value.$el;
  const displayIndex = hasHeaderItems.value ? fileIndexToDisplayIndex.value.get(index) : index;
  if (displayIndex === undefined) return;

  const renderedItem = center ? containerRef.value?.querySelector(`#item-${index}`) : null;
  if (renderedItem && !virtualScrollGeometry.value) {
    renderedItem.scrollIntoView({
      behavior: 'auto',
      block: 'center',
      inline: 'center',
    });
    return;
  }
  
  if (!config.settings.grid.showFilmStrip) {
    let itemTop = 0;
    let itemBottom = 0;

    if (virtualScrollGeometry.value && layoutGeometry.value[displayIndex]) {
      const box = layoutGeometry.value[displayIndex];
      itemTop = box.y;
      itemBottom = box.y + box.height;
    } else {
      // Normal Grid Logic
      const row = Math.floor(displayIndex / columnCount.value);
      itemTop = row * itemHeight.value;
      itemBottom = itemTop + itemHeight.value;
    }

    const scrollTop = el.scrollTop;
    const clientHeight = el.clientHeight;
    
    // Account for top and bottom padding
    const topPadding = 48; // pt-12 = 48px
    const bottomPadding = config.settings.showStatusBar ? 32 : 4; // pb-8 = 32px, pb-1 = 4px

    if (center) {
      el.scrollTop = Math.min(
        Math.max(0, topPadding + (itemTop + itemBottom) / 2 - clientHeight / 2),
        Math.max(0, el.scrollHeight - clientHeight),
      );
      return;
    }
    
    const viewportTop = scrollTop;
    const viewportBottom = scrollTop + clientHeight - (topPadding + bottomPadding);

    // Only scroll if the item is not fully visible
    const isFullyVisible = itemTop >= viewportTop && itemBottom <= viewportBottom;
    
    if (!isFullyVisible) {
      if (itemTop < viewportTop) {
        // Item is above viewport: align under the pt-12 chrome padding
        el.scrollTop = Math.max(0, itemTop - topPadding);
      } else if (itemBottom > viewportBottom) {
        // Item is below viewport, scroll to show it at the bottom (accounting for bottom padding)
        el.scrollTop = itemBottom - clientHeight + (topPadding + bottomPadding);
      }
    }
  } else {
    // Filmstrip mode: center the item
    const isHorizontal = config.settings.grid.previewPosition < 2;
    let itemPos = 0;
    let itemSizeValue = 0;

    if (layoutGeometry.value[displayIndex]) {
      const box = layoutGeometry.value[displayIndex];
      itemPos = isHorizontal ? box.x : box.y;
      itemSizeValue = isHorizontal ? box.width : box.height;
    } else {
      const itemSizeConst = isHorizontal ? filmStripItemSize.value : itemHeight.value;
      itemPos = displayIndex * itemSizeConst;
      itemSizeValue = itemSizeConst;
    }

    const itemCenter = itemPos + itemSizeValue / 2;
    const clientSize = isHorizontal ? el.clientWidth : el.clientHeight;
    
    // Calculate target scroll to center the item
    let targetScroll = itemCenter - clientSize / 2;
    
    // Clamp to bounds
    targetScroll = Math.max(0, targetScroll);
    const maxScroll = (isHorizontal ? el.scrollWidth : el.scrollHeight) - clientSize;
    targetScroll = Math.min(targetScroll, maxScroll);
    
    el.scrollTo({
      [isHorizontal ? 'left' : 'top']: targetScroll,
      behavior: center ? 'auto' : 'smooth'
    });
  }
}

function scrollToPosition(scrollTop: number) {
  if (scroller.value && !config.settings.grid.showFilmStrip) {
    scroller.value.$el.scrollTop = scrollTop;
  }
}

function getColumnCount() {
  return columnCount.value;
}

function getScrollTop() {
  return scroller.value ? scroller.value.$el.scrollTop : 0;
}

function centerItem(index: number) {
  if (index >= 0) scrollToItem(index, true);
}

function getNextItemIndex(currentIndex: number, direction: 'up' | 'down'): number {
  const style = config.settings.grid.style;
  const supportsGeometryNavigation = style === 2 || (!config.settings.grid.showFilmStrip && isGeometryGridStyle(style));
  if (!supportsGeometryNavigation || layoutGeometry.value.length === 0) {
    return -1;
  }

  const currentDisplayIndex = hasHeaderItems.value ? fileIndexToDisplayIndex.value.get(currentIndex) : currentIndex;
  if (currentDisplayIndex === undefined) return currentIndex;

  const currentBox = layoutGeometry.value[currentDisplayIndex];
  if (!currentBox) return currentIndex;

  const centerX = currentBox.x + currentBox.width / 2;
  const currentY = currentBox.y;
  
  // Find all items in the target direction
  let candidates: { index: number; box: Geometry; diffY: number }[] = [];

  layoutGeometry.value.forEach((box, displayIndex) => {
    const item = renderItems.value[displayIndex];
    if (hasHeaderItems.value && !item?.isDateFile) return;
    if (direction === 'down') {
      if (box.y > currentY + 1) { // +1 for tolerance
         candidates.push({ index: hasHeaderItems.value ? item.fileIndex : displayIndex, box, diffY: box.y - currentY });
      }
    } else {
      if (box.y < currentY - 1) { // -1 for tolerance
         candidates.push({ index: hasHeaderItems.value ? item.fileIndex : displayIndex, box, diffY: currentY - box.y });
      }
    }
  });

  if (candidates.length === 0) return currentIndex;

  // Find the closest row (smallest diffY)
  const minDiffY = Math.min(...candidates.map(c => c.diffY));
  
  // Filter candidates to only those in the closest row
  const rowCandidates = candidates.filter(c => Math.abs(c.diffY - minDiffY) < 5); // 5px tolerance

  // Find item with closest centerX
  let closestIndex = -1;
  let minDistX = Infinity;

  rowCandidates.forEach(c => {
    const boxCenterX = c.box.x + c.box.width / 2;
    const dist = Math.abs(boxCenterX - centerX);
    if (dist < minDistX) {
      minDistX = dist;
      closestIndex = c.index;
    }
  });

  return closestIndex !== -1 ? closestIndex : currentIndex;
}

function isDateHeader(item: any) {
  return Boolean(item?.isDateHeader);
}

function getFileItem(item: any) {
  return hasHeaderItems.value ? item?.file : item;
}

function getFileIndex(item: any, displayIndex: number) {
  return hasHeaderItems.value ? item?.fileIndex : displayIndex;
}

function getNearestFileIndexFromDisplayIndex(displayIndex: number) {
  for (let i = displayIndex; i < renderItems.value.length; i++) {
    if (renderItems.value[i]?.isDateFile) return renderItems.value[i].fileIndex;
  }
  for (let i = displayIndex - 1; i >= 0; i--) {
    if (renderItems.value[i]?.isDateFile) return renderItems.value[i].fileIndex;
  }
  return 0;
}

function getDateGroupSelectionState(item: any) {
  const key = String(item?.id ?? item?.label ?? '');
  const cached = dateGroupSelectionState.value.get(key);
  if (cached) return cached;
  // Fallback for callers outside selectMode / missing key
  const startIndex = Number(item?.startIndex ?? 0);
  const endIndex = Number(item?.endIndex ?? startIndex);
  const fileCount = Math.max(0, endIndex - startIndex);
  if (fileCount === 0) {
    return { allSelected: false, partialSelected: false };
  }
  let selectedCount = 0;
  for (let index = startIndex; index < endIndex; index++) {
    if (props.fileList[index]?.isSelected) selectedCount++;
  }
  return {
    allSelected: selectedCount === fileCount,
    partialSelected: selectedCount > 0 && selectedCount < fileCount,
  };
}

function toggleDateGroupSelection(item: any, selected?: boolean) {
  const state = getDateGroupSelectionState(item);
  emit('date-group-select', {
    startIndex: item.startIndex,
    endIndex: item.endIndex,
    selected: selected ?? !state.allSelected,
  });
}

defineExpose({
  getColumnCount,
  scrollToPosition,
  getScrollTop,
  centerItem,
  refreshLayout: updateLayout,
  getNextItemIndex
});

</script>

<style scoped>
</style>
