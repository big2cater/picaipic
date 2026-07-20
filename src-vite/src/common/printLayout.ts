/**
 * Photo print layout / 冲印排版 (paper packing).
 * Dimensions are inches/cm; raster size derived from DPI.
 */

import { BUILTIN_PHOTO_SIZE_PRESETS } from '@/common/photoSizePresets';

export type PaperOrientation = 'landscape' | 'portrait';
export type PhotoOrientation = 'landscape' | 'portrait';

export interface PaperSizeSpec {
  id: string;
  /** i18n key under print_layout.paper.* or display name for custom */
  nameKey?: string;
  name?: string;
  inchW: number;
  inchH: number;
  /** builtin | custom */
  kind: 'builtin' | 'custom';
}

export interface PhotoSlotSpec {
  /** photo size id from photoSizePresets or custom */
  photoId: string;
  orientation: PhotoOrientation;
  /** requested count (rows * cols filled in order); 0 = auto max fit */
  count: number;
}

export interface PrintLayoutPreset {
  id: string;
  /** i18n key or freeform name */
  nameKey?: string;
  name?: string;
  paperId: string;
  paperOrientation: PaperOrientation;
  slots: PhotoSlotSpec[];
  /** gap between photos in cm */
  gapXcm: number;
  gapYcm: number;
  kind: 'builtin' | 'custom';
}

export interface LayoutCell {
  x: number;
  y: number;
  w: number;
  h: number;
  photoId: string;
}

export type PackStrategy = 'auto' | 'h-bands' | 'v-bands';

export interface LayoutPlan {
  paperPxW: number;
  paperPxH: number;
  cells: LayoutCell[];
  /** how many cells requested vs placed */
  placed: number;
  capacity: number;
  /** 0–1 filled area ratio (photos only, gaps excluded) */
  utilization?: number;
  /** packing strategy used */
  strategy?: Exclude<PackStrategy, 'auto'> | 'uniform';
}

export const INCH_TO_CM = 2.54;

export const BUILTIN_PAPER_SIZES: readonly PaperSizeSpec[] = [
  { id: 'paper-3r', kind: 'builtin', nameKey: 'paper_3r', inchW: 5.0, inchH: 3.5 },
  { id: 'paper-4r', kind: 'builtin', nameKey: 'paper_4r', inchW: 6.0, inchH: 4.0 },
  { id: 'paper-5r', kind: 'builtin', nameKey: 'paper_5r', inchW: 7.0, inchH: 5.0 },
  { id: 'paper-6r', kind: 'builtin', nameKey: 'paper_6r', inchW: 8.0, inchH: 6.0 },
  { id: 'paper-8r', kind: 'builtin', nameKey: 'paper_8r', inchW: 10.0, inchH: 8.0 },
  { id: 'paper-a4', kind: 'builtin', nameKey: 'paper_a4', inchW: 8.27, inchH: 11.69 },
  { id: 'paper-a6', kind: 'builtin', nameKey: 'paper_a6', inchW: 4.13, inchH: 5.83 },
] as const;

