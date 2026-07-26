<template>

  <div
    :class="[
      'relative w-screen h-screen flex flex-col overflow-hidden bg-base-300 text-base-content/70',
      isFullScreen ? 'fixed top-0 left-0 z-50' : '',
    ]"
    @mousemove="handleRootMouseMove"
    @mouseleave="handleRootMouseLeave"
  >

    <div
      ref="viewerContainer"
      :class="[
        'relative flex-1 flex justify-center items-center overflow-hidden select-none',
        showEmbeddedStatusBar ? 'pb-8' : '',
      ]"
    >
      <template v-if="!isSplit && fileIndex >= 0">
        <MediaViewer
          ref="mediaViewerRef"
          :mode="2"
          :isFullScreen="isFullScreen"
          :file="fileInfo"
          :nextFilePath="nextFilePath"
          :hasPrevious="fileIndex > 0"
          :hasNext="fileIndex < fileCount - 1"
          :fileIndex="fileIndex"
          :fileCount="fileCount"
          :isSlideShow="isSlideShow"
          :canSlideShow="true"
          :slideShowIntervalIndex="slideShowIntervalIndex"
          :canInteract="true"
          :imageScale="imageScale"
          :imageMinScale="imageMinScale"
          :imageMaxScale="imageMaxScale"
          :isZoomFit="isZoomFit"
          :isSplit="isSplit"
          :splitCount="splitCount"
          :isSyncViewport="isSyncViewport"
          :showWindowControls="true"
          @prev="clickPrev()"
          @next="clickNext()"
          @toggle-slide-show="clickSlideShow()"
          @update:slideShowIntervalIndex="slideShowIntervalIndex = $event"
          @item-action="handleItemAction"
          @scale="clickScale"
          @update:isZoomFit="(val) => handleZoomFitUpdate(val, 'left')"
          @media-dblclick="toggleZoomFit()"
          @toggle-full-screen="toggleNativeFullScreen"
          @close="closeWindow"
          @slideshow-next="handleSlideshowNext"
        />

        <!--
        <div v-if="config.settings.showComment && fileInfo?.comments?.length > 0" 
          class="absolute flex m-2 p-2 bottom-0 left-0 right-0 text-sm bg-base-100/30 rounded-box select-text" 
        >
          <IconComment class="t-icon-size-sm shrink-0 mr-2"></IconComment>
          {{ fileInfo?.comments }}
        </div>
        -->
      </template>

            <template v-else-if="isSplit && fileIndex >= 0">
        <div class="w-full h-full flex flex-col">
          <MediaViewer
            :mode="2"
            :toolbarOnly="true"
            :showToolbar="true"
            :showWindowControls="true"
            :isFullScreen="isFullScreen"
            :file="activePaneFileInfo"
            :nextFilePath="activePaneNextPath"
            :hasPrevious="activePaneIndex > 0"
            :hasNext="activePaneIndex < fileCount - 1"
            :fileIndex="activePaneIndex"
            :fileCount="fileCount"
            :isSlideShow="false"
            :canSlideShow="false"
            :canInteract="true"
            :imageScale="activePaneScale.scale"
            :imageMinScale="activePaneScale.min"
            :imageMaxScale="activePaneScale.max"
            :isZoomFit="getZoomFitByPane(activePane)"
            :isSplit="isSplit"
            :splitCount="splitCount"
            :isSyncViewport="isSyncViewport"
            :forceToolbarVisible="isFullScreen && splitToolbarVisible"
            @prev="clickPrev(activePane)"
            @next="clickNext(activePane)"
            @toggle-slide-show="clickSlideShow(activePane)"
            @item-action="handleItemAction"
            @scale="clickScale($event, activePane)"
            @update:isZoomFit="(val) => handleZoomFitUpdate(val, activePane)"
            @toggle-full-screen="toggleNativeFullScreen"
            @close="closeWindow"
            @slideshow-next="handleSlideshowNext"
          />

          <div
            class="flex-1 min-h-0 grid"
            :class="splitCount >= 4 ? 'grid-cols-2 grid-rows-2' : 'grid-cols-2 grid-rows-1'"
          >
            <div
              v-for="pane in visiblePanes"
              :key="pane"
              class="relative min-h-0 min-w-0 border border-base-content/10"
              @mousedown="setActivePane(pane)"
            >
              <IconDot
                v-if="activePane === pane"
                class="absolute right-2 top-2 z-90 t-icon-size-sm text-primary pointer-events-none"
              />
              <MediaViewer
                :ref="(el) => setPaneViewerRef(pane, el)"
                :mode="2"
                :isFullScreen="isFullScreen"
                :file="getFileInfoByPane(pane)"
                :nextFilePath="getNextPathByPane(pane)"
                :hasPrevious="getIndexByPane(pane) > 0"
                :hasNext="getIndexByPane(pane) < fileCount - 1"
                :fileIndex="getIndexByPane(pane)"
                :fileCount="fileCount"
                :isSlideShow="false"
                :canSlideShow="false"
                :canInteract="activePane === pane"
                :showToolbar="false"
                :imageScale="getScaleByPane(pane).scale"
                :imageMinScale="getScaleByPane(pane).min"
                :imageMaxScale="getScaleByPane(pane).max"
                :isZoomFit="getZoomFitByPane(pane)"
                @prev="clickPrev(pane)"
                @next="clickNext(pane)"
                @toggle-slide-show="clickSlideShow(pane)"
                @item-action="handleItemAction"
                @scale="clickScale($event, pane)"
                @update:isZoomFit="(val) => handleZoomFitUpdate(val, pane)"
                @media-dblclick="toggleZoomFit(pane)"
                @viewport-change="handleViewportChange($event, pane)"
                @toggle-full-screen="toggleNativeFullScreen"
                @close="closeWindow"
                @slideshow-next="handleSlideshowNext"
              />
            </div>
          </div>
        </div>
      </template>

      <!-- no image selected -->
      <div v-else class="flex flex-col items-center justify-center w-full h-full text-base-content/30">
        <IconSearch class="w-8 h-8" />
        <span>{{ $t('tooltip.not_found.files') }}</span>
      </div>
    </div>

    <div
      v-if="showEmbeddedStatusBar"
      class="absolute bottom-0 left-0 right-0 z-30 h-8 bg-base-300/80 backdrop-blur-md"
    >
      <template v-if="!isSplit">
        <StatusBar
          :selected-file="fileInfo"
          :selected-item-index="fileIndex"
          :total-file-count="fileCount"
          :total-file-size="fileInfo?.size || 0"
          :image-scale="imageDisplayScale"
          :show-scale="true"
          :is-embedded="true"
        />
      </template>
      <template v-else>
        <div class="h-8 flex">
          <div class="w-1/2 border-r border-base-content/10">
            <StatusBar
              :selected-file="fileInfo"
              :selected-item-index="fileIndex"
              :total-file-count="fileCount"
              :total-file-size="fileInfo?.size || 0"
              :image-scale="imageDisplayScale"
              :show-scale="true"
              :is-embedded="true"
            />
          </div>
          <div class="w-1/2">
            <StatusBar
              :selected-file="rightFileInfo"
              :selected-item-index="rightFileIndex"
              :total-file-count="fileCount"
              :total-file-size="rightFileInfo?.size || 0"
              :image-scale="rightImageDisplayScale"
              :show-scale="true"
              :is-embedded="true"
            />
          </div>
        </div>
      </template>
    </div>

    <TaggingDialog
      v-if="showTaggingDialog"
      :fileIds="taggingFileIds"
      @ok="updateFileHasTags"
      @cancel="showTaggingDialog = false"
    />

    <MessageBox
      v-if="showCommentMsgbox"
      :title="$t('msgbox.edit_comment.title')"
      :showInput="true"
      :inputText="activeFileInfo?.comments ?? ''"
      :inputPlaceholder="$t('msgbox.edit_comment.placeholder')"
      :multiLine="true"
      :OkText="$t('msgbox.ok')"
      :cancelText="$t('msgbox.cancel')"
      @ok="onEditComment"
      @cancel="showCommentMsgbox = false"
    />

    <PluginActionDialog
      v-if="pluginActionDialog.show"
      :plugin="pluginActionDialog.plugin"
      :capability="pluginActionDialog.capability"
      :source-file="pluginActionDialog.file"
      :busy="pluginActionDialog.busy"
      :error="pluginActionDialog.error"
      :error-code="pluginActionDialog.errorCode"
      :error-domain="pluginActionDialog.errorDomain"
      :error-details="pluginActionDialog.errorDetails"
      :diagnostics="pluginActionDialog.diagnostics"
      :logs="pluginActionDialog.logs"
      :host-environment="aiPluginHostEnvironment"
      :stage="pluginActionDialog.stage"
    :task-status="pluginActionDialog.taskStatus"
      :task-progress="pluginActionDialog.taskProgress"
      :task-message="pluginActionDialog.taskMessage"
      @run="runPluginAction"
      @cancel="closePluginActionDialog"
      @cancel-task="cancelPluginAction"
    />

  </div>

