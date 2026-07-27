---
name: decisions
description: Active PicAiPic architectural decisions and their rationale.
triggers:
  - decision
  - rationale
  - why
  - alternative
  - historical choice
edges:
  - target: context/architecture.md
    condition: when a decision affects subsystem boundaries
  - target: context/stack.md
    condition: when a decision constrains technology choice
  - target: context/plugin-runtime.md
    condition: when a decision concerns plugin security or lifecycle
  - target: context/setup.md
    condition: when a decision affects release or platform workflow
last_updated: 2026-07-26
---



# Decisions

## Decision Log

### Audit ROI: harden import_url; warm embed matrix; keep local calendar days
**Date:** 2026-07-26  
**Status:** Active (shipped partial)  
**Decision:** (1) URL import uses shared reqwest client with **30s total / 10s connect** timeout, **100 MiB** Content-Length + accumulated chunk cap, chunked read (not unbounded `bytes()`). (2) Semantic-search matrix **warms in background** after successful `create_db`; do **not** add full disk ANN serde in this pass. (3) Calendar/filter date ranges stay **local midnight unix** — EXIF `meta_date_to_timestamp` is Local and SQL uses `strftime(..., 'localtime')`; do not flip to UTC without a coordinated migration.  
**Reasoning:** Code review claimed import DoS risk (confirmed), first-search ANN block (already background; warm matrix is the real cold-start fix), and timezone bugs (false positive against intentional local-day contract).  
**Consequences:** Oversized/hung URL downloads fail with explicit errors; first AI search after open still may hit exact matrix if warm incomplete; disk matrix/ANN cache remains future work under `change-library-perf.md`.  

### Ship bilingual int8 as only bundled text tower (Track C option C)
**Date:** 2026-07-24  
**Status:** Active (shipped)  
**Decision:** Installer **ships** CLIP-B/32-aligned **bilingual int8** as `resources/models/text_model.onnx` + tokenizer. Remove EN-only CLIP text from the bundle. **No Settings model switch.** Optional “re-download cloud pack” kept for observation. Self-host downloads on `big2cater/picaipic-binaries` tag `models` with SHA-256 verify. Vision remains bundled CLIP B/32 — **no library reindex**. Legacy EN CLIP URLs stay **commented** in `download_models.*` during observation; EN backup under `scripts/.probe-models/bundled-clip-en-text-backup/`.  
**Reasoning:** Phase 0 pack (`canavar` multilingual-v1 ONNX, `sentence_embedding` 512) passed EN/CN rank alignment and int8≈fp32 retrieval. Owner chose product C over dual-model UI: one text tower for CN+EN offline.  
**Consequences:** Installer text model larger than EN quant (~130MB vs ~64MB). `encode_text` must prefer `sentence_embedding`. Package with `build-exe.bat` / `package_windows.ps1 -Clean`. Guide: `docs/guide/altclip-phase0-probe.md`. Pattern: `change-image-search-model.md`.  

### Similar-from-file uses image→image floors, not text floors
**Date:** 2026-07-24  
**Status:** Active (shipped)  
**Decision:** When `search_text` is empty and `file_id > 0`, apply **image-image** absolute floors **0.88 / 0.82 / 0.74 / 0.62** and thr_caps **12 / 24 / 40 / 100** (same UI `thresholdIndex` 0–3 as text). Exclude the query file id. Relative empty-fallback uses `top1*0.92`. Text / smart tags keep histogram text floors. Changing Settings thr/limit re-runs active similar/search/smart-tag views.  
**Reasoning:** CLIP image-image cosine sits ~0.55–0.95; text floors (~0.14–0.24) pass almost everything, so Low≈Very High with default limit=50 and self-hit≈1.0.  
**Consequences:** “图片相似度” primarily differentiates **Find similar**; log `mode=image`. Retune floors from owner `mode=image` histograms if needed. Pattern: `change-ai-search-filters.md`.  

