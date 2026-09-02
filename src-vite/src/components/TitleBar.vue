<template>

  <!-- Custom Title Bar -->
  <div 
    :class="[
      'w-full flex items-center justify-between select-none cursor-default',
      viewName==='ImageViewer' ? 'h-12' : 'h-10',
    ]"
    @contextmenu.prevent
    data-tauri-drag-region
  >
    <!-- Title Name -->
    <!-- Icon & Title Container -->
    <div v-if="isMac" class="flex-1" data-tauri-drag-region></div>
    <div 
      :class="[
        'flex items-center overflow-hidden',
        showDesktopWindowControls ? 'ml-2' : '',
        isMac ? 'justify-center text-center' : ''
      ]"
      data-tauri-drag-region
    >
      <!-- Icon -->
      <img 
        v-if="icon" 
        :src="icon" 
        class="w-5 h-5 mr-2 select-none rounded" 
        data-tauri-drag-region 
      />
      
      <!-- Title Name -->
      <span 
        class="text-nowrap text-base-content/70 overflow-hidden whitespace-pre text-ellipsis"
        data-tauri-drag-region
      >
        {{ titlebar }}
      </span>
    </div>
    <div v-if="isMac" class="flex-1" data-tauri-drag-region></div>

    <!-- Center Slot -->
    <div
      :class="[
        isMac ? 'hidden' : 'flex-1 flex items-center justify-center'
      ]"
      data-tauri-drag-region
    >
      <slot></slot>
    </div>

    <!-- Window Control Buttons -->
    <div v-if="showDesktopWindowControls" class="h-10 mb-auto flex items-center" @mousedown.stop>
      <IconWinMinus v-if="resizable" 
        class="p-3 w-12 h-full text-base-content/70 hover:text-base-content hover:bg-base-100 transition-colors duration-300" 
        @click.stop="minimizeWindow" 
      />
      <component v-if="resizable" :is="isMaximized ? IconWinRestore : IconWinMaximize" 
        class="p-3 w-12 h-full text-base-content/70 hover:text-base-content hover:bg-base-100 transition-colors duration-300" 
        @click.stop="toggleMaximizeWindow" 
      />
      <IconClose 
        class="p-3 w-12 h-full text-base-content/70 hover:text-base-content hover:bg-red-500 transition-colors duration-300" 
        @mousedown.stop="closeWindow"
        @click.stop="closeWindow" 
      />
    </div>

  </div>

</template>

<script setup>

import { ref, watch, onMounted, onUnmounted } from 'vue';
import { emit } from '@tauri-apps/api/event';
import { getCurrentWindow  } from '@tauri-apps/api/window';
import { isWin, isMac, isLinux } from '@/common/utils';
import { useUIStore } from '@/stores/uiStore';

import { 
  IconWinMinus,
  IconWinMaximize,
  IconWinRestore,
  IconClose 
} from '@/common/icons';

const props = defineProps({
  titlebar: {
    type: String,
    required: true,
  },
  viewName: {
    type: String,
    required: false,
  },
  resizable: {
    type: Boolean,
    default: true,
  },
  icon: {
    type: String,
    default: '',
  }
});

const searchValue = ref('');

const appWindow = getCurrentWindow();
const isMaximized = ref(false);
const showDesktopWindowControls = isWin || isLinux;
const uiStore = useUIStore();
const syncMaximizedToStore = props.viewName === 'Home';
let unlistenResize = null;

watch(() => searchValue.value, (newValue) => { 
  emit('message-from-titlebar', { message: 'search', search: searchValue.value });
});

// drag window
// const onMousedown = (e) => {
//   if (e.detail === 1 && !isMaximized.value) {   // 1: single click
//     appWindow.startDragging();
//   }
// };

function applyMaximizedState(maximized) {
  const next = !!maximized;
  isMaximized.value = next;
  if (syncMaximizedToStore) {
    uiStore.setMaximized(next);
  }
}

async function refreshMaximized() {
  try {
    const maximized = await appWindow.isMaximized();
    applyMaximizedState(maximized);
  } catch (e) {
    console.warn('TitleBar isMaximized failed', e);
  }
}

const minimizeWindow = () => {
  appWindow.minimize();
};

const toggleMaximizeWindow = async () => {
  try {
    if (await appWindow.isMaximized()) {
      await appWindow.unmaximize();
    } else {
      await appWindow.maximize();
    }
    await refreshMaximized();
  } catch (e) {
    console.warn('TitleBar toggle maximize failed', e);
  }
};

const closeWindow = () => {
  appWindow.close();
};

onMounted(async () => {
  await refreshMaximized();
  try {
    unlistenResize = await appWindow.onResized(() => {
      void refreshMaximized();
    });
  } catch (e) {
    console.warn('TitleBar onResized failed', e);
  }
});

onUnmounted(() => {
  if (typeof unlistenResize === 'function') {
    unlistenResize();
    unlistenResize = null;
  }
});

</script>

<style>
@media (max-width: 400px) {
  #responsiveDiv {
    visibility: hidden;
  }
}
@media (min-width: 400px) {
  #responsiveDiv {
    visibility: visible;
  }
}
</style>
