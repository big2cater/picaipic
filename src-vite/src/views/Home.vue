<template>
  
  <div class="w-screen h-screen flex flex-col overflow-hidden select-none bg-base-300 text-base-content/70">
    <transition name="fade">
      <div
        v-if="isSwitchingLibrary"
        class="absolute inset-0 z-60 bg-base-300/60 backdrop-blur-sm flex items-center justify-center"
      >
        <span class="loading loading-spinner loading-lg text-primary"></span>
      </div>
    </transition>

    <BlackHoleBackground
      v-if="showBlackHole"
      :gravity-active="gravityActive"
      :effective-elapsed-sec="effectiveElapsedSec"
      @radii="onHoleRadii"
    />

    <!-- Title Bar -->
    <TitleBar v-if="showDesktopTitleBar" titlebar="PicAiPic" viewName="Home" :icon="iconLogo"/>

    <!-- Main Content -->
    <div class="flex-1 flex overflow-hidden">

      <!-- left pane -->
      <div
        v-if="config.leftPanel.show && !uiStore.isFullScreen"
        ref="leftPanelRootRef"
        tabindex="-1"
        :class="[
          'relative flex my-1 ml-1 z-10 select-none outline-none',
          !leftPanelLayoutExpanded && isMac ? 'mt-12 mb-8': '',
        ]"
        :style="{ width: leftPanelLayoutExpanded ? config.leftPanel.width + 'px' : '64px' }"
        data-tauri-drag-region
        @focus="uiStore.setActivePane('left-sidebar')"
      >
          <div
            class="absolute inset-y-0 left-0 bg-base-200 rounded-box"
            :class="isDraggingSplitter ? '' : 'transition-[width] duration-200 ease-in-out'"
            :style="{ width: leftPanelVisualExpanded ? config.leftPanel.width + 'px' : '64px' }"
          ></div>

          <!-- side bar -->
          <div 
            :class="[
              'fixed top-14 min-w-16 bottom-10 z-10 flex flex-col items-center',
              config.settings.showButtonText ? 'space-y-3' : 'space-y-1' 
            ]" 
            data-tauri-drag-region
          >
            <div v-for="item in visibleButtons" :key="item.index">
              <TButton
                :buttonSize="'large'"
                :icon="item.icon"
                :text="item.text"
                :tooltip="(item as any).tooltip || ''"
                :selected="config.main.sidebarIndex === item.index"
                :disabled="item.disabled"
                @click="clickSidebar(item.index)"
              />
            </div>

            <div class="flex-1"></div>

            <TButton 
              class="mt-auto"
              :class="showDebugBadge ? 'text-warning': ''"
              :buttonSize="'large'" 
              :icon="IconSettings" 
              :text="$t('sidebar.settings')" 
              @click="clickSettings"
            />
          </div>

          <!-- panel-->
          <div
            v-if="leftPanelMounted"
            class="absolute inset-y-0 left-16 pr-0.5 flex flex-col overflow-hidden transition-[transform,opacity] duration-200 ease-in-out"
            :class="leftPanelVisualExpanded ? 'translate-x-0 opacity-100' : '-translate-x-full opacity-0 pointer-events-none'"
            :style="{ width: Math.max(0, config.leftPanel.width - 64) + 'px' }"
          >
            <!-- library title -->
            <div 
              class="mb-2 h-10 flex items-center justify-between whitespace-nowrap shrink-0"
              :class="config.settings.scale < 1 ? 'p-3' : 'p-1'"
              data-tauri-drag-region
            >
              
              <!-- Library dropdown selector -->
              <ContextMenu
                :menuItems="libraryMenuItems"
              >
                <template #trigger="{ toggle }">
                  <button 
                    class="px-2 py-1 flex items-center gap-1 rounded-box text-base-content/70 hover:bg-base-100/30 hover:text-base-content cursor-pointer transition-colors"
                    @click="toggle"
                  >
                    <IconStack class="w-5 h-5 shrink-0" />
                    <span class="overflow-hidden whitespace-pre text-ellipsis max-w-32">{{ currentLibrary?.name || 'Library' }}</span>
                    <IconArrowDown class="w-3 h-3 shrink-0 opacity-50" />
                  </button>
                </template>
              </ContextMenu>

              <button
                v-if="updateAvailable || isInstallingUpdate || isUpdateReadyToRestart || isReleaseNoteVisible"
                class="badge badge-sm border-0 px-2 py-2 font-medium transition-colors"
                :class="isUpdateActionEnabled ? 'badge-primary cursor-pointer' : 'badge-neutral/60 cursor-default'"
                :disabled="isInstallingUpdate"
                :title="updateButtonTooltip"
                @click="handleUpdateAction"
              >
                <span v-if="isInstallingUpdate" class="loading loading-spinner loading-xs"></span>
                <span>{{ updateButtonText }}</span>
              </button>

            </div>

            <!-- Component panel (flex-1 to fill remaining space) -->
            <div class="flex-1 min-h-0 flex flex-col overflow-hidden">
              <div class="flex-1 min-h-0 overflow-hidden">
                <component ref="panelRef" 
                  :key="libraryVersion"
                  :is="buttons[config.main.sidebarIndex].component" 
                  :titlebar="buttons[config.main.sidebarIndex].text"
                />
              </div>
              <CollectionTray
                v-if="config.settings.showCollections"
                class="shrink-0"
                :style="{ maxHeight: (config.collectionTray?.height || 180) + 'px' }"
                :expanded="!!config.collectionTray?.expanded"
                @toggle-expanded="toggleCollectionTray"
              />
            </div>
          </div>
        </div>
      
      <!-- splitter -->
      <div v-if="!uiStore.isFullScreen"
        class="w-1 transition-colors shrink-0"
        :class="{
          'hover:bg-primary cursor-col-resize': config.leftPanel.show && leftPanelLayoutExpanded,
          'bg-primary': config.leftPanel.show && leftPanelLayoutExpanded && isDraggingSplitter,
        }" 
        @mousedown="startDraggingSplitter"
        @mouseup="stopDraggingSplitter"
      ></div>
       
      <!-- content area -->
      <div 
        :class="[
          'flex-1 flex relative',
          showDesktopTitleBar ? 'rounded-tl-box' : '',
        ]"
      >
        <MapHeatmapView v-if="config.main.sidebarIndex === MAP_SIDEBAR_INDEX" />
        <Content v-else ref="contentRef" :key="libraryVersion" :titlebar="buttons[config.main.sidebarIndex].text" :libraryEmpty="libraryEmpty"/>
      </div>
    </div>

    <!-- logo -->
    <div class="fixed bottom-2 left-6 text-[12px] text-base-content/30">
      <span>{{ appName }}</span>
    </div>

    <!-- Manage Libraries Dialog -->
    <ManageLibraries
      v-if="showManageLibraries"
      @ok="onManageLibrariesOk"
      @updated="onManageLibrariesUpdated"
      @cancel="showManageLibraries = false"
    />
  </div>

