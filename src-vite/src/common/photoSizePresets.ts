/**
 * Built-in crop aspect ratios and print/ID photo size presets (Phase A).
 * Pixel sizes are target export dimensions @ declared DPI; crop UI primarily enforces aspect.
 */

export type CropPresetKind = 'free' | 'ratio' | 'photo' | 'custom';

export interface CustomCropRatio {
  id: string;
  name: string;
  ratioW: number;
  ratioH: number;
}

export interface RatioCropPreset {
  id: string;
  kind: 'ratio';
  ratioW: number;
  ratioH: number;
  /** Stable label like "3:2" (landscape-primary for non-square). */
  label: string;
}

export interface PhotoSizeCropPreset {
  id: string;
  kind: 'photo';
  /** i18n key under msgbox.image_editor.photo_sizes */
  nameKey: string;
  inchW: number;
  inchH: number;
  cmW: number;
  cmH: number;
  pxW: number;
  pxH: number;
  dpi: number;
}

export type ResolvedCropPreset =
  | { id: 'free'; kind: 'free' }
  | RatioCropPreset
  | PhotoSizeCropPreset
  | { id: string; kind: 'custom'; name: string; ratioW: number; ratioH: number };

export const FREE_CROP_PRESET_ID = 'free';
export const MANAGE_PHOTO_SIZES_ID = '__manage_photo_sizes__';
export const ADD_CUSTOM_RATIO_ID = '__add_custom_ratio__';

/** Common aspect ratios (roadmap Phase A). */
export const BUILTIN_RATIO_PRESETS: readonly RatioCropPreset[] = [
  { id: 'ratio-1-1', kind: 'ratio', ratioW: 1, ratioH: 1, label: '1:1' },
  { id: 'ratio-3-2', kind: 'ratio', ratioW: 3, ratioH: 2, label: '3:2' },
  { id: 'ratio-4-3', kind: 'ratio', ratioW: 4, ratioH: 3, label: '4:3' },
  { id: 'ratio-16-9', kind: 'ratio', ratioW: 16, ratioH: 9, label: '16:9' },
] as const;

/**
 * Print / ID photo catalog (roadmap table).
 * Dimensions are product defaults: inch × cm × px @ DPI as listed.
 */
export const BUILTIN_PHOTO_SIZE_PRESETS: readonly PhotoSizeCropPreset[] = [
  { id: 'photo-1r', kind: 'photo', nameKey: 'size_1r', inchW: 0.98, inchH: 1.38, pxW: 295, pxH: 413, cmW: 2.5, cmH: 3.5, dpi: 300 },
  { id: 'photo-2r', kind: 'photo', nameKey: 'size_2r', inchW: 1.38, inchH: 1.93, pxW: 413, pxH: 579, cmW: 3.5, cmH: 4.9, dpi: 300 },
  { id: 'photo-2r-large', kind: 'photo', nameKey: 'size_2r_large', inchW: 1.38, inchH: 2.09, pxW: 413, pxH: 626, cmW: 3.5, cmH: 5.3, dpi: 300 },
  { id: 'photo-cn-id', kind: 'photo', nameKey: 'size_cn_id', inchW: 1.02, inchH: 1.26, pxW: 358, pxH: 441, cmW: 2.6, cmH: 3.2, dpi: 350 },
  { id: 'photo-passport', kind: 'photo', nameKey: 'size_passport', inchW: 1.3, inchH: 1.89, pxW: 390, pxH: 567, cmW: 3.3, cmH: 4.8, dpi: 300 },
  { id: 'photo-3r', kind: 'photo', nameKey: 'size_3r', inchW: 5.0, inchH: 3.5, pxW: 1500, pxH: 1050, cmW: 12.7, cmH: 8.9, dpi: 300 },
  { id: 'photo-4r', kind: 'photo', nameKey: 'size_4r', inchW: 6.0, inchH: 4.0, pxW: 1800, pxH: 1200, cmW: 15.2, cmH: 10.2, dpi: 300 },
  { id: 'photo-5r', kind: 'photo', nameKey: 'size_5r', inchW: 7.0, inchH: 5.0, pxW: 2100, pxH: 1500, cmW: 17.8, cmH: 12.7, dpi: 300 },
  { id: 'photo-6r', kind: 'photo', nameKey: 'size_6r', inchW: 8.0, inchH: 6.0, pxW: 2400, pxH: 1800, cmW: 20.3, cmH: 15.2, dpi: 300 },
  { id: 'photo-8r', kind: 'photo', nameKey: 'size_8r', inchW: 10.0, inchH: 8.0, pxW: 3000, pxH: 2400, cmW: 25.4, cmH: 20.3, dpi: 300 },
  { id: 'photo-wallet-small', kind: 'photo', nameKey: 'size_wallet_small', inchW: 2.49, inchH: 3.5, pxW: 748, pxH: 1050, cmW: 6.3, cmH: 8.9, dpi: 300 },
  { id: 'photo-wallet-large', kind: 'photo', nameKey: 'size_wallet_large', inchW: 2.99, inchH: 4.0, pxW: 898, pxH: 1200, cmW: 7.6, cmH: 10.2, dpi: 300 },
] as const;

