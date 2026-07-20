# Built-in tools roadmap (crop presets · collage · batch)

Status: **Phase A + B + C1/C2 + D shipped** (crop, collage magazine cells, batch watermark/text/border/expand, print layout).  
Recorded: 2026-07-19  
Reference UX: 光影魔术手-style local tools (crop preset menu, collage modes, multi-step batch).  
Non-goal for this plan: AI plugins, cloud processing, or full “magic hand” feature parity in one release.

## Product intent

Keep PicAiPic local-first. Add lightweight **host-built-in** tools that users expect from a photo manager/editor:

1. **Crop with photo-size presets** (证件照 / 相纸规格) under a crop sub-menu  
2. **Collage / 拼图** as a first-class tool entry  
3. **Batch processing / 批处理** for multi-image pipelines  

Prefer built-in host paths for deterministic geometry/export work. Do not force these through the AI plugin runtime.

## Priority (suggested ship order)

| Phase | Feature | Why first |
|-------|---------|-----------|
| **A** | Crop aspect / photo-size sub-menu + preset catalog | Extends existing ImageEditor crop; smallest surface |
| **B** | Collage (template / strip / free) | New surface, single-session creative tool |
| **C** | Batch wizard (add → actions → output) | Highest leverage; reuses resize/crop/watermark actions later |

Phases can overlap in design, but implementation should land **A → B → C** unless product priority changes.

---

## A. Crop sub-menu + built-in photo size presets

### UX (target)

- Crop control exposes a **dropdown / sub-menu** (not only free crop).
- Menu groups:
  - **Common ratios:** `1:1`, `3:2`, `4:3`, `16:9` (and free / custom)
  - **Built-in photo sizes** (print / ID style), e.g.:
    - 标准1寸/1R, 标准2寸/2R, 大2寸/2R
    - 二代身份证, 护照照片
    - 5寸/3R, 6寸/4R, 7寸/5R, 8寸/6R, 10寸/8R
    - 小皮夹照, 大皮夹照
  - **Management:** 照片规格管理 · 添加常用比例 · 删除常用比例
- Selecting a preset locks crop box to that aspect (and optionally target pixel size / DPI metadata for export).
- Portrait / landscape toggle remains available where the preset allows.

### Built-in preset catalog (initial data)

Source table for v1 presets (print-oriented). Values are product defaults; UI should show inch + cm + px @ DPI.

| 相纸尺寸 | 实际大小 (英寸) | 像素要求 | 实际大小 (厘米) | DPI |
|----------|-----------------|----------|-----------------|-----|
| 标准1寸/1R | 0.98 × 1.38 | 295 × 413 | 2.5 × 3.5 | 300 |
| 标准2寸/2R | 1.38 × 1.93 | 413 × 579 | 3.5 × 4.9 | 300 |
| 大2寸/2R | 1.38 × 2.09 | 413 × 626 | 3.5 × 5.3 | 300 |
| 二代身份证 | 1.02 × 1.26 | 358 × 441 | 2.6 × 3.2 | 350 |
| 护照照片 | 1.30 × 1.89 | 390 × 567 | 3.3 × 4.8 | 300 |
| 5寸/3R | 5.00 × 3.50 | 1500 × 1050 | 12.7 × 8.9 | 300 |
| 6寸/4R | 6.00 × 4.00 | 1800 × 1200 | 15.2 × 10.2 | 300 |
| 7寸/5R | 7.00 × 5.00 | 2100 × 1500 | 17.8 × 12.7 | 300 |
| 8寸/6R | 8.00 × 6.00 | 2400 × 1800 | 20.3 × 15.2 | 300 |
| 10寸/8R | 10.00 × 8.00 | 3000 × 2400 | 25.4 × 20.3 | 300 |
| 小皮夹照 | 2.49 × 3.50 | 748 × 1050 | 6.3 × 8.9 | 300 |
| 大皮夹照 | 2.99 × 4.00 | 898 × 1200 | 7.6 × 10.2 | 300 |

