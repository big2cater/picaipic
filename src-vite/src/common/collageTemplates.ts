/**
 * Collage templates, strip layouts, free-canvas helpers, and magazine-style
 * pattern cells inspired by 光影魔术手 PatternJigsaw templates.
 */

export type CollageMode = 'template' | 'strip' | 'free';
export type CollageFillMode = 'cover' | 'contain';

/** Built-in layout ids (equal grids + magazine variants + strips + free). */
export type CollageTemplateId =
  | '2'
  | '2v'
  | '3a'
  | '3b'
  | '4'
  | '4m'
  | '6'
  | '6m'
  | '9'
  | 'strip-h'
  | 'strip-v'
  | 'free';

/** Normalized cell rect (0–1 of canvas). */
export interface CollageCellRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CollageTemplate {
  id: CollageTemplateId;
  mode: CollageMode;
  /** Equal-grid dims (template mode without freeform cells). */
  cols: number;
  rows: number;
  defaultOutputSize: number;
  strip?: 'h' | 'v';
  /**
   * Magazine / freeform cells in normalized coords (includes outer margin).
   * When set, preview/export use absolute cells instead of equal CSS grid.
   */
  cells?: readonly CollageCellRect[];
  /** Canvas width/height ratio when cells are freeform. Default 1. */
  aspect?: number;
}

/** Free-canvas item; geometry is normalized 0–1 relative to output canvas. */
export interface FreeCollageItem {
  id: string;
  filePath: string;
  thumb: string;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Degrees, clockwise in CSS/export. */
  rotate: number;
  z: number;
}

/**
 * Equal grids + NeoImaging-style magazine variants (normalized from their XML).
 * Outer margin ~1.2–1.7%; gaps baked into cell geometry.
 */
