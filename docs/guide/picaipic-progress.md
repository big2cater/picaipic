# PicAiPic Progress

Updated: 2026-07-27

## Status board (2026-07-27)

| Track | Status |
|-------|--------|
| Theme menu | **Default / Retro / CMYK / Black hole / Cyberpunk** (`THEME_ID` 0–4) — branch `feat/black-hole-idle-theme` / PR #3 |
| Black-hole cosmos + PhotoVortex | **Shipped** — maximize + **6s** idle; UV lens; RO size cache; theme-gated mount — `change-black-hole-theme.md` |
| Cyberpunk night-city ambient | **Shipped** — `CyberpunkBackground` (sprite-baked rain/particles/kana) — `change-cyberpunk-theme.md` |
| Cyberpunk idle photo glitch | **Shipped** — `PhotoGlitchLayer` WebGL1; **mediump-safe** hash + mod time; intensity `>0` — `change-cyberpunk-theme.md` |
| FX correctness/perf follow-up | **Shipped** — `130b33a` (hash, sprites, reflow, theme-gate GL); `1aa0a59` (resize seedField false; capture rAF cancel) |
| Audit harden pack (2026-07-26) | **Shipped** — import_url limits, copy orphan cleanup, restore atomic write, path_inside, embed warm — `docs/review/code-review-2026-07-26.md` |
| SQLite audit follow-up: S1/S6 | **Shipped / paused at a safe boundary** — `AFile::new` metadata helpers (header/EXIF/orientation/identity/descriptions/capture/RAW merge), temporary SQLite CRUD and binary EXIF fixtures; `cargo check` + full Rust test **107 passed / 3 ignored** — `docs/review/code-review-2026-07-26-supplement.md` |
| Large-library scan profiling | **100k worker measured** — AI on: **9,548.281s** total; traversal **3,233.415s**, drain **6,313.095s**; single-permit embedding dominates. Task timers fixed to exclude semaphore waits; per-file embed logs opt-in |
| Scan preview stuck at N-2 | **Fixed** — always advance `processed`; thumb/embed timeouts |
| RAW grid thumbs | **Embedded JPEG first**, demosaic fallback (`t_libraw`) |
| Built-in A/B/C1/C2 + print layout | Shipped |
| Photo frame / 相框 G-Frame-1 + G2 (blur float/sink + logo) | **Shipped** (2026-07-22) |
| Batch import to library (G2) | Shipped |
| Lap 0.3 UX (prompt/badges/bg/search) | Shipped |
| Multi-key trust + local revoke (G6) | Shipped |
| Sandbox Phase 0–2 | Default on |
| Sandbox Phase 3–5 | Opt-in spikes (default off) |
| G10 FileInfo Live hover | Shipped |
| G11 magazine pack / G12 export DPI / G13 system-print UX | Shipped |
| Correctness pack (faces/storage/collage/batch/dedup/meta API) | Shipped |
| Calendar SIDEBAR index + local-day query | Shipped |
| G1 / G7 / G8 / G9 | **Not doing** (owner) |
| Traditional color match / 追色 + style 33³ `.cube` | **Shipped** (host, 2026-07-20/21) |
| Photo style / 照片格调 + LUT library | **Shipped** (host, 2026-07-21; UI merged + geometry-aware preview) |
| Multi-image compare library entry | **Shipped** (2026-07-21) |
| ImageViewer Edit toolbar (no plugin toolbar) | **Shipped** (2026-07-21) |
| Batch capture-time watermark/text | **Shipped** (2026-07-21) |
| Publish v1.1.0 draft | Owner decision |
| Large-library face cluster ANN / blocked KNN | **P0–P2 + HNSW shipped**; P3 deferred — `docs/guide/face-cluster-ann-plan.md` |
| Toolbar LIVE file-type filter (bit 8) | **Shipped** (2026-07-22) |
| AI search: honor thr + stop-bleed + **histogram-calibrated floors** | **Shipped** (2026-07-22→23) — floors **[0.28, 0.24, 0.20, 0.16]** |
| CLIP embed quality + scan decouple + free-text template | **Shipped** (2026-07-23) |
| Search ranking: **abs primary + thr_cap Top-K; smart tags follow slider** | **Shipped** (2026-07-24) — `change-ai-search-filters.md` |
| Similar-from-file: **image→image floors/caps + exclude self** | **Shipped** (2026-07-24) — Low ≠ VH on Find similar |
| Track C bilingual int8 **product default (option C)** | **Shipped** (2026-07-24) — bundled EN+CN text; no model switch |
| Embed matrix + rayon + optional HNSW ANN | **Shipped** (2026-07-24) — `change-library-perf.md` |
| Dedup Similar (dHash) + exact blake3 | **Shipped** (2026-07-24) — schema v9; mode-aware |
| Image-search model Track A stop-bleed | **Shipped** |
| Image-search model Track B0 CLIP B/16 default | **Abandoned** (2026-07-23) |
| Image-search model Track B SigLIP2 Phase 0 | **Probe done**; real-album **no clear quality win** → **no product UI on this pack** — `siglip2-phase0-probe.md` |
| Settings cross-window hydrate gate + mediaBadges loop fix | **Shipped** (2026-07-22) |
| App icon from `favicon1.ico` (neural-cat) + package -Clean | **Shipped** (2026-07-22) |
	| Smart Albums UX pack (size ops, pickers, sort, local-day dates, empty load) | **Shipped** (2026-07-24) — `change-smart-albums.md` |
	| Smart tags 6-bucket + default High thr + thr re-run | **Shipped** (2026-07-24) — people/pets prompts owner-tuned — `change-smart-tags.md` |
	
Chinese status: `docs/guide/目前的开发情况.md`. Session router: `.mex/ROUTER.md`. Patterns: `change-black-hole-theme.md`, `change-cyberpunk-theme.md`, `fix-library-scan-selection.md`.
	
	## 2026-07-24 Smart Albums UX + smart tags product set
	
	### Smart Albums (规则智能相册)
	- **Size operators:** backend supports `is_not` / `empty` / `not_empty` (was unsupported → query Err → silent empty list). Size values always treated as **MB** (fractional OK; no ≥100000-as-bytes trap).
	- **Pickers:** person / camera / lens dropdowns (`getPersons` / `getCameraInfo` / `getLensInfo`); camera/lens value `Make||Model`. Empty libraries show guidance text.
	- **Errors:** `getSmartFileList` toasts `album.smart_edit.query_error` instead of showing 0 files.
	- **Sort:** SmartAlbumEdit exposes sort type/order (same indices as toolbar); persisted per album.
	- **Dates:** `before` / `after` / `between` use SQLite local calendar-day compare; frontend local midnight + local Y-M-D (not UTC `toISOString`). Default date op `in_last`.
	- **Empty SIDEBAR.SMART:** no album selected → `showEmptyContent` (no infinite loading spinner). Stale smartAlbum id cleared.
	- Pattern: `patterns/change-smart-albums.md`.
	
	### Smart tags (CLIP 智能标签)
	- **Product set (6):** people / pets / landscape / architecture / plants / birds. Dropped family/portraits/kids/land_animals/food/sports/night/insects (use free-text).
	- **Prompts (owner-tuned on ~103-image export):**
	  - people → `a photo of people` (short plural; recovers mall queues / rear-view groups; multi-`or` and pure portrait prompts failed owner checks)
	  - pets → `a photo of a dog or cat or rabbit or hamster or bird pet`
	  - landscape / architecture / plants / birds → short single-subject CLIP phrases
	- **Labels:** zh 人物 / 宠物 / 风景 / 建筑 / 植物 / 鸟类.
	- **Default thr:** `configStore` `imageSearch.thresholdIndex = 1` (High / 0.24) for new installs; saved user settings not auto-migrated.
	- **Thr re-run:** Settings thr/limit change re-queries smart tags via `getImageSearchFileList` directly; numeric coerce on index/limit; clear stuck smart/collection `activePane` when needed.
	- **Not a detector:** named people → face index. Log host stdout `search_similar mode=text …` under `cargo tauri dev`.
	- Owner VH evidence: pets can be tight (e.g. 5 hits); people score band is wide on personal albums — High may still return many “people-ish” images; VH tighter.
	- Pattern: `patterns/change-smart-tags.md`.
	
	## 2026-07-24 Track C product C + similar-image ranking + search perf

