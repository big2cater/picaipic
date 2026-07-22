/**
 * Photo frame / EXIF info-bar presets (G-Frame-1 + G2 blur float/sink + logo).
 * Built-in layout templates + user-saved custom presets (config.photoFrame.presets).
 */

export type PhotoFrameTemplateId =
  | 'classic-white'
  | 'classic-black'
  | 'float-blur'
  | 'sink-blur';

export type PhotoFrameLogoPosition = 'bar-center' | 'top-left' | 'top-right';

export interface PhotoFrameOptions {
  templateId: PhotoFrameTemplateId;
  showBrand: boolean;
  showModel: boolean;
  showLens: boolean;
  showFocalLength: boolean;
  showAperture: boolean;
  showShutter: boolean;
  showISO: boolean;
  showDateTime: boolean;
  /** Bar height as fraction of short edge (0.05–0.22). */
  barRatio: number;
  /** Outer margin / pad as fraction of short edge. */
  marginRatio: number;
  backgroundColor: string;
  textColor: string;
  secondaryTextColor: string;
  /** Background blur strength for float/sink templates. */
  blurSigma: number;
  /** Soft drop-shadow blur under the photo. */
  shadowBlur: number;
  /** Shadow vertical offset as fraction of photo height. */
  shadowOffsetRatio: number;
  /** Shadow opacity 0–1. */
  shadowOpacity: number;
  showLogo: boolean;
  /** Empty = host uses bundled default branding logo. */
  logoPath: string;
  /** Logo long edge as fraction of photo short edge. */
  logoScale: number;
  logoPosition: PhotoFrameLogoPosition;
}

/** User-saved full options snapshot for reuse / batch-like workflows. */
export interface PhotoFramePreset {
  id: string;
  name: string;
  updatedAt: number;
  options: PhotoFrameOptions;
}

export interface PhotoFrameTemplateMeta {
  id: PhotoFrameTemplateId;
  /** i18n key under photo_frame.template_* */
  nameKey: string;
  /** Whether style sliders for blur/shadow are meaningful. */
  isBlurLayout: boolean;
  defaults: PhotoFrameOptions;
}

const fieldDefaults = {
  showBrand: true,
  showModel: true,
  showLens: true,
  showFocalLength: true,
  showAperture: true,
  showShutter: true,
  showISO: true,
  showDateTime: true,
  // Default on; empty path → host bundled logo-pic branding asset.
  showLogo: true,
  logoPath: '',
  logoScale: 0.1,
  logoPosition: 'bar-center' as PhotoFrameLogoPosition,
};

export const PHOTO_FRAME_TEMPLATES: readonly PhotoFrameTemplateMeta[] = [
  {
    id: 'classic-white',
    nameKey: 'photo_frame.template_classic_white',
    isBlurLayout: false,
    defaults: {
      ...fieldDefaults,
      templateId: 'classic-white',
      barRatio: 0.11,
      marginRatio: 0,
      backgroundColor: '#FFFFFF',
      textColor: '#242424',
      secondaryTextColor: '#666666',
      blurSigma: 18,
      shadowBlur: 16,
      shadowOffsetRatio: 0.03,
      shadowOpacity: 0.4,
    },
  },
  {
    id: 'classic-black',
    nameKey: 'photo_frame.template_classic_black',
    isBlurLayout: false,
    defaults: {
      ...fieldDefaults,
      templateId: 'classic-black',
      barRatio: 0.11,
      marginRatio: 0,
      backgroundColor: '#121212',
      textColor: '#F5F5F5',
      secondaryTextColor: '#B4B4B4',
      blurSigma: 18,
      shadowBlur: 16,
      shadowOffsetRatio: 0.03,
      shadowOpacity: 0.4,
    },
  },
  {
    id: 'float-blur',
    nameKey: 'photo_frame.template_float_blur',
    isBlurLayout: true,
    defaults: {
      ...fieldDefaults,
      templateId: 'float-blur',
      barRatio: 0.12,
      marginRatio: 0.1,
      backgroundColor: '#141414',
      textColor: '#FAFAFA',
      secondaryTextColor: '#D2D2D2',
      blurSigma: 20,
      shadowBlur: 18,
      shadowOffsetRatio: 0.035,
      shadowOpacity: 0.45,
      logoPosition: 'bar-center',
    },
  },
  {
    id: 'sink-blur',
    nameKey: 'photo_frame.template_sink_blur',
    isBlurLayout: true,
    defaults: {
      ...fieldDefaults,
      templateId: 'sink-blur',
      barRatio: 0.11,
      marginRatio: 0.08,
      backgroundColor: '#101010',
      textColor: '#FAFAFA',
      secondaryTextColor: '#D0D0D0',
      blurSigma: 22,
      shadowBlur: 22,
      shadowOffsetRatio: 0.06,
      shadowOpacity: 0.5,
      logoPosition: 'bar-center',
    },
  },
] as const;

const TEMPLATE_IDS = new Set(PHOTO_FRAME_TEMPLATES.map((t) => t.id));

export function createDefaultPhotoFrameOptions(
  templateId: PhotoFrameTemplateId = 'classic-white',
): PhotoFrameOptions {
  const tpl =
    PHOTO_FRAME_TEMPLATES.find((t) => t.id === templateId) || PHOTO_FRAME_TEMPLATES[0];
  return { ...tpl.defaults };
}