/** Built-in packing styles matching common 冲印 templates. */
export const BUILTIN_PRINT_LAYOUTS: readonly PrintLayoutPreset[] = [
  {
    id: 'layout-8x1r-3r',
    kind: 'builtin',
    nameKey: 'layout_8x1r_3r',
    paperId: 'paper-3r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [{ photoId: 'photo-1r', orientation: 'portrait', count: 8 }],
  },
  {
    id: 'layout-9x-id-3r',
    kind: 'builtin',
    nameKey: 'layout_9x_id_3r',
    paperId: 'paper-3r',
    paperOrientation: 'landscape',
    gapXcm: 0.15,
    gapYcm: 0.15,
    slots: [{ photoId: 'photo-cn-id', orientation: 'portrait', count: 9 }],
  },
  {
    id: 'layout-4x-passport-3r',
    kind: 'builtin',
    nameKey: 'layout_4x_passport_3r',
    paperId: 'paper-3r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [{ photoId: 'photo-passport', orientation: 'portrait', count: 4 }],
  },
  {
    id: 'layout-4x-2r-large-3r',
    kind: 'builtin',
    nameKey: 'layout_4x_2r_large_3r',
    paperId: 'paper-3r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [{ photoId: 'photo-2r-large', orientation: 'portrait', count: 4 }],
  },
  {
    id: 'layout-16x1r-4r',
    kind: 'builtin',
    nameKey: 'layout_16x1r_4r',
    paperId: 'paper-4r',
    paperOrientation: 'landscape',
    gapXcm: 0.15,
    gapYcm: 0.15,
    slots: [{ photoId: 'photo-1r', orientation: 'portrait', count: 16 }],
  },
  {
    id: 'layout-8x2r-4r',
    kind: 'builtin',
    nameKey: 'layout_8x2r_4r',
    paperId: 'paper-4r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [{ photoId: 'photo-2r', orientation: 'portrait', count: 8 }],
  },
  {
    id: 'layout-mix-1r2r-3r',
    kind: 'builtin',
    nameKey: 'layout_mix_1r2r_3r',
    paperId: 'paper-3r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [
      { photoId: 'photo-1r', orientation: 'portrait', count: 4 },
      { photoId: 'photo-2r', orientation: 'portrait', count: 2 },
    ],
  },
  {
    id: 'layout-mix-1r2r-4r-a',
    kind: 'builtin',
    nameKey: 'layout_mix_1r2r_4r_a',
    paperId: 'paper-4r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [
      { photoId: 'photo-1r', orientation: 'portrait', count: 8 },
      { photoId: 'photo-2r', orientation: 'portrait', count: 2 },
    ],
  },
  {
    id: 'layout-mix-1r2r-4r-b',
    kind: 'builtin',
    nameKey: 'layout_mix_1r2r_4r_b',
    paperId: 'paper-4r',
    paperOrientation: 'landscape',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [
      { photoId: 'photo-1r', orientation: 'portrait', count: 6 },
      { photoId: 'photo-2r', orientation: 'portrait', count: 4 },
    ],
  },
  {
    id: 'layout-2x-wallet-s-3r',
    kind: 'builtin',
    nameKey: 'layout_2x_wallet_s_3r',
    paperId: 'paper-3r',
    paperOrientation: 'landscape',
    // Keep gap tiny so two wallet photos still fit on 3R (reference: 光影魔术手 fills paper).
    gapXcm: 0.12,
    gapYcm: 0.12,
    slots: [{ photoId: 'photo-wallet-small', orientation: 'portrait', count: 2 }],
  },
  {
    id: 'layout-2x-wallet-l-4r',
    kind: 'builtin',
    nameKey: 'layout_2x_wallet_l_4r',
    paperId: 'paper-4r',
    paperOrientation: 'landscape',
    gapXcm: 0.12,
    gapYcm: 0.12,
    slots: [{ photoId: 'photo-wallet-large', orientation: 'portrait', count: 2 }],
  },
  {
    id: 'layout-8x1r-a4',
    kind: 'builtin',
    nameKey: 'layout_8x1r_a4',
    paperId: 'paper-a4',
    paperOrientation: 'portrait',
    gapXcm: 0.25,
    gapYcm: 0.25,
    slots: [{ photoId: 'photo-1r', orientation: 'portrait', count: 8 }],
  },
  {
    id: 'layout-16x1r-a4',
    kind: 'builtin',
    nameKey: 'layout_16x1r_a4',
    paperId: 'paper-a4',
    paperOrientation: 'portrait',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [{ photoId: 'photo-1r', orientation: 'portrait', count: 16 }],
  },
  {
    id: 'layout-8x2r-a4',
    kind: 'builtin',
    nameKey: 'layout_8x2r_a4',
    paperId: 'paper-a4',
    paperOrientation: 'portrait',
    gapXcm: 0.25,
    gapYcm: 0.25,
    slots: [{ photoId: 'photo-2r', orientation: 'portrait', count: 8 }],
  },
  {
    id: 'layout-4x-wallet-l-a4',
    kind: 'builtin',
    nameKey: 'layout_4x_wallet_l_a4',
    paperId: 'paper-a4',
    paperOrientation: 'portrait',
    gapXcm: 0.25,
    gapYcm: 0.25,
    slots: [{ photoId: 'photo-wallet-large', orientation: 'portrait', count: 4 }],
  },
  {
    id: 'layout-mix-1r2r-a4',
    kind: 'builtin',
    nameKey: 'layout_mix_1r2r_a4',
    paperId: 'paper-a4',
    paperOrientation: 'portrait',
    gapXcm: 0.2,
    gapYcm: 0.2,
    slots: [
      { photoId: 'photo-1r', orientation: 'portrait', count: 8 },
      { photoId: 'photo-2r', orientation: 'portrait', count: 4 },
    ],
  },
] as const;