export const COLLAGE_TEMPLATES: readonly CollageTemplate[] = [
  // 2 side-by-side (光影 2/1)
  {
    id: '2',
    mode: 'template',
    cols: 2,
    rows: 1,
    defaultOutputSize: 2400,
    aspect: 800 / 600,
    cells: [
      { x: 0.0125, y: 0.0167, w: 0.4813, h: 0.9667 },
      { x: 0.5062, y: 0.0167, w: 0.4813, h: 0.9667 },
    ],
  },
  // 2 stacked (光影 2/2)
  {
    id: '2v',
    mode: 'template',
    cols: 1,
    rows: 2,
    defaultOutputSize: 2400,
    aspect: 600 / 800,
    cells: [
      { x: 0.0167, y: 0.0125, w: 0.9667, h: 0.4813 },
      { x: 0.0167, y: 0.5062, w: 0.9667, h: 0.4813 },
    ],
  },
  // 3: tall left + 2 right (光影 3/1)
  {
    id: '3a',
    mode: 'template',
    cols: 2,
    rows: 2,
    defaultOutputSize: 2400,
    aspect: 1,
    cells: [
      { x: 0.0167, y: 0.0167, w: 0.475, h: 0.9667 },
      { x: 0.5083, y: 0.0167, w: 0.475, h: 0.475 },
      { x: 0.5083, y: 0.5083, w: 0.475, h: 0.475 },
    ],
  },
  // 3: 2 left + tall right (光影 3/2)
  {
    id: '3b',
    mode: 'template',
    cols: 2,
    rows: 2,
    defaultOutputSize: 2400,
    aspect: 1,
    cells: [
      { x: 0.0167, y: 0.0167, w: 0.475, h: 0.475 },
      { x: 0.0167, y: 0.5083, w: 0.475, h: 0.475 },
      { x: 0.5083, y: 0.0167, w: 0.475, h: 0.9667 },
    ],
  },
  // 4 equal (光影 4/2)
  {
    id: '4',
    mode: 'template',
    cols: 2,
    rows: 2,
    defaultOutputSize: 2400,
    aspect: 1,
    cells: [
      { x: 0.0125, y: 0.0125, w: 0.4813, h: 0.4813 },
      { x: 0.5062, y: 0.0125, w: 0.4813, h: 0.4813 },
      { x: 0.0125, y: 0.5062, w: 0.4813, h: 0.4813 },
      { x: 0.5062, y: 0.5062, w: 0.4813, h: 0.4813 },
    ],
  },
  // 4 magazine: tall left + 3 right stack (光影 4/1)
  {
    id: '4m',
    mode: 'template',
    cols: 2,
    rows: 3,
    defaultOutputSize: 2400,
    aspect: 600 / 640,
    cells: [
      { x: 0.0167, y: 0.0156, w: 0.475, h: 0.9688 },
      { x: 0.5083, y: 0.0156, w: 0.475, h: 0.3125 },
      { x: 0.5083, y: 0.3438, w: 0.475, h: 0.3125 },
      { x: 0.5083, y: 0.6719, w: 0.475, h: 0.3125 },
    ],
  },
  // 6 equal-ish 3×2
  {
    id: '6',
    mode: 'template',
    cols: 3,
    rows: 2,
    defaultOutputSize: 2400,
    aspect: 1,
    cells: equalGridCells(3, 2, 0.0125),
  },
  // 6 magazine (光影 6/1)
  {
    id: '6m',
    mode: 'template',
    cols: 3,
    rows: 3,
    defaultOutputSize: 2400,
    aspect: 1,
    cells: [
      { x: 0.0127, y: 0.0127, w: 0.9747, h: 0.3165 },
      { x: 0.0127, y: 0.3418, w: 0.481, h: 0.3165 },
      { x: 0.5063, y: 0.3418, w: 0.481, h: 0.3165 },
      { x: 0.0127, y: 0.6709, w: 0.3165, h: 0.3165 },
      { x: 0.3418, y: 0.6709, w: 0.3165, h: 0.3165 },
      { x: 0.6709, y: 0.6709, w: 0.3165, h: 0.3165 },
    ],
  },
  // 9 equal (光影 9/1)
  {
    id: '9',
    mode: 'template',
    cols: 3,
    rows: 3,
    defaultOutputSize: 2400,
    aspect: 1,
    cells: equalGridCells(3, 3, 0.0127),
  },
] as const;

export const COLLAGE_STRIP_TEMPLATES: readonly CollageTemplate[] = [
  { id: 'strip-h', mode: 'strip', cols: 0, rows: 1, defaultOutputSize: 2400, strip: 'h' },
  { id: 'strip-v', mode: 'strip', cols: 1, rows: 0, defaultOutputSize: 2400, strip: 'v' },
] as const;

export const ALL_COLLAGE_LAYOUTS: readonly CollageTemplate[] = [
  ...COLLAGE_TEMPLATES,
  ...COLLAGE_STRIP_TEMPLATES,
];

export const COLLAGE_STRIP_MAX_CELLS = 12;
export const COLLAGE_FREE_MAX_ITEMS = 20;
export const COLLAGE_FREE_SNAP = 0.015;
export const COLLAGE_FREE_MIN_SIZE = 0.08;

function equalGridCells(cols: number, rows: number, margin: number): CollageCellRect[] {
  const gap = margin;
  const cellW = (1 - 2 * margin - (cols - 1) * gap) / cols;
  const cellH = (1 - 2 * margin - (rows - 1) * gap) / rows;
  const out: CollageCellRect[] = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      out.push({
        x: margin + c * (cellW + gap),
        y: margin + r * (cellH + gap),
        w: cellW,
        h: cellH,
      });
    }
  }
  return out;
}

export function getCollageTemplate(id: string): CollageTemplate {
  return ALL_COLLAGE_LAYOUTS.find((t) => t.id === id) || COLLAGE_TEMPLATES.find((t) => t.id === '4')!;
}

export function isFreeformTemplate(id: string): boolean {
  const t = getCollageTemplate(id);
  return t.mode === 'template' && Array.isArray(t.cells) && t.cells.length > 0;
}