### Product default (Track C option C)
- **Vision:** still bundled **CLIP ViT-B/32** (`vision_model.onnx`) — library embeds **not** rebuilt.
- **Text:** installer ships **CLIP-aligned bilingual int8** as `resources/models/text_model.onnx` + matching `tokenizer.json` (sha256 text `50357311…`). EN-only CLIP text **removed from bundle** (backup: `scripts/.probe-models/bundled-clip-en-text-backup/`).
- **Settings:** no model dropdown; label **中英内置** / bundled bilingual; optional **重下云端包** (observation). Cloud self-host: `big2cater/picaipic-binaries` tag `models`. EN CLIP download URLs kept **commented** in `download_models.*` for observation.
- **Runtime:** `encode_text` prefers `sentence_embedding` (512-d); smoke EN+ZH; max_len 128 for bilingual. Optional app-data Multilingual re-download may override via settings during observation.
- **No** SigLIP2 / multi-model vision UI on the current pack.
- Guide: `docs/guide/altclip-phase0-probe.md`. Pattern: `change-image-search-model.md`. Package: `build-exe.bat` → `package_windows.ps1 -Clean` (bundles `resources/models/*`).

### Search ranking (text + image-image)
**Text bug fixed (earlier 2026-07-24):** always `max(abs, top1*0.85)` hid Low/Med/High on strong text queries.

**Similar-from-file bug fixed (same day):** image→image cosine is ~0.55–0.95, so text floors (0.14–0.24) never differentiated Low vs Very High; default `limit=50` + self-hit≈1.0 made results look identical.

**Current contract** (`AFile::search_similar_images`):
1. Collect candidates with junk floor `score >= 0.16`.
2. Sort cosine desc.
3. **Mode split:**
   - **Text / smart tags:** `absolute_floor = max(0.16, settings_thr * 0.85)`; thr_cap VH30/H40/M50/L200; rel `top1*0.85` empty-fallback only.
   - **Similar-from-file (`file_id`, empty text):** floors VH **0.88** / H **0.82** / M **0.74** / L **0.62**; thr_cap **12 / 24 / 40 / 100**; exclude query id; rel `top1*0.92` empty-fallback.
4. User `limit` is a **hard cap** for all tiers: `top_k = min(limit_or_thr_cap, thr_cap, 200)`. Soft max 200.
5. Never force `0.25` when `search_text` is set.
6. Changing Settings `thresholdIndex` / `limit` re-runs active similar temp view, search sidebar, and smart tags (`Content.vue` watch).
7. Log: `mode=text|image settings_thr floor= rel_floor= floor_mode= thr_cap= top_k= above_floor= returned=`.

### Thresholds (histogram-calibrated UI ladder)
| Layer | Values |
|-------|--------|
| Settings VH/H/M/L (UI index) | **0.28 / 0.24 / 0.20 / 0.16** |
| Text / smart-tag host floors | `max(0.16, thr*0.85)` → ~0.14–0.24 band |
| Similar-from-file host floors | **0.88 / 0.82 / 0.74 / 0.62** (same UI index) |
| Smart tags | **Same `thresholdIndex`** (text path; no hard-coded thr) |
| Historical stop-bleed | was 0.30/0.26/0.22/0.18 — superseded |
| Pre-stop-bleed | 0.40/0.34/0.28/0.22 — too high for CLIP text band |

Evidence (text, owner logs): strong bird/landscape max ≈ 0.25–0.28; `>0.28` rare; family with people max ≈ 0.255.  
Re-calibrate text: `scripts/calibrate_search_thresholds.py`. Image floors may need owner retune from `mode=image` log lines.

### Free-text + smart tags
	- Short bare EN labels → `a photo of a/an {label}` in `AiEngine::normalize_clip_text_query`.
	- Smart-tag product set (2026-07-24): **people / pets / landscape / architecture / plants / birds** — see section above + `change-smart-tags.md`.
	- Smart tags call `getImageSearchFileList` **without** `thresholdOverride` (follow settings slider; default High for new installs).
	- Log pitfall: `preview=` is 40-char **display** only — use `text_chars` / `templated` / `enc_preview` / `floor_mode` / `thr_cap` / `mode=`.

### Embed source ladder (quality + scan)
1. Thumb permit released **before** CLIP embed; folder-sync embed fire-and-forget.
2. Decode **outside** AiEngine mutex (`load_image_for_clip_embed` → `encode_image_from_dynamic`).
3. JPEG: libjpeg-turbo **scaled** to `EMBED_SOURCE_MAX_EDGE` (1024).
4. RAW (type 3): LibRaw preview @ 1024 (`used=raw_preview`).
5. Other: open + longest-edge cap; UI thumbnail last resort.
6. Embed semaphore **1** (honest single ONNX session).

### Large-library search perf + dedup (same day)
- Process-local **embed matrix** (exact cosine); SQL BLOB fallback; invalidate on write/clear/library switch.
- Rayon scoring for large N; optional background **HNSW** (`instant-distance`) with exact rerank (N≥8000).
- Dedup: exact blake3 + **Similar dHash** (schema v9); scoped rebuild; mode-aware keep/delete.

### Key paths
- `t_sqlite.rs` ranking (`image_image_absolute_floor` / `image_image_top_k`) + matrix/ANN + `generate_embedding`
- `t_ai.rs` bilingual default text, `sentence_embedding`, free-text template, download self-host
- `t_image.rs` / `t_utils.rs` embed ladder
- `t_dedup.rs` / `t_migration.rs` Similar dHash
- `Content.vue` smart-tag + threshold re-run watch
- `Settings.vue` / `configStore.js` / `smartTags.ts` / `download_models.*`

### Track B0 / B / C (summary)
| Track | Outcome |
|-------|---------|
| B0 CLIP B/16 default | **Abandoned** after owner trial ≈ B/32 |
| B SigLIP2 Phase 0 | Python+Rust OK on **quantized**; int8 Rust fail; offline compare ran; owner: CLIP weak insects/plants; SigLIP2 small-bird→insect — **no ship UI** |
| **C bilingual text (no reindex)** | **Product default shipped (option C)** — canavar CLIP-B/32 multilingual int8 bundled; no Settings switch |

Scripts: `compare_clip_vs_siglip2.py`, `probe_siglip2_onnx.py`, `probe_siglip2_ort`, `compare_clip_en_vs_altclip_cn.py`, `calibrate_search_thresholds.py`, `download_models.ps1`/`.sh`.  
Patterns: `change-ai-search-filters.md`, `change-smart-tags.md`, `change-image-search-model.md`, `change-library-perf.md`.  
Guides: `docs/guide/siglip2-phase0-probe.md`, `docs/guide/altclip-phase0-probe.md`.





## 2026-07-22 Photo frame / 相框 G-Frame-1 + G2

- Multi-select entry **Photo frame** → classic white/black bars + **float-blur** / **sink-blur**.
- Host: EXIF/LibRaw summary; classic solid bar; float/sink via `make_cover_blur_bg` + `make_soft_shadow` + translucent text strip; optional local logo (`load_frame_logo` / `place_frame_logo`).
- IPC: `photo_frame_preview` / `export_photo_frame` / `cancel_photo_frame_export`; event `photo-frame-progress`.
- Frontend: `photoFrameTemplates.ts`, `PhotoFrameDialog.vue` (blur/shadow/logo controls); optional import into open album.
- Save-as only; originals untouched; fully local.
- Follow-up G3/G4: top/side magazine, batch action `photoFrame`, logo library UI.
- Pattern: `.mex/patterns/change-photo-frame.md`. Roadmap Phase G. Chinese: `docs/guide/目前的开发情况.md`.

## 2026-07-21 Editor preview / compare entry / viewer edit / capture-time stamp

### Photo-style / adjust preview
- Unified presets + manual; layered CSS/host preview; combined color-match+style.
- `PreviewGeometry` (flip/rotate/crop + full size) applied before grade; crop decode budget scales up to 8192.
- Caches: layout decode LRU, JPEG result LRU, client fingerprint short-circuit; viewport `previewMaxEdge`.
- Histogram samples host bake when present; compare panes crop-aligned.

### Multi-image compare entry
- Single-file menu: **Compare with next…**
- Multi-select: **Compare selected…** (first two) → `forceSplit` 2-up + sync.
- Viewer toolbar still cycles 1/2/4 panes.

### ImageViewer toolbar
- Removed `image.toolbar` plugin buttons (e.g. SA-LUT on the bar).
- Added built-in **Edit image…** opening the host ImageEditor; plugins remain on context menu.

### Batch watermark / text
- Optional EXIF capture-time stamp (`includeCaptureTime`, `captureTimeFormat`: datetime|date|time).
- Text prefix optional; image watermark can also stamp time after the mark image.
- Pattern: `patterns/change-batch-process.md`.