export function inchToCm(inch: number): number {
  return inch * INCH_TO_CM;
}

export function cmToInch(cm: number): number {
  return cm / INCH_TO_CM;
}

export function paperPixelSize(
  paper: PaperSizeSpec,
  orientation: PaperOrientation,
  dpi: number,
): { w: number; h: number; inchW: number; inchH: number } {
  let inchW = paper.inchW;
  let inchH = paper.inchH;
  const landscape = inchW >= inchH;
  if (orientation === 'landscape' && !landscape) {
    [inchW, inchH] = [inchH, inchW];
  } else if (orientation === 'portrait' && landscape) {
    [inchW, inchH] = [inchH, inchW];
  }
  const d = Math.max(72, Math.min(600, dpi || 300));
  return {
    inchW,
    inchH,
    w: Math.max(64, Math.round(inchW * d)),
    h: Math.max(64, Math.round(inchH * d)),
  };
}

export function photoInchSize(
  photoId: string,
  orientation: PhotoOrientation,
): { inchW: number; inchH: number } | null {
  const p = BUILTIN_PHOTO_SIZE_PRESETS.find((x) => x.id === photoId);
  if (!p) return null;
  let inchW = p.inchW;
  let inchH = p.inchH;
  const isLandscape = inchW >= inchH;
  if (orientation === 'portrait' && isLandscape) {
    [inchW, inchH] = [inchH, inchW];
  } else if (orientation === 'landscape' && !isLandscape) {
    [inchW, inchH] = [inchH, inchW];
  }
  return { inchW, inchH };
}

type ResolvedSlot = {
  photoId: string;
  inchW: number;
  inchH: number;
  /** 0 = fill remaining band with max fit */
  count: number;
};

function clampDpi(dpi: number): number {
  return Math.max(72, Math.min(600, dpi || 300));
}

function inchCellsToPx(
  cellsInch: Array<{ x: number; y: number; w: number; h: number; photoId: string }>,
  dpi: number,
): LayoutCell[] {
  const d = clampDpi(dpi);
  return cellsInch.map((c) => ({
    x: Math.round(c.x * d),
    y: Math.round(c.y * d),
    w: Math.max(1, Math.round(c.w * d)),
    h: Math.max(1, Math.round(c.h * d)),
    photoId: c.photoId,
  }));
}

function utilizationOf(
  cells: LayoutCell[],
  paperPxW: number,
  paperPxH: number,
): number {
  const area = paperPxW * paperPxH;
  if (area <= 0) return 0;
  const filled = cells.reduce((sum, c) => sum + c.w * c.h, 0);
  return Math.max(0, Math.min(1, filled / area));
}

/**
 * Pack one photo size onto paper as a regular grid.
 *
 * Matches 光影魔术手-style 冲印预览: keep photo aspect, but scale the grid so
 * the sheet is almost fully filled (small outer margin), instead of leaving large
 * empty borders from fixed physical inch sizes.
 */
