/**
 * Batch processing action model (Phase C).
 * Actions map to built-in ImageEditor / host edit_image capabilities.
 */

import {
  BUILTIN_PHOTO_SIZE_PRESETS,
  BUILTIN_RATIO_PRESETS,
  resolveCropPreset,
  type CustomCropRatio,
} from '@/common/photoSizePresets';

export type BatchActionType =
  | 'resize'
  | 'crop'
  | 'rotate'
  | 'flip'
  | 'brightness'
  | 'contrast'
  | 'saturation'
  | 'hue'
  | 'blur'
  | 'filter'
  | 'border'
  | 'expand'
  | 'watermark'
  | 'text'
  | 'colorMatch'
  | 'photoStyle';

export type BatchResizeMode = 'longEdge' | 'width' | 'height' | 'percent' | 'exact';
export type BatchCropMode = 'aspect' | 'photo';
export type BatchOverwritePolicy = 'skip' | 'overwrite' | 'rename';
export type BatchOutputMode = 'saveAs' | 'overwrite';
export type BatchNameMode = 'original' | 'prefix' | 'suffix' | 'sequence';
export type BatchAnchor =
  | 'center'
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'
  | 'top'
  | 'bottom'
  | 'left'
  | 'right';

export interface BatchActionBase {
  id: string;
  type: BatchActionType;
}

export interface BatchResizeAction extends BatchActionBase {
  type: 'resize';
  mode: BatchResizeMode;
  /** longEdge / width / height / exact */
  width?: number;
  height?: number;
  /** percent 1–500 */
  percent?: number;
}

export interface BatchCropAction extends BatchActionBase {
  type: 'crop';
  mode: BatchCropMode;
  /** ratio-* | photo-* | custom id */
  presetId: string;
  portrait?: boolean;
  /** When photo mode, also resize to declared px (default true). */
  applyTargetPixels?: boolean;
}

export interface BatchRotateAction extends BatchActionBase {
  type: 'rotate';
  degrees: 90 | 180 | 270;
}

export interface BatchFlipAction extends BatchActionBase {
  type: 'flip';
  axis: 'horizontal' | 'vertical';
}

export interface BatchBrightnessAction extends BatchActionBase {
  type: 'brightness';
  value: number; // -100..100
}

export interface BatchContrastAction extends BatchActionBase {
  type: 'contrast';
  value: number; // -100..100
}

export interface BatchSaturationAction extends BatchActionBase {
  type: 'saturation';
  value: number; // 0..200 (100 = normal, matches editor %)
}

export interface BatchHueAction extends BatchActionBase {
  type: 'hue';
  value: number; // -180..180
}

export interface BatchBlurAction extends BatchActionBase {
  type: 'blur';
  value: number; // 0..20
}

export interface BatchFilterAction extends BatchActionBase {
  type: 'filter';
  filter: 'grayscale' | 'sepia' | 'invert';
}

export interface BatchBorderAction extends BatchActionBase {
  type: 'border';
  /** Border thickness in px on each side. */
  width: number;
  color: string;
}

export interface BatchExpandAction extends BatchActionBase {
  type: 'expand';
  top: number;
  right: number;
  bottom: number;
  left: number;
  color: string;
}

export interface BatchWatermarkAction extends BatchActionBase {
  type: 'watermark';
  imagePath: string;
  position: BatchAnchor;
  /** Relative scale 1–100 (% of destination short edge). */
  scale: number;
  /** Opacity 0–100. */
  opacity: number;
  margin: number;
  includeCaptureTime?: boolean;
  captureTimeFormat?: 'datetime' | 'date' | 'time';
}

export interface BatchTextAction extends BatchActionBase {
  type: 'text';
  text: string;
  position: BatchAnchor;
  /** Font size in px (export canvas). */
  fontSize: number;
  color: string;
  opacity: number;
  margin: number;
  /** Stamp EXIF DateTimeOriginal (per file). */
  includeCaptureTime?: boolean;
  /** datetime | date | time */
  captureTimeFormat?: 'datetime' | 'date' | 'time';
}

export interface BatchColorMatchAction extends BatchActionBase {
  type: 'colorMatch';
  referenceFilePath: string;
  /** 0–100 UI percent */
  intensity: number;
  /** 0–100 UI percent */
  tonePreservation: number;
  autoWb: boolean;
  /** 0–100 UI percent */
  highlightProtection: number;
  /** 0–100 UI percent */
  shadowProtection: number;
}

export interface BatchPhotoStyleAction extends BatchActionBase {
  type: 'photoStyle';
  styleId: string;
  /** Optional inline style overrides; host uses style* fields when present. */
  styleBrightness?: number;
  styleContrast?: number;
  styleSaturation?: number;
  styleHue?: number;
  highlights?: number;
  shadows?: number;
  fade?: number;
  vignette?: number;
  grain?: number;
  filter?: string;
  lutId?: string;
  lutIntensity?: number;
}