</template>

<script setup lang="ts">
import { ref, computed, defineAsyncComponent, onBeforeUnmount, onMounted, watch, nextTick, provide } from 'vue';
import { useI18n } from 'vue-i18n';
import { emit, listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { getName } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { config, libConfig } from '@/common/config';
import { useAppUpdater } from '@/common/updater';
import { useUIStore } from '@/stores/uiStore';
import { isWin, isMac, isLinux, SCALE_VALUES } from '@/common/utils';
import { matchesShortcut, ShortcutPlatform } from '@/common/shortcuts';
import { getAppConfig, switchLibrary, cancelIndexing, cancelFaceIndex, setImportAiPrompts } from '@/common/api';
import { useIdle } from '@/composables/useIdle';

// vue components
import TitleBar from '@/components/TitleBar.vue';
import TButton from '@/components/TButton.vue';
import ContextMenu from '@/components/ContextMenu.vue';
import BlackHoleBackground from '@/components/BlackHoleBackground.vue';
import iconLogo from '@/assets/images/icon.png';

const Library = defineAsyncComponent(() => import('@/components/Library.vue'));
const ImageSearch = defineAsyncComponent(() => import('@/components/ImageSearch.vue'));
const Favorite = defineAsyncComponent(() => import('@/components/Favorite.vue'));
const Tag = defineAsyncComponent(() => import('@/components/Tag.vue'));
const SmartAlbumList = defineAsyncComponent(() => import('@/components/SmartAlbumList.vue'));
const Calendar = defineAsyncComponent(() => import('@/components/Calendar.vue'));
const Location = defineAsyncComponent(() => import('@/components/Location.vue'));
const Person = defineAsyncComponent(() => import('@/components/Person.vue'));
const Camera = defineAsyncComponent(() => import('@/components/Camera.vue'));
const MapHeatmapView = defineAsyncComponent(() => import('@/components/MapHeatmapView.vue'));
const Content = defineAsyncComponent(() => import('@/components/Content.vue'));
const ManageLibraries = defineAsyncComponent(() => import('@/components/ManageLibraries.vue'));

import {
  IconHeart,
  IconTag,
  IconLocation,
  IconPerson,
  IconCameraAperture,
  IconSearch,
  IconSettings,
  IconDot,
  IconStack,
  IconArrowDown,
  IconCalendarDay,
  IconFolders,
  IconMapDefault,
} from '@/common/icons';

const isSwitchingLibrary = ref(false);
const libraryVersion = ref(0);
const libraryEmpty = ref(false);

const checkLibraryEmpty = async () => {
  try {
    const albums = await invoke<any[]>('get_all_albums');
    libraryEmpty.value = (albums?.length ?? 0) === 0;
    if (libraryEmpty.value) {
      config.main.sidebarIndex = 0;
    }
  } catch {
    libraryEmpty.value = false;
  }
};
function toggleCollectionTray() {
  if (!config.collectionTray) {
    (config as any).collectionTray = { expanded: true, height: 180 };
  }
  config.collectionTray.expanded = !config.collectionTray.expanded;
}

const SETTINGS_BASE_WIDTH = 600;
const SETTINGS_BASE_HEIGHT = 620;

/// i18n
const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[locale.value] as any);