</template>

<script setup lang="ts">

import { ref, watch, computed, onMounted, onUnmounted } from 'vue';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit, listen } from '@tauri-apps/api/event';
import { useI18n } from 'vue-i18n';
import { useUIStore } from '@/stores/uiStore';
import { usePluginStore } from '@/stores/pluginStore';
import { useToast } from '@/common/toast';
import { config } from '@/common/config';
import { isWin, isMac, isLinux, setTheme, getSlideShowInterval, SCALE_VALUES, cycleViewerBackgroundMode } from '@/common/utils';
import { matchesShortcut, ShortcutActionId, ShortcutPlatform } from '@/common/shortcuts';
import {
  editFileComment,
  getFileInfo,
  getAiPluginDiagnostics,
  getAiPluginHostEnvironment,
  getAiPluginLogs,
  getAiPluginTask,
  grantAiPluginPermissions,
  getTagsForFile,
  importFile,
  invokeAiPluginCapability,
  cancelAiPluginTask,
  setFileFavorite,
  setFileRating,
  setFileRotate,
  startAiPlugin,
} from '@/common/api';

import MediaViewer from '@/components/MediaViewer.vue';
import MessageBox from '@/components/MessageBox.vue';
import TButton from '@/components/TButton.vue';
import StatusBar from '@/components/StatusBar.vue';
import TaggingDialog from '@/components/TaggingDialog.vue';
import PluginActionDialog from '@/components/PluginActionDialog.vue';
import {
  AiPluginHostEnvironment,
  buildPluginPermissionGrantRequest,
  missingPluginPermissionFlags,
  pluginAllowedDomains,
  pluginPermissions,
  pluginStartRequest,
} from '@/common/pluginRuntime';

import { 
  IconSearch,
  IconComment,
  IconDot,
 } from '@/common/icons';

/// i18n
const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[locale.value] as any);
const uiStore = useUIStore();
const pluginStore = usePluginStore();
const toast = useToast();

const appWindow = getCurrentWebviewWindow()
const shortcutPlatform: ShortcutPlatform = isMac ? 'mac' : (isLinux ? 'linux' : 'windows');

// input parameters
const fileId = ref(0);       // File ID
const fileIndex = ref(0);       // Index of the current file
const fileCount = ref(0);       // Total number of files

const fileInfo = ref<any>(null);
const nextFilePath = ref('');
const iconRotate = ref(0);      // icon rotation angle
const isTransitionDisabled = ref(true);

type ViewerPane = 'left' | 'right' | 'bottomLeft' | 'bottomRight';
const ALL_PANES: ViewerPane[] = ['left', 'right', 'bottomLeft', 'bottomRight'];

const mediaViewerRef = ref<any>(null); // left media viewer reference
const rightMediaViewerRef = ref<any>(null); // right media viewer reference (split mode)
const bottomLeftMediaViewerRef = ref<any>(null);
const bottomRightMediaViewerRef = ref<any>(null);
const isFullScreen = ref(false);
const isZoomFit = ref(true);
const rightIsZoomFit = ref(true);
const bottomLeftIsZoomFit = ref(true);
const bottomRightIsZoomFit = ref(true);
/** 1 | 2 | 4 comparison grid. isSplit mirrors splitCount > 1 for legacy bindings. */
const splitCount = ref<1 | 2 | 4>(1);
const isSplit = computed(() => splitCount.value > 1);
const activePane = ref<ViewerPane>('left');
const isSyncViewport = ref(false);
const isCompareModeSession = ref(false);
const syncingPane = ref<ViewerPane | ''>('');
const animateSyncOnce = ref(false);
const splitToolbarVisible = ref(false);

const isSlideShow = ref(false);     // Slide show state
const slideShowIntervalIndex = ref(Number(config.settings.slideShowInterval ?? 0));
let timer: NodeJS.Timeout | null = null;  // Timer for slide show

const imageScale = ref(1);          // Image scale
const imageDisplayScale = ref(1);   // User-facing image scale
const imageMinScale = ref(0);       // Minimum image scale
const imageMaxScale = ref(10);      // Maximum image scale
const rightImageScale = ref(1);     // Right image scale
const rightImageDisplayScale = ref(1); // User-facing right image scale
const rightImageMinScale = ref(0);  // Right minimum scale
const rightImageMaxScale = ref(10); // Right maximum scale
const bottomLeftImageScale = ref(1);
const bottomLeftImageDisplayScale = ref(1);
const bottomLeftImageMinScale = ref(0);
const bottomLeftImageMaxScale = ref(10);
const bottomRightImageScale = ref(1);
const bottomRightImageDisplayScale = ref(1);
const bottomRightImageMinScale = ref(0);
const bottomRightImageMaxScale = ref(10);

const rightFileId = ref(0);         // Right file ID
const rightFileIndex = ref(-1);     // Right file index
const rightFileInfo = ref<any>(null);
const rightNextFilePath = ref('');
const bottomLeftFileId = ref(0);
const bottomLeftFileIndex = ref(-1);
const bottomLeftFileInfo = ref<any>(null);
const bottomLeftNextFilePath = ref('');
const bottomRightFileId = ref(0);
const bottomRightFileIndex = ref(-1);
const bottomRightFileInfo = ref<any>(null);
const bottomRightNextFilePath = ref('');
const showTaggingDialog = ref(false);
const showCommentMsgbox = ref(false);
const taggingFileIds = ref<number[]>([]);
const aiPluginHostEnvironment = ref<AiPluginHostEnvironment | null>(null);
const pluginActionDialog = ref({
  show: false,
  busy: false,
  plugin: null as any,
  capability: null as any,
  file: null as any,
  error: '',
  errorCode: '',
  errorDomain: '',
  errorDetails: null as any,
  diagnostics: null as any,
  logs: [] as any[],
  taskId: '',
  stage: '',
  taskStatus: '',
  taskProgress: 0,
  taskMessage: '',
});

let unlistenImg: () => void;
let unlistenGridView: () => void;
let unlistenFilesDeleted: (() => void) | null = null;

const activeFileInfo = computed(() => getFileInfoByPane(getActivePane()));

const activeFileId = computed(() => getFileIdByPane(getActivePane()));
const showEmbeddedStatusBar = computed(() => config.settings.showStatusBar && !isFullScreen.value);

function normalizeScale(value: number) {
  return SCALE_VALUES.find((item) => item === Number(value)) ?? 1;
}

function applyViewerScale(scale: number) {
  const normalizedScale = normalizeScale(scale);
  document.documentElement.style.fontSize = `${normalizedScale * 16}px`;
}