export type BatchAction =
  | BatchResizeAction
  | BatchCropAction
  | BatchRotateAction
  | BatchFlipAction
  | BatchBrightnessAction
  | BatchContrastAction
  | BatchSaturationAction
  | BatchHueAction
  | BatchBlurAction
  | BatchFilterAction
  | BatchBorderAction
  | BatchExpandAction
  | BatchWatermarkAction
  | BatchTextAction
  | BatchColorMatchAction
  | BatchPhotoStyleAction;

export interface BatchActionTemplate {
  id: string;
  name: string;
  updatedAt: number;
  actions: BatchAction[];
}

export interface BatchFileItem {
  id?: number | string;
  file_path: string;
  name?: string;
  file_type?: number | null;
  thumbnail?: string;
  e_orientation?: number | null;
  orientation?: number | null;
  width?: number | null;
  height?: number | null;
}

export const BATCH_ACTION_PALETTE: readonly {
  type: BatchActionType;
  defaultFactory: () => Omit<BatchAction, 'id'>;
}[] = [
  {
    type: 'resize',
    defaultFactory: () => ({ type: 'resize', mode: 'longEdge', width: 1920 }),
  },
  {
    type: 'crop',
    defaultFactory: () => ({
      type: 'crop',
      mode: 'aspect',
      presetId: 'ratio-1-1',
      portrait: false,
      applyTargetPixels: true,
    }),
  },
  {
    type: 'rotate',
    defaultFactory: () => ({ type: 'rotate', degrees: 90 }),
  },
  {
    type: 'flip',
    defaultFactory: () => ({ type: 'flip', axis: 'horizontal' }),
  },
  {
    type: 'brightness',
    defaultFactory: () => ({ type: 'brightness', value: 10 }),
  },
  {
    type: 'contrast',
    defaultFactory: () => ({ type: 'contrast', value: 10 }),
  },
  {
    type: 'saturation',
    defaultFactory: () => ({ type: 'saturation', value: 120 }),
  },
  {
    type: 'hue',
    defaultFactory: () => ({ type: 'hue', value: 0 }),
  },
  {
    type: 'blur',
    defaultFactory: () => ({ type: 'blur', value: 2 }),
  },
  {
    type: 'filter',
    defaultFactory: () => ({ type: 'filter', filter: 'grayscale' }),
  },
  {
    type: 'border',
    defaultFactory: () => ({ type: 'border', width: 16, color: '#ffffff' }),
  },
  {
    type: 'expand',
    defaultFactory: () => ({
      type: 'expand',
      top: 40,
      right: 40,
      bottom: 40,
      left: 40,
      color: '#ffffff',
    }),
  },
  {
    type: 'watermark',
    defaultFactory: () => ({
      type: 'watermark',
      imagePath: '',
      position: 'bottom-right',
      scale: 18,
      opacity: 70,
      margin: 24,
      includeCaptureTime: false,
      captureTimeFormat: 'datetime',
    }),
  },
  {
    type: 'text',
    defaultFactory: () => ({
      type: 'text',
      text: 'PicAiPic',
      position: 'bottom-right',
      fontSize: 36,
      color: '#ffffff',
      opacity: 85,
      margin: 24,
      includeCaptureTime: false,
      captureTimeFormat: 'datetime',
    }),
  },
  {
    type: 'colorMatch',
    defaultFactory: () => ({
      type: 'colorMatch',
      referenceFilePath: '',
      intensity: 100,
      tonePreservation: 50,
      autoWb: true,
      highlightProtection: 80,
      shadowProtection: 80,
    }),
  },
] as const;

export const BATCH_TEMPLATE_LIMIT = 20;