const uiStore = useUIStore();

const { idle } = useIdle(15000);
const reducedMotion = ref(false);
const docHidden = ref(typeof document !== 'undefined' ? document.hidden : false);
const effectiveElapsedSec = ref(0);
const holeRadii = ref({ R_event: 0, R_inf: 0 });
let growthRaf = 0;
let growthAnchor = 0;
let growthAccum = 0;
let reducedMotionMq: MediaQueryList | null = null;

const gravityActive = computed(() =>
  !!config.settings.blackHoleMode
  && !!uiStore.isMaximized
  && idle.value
  && !reducedMotion.value
  && !docHidden.value
  && uiStore.inputStack.length === 0
  && !isSwitchingLibrary.value
);

const showBlackHole = computed(
  () => !!config.settings.blackHoleMode && !reducedMotion.value,
);

function onHoleRadii(payload: { R_event: number; R_inf: number }) {
  holeRadii.value = payload;
}

provide('bhGravityActive', gravityActive);
provide('bhRadii', holeRadii);

function onVisibilityChange() {
  docHidden.value = document.hidden;
}

function applyReducedMotionMq() {
  reducedMotion.value = !!reducedMotionMq?.matches;
}

function growthLoop(ts: number) {
  if (gravityActive.value && !docHidden.value) {
    if (!growthAnchor) growthAnchor = ts;
    effectiveElapsedSec.value = growthAccum + (ts - growthAnchor) / 1000;
  }
  growthRaf = requestAnimationFrame(growthLoop);
}

watch(gravityActive, (on, was) => {
  if (on && !was) {
    growthAccum = 0;
    growthAnchor = performance.now();
    effectiveElapsedSec.value = 0;
  } else if (!on) {
    growthAccum = 0;
    growthAnchor = 0;
    effectiveElapsedSec.value = 0;
  }
});

