# Built-in tools roadmap (crop presets · collage · batch)

Status: **planned** (product backlog; not implemented).  
Recorded: 2026-07-18  
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

- [ ] Crop opens sub-menu with ratios + built-in photo sizes  
- [ ] Selecting a preset constrains crop aspect correctly (portrait/landscape)  
- [ ] Custom favorite ratio can be added and removed  
- [ ] Built-in list matches catalog above (labels + aspect + declared px/DPI)  
- [ ] Existing free crop / apply / cancel still work  

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

**B2**

- More templates; horizontal/vertical 拼接
- Cell fill modes: cover / contain
- Optional border radius / stroke

**B3**

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

- [ ] 拼图 entry visible and opens mode chooser  
- [ ] Template 2/4/9 works with multi-select or picker  
- [ ] Export produces a single image file  
- [ ] Cancel discards work without writing library originals  

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
| 调整尺寸 | C1 |
| 裁剪（含规格预设） | C1 (simple) / C2 (full presets) |
| 添加水印 | C2 |
| 添加文字 | C2 |
| 添加边框 | C2 |
| 扩边 | C2 |
| 一键动作（效果链） | C3 |
| 插入模板 | C3 |

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

- [ ] Wizard opens; can add images/folder and clear list  
- [ ] At least: resize and/or simple crop action runnable in order  
- [ ] Output to “另存为” folder with jpg/png and quality  
- [ ] Progress + failure summary; cancel stops further files  
- [ ] Default path does not silently overwrite originals  

---

## Cross-cutting

- **i18n:** en + zh from day one for new strings  
- **Local-first:** no required network  
- **Safety:** trash/overwrite rules consistent with existing file ops  
- **Testing:** unit tests for preset math (aspect, px@DPI); UI smoke for wizard steps  
- **Docs:** update this file + progress notes when a phase ships  

## Out of scope (for now)

- Full 光影魔术手 parity (all filters, beauty, print driver, etc.)  
- Cloud sync / online template store  
- Plugin-hosted batch (may wrap later; v1 is built-in)  
- Publishing v1.1.0 draft release (independent decision)  

## Tracking

- Product plan: this document  
- Session router “future work” points here  
- Progress: `docs/guide/picaipic-progress.md`, `docs/guide/目前的开发情况.md`  
