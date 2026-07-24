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

      <div class="flex items-center gap-2 flex-wrap">
        <span class="text-xs opacity-70">{{ $t('album.smart_edit.sort') }}</span>
        <select v-model.number="sortType" class="select select-bordered select-sm min-w-[7rem]">
          <option v-for="(label, idx) in sortTypeOptions" :key="idx" :value="idx">{{ label }}</option>
        </select>
        <select
          v-model.number="sortOrder"
          class="select select-bordered select-sm"
          :disabled="sortType === 8"
        >
          <option v-for="(label, idx) in sortOrderOptions" :key="idx" :value="idx">{{ label }}</option>
        </select>
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
            <select
              v-model="rule.operator"
              class="select select-bordered select-xs min-w-[6rem]"
              @change="onOperatorChange(rule)"
            >
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
                <input
                  :value="Number(rule.value?.amount) || 1"
                  type="number"
                  min="1"
                  class="input input-bordered input-xs w-20"
                  @input="setRelativeDateAmount(rule, ($event.target as HTMLInputElement).value)"
                />
                <select
                  :value="rule.value?.unit || 'day'"
                  class="select select-bordered select-xs"
                  @change="setRelativeDateUnit(rule, ($event.target as HTMLSelectElement).value)"
                >
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
              <template v-if="!['empty','not_empty'].includes(rule.operator)">
                <input v-model.number="rule.value" type="number" min="0" step="0.1" class="input input-bordered input-xs w-28" />
                <span class="text-xs opacity-50">MB</span>
              </template>
            </template>
            <template v-else-if="rule.field === 'tag'">
              <template v-if="!['empty','not_empty'].includes(rule.operator)">
                <select
                  v-if="tags.length"
                  v-model.number="rule.value"
                  class="select select-bordered select-xs min-w-[8rem]"
                >
                  <option :value="0" disabled>{{ $t('album.smart_edit.pick_tag') }}</option>
                  <option v-for="tag in tags" :key="tag.id" :value="tag.id">{{ tag.name }}</option>
                </select>
                <span v-else class="text-xs opacity-50">{{ $t('album.smart_edit.no_tags') }}</span>
              </template>
            </template>
            <template v-else-if="rule.field === 'person'">
              <template v-if="!['empty','not_empty'].includes(rule.operator)">
                <select
                  v-if="persons.length"
                  v-model.number="rule.value"
                  class="select select-bordered select-xs min-w-[8rem]"
                >
                  <option :value="0" disabled>{{ $t('album.smart_edit.pick_person') }}</option>
                  <option v-for="person in persons" :key="person.id" :value="person.id">
                    {{ person.name || (`Person ${person.id}`) }}
                  </option>
                </select>
                <span v-else class="text-xs opacity-50">{{ $t('album.smart_edit.no_persons') }}</span>
              </template>
            </template>
            <template v-else-if="rule.field === 'camera'">
              <select
                v-if="cameraOptions.length"
                v-model="rule.value"
                class="select select-bordered select-xs min-w-[10rem]"
              >
                <option value="" disabled>{{ $t('album.smart_edit.pick_camera') }}</option>
                <option v-for="opt in cameraOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
              </select>
              <span v-else class="text-xs opacity-50">{{ $t('album.smart_edit.no_cameras') }}</span>
            </template>
            <template v-else-if="rule.field === 'lens'">
              <select
                v-if="lensOptions.length"
                v-model="rule.value"
                class="select select-bordered select-xs min-w-[10rem]"
              >
                <option value="" disabled>{{ $t('album.smart_edit.pick_lens') }}</option>
                <option v-for="opt in lensOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
              </select>
              <span v-else class="text-xs opacity-50">{{ $t('album.smart_edit.no_lenses') }}</span>
            </template>
            <template v-else-if="rule.field === 'extension'">
              <input
                v-model="rule.value"
                type="text"
                class="input input-bordered input-xs flex-1 min-w-[8rem]"
                :placeholder="$t('album.smart_edit.extension_placeholder')"
              />
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
import { getAllTags, getCameraInfo, getLensInfo, getPersons } from '@/common/api';