export function createBatchActionId(): string {
  return `act-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function createBatchTemplateId(): string {
  return `tpl-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function createBatchAction(type: BatchActionType): BatchAction {
  const entry = BATCH_ACTION_PALETTE.find((p) => p.type === type);
  const body = entry ? entry.defaultFactory() : { type: 'resize', mode: 'longEdge' as const, width: 1920 };
  return { id: createBatchActionId(), ...(body as any) } as BatchAction;
}

export function isBatchImageFile(file: { file_type?: number | null } | null | undefined): boolean {
  const t = Number(file?.file_type || 0);
  return t === 1 || t === 3;
}

export function filterBatchImageFiles<T extends { file_type?: number | null }>(files: T[]): T[] {
  return (files || []).filter((f) => isBatchImageFile(f));
}

export function cropPresetOptions(custom: CustomCropRatio[] = []) {
  return [
    ...BUILTIN_RATIO_PRESETS.map((p) => ({
      id: p.id,
      mode: 'aspect' as const,
      label: p.label,
    })),
    ...BUILTIN_PHOTO_SIZE_PRESETS.map((p) => ({
      id: p.id,
      mode: 'photo' as const,
      labelKey: p.nameKey,
    })),
    ...custom.map((c) => ({
      id: c.id,
      mode: 'aspect' as const,
      label: `${c.name} (${c.ratioW}:${c.ratioH})`,
    })),
  ];
}

export function describeAction(
  action: BatchAction,
  t: (key: string, params?: Record<string, unknown>) => string,
  custom: CustomCropRatio[] = [],
): string {
  switch (action.type) {
    case 'resize': {
      if (action.mode === 'percent') return t('batch.action_resize_percent', { n: action.percent || 100 });
      if (action.mode === 'longEdge') return t('batch.action_resize_long', { n: action.width || 0 });
      if (action.mode === 'width') return t('batch.action_resize_w', { n: action.width || 0 });
      if (action.mode === 'height') return t('batch.action_resize_h', { n: action.height || 0 });
      return t('batch.action_resize_exact', { w: action.width || 0, h: action.height || 0 });
    }
    case 'crop': {
      const preset = resolveCropPreset(action.presetId, custom);
      if (preset.kind === 'photo') {
        return t('batch.action_crop_photo', { id: action.presetId });
      }
      if (preset.kind === 'free') return t('batch.action_crop');
      const label =
        preset.kind === 'custom'
          ? preset.name
          : preset.kind === 'ratio'
            ? preset.label
            : action.presetId;
      return t('batch.action_crop_aspect', { label });
    }
    case 'rotate':
      return t('batch.action_rotate', { n: action.degrees });
    case 'flip':
      return action.axis === 'horizontal' ? t('batch.action_flip_h') : t('batch.action_flip_v');
    case 'brightness':
      return t('batch.action_brightness', { n: action.value });
    case 'contrast':
      return t('batch.action_contrast', { n: action.value });
    case 'saturation':
      return t('batch.action_saturation', { n: action.value });
    case 'hue':
      return t('batch.action_hue', { n: action.value });
    case 'blur':
      return t('batch.action_blur', { n: action.value });
    case 'filter':
      return t(`batch.action_filter_${action.filter}`);
    case 'border':
      return t('batch.action_border', { n: action.width, color: action.color });
    case 'expand':
      return t('batch.action_expand', {
        t: action.top,
        r: action.right,
        b: action.bottom,
        l: action.left,
      });
    case 'watermark':
      return t('batch.action_watermark', {
        pos: t(`batch.pos_${(action.position || 'bottom-right').replace('-', '_')}`),
      });
    case 'text': {
      if ((action as any).includeCaptureTime) {
        return t('batch.action_text_capture_time');
      }
      const sample = String(action.text || '').slice(0, 16);
      return t('batch.action_text', { text: sample || '…' });
    }
    case 'colorMatch': {
      const path = String(action.referenceFilePath || '');
      const name = path
        ? path.replace(/\\/g, '/').split('/').pop() || path
        : '…';
      return t('batch.action_colorMatch', { name });
    }
    default:
      return t('batch.action_unknown');
  }
}

export function normalizeBatchTemplates(raw: unknown): BatchActionTemplate[] {
  if (!Array.isArray(raw)) return [];
  const out: BatchActionTemplate[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const t = item as BatchActionTemplate;
    const id = String(t.id || '').trim();
    const name = String(t.name || '').trim();
    if (!id || !name || !Array.isArray(t.actions) || t.actions.length === 0) continue;
    out.push({
      id,
      name,
      updatedAt: Number(t.updatedAt) || Date.now(),
      actions: t.actions.map((a) => ({ ...a, id: a.id || createBatchActionId() })) as BatchAction[],
    });
  }
  return out
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, BATCH_TEMPLATE_LIMIT);
}

/** Serialize actions for host (strip UI-only ids ok to keep). */
export function actionsForHost(actions: BatchAction[]) {
  return actions.map((a) => {
    const { id: _id, ...rest } = a as any;
    if (a.type === 'colorMatch') {
      return {
        type: 'colorMatch',
        referenceFilePath: a.referenceFilePath,
        intensity: (a.intensity ?? 100) / 100,
        tonePreservation: (a.tonePreservation ?? 50) / 100,
        autoWb: a.autoWb !== false,
        highlightProtection: (a.highlightProtection ?? 80) / 100,
        shadowProtection: (a.shadowProtection ?? 80) / 100,
      };
    }
    if (a.type === 'photoStyle') {
      return {
        type: 'photoStyle',
        styleBrightness: a.styleBrightness ?? 0,
        styleContrast: a.styleContrast ?? 0,
        styleSaturation: a.styleSaturation ?? 100,
        styleHue: a.styleHue ?? 0,
        highlights: a.highlights ?? 0,
        shadows: a.shadows ?? 0,
        fade: a.fade ?? 0,
        vignette: a.vignette ?? 0,
        grain: a.grain ?? 0,
        filter: a.filter || null,
        lutId: a.lutId || null,
        lutIntensity: a.lutIntensity ?? 100,
      };
    }
    return rest;
  });
}

export function buildOutputFileName(
  sourceName: string,
  index: number,
  nameMode: BatchNameMode,
  prefix: string,
  suffix: string,
  ext: string,
): string {
  const base = sourceName.includes('.')
    ? sourceName.slice(0, sourceName.lastIndexOf('.'))
    : sourceName;
  let stem = base;
  if (nameMode === 'prefix') stem = `${prefix || 'out'}_${base}`;
  else if (nameMode === 'suffix') stem = `${base}_${suffix || 'edit'}`;
  else if (nameMode === 'sequence') stem = `${prefix || 'img'}_${String(index + 1).padStart(3, '0')}`;
  return `${stem}.${ext}`;
}