watch(docHidden, (hidden) => {
  if (hidden) {
    if (gravityActive.value && growthAnchor) {
      growthAccum += (performance.now() - growthAnchor) / 1000;
      growthAnchor = 0;
    }
  } else if (gravityActive.value) {
    growthAnchor = performance.now();
  }
});

// Panel component ref
const panelRef = ref<any>(null);
const contentRef = ref<any>(null);
const leftPanelRootRef = ref<HTMLElement | null>(null);
const showPanel = ref(true);
const LEFT_PANEL_ANIMATION_MS = 200;
const leftPanelMounted = ref(showPanel.value);
const leftPanelVisualExpanded = ref(showPanel.value);
const leftPanelLayoutExpanded = ref(showPanel.value);
let leftPanelAnimationTimer: ReturnType<typeof setTimeout> | null = null;
let leftPanelAnimationVersion = 0;

function clearLeftPanelAnimationTimer() {
  if (leftPanelAnimationTimer) {
    clearTimeout(leftPanelAnimationTimer);
    leftPanelAnimationTimer = null;
  }
}

async function commitLeftPanelLayout(expanded: boolean) {
  leftPanelLayoutExpanded.value = expanded;
  await nextTick();
  await contentRef.value?.refreshCenteredGridLayout?.();
}

watch(showPanel, async (expanded) => {
  clearLeftPanelAnimationTimer();
  const animationVersion = ++leftPanelAnimationVersion;

  if (expanded) {
    leftPanelMounted.value = true;
    await nextTick();
    if (animationVersion !== leftPanelAnimationVersion) return;
    leftPanelVisualExpanded.value = true;
    leftPanelAnimationTimer = setTimeout(() => {
      if (animationVersion !== leftPanelAnimationVersion) return;
      leftPanelAnimationTimer = null;
      void commitLeftPanelLayout(true);
    }, LEFT_PANEL_ANIMATION_MS);
    return;
  }

  leftPanelVisualExpanded.value = false;
  leftPanelAnimationTimer = setTimeout(() => {
    if (animationVersion !== leftPanelAnimationVersion) return;
    leftPanelAnimationTimer = null;
    void commitLeftPanelLayout(false).then(() => {
      if (animationVersion !== leftPanelAnimationVersion) return;
      leftPanelMounted.value = false;
    });
  }, LEFT_PANEL_ANIMATION_MS);
});

// Library state
interface Library {
  id: string;
  name: string;
  created_at: number;
  hidden: boolean;
}

interface AppConfig {
  current_library_id: string;
  libraries: Library[];
}

const appConfig = ref<AppConfig | null>(null);
const currentLibrary = computed(() => 
  appConfig.value?.libraries.find(l => l.id === appConfig.value?.current_library_id) || null
);

// Manage Libraries dialog state
const showManageLibraries = ref(false);
const showDesktopTitleBar = isWin || isLinux;

/// Splitter for resizing the left pane
const isDraggingSplitter = ref(false);

const appName = ref('');
const showDebugBadge = import.meta.env.DEV;
let unlistenOpenPreferences: (() => void) | null = null;
let unlistenOpenAbout: (() => void) | null = null;
let unlistenAlbumsRefreshed: (() => void) | null = null;
let unlistenAddAlbumRequested: (() => void) | null = null;
let unlistenEditAlbumRequested: (() => void) | null = null;
const shortcutPlatform: ShortcutPlatform = isMac ? 'mac' : (isLinux ? 'linux' : 'windows');
const {
  updateAvailable,
  isCheckingUpdate,
  isInstallingUpdate,
  isUpdateReadyToRestart,
  isReleaseNoteVisible,
  updateButtonTooltip,
  updateButtonText,
  isUpdateActionEnabled,
  checkForUpdates,
  handleUpdateAction,
} = useAppUpdater(localeMsg);

