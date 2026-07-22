<template>
  <ModalDialog
    :title="isEdit ? $t('album.smart_edit.title_edit') : $t('album.smart_edit.title_add')"
    :width="560"
    @cancel="emit('cancel')"
  >
    <div class="flex flex-col gap-3 text-sm select-none max-h-[70vh] overflow-auto">
      <label class="form-control w-full">
        <span class="label-text text-xs opacity-70 mb-1">{{ $t('album.smart_edit.name') }}</span>
        <input
          v-model="name"
          type="text"
          maxlength="64"
          class="input input-bordered input-sm w-full"
          :placeholder="$t('album.smart_edit.name_placeholder')"
        />
      </label>

      <div class="flex items-center gap-2">
        <span class="text-xs opacity-70">{{ $t('album.smart_edit.match') }}</span>
        <select v-model="matchMode" class="select select-bordered select-sm">
          <option value="all">{{ $t('album.smart_edit.match_all') }}</option>
          <option value="any">{{ $t('album.smart_edit.match_any') }}</option>
        </select>
        <span class="text-xs opacity-50 ml-auto">{{ rules.length }}/20</span>
      </div>

      <div class="space-y-2">
        <div
          v-for="(rule, idx) in rules"
          :key="rule.id"
          class="rounded-box border border-base-content/10 p-2 space-y-1.5"
        >
          <div class="flex gap-1.5 flex-wrap items-center">
            <select v-model="rule.field" class="select select-bordered select-xs min-w-[7rem]" @change="onFieldChange(rule)">
              <option v-for="f in fieldOptions" :key="f.id" :value="f.id">{{ f.label }}</option>
            </select>
            <select v-model="rule.operator" class="select select-bordered select-xs min-w-[6rem]">
              <option v-for="op in operatorsFor(rule.field)" :key="op.id" :value="op.id">{{ op.label }}</option>
            </select>
            <button type="button" class="btn btn-ghost btn-xs ml-auto" @click="removeRule(idx)">×</button>
          </div>

          <!-- value controls -->
          <div v-if="needsValue(rule)" class="flex gap-1.5 flex-wrap items-center">
            <template v-if="rule.field === 'favorite' || rule.field === 'has_gps'">
              <select v-model="rule.value" class="select select-bordered select-xs">
                <option :value="true">{{ $t('album.smart_edit.yes') }}</option>
                <option :value="false">{{ $t('album.smart_edit.no') }}</option>
              </select>
            </template>
            <template v-else-if="rule.field === 'rating'">
              <select v-if="!['empty','not_empty'].includes(rule.operator)" v-model.number="rule.value" class="select select-bordered select-xs">
                <option v-for="n in [5,4,3,2,1,0]" :key="n" :value="n">{{ n }}</option>
              </select>
            </template>
            <template v-else-if="rule.field === 'file_type'">
              <select v-model.number="rule.value" class="select select-bordered select-xs">
                <option :value="1">{{ $t('album.smart_edit.type_image') }}</option>
                <option :value="2">{{ $t('album.smart_edit.type_video') }}</option>
                <option :value="4">{{ $t('album.smart_edit.type_raw') }}</option>
                <option :value="8">{{ $t('album.smart_edit.type_live') }}</option>
              </select>
            </template>
            <template v-else-if="rule.field === 'orientation'">
              <select v-model="rule.value" class="select select-bordered select-xs">
                <option value="landscape">{{ $t('album.smart_edit.landscape') }}</option>
                <option value="portrait">{{ $t('album.smart_edit.portrait') }}</option>
                <option value="square">{{ $t('album.smart_edit.square') }}</option>
              </select>
            </template>
            <template v-else-if="['date_taken','date_created','date_modified'].includes(rule.field)">
              <template v-if="rule.operator === 'in_last' || rule.operator === 'older_than'">
                <input v-model.number="rule.value.amount" type="number" min="1" class="input input-bordered input-xs w-20" />
                <select v-model="rule.value.unit" class="select select-bordered select-xs">
                  <option value="day">{{ $t('album.smart_edit.unit_day') }}</option>
                  <option value="week">{{ $t('album.smart_edit.unit_week') }}</option>
                  <option value="month">{{ $t('album.smart_edit.unit_month') }}</option>
                  <option value="year">{{ $t('album.smart_edit.unit_year') }}</option>
                </select>
              </template>
              <template v-else-if="!['empty','not_empty'].includes(rule.operator)">
                <input
                  :value="tsToDateInput(rule.value)"
                  type="date"
                  class="input input-bordered input-xs"
                  @change="rule.value = dateInputToTs(($event.target as HTMLInputElement).value)"
                />
              </template>
            </template>
            <template v-else-if="rule.field === 'size'">
              <input v-model.number="rule.value" type="number" min="0" step="0.1" class="input input-bordered input-xs w-28" />
              <span class="text-xs opacity-50">MB</span>
            </template>
            <template v-else-if="rule.field === 'tag'">
              <select
                v-if="!['empty','not_empty'].includes(rule.operator)"
                v-model.number="rule.value"
                class="select select-bordered select-xs min-w-[8rem]"
              >
                <option :value="0" disabled>{{ $t('album.smart_edit.pick_tag') }}</option>
                <option v-for="tag in tags" :key="tag.id" :value="tag.id">{{ tag.name }}</option>
              </select>
            </template>
            <template v-else>
              <input v-model="rule.value" type="text" class="input input-bordered input-xs flex-1 min-w-[8rem]" />
            </template>
          </div>
        </div>
      </div>

      <button
        type="button"
        class="t-button-default text-xs self-start"
        :disabled="rules.length >= 20"
        @click="addRule"
      >
        + {{ $t('album.smart_edit.add_rule') }}
      </button>

      <p v-if="errorMessage" class="text-error text-xs">{{ errorMessage }}</p>

      <div class="flex justify-end gap-2 pt-1">
        <button class="t-button-default" @click="emit('cancel')">{{ $t('msgbox.cancel') }}</button>
        <button class="t-button-primary" @click="submit">{{ $t('msgbox.ok') }}</button>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import { getAllTags } from '@/common/api';