### In-memory embed matrix; limit hard cap; (amended) aligned Multilingual pack allowed
**Date:** 2026-07-24  
**Status:** Active (shipped; Multilingual path amended same day for Track C)  
**Decision:** (1) Score AI search from a process-local embedding matrix (exact cosine) with SQL BLOB fallback; invalidate on embed write/clear/`clear_conn_pool`; optional background HNSW for large N. (2) User `imageSearch.limit` is a **hard cap** for all tiers (`top_k = min(limit_or_thr_cap, thr_cap, 200)`). (3) Face `cosine_distance` fails closed on dim mismatch. (4) **Legacy sentence-space** multilingual remains rejected; **CLIP-aligned** bilingual pack (product C / optional app-data re-download) is allowed and is the bundled default text tower.  
**Reasoning:** Full-table BLOB re-read is the large-library bottleneck. Hard-cap is clearer than Low bypassing limit. Space-incompatible sentence towers empty CN search; aligned `sentence_embedding` does not.  
**Consequences:** Second AI search on same library should log `matrix=1` (or ANN). Plan: `docs/superpowers/plans/2026-07-24-ai-search-stopbleed-and-embed-cache.md`.  

### Keep CLIP B/32 vision default; do not ship SigLIP2 base-224 pack as product model alone
**Date:** 2026-07-23  
**Status:** Active  
**Decision:** Production **vision** stays **bundled CLIP ViT-B/32**. Track B0 (CLIP B/16 default) is **abandoned**. Track B Phase 0 for `onnx-community/siglip2-base-patch16-224` (prefer quantized for Rust ort) is **probe-complete** but **not** promoted to Settings sideload UI on this pack alone. Offline compare script may remain for future packs. **Text** default is bilingual int8 (Track C option C) — not a vision swap.  
**Reasoning:** Owner subjective: CLIP weak on insects/plants but OK elsewhere; SigLIP2 confuses small birds as insects — no clear quality win. Rust int8 fails (ConvInteger); quant/fp16 load OK but quality gate fails. B/16 felt ≈ B/32, not worth reindex.  
**Consequences:** Improve CLIP path (floors, Top-K, embed ladder, free-text template, calibrated thr, bilingual text) instead of swapping vision default. Future multi-model needs a pack that beats 植物/昆虫/小主体 or an explicit optional BETA with documented limits. Scripts: `compare_clip_vs_siglip2.py`, probes; guide: `docs/guide/siglip2-phase0-probe.md`.  

### CLIP text search floors are histogram-calibrated; slider owns primary cut
**Date:** 2026-07-23 (floors); **amended 2026-07-24** (ranking ownership + image-image split)  
**Status:** Active (shipped)  
**Decision:** Settings UI ladder **[0.28, 0.24, 0.20, 0.16]**. **Text primary cut** = `absolute_floor = max(0.16, thr*0.85)`. Relative `top1*0.85` is **empty-fallback only** (not `max(abs, rel)` default). **Text thr_cap Top-K:** VH 30 / H 40 / M 50 / L 200, then **`top_k = min(user_limit_or_thr_cap, thr_cap, 200)`**. **Smart tags follow `thresholdIndex`** (text path). **Similar-from-file** uses the image-image decision above (same UI index). Never force 0.25 for non-empty `search_text`. Re-calibrate text with `scripts/calibrate_search_thresholds.py` after model or library distribution shifts.  
**Reasoning:** Owner logs: strong bird/landscape max ≈ 0.25–0.28; VH 0.30 emptied results; family with people max ≈ 0.255. Pre-stop-bleed 0.40 and provisional 0.30 stop-bleed were too strict. **2026-07-24:** always applying `max(abs, top1*0.85)` made Low/Med/High identical on strong **text** queries; image-image needed a separate score band.  
**Consequences:** Slider changes cutoff and result count; smart tags respect similarity setting; log `mode` / `floor_mode` / `thr_cap`. Empty family with no people photos is content, not thr. Pattern: `change-ai-search-filters.md`, `change-smart-tags.md`.  

### Smart tags share free-text similarity slider
**Date:** 2026-07-24  
**Status:** Active (shipped)  
**Decision:** Smart-tag search does **not** pass a hard-coded `thresholdOverride`. It uses the same `imageSearch.thresholdIndex` / ladder as free-text search.  
**Reasoning:** Fixed thr (old 0.28→0.22→0.20) made “similarity settings” appear broken when users tested via smart tags.  
**Consequences:** Changing VH/H/M/L changes smart-tag result strictness; prompts remain short CLIP-style English.  