const props = defineProps<{
  smartAlbum?: any | null;
}>();

const emit = defineEmits<{
  cancel: [];
  ok: [any];
}>();

const { t, tm } = useI18n();
const isEdit = computed(() => !!props.smartAlbum?.id);
const name = ref(props.smartAlbum?.name || '');
const matchMode = ref<'all' | 'any'>(props.smartAlbum?.query?.match || 'all');
const sortType = ref(Number(props.smartAlbum?.sort?.type ?? 0));
const sortOrder = ref(Number(props.smartAlbum?.sort?.order ?? 1));
const errorMessage = ref('');
const tags = ref<any[]>([]);
const persons = ref<any[]>([]);
const cameras = ref<any[]>([]);
const lenses = ref<any[]>([]);

const sortTypeOptions = computed(() => {
  const opts = tm('toolbar.filter.sort_type_options');
  return Array.isArray(opts) ? (opts as string[]) : [];
});
const sortOrderOptions = computed(() => {
  const opts = tm('toolbar.filter.sort_order_options');
  return Array.isArray(opts) ? (opts as string[]) : [];
});

function flattenMakeModelOptions(groups: any[]) {
  const out: { value: string; label: string }[] = [];
  for (const g of groups || []) {
    const make = String(g?.make || '').trim();
    if (!make) continue;
    const models = Array.isArray(g?.models) ? g.models : [];
    for (const modelRaw of models) {
      const model = String(modelRaw || '').trim();
      if (!model) continue;
      out.push({
        value: `${make}||${model}`,
        label: `${make} ${model}`,
      });
    }
  }
  return out;
}

