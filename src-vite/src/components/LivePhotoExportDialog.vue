<template>
  <ModalDialog
    :title="$t('live_photo.export_title')"
    :width="480"
    @cancel="clickCancel"
  >
    <div class="flex flex-col gap-3 text-sm select-none">
      <div class="text-base-content/50">
        {{ $t('live_photo.export_hint') }}
      </div>

      <div class="flex flex-col gap-2">
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            v-model="mode"
            type="radio"
            class="radio radio-xs radio-primary"
            value="still"
          />
          <span>{{ $t('live_photo.export_still') }}</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            v-model="mode"
            type="radio"
            class="radio radio-xs radio-primary"
            value="video"
          />
          <span>{{ $t('live_photo.export_video') }}</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            v-model="mode"
            type="radio"
            class="radio radio-xs radio-primary"
            value="pair"
          />
          <span>{{ $t('live_photo.export_pair') }}</span>
        </label>

        <div class="border-t border-base-content/10 my-1"></div>

        <label
          class="flex items-center gap-2 cursor-pointer"
          :class="{ 'opacity-40 cursor-not-allowed': !canToMotion }"
        >
          <input
            v-model="mode"
            type="radio"
            class="radio radio-xs radio-primary"
            value="to_motion"
            :disabled="!canToMotion"
          />
          <span>{{ $t('live_photo.export_to_motion') }}</span>
        </label>
        <label
          class="flex items-center gap-2 cursor-pointer"
          :class="{ 'opacity-40 cursor-not-allowed': !canToPair }"
        >
          <input
            v-model="mode"
            type="radio"
            class="radio radio-xs radio-primary"
            value="to_pair"
            :disabled="!canToPair"
          />
          <span>{{ $t('live_photo.export_to_pair') }}</span>
        </label>
        <label class="flex items-center gap-2 cursor-pointer">
          <input
            v-model="mode"
            type="radio"
            class="radio radio-xs radio-primary"
            value="set_keyframe"
          />
          <span>{{ $t('live_photo.export_keyframe') }}</span>
        </label>
      </div>

      <div v-if="mode === 'set_keyframe'" class="flex items-center gap-2">
        <span class="text-base-content/50 shrink-0">{{ $t('live_photo.keyframe_sec') }}</span>
        <input
          v-model.number="keyframeSec"
          type="number"
          min="0"
          step="0.1"
          class="input input-xs input-bordered w-24 bg-base-100/40"
        />
      </div>

      <label class="flex items-center gap-2 cursor-pointer text-base-content/70">
        <input
          v-model="replaceExisting"
          type="checkbox"
          class="checkbox checkbox-xs checkbox-primary"
        />
        <span>{{ $t('live_photo.export_replace') }}</span>
      </label>
    </div>

    <div class="flex justify-end items-center gap-2 shrink-0 pt-4">
      <button
        class="px-4 py-1.5 rounded-box text-base-content/70 hover:bg-base-100/30 cursor-pointer text-sm"
        :disabled="isProcessing"
        @click="clickCancel"
      >
        {{ $t('msgbox.cancel') }}
      </button>
      <button
        class="px-4 py-1.5 rounded-box text-sm cursor-pointer"
        :class="!isProcessing
          ? 'bg-primary text-primary-content hover:opacity-90'
          : 'bg-base-100/40 text-base-content/30 cursor-default'"
        :disabled="isProcessing"
        @click="doExport"
      >
        {{ isProcessing ? $t('live_photo.exporting') : $t('live_photo.export_action') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { open as openDialog, save } from '@tauri-apps/plugin-dialog';
import ModalDialog from '@/components/ModalDialog.vue';
import { exportLivePhoto } from '@/common/api';
import { useToast } from '@/common/toast';

type ExportMode =
  | 'still'
  | 'video'
  | 'pair'
  | 'to_motion'
  | 'to_pair'
  | 'set_keyframe';

const props = defineProps<{
  file: {
    id: number;
    name?: string;
    live_photo_type?: number | null;
    file_type?: number;
  };
}>();

const emit = defineEmits<{
  (e: 'done', outputs: string[]): void;
  (e: 'cancel'): void;
}>();

const { t } = useI18n();
const toast = useToast();

const liveType = computed(() => Number(props.file?.live_photo_type || 0));
// Apple image (1) or video (2) can convert to Motion if pair exists (backend checks).
const canToMotion = computed(() => liveType.value === 1 || liveType.value === 2);
const canToPair = computed(() => liveType.value === 3);

const mode = ref<ExportMode>('still');
const replaceExisting = ref(false);
const keyframeSec = ref(0);
const isProcessing = ref(false);

function stemFromName(name?: string): string {
  if (!name) return 'live_photo';
  const idx = name.lastIndexOf('.');
  return idx > 0 ? name.slice(0, idx) : name;
}

function defaultStillName(): string {
  const stem = stemFromName(props.file?.name);
  const type = liveType.value;
  if (type === 3 || mode.value === 'set_keyframe' || mode.value === 'to_motion') {
    return `${stem}.jpg`;
  }
  return props.file?.name || `${stem}.jpg`;
}

function defaultVideoName(): string {
  const stem = stemFromName(props.file?.name);
  if (liveType.value === 3) return `${stem}.mp4`;
  return `${stem}.mov`;
}

function defaultMotionName(): string {
  return `${stemFromName(props.file?.name)}.jpg`;
}

const needsFolder = computed(
  () => mode.value === 'pair' || mode.value === 'to_pair'
);

const clickCancel = () => {
  if (isProcessing.value) return;
  emit('cancel');
};

const doExport = async () => {
  if (isProcessing.value) return;
  if (mode.value === 'to_motion' && !canToMotion.value) {
    toast.error(t('live_photo.export_to_motion_need_apple'));
    return;
  }
  if (mode.value === 'to_pair' && !canToPair.value) {
    toast.error(t('live_photo.export_to_pair_need_motion'));
    return;
  }

  isProcessing.value = true;
  try {
    const conflict = replaceExisting.value ? 'replace' : 'keep_both';
    let destPath: string | null = null;
    let destDir: string | null = null;

    if (needsFolder.value) {
      destDir = await openDialog({
        directory: true,
        multiple: false,
        title: t('live_photo.export_choose_folder'),
      });
      if (!destDir) {
        isProcessing.value = false;
        return;
      }
    } else {
      let filters: { name: string; extensions: string[] }[];
      let defaultPath: string;
      if (mode.value === 'video') {
        filters = [
          { name: 'Video', extensions: ['mp4', 'mov', 'm4v'] },
          { name: 'All', extensions: ['*'] },
        ];
        defaultPath = defaultVideoName();
      } else if (mode.value === 'to_motion') {
        filters = [
          { name: 'Motion Photo JPEG', extensions: ['jpg', 'jpeg'] },
          { name: 'All', extensions: ['*'] },
        ];
        defaultPath = defaultMotionName();
      } else {
        filters = [
          { name: 'Image', extensions: ['jpg', 'jpeg', 'heic', 'heif', 'png', 'webp'] },
          { name: 'All', extensions: ['*'] },
        ];
        defaultPath = defaultStillName();
      }
      destPath = await save({
        title: t('live_photo.export_title'),
        defaultPath,
        filters,
      });
      if (!destPath) {
        isProcessing.value = false;
        return;
      }
    }

    const result = await exportLivePhoto({
      fileId: props.file.id,
      mode: mode.value,
      destPath: destPath || undefined,
      destDir: destDir || undefined,
      options: {
        conflict,
        keyframeSec: mode.value === 'set_keyframe' ? Number(keyframeSec.value) || 0 : undefined,
        stampContentId: mode.value === 'to_pair' ? true : undefined,
      },
    });

    if (!result?.outputs?.length) {
      throw new Error(t('live_photo.export_failed'));
    }

    toast.success(
      t('live_photo.export_success', {
        count: result.outputs.length,
        path: result.outputs[0],
      })
    );
    emit('done', result.outputs);
  } catch (error: any) {
    toast.error(error?.message || error?.toString?.() || t('live_photo.export_failed'));
  } finally {
    isProcessing.value = false;
  }
};
</script>