### Smart tags product set is six coarse CLIP buckets; people prompt is short plural
**Date:** 2026-07-24  
**Status:** Active (shipped)  
**Decision:** Ship **six** smart tags only: people / pets / landscape / architecture / plants / birds. Drop family/portraits/kids/land_animals/food/sports/night/insects (free-text instead). People prompt = **`a photo of people`**; pets = common species list (`dog or cat or rabbit or hamster or bird pet`). Default `imageSearch.thresholdIndex = 1` (High) for **new installs**; do not auto-migrate saved settings. Settings thr/limit re-runs smart-tag search via direct `getImageSearchFileList` + numeric coerce.  
**Reasoning:** Owner ~103-image logs: abstract `human` over-fires; multi-`or` people prompts dilute max and scramble top3; `a portrait of a person` misses rear-view mall queues; pets need concrete species. CLIP text scores sit in a narrow band — smart tags are coarse retrieval chips, not detectors; named people remain face-index.  
**Alternatives considered:** Multi-or face/portrait/group (failed); land_animals abstract bucket (failed); per-tag thr override (rejected — keep one slider).  
**Consequences:** UI labels 人物/宠物/…; pattern `change-smart-tags.md`; host log `search_similar mode=text` for diagnosis.  

### Smart Albums rule engine UX: supported size ops, pickers, local-day dates, no silent empty
**Date:** 2026-07-24  
**Status:** Active (shipped)  
**Decision:** Size ops include `is_not`/`empty`/`not_empty`; size always MB→bytes. Person/camera/lens pickers; toast on query error; per-album sort UI; date before/after/between use local calendar-day SQL; SIDEBAR.SMART with no selection shows empty guidance (never infinite loading).  
**Reasoning:** Unsupported size ops returned Err and Content swallowed it as 0 files; empty SMART sidebar never set `contentReady`; free-text Make||Model and UTC date display caused empty/wrong results.  
**Consequences:** Pattern `change-smart-albums.md`; stale smartAlbum id clears selection.  


### Prefer mid-edge original/RAW preview embeds over UI thumbs for CLIP
**Date:** 2026-07-23  
**Status:** Active (shipped)  
**Decision:** Embed decode ladder: JPEG turbo scaled @1024 → RAW LibRaw preview @1024 → open+cap → UI thumb last. Decode outside AiEngine mutex; release thumb permit before embed; embed semaphore 1.  
**Reasoning:** Thumb-only embeds (especially RW2 when `image::open` fails) hurt retrieval vs offline full-image scripts; full-res decode blocked scan previews. Mid-edge closes quality gap without full-res cost.  
**Consequences:** Libraries need re-embed/rescan for full benefit on old rows. Pattern: `change-ai-search-filters.md`.  

### Large-library face clustering will leave all-pairs for an adaptive ANN/blocked path
**Date:** 2026-07-22  
**Status:** Active (P0–P2 + in-process HNSW shipped; **P3 not doing for now**)  
**Decision:** Keep Chinese Whispers + Top-K + frozen seeds. Graph: **`auto`**: exact if `n < 8000`, else **HNSW ANN** (`instant-distance` pure Rust); **`fast`**: always ANN; **`exact`**: all-pairs. ANN non-cancel failure → **blocked exact** fallback. Settings `face.clusterMode`. No GPU face EP (G8 cancelled). **Do not implement P3** (disk ANN / incremental insert) unless measurement shows re-cluster is a real user pain; then prefer embedding binary cache over incremental HNSW.  
**Reasoning:** Sub-quadratic graph build is already in-tree; face-index wall time is mostly ONNX inference, not clustering. `instant-distance` is rebuild-from-points (no true incremental API); graph serde is version-brittle; partial-update risks frozen-seed / same-file regressions.  
**Consequences:** Program exit = P0–P2 + ANN + docs. Revisit only after real 20k+ face timings (parse vs graph vs whisper). Higher ROI for face speed remains inference workers/batching, not P3.  

### Photo frame is EXIF info framing (classic + blur float/sink + optional logo)
**Date:** 2026-07-22  
**Status:** Active (shipped G-Frame-1 + G2)  
**Decision:** Ship photix-inspired EXIF frames via independent multi-select dialog: classic white/black bars, then **float-blur** / **sink-blur** (cover blur + soft shadow) and optional **local logo path**. Host-local EXIF + draw; save-as only. Defer top/side magazine layouts, logo library UI, and batch-wizard action.  
**Reasoning:** Users asked for “相框” referencing photix-mark-web (EXIF watermark framing), then asked for Immich-style blur float/sink and logo because classic-only looked plain. Independent dialog matches collage/print/batch entry patterns; logo is user file pick (privacy, no cloud pack).  
**Consequences:** Batch `photoFrame` and magazine layouts remain follow-ups; logo is path-based, not a built-in brand pack.  

