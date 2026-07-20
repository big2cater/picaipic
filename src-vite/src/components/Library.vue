<template>

  <div class="sidebar-panel">
    <!-- Quick entries: All / Favorites / On this day -->
    <div class="shrink-0">
      <div
        v-for="item in libraryItems"
        :key="item.id"
        :class="[
          'sidebar-item',
          isQuickItemSelected(item.id) ? 'sidebar-item-selected' : 'sidebar-item-hover',
        ]"
        @click="selectQuickItem(item.id)"
      >
        <component :is="item.icon" class="mx-1 w-5 h-5 shrink-0" />
        <div class="sidebar-item-label">
          <span>{{ item.label }}</span>
        </div>
        <div class="ml-auto flex items-center">
          <span v-if="item.count && item.count > 0" class="sidebar-item-count">
            {{ item.count.toLocaleString() }}
          </span>
        </div>
      </div>
    </div>

    <!-- Albums / folders tree (owns its own scroll) -->
    <AlbumList ref="albumListRef"
      :key="albumListKey"
      selectionSource="album"
    />
  </div>

</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useI18n } from 'vue-i18n';
import { listen } from '@tauri-apps/api/event';
import { libConfig } from '@/common/config';
import { LIB_ITEM, type LibItem } from '@/common/constants';
import { IconPhotoAll, IconHeartFilled, IconHistory } from '@/common/icons';
import { getTotalCountAndSum, getQueryCountAndSum } from '@/common/api';
import AlbumList from '@/components/AlbumList.vue';

defineProps({
  titlebar: {
    type: String,
    required: true
  }
});

const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[locale.value] as any);

const totalCount = ref(0);
const favoriteCount = ref(0);
const todayCount = ref(0);

const albumListRef = ref<InstanceType<typeof AlbumList> | null>(null);
let unlistenLibraryTotalRefreshed: (() => void) | null = null;
let unlistenLibrarySwitched: (() => void) | null = null;

// refresh component
const albumListKey = ref(0);

// Check if there are any albums
const hasAlbums = computed(() => (albumListRef.value?.albums?.length ?? 0) > 0);

const buildQueryParams = ({ isFavorite = false, startDate = 0, endDate = 0 } = {}) => ({
  searchFileType: 0,
  sortType: 0,
  sortOrder: 0,
  searchFileName: "",
  searchAllSubfolders: "",
  searchFolder: "",
  startDate,
  endDate,
  calendarSort: 0,
  make: "",
  model: "",
  lensMake: "",
  lensModel: "",
  locationAdmin1: "",
  locationName: "",
  isFavorite,
  rating: -1,
  tagId: 0,
  personId: 0,
});

const libraryItems = computed(() => [
  {
    id: LIB_ITEM.ALL,
    label: localeMsg.value.library?.all_files || localeMsg.value.album?.all_files || 'All files',
    icon: IconPhotoAll,
    count: totalCount.value,
  },
  {
    id: LIB_ITEM.FAV,
    label: localeMsg.value.library?.favorites || localeMsg.value.favorite?.files || 'Favorites',
    icon: IconHeartFilled,
    count: favoriteCount.value,
  },
  {
    id: LIB_ITEM.TODAY,
    label: localeMsg.value.library?.on_this_day || localeMsg.value.calendar?.on_this_day || 'On this day',
    icon: IconHistory,
    count: todayCount.value,
  },
]);

function isQuickItemSelected(itemId: LibItem) {
  const current = (libConfig.library as any)?.item || LIB_ITEM.ALL;
  // Favorites / Today are selected only when that quick entry is active
  if (itemId === LIB_ITEM.FAV || itemId === LIB_ITEM.TODAY) {
    return current === itemId;
  }
  // All files: quick entry selected when library.item is all-files and album.id === 0
  return current === LIB_ITEM.ALL && Number(libConfig.album.id || 0) === 0;
}

function selectQuickItem(itemId: LibItem) {
  if (!libConfig.library) {
    (libConfig as any).library = { item: LIB_ITEM.ALL };
  }
  libConfig.library.item = itemId;
  // Clear album/folder selection so Content routes the library-wide query
  libConfig.album.id = 0;
  libConfig.album.folderId = null;
  libConfig.album.folderPath = '';
  libConfig.album.selected = false;
  // Leave collection/smart panes if user jumps back into library shortcuts
  if (libConfig.activePane === 'collection' || libConfig.activePane === 'smart') {
    libConfig.activePane = 'main';
  }
}

const refreshTotalCount = async () => {
  const result = await getTotalCountAndSum();
  totalCount.value = result ? result[0] : 0;
};

const refreshFavoriteCount = async () => {
  const result = await getQueryCountAndSum(buildQueryParams({ isFavorite: true }));
  favoriteCount.value = result ? Number(result[0]) : 0;
};

const refreshTodayCount = async () => {
  const result = await getQueryCountAndSum(buildQueryParams({ startDate: -1, endDate: -1 }));
  todayCount.value = result ? Number(result[0]) : 0;
};

const refreshAllCounts = async () => {
  await Promise.all([
    refreshTotalCount(),
    refreshFavoriteCount(),
    refreshTodayCount(),
  ]);
};

onMounted(async () => {
  if (!libConfig.library) {
    (libConfig as any).library = { item: LIB_ITEM.ALL };
  }
  await refreshAllCounts();
  unlistenLibraryTotalRefreshed = await listen('library-total-refreshed', refreshAllCounts);
  unlistenLibrarySwitched = await listen('library-switched', async () => {
    albumListKey.value++;          // force AlbumList remount
    await refreshAllCounts();
  });
});

onBeforeUnmount(() => {
  if (unlistenLibraryTotalRefreshed) {
    unlistenLibraryTotalRefreshed();
    unlistenLibraryTotalRefreshed = null;
  }
  if (unlistenLibrarySwitched) {
    unlistenLibrarySwitched();
    unlistenLibrarySwitched = null;
  }
});

defineExpose({
  albumListRef,
  hasAlbums,
});

</script>
