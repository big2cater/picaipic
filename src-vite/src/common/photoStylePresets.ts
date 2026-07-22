/**
 * Unified adjust recipes for ImageEditor presets.
 * A recipe = base CSS-like params + optional host effects (highlights/shadows/fade/vignette/grain) + optional LUT.
 * Host apply order (t_lut): base → LUT → effects.
 */

export type PhotoStyleFilter = '' | 'grayscale' | 'sepia' | 'invert';

/** Full recipe used by both CSS-fast and host photo-style paths. */
export interface AdjustRecipe {
  id: string;
  name: string;
  builtIn: boolean;
  brightness: number; // -100..100
  contrast: number; // -100..100
  saturation: number; // 0..200, 100 neutral
  hue: number; // -180..180
  blur: number; // 0..20 (CSS-only; not part of host style)
  filter: PhotoStyleFilter;
  highlights: number; // -100..100 (host)
  shadows: number; // -100..100 (host)
  fade: number; // 0..100 (host)
  vignette: number; // 0..100 (host)
  grain: number; // 0..100 (host)
  lutId: string; // library id or ''
  lutIntensity: number; // 0..100
  updatedAt?: number;
}

/** @deprecated alias — same shape as AdjustRecipe */
export type PhotoStylePreset = AdjustRecipe;

export const PHOTO_STYLE_LIMIT = 40;

export function createPhotoStyleId(prefix = 'style'): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function defaultPhotoStyle(partial: Partial<AdjustRecipe> = {}): AdjustRecipe {
  return {
    id: partial.id || createPhotoStyleId(),
    name: partial.name || 'Custom',
    builtIn: partial.builtIn ?? false,
    brightness: partial.brightness ?? 0,
    contrast: partial.contrast ?? 0,
    saturation: partial.saturation ?? 100,
    hue: partial.hue ?? 0,
    blur: partial.blur ?? 0,
    filter: partial.filter ?? '',
    highlights: partial.highlights ?? 0,
    shadows: partial.shadows ?? 0,
    fade: partial.fade ?? 0,
    vignette: partial.vignette ?? 0,
    grain: partial.grain ?? 0,
    lutId: partial.lutId ?? '',
    lutIntensity: partial.lutIntensity ?? 100,
    updatedAt: partial.updatedAt ?? Date.now(),
  };
}

/** True when only CSS-capable fields (plus blur) are in play — no host round-trip needed. */
export function needsHostPreview(style: Pick<
  AdjustRecipe,
  'highlights' | 'shadows' | 'fade' | 'vignette' | 'grain' | 'lutId' | 'lutIntensity'
>): boolean {
  const lutActive = !!String(style.lutId || '').trim() && Number(style.lutIntensity ?? 100) > 0;
  return (
    Number(style.highlights || 0) !== 0
    || Number(style.shadows || 0) !== 0
    || Number(style.fade || 0) !== 0
    || Number(style.vignette || 0) !== 0
    || Number(style.grain || 0) !== 0
    || lutActive
  );
}

/** Host photo-style identity (blur is separate). */
export function isPhotoStyleIdentity(style: Pick<
  AdjustRecipe,
  | 'brightness'
  | 'contrast'
  | 'saturation'
  | 'hue'
  | 'filter'
  | 'highlights'
  | 'shadows'
  | 'fade'
  | 'vignette'
  | 'grain'
  | 'lutId'
>): boolean {
  return (
    Number(style.brightness || 0) === 0
    && Number(style.contrast || 0) === 0
    && Number(style.saturation ?? 100) === 100
    && Number(style.hue || 0) === 0
    && !style.filter
    && Number(style.highlights || 0) === 0
    && Number(style.shadows || 0) === 0
    && Number(style.fade || 0) === 0
    && Number(style.vignette || 0) === 0
    && Number(style.grain || 0) === 0
    && !String(style.lutId || '').trim()
  );
}

export function sameRecipeValues(
  a: Pick<AdjustRecipe, 'brightness' | 'contrast' | 'saturation' | 'hue' | 'blur' | 'filter' | 'highlights' | 'shadows' | 'fade' | 'vignette' | 'grain' | 'lutId' | 'lutIntensity'>,
  b: typeof a,
): boolean {
  return (
    a.brightness === b.brightness
    && a.contrast === b.contrast
    && a.saturation === b.saturation
    && a.hue === b.hue
    && a.blur === b.blur
    && (a.filter || '') === (b.filter || '')
    && Number(a.highlights || 0) === Number(b.highlights || 0)
    && Number(a.shadows || 0) === Number(b.shadows || 0)
    && Number(a.fade || 0) === Number(b.fade || 0)
    && Number(a.vignette || 0) === Number(b.vignette || 0)
    && Number(a.grain || 0) === Number(b.grain || 0)
    && String(a.lutId || '') === String(b.lutId || '')
    && Number(a.lutIntensity ?? 100) === Number(b.lutIntensity ?? 100)
  );
}

/**
 * Built-in look recipes shown in the Presets strip.
 * Legacy CSS looks kept; Panasonic-like styles merged (no second panel).
 * Duplicate names resolved: richer style values win for vivid/cinematic; mono → bw.
 */