## 2026-07-21 Photo style / 照片格调 + LUT library (UI merge)

PhotonCamera-inspired **local** host tools (not a GLES port; not SA-LUT plugin):

- **LUT library:** import/rename/favorite/delete `.cube` under `app_data/luts/` (`t_lut.rs`, `LutLibraryDialog.vue`).
- **Unified recipes:** built-in + custom = CSS base params + host effects (highlights/shadows/fade/vignette/grain) + optional library LUT + intensity.
- **ImageEditor IA:** no separate photo-style panel. Recipes live in **Presets** strip + **Manual** (effects/LUT). Save-as custom appears in presets; custom order is stable (config array order).
- **Preview layering:** CSS filter for base-only changes (instant); host `apply_photo_style_preview` only when host-only fields are active (maxEdge 1200).
- **Apply order (host save/batch):** base → LUT → effects.
- **Surfaces:** ImageEditor Adjust; `edit_image.photoStyle`; batch `photoStyle`.
- **Config:** `imageEditor.photoStyles`, `activePhotoStyleId` (= selected preset id), expanded `custom` recipe fields.
- Pattern: `.mex/patterns/change-photo-style.md`
- Product: `docs/guide/builtin-tools-roadmap.md` Phase F
- Verify: `pnpm --dir src-vite build`; optional `cargo test --manifest-path src-tauri/Cargo.toml lut`.
- **Perf/correctness follow-up:** layout decode LRU for interactive previews; combined color-match+style preview; host-preview keeps CSS blur; debounced named-custom persist.

### PhotonCamera AI analysis note (reference only)
- Photon **AI color simulation / LUT creator analysis** uses a **cloud OpenAI-compatible vision API** (`OpenAIApiClient`, `/chat/completions`; user Key/BaseURL/Model or built-in proxy).
- Photon also has **local** models for depth (MiDaS/DepthAnything) and detection (YOLOX) — separate from AI recolor analysis.
- PicAiPic’s photo-style/LUT/color-match slice remains **fully local**; no Photon-style cloud recolor path in this cut.

## 2026-07-21 Traditional color match / 追色 + single-image style LUT

Host-built-in traditional tools (not SA-LUT plugin):

- **Apply match:** global Lab median/percentile transfer from a reference image onto a target (`t_color_match.rs`).
  - ImageEditor **Adjust** tab → **Color Match** panel (pick reference, intensity / tone lock / highlight-shadow protect / auto WB, debounced `color_match_preview`).
  - Save via `edit_image` optional `colorMatch` (geometry → match → CSS-style adjustments).
  - Batch action `colorMatch` with shared `referenceFilePath`.
- **Export `.cube`:** **single-image style bake** (default 33³) via `export_color_match_lut` / `build_style_cube_from_image`.
  - Prefer selected reference image; else current photo.
  - Not a dual-image source×reference match map; not cancelled G7 SA-LUT `export-lut`.
- Pattern: `.mex/patterns/change-color-match.md`
- Product: `docs/guide/builtin-tools-roadmap.md` Phase E
- Verify: `cargo test --manifest-path src-tauri/Cargo.toml color_match`; frontend build.

## 2026-07-22 Color-match perf + batch cancel hardening (code audit)

- **Color match:** stats downsample both target and reference to max-edge 1024; single full-res f32 plane + one pixel pass (WB → Lab blend → protect → tone); one sort per Lab channel for median/p16/p84; Lab a/b clamp restored to full 0..255 (was 72..186 crushing saturated colors); style LUT size outside 17–65 returns error (no silent clamp).
- **Batch:** write `{dest}.picaipic-batch.tmp` then rename; cancel/error removes temp only (overwrite-safe); progress `current` clamped to `total`.
- Tests: `same_image_stays_close`, `warm_reference_pulls_mean_warmer`, `highlight_protection_*`, `style_cube_*`, `saturated_reference_keeps_chroma`, `style_cube_rejects_out_of_range_size` — all pass.
- Patterns: `change-color-match.md`, `change-batch-process.md`.

## 2026-07-22 Photo frame bug pack

- Info bar: left/right column max-width + min center gap; `fit_frame_text` shrink then `…` truncate (no L/R overlap).
- Export: `{dest}.picaipic-batch.tmp` → rename; cancel scrubs temps only; worker limit ≤2; source long edge >8192 downscaled before frame; single `to_rgba8` in apply.
- Datetime: EXIF + ISO-8601 (`T`, fraction, Z/offset) → `YYYY-MM-DD HH:MM:SS` (`format_frame_date_time` tests).
- Pattern: `change-photo-frame.md`.

## 2026-07-22 Collage / cluster / watermark audit pack

- **Collage:** `save_collage_image` writes `{dest}.picaipic-collage.tmp` then rename (no mid-encode clobber); strip templates cap 12→48.
- **Decode:** `load_image_for_layout` uses libjpeg-turbo `decode_rgb8_scaled` for JPEG (shared by collage + photo-frame export).
- **Cluster:** `insert_top_k` linear insert (no per-pair full sort); cancel returns `Err("cancelled…")` so UI is not “success 0”; cosine length `debug_assert`. Still **O(n²)** pairs — next product plan below.
- **Watermark/batch text:** `read_capture_time_label` uses `format_frame_date_time` (ISO-safe); watermark source + capture-time labels cached per batch worker.

## 2026-07-22 Large-library face clustering (P0–P2 + HNSW)

- Plan: **`docs/guide/face-cluster-ann-plan.md`** (status in progress; P3 deferred).
- **P0:** `[cluster]` phase logs (n, assigned/unassigned, graph/whisper/assign/total ms); synthetic exact-graph bench tests.
- **P1:** `build_knn_graph_exact` + `build_knn_graph_blocked` + adaptive; `CLUSTER_N_EXACT=8000`, `CLUSTER_BLOCK_SIZE=2048`.
- **P2:** `face.clusterMode` = `auto` \| `exact` \| `fast` (Settings + IPC); edge parity report tests.
- **ANN:** pure-Rust **`instant-distance` 0.6.1** HNSW for auto large-n / fast; non-cancel failure → blocked exact fallback; knobs `CLUSTER_ANN_EF_*`.
- **P3 disk/incremental ANN:** owner deferred (low ROI; inference dominates index time; crate is rebuild-from-points). Prefer measure + optional embedding binary cache later.
- Runbook: `.mex/patterns/change-face-index.md`. Decision: `.mex/context/decisions.md`.

## 2026-07-22 LIVE toolbar filter + smart tags + AI search threshold

- File-type bitmask adds **8 = LIVE/Motion still** (`live_photo_type` IN 1,3,4); companion MOV type 2 still list-excluded. UI All/Image/RAW/Video/LIVE; smart album type LIVE.
- Smart-tag prompts refined; `SMART_TAG_SEARCH_THRESHOLD = 0.28`.
- **`search_similar_images` honors `params.threshold`** (no force 0.25 when `search_text` set). Settings floors retuned for CLIP: **0.40 / 0.34 / 0.28 / 0.22**.
- Patterns: `change-ai-search-filters.md`, `change-live-photo.md`.

## 2026-07-22 Settings sync + mediaBadges hang + api rethrow

- **mediaBadges infinite loop:** Settings deep-watch + dual-window `main.js` listeners + replace-every-time setter → hang. Fix: `setGridMediaBadges` equal-noop; no set inside computed getters.
- **Hydrate gate:** `settingsHydrating` + `emitSettings` so opening Settings does not fan out every setting to main.
- Mutating IPC rethrow: `importFile` / `importUrl` / `importFileBytes` / `updateFileInfo` (plus existing rating/favorite/rotate/batch).
- Pattern: `.mex/patterns/settings-cross-window-sync.md`.

## 2026-07-22 App icons from favicon1.ico + package script

- **Canonical app mark:** repo-root **`favicon1.ico`** (neural-cat PicAiPic). Frame wordmark stays `resources/branding/default-frame-logo.png` from `logo-pic.png` — never overwrite app icons with frame logo.
- Regenerated `src-tauri/icons/*` via `pnpm … tauri icon` from favicon1 master; frontend `assets/images/icon.png` + `docs/public/icon.png` synced.
- **`build.rs`:** `rerun-if-changed` on icon assets so ICO content changes force re-link.
- **`scripts/regenerate_app_icons.ps1`**; **`package_windows.ps1`** always regenerates icons from `favicon1.ico`; **`-Clean`** runs `cargo clean -p PicAiPic`.
- **`build-exe.bat`** always passes **`-Clean`**.
- Pattern: `change-photo-frame.md` (app icon vs frame logo), `release-build.md` app-icons section.