export function packUniformGrid(options: {
  paperInchW: number;
  paperInchH: number;
  photoInchW: number;
  photoInchH: number;
  gapXcm: number;
  gapYcm: number;
  count: number;
  photoId: string;
  dpi: number;
  /** when false, align to origin (0,0) instead of centering */
  center?: boolean;
  /** outer paper margin in cm (default ~0.15cm like reference tools) */
  marginCm?: number;
}): LayoutCell[] {
  const { paperInchW, paperInchH, photoInchW, photoInchH, photoId, dpi } = options;
  if (photoInchW <= 0 || photoInchH <= 0 || paperInchW <= 0 || paperInchH <= 0) return [];

  const gapXinch = cmToInch(Math.max(0, options.gapXcm));
  const gapYinch = cmToInch(Math.max(0, options.gapYcm));
  const marginInch = cmToInch(Math.max(0, options.marginCm ?? 0.15));
  const aspect = photoInchW / photoInchH;

  // Physical capacity estimate (how many can fit at true size). Used as upper bound.
  const physCols = Math.max(1, Math.floor((paperInchW + gapXinch) / (photoInchW + gapXinch)));
  const physRows = Math.max(1, Math.floor((paperInchH + gapYinch) / (photoInchH + gapYinch)));
  const physCap = physCols * physRows;
  const requested = options.count > 0 ? options.count : physCap;
  if (requested <= 0) return [];

  // Prefer a near-square grid for the requested count, then expand if needed so
  // scaled cells still fill the paper without huge empty gutters.
  let best: { cols: number; rows: number; score: number } | null = null;
  const maxProbe = Math.max(physCap, requested, 1);
  for (let cols = 1; cols <= Math.max(requested, physCols, 8); cols++) {
    const rows = Math.ceil(requested / cols);
    if (cols * rows < requested) continue;
    // Score: prefer grids close to paper aspect and not far above requested count.
    const gridAspect = (cols * aspect) / rows;
    const paperAspect = paperInchW / paperInchH;
    const aspectPenalty = Math.abs(Math.log(gridAspect / paperAspect));
    const wastePenalty = (cols * rows - requested) * 0.15;
    const score = aspectPenalty + wastePenalty;
    if (!best || score < best.score) best = { cols, rows, score };
    if (cols * rows > maxProbe * 2) break;
  }
  if (!best) return [];

  const useCols = best.cols;
  const useRows = best.rows;
  const n = Math.min(requested, useCols * useRows);

  // Scale cell size so the used grid fills the paper (minus outer margin + gaps).
  const availW = Math.max(0.01, paperInchW - 2 * marginInch - (useCols - 1) * gapXinch);
  const availH = Math.max(0.01, paperInchH - 2 * marginInch - (useRows - 1) * gapYinch);
  const cellFromW = availW / useCols;
  const cellFromH = availH / useRows;
  // Preserve photo aspect while maximizing fill.
  let cellW = cellFromW;
  let cellH = cellW / aspect;
  if (cellH > cellFromH) {
    cellH = cellFromH;
    cellW = cellH * aspect;
  }
  // Never upscale beyond a gentle limit over physical size (avoid giant ID photos on A4 looking wrong).
  // Still allow fill-up: cap at max(physical, fill-derived) so 3R/4R sheets stay full.
  const maxW = Math.max(photoInchW, cellFromW);
  const maxH = Math.max(photoInchH, cellFromH);
  cellW = Math.min(cellW, maxW);
  cellH = Math.min(cellH, maxH);
  // Re-fit if clamp broke aspect slightly
  if (cellW / cellH > aspect) cellW = cellH * aspect;
  else cellH = cellW / aspect;

  const gridW = useCols * cellW + (useCols - 1) * gapXinch;
  const gridH = useRows * cellH + (useRows - 1) * gapYinch;
  const center = options.center !== false;
  const originX = center ? Math.max(marginInch, (paperInchW - gridW) / 2) : marginInch;
  const originY = center ? Math.max(marginInch, (paperInchH - gridH) / 2) : marginInch;
  const d = clampDpi(dpi);

  const cells: LayoutCell[] = [];
  for (let i = 0; i < n; i++) {
    const r = Math.floor(i / useCols);
    const c = i % useCols;
    const ix = originX + c * (cellW + gapXinch);
    const iy = originY + r * (cellH + gapYinch);
    cells.push({
      x: Math.round(ix * d),
      y: Math.round(iy * d),
      w: Math.max(1, Math.round(cellW * d)),
      h: Math.max(1, Math.round(cellH * d)),
      photoId,
    });
  }
  return cells;
}

/**
 * Sequential horizontal shelf packing: each slot becomes a top-to-bottom band.
 * Bands scale to paper width (fill) while preserving each photo aspect.
 */