const RATIO_BY_ID = new Map(BUILTIN_RATIO_PRESETS.map((p) => [p.id, p]));
const PHOTO_BY_ID = new Map(BUILTIN_PHOTO_SIZE_PRESETS.map((p) => [p.id, p]));

/** Legacy cropShape number → new preset id (best-effort). */
const LEGACY_CROP_SHAPE_TO_PRESET: Record<string, string> = {
  '0': FREE_CROP_PRESET_ID,
  '1': 'ratio-1-1',
  '2': 'ratio-4-3',
  '3': 'ratio-3-2',
  '4': FREE_CROP_PRESET_ID, // was 16:10 / 10:16
  '5': 'ratio-16-9',
  '6': FREE_CROP_PRESET_ID, // was 2:1 / 1:2
};

export function formatRatioLabel(ratioW: number, ratioH: number, isPortrait = false): string {
  const w = isPortrait ? ratioH : ratioW;
  const h = isPortrait ? ratioW : ratioH;
  return `${stripTrailingZeros(w)}:${stripTrailingZeros(h)}`;
}

function stripTrailingZeros(n: number): string {
  if (Number.isInteger(n)) return String(n);
  return String(Number(n.toFixed(4)));
}

/**
 * Crop box width/height aspect (display width / display height).
 * isPortrait swaps the stored pair (same behavior as the old label flip).
 */
export function getCropAspectRatio(
  ratioW: number,
  ratioH: number,
  isPortrait: boolean,
): number {
  if (!(ratioW > 0) || !(ratioH > 0)) return 1;
  return isPortrait ? ratioH / ratioW : ratioW / ratioH;
}

export function getPresetBaseRatio(preset: ResolvedCropPreset): { ratioW: number; ratioH: number } | null {
  if (preset.kind === 'free') return null;
  if (preset.kind === 'photo') {
    return { ratioW: preset.pxW, ratioH: preset.pxH };
  }
  return { ratioW: preset.ratioW, ratioH: preset.ratioH };
}

export function getPhotoTargetPixels(
  preset: PhotoSizeCropPreset,
  isPortrait: boolean,
): { width: number; height: number } {
  if (isPortrait) {
    // Prefer taller output when portrait is requested.
    if (preset.pxH >= preset.pxW) {
      return { width: preset.pxW, height: preset.pxH };
    }
    return { width: preset.pxH, height: preset.pxW };
  }
  if (preset.pxW >= preset.pxH) {
    return { width: preset.pxW, height: preset.pxH };
  }
  return { width: preset.pxH, height: preset.pxW };
}

export function resolveCropPreset(
  presetId: string | null | undefined,
  customRatios: CustomCropRatio[] = [],
): ResolvedCropPreset {
  const id = String(presetId || FREE_CROP_PRESET_ID);
  if (id === FREE_CROP_PRESET_ID) {
    return { id: FREE_CROP_PRESET_ID, kind: 'free' };
  }

  const ratio = RATIO_BY_ID.get(id);
  if (ratio) return ratio;

  const photo = PHOTO_BY_ID.get(id);
  if (photo) return photo;

  const custom = customRatios.find((item) => item.id === id);
  if (custom) {
    return {
      id: custom.id,
      kind: 'custom',
      name: custom.name,
      ratioW: custom.ratioW,
      ratioH: custom.ratioH,
    };
  }

  return { id: FREE_CROP_PRESET_ID, kind: 'free' };
}

export function migrateLegacyCropShape(cropShape: unknown): string {
  const key = String(cropShape ?? '0');
  return LEGACY_CROP_SHAPE_TO_PRESET[key] || FREE_CROP_PRESET_ID;
}

export function parseRatioParts(input: string): { ratioW: number; ratioH: number } | null {
  const text = String(input || '').trim().replace(/：/g, ':').replace(/x/gi, ':').replace(/×/g, ':');
  if (!text) return null;

  const parts = text.split(':').map((p) => p.trim()).filter(Boolean);
  if (parts.length !== 2) return null;

  const ratioW = Number(parts[0]);
  const ratioH = Number(parts[1]);
  if (!Number.isFinite(ratioW) || !Number.isFinite(ratioH) || ratioW <= 0 || ratioH <= 0) {
    return null;
  }

  return { ratioW, ratioH };
}

export function createCustomRatioId(): string {
  return `custom-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function normalizeCustomCropRatios(raw: unknown): CustomCropRatio[] {
  if (!Array.isArray(raw)) return [];

  const result: CustomCropRatio[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const id = String((item as CustomCropRatio).id || '').trim();
    const ratioW = Number((item as CustomCropRatio).ratioW);
    const ratioH = Number((item as CustomCropRatio).ratioH);
    if (!id || !(ratioW > 0) || !(ratioH > 0)) continue;
    const name = String((item as CustomCropRatio).name || '').trim() || formatRatioLabel(ratioW, ratioH);
    result.push({ id, name, ratioW, ratioH });
  }
  return result;
}