Also keep pure ratio presets: `1:1`, `3:2`, `4:3`, `16:9`.

### Behavior notes

- **Crop apply** still uses the existing editor apply/cancel/restore flow.
- Preset “像素要求” is the **target output size** when exporting/saving from that crop mode; crop UI primarily enforces **aspect**.
- User-defined favorites: store under app config (per user), not per-library, unless we later decide library-scoped presets.
- Spec management UI can be a simple dialog: list built-in (read-only) + user customs (edit/delete).

### Likely touchpoints

- `src-vite/src/views/ImageEditor.vue` (crop toolbar / `cropShape` select → menu)
- i18n `en.json` / `zh.json`
- Optional shared preset module: e.g. `src-vite/src/common/photoSizePresets.ts`
- Optional host command only if export-at-DPI needs server-side resize; otherwise frontend + existing save path may suffice for v1

### Acceptance (Phase A)

- [x] Crop opens sub-menu with ratios + built-in photo sizes  
- [x] Selecting a preset constrains crop aspect correctly (portrait/landscape)  
- [x] Custom favorite ratio can be added and removed  
- [x] Built-in list matches catalog above (labels + aspect + declared px/DPI)  
- [x] Existing free crop / apply / cancel still work  

### Implementation notes (Phase A, 2026-07-18)

- Shared catalog: `src-vite/src/common/photoSizePresets.ts`
- Config (persisted, app-wide): `imageEditor.cropPresetId`, `imageEditor.customCropRatios` (legacy `cropShape` migrated on load)
- UI: grouped crop `<select>` + manage/add dialogs in `ImageEditor.vue`
- Photo presets show target px@DPI hint and prefill resize width/height when selected
- Dialogs: `PhotoSizeManageDialog.vue`, `AddCustomCropRatioDialog.vue`  

---

## B. Collage / 拼图

### UX (target)

Top-level tool entry **拼图** with sub-modes (aligned to reference):

| Mode | Intent |
|------|--------|
| **模板拼图** | Fixed layouts (2/3/4/6/9 grid, magazine-style templates) |
| **图片拼接** | Strip join: horizontal or vertical sequence |
| **自由拼图** | Free drag/resize/rotate on a canvas |

### Scope by slice

**B1 – MVP**

- Entry from editor or selection context (exact entry TBD in design)
- Template grid: 2 / 4 / 9 equal cells
- Pick images from current selection or file picker
- Export one JPEG/PNG to chosen path or library album (policy TBD)
- Background color + cell gap + outer margin

**B2** ✅ (2026-07-18)

- More templates; horizontal/vertical 拼接
- Cell fill modes: cover / contain
- Optional border radius / stroke

**B3** ✅ (2026-07-18)

- Free collage canvas
- Z-order, snap, rotate
- Save project draft (optional; not required for MVP)

### Constraints

- Local-only; no upload
- Large inputs: downscale for interactive canvas; full-res on export
- Do not mutate library originals unless user explicitly saves into library with confirmation

### Likely touchpoints

- New Vue view/dialog under `src-vite/src/views` or `components`
- Possible Rust export helper for high-res composite (or canvas export in frontend first)
- Album/selection integration via existing Content selection APIs

### Acceptance (Phase B1)

- [x] 拼图 entry visible and opens mode chooser  
- [x] Template 2/4/9 works with multi-select or picker  
- [x] Export produces a single image file  
- [x] Cancel discards work without writing library originals  

### Implementation notes (Phase B1, 2026-07-18)

- Entry: multi-select → right panel **拼图** (`SelectionPanel` → `CollageDialog`)
- Templates: equal grids `2` (1×2), `4` (2×2), `9` (3×3); gap / margin / background / JPEG|PNG
- Preview uses selection thumbnails (cover); full export via host `export_collage` (HEIC/RAW preview path + cover crop)
- Export is **save-as** only (does not import back into library or mutate originals)
- Frontend: `collageTemplates.ts`, `CollageDialog.vue`; API: `exportCollage`