export function packHorizontalBands(options: {
  paperInchW: number;
  paperInchH: number;
  slots: ResolvedSlot[];
  gapXcm: number;
  gapYcm: number;
  dpi: number;
}): { cells: LayoutCell[]; capacity: number } {
  const gapXinch = cmToInch(Math.max(0, options.gapXcm));
  const gapYinch = cmToInch(Math.max(0, options.gapYcm));
  const marginInch = cmToInch(0.15);
  const paperW = options.paperInchW;
  const paperH = options.paperInchH;
  const inchCells: Array<{ x: number; y: number; w: number; h: number; photoId: string }> = [];
  let cursorY = marginInch;
  let capacity = 0;
  const innerW = Math.max(0.01, paperW - 2 * marginInch);
  const bottom = paperH - marginInch;

  for (let si = 0; si < options.slots.length; si++) {
    const slot = options.slots[si];
    if (!slot || slot.inchW <= 0 || slot.inchH <= 0) continue;
    if (cursorY > marginInch) cursorY += gapYinch;
    const remainH = bottom - cursorY;
    if (remainH <= 0) break;

    const aspect = slot.inchW / slot.inchH;
    // Prefer cols that fit physical size; fall back to fewer wider cells.
    let cols = Math.max(1, Math.floor((innerW + gapXinch) / (slot.inchW + gapXinch)));
    if (slot.count > 0) cols = Math.min(cols, Math.max(1, slot.count));
    let cellW = (innerW - (cols - 1) * gapXinch) / cols;
    let cellH = cellW / aspect;
    // If one row is taller than remaining, shrink.
    if (cellH > remainH) {
      cellH = remainH;
      cellW = cellH * aspect;
    }

    const maxRows = Math.max(1, Math.floor((remainH + gapYinch) / (cellH + gapYinch)));
    const bandCapacity = cols * maxRows;
    capacity += bandCapacity;
    const n = slot.count > 0 ? Math.min(slot.count, bandCapacity) : bandCapacity;
    if (n <= 0) continue;
    const useRows = Math.ceil(n / cols);
    const bandH = useRows * cellH + (useRows - 1) * gapYinch;
    if (bandH > remainH + 1e-9) break;
    const gridW = cols * cellW + (cols - 1) * gapXinch;
    const originX = Math.max(marginInch, (paperW - gridW) / 2);
    const originY = cursorY;

    for (let i = 0; i < n; i++) {
      const r = Math.floor(i / cols);
      const c = i % cols;
      inchCells.push({
        x: originX + c * (cellW + gapXinch),
        y: originY + r * (cellH + gapYinch),
        w: cellW,
        h: cellH,
        photoId: slot.photoId,
      });
    }
    cursorY = originY + bandH;
  }

  return { cells: inchCellsToPx(inchCells, options.dpi), capacity };
}

/**
 * Sequential vertical band packing: each slot becomes a left-to-right strip.
 */
export function packVerticalBands(options: {
  paperInchW: number;
  paperInchH: number;
  slots: ResolvedSlot[];
  gapXcm: number;
  gapYcm: number;
  dpi: number;
}): { cells: LayoutCell[]; capacity: number } {
  const gapXinch = cmToInch(Math.max(0, options.gapXcm));
  const gapYinch = cmToInch(Math.max(0, options.gapYcm));
  const marginInch = cmToInch(0.15);
  const paperW = options.paperInchW;
  const paperH = options.paperInchH;
  const inchCells: Array<{ x: number; y: number; w: number; h: number; photoId: string }> = [];
  let cursorX = marginInch;
  let capacity = 0;
  const innerH = Math.max(0.01, paperH - 2 * marginInch);
  const right = paperW - marginInch;

  for (let si = 0; si < options.slots.length; si++) {
    const slot = options.slots[si];
    if (!slot || slot.inchW <= 0 || slot.inchH <= 0) continue;
    if (cursorX > marginInch) cursorX += gapXinch;
    const remainW = right - cursorX;
    if (remainW <= 0) break;

    const aspect = slot.inchW / slot.inchH;
    let rows = Math.max(1, Math.floor((innerH + gapYinch) / (slot.inchH + gapYinch)));
    if (slot.count > 0) rows = Math.min(rows, Math.max(1, slot.count));
    let cellH = (innerH - (rows - 1) * gapYinch) / rows;
    let cellW = cellH * aspect;
    if (cellW > remainW) {
      cellW = remainW;
      cellH = cellW / aspect;
    }

    const maxCols = Math.max(1, Math.floor((remainW + gapXinch) / (cellW + gapXinch)));
    const bandCapacity = rows * maxCols;
    capacity += bandCapacity;
    const n = slot.count > 0 ? Math.min(slot.count, bandCapacity) : bandCapacity;
    if (n <= 0) continue;
    const useCols = Math.ceil(n / rows);
    const bandW = useCols * cellW + (useCols - 1) * gapXinch;
    if (bandW > remainW + 1e-9) break;
    const gridH = rows * cellH + (rows - 1) * gapYinch;
    const originX = cursorX;
    const originY = Math.max(marginInch, (paperH - gridH) / 2);

    for (let i = 0; i < n; i++) {
      const c = Math.floor(i / rows);
      const r = i % rows;
      inchCells.push({
        x: originX + c * (cellW + gapXinch),
        y: originY + r * (cellH + gapYinch),
        w: cellW,
        h: cellH,
        photoId: slot.photoId,
      });
    }
    cursorX = originX + bandW;
  }

  return { cells: inchCellsToPx(inchCells, options.dpi), capacity };
}