// buttons
const buttons = computed(() =>  [
  { icon: IconFolders, component: Library, text: localeMsg.value.sidebar.album },
  { icon: IconTag, component: SmartAlbumList, text: localeMsg.value.sidebar.smart_albums || localeMsg.value.album?.smart_album_list || 'Smart Albums' },
  { icon: IconHeart, component: Favorite, text: localeMsg.value.sidebar.favorite },
  { icon: IconSearch, component: ImageSearch, text: localeMsg.value.sidebar.search },
  { icon: IconCalendarDay, component: Calendar, text: localeMsg.value.sidebar.calendar },
  { icon: IconTag, component: Tag, text: localeMsg.value.sidebar.tag },
  { icon: IconPerson, component: Person, text: localeMsg.value.sidebar.people, hidden: !config.settings.face.enabled },
  { icon: IconLocation, component: Location, text: localeMsg.value.sidebar.location },
  { icon: IconCameraAperture, component: Camera, text: localeMsg.value.sidebar.camera },
  { icon: IconMapDefault, component: null, text: localeMsg.value.sidebar.map },
]);

// dedicated full-area heatmap view, shown instead of Content
// Absolute indices must match Home `buttons` order and Content `SIDEBAR` constants.
const MAP_SIDEBAR_INDEX = 9; // SIDEBAR.MAP

const visibleButtons = computed(() =>
  buttons.value
    .map((item, index) => ({ ...item, index, disabled: libraryEmpty.value && index !== 0 }))
    .filter(item => !item.hidden)
);

watch(() => config.settings.face.enabled, (enabled) => {
  // Person panel is absolute index 6 (SIDEBAR.PERSON) when face is enabled.
  if (!enabled && config.main.sidebarIndex === 6) {
    config.main.sidebarIndex = 0;
  }
});

watch(() => config.libraryChangedVersion, async () => {
  appConfig.value = await getAppConfig();
});

const libraryMenuItems = computed(() => {
  const items: any[] = [];
  
  // Add all libraries for switching
  if (appConfig.value?.libraries) {
    for (const lib of appConfig.value.libraries) {
      if (lib.hidden) continue; // Skip hidden libraries
      const isSelected = lib.id === appConfig.value.current_library_id;
      items.push({
        label: lib.name,
        icon: isSelected ? IconDot : null,
        action: () => {
          if (!isSelected) {
            doSwitchLibrary(lib.id);
          }
        }
      });
    }
  }
  items.push({
    label: "-",
    action: () => {}
  });
  items.push({
    label: localeMsg.value.menu.library.manage,
    // icon: IconEdit,
    action: () => {
      showManageLibraries.value = true;
    }
  });
  return items;
});


onMounted(async () => {
  window.addEventListener('keydown', handleHomeKeyDown);
  unlistenOpenPreferences = await listen('app-open-preferences', () => {
    void clickSettings();
  });
  unlistenOpenAbout = await listen('app-open-about', () => {
    void clickSettings(5);
  });

  appConfig.value = await getAppConfig();

  void checkLibraryEmpty();

  unlistenAddAlbumRequested = await listen('add-album-requested', async () => {
    if (config.main.sidebarIndex !== 0) config.main.sidebarIndex = 0;
    showPanel.value = true;
    await nextTick();
    (panelRef.value as any)?.albumListRef?.clickNewAlbum();
  });

  unlistenEditAlbumRequested = await listen('edit-album-requested', async (event: any) => {
    const albumId = Number(event.payload?.albumId || 0);
    if (albumId <= 0) return;
    if (config.main.sidebarIndex !== 0) config.main.sidebarIndex = 0;
    showPanel.value = true;
    await nextTick();
    (panelRef.value as any)?.albumListRef?.openAlbumEdit(albumId);
  });

  unlistenAlbumsRefreshed = await listen('albums-refreshed', () => {
    void checkLibraryEmpty();
  });

  try {
    const name = await getName();
    if (name) appName.value = name;
  } catch (e) {
    console.error('Failed to get app name:', e);
  }

  if (config.settings.autoCheckUpdates !== false) {
    void checkForUpdates(false);
  }

  // Sync scan-time AI PNG prompt import flag with persisted UI setting (default on).
  void setImportAiPrompts(config.settings.importAiPromptsToComments !== false);

  reducedMotionMq = window.matchMedia('(prefers-reduced-motion: reduce)');
  applyReducedMotionMq();
  reducedMotionMq.addEventListener?.('change', applyReducedMotionMq);
  document.addEventListener('visibilitychange', onVisibilityChange);
  growthRaf = requestAnimationFrame(growthLoop);
});