### ImageViewer toolbar prefers built-in Edit over plugin toolbar icons
**Date:** 2026-07-21  
**Status:** Active (shipped)  
**Decision:** Do not render image.toolbar plugin placements on the ImageViewer chrome. Provide a fixed Edit image button that opens the host ImageEditor. Plugins remain available from the image context menu.  
**Reasoning:** SA-LUT and similar tools cluttered the viewer bar; editing is the primary built-in action users expect on the chrome.  
**Consequences:** Sample plugins may still declare image.toolbar; host ignores that placement for now.  

### Batch watermark optional EXIF capture time
**Date:** 2026-07-21  
**Status:** Active (shipped)  
**Decision:** Text and image watermark batch actions may stamp each file EXIF capture time (format selectable).  
**Reasoning:** Common photo-export need without manual per-file text.  
**Consequences:** Missing EXIF yields a placeholder dash; does not fail the batch.  

### Unify photo style into presets + manual (layered preview)
**Date:** 2026-07-21  
**Status:** Active (shipped)  
**Decision:** Remove the standalone ImageEditor photo-style panel. Merge built-in/custom styles into the existing **Presets** strip and put host-only controls (highlights/shadows/fade/vignette/grain/LUT) under **Manual**. Use **layered preview**: CSS filters for base fields (instant); host `apply_photo_style_preview` only when host-only fields are active. One recipe model in `photoStylePresets.ts`. Custom presets keep config array order (no reorder on edit).  
**Reasoning:** Owner reported duplicate sliders vs manual adjust, presets vs style lists, and laggy style preview. Dual tracks were an MVP shortcut, not the target IA.  
**Alternatives considered:** Keep two panels with synced values (still noisy); always host-preview everything (keeps lag); CSS-only effects (cannot do LUT/grain well).  
**Consequences:** `activePhotoStyleId` maps to selected preset id; named custom recipes update in place; save-as adds to presets; batch `photoStyle` unchanged. Runbook: `patterns/change-photo-style.md`.

### Photo style + LUT library is local recipe system, not Photon cloud AI
**Date:** 2026-07-21  
**Status:** Active (shipped MVP)  
**Decision:** Implement Panasonic-like **photo styles** and a **user LUT library** on the host (`t_lut.rs` + ImageEditor + batch), inspired by PhotonCamera’s management/recipe layering. Keep apply order base→LUT→effects. Do **not** port Photon GLES realtime pipeline or Photon AI recolor (which uses OpenAI-compatible cloud vision APIs).  
**Reasoning:** Owner wants customizable looks (params + effects + LUT), not LUT-only filters. Local-first PicAiPic non-negotiables forbid making built-in looks depend on remote AI. Photon is a useful product/reference for library UX and recipe composition, not a drop-in codebase.  
**Alternatives considered:** LUT-only apply without style recipes (too weak vs 照片格调); full Photon ColorRecipe/GLES port (too large, camera-centric); cloud AI recolor like Photon (conflicts with local-first default for built-ins).  
**Consequences:** Style cubes from traditional color match can be imported into the LUT library and referenced by styles. Future optional AI recolor would be a separate, explicit feature (likely plugin), not mixed into host photo-style MVP. Runbook: `patterns/change-photo-style.md`.


### Host traditional color match vs single-image style LUT
**Date:** 2026-07-21  
**Status:** Active (shipped)  
**Decision:** Ship pure-Rust global Lab color match on the host (ImageEditor adjust panel + batch `colorMatch`) without segmentation/OpenCV/plugin runtime. Export `.cube` as a **single-image style bake** (default 33³): prefer the selected reference image, else the current photo. Do not encode a dual-image source×reference match map into the LUT. Keep this separate from cancelled SA-LUT plugin **G7** `export-lut`.  
**Reasoning:** Owner wants apply-match and LUT export as independent workflows; a style LUT from one photo is reusable and matches “export this image’s look,” while dual-image match maps are redundant with “save matched image then export.”  
**Alternatives considered:** Dual-image match-map cube (rejected by owner); SA-LUT neural LUT export (cancelled G7); region/SAM matching (deferred).  
**Consequences:** `export_color_match_lut` takes only `sourceFilePath` + dest + size; UI labels must say “style LUT,” not “export match map.” Runbook: `patterns/change-color-match.md`.