const cameraOptions = computed(() => flattenMakeModelOptions(cameras.value));
const lensOptions = computed(() => flattenMakeModelOptions(lenses.value));

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
  if (field === 'extension') {
    return common;
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
    // Default first op is relative window (amount+unit UI) — less confusing than bare date.
    return [
      { id: 'in_last', label: t('album.smart_edit.op_in_last') },
      { id: 'older_than', label: t('album.smart_edit.op_older_than') },
      { id: 'before', label: t('album.smart_edit.op_before') },
      { id: 'after', label: t('album.smart_edit.op_after') },
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

function defaultDateValue(operator: string) {
  if (operator === 'in_last' || operator === 'older_than') {
    return { amount: 30, unit: 'day' };
  }
  if (operator === 'empty' || operator === 'not_empty') {
    return null;
  }
  // before / after / between: local midnight today
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
}

function ensureRelativeDateValue(rule: Rule) {
  if (typeof rule.value !== 'object' || rule.value === null || Array.isArray(rule.value)) {
    rule.value = { amount: 30, unit: 'day' };
    return;
  }
  if (!Number(rule.value.amount)) rule.value.amount = 30;
  if (!rule.value.unit) rule.value.unit = 'day';
}

function setRelativeDateAmount(rule: Rule, raw: string) {
  ensureRelativeDateValue(rule);
  rule.value = {
    ...rule.value,
    amount: Math.max(1, Number(raw) || 1),
  };
}

function setRelativeDateUnit(rule: Rule, unit: string) {
  ensureRelativeDateValue(rule);
  rule.value = {
    ...rule.value,
    unit: unit || 'day',
  };
}

function onFieldChange(rule: Rule) {
  const ops = operatorsFor(rule.field);
  rule.operator = ops[0]?.id || 'is';
  if (rule.field === 'favorite' || rule.field === 'has_gps') rule.value = true;
  else if (rule.field === 'rating') rule.value = 5;
  else if (rule.field === 'file_type') rule.value = 1;
  else if (rule.field === 'orientation') rule.value = 'landscape';
  else if (rule.field.startsWith('date_')) rule.value = defaultDateValue(rule.operator);
  else if (rule.field === 'size') rule.value = 5;
  else if (rule.field === 'tag' || rule.field === 'person') rule.value = 0;
  else if (rule.field === 'camera') rule.value = cameraOptions.value[0]?.value || '';
  else if (rule.field === 'lens') rule.value = lensOptions.value[0]?.value || '';
  else if (rule.field === 'extension') rule.value = 'jpg';
  else rule.value = '';
}

function onOperatorChange(rule: Rule) {
  if (!rule.field.startsWith('date_')) return;
  const op = rule.operator;
  if (op === 'in_last' || op === 'older_than') {
    ensureRelativeDateValue(rule);
  } else if (op === 'empty' || op === 'not_empty') {
    rule.value = null;
  } else if (typeof rule.value === 'object' || rule.value == null || !Number(rule.value)) {
    // before / after need a day timestamp, not a relative object
    rule.value = defaultDateValue(op);
  }
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
  // Local calendar day (not UTC) — matches backend localtime day compare.
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}
function dateInputToTs(s: string) {
  if (!s) return 0;
  // Local midnight epoch for the selected calendar day.
  const d = new Date(s + 'T00:00:00');
  return Math.floor(d.getTime() / 1000);
}

onMounted(async () => {
  try {
    const [tagList, personList, cameraList, lensList] = await Promise.all([
      getAllTags(),
      getPersons(),
      getCameraInfo(),
      getLensInfo(),
    ]);
    tags.value = Array.isArray(tagList) ? tagList : [];
    persons.value = Array.isArray(personList) ? personList : [];
    cameras.value = Array.isArray(cameraList) ? cameraList : [];
    lenses.value = Array.isArray(lensList) ? lensList : [];
  } catch {
    tags.value = [];
    persons.value = [];
    cameras.value = [];
    lenses.value = [];
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
  // normalize date relative values + trim text fields
  const normalized = rules.value.map((r) => {
    const copy = { ...r, value: typeof r.value === 'object' && r.value ? { ...r.value } : r.value };
    if (['date_taken', 'date_created', 'date_modified'].includes(r.field)) {
      if (r.operator === 'in_last' || r.operator === 'older_than') {
        if (typeof r.value !== 'object' || r.value === null || Array.isArray(r.value)) {
          copy.value = { amount: Number(r.value) || 30, unit: 'day' };
        } else {
          copy.value = {
            amount: Math.max(1, Number(r.value.amount) || 30),
            unit: r.value.unit || 'day',
          };
        }
      } else if (r.operator === 'before' || r.operator === 'after') {
        if (typeof r.value === 'object' || !Number(r.value)) {
          copy.value = defaultDateValue(r.operator);
        }
      }
    }
    if (r.field === 'extension' || r.field === 'name') {
      copy.value = String(r.value ?? '').trim();
    }
    return copy;
  });

  for (const r of normalized) {
    if (['empty', 'not_empty'].includes(r.operator)) continue;
    if ((r.field === 'tag' || r.field === 'person') && !Number(r.value)) {
      errorMessage.value = r.field === 'tag'
        ? (tags.value.length ? t('album.smart_edit.pick_tag') : t('album.smart_edit.no_tags'))
        : (persons.value.length ? t('album.smart_edit.pick_person') : t('album.smart_edit.no_persons'));
      return;
    }
    if ((r.field === 'camera' || r.field === 'lens') && !String(r.value || '').includes('||')) {
      errorMessage.value = r.field === 'camera'
        ? (cameraOptions.value.length ? t('album.smart_edit.pick_camera') : t('album.smart_edit.no_cameras'))
        : (lensOptions.value.length ? t('album.smart_edit.pick_lens') : t('album.smart_edit.no_lenses'));
      return;
    }
    if ((r.field === 'name' || r.field === 'extension') && !String(r.value || '').trim()) {
      errorMessage.value = t('album.smart_edit.rules_required');
      return;
    }
  }

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
    sort: {
      type: Number(sortType.value) || 0,
      order: sortType.value === 8 ? 0 : (Number(sortOrder.value) || 0),
    },
    coverFileId: props.smartAlbum?.coverFileId ?? null,
    count: props.smartAlbum?.count ?? null,
    createdAt: props.smartAlbum?.createdAt || now,
    updatedAt: now,
  };
  emit('ok', payload);
}
</script>