## 2026-07-22 Photo frame EXIF single-open

- `read_frame_exif_summary`: one 512KB head read for JPEG/common; kamadak + little_exif share the buffer; RAW → LibRaw only (no redundant head scan); datetime from same buffer; EXIF summary still cached by path+mtime for preview toggles.

## 2026-07-22 Photo frame custom presets + default logo + app icon

- **Presets:** `config.photoFrame.presets[]` (id/name/updatedAt/options); dialog save/apply/delete; remembers last template + preset + import preference.
- **Default logo:** frontend `showLogo: true`, empty `logoPath`; host `resolve_frame_logo_path` falls back to `resources/branding/default-frame-logo.png` (from repo `logo-pic.png`); packaged via `tauri.conf.json` resources `branding/*`.
- **Windows app icon (corrected same day):** do **not** use `logo-pic.png` for chrome. Use **`favicon1.ico`** → `scripts/regenerate_app_icons.ps1` / package `-Clean`. Bundle still `icons/icon.png` + `icons/icon.ico`.

## 2026-07-22 Audit follow-ups (pool / face / plugin / SQL)

- **SQLite pool:** path keys normalized (`/` + lowercase on Windows + canonicalize when possible); idle pool capped at `MAX_CONN_POOL = 8` (excess Drop closes conn).
- **`update_column`:** allow-lists for albums / afolders / afiles (identifier injection defense-in-depth; values already parameterized).
- **Face index:** cancel still reports discarded jobs so progress can reach 100%; unfinished feeder path forces `current = total` on cancel; detection/embedding `unwrap` → `ok_or_else` Err. Workers remain per-thread engines (ort `Session` not Sync; count already 2–4).
- **Plugin setup log:** `AiPluginSetupJob::push_log` ring-caps to 2000 lines.

## 2026-07-20 Correctness pack + calendar fix

### High / medium severity product bugs

- **Incremental face clustering:** do not wipe persons on re-index; seed/freeze existing `person_id`; only unassigned faces join or create `Person N` (`t_cluster.rs`, `Face::get_all_for_clustering` + `next_auto_person_number`).
- **SCRFD det_500m:** comments/IO guards match bundled ONNX (9 outputs, score/box vs anchor count).
- **DB storage migrate:** `clear_conn_pool` after `change_db_storage_dir` / `reset_db_storage_dir`.
- **Collage:** host errors when source count exceeds cell count.
- **Batch dialog:** hue min `-180`; remove `&& false` dead disable.
- **Dedup:** skip rows without `file.id` instead of panic.
- **Rename/move:** refresh `modified_at` + `inode` after successful path change.
- **api.js mutating metadata:** rating/favorite/rotate/batch rethrow; optimistic UI rollback in Content/ImageViewer.

### Calendar empty list (user-reported)

- **Root cause:** Smart Albums inserted at sidebar index **1** shifted later panels; `Content.vue` still treated **calendar as 3** (now Search). Calendar absolute index is **4**.
- **Fix:** `SIDEBAR` map in `constants.ts`; Content/Home routing and search shortcut updated.
- **Also:** day-count filters align with content queries; local-day `strftime` range filter; month/day numbers on dots; numeric QueryParams coercion.
- Runbook: `.mex/patterns/change-calendar.md`.
- Packaging does **not** clear user DB/WebView cache; not required for this fix.

### Verification

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Manual: calendar click filled day/month → files appear

## 2026-07-19 Lap 0.3 UX pack (prompt import · media badges · viewer bg · search filters)

Aligned further with upstream lap v0.3.0 browsing/metadata UX while keeping PicAiPic plugin/built-in-tool differentiation.

### AI PNG/JPEG prompt → empty comments

- Scan-time import of generation prompts into **empty** `afiles.comments` only (never overwrite user notes).
- **PNG**: `tEXt` / `iTXt` / `zTXt` — Automatic1111 `parameters`, NovelAI/Invoke JSON, ComfyUI workflow text.
- **JPEG**: EXIF `UserComment` (charset-aware), `COM` markers, heuristic `ImageDescription` fallback.
- Default **on** (`importAiPromptsToComments` + Rust `AtomicBool`); Settings → Library → Metadata import.
- Applies on **new insert** and **changed-file rescan** only (no full-library empty-comment backfill).
- Module: `src-tauri/src/t_ai_prompt.rs` (+ `flate2` for zTXt); hook in `AFile::new` / `update_file_info`.
- Unit tests: `cargo test --manifest-path src-tauri/Cargo.toml t_ai_prompt`.
- Runbook: `.mex/patterns/change-ai-prompt-import.md`.

### Thumbnail media-info badges

- Settings → View → per-flag overlays: format, ISO, shutter, aperture, focal length, exposure.
- Default **all off**; max **4** badges per thumb; bottom-left layout (status badges stay top-left).
- State: `config.settings.grid.mediaBadges`; render: `Thumbnail.vue`.
- Runbook: `.mex/patterns/change-media-badges.md`.

### Viewer background modes

- Canvas modes: theme / black / white / gray / checkerboard.
- Shortcut **B** cycles; toolbar palette button; Settings → Viewer select.
- State: `mediaViewer.backgroundMode`; helpers in `utils.ts`; checker CSS in `app.css`.
- Applies to standalone ImageViewer and in-app quick view / filmstrip preview.
- Runbook: `.mex/patterns/change-viewer-background.md`.

### AI search file-type filter + result grouping

- `ImageSearchParams.search_file_type` (same bitmask as library: image/video/raw).
- Vector search SQL filters before cosine scoring; filename search already used query mask.
- Toolbar type filter enabled in search sidebar and similar-from-file temp view; changing filter re-runs active search.
- Grid section headers: Visual matches / Similar images / Filename matches (`GridView` `sectionLabel`).
- Runbook: `.mex/patterns/change-ai-search-filters.md`.

### Verification (this pack)

- `cargo check` / `cargo test … t_ai_prompt` / `pnpm --dir src-vite build` passed for the relevant slices.
- Docs/MEX: `docs/guide/目前的开发情况.md`, `.mex/ROUTER.md`, patterns INDEX, decision log.

### Still open after this pack

- Publish v1.1.0 draft / sandbox deeper enforcement (netns/WFP/seccomp) — optional.

### Explicitly not doing (2026-07-20)

- **G1** collage-in-batch, **G7** SA-LUT export-lut (host traditional single-image style `.cube` is separate and shipped), **G8** face GPU EP, **G9** whole-library empty-comment backfill.

### G10–G13 polish (2026-07-20)

- **G10:** FileInfo preview hover (~280ms) / long-press (400ms) plays Live/Motion motion; labels i18n.
- **G11:** `packMagazine` free-rect strategy; custom layout + auto scoring includes magazine.
- **G12:** DPI moved under Export options; “Export DPI” + hints (not OS printer DPI).
- **G13:** Print footer explains system dialog for printer/tray; still `window.print`, no host device picker.

## 2026-07-20 G2 · G6 · sandbox scaffold

### G2 — Batch outputs → optional library import (MVP)
- Host `BatchProcessResult.outputPaths` lists successful write paths.
- Wizard checkbox `batchProcess.importToLibrary` (default off); saveAs copies into current album via `importFile`; overwrite refreshes `updateFileInfo` only.
- Pattern: `.mex/patterns/change-batch-process.md`.

### G6 — Signing multi-key + local revoke
- Registry: per-publisher `keys[]` (`active|retired`) + top-level `revokedKeys`.
- `trust_publisher` adds keys; `revoke_publisher_key` / `list_revoked_keys`; Settings shows keys + revoke.
- Unit tests: multi-key accept, revoked reject, legacy normalize, NeedsTrust for second key.
- Docs: open Q3 resolved in `docs/ai-plugin-security-hardening.md`.

### Sandbox Phase 3–5
- **Phase 3 opt-in spike:** `PICAIPIC_ENABLE_PLUGIN_NETWORK_SANDBOX=1` + no runtime network grant → Windows `netsh` outbound program block (soft-fail → policy_only) + `PICAIPIC_PLUGIN_NETWORK_POLICY`; rule dropped on stop.
- **Phase 4 opt-in spike:** `PICAIPIC_ENABLE_LINUX_LANDLOCK=1` → Landlock ABI probe + RO/RW path rules + child `pre_exec` restrict_self; soft-fail if kernel/ABI missing.
- **Phase 5 env hygiene (opt-in real):** `PICAIPIC_ENABLE_PLUGIN_ENV_HYGIENE=1` → `env_clear` + allowlist on plugin start/setup; default still inherits host env.
- `docs/ai-plugin-sandbox-roadmap.md` phase board updated.