### Calendar empty content was sidebar-index drift, not empty library
**Date:** 2026-07-20  
**Status:** Active (shipped)  
**Decision:** Treat left-sidebar indices as a named `SIDEBAR` contract in `constants.ts`. After Smart Albums was inserted at index 1, Content must route calendar at **4** (not legacy 3). Calendar day counts and file queries share file-type / exclusion / Live-companion filters and local-day `strftime` comparison.  
**Reasoning:** Users saw dots with counts but “未找到文件” because clicks updated `libConfig.calendar` while Content still ran the search branch. Packaging/cache was a red herring.  
**Alternatives considered:** Hard-coded numeric fix only in the calendar branch (rejected: same class of bug remains for tags/person/shortcuts); clearing WebView cache (rejected: does not change routing).  
**Consequences:** All new sidebar buttons require updating `SIDEBAR` + Content/Home. Runbook: `patterns/change-calendar.md`.

### Incremental face clustering preserves manual person labels
**Date:** 2026-07-20  
**Status:** Active (shipped)  
**Decision:** `cluster_faces` no longer calls `reset_all_assignments` on normal re-index. Existing `person_id` assignments are frozen seeds; only unassigned faces join clusters or create new `Person N` via `next_auto_person_number`.  
**Reasoning:** Full wipe destroyed renames/merges on every face index — high-severity UX bug for a photo manager.  
**Alternatives considered:** Post-cluster name remapping only (weaker for manual splits); schema `is_manual` flag (deferred — frozen assignments already preserve user edits).  
**Consequences:** Explicit full-reset paths may still use `reset_all_assignments`. See `patterns/change-face-index.md`.

### Gap triage 2026-07-20: cancel G1/G7/G8/G9; ship G2/G6/G10–G13; sandbox 3–5 opt-in only
**Date:** 2026-07-20  
**Status:** Active  
**Decision:** Do **not** implement G1 (collage-in-batch), G7 (`export-lut`), G8 (face GPU EP), or G9 (whole-library empty-comment backfill). Ship batch library import (G2), multi-key trust + local revoke (G6), FileInfo Live hover (G10), print magazine pack (G11), export-only DPI UI (G12), and system-print UX copy (G13) without host `print_file`. Sandbox Phase 3–5 remain **opt-in env flags**, default confinement stays Phase 0–2.  
**Reasoning:** Owner priority is product polish and trust/security foundation without expanding AI plugin business surface (LUT export / GPU face) or expensive full-library backfill. Print device selection stays in the OS dialog for local-first simplicity.  
**Alternatives considered:** Native printer/tray host API (rejected for v1); default-on Landlock/network sandbox (rejected until GPU matrix); whole-library prompt backfill (rejected for large-library cost).  
**Consequences:** Docs/ROUTER list cancelled IDs explicitly; scan-time empty-only prompt import remains the only path. See `docs/guide/目前的开发情况.md`, `patterns/change-print-layout.md`, `patterns/change-live-photo.md`.

### Import AI generation prompts into empty comments only (PNG + JPEG)
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** During library scan, import A1111/NovelAI/InvokeAI/ComfyUI prompts from PNG text chunks and JPEG UserComment/COM into `afiles.comments` **only when empty**. Default **on**. No full-library backfill of unchanged files in v1.
**Reasoning:** Matches lap #197 UX and PicAiPic’s AI-oriented library without overwriting user notes; scan-time is local-first and free of cloud upload.
**Alternatives considered:** Always overwriting comments (rejected: destroys user data); JPEG-only or PNG-only first (PNG first shipped, JPEG extended same day); mtime-unchanged rescan fill (rejected for large-library cost).
**Consequences:** `t_ai_prompt.rs` + Settings Library toggle; re-scan merge must preserve non-empty comments and use `update_column` because `AFile::update` omits comments. Optional backfill remains future work. See `patterns/change-ai-prompt-import.md`.

### Configurable thumbnail media badges default off
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** Media-info overlays (format/ISO/shutter/aperture/focal/exposure) are **opt-in** flags under `settings.grid.mediaBadges`, cap **4** badges per thumb.
**Reasoning:** Dense 10k–100k grids need low visual noise by default; power users can enable capture metadata like lap #174.
**Alternatives considered:** Always-on capture strip (rejected for clutter); reusing only bottom labels without overlays (incomplete vs upstream badge UX).
**Consequences:** Cross-window sync via settings events; older Pinia configs normalize missing `mediaBadges`. See `patterns/change-media-badges.md`.