export function collageCellCount(id: string, imageCount = 0): number {
  const t = getCollageTemplate(id);
  if (t.mode === 'strip') {
    return Math.max(1, Math.min(COLLAGE_STRIP_MAX_CELLS, Math.max(1, imageCount)));
  }
  if (t.cells?.length) return t.cells.length;
  return t.cols * t.rows;
}

export function collageGridDims(
  id: string,
  imageCount = 0,
): { cols: number; rows: number } {
  const t = getCollageTemplate(id);
  if (t.mode === 'strip') {
    const n = collageCellCount(id, imageCount);
    return t.strip === 'v' ? { cols: 1, rows: n } : { cols: n, rows: 1 };
  }
  return { cols: t.cols, rows: t.rows };
}

/** Normalized cell rects for preview/export (strip generates equal cells). */
export function collageCellRects(id: string, imageCount = 0): CollageCellRect[] {
  const t = getCollageTemplate(id);
  if (t.mode === 'strip') {
    const n = collageCellCount(id, imageCount);
    const margin = 0.012;
    if (t.strip === 'v') {
      const h = (1 - 2 * margin - (n - 1) * margin) / n;
      return Array.from({ length: n }, (_, i) => ({
        x: margin,
        y: margin + i * (h + margin),
        w: 1 - 2 * margin,
        h,
      }));
    }
    const w = (1 - 2 * margin - (n - 1) * margin) / n;
    return Array.from({ length: n }, (_, i) => ({
      x: margin + i * (w + margin),
      y: margin,
      w,
      h: 1 - 2 * margin,
    }));
  }
  if (t.cells?.length) return t.cells.map((c) => ({ ...c }));
  return equalGridCells(Math.max(1, t.cols), Math.max(1, t.rows), 0.0125);
}

export function collageOutputSize(
  id: string,
  imageCount = 0,
): { width: number; height: number } {
  if (id === 'free') {
    return { width: 2400, height: 2400 };
  }
  const t = getCollageTemplate(id);
  const base = t.defaultOutputSize;
  if (t.mode === 'strip') {
    const n = collageCellCount(id, imageCount);
    const shortEdge = Math.round(base * 0.45);
    if (t.strip === 'v') {
      return { width: shortEdge, height: Math.min(8192, shortEdge * n) };
    }
    return { width: Math.min(8192, shortEdge * n), height: shortEdge };
  }
  if (t.aspect && t.aspect > 0) {
    if (t.aspect >= 1) {
      return { width: base, height: Math.max(640, Math.round(base / t.aspect)) };
    }
    return { width: Math.max(640, Math.round(base * t.aspect)), height: base };
  }
  const { cols, rows } = collageGridDims(id, imageCount);
  if (cols === rows) {
    return { width: base, height: base };
  }
  if (cols > rows) {
    return { width: base, height: Math.max(640, Math.round((base * rows) / cols)) };
  }
  return { width: Math.max(640, Math.round((base * cols) / rows)), height: base };
}

export function isImageLikeFile(file: { file_type?: number | null } | null | undefined): boolean {
  const t = Number(file?.file_type || 0);
  return t === 1 || t === 3;
}

export function filterCollageSourceFiles<T extends { file_type?: number | null }>(files: T[]): T[] {
  return (files || []).filter((f) => isImageLikeFile(f));
}

export function pickDefaultTemplateId(imageCount: number): CollageTemplateId {
  if (imageCount >= 9) return '9';
  if (imageCount >= 6) return '6';
  if (imageCount >= 4) return '4';
  if (imageCount >= 3) return '3a';
  if (imageCount >= 2) return '2';
  return '4';
}