## 2026-07-18 merge + v1.1.0 line

- Merged Live Photo polish (#1) and plugin sandbox Phase 0–2 + private runtime + model UX (#2) to `main`.
- Windows/Linux PR builds green after Actions artifact-quota hardening (`pr-build.yml` best-effort upload).
- App version aligned to **1.1.0** for the next signed multi-arch release draft.

## 2026-07-18 Phase A shipped: crop presets + photo sizes

- ImageEditor crop dropdown: free crop, common ratios (`1:1` / `3:2` / `4:3` / `16:9`), 12 built-in print/ID sizes, user custom ratios.
- Catalog module: `src-vite/src/common/photoSizePresets.ts`; config: `cropPresetId` + `customCropRatios` (persisted).
- Manage dialog (built-in table + delete custom) and add-ratio dialog; portrait/landscape still swaps aspect; photo presets prefill resize target px.
- Frontend build verified: `pnpm --dir src-vite build`.

## 2026-07-18 Phase B1–B3 shipped: collage / 拼图 complete for plan

- Multi-select right panel **拼图** → `CollageDialog`.
- B1: grids 2/4/9, gap/margin/background, JPEG/PNG save-as.
- B2: grids 3/6; strip H/V (≤12); fill cover/contain; cell radius + stroke.
- B3: free canvas — drag/resize/rotate/z-order/snap; host free items export.
- Free drafts: save/load/delete layouts in app config (`collage.freeDrafts`), path-matched restore.
- Host: `export_collage` (template/strip/free).

## 2026-07-18 Phase C1 shipped: batch wizard + composable actions

- Multi-select → **批处理** three-step wizard: files → ordered action chain → output.
- Action palette (built-in tools): resize, crop (ratios/photo/custom), rotate, flip, brightness/contrast/saturation/hue/blur, filters.
- One-click templates: save/load action chains in `config.batchProcess.templates`.
- Host `batch_process_images` + `cancel_batch_process`; progress event; save-as default; overwrite confirms.
## 2026-07-18 Phase C2 shipped: border / expand / watermark / text

- Batch palette adds border, canvas expand, image watermark, text overlay (anchor/opacity/margin).
- Host raster ops + `ab_glyph` system-font text; still local-only, save-as default.
- Optional later C3: insert collage template as a batch step.

## 2026-07-18 Photo print layout / 冲印排版

- Multi-select → **冲印排版**: paper templates (3R–8R/A4/A6), built-in packs (1R/2R/ID/passport/wallet mixes), custom layout builder.
- Paper size manager (inch/cm); custom papers/layouts in `config.printLayout`.
- Preview + export high-res sheet via `export_print_layout` (cover-fit cells, optional gray guides).


This document records the current implementation status for turning the existing
PicAiPic codebase into PicAiPic: a Windows x64 local album app with lightweight
built-in functions and independently registered AI plugins.

For detailed plugin runtime status, use:

- `docs/guide/plugin-runtime-status-2026-06-20.md`
- `docs/guide/ai-plugin-interface.md`
- `docs/guide/ai-plugin-development-roadmap.md`

## 2026-07-17 Live Photo / Motion Photo + reliability fixes

### Live Photo / Motion Photo (schema v6)

- Apple Live Photo: HEIC/JPEG still + companion MOV paired by EXIF ContentIdentifier
  (`Tag(Context::Tiff, 0x0011)`) and ffprobe `com.apple.quicktime.content.identifier`
  (dotted and underscored key variants). Stem-based same-folder fallback when UUID is missing.
- Google Motion Photo: single JPEG with embedded MP4; XMP parsed in `t_xmp.rs` (`quick-xml`);
  `content_id` stores `motion:<offset>:<length>`.
- HEIC-internal video (`live_photo_type=4`): detect/extract via libheif items/sequences with
  ffmpeg demux fallback on Windows/Linux (not macOS product target).
- DB columns on `afiles`: `content_id`, `paired_file_id`, `live_photo_type`
  (0=none, 1=Apple image, 2=Apple video, 3=Motion Photo, 4=HEIC-internal).
  Migration and open-time repair via `ensure_live_photo_columns`.
- Motion extract cache: `app_cache_dir()/motion_cache/` with source-keyed reuse, size-based
  prune, startup purge of legacy OS-temp extracts; cleared with `clear_video_cache`.
- Preview: MediaViewer 400ms long-press plays paired MOV or extracted motion video; LIVE badge
  on Thumbnail; FileInfo type labels; i18n en/zh.
- Export/convert (`export_live_photo` + `LivePhotoExportDialog`): still / video / pair /
  to_motion / to_pair / set_keyframe.
- Shared parser: `t_xmp::parse_motion_content_id` is the single source of truth for
  `motion:<offset>:<length>` (used by `t_cmds` and `t_live_photo`).
- **Polish (same day):**
  - Optional **confirmed** JPEG keyframe overwrite of the library still
    (`overwrite_original`; staged promote; Motion Photo keeps trailer; HEIC not supported).
  - Album-level `rescan_live_photo_metadata` repairs type `0`/`4` without full reindex, then
    re-pairs; AlbumList context menu + FileInfo export entry.
  - User guide: `docs/guide/live-photo.md`.

Runbook: `.mex/patterns/change-live-photo.md`.

### Reliability / consistency fixes

- `rename_file` / `rename_folder`: if disk rename succeeds and DB update fails, roll disk
  back to the old name (aligned with `move_file` rollback).
- `edit_album`: name-column errors propagate (no longer swallowed with `let _ =`).
- Dedup `get_files_by_sizes`: reuses precomputed suspicious sizes via chunked `IN` binds
  instead of a redundant full-table `GROUP BY`.
- MediaViewer: null-safe `props.file?.file_type` on floating toolbar; Live Photo playback
  guards when `props.file` is cleared mid long-press.
- `getBuildTime`: drop double semicolon; treat `0` with `!= null`.

### Verification (this pass)

- `cargo check --manifest-path src-tauri/Cargo.toml` passed.
- `pnpm --dir src-vite build` passed for the Live Photo polish UI pass.
- Full plugin-host regression not re-run in this pass; run
  `scripts/check_plugin_host.ps1` before release.

### Still open

- Broader HEIC sequence sample coverage; unusual sequence brands may fail ffmpeg demux.
- Sandbox **Phase 3–5 only** (Phase 0–2 done): network OS block, Linux Landlock/seccomp,
  env hygiene, optional cache ref/range zero-copy — `docs/ai-plugin-sandbox-roadmap.md`.
- Signing-key rotation/revocation design.
- Release-executable plugin regression after host/plugin changes.
- Manual SA-LUT/NAFNet staged-path checklist on release builds
  (`docs/ai-plugin-sandbox-phase0-verify.md`).

### 2026-07-17 shared→plugin-private confirmed switch

- Settings probe conflict block now offers **Use private runtime** when blocking
  conflicts exist and the profile still uses a non-private binding.
- User confirmation persists a synthetic `scope: "plugin"` binding via
  `switch_ai_plugin_profile_to_private_runtime`, clears that profile's probe
  cache, and marks the profile `needsVerify` without touching shared runtimes.
- After the switch, the user still re-runs Setup → Probe → Smoke for the private
  env under `plugin-runtimes/<plugin-id>/<envDir>`.

### 2026-07-17 model UX reinforcement

- `list_ai_plugins` now includes `modelFiles` presence under the managed model
  directory (`plugin-data/<id>/models`).
- Settings storage panel shows declared model files and offers:
  - **Open & validate** → `check_ai_plugin_model_files` + reveal model dir
  - **Import model files** → `import_ai_plugin_model_files` copies selected
    files by basename into declared model paths (containment-checked)
- External model-dir binding rows also open+validate the bound directory.

### 2026-07-17 sandbox Phase 0 (design + small correctness fixes)

- Roadmap: `docs/ai-plugin-sandbox-roadmap.md` (phased; no Settings sandbox panel).
- Input staging default is **platform-agnostic** (was Windows-gated).
- Staging copy failures **fail closed** (no silent fallthrough to original paths).
- Diagnostics: task queue message + `plugin-cache/.../inputs/staging-report.json`
  with staged file/byte counts and skip counters.
- Unit tests cover rewrite, fail-closed, and disabled messaging.
- Manual SA-LUT/NAFNet checklist: `docs/ai-plugin-sandbox-phase0-verify.md`.
- Network/Linux OS sandbox remain future opt-in research spikes.

### 2026-07-17 sandbox Phase 1 (host write allow-list)

- Single helper `plugin_writable_roots`: data/cache/outputs/plugin-runtimes/code
  + manifest shared runtimes + persisted model-dir bindings + call-site extras
  (task dir / task output).
- Used by invoke-time staging skip list and start-time optional deny-ACL
  exclusions (no Settings UI; no OS allow-list enforcement beyond existing ACL opt-in).
- Output adoption remains stricter: paths must stay under the **task output**
  directory only.

### 2026-07-17 sandbox Phase 2 (same-volume hardlink staging) — **done**

- Phase 2 **mainline complete**: `stage_one_file` tries hardlink first, then copy.
- Not full universal zero-copy: cross-volume still copies; cache ref/range not implemented.
- Staging report + task message include `hardlinkedFiles` / `copiedFiles`.
- Unit tests cover hardlink path on same temp volume; fail-closed still enforced.
- **Next sandbox work is Phase 3/4 research only** (do not ship as default).


## 2026-07-10 v1.0.0 stabilization pass

- Completed the active Lap → PicAiPic migration in UI text, updater/repository
  links, backup naming, dependency dialogs, help labels, CI artifact names,
  Chinese documentation, and VitePress configuration.
- Fixed cross-library thumbnail and preview isolation. Protocol URLs now select
  the encoded library's validated database and cache rather than relying on
  whichever library is current when an asynchronous request finishes.
- Enforced plugin host compatibility ranges (`minPicAiPicVersion`, optional
  `maxPicAiPicVersion`) alongside the v1 plugin API major gate.
- Standardized JavaScript tooling on pnpm, removed npm lockfiles, and aligned
  Cargo/Tauri/frontend/docs metadata at `1.0.0`.
- Split Home's heavy panels and Content into async chunks. The Home entry
  chunk dropped from about 527 KB to about 15 KB.
- Added `docs/guide/release-notes/v1.0.0.md` and moved the website's current
  release links to v1.0.0/current GitHub repository paths.

Verification: frontend production build, Rust format/check, seven non-ignored
Rust tests, `scripts/check_plugin_host.ps1`, and strict packaging for both
reference plugins all passed.

## Product Direction

PicAiPic is the main application body. The original source code's lightweight
album, browsing, editing, search, face, deduplication, and media features should
remain stable unless a later task explicitly changes them.

AI capabilities are not bundled into one large built-in system. Each upstream
open-source project should be wrapped as an independent PicAiPic plugin. Plugin
packages are registered by adding or dropping a plugin directory; runtime setup
is a separate profile-level workflow.

`D:\ailab\20260610133133` is reference material and a source pool for future
wrapping work. It should not be mounted into PicAiPic as one big plugin.

## Current AI Plugin Host

Implemented backend pieces:

- `src-tauri/src/t_plugin.rs`
- Tauri command registration in `src-tauri/src/main.rs`
- frontend API wrappers in `src-vite/src/common/api.js`
- plugin Settings UI in `src-vite/src/views/Settings.vue`
- menu/capability integration through `pluginStore`, file menus, media viewers,
  and `PluginActionDialog`

Current host capabilities:

- reads and validates `picaipic.plugin.json`
- maintains `plugin-registry.json`
- registers and unregisters plugin directories
- discovers plugins from app data, ProgramData, registered paths, and
  `PICAIPIC_PLUGIN_PATHS`
- lists plugins, capabilities, runtime profiles, menu contributions, setup
  state, diagnostics, logs, and validation warnings
- starts and stops `local-http` plugins
- invokes plugin capabilities through normalized HTTP payloads
- passes plugin runtime environment variables such as `PICAIPIC_PLUGIN_ROOT`,
  `PICAIPIC_PLUGIN_PORT`, `PICAIPIC_OUTPUT_DIR`, and runtime binding variables

## Runtime Profiles

Runtime profiles use three user-facing actions in Settings:

- `Setup`: records or prepares safe local runtime setup artifacts.
- `Run setup`: executes the plugin-declared setup command only after backend
  preview and explicit user confirmation.
- `Smoke`: starts the plugin, calls `POST /smoke-test`, displays structured
  results, and is the only action that can mark a profile `verified`.

The profile state flow is:

```text
notInstalled -> needsVerify -> verified / failed
```

Diagnostics alone do not mark a profile usable.

## Runtime Binding Direction

Do not make one private virtual environment per plugin the default strategy.
That becomes too large for AI plugins. Runtime environments are modeled as
bindings:

```text
external - existing user/project runtime
shared   - future PicAiPic-managed reusable runtime pool
plugin   - plugin-private runtime, used only when isolation is required
```

Profiles can declare:

- a default `runtimeBinding`
- additional `runtimeBindings`
- plugin-private `envDir` only when needed

Settings shows a runtime binding selector when multiple candidates exist. The
selected binding is passed to Setup, Run setup, and Smoke, and is persisted as a
snapshot in `profileStates`.

Host AI environment discovery now performs lightweight Python runtime discovery:

- Python paths declared by external runtime bindings
- plugin-local common environment folders such as `.venv`, `venv`, and `env`
- conda/venv folders under common user, ProgramData, and Poetry cache locations
- PATH commands such as `python`, `py`, `python3`

Discovered Python runtimes are shown in Settings and proposed as external
runtime candidates without changing the plugin manifest. Discovery is capped and
only runs a cheap `--version` probe so opening Settings does not import heavy AI
packages.

Settings also has an on-demand `Probe` action beside Python-backed runtime
bindings. Probe runs only when requested and checks the selected Python for
Python version, torch, CUDA, ROCm, DirectML, ONNX Runtime, and backend
availability hints. Probe results are persisted in `runtimeProbeStates` with a
runtime fingerprint. The host marks cached probe results stale when Python,
`pyvenv.cfg`, requirements, runtime binding, or TTL changes. Capability
invocation now performs a runtime probe preflight gate for Python-backed
profiles. Smoke remains the only action that can mark a runtime profile
`verified`.

## Probe UX Enhancement

The on-demand Probe action now provides richer detail, multi-binding cached
state display, and structured failure remediation. Three areas were improved:

Probe result detail is now grouped instead of a flat key-value list. The
Settings probe card shows five groups: General (target, duration, binding),
Python (version, platform, executable), torch (version, CUDA version, HIP
version, device count, MPS availability), Backends (per-backend available
state with device count, version, and tensor probe result), ONNX Runtime
(version and providers), and Packages (torch, torchDirectML, onnxruntime
availability and errors). Each item carries a tone — ok, bad, or neutral —
rendered with color cues so users can scan the result at a glance.

Multi-binding cached state display lets users see the probe status of every
runtime binding without probing each one. The backend `list_ai_plugins`
response now includes a `runtimeProbeStates` array on each install profile,
containing all persisted probe states for that plugin+profile pair. The
runtime binding selector in Settings appends a status marker to each option:
`✓` for passed, `✗` for failed, `⟳` for stale, and no marker for not-probed.
The frontend matches probe states to bindings by Python path first, then by
binding id, so switching the binding selector shows the correct cached result
immediately.

Failure remediation advice is now structured as `action` or `diagnostic`
items instead of a flat string list. Action items are rendered with a `→`
prefix and primary color; diagnostic items use a muted style. The advice
engine covers twelve failure scenarios: stale cache (three sub-reasons),
available runtime, torch not installed, torch import error, ONNX Runtime
missing, DirectML not installed, DirectML initialization failure, GPU device
count zero, tensor probe failure (OOM and non-OOM), probe timeout, no binding
selected, and unknown fallback. Tensor probe failure takes priority over the
"available → Smoke" path so a failed GPU tensor test is never hidden behind a
green checkmark.

## Plugin Action Dialog Progress

The PluginActionDialog (shown when a user triggers a plugin capability from the
image context menu or toolbar) previously showed only a spinner during the
entire task. It now shows real-time task progress and supports cancellation.

The dialog receives `taskStatus`, `taskProgress`, and `taskMessage` props from
the parent Content component. The `waitForPluginTaskOutput()` polling loop
updates these fields on every `getAiPluginTask` poll, so the dialog reflects
the current task state (queued, running, cancelling), a progress bar
(0–100%), and the plugin's progress message text. A "Cancel Task" button
appears when the task is in an active state and calls the existing
`cancel_ai_plugin_task` backend command.

## Setup Command Streaming And Cancellation

The Run setup command previously executed as a black box: stdout and stderr
were collected only after the command finished, the UI showed a loading
spinner with no progress, and there was no way to cancel a long-running
install.

The backend `run_setup_command()` now spawns the child process and reads
stdout and stderr line by line. Every 5 lines the job state is saved to the
registry, so the frontend can poll `list_ai_plugins` and see the log grow in
real time. A new `SetupCancellationState` global state tracks cancel requests
by job id. The command loop checks the cancel flag on each iteration and
kills the child process if cancellation is requested. A new
`cancel_ai_plugin_setup` Tauri command exposes this to the frontend.

The Settings UI now runs a 2-second polling loop during setup execution to
refresh the plugin list and show the live job status, progress bar, and
scrolling log output (max-height with overflow). A "Cancel" button appears
next to the progress percentage when the job is running. The setup job log
area is now scrollable instead of rendering the full log as a single block.

## Filesystem Handoff

Plugin invocation still uses loopback HTTP with JSON control payloads and file
paths for large image/LUT data. Host-created default outputs are now
task-scoped:

```text
app-cache\plugins\<pluginId>\tasks\<taskId>\outputs
```

The host validates returned output paths before accepting a successful invoke
response:

- returned paths must stay inside the task output directory
- returned files must exist and be non-empty regular files

The host also performs ledger-aware task cache cleanup. Failed and cancelled
task directories are removed best-effort. Orphan task directories are removed
after 24 hours, orphan `.tmp` files after 15 minutes, and unadopted successful
outputs are expired after 24 hours, marked `discarded`, and removed from disk.
SA-LUT image output now writes to a same-directory temporary file, flushes and
fsyncs it, then commits with atomic replace before returning the final path.

Plugin invocation now records a host-owned `taskStates` ledger in
`plugin-registry.json`. Tasks move through `queued`, `running`, `cancelling`,
`succeeded`, `failed`, `cancelled`, and later `imported` / `discarded` states.
Successful invoke responses store returned outputs in the ledger. After SA-LUT
output is imported into the current album, the frontend marks the task adopted
and asks the host to delete the task directory. Settings shows recent plugin
tasks and can explicitly discard unadopted successful tasks, marking them
`discarded` and deleting their task directory. Failed task states persist
structured error metadata: `errorCode`, `errorDomain`, optional details, and a
host-derived `retryable` flag. Retryable tasks store a safe invoke request
snapshot and can be retried from Settings. Retry creates a fresh task id and
task output directory instead of overwriting the failed task.

Cancellation is part of the task contract. The host exposes
`cancel_ai_plugin_task`, records `cancelling` / `cancelled` / failed cancel
states, and calls plugin `POST /tasks/{taskId}/cancel`. SA-LUT implements a
best-effort cancel registry and checks cancellation around model load, image
read, inference boundaries, output encode/write, and finalization. This is not
a hard interrupt for a model call already inside a blocking inference operation.

SA-LUT `color-transfer` now uses async invoke. `POST /invoke/color-transfer`
returns `202 Accepted` with a `taskId`, initial `queued` status, and task
tracking endpoints. Work runs in a plugin-side background worker with one active
task by default. The plugin exposes `GET /tasks/{taskId}/events` as a long-poll
event stream; the host consumes events first and falls back to
`GET /tasks/{taskId}` for older plugins. Failed and cancelled task directories
are cleaned best-effort; successful outputs remain for import/adopt/discard and
are later expired by host-side TTL cleanup if they were never adopted.
Settings now shows recent plugin tasks with clearer status badges, progress,
output counts, retry/cancel/discard actions, and a `Cleaned` label for expired
or discarded outputs.

## SA-LUT Plugin

Current local plugin:

```text
plugins/picai-salut-color
```

SA-LUT currently exposes:

```text
GET  /health
GET  /status
GET  /diagnostics
GET  /tasks/{taskId}
GET  /tasks/{taskId}/events
POST /smoke-test
POST /verify
POST /invoke/color-transfer
POST /invoke/export-lut
POST /tasks/{taskId}/cancel
```

`color-transfer` is wired through the local HTTP plugin wrapper.
`export-lut` is declared but still not implemented.

The ROCm profile declares the existing external Windows SA-LUT runtime:

```text
D:\ailab\20260610133133\backend\venv\Scripts\python.exe
```

This avoids duplicating a large PyTorch/ROCm environment under the plugin
directory. A plugin-private ROCm fallback binding is also declared, but it is
not the default.

## Verification Commands

Latest checks used during this work:

```text
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm --dir src-vite build
cargo test --manifest-path src-tauri/Cargo.toml -- --skip real_signed_zips_verify
.\scripts\check_plugin_host.ps1
.\scripts\package_plugin.ps1 -All -FailOnWarnings
python -m json.tool plugins\picai-salut-color\picaipic.plugin.json
python -m py_compile plugins\picai-salut-color\backend\main.py plugins\picai-salut-color\backend\salut_adapter.py
python scripts\stress_salut_async.py --tasks 8 --duration-ms 300 --cancel-every 3
python scripts\stress_salut_http.py --tasks 6 --duration-ms 250 --cancel-every 3
python -m json.tool plugins\picai-nafnet-restore\picaipic.plugin.json
python -m py_compile plugins\picai-nafnet-restore\backend\main.py plugins\picai-nafnet-restore\backend\nafnet_adapter.py plugins\picai-nafnet-restore\backend\denoiser.py
python scripts\stress_nafnet_http.py --tasks 4 --duration-ms 120 --cancel-every 2
```

## 2026-07-08 Plugin-level external model directory binding

- **Manifest-declared `modelBindings[]`**: new top-level manifest field. Each
  binding declares an `envVar` (e.g. `SALUT_MODEL_DIR`, `NAFNET_SOURCE_DIR`),
  optional extra `envVars`, a `layout` (`"files"` or `"sourceTree"`), and
  `expectedFiles`/`expectedGlobs` for validation. The host reads the manifest
  and injects the user-selected directory as the declared env var into the
  plugin process — no host-side hardcoded plugin-id→envVar mapping. New
  plugins add `modelBindings[]` to their manifest with zero backend changes.
- **`AiPluginProfileState.model_dir_bindings`**: per-profile persisted binding
  map (key = binding id, value = directory absolute path). `#[serde(default)]`
  keeps old registries forward-compatible. Setup/smoke flows that reconstruct
  profile state preserve existing bindings via `persisted_model_dir_bindings`.
- **`build_setup_environment` injection**: after the default
  `PICAIPIC_PLUGIN_MODEL_DIR`, the host injects each binding's `envVar` (and
  extra `envVars`) from the persisted map. Bindings without a persisted
  directory are skipped so the plugin falls back to its default resolution.
  This mirrors the runtime-binding precedence — `.local.env` still wins for
  developers because `start.bat`'s `for /f` loop runs after host injection.
