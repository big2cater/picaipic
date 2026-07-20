<template>
  <div class="sidebar-panel">
    <div class="sidebar-panel-header">
      <span class="sidebar-panel-header-title flex-1">{{ $t('album.smart_album_list') }}</span>
      <TButton
        :icon="IconAdd"
        :buttonSize="'small'"
        :tooltip="$t('album.add_smart_album')"
        @click="clickAdd"
      />
    </div>

    <ul
      v-if="albums.length > 0"
      class="flex-1 overflow-x-hidden overflow-y-auto rounded-box select-none"
    >
      <li v-for="album in albums" :key="album.id">
        <div
          :class="[
            'sidebar-item group',
            isSelected(album) ? 'sidebar-item-selected' : 'sidebar-item-hover',
          ]"
          @click="selectAlbum(album)"
        >
          <IconTag class="mx-1 w-5 h-5 shrink-0 opacity-70" />
          <span class="sidebar-item-label">{{ album.name }}</span>
          <span
            v-if="album.count != null"
            :class="['sidebar-item-count ml-auto', isSelected(album) ? 'hidden' : 'group-hover:hidden']"
          >
            {{ Number(album.count || 0).toLocaleString() }}
          </span>
          <div :class="['flex items-center', isSelected(album) ? '' : 'hidden group-hover:flex']">
            <ContextMenu
              :iconMenu="IconMore"
              :menuItems="menuItems(album)"
              :smallIcon="true"
            />
          </div>
        </div>
      </li>
    </ul>
    <div v-else class="mt-2 px-2 flex flex-col items-center justify-center text-base-content/30">
      <span class="text-sm text-center">{{ $t('album.no_smart_albums') }}</span>
    </div>

    <SmartAlbumEdit
      v-if="showEdit"
      :smartAlbum="editing"
      @ok="saveAlbum"
      @cancel="closeEdit"
    />
    <MessageBox
      v-if="deleting"
      :title="$t('album.smart_edit.delete')"
      :message="$t('album.smart_edit.delete_message', { name: deleting.name || '' })"
      :OkText="$t('album.smart_edit.delete')"
      :cancelText="$t('msgbox.cancel')"
      :warningOk="true"
      @ok="confirmDelete"
      @cancel="deleting = null"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { libConfig } from '@/common/config';
import { IconAdd, IconEdit, IconMore, IconTag, IconTrash } from '@/common/icons';
import TButton from '@/components/TButton.vue';
import ContextMenu from '@/components/ContextMenu.vue';
import MessageBox from '@/components/MessageBox.vue';
import SmartAlbumEdit from '@/components/SmartAlbumEdit.vue';

const { t } = useI18n();
const albums = computed(() => libConfig.smartAlbums || []);
const showEdit = ref(false);
const editing = ref<any | null>(null);
const deleting = ref<any | null>(null);

function isSelected(album: any) {
  return libConfig.smartAlbum?.type === 'custom' && libConfig.smartAlbum?.id === album.id;
}

function selectAlbum(album: any) {
  libConfig.activePane = 'smart';
  libConfig.smartAlbum = { type: 'custom', id: album.id };
  // leave collection mode if any
  if (libConfig.collection) {
    // keep selectedId but pane switches
  }
}

function clickAdd() {
  editing.value = null;
  showEdit.value = true;
}

function menuItems(album: any) {
  return [
    {
      label: t('album.smart_edit.title_edit'),
      icon: IconEdit,
      action: () => {
        editing.value = album;
        showEdit.value = true;
      },
    },
    {
      label: t('album.smart_edit.delete'),
      icon: IconTrash,
      action: () => {
        deleting.value = album;
      },
    },
  ];
}

function closeEdit() {
  showEdit.value = false;
  editing.value = null;
}

function saveAlbum(album: any) {
  const list = [...(libConfig.smartAlbums || [])];
  const idx = list.findIndex((x: any) => x.id === album.id);
  if (idx >= 0) list[idx] = album;
  else list.push(album);
  libConfig.smartAlbums = list;
  libConfig.activePane = 'smart';
  libConfig.smartAlbum = { type: 'custom', id: album.id };
  closeEdit();
}

function confirmDelete() {
  const album = deleting.value;
  if (!album) return;
  libConfig.smartAlbums = (libConfig.smartAlbums || []).filter((x: any) => x.id !== album.id);
  if (libConfig.smartAlbum?.type === 'custom' && libConfig.smartAlbum?.id === album.id) {
    libConfig.smartAlbum = { type: null, id: null };
    libConfig.activePane = 'main';
  }
  deleting.value = null;
}
</script>