function handleRootMouseMove(event: MouseEvent) {
  if (!isFullScreen.value || !isSplit.value) {
    splitToolbarVisible.value = false;
    return;
  }
  const root = event.currentTarget as HTMLElement | null;
  if (!root) return;
  const rect = root.getBoundingClientRect();
  splitToolbarVisible.value = event.clientY - rect.top < 60;
}

function handleRootMouseLeave() {
  splitToolbarVisible.value = false;
}

onMounted(async() => {
  appWindow.setFocus();
  applyViewerScale(Number(config.settings.scale || 1));
  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('resize', handleResize);

  const urlParams = new URLSearchParams(window.location.search);
  
  fileId.value    = Number(urlParams.get('fileId'));
  fileIndex.value = Number(urlParams.get('fileIndex'));
  fileCount.value = Number(urlParams.get('fileCount'));
  nextFilePath.value = decodeURIComponent(urlParams.get('nextFilePath') || '');
  const initialRightFileId = Number(urlParams.get('rightFileId') || '0');
  const initialRightFileIndex = Number(urlParams.get('rightFileIndex') || '-1');
  rightNextFilePath.value = decodeURIComponent(urlParams.get('rightNextFilePath') || '');
  const forceSplit = urlParams.get('forceSplit') === '1';
  isCompareModeSession.value = urlParams.get('compareMode') === '1';

  const preferredSplit = Number(config.imageViewer?.splitCount) || (config.imageViewer?.isSplit ? 2 : 1);
  splitCount.value = forceSplit ? 2 : (preferredSplit === 4 ? 4 : preferredSplit === 2 ? 2 : 1);
  if (isCompareModeSession.value) {
    splitCount.value = 2;
    isSyncViewport.value = true;
  } else {
    isSyncViewport.value = splitCount.value > 1 ? !!config.imageViewer?.isSyncViewport : false;
  }
  rightFileId.value = initialRightFileId > 0 ? initialRightFileId : 0;
  rightFileIndex.value = initialRightFileId > 0 ? initialRightFileIndex : -1;
  rightFileInfo.value = null;
  rightIsZoomFit.value = true;
  activePane.value = 'left';
  isFullScreen.value = !!config.imageViewer?.isFullScreen;

  // Listen 
  unlistenImg = await listen('update-img', async (event: any) => {
    if(uiStore.inputStack.length > 0) {
      return;
    }

    const rawPane = String(event.payload?.pane || 'left');
    const pane: ViewerPane = (['left', 'right', 'bottomLeft', 'bottomRight'].includes(rawPane)
      ? rawPane
      : 'left') as ViewerPane;
    if (typeof event.payload?.compareMode === 'boolean') {
      isCompareModeSession.value = !!event.payload.compareMode;
    }
    if (typeof event.payload?.forceSplit === 'boolean') {
      splitCount.value = event.payload.forceSplit ? 2 : 1;
      if (splitCount.value > 1 && typeof event.payload?.forceSyncViewport === 'boolean') {
        isSyncViewport.value = !!event.payload.forceSyncViewport;
      }
      if (splitCount.value <= 1) clearExtraPanes();
    }
    if (typeof event.payload?.splitCount === 'number') {
      const n = Number(event.payload.splitCount);
      splitCount.value = n >= 4 ? 4 : n >= 2 ? 2 : 1;
    }
    if (event.payload?.resetSplit) {
      if (isCompareModeSession.value) {
        splitCount.value = 2;
        isSyncViewport.value = true;
      } else {
        const pref = Number(config.imageViewer?.splitCount) || (config.imageViewer?.isSplit ? 2 : 1);
        splitCount.value = pref === 4 ? 4 : pref === 2 ? 2 : 1;
        isSyncViewport.value = splitCount.value > 1 ? !!config.imageViewer?.isSyncViewport : false;
      }
      if (splitCount.value <= 1) clearExtraPanes();
    }

    fileCount.value = Number(event.payload.fileCount);
    applyPaneUpdate(pane, event.payload);
  });


  unlistenGridView = await listen('message-from-content', (event) => {
    const { message, fileId: targetFileId, changes } = event.payload as any;
    switch (message) {
      case 'rotate':
        if (targetFileId === fileId.value) {
          mediaViewerRef.value?.rotateRight();
          iconRotate.value += 90;
          if (fileInfo.value) {
            fileInfo.value.rotate = (fileInfo.value.rotate || 0) + 90;
          }
        } else if (targetFileId === rightFileId.value) {
          rightMediaViewerRef.value?.rotateRight();
          if (rightFileInfo.value) {
            rightFileInfo.value.rotate = (rightFileInfo.value.rotate || 0) + 90;
          }
        }
        break;
      case 'update-file-meta':
        if (targetFileId === fileId.value && fileInfo.value) {
          Object.assign(fileInfo.value, changes || {});
        }
        if (targetFileId === rightFileId.value && rightFileInfo.value) {
          Object.assign(rightFileInfo.value, changes || {});
        }
        break;
      default:
        break;
    }
  });

  unlistenFilesDeleted = await listen('files-deleted', (event: any) => {
    const deletedIds = Array.isArray(event?.payload?.fileIds)
      ? event.payload.fileIds.map((id: any) => Number(id)).filter((id: number) => id > 0)
      : [];
    const nextCount = Number(event?.payload?.fileCount);
    if (!Number.isNaN(nextCount) && nextCount >= 0) {
      fileCount.value = nextCount;
    }

    if (fileCount.value <= 0) {
      fileId.value = 0;
      fileIndex.value = -1;
      nextFilePath.value = '';
      clearExtraPanes();
      return;
    }

    const leftDeleted = deletedIds.includes(fileId.value);
    if (leftDeleted || fileIndex.value >= fileCount.value) {
      const targetIndex = Math.max(0, Math.min(fileIndex.value, fileCount.value - 1));
      requestFileAtIndex(targetIndex, 'left');
    }

    if (isSplit.value) {
      for (const pane of visiblePanes.value) {
        if (pane === 'left') continue;
        const paneId = getFileIdByPane(pane);
        const paneIndex = getIndexByPane(pane);
        const deleted = paneId > 0 && deletedIds.includes(paneId);
        if (deleted || paneIndex >= fileCount.value) {
          const fallbackBase = paneIndex >= 0 ? paneIndex : (fileIndex.value + 1);
          const targetIndex = Math.max(0, Math.min(fallbackBase, fileCount.value - 1));
          requestFileAtIndex(targetIndex, pane);
        }
      }
    }
  });

  setTimeout(() => {
    isTransitionDisabled.value = false;
  }, 500);

  await handleResize();
  
  // Show window after mount (if it was created hidden)
  try {
    await appWindow.show();
  } catch (e) {
    // Window might already be visible, ignore error
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('resize', handleResize);
  document.documentElement.style.fontSize = '';
  clearSlideShowTimer();
  
  // unlisten
  unlistenImg();
  unlistenGridView();
  if (unlistenFilesDeleted) unlistenFilesDeleted();
});

// Handle keyboard shortcuts
function handleKeyDown(event: KeyboardEvent) {
  if(uiStore.inputStack.length > 0) {
    return;
  }

  // Disable keyboard events during slideshow except close and toggle slideshow.
  if (
    isSlideShow.value &&
    !matchesShortcut('view.close', event, shortcutPlatform) &&
    !matchesShortcut('slideshow.toggle', event, shortcutPlatform)
  ) {
    return;
  }

  const ratingShortcut = getMatchedRating(event);
  if (ratingShortcut !== null) {
    event.preventDefault();
    void setCurrentFileRating(ratingShortcut, getActiveFilePane());
    return;
  }

  if (matchesShortcut('slideshow.toggle', event, shortcutPlatform)) {
    event.preventDefault();
    clickSlideShow(getActiveFilePane());
    return;
  }

  if (matchesShortcut('meta.favorite', event, shortcutPlatform)) {
    event.preventDefault();
    void toggleFavorite(getActiveFilePane());
    return;
  }

  if (matchesShortcut('meta.tag', event, shortcutPlatform)) {
    event.preventDefault();
    clickTag(getActiveFilePane());
    return;
  }

  if (matchesShortcut('meta.comment', event, shortcutPlatform)) {
    event.preventDefault();
    openCommentEditor(getActiveFilePane());
    return;
  }

  if (matchesShortcut('meta.rotate', event, shortcutPlatform)) {
    event.preventDefault();
    void clickRotate(getActiveFilePane());
    return;
  }

  if (matchesShortcut('view.togglePane', event, shortcutPlatform) && isSplit.value) {
    event.preventDefault();
    {
      const panes = visiblePanes.value;
      const idx = panes.indexOf(activePane.value);
      setActivePane(panes[(idx + 1 + panes.length) % panes.length]);
    }
    return;
  }

  if (matchesShortcut('view.cycleBackground', event, shortcutPlatform)) {
    event.preventDefault();
    cycleViewerBackground();
    return;
  }

  const matchedAction = getMatchedViewAction(event);
  if (matchedAction) {
    event.preventDefault();
    viewActions[matchedAction]?.();
  }
}

function cycleViewerBackground() {
  config.setMediaViewerBackgroundMode(cycleViewerBackgroundMode(config.mediaViewer?.backgroundMode));
}

const ratingActions: Array<{ actionId: ShortcutActionId; rating: number }> = [
  { actionId: 'meta.rating.clear', rating: 0 },
  { actionId: 'meta.rating.one', rating: 1 },
  { actionId: 'meta.rating.two', rating: 2 },
  { actionId: 'meta.rating.three', rating: 3 },
  { actionId: 'meta.rating.four', rating: 4 },
  { actionId: 'meta.rating.five', rating: 5 },
];

function getMatchedRating(event: KeyboardEvent) {
  const match = ratingActions.find(({ actionId }) => matchesShortcut(actionId, event, shortcutPlatform));
  return match ? match.rating : null;
}

const viewActions: Partial<Record<ShortcutActionId, () => void>> = {
  'view.previous': () => clickPrev(getActivePane()),
  'view.next': () => clickNext(getActivePane()),
  'view.first': () => clickHome(getActivePane()),
  'view.last': () => clickEnd(getActivePane()),
  'view.zoomIn': () => clickZoomIn(getActivePane()),
  'view.zoomOut': () => clickZoomOut(getActivePane()),
  'view.zoomInDirectional': () => clickZoomIn(getActivePane()),
  'view.zoomOutDirectional': () => clickZoomOut(getActivePane()),
  'view.zoomFit': () => toggleZoomFit(getActivePane()),
  'view.close': () => closeWindow(),
};

const viewActionOrder: ShortcutActionId[] = [
  'view.previous',
  'view.next',
  'view.first',
  'view.last',
  'view.zoomIn',
  'view.zoomOut',
  'view.zoomInDirectional',
  'view.zoomOutDirectional',
  'view.zoomFit',
  'view.close',
];

function getMatchedViewAction(event: KeyboardEvent) {
  if (isMac && event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    if (event.key === 'ArrowUp') return 'view.first';
    if (event.key === 'ArrowDown') return 'view.last';
  }
  return viewActionOrder.find((actionId) => matchesShortcut(actionId, event, shortcutPlatform));
}

const visiblePanes = computed<ViewerPane[]>(() => {
  if (splitCount.value >= 4) return ALL_PANES;
  if (splitCount.value >= 2) return ['left', 'right'];
  return ['left'];
});

const activePaneFileInfo = computed(() => getFileInfoByPane(activePane.value));
const activePaneIndex = computed(() => getIndexByPane(activePane.value));
const activePaneNextPath = computed(() => getNextPathByPane(activePane.value));
const activePaneScale = computed(() => getScaleByPane(activePane.value));

function getActivePane(): ViewerPane {
  return isSplit.value ? activePane.value : 'left';
}

function setActivePane(pane: ViewerPane) {
  activePane.value = pane;
}

function setPaneViewerRef(pane: ViewerPane, el: any) {
  const r = el && '$el' in (el as any) ? el : el;
  if (pane === 'left') mediaViewerRef.value = r;
  else if (pane === 'right') rightMediaViewerRef.value = r;
  else if (pane === 'bottomLeft') bottomLeftMediaViewerRef.value = r;
  else bottomRightMediaViewerRef.value = r;
}

function getViewerRef(pane: ViewerPane) {
  if (pane === 'right') return rightMediaViewerRef.value;
  if (pane === 'bottomLeft') return bottomLeftMediaViewerRef.value;
  if (pane === 'bottomRight') return bottomRightMediaViewerRef.value;
  return mediaViewerRef.value;
}

function getFileInfoByPane(pane: ViewerPane = 'left') {
  if (pane === 'right') return rightFileInfo.value;
  if (pane === 'bottomLeft') return bottomLeftFileInfo.value;
  if (pane === 'bottomRight') return bottomRightFileInfo.value;
  return fileInfo.value;
}

function getFileIdByPane(pane: ViewerPane = 'left') {
  if (pane === 'right') return rightFileId.value;
  if (pane === 'bottomLeft') return bottomLeftFileId.value;
  if (pane === 'bottomRight') return bottomRightFileId.value;
  return fileId.value;
}

function getIndexByPane(pane: ViewerPane = 'left') {
  if (pane === 'right') return rightFileIndex.value;
  if (pane === 'bottomLeft') return bottomLeftFileIndex.value;
  if (pane === 'bottomRight') return bottomRightFileIndex.value;
  return fileIndex.value;
}

function getNextPathByPane(pane: ViewerPane = 'left') {
  if (pane === 'right') return rightNextFilePath.value;
  if (pane === 'bottomLeft') return bottomLeftNextFilePath.value;
  if (pane === 'bottomRight') return bottomRightNextFilePath.value;
  return nextFilePath.value;
}

function getScaleByPane(pane: ViewerPane) {
  if (pane === 'right') {
    return { scale: rightImageScale.value, min: rightImageMinScale.value, max: rightImageMaxScale.value, display: rightImageDisplayScale.value };
  }
  if (pane === 'bottomLeft') {
    return { scale: bottomLeftImageScale.value, min: bottomLeftImageMinScale.value, max: bottomLeftImageMaxScale.value, display: bottomLeftImageDisplayScale.value };
  }
  if (pane === 'bottomRight') {
    return { scale: bottomRightImageScale.value, min: bottomRightImageMinScale.value, max: bottomRightImageMaxScale.value, display: bottomRightImageDisplayScale.value };
  }
  return { scale: imageScale.value, min: imageMinScale.value, max: imageMaxScale.value, display: imageDisplayScale.value };
}

function clearExtraPanes() {
  rightFileId.value = 0;
  rightFileIndex.value = -1;
  rightFileInfo.value = null;
  rightNextFilePath.value = '';
  rightIsZoomFit.value = true;
  bottomLeftFileId.value = 0;
  bottomLeftFileIndex.value = -1;
  bottomLeftFileInfo.value = null;
  bottomLeftNextFilePath.value = '';
  bottomLeftIsZoomFit.value = true;
  bottomRightFileId.value = 0;
  bottomRightFileIndex.value = -1;
  bottomRightFileInfo.value = null;
  bottomRightNextFilePath.value = '';
  bottomRightIsZoomFit.value = true;
}

function applyPaneUpdate(pane: ViewerPane, payload: any) {
  const id = Number(payload.fileId);
  const index = Number(payload.fileIndex);
  const next = payload.nextFilePath || '';
  if (pane === 'right') {
    rightFileId.value = id;
    rightFileIndex.value = index;
    rightNextFilePath.value = next;
  } else if (pane === 'bottomLeft') {
    bottomLeftFileId.value = id;
    bottomLeftFileIndex.value = index;
    bottomLeftNextFilePath.value = next;
  } else if (pane === 'bottomRight') {
    bottomRightFileId.value = id;
    bottomRightFileIndex.value = index;
    bottomRightNextFilePath.value = next;
  } else {
    fileId.value = id;
    fileIndex.value = index;
    nextFilePath.value = next;
  }
}

function haveMatchingSyncableMedia() {
  const types = visiblePanes.value
    .map((p) => getFileInfoByPane(p)?.file_type)
    .filter((t) => t != null);
  if (types.length < 2) return false;
  const isImageType = (t: number) => t === 1 || t === 3;
  const first = types[0];
  if (isImageType(first)) return types.every((t) => isImageType(t));
  if (first === 2) return types.every((t) => t === 2);
  return false;
}

function syncViewportFrom(pane: ViewerPane, animate = false) {
  if (!isSplit.value || !isSyncViewport.value) return;
  if (!haveMatchingSyncableMedia()) return;

  const sourceRef = getViewerRef(pane);
  const viewport = sourceRef?.getViewportState?.();
  if (!viewport) return;

  syncingPane.value = pane;
  for (const target of visiblePanes.value) {
    if (target === pane) continue;
    getViewerRef(target)?.applyViewportState?.(viewport, !animate);
  }
  syncingPane.value = '';
}

function handleViewportChange(viewport: any, pane: ViewerPane) {
  if (!isSplit.value || !isSyncViewport.value) return;
  if (syncingPane.value) return;
  if (!haveMatchingSyncableMedia()) return;

  const shouldAnimate = animateSyncOnce.value;
  animateSyncOnce.value = false;
  syncingPane.value = pane;
  for (const target of visiblePanes.value) {
    if (target === pane) continue;
    getViewerRef(target)?.applyViewportState?.(viewport, !shouldAnimate);
  }
  syncingPane.value = '';
}

function getZoomFitByPane(pane: ViewerPane) {
  if (pane === 'right') return rightIsZoomFit.value;
  if (pane === 'bottomLeft') return bottomLeftIsZoomFit.value;
  if (pane === 'bottomRight') return bottomRightIsZoomFit.value;
  return isZoomFit.value;
}

function setZoomFitByPane(pane: ViewerPane, val: boolean) {
  if (pane === 'right') rightIsZoomFit.value = val;
  else if (pane === 'bottomLeft') bottomLeftIsZoomFit.value = val;
  else if (pane === 'bottomRight') bottomRightIsZoomFit.value = val;
  else isZoomFit.value = val;
}

function handleZoomFitUpdate(val: boolean, pane: ViewerPane) {
  setActivePane(pane);
  setZoomFitByPane(pane, val);
  if (isSplit.value && isSyncViewport.value && haveMatchingSyncableMedia()) {
    animateSyncOnce.value = true;
  }
}

// Handle resize event
const handleResize = async () => {
  if(isMac) {
    const checkFullScreen = async () => {
      isFullScreen.value = await appWindow.isFullscreen();
    };
    await checkFullScreen();
    setTimeout(checkFullScreen, 600); 
  }
};

/// watch appearance
watch(() => config.settings.appearance, (newAppearance) => {
  setTheme(newAppearance, newAppearance === 0 ? config.settings.lightTheme : config.settings.darkTheme);
});

/// watch light theme
watch(() => config.settings.lightTheme, (newLightTheme) => {
  setTheme(config.settings.appearance, newLightTheme);
});

/// watch dark theme
watch(() => config.settings.darkTheme, (newDarkTheme) => {
  setTheme(config.settings.appearance, newDarkTheme);
});

watch(() => Number(config.settings.scale || 1), (newScale) => {
  applyViewerScale(newScale);
});

// watch language
watch(() => config.settings.language, (newLanguage) => {
    locale.value = newLanguage; // update locale based on config.settings.language
});

// watch full screen
watch(() => isFullScreen.value, async (newFullScreen) => {
  if (!config.imageViewer) {
    (config as any).imageViewer = { isSplit: false, isSyncViewport: false, isFullScreen: false };
  }
  config.imageViewer.isFullScreen = newFullScreen;

  if(isWin) {
    await appWindow.setFullscreen(newFullScreen);
    await appWindow.setResizable(!newFullScreen);
    // await appWindow.setDecorations(false);
  } else if (isMac) {
      if (newFullScreen !== await appWindow.isFullscreen()) {
        await appWindow.setFullscreen(newFullScreen);
    }
  }
}); 

// watch file changed
watch(() => fileId.value, async () => {
  fileInfo.value = await getFileInfo(fileId.value);
  iconRotate.value = fileInfo.value.rotate || 0;
  if (isSlideShow.value) {
    scheduleNextSlide();
  }
});

watch(() => rightFileId.value, async () => {
  if (rightFileId.value > 0) {
    rightFileInfo.value = await getFileInfo(rightFileId.value);
  } else {
    rightFileInfo.value = null;
  }
});

watch(() => bottomLeftFileId.value, async () => {
  if (bottomLeftFileId.value > 0) {
    bottomLeftFileInfo.value = await getFileInfo(bottomLeftFileId.value);
  } else {
    bottomLeftFileInfo.value = null;
  }
});

watch(() => bottomRightFileId.value, async () => {
  if (bottomRightFileId.value > 0) {
    bottomRightFileInfo.value = await getFileInfo(bottomRightFileId.value);
  } else {
    bottomRightFileInfo.value = null;
  }
});

// watch file index
watch(() => fileIndex.value, async (newIndex) => {
  if(newIndex === -1) {
    stopSlideShow();
    iconRotate.value = 0; // reset rotation
  } 
});

// Check if current file is a video
function isCurrentFileVideo() {
  return fileInfo.value?.file_type === 2;
}

function clearSlideShowTimer() {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
}

function advanceSlideShow() {
  if (fileCount.value <= 0) return;

  if (fileIndex.value >= fileCount.value - 1) {
    requestFileAtIndex(0, 'left');
    return;
  }
  requestFileAtIndex(fileIndex.value + 1, 'left');
}

// Schedule next slide based on file type
function scheduleNextSlide() {
  clearSlideShowTimer();

  if (!isSlideShow.value) return;

  // If current file is video, don't set timer - video's ended event will trigger next
  if (isCurrentFileVideo()) {
    return;
  }

  const interval = getSlideShowInterval(slideShowIntervalIndex.value) * 1000;
  timer = setTimeout(() => {
    advanceSlideShow();
  }, interval);
}

function startSlideShow() {
  scheduleNextSlide();
}

function stopSlideShow() {
  isSlideShow.value = false;
  clearSlideShowTimer();
}

// Called when video ends in slideshow mode
function handleSlideshowNext() {
  if (isSlideShow.value) {
    advanceSlideShow();
  }
}

watch(() => slideShowIntervalIndex.value, () => {
  if (isSlideShow.value && !isCurrentFileVideo()) {
    scheduleNextSlide();
  }
});

function ensureExtraPanesLoaded() {
  if (splitCount.value <= 1) return;
  if (fileCount.value <= 0 || fileIndex.value < 0) return;

  const need = (pane: ViewerPane, offset: number) => {
    const idx = getIndexByPane(pane);
    const id = getFileIdByPane(pane);
    if (idx >= 0 && id > 0) return;
    const nextIndex = Math.min(fileIndex.value + offset, fileCount.value - 1);
    requestFileAtIndex(nextIndex, pane);
  };

  need('right', 1);
  if (splitCount.value >= 4) {
    need('bottomLeft', 2);
    need('bottomRight', 3);
  }
}

watch(() => splitCount.value, (val) => {
  if (isCompareModeSession.value) {
    if (val <= 1) {
      isSyncViewport.value = false;
    } else {
      ensureExtraPanesLoaded();
    }
    return;
  }
  if (!config.imageViewer) {
    (config as any).imageViewer = { isSplit: false, splitCount: 1, isSyncViewport: false, isFullScreen: false };
  }
  config.imageViewer.isSplit = val > 1;
  config.imageViewer.splitCount = val;
  if (val <= 1) {
    isSyncViewport.value = false;
    clearExtraPanes();
  } else {
    ensureExtraPanesLoaded();
  }
});

watch(() => isSyncViewport.value, (val) => {
  if (isCompareModeSession.value) return;
  if (!config.imageViewer) {
    (config as any).imageViewer = { isSplit: false, isSyncViewport: false, isFullScreen: false };
  }
  config.imageViewer.isSyncViewport = val;
});

watch(() => [fileIndex.value, fileCount.value], () => {
  ensureExtraPanesLoaded();
});

function requestFileAtIndex(index: number, pane: ViewerPane = 'left') {
  emit('message-from-image-viewer', { message: 'request-file-at-index', index, pane });
}

function getActiveFilePane() {
  return isSplit.value ? activePane.value : 'left';
}

function syncFileMetaToContent(targetFileId: number, changes: Record<string, any>) {
  emit('message-from-image-viewer', {
    message: 'update-file-meta',
    fileId: targetFileId,
    changes,
  });
}

function getPluginOutputPath(result: any) {
  const outputs = result?.result?.outputs;
  if (!Array.isArray(outputs)) return '';
  const output = outputs.find((item: any) => item?.path) || outputs[0];
  return String(output?.path || '');
}

function pluginTaskErrorMessage(task: any) {
  return task?.error || task?.pluginStatus?.error?.message || `Plugin task ended with status ${task?.status || 'unknown'}.`;
}

async function waitForPluginTaskOutput(pluginId: string, invokeResult: any) {
  let outputPath = getPluginOutputPath(invokeResult);

  const taskId = String(invokeResult?.taskId || invokeResult?.result?.taskId || invokeResult?.taskState?.taskId || '');
  if (!taskId) return outputPath || '';

  pluginActionDialog.value.taskId = taskId;
  pluginActionDialog.value.stage = 'queued';
  pluginActionDialog.value.taskStatus = String(invokeResult?.status || invokeResult?.taskState?.status || 'queued');
  pluginActionDialog.value.taskProgress = Number(invokeResult?.progress || invokeResult?.taskState?.progress || 0);
  pluginActionDialog.value.taskMessage = String(invokeResult?.message || invokeResult?.taskState?.message || '');

  if (outputPath) return outputPath;

  for (let attempt = 0; attempt < 120; attempt += 1) {
    const response = await getAiPluginTask(pluginId, taskId);
    const task = {
      ...(response?.state || {}),
      pluginStatus: response?.pluginStatus,
      pluginStatusError: response?.pluginStatusError,
    };

    pluginActionDialog.value.stage = String(task.status || '');
    pluginActionDialog.value.taskStatus = String(task.status || '');
    pluginActionDialog.value.taskProgress = Number(task.progress || 0);
    pluginActionDialog.value.taskMessage = String(task.message || '');
    pluginActionDialog.value.errorCode = String(task.errorCode || '');
    pluginActionDialog.value.errorDomain = String(task.errorDomain || '');
    pluginActionDialog.value.errorDetails = task.errorDetails || null;

    outputPath = getPluginOutputPath({ result: { outputs: task.outputs || response?.pluginStatus?.outputs || response?.pluginStatus?.state?.outputs || [] } });
    if (task.status === 'succeeded' && outputPath) return outputPath;
    if (['failed', 'cancelled', 'canceled'].includes(String(task.status || '').toLowerCase())) {
      throw new Error(pluginTaskErrorMessage(task));
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  throw new Error('Plugin task did not finish in time.');
}

async function cancelPluginAction() {
  const taskId = pluginActionDialog.value.taskId;
  const pluginId = pluginActionDialog.value.plugin?.id;
  if (!taskId || !pluginId) return;
  try {
    pluginActionDialog.value.stage = 'cancelling';
    pluginActionDialog.value.taskStatus = 'cancelling';
    await cancelAiPluginTask(pluginId, taskId);
  } catch (error: any) {
    pluginActionDialog.value.error = error?.message || String(error);
    pluginActionDialog.value.errorCode = 'CANCEL_FAILED';
    pluginActionDialog.value.errorDomain = 'transport';
  }
}

function getParentPath(path: string) {
  const value = String(path || '');
  const separatorIndex = Math.max(value.lastIndexOf('\\'), value.lastIndexOf('/'));
  return separatorIndex > 0 ? value.slice(0, separatorIndex) : '';
}

function closePluginActionDialog(force = false) {
  if (pluginActionDialog.value.busy && !force) return;
  pluginActionDialog.value = {
    show: false,
    busy: false,
    plugin: null,
    capability: null,
    file: null,
    error: '',
    errorCode: '',
    errorDomain: '',
    errorDetails: null,
    diagnostics: null,
    logs: [],
    taskId: '',
    stage: '',
    taskStatus: '',
    taskProgress: 0,
    taskMessage: '',
  };
}

async function runPluginMenuAction(action: any, file: any) {
  if (!action?.pluginId || !action?.capabilityId || !file?.file_path) return;
  if (file.file_type !== 1 && file.file_type !== 3) {
    toast.warning('This plugin action only supports images.');
    return;
  }

  await pluginStore.loadPlugins();
  const plugin = pluginStore.plugins.find((item: any) => item?.id === action.pluginId);
  const capability = plugin?.capabilities?.find((item: any) => item?.id === action.capabilityId);
  if (!plugin || !capability) {
    toast.error('Plugin action is unavailable.');
    return;
  }

  if (!aiPluginHostEnvironment.value) {
    try {
      aiPluginHostEnvironment.value = await getAiPluginHostEnvironment();
    } catch (error) {
      console.warn('Failed to load AI plugin host environment:', error);
    }
  }

  pluginActionDialog.value = {
    show: true,
    busy: false,
    plugin,
    capability,
    file,
    error: '',
    errorCode: '',
    errorDomain: '',
    errorDetails: null,
    diagnostics: null,
    logs: [],
    taskId: '',
    stage: '',
    taskStatus: '',
    taskProgress: 0,
    taskMessage: '',
  };
}

function inferPluginErrorDomain(message: string) {
  const text = String(message || '').toLowerCase();
  if (text.includes('timeout') || text.includes('timed out') || text.includes('http') || text.includes('transport')) return 'transport';
  if (text.includes('file') || text.includes('path') || text.includes('directory') || text.includes('permission')) return 'filesystem';
  if (text.includes('cuda') || text.includes('rocm') || text.includes('gpu') || text.includes('oom') || text.includes('out of memory') || text.includes('device')) return 'device_backend';
  if (text.includes('python') || text.includes('torch') || text.includes('runtime')) return 'runtime';
  return 'plugin';
}

async function loadPluginFailureDetails(pluginId: string) {
  const [diagnosticsResult, logsResult] = await Promise.allSettled([
    getAiPluginDiagnostics(pluginId),
    getAiPluginLogs(pluginId),
  ]);

  if (diagnosticsResult.status === 'fulfilled') {
    pluginActionDialog.value.diagnostics = diagnosticsResult.value?.diagnostics || diagnosticsResult.value;
  }

  if (logsResult.status === 'fulfilled') {
    pluginActionDialog.value.logs = Array.isArray(logsResult.value?.files)
      ? logsResult.value.files
      : [];
  }
}

async function runPluginAction(form: { inputs: Record<string, any>; parameters: Record<string, any> }) {
  const plugin = pluginActionDialog.value.plugin;
  const capability = pluginActionDialog.value.capability;
  const sourceFile = pluginActionDialog.value.file;
  if (!plugin?.id || !capability?.id) return;

  pluginActionDialog.value.busy = true;
  try {
    const permissions = pluginPermissions(plugin);
    if (permissions.network?.uploadSelectedFiles) {
      const missing = missingPluginPermissionFlags(plugin, { uploadSelectedFiles: true });
      if (missing.length > 0) {
        const inputKeys = Object.keys(form.inputs || {});
        const confirmed = await ask(
          `Plugin: ${plugin.name || plugin.id}\nCapability: ${capability.name || capability.id}\nInputs: ${inputKeys.join(', ') || 'selected file'}\nDomains: ${(pluginAllowedDomains(plugin) || []).join(', ') || 'No declared domains'}\n\nThis plugin declares that it may upload selected user files.\nAllow this action?`,
          {
            title: 'Allow plugin to upload selected files?',
            kind: 'warning',
            okLabel: 'Allow',
            cancelLabel: 'Cancel',
          },
        );
        if (!confirmed) return;
        await grantAiPluginPermissions(
          plugin.id,
          buildPluginPermissionGrantRequest(plugin, { uploadSelectedFiles: true }),
        );
        await pluginStore.loadPlugins(true);
      }
    }

    pluginActionDialog.value.stage = 'starting';
    pluginActionDialog.value.taskMessage = 'Starting plugin backend';
    toast.info('Starting plugin...');
    await startAiPlugin(plugin.id, pluginStartRequest(plugin, aiPluginHostEnvironment.value));

    pluginActionDialog.value.stage = 'invoking';
    pluginActionDialog.value.taskMessage = 'Invoking plugin capability';

    const result = await invokeAiPluginCapability(plugin.id, capability.id, {
      inputs: form.inputs,
      parameters: form.parameters,
      runtime: {
        preferredDevice: form.parameters?.device || 'auto',
      },
      resultPolicy: 'copyIntoAlbum',
    });

    const outputPath = await waitForPluginTaskOutput(plugin.id, result);
    if (!outputPath) {
      throw new Error('Plugin did not return an output file.');
    }

    const folderId = Number(sourceFile?.folder_id || 0);
    const albumId = Number(sourceFile?.album_id || 0);
    const folderPath = getParentPath(sourceFile?.file_path || '');
    if (folderId > 0 && folderPath) {
      pluginActionDialog.value.stage = 'importing';
      pluginActionDialog.value.taskMessage = 'Importing plugin result';
      const imported = await importFile(outputPath, folderId, folderPath);
      if (imported) {
        toast.success('Plugin result imported.');
        emit('message-from-image-viewer', {
          message: 'plugin-result-imported',
          albumId,
          fileId: imported.id,
        });
        closePluginActionDialog(true);
        return;
      }
    }

    toast.success(`Plugin result ready: ${outputPath}`);
    closePluginActionDialog(true);
  } catch (error: any) {
    const message = error?.message || String(error);
    pluginActionDialog.value.stage = message.toLowerCase().includes('time') ? 'timedout' : 'failed';
    pluginActionDialog.value.error = message;
    if (!pluginActionDialog.value.errorDomain) {
      pluginActionDialog.value.errorDomain = inferPluginErrorDomain(message);
    }
    if (!pluginActionDialog.value.errorCode && pluginActionDialog.value.stage === 'timedout') {
      pluginActionDialog.value.errorCode = 'TIMEOUT';
    }
    toast.error(message);
    if (plugin?.id) {
      await loadPluginFailureDetails(plugin.id);
    }
  } finally {
    pluginActionDialog.value.busy = false;
  }
}

function clickPrev(pane: ViewerPane = 'left') {
  setActivePane(pane);
  const currentIndex = getIndexByPane(pane);
  const viewerRef = getViewerRef(pane);
  if (currentIndex > 0) {
    requestFileAtIndex(currentIndex - 1, pane);
  } else {
    viewerRef?.showMessage((localeMsg.value as any).tooltip.image_viewer.first_image);
  }
}

function clickNext(pane: ViewerPane = 'left') {
  setActivePane(pane);
  const currentIndex = getIndexByPane(pane);
  const viewerRef = getViewerRef(pane);

  // Fix loop for slideshow
  if (isSlideShow.value && currentIndex >= fileCount.value - 1) {
    requestFileAtIndex(0, pane);
    return;
  }
  
  if (currentIndex < fileCount.value - 1) {
    requestFileAtIndex(currentIndex + 1, pane);
  } else {
    viewerRef?.showMessage((localeMsg.value as any).tooltip.image_viewer.last_image);
  }
}

function clickHome(pane: ViewerPane = 'left') {
  setActivePane(pane);
  requestFileAtIndex(0, pane);
}

function clickEnd(pane: ViewerPane = 'left') {
  setActivePane(pane);
  requestFileAtIndex(fileCount.value - 1, pane);
}

function clickSlideShow(pane: ViewerPane = 'left') {
  setActivePane(pane);
  isSlideShow.value = !isSlideShow.value;
  if (isSlideShow.value) {
    startSlideShow();
  } else {
    stopSlideShow();
  }
}

const clickZoomIn = (pane: ViewerPane = 'left') => {
  setActivePane(pane);
  const viewerRef = pane === 'right' ? rightMediaViewerRef.value : mediaViewerRef.value;
  viewerRef?.zoomIn();
};

const clickZoomOut = (pane: ViewerPane = 'left') => {
  setActivePane(pane);
  const viewerRef = pane === 'right' ? rightMediaViewerRef.value : mediaViewerRef.value;
  viewerRef?.zoomOut();
};

const clickZoomActual = (pane: ViewerPane = 'left') => {
  setActivePane(pane);
  const viewerRef = pane === 'right' ? rightMediaViewerRef.value : mediaViewerRef.value;
  viewerRef?.zoomActual();
};

const toggleZoomFit = (pane: ViewerPane = 'left') => {
  const current = getZoomFitByPane(pane);
  handleZoomFitUpdate(!current, pane);
};

const toggleNativeFullScreen = () => {
  isFullScreen.value = !isFullScreen.value;
};

const closeWindow = () => {
  appWindow.close();
}

const clickScale = (event: any, pane: ViewerPane = 'left') => {
  if (pane === 'right') {
    rightImageScale.value = event.scale;
    rightImageDisplayScale.value = event.displayScale ?? event.scale;
    rightImageMinScale.value = event.minScale;
    rightImageMaxScale.value = event.maxScale;
    return;
  }
  if (pane === 'bottomLeft') {
    bottomLeftImageScale.value = event.scale;
    bottomLeftImageDisplayScale.value = event.displayScale ?? event.scale;
    bottomLeftImageMinScale.value = event.minScale;
    bottomLeftImageMaxScale.value = event.maxScale;
    return;
  }
  if (pane === 'bottomRight') {
    bottomRightImageScale.value = event.scale;
    bottomRightImageDisplayScale.value = event.displayScale ?? event.scale;
    bottomRightImageMinScale.value = event.minScale;
    bottomRightImageMaxScale.value = event.maxScale;
    return;
  }

  imageScale.value = event.scale;
  imageDisplayScale.value = event.displayScale ?? event.scale;
  imageMinScale.value = event.minScale;
  imageMaxScale.value = event.maxScale;
};

const cycleSplit = () => {
  // 1 → 2 → 4 → 1
  const next: 1 | 2 | 4 = splitCount.value >= 4 ? 1 : splitCount.value >= 2 ? 4 : 2;
  setSplitCount(next);
};

const setSplitCount = (next: 1 | 2 | 4) => {
  const prev = splitCount.value;
  if (next === prev) return;
  activePane.value = 'left';
  if (next <= 1) {
    isSyncViewport.value = false;
  }
  if (next > 1 && isSlideShow.value) {
    stopSlideShow();
  }
  // reset zoom state for panes that become visible
  if (next >= 2) {
    rightIsZoomFit.value = true;
    rightImageScale.value = 1;
    rightImageMinScale.value = 0;
    rightImageMaxScale.value = 10;
  }
  if (next >= 4) {
    bottomLeftIsZoomFit.value = true;
    bottomLeftImageScale.value = 1;
    bottomLeftImageMinScale.value = 0;
    bottomLeftImageMaxScale.value = 10;
    bottomRightIsZoomFit.value = true;
    bottomRightImageScale.value = 1;
    bottomRightImageMinScale.value = 0;
    bottomRightImageMaxScale.value = 10;
  }
  splitCount.value = next;
  if (next > 1) {
    ensureExtraPanesLoaded();
  } else {
    clearExtraPanes();
    if (isSlideShow.value) scheduleNextSlide();
  }
};

const toggleSplit = () => {
  // legacy: toggle between 1 and 2
  setSplitCount(splitCount.value > 1 ? 1 : 2);
};

const toggleSyncViewport = () => {
  if (!isSplit.value) return;
  isSyncViewport.value = !isSyncViewport.value;
  if (isSyncViewport.value) {
    syncViewportFrom(activePane.value);
  }
};

const toggleFavorite = async (pane: ViewerPane = 'left') => {
  const target = getFileInfoByPane(pane);
  const currentFileId = getFileIdByPane(pane);
  if (!target || currentFileId <= 0) return;

  const previous = target.is_favorite;
  target.is_favorite = !target.is_favorite;
  try {
    await setFileFavorite(currentFileId, target.is_favorite);
    syncFileMetaToContent(currentFileId, { is_favorite: target.is_favorite });
  } catch (error) {
    target.is_favorite = previous;
    console.error('Failed to toggle favorite:', error);
  }
};

const setCurrentFileRating = async (rating: number, pane: ViewerPane = 'left') => {
  const target = getFileInfoByPane(pane);
  const currentFileId = getFileIdByPane(pane);
  if (!target || currentFileId <= 0) return;

  const previous = target.rating;
  const normalized = Number(target.rating || 0) === rating ? 0 : rating;
  target.rating = normalized;
  try {
    await setFileRating(currentFileId, normalized);
    syncFileMetaToContent(currentFileId, { rating: normalized });
  } catch (error) {
    target.rating = previous;
    console.error('Failed to set rating:', error);
  }
};

const clickRotate = async (pane: 'left' | 'right' = 'left') => {
  const target = getFileInfoByPane(pane);
  const currentFileId = getFileIdByPane(pane);
  const viewerRef = getViewerRef(pane);
  if (!target || currentFileId <= 0) return;

  const previous = Number(target.rotate) || 0;
  target.rotate = previous + 90;
  viewerRef?.rotateRight?.();
  try {
    await setFileRotate(currentFileId, target.rotate);
    syncFileMetaToContent(currentFileId, { rotate: target.rotate });
  } catch (error) {
    target.rotate = previous;
    console.error('Failed to rotate file:', error);
  }
};

const clickTag = (pane: 'left' | 'right' = 'left') => {
  const currentFileId = getFileIdByPane(pane);
  if (currentFileId <= 0) return;

  setActivePane(pane);
  taggingFileIds.value = [currentFileId];
  showTaggingDialog.value = true;
};

const openCommentEditor = (pane: 'left' | 'right' = 'left') => {
  const currentFileId = getFileIdByPane(pane);
  if (currentFileId <= 0) return;

  setActivePane(pane);
  showCommentMsgbox.value = true;
};

const onEditComment = async (newComment: any) => {
  const target = activeFileInfo.value;
  const currentFileId = activeFileId.value;
  if (!target || currentFileId <= 0) return;

  const result = await editFileComment(currentFileId, newComment);
  if (result) {
    target.comments = newComment;
    showCommentMsgbox.value = false;
    syncFileMetaToContent(currentFileId, { comments: newComment });
  }
};

async function updateFileHasTags(fileIds: number[]) {
  if (!Array.isArray(fileIds) || fileIds.length === 0) {
    showTaggingDialog.value = false;
    return;
  }

  for (const taggedFileId of fileIds) {
    if (taggedFileId === fileId.value && fileInfo.value) {
      const tags = (await getTagsForFile(taggedFileId)) || [];
      fileInfo.value.has_tags = tags.length > 0;
      fileInfo.value.tags = tags;
      syncFileMetaToContent(taggedFileId, { has_tags: fileInfo.value.has_tags, tags });
    }

    if (taggedFileId === rightFileId.value && rightFileInfo.value) {
      const tags = (await getTagsForFile(taggedFileId)) || [];
      rightFileInfo.value.has_tags = tags.length > 0;
      rightFileInfo.value.tags = tags;
      syncFileMetaToContent(taggedFileId, { has_tags: rightFileInfo.value.has_tags, tags });
    }
  }

  showTaggingDialog.value = false;
}


async function openImageEditorForActivePane() {
  const pane = getActiveFilePane();
  const file = getFileInfoByPane(pane);
  const fileId = Number(file?.id || 0);
  if (fileId <= 0) return;
  // Only still images / RAW
  const ft = Number(file?.file_type || 0);
  if (ft !== 1 && ft !== 3) return;

  const webViewLabel = 'imageeditor';
  const existing = await WebviewWindow.getByLabel(webViewLabel);
  if (existing) {
    await existing.emit('update-file', { fileId });
    await existing.show();
    await existing.setFocus();
    return;
  }

  const newWindow = new WebviewWindow(webViewLabel, {
    url: `/image-editor?fileId=${fileId}`,
    title: 'Image Editor',
    width: 1100,
    height: 700,
    minWidth: 800,
    minHeight: 500,
    resizable: true,
    maximizable: false,
    visible: false,
    transparent: true,
    decorations: isMac,
    ...(isMac && {
      titleBarStyle: 'overlay',
      hiddenTitle: true,
      minimizable: false,
    }),
  });

  newWindow.once('tauri://created', () => {
    newWindow.show();
    newWindow.setFocus();
  });
  newWindow.once('tauri://error', (e) => {
    console.error('Failed creating ImageEditor window:', e);
  });
}

const handleItemAction = async (payload: { action: any }) => {
  if (payload.action?.type === 'plugin-menu') {
    const pane = getActiveFilePane();
    await runPluginMenuAction(payload.action, getFileInfoByPane(pane));
    return;
  }

  if (typeof payload.action !== 'string') return;

  const pane = getActiveFilePane();

  switch (payload.action) {
    case 'favorite':
      await toggleFavorite(pane);
      break;
    case 'rotate':
      await clickRotate(pane);
      break;
    case 'tag':
      clickTag(pane);
      break;
    case 'comment':
      openCommentEditor(pane);
      break;
    case 'edit':
      await openImageEditorForActivePane();
      break;
    case 'rating-0':
    case 'rating-1':
    case 'rating-2':
    case 'rating-3':
    case 'rating-4':
    case 'rating-5':
      await setCurrentFileRating(Number(payload.action.split('-')[1]), pane);
      break;
    case 'zoom-in':
      clickZoomIn(pane);
      break;
    case 'zoom-out':
      clickZoomOut(pane);
      break;
    case 'zoom-actual':
      clickZoomActual(pane);
      break;
    case 'toggle-split':
      toggleSplit();
      break;
    case 'cycle-split':
      cycleSplit();
      break;
    case 'set-split-count': {
      const n = Number((payload as any)?.count ?? payload?.action?.count ?? 2);
      setSplitCount(n >= 4 ? 4 : n >= 2 ? 2 : 1);
      break;
    }
    case 'toggle-sync-viewport':
      toggleSyncViewport();
      break;
    default:
      break;
  }
};

</script>

<style scoped>
* {
  user-select: none;
}
</style>