- **Three Tauri commands**: `set_ai_plugin_model_dir_binding` (validate dir,
  persist, return check result), `clear_ai_plugin_model_dir_binding`,
  `check_ai_plugin_model_bindings` (validate without persisting). All
  registered in `main.rs`.
- **`list_ai_plugins` summary**: `AiPluginSummary.model_bindings` carries the
  manifest declarations; each `PluginInstallProfileSummary.modelBindingChecks`
  carries the live validation (present/missing files, `ok` flag) for that
  profile's persisted bindings.
- **Settings UI**: each profile row shows a model-binding card when the plugin
  declares `modelBindings`. Each binding shows a status chip (ready/missing/
  not-bound), the bound directory path, and Bind/Change/Open/Clear buttons.
  Directory picker uses the established `openDialog({ directory: true })`
  idiom; Open reuses `revealPath`.
- **SA-LUT manifest** declares `salut-model-dir` (`envVar: SALUT_MODEL_DIR`,
  `expectedFiles: [vgg_normalised.pth, epoch=100-step=4127466.ckpt.state.pt]`).
  **NAFNet manifest** declares `nafnet-source-dir` (`envVar: NAFNET_SOURCE_DIR`,
  `layout: sourceTree`, `expectedGlobs: [experiments/pretrained_models/*.pth]`).