export const BUILTIN_ADJUST_RECIPES: readonly AdjustRecipe[] = [
  defaultPhotoStyle({ id: 'natural', name: 'Original', builtIn: true }),
  defaultPhotoStyle({
    id: 'vivid',
    name: 'Vivid',
    builtIn: true,
    contrast: 12,
    saturation: 125,
    highlights: 5,
  }),
  defaultPhotoStyle({
    id: 'muted',
    name: 'Muted',
    builtIn: true,
    contrast: -10,
    saturation: 80,
  }),
  defaultPhotoStyle({
    id: 'warm',
    name: 'Warm',
    builtIn: true,
    brightness: 5,
    hue: 5,
  }),
  defaultPhotoStyle({
    id: 'cool',
    name: 'Cool',
    builtIn: true,
    brightness: 5,
    hue: -5,
  }),
  defaultPhotoStyle({
    id: 'bw',
    name: 'Black & White',
    builtIn: true,
    saturation: 0,
    contrast: 8,
    filter: 'grayscale',
  }),
  defaultPhotoStyle({
    id: 'vintage',
    name: 'Vintage',
    builtIn: true,
    brightness: 10,
    contrast: -10,
    saturation: 60,
    filter: 'sepia',
  }),
  defaultPhotoStyle({
    id: 'kodak',
    name: 'Kodak',
    builtIn: true,
    brightness: 10,
    contrast: 15,
    saturation: 120,
    hue: -5,
  }),
  defaultPhotoStyle({
    id: 'toyo',
    name: 'Toyo',
    builtIn: true,
    brightness: 5,
    saturation: 110,
    hue: 5,
  }),
  defaultPhotoStyle({
    id: 'cinematic',
    name: 'Cinematic',
    builtIn: true,
    contrast: 16,
    saturation: 85,
    shadows: 10,
    highlights: -8,
    vignette: 22,
    fade: 10,
  }),
  defaultPhotoStyle({
    id: 'dramatic',
    name: 'Dramatic',
    builtIn: true,
    contrast: 30,
    saturation: 110,
  }),
  defaultPhotoStyle({
    id: 'cyberpunk',
    name: 'Cyberpunk',
    builtIn: true,
    brightness: 10,
    contrast: 20,
    saturation: 130,
    hue: -15,
  }),
  defaultPhotoStyle({
    id: 'invert',
    name: 'Invert',
    builtIn: true,
    filter: 'invert',
  }),
  defaultPhotoStyle({
    id: 'portrait',
    name: 'Portrait',
    builtIn: true,
    contrast: -4,
    saturation: 95,
    shadows: 8,
    highlights: -6,
  }),
  defaultPhotoStyle({
    id: 'landscape',
    name: 'Landscape',
    builtIn: true,
    contrast: 10,
    saturation: 115,
    highlights: -4,
    shadows: 4,
  }),
  defaultPhotoStyle({
    id: 'nostalgic',
    name: 'Nostalgic',
    builtIn: true,
    contrast: -8,
    saturation: 80,
    fade: 28,
    grain: 18,
    hue: 6,
  }),
] as const;

/** @deprecated use BUILTIN_ADJUST_RECIPES — kept for batch / older imports */
export const BUILTIN_PHOTO_STYLES: readonly AdjustRecipe[] = BUILTIN_ADJUST_RECIPES;

const BUILTIN_BY_ID = new Map(BUILTIN_ADJUST_RECIPES.map((r) => [r.id, r]));

export function getBuiltinRecipe(id: string): AdjustRecipe | undefined {
  return BUILTIN_BY_ID.get(id);
}

export function normalizePhotoStyles(raw: unknown): AdjustRecipe[] {
  if (!Array.isArray(raw)) return [];
  const out: AdjustRecipe[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const o = item as Partial<AdjustRecipe>;
    const id = String(o.id || '').trim();
    const name = String(o.name || '').trim();
    if (!id || !name || o.builtIn) continue; // only user customs in config array
    // Skip ids that collide with builtins
    if (BUILTIN_BY_ID.has(id) || id === 'auto' || id === 'custom') continue;
    out.push(
      defaultPhotoStyle({
        ...o,
        id,
        name,
        builtIn: false,
      }),
    );
  }
  // Preserve config array order so presets strip does not jump when switching/editing.
  return out.slice(0, PHOTO_STYLE_LIMIT);
}

export function allPhotoStyles(custom: AdjustRecipe[]): AdjustRecipe[] {
  return [...BUILTIN_ADJUST_RECIPES, ...normalizePhotoStyles(custom)];
}

export function findPhotoStyle(id: string, custom: AdjustRecipe[]): AdjustRecipe | undefined {
  if (!id) return undefined;
  return allPhotoStyles(custom).find((s) => s.id === id);
}

export function styleForHost(style: AdjustRecipe) {
  return {
    brightness: style.brightness,
    contrast: style.contrast,
    saturation: style.saturation,
    hue: style.hue,
    highlights: style.highlights,
    shadows: style.shadows,
    fade: style.fade,
    vignette: style.vignette,
    grain: style.grain,
    filter: style.filter || null,
    lutId: style.lutId || null,
    lutIntensity: style.lutIntensity,
  };
}

export function cloneAsCustom(style: AdjustRecipe, name?: string): AdjustRecipe {
  return defaultPhotoStyle({
    ...style,
    id: createPhotoStyleId('custom'),
    name: name || (style.name + ' copy'),
    builtIn: false,
    updatedAt: Date.now(),
  });
}