### Implementation notes (Phase B2, 2026-07-18)

- Templates expanded: `2` / `3` / `4` / `6` / `9` equal grids
- Strip mode: `strip-h` / `strip-v` (up to 12 cells from selection order)
- Fill: `cover` | `contain` (preview + host export)
- Cell chrome: corner radius + stroke width/color (export uses rounded-rect SDF mask)
- Output aspect follows layout (non-square grids and elongated strips)

### Implementation notes (Phase B3, 2026-07-18)

- Mode **自由拼图**: free canvas with normalized item geometry (`x/y/w/h/rotate/z`)
- Interaction: drag move, SE resize handle, ±15° rotate, bring front / send back, optional snap to edges/mid/other items
- Host: `export_collage` with `template: "free"` + `items[]`; rotate expands cell AABB then overlays by center
- Free project drafts: app-config `collage.freeDrafts` (name + geometry + style); load remaps by file path against current selection


### Implementation notes (Phase B magazine + cell-sized decode, 2026-07-19)

- Magazine freeform templates (NeoImaging PatternJigsaw-normalized): `2`, `2v`, `3a`, `3b`, `4`, `4m`, `6`, `6m`, `9` with `cells[]` in `collageTemplates.ts`
- Preview uses absolute cell rects; export sends `template: "cells"` + normalized cells to host
- Host `export_collage` / `export_collage_cells` / free: downscale sources to on-canvas cell need (`load_image_for_layout` + `downscale_image_for_fit_cells`); parallel unique decode for freeform/free
- Free drafts helpers restored in `collageTemplates.ts` (`COLLAGE_FREE_DRAFT_LIMIT`, serialize/restore/snap)

---

## C. Batch processing / 批处理

### UX (target) — 3-step wizard

Reference flow:

1. **第一步：添加照片**  
   - Counter: 共 N 张待处理图片  
   - Actions: 添加图片 · 添加文件夹 · 清空列表  
   - Grid/list toggle for thumbnails  
2. **第二步：动作设置**  
   - Left: ordered **动作列表** (reorder up/down, clear)  
   - Right: add actions from palette  
   - Optional: 载入模板  
   - 预览 / 上一步 / 下一步  
3. **第三步：输出设置**  
   - 输出路径：另存为 / 原文件路径  
   - 输出文件名：原文件名 / 重命名（格式、序号、EXIF tokens — phased）  
   - 重名策略：覆盖提示 / 直接覆盖 / 跳过  
   - 格式：jpg / png / bmp  
   - 质量滑条、高质量 JPEG、限制文件大小、删除 Exif  
   - **开始批处理**

### Action palette (planned; ship in waves)

| Action | Wave |
|--------|------|
| 调整尺寸 | C1 ✅ |
| 裁剪（含规格预设） | C1 ✅ (ratios + photo sizes + custom favorites) |
| 旋转 / 翻转 | C1 ✅ |
| 亮度 / 对比度 / 饱和度 / 色相 / 模糊 / 滤镜 | C1 ✅ |
| 一键动作（可保存动作链模板） | C1 ✅ |
| 添加水印 | C2 ✅ (image stamp) |
| 添加文字 | C2 ✅ (system font) |
| 添加边框 | C2 ✅ |
| 扩边 | C2 ✅ |
| 插入拼图模板 | C3 |

### Engine notes

- Process **offline** on host; show progress + per-file errors  
- Prefer a queue with cancel  
- Destructive “原文件路径 + 覆盖” requires clear confirmation  
- Reuse Phase A presets when batch crop/resize targets photo sizes  
- 10k-library performance: batch should work on an explicit file list, not whole library scans by default  

### Likely touchpoints

- New batch wizard component + route/modal from Content toolbar  
- Host commands for multi-file transform/export (new or extended image pipeline)  
- Config for batch templates under app data  

### Acceptance (Phase C1)