- Validation: `cargo fmt --check`, `cargo check` (zero warnings), `pnpm build`,
  `python -m json.tool` (both manifests), `python -m py_compile` (both
  backends) all pass.

## 2026-07-07 Project rename, signing hardening, release build

- **Signature canonicalization fix**: the Ed25519 package signature was
  fragile — Python signed with unsorted JSON keys, Rust verified with
  struct field order. Both sides now use lexicographic key ordering
  (Python `sort_keys=True`, Rust `serde_json::Value` with BTreeMap), so
  the signature is field-order independent. Also fixed `Option::None`
  serialization mismatch (`skip_serializing_if` on `signature` and
  `created_at` fields). Unit tests cover cross-language consistency,
  key-order independence, and tamper rejection. See `t_plugin.rs` tests.
- **Project identity renamed from Lap to PicAiPic**: `productName`,
  `identifier` (`com.julyx10.lap` → `com.big2cater.picaipic`), Cargo.toml,
  window title, fallback URLs, and all user-facing docs updated. This
  changes the app data directory (`%LOCALAPPDATA%\com.big2cater.picaipic`),
  so prior Lap-era local data is not visible to the new identity.
- **Updater signing key rotated**: the old updater pubkey in
  `tauri.conf.json` belonged to upstream julyx10; the matching private key
  was never available to this fork. Generated a new minisign keypair;
  public key is in `tauri.conf.json`, private key is gitignored locally.
  Updater endpoint moved from `julyx10/lap` to `big2cater/picaipic`.