const props = defineProps<{
  smartAlbum?: any | null;
}>();

const emit = defineEmits<{
  cancel: [];
  ok: [any];
}>();

const { t } = useI18n();
const isEdit = computed(() => !!props.smartAlbum?.id);
const name = ref(props.smartAlbum?.name || '');
const matchMode = ref<'all' | 'any'>(props.smartAlbum?.query?.match || 'all');
const errorMessage = ref('');
const tags = ref<any[]>([]);

type Rule = {
  id: string;
  field: string;
  operator: string;
  value: any;
};

function newId() {
  return crypto.randomUUID?.() || `r_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

function defaultRule(): Rule {
  return { id: newId(), field: 'favorite', operator: 'is', value: true };
}

const rules = ref<Rule[]>(
  Array.isArray(props.smartAlbum?.query?.rules) && props.smartAlbum.query.rules.length
    ? JSON.parse(JSON.stringify(props.smartAlbum.query.rules))
    : [defaultRule()],
);

const fieldOptions = computed(() => [
  { id: 'favorite', label: t('album.smart_edit.field_favorite') },
  { id: 'rating', label: t('album.smart_edit.field_rating') },
  { id: 'name', label: t('album.smart_edit.field_name') },
  { id: 'file_type', label: t('album.smart_edit.field_file_type') },
  { id: 'extension', label: t('album.smart_edit.field_extension') },
  { id: 'date_taken', label: t('album.smart_edit.field_date_taken') },
  { id: 'date_created', label: t('album.smart_edit.field_date_created') },
  { id: 'date_modified', label: t('album.smart_edit.field_date_modified') },
  { id: 'size', label: t('album.smart_edit.field_size') },
  { id: 'orientation', label: t('album.smart_edit.field_orientation') },
  { id: 'tag', label: t('album.smart_edit.field_tag') },
  { id: 'person', label: t('album.smart_edit.field_person') },
  { id: 'has_gps', label: t('album.smart_edit.field_has_gps') },
  { id: 'camera', label: t('album.smart_edit.field_camera') },
  { id: 'lens', label: t('album.smart_edit.field_lens') },
]);

function operatorsFor(field: string) {
  const common = [
    { id: 'is', label: t('album.smart_edit.op_is') },
    { id: 'is_not', label: t('album.smart_edit.op_is_not') },
  ];
  if (field === 'name') {
    return [
      { id: 'contains', label: t('album.smart_edit.op_contains') },
      { id: 'not_contains', label: t('album.smart_edit.op_not_contains') },
      { id: 'is', label: t('album.smart_edit.op_is') },
    ];
  }
  if (field === 'rating' || field === 'size') {
    return [
      ...common,
      { id: 'gt', label: '>' },
      { id: 'gte', label: '≥' },
      { id: 'lt', label: '<' },
      { id: 'lte', label: '≤' },
      { id: 'empty', label: t('album.smart_edit.op_empty') },
      { id: 'not_empty', label: t('album.smart_edit.op_not_empty') },
    ];
  }
  if (field.startsWith('date_')) {
    return [
      { id: 'before', label: t('album.smart_edit.op_before') },
      { id: 'after', label: t('album.smart_edit.op_after') },
      { id: 'in_last', label: t('album.smart_edit.op_in_last') },
      { id: 'older_than', label: t('album.smart_edit.op_older_than') },
      { id: 'empty', label: t('album.smart_edit.op_empty') },
      { id: 'not_empty', label: t('album.smart_edit.op_not_empty') },
    ];
  }
  if (field === 'tag' || field === 'person') {
    return [
      { id: 'has', label: t('album.smart_edit.op_has') },
      { id: 'not_has', label: t('album.smart_edit.op_not_has') },
      { id: 'empty', label: t('album.smart_edit.op_empty') },
      { id: 'not_empty', label: t('album.smart_edit.op_not_empty') },
    ];
  }
  if (field === 'favorite' || field === 'has_gps' || field === 'orientation' || field === 'file_type') {
    return [{ id: 'is', label: t('album.smart_edit.op_is') }];
  }
  return common;
}

function needsValue(rule: Rule) {
  return !['empty', 'not_empty'].includes(rule.operator);
}

function onFieldChange(rule: Rule) {
  const ops = operatorsFor(rule.field);
  rule.operator = ops[0]?.id || 'is';
  if (rule.field === 'favorite' || rule.field === 'has_gps') rule.value = true;
  else if (rule.field === 'rating') rule.value = 5;
  else if (rule.field === 'file_type') rule.value = 1;
  else if (rule.field === 'orientation') rule.value = 'landscape';
  else if (rule.field.startsWith('date_')) rule.value = { amount: 30, unit: 'day' };
  else if (rule.field === 'size') rule.value = 5;
  else if (rule.field === 'tag' || rule.field === 'person') rule.value = 0;
  else rule.value = '';
}

function addRule() {
  if (rules.value.length >= 20) return;
  rules.value.push(defaultRule());
}
function removeRule(idx: number) {
  rules.value.splice(idx, 1);
  if (!rules.value.length) rules.value.push(defaultRule());
}

function tsToDateInput(ts: any) {
  const n = Number(ts);
  if (!n) return '';
  const d = new Date(n * 1000);
  if (Number.isNaN(d.getTime())) return '';
  return d.toISOString().slice(0, 10);
}
function dateInputToTs(s: string) {
  if (!s) return 0;
  const d = new Date(s + 'T00:00:00');
  return Math.floor(d.getTime() / 1000);
}

onMounted(async () => {
  try {
    const list = await getAllTags();
    tags.value = Array.isArray(list) ? list : [];
  } catch {
    tags.value = [];
  }
});

function submit() {
  errorMessage.value = '';
  const n = name.value.trim();
  if (!n) {
    errorMessage.value = t('album.smart_edit.name_required');
    return;
  }
  if (!rules.value.length) {
    errorMessage.value = t('album.smart_edit.rules_required');
    return;
  }
  // normalize date relative values
  const normalized = rules.value.map((r) => {
    const copy = { ...r, value: typeof r.value === 'object' && r.value ? { ...r.value } : r.value };
    if (['date_taken', 'date_created', 'date_modified'].includes(r.field)) {
      if (r.operator === 'in_last' || r.operator === 'older_than') {
        if (typeof r.value !== 'object' || r.value === null) {
          copy.value = { amount: Number(r.value) || 7, unit: 'day' };
        }
      }
    }
    return copy;
  });

  const now = Math.floor(Date.now() / 1000);
  const payload = {
    id: props.smartAlbum?.id || newId(),
    name: n,
    description: props.smartAlbum?.description || '',
    source: 'rules',
    query: {
      version: 1,
      match: matchMode.value,
      rules: normalized,
    },
    group: props.smartAlbum?.group || { type: 0 },
    sort: props.smartAlbum?.sort || { type: 0, order: 1 },
    coverFileId: props.smartAlbum?.coverFileId ?? null,
    count: props.smartAlbum?.count ?? null,
    createdAt: props.smartAlbum?.createdAt || now,
    updatedAt: now,
  };
  emit('ok', payload);
}
</script>