### Viewer canvas background is app chrome, not global theme
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** Image/quick-viewer media area supports theme/black/white/gray/checker via `mediaViewer.backgroundMode`; cycle with **B**; does **not** change app-wide appearance theme.
**Reasoning:** Checking transparency and edge contrast needs a local canvas override (lap #173) without breaking daisyUI theme preference.
**Alternatives considered:** Only dark/light app theme (insufficient for checker/transparent review); per-window non-persisted mode (worse UX).
**Consequences:** Paint media area only; sync Settings/toolbar/shortcut. See `patterns/change-viewer-background.md`.

### AI search uses same file-type bitmask as library queries
**Date:** 2026-07-19
**Status:** Active (shipped)
**Decision:** `ImageSearchParams.search_file_type` reuses library mask (0 all / 1 image / 2 video / 4 raw) and filters vector-search SQL before cosine scoring; search results show Visual/Similar/Filename section headers.
**Reasoning:** Users expect the same Image/RAW/Video control in AI search as in albums; filtering candidates early avoids scoring irrelevant embeddings.
**Alternatives considered:** Frontend-only post-filter (wastes embedding work / wrong limit); separate AI-only type enum (duplicate UX).
**Consequences:** Toolbar filter enabled in search-like views; similar-from-file temp mode re-runs on filter change. See `patterns/change-ai-search-filters.md`.

### Ship built-in crop/collage/batch as host tools, not AI plugins
**Date:** 2026-07-18
**Status:** Active (Phase A/B/C1/C2 + print layout shipped; C3 optional)
**Decision:** Photo-size crop presets, collage/拼图, multi-image batch processing, and print layout are **host-built-in** features. Plan and phased scope live in `docs/guide/builtin-tools-roadmap.md` (A crop → B collage → C batch → print). Do not route v1 of these through the AI plugin runtime.
**Reasoning:** Geometry, export, and deterministic batch work fit the local editor/library host; plugins remain for heavy/model-specific AI. Reference UX is 光影魔术手-style, not full parity.
**Alternatives considered:** Implementing batch only as plugins was rejected for v1 (worse offline UX and trust surface for basic resize/crop). Full “magic hand” parity in one release was rejected as scope risk.
**Consequences:** Keep originals safe (explicit overwrite); share crop presets between editor and batch; C3 collage-as-batch-step remains optional.

### Prefer GitHub Release assets over Actions artifacts for installers
**Date:** 2026-07-18
**Status:** Active
**Decision:** Multi-arch app installers for tagged releases upload primarily to the **GitHub Release** for that tag. Actions `upload-artifact` for PR/release builds is **best-effort** (continue-on-error, short retention) and must not fail a successful compile/bundle.
**Reasoning:** Free-tier Actions artifact storage quota repeatedly failed CI after successful Tauri builds (`CreateArtifact: Artifact storage quota has been hit`).
**Alternatives considered:** Only deleting old artifacts manually was insufficient as a sole fix; keeping hard-fail upload re-blocked release drafts.
**Consequences:** `release.yml` / `release-windows.yml` / `pr-build.yml` follow this policy. `latest.json` is assembled from release assets. Operators may still prune stale Actions artifacts occasionally.

### Keep media local and use a folder-first workflow
**Date:** 2024-08-08
**Status:** Active
**Decision:** PicAiPic works directly with user-selected folders and performs search/AI locally without required upload.
**Reasoning:** Privacy, no library lock-in, offline usability, and suitability for large personal collections are core product promises.
**Alternatives considered:** Cloud-managed libraries and mandatory import into proprietary storage were rejected because they violate privacy and ownership goals.
**Consequences:** Original files remain external source data; uninstall/database cleanup must not delete them, and features must tolerate filesystem changes.

### Use Tauri/Rust for privileged work and Vue for UI
**Date:** 2024-08-08
**Status:** Active
**Decision:** Vue renders the desktop UI while Rust owns filesystem, database, decoding, AI, and process operations behind Tauri commands.
**Reasoning:** The split combines a productive UI stack with native performance, broad media integration, and a controlled privilege boundary.
**Alternatives considered:** A browser-only app cannot safely access local libraries/native codecs; putting privileged work in JavaScript would weaken control and performance.
**Consequences:** IPC contracts must stay synchronized, and long-running Rust operations need events/cancellation to keep the UI responsive.

### Store metadata in per-library SQLite databases
**Date:** 2026-01-15
**Status:** Active
**Decision:** Each configured library has a local SQLite database, with versioned migrations and optional custom database storage.
**Reasoning:** SQLite is embedded, offline, portable, and sufficient for large-library metadata without operating a service.
**Alternatives considered:** A server database adds deployment complexity; one global database increases coupling between independent libraries.
**Consequences:** Schema changes require forward migrations, storage moves require WAL checkpoint/copy safeguards, and backup/restore covers multiple library DBs.

### Run AI extensions as signed, permissioned local HTTP plugins
**Date:** 2026-07-04
**Status:** Active
**Decision:** AI plugins run as host-managed loopback processes with signed packages, publisher trust, explicit permissions, bearer-token authentication, runtime profiles, and staged inputs.
**Reasoning:** Independent Python/PyTorch stacks need isolation and lifecycle control without embedding every model dependency in the host.
**Alternatives considered:** In-process plugins risk dependency/ABI conflicts; unrestricted subprocesses lack a defensible trust boundary.
**Consequences:** Contract changes are cross-cutting; release builds reject unsigned packages, input paths are rewritten to staged copies, and runtime drift can block invocation.

### Make Windows deny-ACL plugin confinement opt-in
**Date:** 2026-07-10
**Status:** Active
**Decision:** Input staging is the default v1 confinement; the Windows `icacls` deny-write path is enabled only with `PICAIPIC_ENABLE_PLUGIN_ACL_SANDBOX=1`.
**Reasoning:** Applying ACLs to real user directories caused disruptive prompts and confusing host behavior; staging provides a safer default without mutating user directory ACLs.
**Alternatives considered:** Default deny-ACL was implemented and tested but rejected as the normal path; network blocking and restricted-token approaches remain future work.
**Consequences:** Do not describe v1 as a complete OS sandbox. Preserve stale-ACL cleanup and the development disable switch.


### Treat sandbox hardening as a phased host design, not a UI bundle
**Date:** 2026-07-17
**Status:** Active
**Decision:** Further sandbox work follows `docs/ai-plugin-sandbox-roadmap.md`: Phase 0 correctness (cross-platform staging, fail-closed staging errors), then host write allow-list clarity, then optional large-file hardlink/zero-copy, with network OS block and Linux Landlock as separate opt-in research spikes. Do not ship experimental ACL/Landlock toggles in Settings until Phase 0–1 are verified.
**Reasoning:** OS confinement is high-risk, platform-specific, and GPU-sensitive; mixing it with product UI work caused scope and safety confusion. Host-side path control already provides the defensible default.
**Alternatives considered:** Default OS network/process isolation was deferred because it breaks common Python/GPU stacks and needs admin/capability spikes. Env scrubbing remains deferred to preserve venv activation.
**Consequences:** Product docs must not claim kernel network sandboxing. Linux release builds use the same input-staging default as Windows. Experimental OS modes stay behind explicit env flags.

### Support Windows and Linux releases, not macOS
**Date:** 2026-07-07
**Status:** Active
**Decision:** Current release scope is Windows x64/arm64 and Linux x86_64/aarch64.
**Reasoning:** The AI plugin confinement/runtime implementation is Windows-oriented and no macOS Seatbelt integration exists.
**Alternatives considered:** Keeping macOS packaging was rejected until plugin security and runtime support can meet the contract.
**Consequences:** CI/release docs must not claim macOS support; remaining conditional Rust branches are not proof of a supported target.

### Support both Apple Live Photo and Google Motion Photo with long-press preview
**Date:** 2026-07-13
**Status:** Active
**Decision:** PicAiPic simultaneously supports Apple Live Photo (HEIC/JPEG + MOV paired by ContentIdentifier UUID) and Google Motion Photo (JPEG with embedded MP4 offset in XMP `Container:Directory`). Paired MOV files remain visible as independent videos in the library but are linked to their companion image via `paired_file_id`. Users long-press an image in MediaViewer to play the paired video and release to return to the static image.
**Reasoning:** Live Photo and Motion Photo are the two dominant formats for hybrid still+motion captures; supporting both covers the majority of consumer device ecosystems (iPhone + Pixel/Samsung). Keeping the MOV visible as an independent video preserves user expectations that all imported files are browseable, while the link enables the Live Photo interaction. Long-press mirrors iOS native behavior and is the most intuitive gesture.
**Alternatives considered:** Integrating the external `live-photo-conv` Vala/GTK/GStreamer project was rejected because it only handles Android Motion Photo (not Apple), requires GStreamer + GObject dependencies incompatible with the Tauri stack, and duplicates capability the project already has (libheif + FFmpeg + EXIF). Hidden-then-linked MOV files were rejected as they break the expectation that all imported files are visible. Click-to-play button was rejected as less intuitive than long-press.
**Consequences:** DB schema is at v6 with `content_id`, `paired_file_id`, `live_photo_type` columns on `afiles`. `t_xmp.rs` module depends on `quick-xml`. HEIC container-internal video track extraction (some Apple Live Photos embed video in HEIC rather than separate MOV) is deferred to a future iteration. File-name stem pair fallback handles exported photos that lost ContentIdentifier metadata but requires same-folder + same-stem naming convention.

### Rollback disk renames when DB metadata updates fail
**Date:** 2026-07-17
**Status:** Active
**Decision:** After a successful filesystem rename (file or root folder), any subsequent SQLite metadata update failure must best-effort rename the path back to the original name before returning failure to the frontend. Partial multi-column DB writes (for example `name_pinyin` then `name`) also restore earlier columns when a later step fails.
**Reasoning:** The UI treats a failed rename as no-op, but an unreverted disk rename leaves `afiles.name` / virtual `file_path` pointing at a missing path and breaks open/thumbnail/reindex. `move_file` already rolled back disk on DB failure; rename must match that invariant.
**Alternatives considered:** Returning success after disk rename while logging DB failure was rejected because the list would show the old name and subsequent operations would target the wrong path. Leaving the new disk name and repairing only DB was rejected because the command already returned failure to the client.
**Consequences:** `rename_file` and `rename_folder` call `t_utils::rename_*` again with the original basename on DB error. If rollback itself fails, log a critical message; full two-phase rename transactions remain a future hardening option.

### Black-hole gravity uses photo-area WebGL vortex (not CSS cards as primary)
**Date:** 2026-07-26
**Status:** Active
**Decision:** Live gravity path freezes the photo-area visible thumbnails into a texture and runs `PhotoVortexLayer` (FragCoord-style UV rotation lens). CSS `useGravityWarp` remains in-tree but is not driven from GridView. Idle threshold is **6s** (was 15s). Effect is photo-region only; chrome stays interactive.
**Reasoning:** Owner wanted continuous space-warping absorb like shader demos, not rigid floating cards; CSS transforms could not match that look and caused flicker/menu stacking bugs.
**Alternatives considered:** Keep refining CSS 6-layer warp; full geodesic raytrace (deferred for cost).
**Consequences:** Design docs that still describe CSS-only absorb are historical for card math; product QA follows PhotoVortex. Pattern: `change-black-hole-theme.md`.

### RAW grid thumbs prefer embedded JPEG before demosaic
**Date:** 2026-07-26
**Status:** Active
**Decision:** `get_raw_thumbnail` tries camera-embedded JPEG first (best max-edge ≥ target), then half_size demosaic only if needed. Scan thumb failures/timeouts always advance `processed` so preview phase cannot stick at N-2.
**Reasoning:** JPG+RW2 libraries spent most scan time demosaicing every RAW under heavy concurrency 1–2; embedded previews are good enough for grid UI.
**Alternatives considered:** Always demosaic for color fidelity (too slow); pair-display only JPEG and hide RAW (product still wants both visible).
**Consequences:** Occasional orientation/WB differences vs demosaic; terminal may log timeouts. Pattern: `fix-library-scan-selection.md`.

## 2026-07-26 — Supplement review triage

- External/static supplement report overstated urgency: many items are maintainability debt, not defects.
- Accepted now: poison-safe mutex helper + debug log cleanup.
- Deferred: Content.vue split, AFile::new split, CLIP multi-session, face_indices Arc, sleep eventization.
- Rejected as written: "t_sqlite zero tests" (has embed/search unit tests); P1 "decode under lock" (already outside).

## 2026-07-27 — Keep AFile split incremental and connection-testable

- Extract only stable, local responsibilities from `AFile::new` first: image/RAW header pre-read remains behaviorally identical behind `read_file_header`.
- Keep CRUD production entry points on the library connection path, but implement their SQL cores with an explicit `&Connection`/`&mut Connection` so temporary SQLite fixtures can exercise insert/update/delete without mutating global library configuration.
- Defer the remaining metadata extraction split until representative EXIF/RAW fixtures exist.
- The next increment isolates `read_image_exif` without changing the header-first/full-JPEG fallback policy; field mapping, RAW overlay, Live Photo detection, and geocoding remain in `AFile::new`.
- A minimal little-endian TIFF/EXIF byte fixture is sufficient to lock orientation behavior without committing media assets; orientation remains EXIF-first, binary-fallback, default-1.