- [x] Wizard opens; can add images and clear list  
- [x] Ordered composable actions: resize, crop presets, rotate/flip, adjustments, filters  
- [x] Output to “另存为” folder with jpg/png/webp and quality; overwrite needs confirm  
- [x] Progress + failure summary; cancel stops further files  
- [x] Default path does not silently overwrite originals  
- [x] One-click action templates save/load in app config  

### Implementation notes (Phase C1, 2026-07-18)

- Entry: multi-select → **批处理** (`SelectionPanel` → `BatchProcessDialog`)
- Frontend: `batchProcess.ts`, three-step wizard (files → action chain palette → output)
- Host: `batch_process_images` / `cancel_batch_process`; progress event `batch-process-progress`
- Templates: `config.batchProcess.templates` (action chains = 一键动作)
- Explicit file list only (no whole-library scan)

### Implementation notes (Phase C2, 2026-07-18)

- New palette actions: `border`, `expand`, `watermark` (image path + anchor/scale/opacity), `text` (string + font size/color/opacity/anchor)
- Host draw helpers in `t_image.rs`; text uses `ab_glyph` + OS fonts (Segoe UI/Arial/msyh on Windows, DejaVu/Noto on Linux, Arial/PingFang on macOS)
- Still optional later: collage-template insert as a batch action (C3)

---

## Cross-cutting

- **i18n:** en + zh from day one for new strings  
- **Local-first:** no required network  
- **Safety:** trash/overwrite rules consistent with existing file ops  
- **Testing:** unit tests for preset math (aspect, px@DPI); UI smoke for wizard steps  
- **Docs:** update this file + progress notes when a phase ships  

## D. Photo print layout / 冲印排版

Status: **shipped + refined** (2026-07-19).

### Shipped
- Built-in paper sizes (3R–8R, A4/A6) + photo-size packing templates including **A4** packs
- Fill-the-paper packing (光影-style): scale cells to sheet, preserve aspect; H/V bands + auto utilization
- Custom paper form dialog (inch/cm) and custom layout dialog
- **Export:** full plan-DPI sheet via host `export_print_layout`
- **Print (fast path):** same paper aspect, long edge ~1800px (`PRINT_MAX_EDGE`) → temp JPEG → prefer blob URL → hidden `.print-only` + `window.print()` (not host `print_file`; not full export DPI before dialog)
- Optional import of exported/printed sheet into current album (`import_file`)
- Host: parallel unique decode; **source downscale to largest cell on the target canvas**
- Session print-cache + ~200ms background warm; safe temp delete + 24h stale purge; shallow fingerprint (no deep-watch cache thrash)
- Freeze fix: never mutate pinia `printLayout` from `computed`

### DPI note
- UI DPI = **export** pixel density (inch × DPI), not OS printer driver DPI and not the print-path canvas size.
- System print dialog still controls printer quality/paper.

### Still optional later
- Richer native printer/tray API (v1 keeps OS dialog only; G13 clarifies UX)
- Batch wizard collage-as-action (G1 cancelled); batch re-import shipped as G2

### Shipped polish (2026-07-20)
- Magazine free-rect packing (`magazine` / auto) — G11
- Export DPI under advanced export options — G12
- System-print hint (no host print_file) — G13

### Touchpoints
- `src-vite/src/common/printLayout.ts`, `PrintLayoutDialog.vue`, `AddCustomPaperDialog.vue`
- Host: `export_print_layout`, `temp_file_path`, `delete_temp_file`, `cleanup_stale_temp_files`
- Pattern: `patterns/change-print-layout.md`

## Out of scope (for now)

- Full 光影魔术手 parity (all filters, beauty, print driver, etc.)  
- Cloud sync / online template store  
- Plugin-hosted batch (may wrap later; v1 is built-in)  
- Publishing v1.1.0 draft release (independent decision)  

## Tracking

- Product plan: this document  
- Session router “future work” points here  
- Progress: `docs/guide/picaipic-progress.md`, `docs/guide/目前的开发情况.md`  