- **macOS support removed**: AI plugins are incompatible with macOS
  (plugin confinement is Windows-oriented; no macOS Seatbelt). Deleted
  `tauri.macos.conf.json`, `infoplist/`, homebrew workflow, and macOS
  matrix entries from release/pr-build workflows. Rust `cfg(macos)`
  branches kept intact (harmless, preserves structure). Platform scope is
  now Windows + Linux only.
- **Languages trimmed to en + zh**: dropped 7 locales (de/es/fr/ja/ko/pt/ru)
  and their i18n READMEs. Frontend bundle reduced ~40%.
- **Release build verified**: `package_windows.ps1` produces
  `PicAiPic.exe`, NSIS installer, and `.sig` updater signature. The
  script auto-loads the updater key from `picaipic-updater-key.key`.
- **End-to-end trust flow validated**: installing a signed plugin zip
  triggers the `TRUST_REQUIRED` consent dialog, user confirms, publisher
  is written to `plugin-registry.json`, and install completes. Verified
  with the real salut-color plugin package.
- **Two plugin packages signed**: both `picai-salut-color` and
  `picai-nafnet-restore` zips are signed with the release key
  (publisher `local`, pubkey `e7Ccs...pe8=`).
- **Dev server IPv4 fix**: Vite v8 binds IPv6 by default; Tauri devUrl
  resolves to IPv4. Set `server.host = '127.0.0.1'` in `vite.config.js`.

## 2026-07-10 Plugin sandbox policy update

- **Default confinement changed to input staging only**: external image inputs
  are copied into `plugin-cache/<id>/tasks/<taskId>/inputs/` before invoke and
  payload `path` values are rewritten. Plugins read staged copies instead of
  raw source-image paths.
- **Windows deny-ACL write confinement is now opt-in**:
  `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1` enables the old
  `icacls /deny <user>:(W) /L` path. It is no longer default because it mutates real user
  directory ACLs while plugins run and can trigger confusing host UI access
  prompts.
- **Development escape hatch preserved**:
  `PICAIPIC_DISABLE_PLUGIN_SANDBOX=1` skips both input staging and optional
  ACL handling.
- **Stale ACL cleanup added**: default plugin startup best-effort removes
  old deny ACEs left by previous builds or crashed runs, then continues
  without re-applying them unless opt-in ACL mode is set.
- **Verification**: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  and `cargo check --manifest-path src-tauri/Cargo.toml` passed. Local
  `cargo build --release` still fails at MSVC/CRT link time in existing
  `libort_sys`/`LibRaw` dependencies, unrelated to the sandbox code.

## Next Work

- ~~Write a `latest.json` generation script and create the first GitHub
  release (v0.2.4) with the NSIS installer + `.sig` + `latest.json`, so
  the in-app auto-updater has a real endpoint to check.~~ **Completed
  (2026-07-08):** first release `v1.0.0` (Draft) built end-to-end via
  `release.yml` + `release-windows.yml`. `latest.json` carries all four
  platforms (linux-x86_64, linux-aarch64, windows-x86_64, windows-aarch64)
  with valid signatures. Three CI build blockers were fixed along the way:
  `beforeBuildCommand` hardcoded a Windows absolute path (broke Linux CI),
  `third_party/` submodules were blocked by `.gitignore` so gitlinks never
  landed in commits (broke Rust `build.rs`), and `t_sandbox.rs` icacls
  calls lacked `#[cfg(target_os = "windows")]` guards (broke Linux
  compilation). The release stays as Draft until feature completeness.
- ~~Migrate AI model / ffmpeg binary downloads from `julyx10/lap-binaries`
  to a `big2cater/picaipic-binaries` release, so the fork does not depend
  on the upstream binary repo.~~ **Completed (2026-07-08):** ten binaries
  (8 ffmpeg/ffprobe sidecars for Windows x64/arm64 + Linux x86_64/aarch64,
  plus `text_model.onnx` and `tokenizer.json`) re-uploaded to
  `big2cater/picaipic-binaries` under `ffmpeg-8.1` and `models` release
  tags. `t_ai.rs` and `download_ffmpeg_sidecar.{ps1,sh}` now point at the
  new repo. The `picaipic-binaries` repo was set to public so anonymous
  release-asset downloads work at runtime.
- Design plugin signing key rotation (security-hardening open question 3):
  if an author's private key is compromised, there is currently no
  revocation/rotation path in the trust store.
- Migrate prior Lap-era local data (`com.julyx10.lap.debug` directory)
  to the new `com.big2cater.picaipic.debug` path, so existing dev-time
  plugin installs and config carry over.
- ~~Model import / external model directory binding support, so users with
  model files already on disk do not need to hand-edit `.local.env`.~~
  **Completed (2026-07-08):** plugin-level external model directory binding
  landed. Manifest `modelBindings[]` declares the env var + expected files;
  Settings UI lets users pick a directory; host injects it into the plugin
  process and validates file presence. Optional **bulk file import into the
  plugin model directory** remains a UX nice-to-have, not a blocker.
- Avoid concrete `export-lut` business logic until runtime binding
  confidence is stable across both SA-LUT and NAFNet.
- Add user-confirmed one-click fallback from a conflicting shared runtime to a
  plugin-private runtime (detect + text advice + manual private selection already work).
- Design publisher signing-key rotation/revocation and continue release-exe
  plugin regression after host/package changes.
- Strengthen network confinement and Linux process isolation without breaking
  GPU/runtime compatibility (beyond default input staging / opt-in Windows ACL).

## 2026-07-18 — 冲印排版后续 (A/B/C/D)

- 混排：上/下带与左/右带 + auto 利用率选择；预览显示利用率与策略
- 自定义相纸：完整表单（英寸/厘米切换），不再用 prompt
- 冲印排版打印：print-sized 临时图 + `window.print`（与单图右键一致；非 host print_file；导出仍全 DPI）
- 导入图库：可选勾选，导出后可选写入当前相册

## 2026-07-19 — 冲印/拼图对照光影魔术手修正

- 冲印：按相纸“铺满”缩放格位（不再大白边居中），补 A4 内置样式，打印走 window.print（与单图右键一致）
- 拼图：引入杂志式模板 cells（2v/3a/3b/4m/6m 等，源自 NeoImaging PatternJigsaw），预览/导出按归一化格子

## 2026-07-19 — 冲印/拼图性能与文档收口

### 冲印排版
- 铺满相纸排版 + A4 内置样式；**导出**全 DPI，**打印**用约 1800px 长边快路径 + `window.print`
- 源图按**格位目标像素**下采样（非无谓全分辨率解码）；同源并行解码
- 会话内 temp 缓存 + 后台预热；切换版式/关对话框删除 temp；24h 陈旧清理
- 安全删除：仅系统 temp 且 `print_layout_*` / `picaipic_*` 前缀
- 可选导出/打印后导入当前图库
- 打开卡死修复：禁止在 computed 内写 pinia

### 拼图
- 杂志式模板 cells（NeoImaging 归一化）：2/2v/3a/3b/4/4m/6/6m/9
- 网格/cells/自由导出均按格位下采样源图
- free drafts 辅助函数保留在 `collageTemplates.ts`

### 验证
- `pnpm --dir src-vite build`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## 2026-07-19 — 冲印打印快路径（print vs export）

- **导出**：仍为 plan DPI 全分辨率（冲印店/存档）
- **打印**：相纸比例 + 长边约 1800px 合成，再 `window.print`（避免等全 DPI 才弹系统打印框）
- 源图仍按**当前画布格位**下采样；打印缓存独立（blob URL + 浅 fingerprint）；切换/关闭清理 temp
- 验证：`pnpm --dir src-vite build`；手测同一版式二次打印应明显更快
