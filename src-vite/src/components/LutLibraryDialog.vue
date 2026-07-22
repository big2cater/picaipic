<template>
  <ModalDialog :title="$t('photo_style.lut_library_title')" :width="560" :height="520" @cancel="emit('cancel')">
    <div class="flex flex-col gap-3 text-sm h-full min-h-0 select-none">
      <div class="flex flex-wrap gap-2">
        <button type="button" class="t-button-default text-xs" :disabled="busy" @click="importOne">
          {{ $t('photo_style.lut_import') }}
        </button>
        <button type="button" class="t-button-default text-xs" :disabled="busy" @click="refresh">
          {{ $t('photo_style.lut_refresh') }}
        </button>
        <span class="ml-auto text-[11px] text-base-content/40">{{ entries.length }}</span>
      </div>

      <div v-if="error" class="text-xs text-error">{{ error }}</div>
      <div v-if="busy" class="text-xs text-base-content/50 flex items-center gap-2">
        <span class="loading loading-spinner loading-xs"></span>
        {{ $t('photo_style.lut_working') }}
      </div>

      <div class="flex-1 min-h-0 overflow-auto border border-base-content/10 rounded-box">
        <div v-if="!entries.length" class="p-6 text-center text-base-content/40 text-xs">
          {{ $t('photo_style.lut_empty') }}
        </div>
        <div
          v-for="e in entries"
          :key="e.id"
          class="flex items-center gap-2 px-3 py-2 border-b border-base-content/5 hover:bg-base-200/40"
          :class="selectedId === e.id ? 'bg-primary/10' : ''"
          @click="selectedId = e.id"
        >
          <button
            type="button"
            class="text-base-content/40 hover:text-warning"
            :title="$t('photo_style.lut_favorite')"
            @click.stop="toggleFavorite(e)"
          >{{ e.favorite ? '★' : '☆' }}</button>
          <div class="min-w-0 flex-1">
            <div class="truncate font-medium text-xs">{{ e.name }}</div>
            <div class="truncate text-[10px] text-base-content/40">{{ e.fileName }}</div>
          </div>
          <button
            type="button"
            class="t-button-default text-[10px]"
            @click.stop="renameEntry(e)"
          >{{ $t('photo_style.lut_rename') }}</button>
          <button
            type="button"
            class="t-button-default text-[10px] text-error"
            @click.stop="removeEntry(e)"
          >{{ $t('photo_style.lut_delete') }}</button>
        </div>
      </div>

      <div class="flex justify-end gap-2 pt-1">
        <button type="button" class="t-button-default text-xs" @click="emit('cancel')">
          {{ $t('msgbox.cancel') }}
        </button>
        <button
          type="button"
          class="btn btn-sm btn-primary"
          :disabled="!selectedId"
          @click="choose"
        >{{ $t('photo_style.lut_use') }}</button>
      </div>
    </div>

    <MessageBox
      v-if="showRename"
      :title="$t('photo_style.lut_rename')"
      :message="$t('photo_style.lut_rename_prompt')"
      :showInput="true"
      :inputText="renameValue"
      :inputPlaceholder="$t('photo_style.lut_rename_prompt')"
      :OkText="$t('msgbox.ok')"
      :cancelText="$t('msgbox.cancel')"
      @ok="onRenameOk"
      @cancel="showRename = false"
    />
  </ModalDialog>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { open as openDialog, ask } from '@tauri-apps/plugin-dialog';
import ModalDialog from '@/components/ModalDialog.vue';
import MessageBox from '@/components/MessageBox.vue';
import {
  listLutLibrary,
  importLutFile,
  deleteLutEntry,
  updateLutEntry,
} from '@/common/api';

const emit = defineEmits<{
  cancel: [];
  select: [id: string];
}>();

const props = defineProps<{
  initialSelectedId?: string;
}>();

const { t } = useI18n();
const entries = ref<any[]>([]);
const selectedId = ref(props.initialSelectedId || '');
const busy = ref(false);
const error = ref('');
const showRename = ref(false);
const renameValue = ref('');
const renameId = ref('');

async function refresh() {
  busy.value = true;
  error.value = '';
  try {
    entries.value = (await listLutLibrary()) || [];
  } catch (e: any) {
    error.value = String(e?.message || e || t('photo_style.lut_failed'));
  } finally {
    busy.value = false;
  }
}

async function importOne() {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: 'Cube LUT', extensions: ['cube'] }],
  });
  if (!selected) return;
  busy.value = true;
  error.value = '';
  try {
    const entry = await importLutFile(String(selected));
    await refresh();
    if (entry?.id) selectedId.value = entry.id;
  } catch (e: any) {
    error.value = String(e?.message || e || t('photo_style.lut_failed'));
  } finally {
    busy.value = false;
  }
}

async function toggleFavorite(e: any) {
  try {
    await updateLutEntry(e.id, { favorite: !e.favorite });
    await refresh();
  } catch (err: any) {
    error.value = String(err?.message || err);
  }
}

function renameEntry(e: any) {
  renameId.value = e.id;
  renameValue.value = e.name || '';
  showRename.value = true;
}

async function onRenameOk(name: string) {
  showRename.value = false;
  const n = String(name || '').trim();
  if (!n || !renameId.value) return;
  try {
    await updateLutEntry(renameId.value, { name: n });
    await refresh();
  } catch (err: any) {
    error.value = String(err?.message || err);
  }
}

async function removeEntry(e: any) {
  const ok = await ask(t('photo_style.lut_delete_confirm', { name: e.name }), {
    title: t('photo_style.lut_library_title'),
    kind: 'warning',
    okLabel: t('photo_style.lut_delete'),
    cancelLabel: t('msgbox.cancel'),
  });
  if (!ok) return;
  try {
    await deleteLutEntry(e.id);
    if (selectedId.value === e.id) selectedId.value = '';
    await refresh();
  } catch (err: any) {
    error.value = String(err?.message || err);
  }
}

function choose() {
  if (!selectedId.value) return;
  emit('select', selectedId.value);
}

onMounted(() => {
  void refresh();
});
</script>