function freeItemId(): string {
  return `free-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

/**
 * Seed free-canvas items from selection (cascade layout).
 * Geometry is normalized 0–1.
 */
export function initFreeCollageItems(
  files: Array<{
    file_path?: string;
    name?: string;
    thumbnail?: string;
  }>,
): FreeCollageItem[] {
  const list = (files || [])
    .map((f) => ({
      filePath: String(f.file_path || ''),
      name: String(f.name || f.file_path || ''),
      thumb: String(f.thumbnail || ''),
    }))
    .filter((f) => f.filePath)
    .slice(0, COLLAGE_FREE_MAX_ITEMS);

  const n = Math.max(1, list.length);
  const size = Math.min(0.42, Math.max(0.22, 0.9 / Math.sqrt(n)));
  const step = Math.min(0.12, 0.55 / n);

  return list.map((f, i) => ({
    id: freeItemId(),
    filePath: f.filePath,
    thumb: f.thumb,
    name: f.name,
    x: Math.min(0.55, 0.08 + i * step),
    y: Math.min(0.55, 0.08 + i * step * 0.85),
    w: size,
    h: size,
    rotate: 0,
    z: i + 1,
  }));
}

export function clampFreeItem(item: FreeCollageItem): FreeCollageItem {
  const w = Math.min(1, Math.max(COLLAGE_FREE_MIN_SIZE, item.w));
  const h = Math.min(1, Math.max(COLLAGE_FREE_MIN_SIZE, item.h));
  const x = Math.min(1 - w, Math.max(0, item.x));
  const y = Math.min(1 - h, Math.max(0, item.y));
  let rotate = item.rotate % 360;
  if (rotate > 180) rotate -= 360;
  if (rotate < -180) rotate += 360;
  return { ...item, x, y, w, h, rotate };
}

export function snapFreeScalar(
  value: number,
  guides: number[],
  threshold = COLLAGE_FREE_SNAP,
): { value: number; snapped: boolean } {
  let best = value;
  let bestDist = threshold;
  let snapped = false;
  for (const g of guides) {
    const d = Math.abs(value - g);
    if (d <= bestDist) {
      bestDist = d;
      best = g;
      snapped = true;
    }
  }
  return { value: best, snapped };
}

export function sortFreeByZ(items: FreeCollageItem[]): FreeCollageItem[] {
  return [...items].sort((a, b) => a.z - b.z || a.id.localeCompare(b.id));
}

export const COLLAGE_FREE_DRAFT_LIMIT = 20;

export interface FreeCollageDraftItem {
  filePath: string;
  x: number;
  y: number;
  w: number;
  h: number;
  rotate: number;
  z: number;
}

export interface FreeCollageDraft {
  id: string;
  name: string;
  updatedAt: number;
  fillMode: CollageFillMode;
  radius: number;
  strokeWidth: number;
  strokeColor: string;
  background: string;
  outputFormat: 'jpg' | 'png' | string;
  snapEnabled: boolean;
  items: FreeCollageDraftItem[];
}

export function createFreeDraftId(): string {
  return `draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function serializeFreeDraftItems(items: FreeCollageItem[]): FreeCollageDraftItem[] {
  return sortFreeByZ(items).map((it) => ({
    filePath: it.filePath,
    x: it.x,
    y: it.y,
    w: it.w,
    h: it.h,
    rotate: it.rotate,
    z: it.z,
  }));
}

export function normalizeFreeDrafts(raw: unknown): FreeCollageDraft[] {
  if (!Array.isArray(raw)) return [];
  const out: FreeCollageDraft[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const d = item as FreeCollageDraft;
    const id = String(d.id || '').trim();
    if (!id || !Array.isArray(d.items) || d.items.length === 0) continue;
    const items: FreeCollageDraftItem[] = [];
    for (const it of d.items) {
      if (!it || typeof it !== 'object') continue;
      const filePath = String((it as FreeCollageDraftItem).filePath || '').trim();
      if (!filePath) continue;
      const clamped = clampFreeItem({
        id: freeItemId(),
        filePath,
        thumb: '',
        name: '',
        x: Number((it as FreeCollageDraftItem).x) || 0,
        y: Number((it as FreeCollageDraftItem).y) || 0,
        w: Number((it as FreeCollageDraftItem).w) || 0.3,
        h: Number((it as FreeCollageDraftItem).h) || 0.3,
        rotate: Number((it as FreeCollageDraftItem).rotate) || 0,
        z: Number((it as FreeCollageDraftItem).z) || 1,
      });
      items.push({
        filePath,
        x: clamped.x,
        y: clamped.y,
        w: clamped.w,
        h: clamped.h,
        rotate: clamped.rotate,
        z: clamped.z,
      });
    }
    if (!items.length) continue;
    out.push({
      id,
      name: String(d.name || id),
      updatedAt: Number(d.updatedAt) || Date.now(),
      fillMode: d.fillMode === 'contain' ? 'contain' : 'cover',
      radius: Math.max(0, Number(d.radius) || 0),
      strokeWidth: Math.max(0, Number(d.strokeWidth) || 0),
      strokeColor: String(d.strokeColor || '#000000'),
      background: String(d.background || '#ffffff'),
      outputFormat: d.outputFormat === 'png' ? 'png' : 'jpg',
      snapEnabled: d.snapEnabled !== false,
      items,
    });
  }
  return out.slice(0, COLLAGE_FREE_DRAFT_LIMIT);
}

export function draftMatchCount(
  draft: FreeCollageDraft,
  files: Array<{ file_path?: string }>,
): number {
  const set = new Set((files || []).map((f) => String(f.file_path || '')).filter(Boolean));
  return draft.items.filter((it) => set.has(it.filePath)).length;
}

export function restoreFreeItemsFromDraft(
  draftItems: FreeCollageDraftItem[],
  files: Array<{ file_path?: string; name?: string; thumbnail?: string }>,
): FreeCollageItem[] {
  const byPath = new Map(
    (files || [])
      .map((f) => [String(f.file_path || ''), f] as const)
      .filter(([p]) => !!p),
  );
  const restored: FreeCollageItem[] = [];
  for (const it of draftItems || []) {
    const file = byPath.get(String(it.filePath || ''));
    if (!file) continue;
    restored.push(
      clampFreeItem({
        id: freeItemId(),
        filePath: String(file.file_path || it.filePath),
        thumb: String(file.thumbnail || ''),
        name: String(file.name || file.file_path || it.filePath),
        x: it.x,
        y: it.y,
        w: it.w,
        h: it.h,
        rotate: it.rotate,
        z: it.z,
      }),
    );
  }
  return reindexFreeZ(restored);
}

export function freeSnapGuides(
  items: FreeCollageItem[],
  excludeId?: string | null,
): { x: number[]; y: number[] } {
  const x = new Set<number>([0, 0.5, 1]);
  const y = new Set<number>([0, 0.5, 1]);
  for (const it of items) {
    if (excludeId && it.id === excludeId) continue;
    x.add(it.x);
    x.add(it.x + it.w / 2);
    x.add(it.x + it.w);
    y.add(it.y);
    y.add(it.y + it.h / 2);
    y.add(it.y + it.h);
  }
  return { x: [...x], y: [...y] };
}

export function reindexFreeZ(items: FreeCollageItem[]): FreeCollageItem[] {
  return sortFreeByZ(items).map((it, i) => ({ ...it, z: i + 1 }));
}

export function bringFreeToFront(items: FreeCollageItem[], id: string): FreeCollageItem[] {
  const maxZ = items.reduce((m, it) => Math.max(m, it.z), 0);
  return items.map((it) => (it.id === id ? { ...it, z: maxZ + 1 } : it));
}

export function sendFreeToBack(items: FreeCollageItem[], id: string): FreeCollageItem[] {
  const minZ = items.reduce((m, it) => Math.min(m, it.z), 0);
  return items.map((it) => (it.id === id ? { ...it, z: minZ - 1 } : it));
}