onBeforeUnmount(() => {
  clearLeftPanelAnimationTimer();
  window.removeEventListener('keydown', handleHomeKeyDown);
  unlistenOpenPreferences?.();
  unlistenOpenPreferences = null;
  unlistenOpenAbout?.();
  unlistenOpenAbout = null;
  unlistenAlbumsRefreshed?.();
  unlistenAlbumsRefreshed = null;
  unlistenAddAlbumRequested?.();
  unlistenAddAlbumRequested = null;
  unlistenEditAlbumRequested?.();
  unlistenEditAlbumRequested = null;
  if (growthRaf) cancelAnimationFrame(growthRaf);
  growthRaf = 0;
  document.removeEventListener('visibilitychange', onVisibilityChange);
  reducedMotionMq?.removeEventListener?.('change', applyReducedMotionMq);
  reducedMotionMq = null;
});

function handleHomeKeyDown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.isContentEditable) {
    return;
  }

  if (event.key === 'Tab' && uiStore.inputStack.length === 0) {
    event.preventDefault();
    event.stopPropagation();
    if (uiStore.activePane === 'left-sidebar' || !leftPanelRootRef.value) {
      contentRef.value?.focusContent?.();
    } else {
      uiStore.setActivePane('left-sidebar');
      const albumListRoot = leftPanelRootRef.value.querySelector<HTMLElement>('[data-album-list-root="true"]');
      const folderTreeRoot = albumListRoot?.querySelector<HTMLElement>(
        '[data-selected-album-folder="true"] [data-folder-tree-root="true"]',
      );
      (folderTreeRoot || albumListRoot || leftPanelRootRef.value).focus({ preventScroll: true });
    }
    return;
  }

  if (matchesShortcut('app.search', event, shortcutPlatform)) {
    event.preventDefault();
    event.stopPropagation();
    if (!libraryEmpty.value) {
      // Search is absolute index 3 after Smart Albums was inserted at 1.
      const SEARCH_SIDEBAR_INDEX = 3;
      if (config.main.sidebarIndex === SEARCH_SIDEBAR_INDEX && showPanel.value) {
        nextTick(() => (panelRef.value as any)?.focusSearchInput?.());
      } else {
        config.main.sidebarIndex = SEARCH_SIDEBAR_INDEX;
        showPanel.value = true;
      }
    }
    return;
  }

  if (!matchesShortcut('app.sidebar.toggle', event, shortcutPlatform)) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();
  if (!libraryEmpty.value) {
    showPanel.value = !showPanel.value;
  }
}

const doSwitchLibrary = async (libraryId: string) => {
  try {
    isSwitchingLibrary.value = true;

    // Save current library state before switching (preserves the indexing queue)
    await libConfig.save();

    // Prevent auto-save during shutdown of the current library's background work.
    libConfig._initialized = false;

    // Cancel any running indexing before switching
    if (libConfig.index.status > 0 && libConfig.index.albumQueue.length > 0) {
      const queueCopy = [...libConfig.index.albumQueue];
      for (const albumId of queueCopy) {
        await cancelIndexing(albumId);
      }
    }
    
    // Cancel face indexing if running
    await cancelFaceIndex();
    
    await switchLibrary(libraryId);

    // Reload library state in-place (no page reload)
    await libConfig.reload();
    appConfig.value = await getAppConfig();
    libraryVersion.value++;
    void checkLibraryEmpty();
    await emit('library-switched');
  } catch (error) {
    libConfig._initialized = true;
    console.error('Failed to switch library:', error);
  } finally {
    isSwitchingLibrary.value = false;
  }
};