function resolveSlots(slots: PhotoSlotSpec[]): ResolvedSlot[] {
  const out: ResolvedSlot[] = [];
  for (const slot of slots) {
    if (!slot?.photoId) continue;
    const photo = photoInchSize(slot.photoId, slot.orientation);
    if (!photo) continue;
    out.push({
      photoId: slot.photoId,
      inchW: photo.inchW,
      inchH: photo.inchH,
      count: Math.max(0, Number(slot.count) || 0),
    });
  }
  return out;
}

function scorePack(cells: LayoutCell[], capacity: number, paperPxW: number, paperPxH: number): number {
  // Prefer more placed cells, then higher area utilization, then higher reported capacity.
  const util = utilizationOf(cells, paperPxW, paperPxH);
  return cells.length * 1000 + util * 100 + capacity * 0.01;
}

/**
 * Build a full layout plan from paper + one or more photo slot groups.
 * - 1 slot: centered uniform grid
 * - 2+ slots: shelf packing — try horizontal bands (上带+下带) and vertical bands (左带+右带),
 *   pick the better utilization (or honor an explicit strategy)
 * - slot.count = 0 means auto max-fit in remaining space for that band
 */
export function buildLayoutPlan(options: {
  paper: PaperSizeSpec;
  paperOrientation: PaperOrientation;
  slots: PhotoSlotSpec[];
  gapXcm: number;
  gapYcm: number;
  dpi: number;
  strategy?: PackStrategy;
}): LayoutPlan {
  const paperPx = paperPixelSize(options.paper, options.paperOrientation, options.dpi);
  const empty: LayoutPlan = {
    paperPxW: paperPx.w,
    paperPxH: paperPx.h,
    cells: [],
    placed: 0,
    capacity: 0,
    utilization: 0,
    strategy: 'uniform',
  };

  const resolved = resolveSlots(options.slots || []);
  if (!resolved.length) return empty;

  if (resolved.length === 1) {
    const slot = resolved[0];
    const packed = packUniformGrid({
      paperInchW: paperPx.inchW,
      paperInchH: paperPx.inchH,
      photoInchW: slot.inchW,
      photoInchH: slot.inchH,
      gapXcm: options.gapXcm,
      gapYcm: options.gapYcm,
      count: slot.count,
      photoId: slot.photoId,
      dpi: options.dpi,
      center: true,
    });
    // Capacity = max fill for this photo size (count=0), not physical-inch floor
    // (fill packing may place more/less than naive inch math).
    const maxPacked = packUniformGrid({
      paperInchW: paperPx.inchW,
      paperInchH: paperPx.inchH,
      photoInchW: slot.inchW,
      photoInchH: slot.inchH,
      gapXcm: options.gapXcm,
      gapYcm: options.gapYcm,
      count: 0,
      photoId: slot.photoId,
      dpi: options.dpi,
      center: true,
    });
    const capacity = Math.max(packed.length, maxPacked.length);
    return {
      paperPxW: paperPx.w,
      paperPxH: paperPx.h,
      cells: packed,
      placed: packed.length,
      capacity,
      utilization: utilizationOf(packed, paperPx.w, paperPx.h),
      strategy: 'uniform',
    };
  }

  const strategy = options.strategy || 'auto';
  const hPack = packHorizontalBands({
    paperInchW: paperPx.inchW,
    paperInchH: paperPx.inchH,
    slots: resolved,
    gapXcm: options.gapXcm,
    gapYcm: options.gapYcm,
    dpi: options.dpi,
  });
  const vPack = packVerticalBands({
    paperInchW: paperPx.inchW,
    paperInchH: paperPx.inchH,
    slots: resolved,
    gapXcm: options.gapXcm,
    gapYcm: options.gapYcm,
    dpi: options.dpi,
  });

  let chosen = hPack;
  let chosenStrategy: 'h-bands' | 'v-bands' = 'h-bands';
  if (strategy === 'v-bands') {
    chosen = vPack;
    chosenStrategy = 'v-bands';
  } else if (strategy === 'h-bands') {
    chosen = hPack;
    chosenStrategy = 'h-bands';
  } else {
    const hScore = scorePack(hPack.cells, hPack.capacity, paperPx.w, paperPx.h);
    const vScore = scorePack(vPack.cells, vPack.capacity, paperPx.w, paperPx.h);
    if (vScore > hScore) {
      chosen = vPack;
      chosenStrategy = 'v-bands';
    }
  }

  return {
    paperPxW: paperPx.w,
    paperPxH: paperPx.h,
    cells: chosen.cells,
    placed: chosen.cells.length,
    capacity: Math.max(chosen.capacity, chosen.cells.length),
    utilization: utilizationOf(chosen.cells, paperPx.w, paperPx.h),
    strategy: chosenStrategy,
  };
}