export function applyTemplatePreset(
  current: PhotoFrameOptions,
  templateId: PhotoFrameTemplateId,
): PhotoFrameOptions {
  const base = createDefaultPhotoFrameOptions(templateId);
  // Keep field toggles + logo choice; rest follows template defaults.
  return {
    ...base,
    showBrand: current.showBrand,
    showModel: current.showModel,
    showLens: current.showLens,
    showFocalLength: current.showFocalLength,
    showAperture: current.showAperture,
    showShutter: current.showShutter,
    showISO: current.showISO,
    showDateTime: current.showDateTime,
    showLogo: current.showLogo,
    logoPath: current.logoPath,
    logoScale: current.logoScale,
    logoPosition: current.logoPosition,
  };
}

export function isBlurFrameTemplate(id: PhotoFrameTemplateId | string): boolean {
  // Exact layout ids only (matches host frame_layout_kind).
  return id === 'float-blur' || id === 'sink-blur';
}

export function isPhotoFrameImageFile(file: { file_type?: number | null } | null | undefined): boolean {
  const t = Number(file?.file_type || 0);
  return t === 1 || t === 3;
}

export function filterPhotoFrameImageFiles<T extends { file_type?: number | null }>(files: T[]): T[] {
  return (files || []).filter((f) => isPhotoFrameImageFile(f));
}

export function createPhotoFramePresetId(): string {
  return `pf_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

function clampNum(n: unknown, min: number, max: number, fallback: number): number {
  const v = Number(n);
  if (!Number.isFinite(v)) return fallback;
  return Math.min(max, Math.max(min, v));
}

function asBool(v: unknown, fallback: boolean): boolean {
  if (typeof v === 'boolean') return v;
  return fallback;
}

function asColor(v: unknown, fallback: string): string {
  const s = String(v || '').trim();
  if (/^#[0-9a-fA-F]{6}$/.test(s)) return s;
  return fallback;
}

function asLogoPos(v: unknown): PhotoFrameLogoPosition {
  const s = String(v || '');
  // Legacy bar-left / bar-right collapse to centered bar logo.
  if (s === 'top-left' || s === 'top-right' || s === 'bar-center') return s;
  if (s === 'bar-left' || s === 'bar-right') return 'bar-center';
  return 'bar-center';
}

function asTemplateId(v: unknown): PhotoFrameTemplateId {
  const s = String(v || '');
  if (TEMPLATE_IDS.has(s as PhotoFrameTemplateId)) return s as PhotoFrameTemplateId;
  return 'classic-white';
}

/** Normalize a partial options object against built-in defaults. */
export function normalizePhotoFrameOptions(raw: unknown): PhotoFrameOptions {
  const base = createDefaultPhotoFrameOptions('classic-white');
  const o = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const templateId = asTemplateId(o.templateId);
  const tplDefaults = createDefaultPhotoFrameOptions(templateId);
  return {
    templateId,
    showBrand: asBool(o.showBrand, tplDefaults.showBrand),
    showModel: asBool(o.showModel, tplDefaults.showModel),
    showLens: asBool(o.showLens, tplDefaults.showLens),
    showFocalLength: asBool(o.showFocalLength, tplDefaults.showFocalLength),
    showAperture: asBool(o.showAperture, tplDefaults.showAperture),
    showShutter: asBool(o.showShutter, tplDefaults.showShutter),
    showISO: asBool(o.showISO, tplDefaults.showISO),
    showDateTime: asBool(o.showDateTime, tplDefaults.showDateTime),
    barRatio: clampNum(o.barRatio, 0.05, 0.22, tplDefaults.barRatio),
    marginRatio: clampNum(o.marginRatio, 0, 0.20, tplDefaults.marginRatio),
    backgroundColor: asColor(o.backgroundColor, tplDefaults.backgroundColor),
    textColor: asColor(o.textColor, tplDefaults.textColor),
    secondaryTextColor: asColor(o.secondaryTextColor, tplDefaults.secondaryTextColor),
    blurSigma: clampNum(o.blurSigma, 2, 48, tplDefaults.blurSigma),
    shadowBlur: clampNum(o.shadowBlur, 2, 40, tplDefaults.shadowBlur),
    shadowOffsetRatio: clampNum(o.shadowOffsetRatio, 0, 0.12, tplDefaults.shadowOffsetRatio),
    shadowOpacity: clampNum(o.shadowOpacity, 0.05, 0.9, tplDefaults.shadowOpacity),
    showLogo: asBool(o.showLogo, tplDefaults.showLogo),
    logoPath: typeof o.logoPath === 'string' ? o.logoPath : '',
    logoScale: clampNum(o.logoScale, 0.04, 0.22, tplDefaults.logoScale),
    logoPosition: asLogoPos(o.logoPosition),
  };
}

export function normalizePhotoFramePresets(raw: unknown): PhotoFramePreset[] {
  if (!Array.isArray(raw)) return [];
  const out: PhotoFramePreset[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const r = item as Record<string, unknown>;
    const id = String(r.id || '').trim() || createPhotoFramePresetId();
    const name = String(r.name || '').trim() || 'Preset';
    const updatedAt = Number(r.updatedAt) || Date.now();
    out.push({
      id,
      name,
      updatedAt,
      options: normalizePhotoFrameOptions(r.options),
    });
  }
  // Newest first
  out.sort((a, b) => b.updatedAt - a.updatedAt);
  return out;
}

export function clonePhotoFrameOptions(options: PhotoFrameOptions): PhotoFrameOptions {
  return JSON.parse(JSON.stringify(options)) as PhotoFrameOptions;
}

export interface PhotoFrameFileItem {
  id?: number | string;
  file_path: string;
  name?: string;
  file_type?: number | null;
  thumbnail?: string;
  e_orientation?: number | null;
  orientation?: number | null;
}