const onManageLibrariesOk = async () => {
  const oldLibId = appConfig.value?.current_library_id;
  appConfig.value = await getAppConfig();
  showManageLibraries.value = false;

  if (oldLibId && appConfig.value?.current_library_id !== oldLibId) {
    isSwitchingLibrary.value = true;
    try {
      // The backend has already switched; reload in-place.
      await libConfig.reload();
      libraryVersion.value++;
      void checkLibraryEmpty();
      await emit('library-switched');
    } finally {
      isSwitchingLibrary.value = false;
    }
  }
};

const onManageLibrariesUpdated = async () => {
  appConfig.value = await getAppConfig();
};

// click sidebar
function clickSidebar(index: number) {
  if (libConfig.activePane === 'collection' || libConfig.activePane === 'smart') {
    libConfig.activePane = 'main';
    if (libConfig.smartAlbum) libConfig.smartAlbum = { type: null, id: null };
  }

  if (libraryEmpty.value && index !== 0) return;
  if (index === MAP_SIDEBAR_INDEX) {
    // map view has no filter panel - give it the full content area
    showPanel.value = false;
    config.main.sidebarIndex = index;
    return;
  }
  if (config.main.sidebarIndex === index) {
    showPanel.value = !showPanel.value;
  } else {
    showPanel.value = true;
    config.main.sidebarIndex = index;
  }
}

// Dragging the splitter
function startDraggingSplitter(event: MouseEvent) {
  if(!config.leftPanel.show || !leftPanelLayoutExpanded.value) return; // no expanded left pane

  isDraggingSplitter.value = true;
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopDraggingSplitter);
}

// Stop dragging the splitter
function stopDraggingSplitter(event: MouseEvent) {
  isDraggingSplitter.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopDraggingSplitter);
}

// Handle mouse move event
function handleMouseMove(event: MouseEvent) {
  if (isDraggingSplitter.value) {
    const pointerX = event.clientX;
    const maxLeftPaneWidth = window.innerWidth / 2;
    config.leftPanel.width = Math.max(160, Math.min(pointerX - 6, maxLeftPaneWidth)); // -2: border width(2px)
  }
}

/// click settings icon
async function clickSettings(tabIndex?: number) {
  if (typeof tabIndex === 'number') {
    config.settings.tabIndex = tabIndex;
    await emit('settings-settingsTabIndex-changed', tabIndex);
  }

  // check if the settings window is already open
  const settingsWindow = await WebviewWindow.getByLabel('settings');
  if (settingsWindow) {
    if (isWin && await settingsWindow.isMinimized()) {
      await settingsWindow.unminimize();
    }
    await settingsWindow.show();
    if (isWin) {
      await settingsWindow.setFocus();
    }
    return;
  }

  const options: any = {
    url: '/settings',
    title: 'Settings',
    width: Math.round(SETTINGS_BASE_WIDTH * getSettingsWindowScale()),
    height: Math.round(SETTINGS_BASE_HEIGHT * getSettingsWindowScale()),
    minWidth: Math.round(SETTINGS_BASE_WIDTH * getSettingsWindowScale()),
    minHeight: Math.round(SETTINGS_BASE_HEIGHT * getSettingsWindowScale()),
    resizable: true,
    maximizable: false,
    visible: false, // Start hidden, will show after mount
    transparent: true, // Prevent white flash on show (Tauri 2.x workaround)
    decorations: isMac, // true for macOS, false for Windows
    ...(isMac && {
      titleBarStyle: 'Overlay',
      hiddenTitle: true,
      minimizable: false,
    }),
  };

  // create a new settings window
  const newSettingsWindow = new WebviewWindow('settings', options);
  
  newSettingsWindow.once('tauri://created', () => {
    console.log('settings window created');
  });

  newSettingsWindow.once('tauri://close-requested', () => {
    newSettingsWindow.close();
    console.log('settings window closed');
  });
}

function getSettingsWindowScale() {
  return SCALE_VALUES.find((item) => item === Number(config.settings.scale)) ?? 1;
}

</script>