export function createCustomId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
}

export function normalizePaperSizes(raw: unknown): PaperSizeSpec[] {
  if (!Array.isArray(raw)) return [];
  const out: PaperSizeSpec[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const p = item as PaperSizeSpec;
    const id = String(p.id || '').trim();
    const inchW = Number(p.inchW);
    const inchH = Number(p.inchH);
    if (!id || !(inchW > 0) || !(inchH > 0)) continue;
    out.push({
      id,
      kind: 'custom',
      name: String(p.name || id),
      inchW,
      inchH,
    });
  }
  return out;
}

export function normalizePrintLayouts(raw: unknown): PrintLayoutPreset[] {
  if (!Array.isArray(raw)) return [];
  const out: PrintLayoutPreset[] = [];
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue;
    const p = item as PrintLayoutPreset;
    const id = String(p.id || '').trim();
    if (!id || !p.paperId || !Array.isArray(p.slots) || p.slots.length === 0) continue;
    out.push({
      id,
      kind: 'custom',
      name: String(p.name || id),
      paperId: String(p.paperId),
      paperOrientation: p.paperOrientation === 'portrait' ? 'portrait' : 'landscape',
      gapXcm: Math.max(0, Number(p.gapXcm) || 0.3),
      gapYcm: Math.max(0, Number(p.gapYcm) || 0.3),
      slots: p.slots.map((s) => ({
        photoId: String(s.photoId || ''),
        orientation: s.orientation === 'landscape' ? 'landscape' : 'portrait',
        count: Math.max(0, Number(s.count) || 0),
      })).filter((s) => s.photoId),
    });
  }
  return out;
}

export function allPapers(custom: PaperSizeSpec[] = []): PaperSizeSpec[] {
  return [...BUILTIN_PAPER_SIZES, ...custom];
}

export function allLayouts(custom: PrintLayoutPreset[] = []): PrintLayoutPreset[] {
  return [...BUILTIN_PRINT_LAYOUTS, ...custom];
}

export function findPaper(id: string, custom: PaperSizeSpec[] = []): PaperSizeSpec | undefined {
  return allPapers(custom).find((p) => p.id === id);
}

export function findLayout(id: string, custom: PrintLayoutPreset[] = []): PrintLayoutPreset | undefined {
  return allLayouts(custom).find((l) => l.id === id);
}
