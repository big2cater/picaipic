---
name: fix-library-scan-selection
description: Concurrent scan duplicate afiles (#190), post-delete/move selection+timeline refresh, and Live Photo recount/repair after move.
last_updated: 2026-07-19
---

# Fix scan duplicates, stale selection, Live Photo move counts

## When to use
- Concurrent album scan / folder-sync creates duplicate rows
- Move/delete leaves wrong selection, date-group headers, or stale search names
- Cross-album move leaves wrong sidebar totals or Live Photo pairing

## Fixes shipped (2026-07-19)

### #190 Concurrent scan duplicates
| Layer | Change |
|-------|--------|
| Schema v8 | `migrate_unique_album_files`: collapse dup `(folder_id,name)`, `uidx_afiles_folder_id_name` UNIQUE |
| Insert | `INSERT ... ON CONFLICT(folder_id, name) DO NOTHING`; `add_to_db` re-enters existing path when `inserted==0` |
| Concurrency | `AlbumScanGuard` on `index_album_worker`; folder mtime sync skips active albums |

### #2 Stale selection / date groups after delete-move
| Layer | Change |
|-------|--------|
| Content | `removeFilesByIds` — id-set removal, keep focus by id when possible, rebuild multi-select by id |
| Timeline | `refreshTimelineAfterListMutation` after list membership changes |
| Call sites | `files-deleted` listener, multi-delete, multi-move, `removeFromFileList` |

### #3 Live Photo move / sidebar counts
| Layer | Change |
|-------|--------|
| `move_file` | After DB update: `pair_live_photos` + `recount_album` for old and new album ids |

### Defense-in-depth
- `#203` style: `thumbnail_ready` uses `file && !file.isPlaceholder`
- `Thumbnail.vue` optional-chains `props.file?.thumbnail`

### Second wave (2026-07-19)

#### #204 JPEG / RAW metadata consistency
| Layer | Change |
|-------|--------|
| `t_libraw` | value/unit spaces (`24 mm`, `1/30 s`); lens_model no longer synthesized from focal range |
| `t_image` | kamadak `continue_on_error` + partial recovery; `read_capture_settings_with_little_exif` for legacy JPEG |
| `t_sqlite` `AFile::new` | header miss → full JPEG EXIF scan; little_exif capture fallback only when all capture fields empty |
| `get_exif_field` | keep crate unit spacing (no aggressive reformat) |
| UI | `formatCaptureSettingValue` trims display precision only |

#### #186 Video probe + FileInfo preview
| Layer | Change |
|-------|--------|
| `t_video` | probesize/analyzeduration bounds; 5s UI / 20s index timeout; skip duration PTS for mpg/mpeg/vob/ts/mts/m2ts |
| `video.ts` | `isWebViewVideoPlaybackDisabled` for mpg/mpeg/vob |
| `FileInfo` | zoom controls stay available during video preview; hide play overlay for unsupported WebView formats |

#### #199 Folder selection restore flicker
| Layer | Change |
|-------|--------|
| `useAlbumSelection.selectFolder` | keep known `folderId` when restoring the same folder; only resolve when still current |

#### Multi-select external open safeguards
| Layer | Change |
|-------|--------|
| Host | `open_files_with_app` (batch args); `open_file_with_app` delegates |
| Content | selection-aware open; reject mixed image+video; warn when count > 100 |
| i18n | `msgbox.open_external_many`, `tooltip.open_external` (en/zh) |

### Third wave (2026-07-19)

#### Multi-select shared context menu + panel entry
| Layer | Change |
|-------|--------|
| `Thumbnail` | select-mode right-click emits `select-contextmenu` (does not open per-item menu) |
| `GridView` | forwards `select-contextmenu` with file index |
| `Content` | one shared `ContextMenu` for the selection: external open / copy / favorite / rating / tag / comment / rotate / move / trash |
| `SelectionPanel` | **Open in external app** button → `openExternal` |

#### Video.vue mpg fail-fast
| Layer | Change |
|-------|--------|
| `Video.vue` | `isWebViewVideoPlaybackDisabled` short-circuits load; shows format + external-player guidance |

### Fourth wave (2026-07-19)

#### Video compatibility UX
| Layer | Change |
|-------|--------|
| Loading overlay | compatibility copy + external-player button while remux/transcode runs |
| Load strategy | direct WebView try → `compatible` prepare → `process` force; MKV skips direct; 30s hard deadline |
| External open | cancels in-flight prepare before launching external player |
| i18n | `video.loading_compatible`, `video.errors.unsafe_webview` |

#### Multi-select plugin actions
| Layer | Change |
|-------|--------|
| Shared menu | appends `image.selection.multi` then `image.selection.single` plugin menus (deduped) |
| Target | focused selected image, else first selected image; toast when multi >1 |
| Bootstrap | `pluginStore.loadPlugins()` on Content mount |

## Verify
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `pnpm --dir src-vite build`
- Manual: open existing library (migration 8 once); concurrent scan + folder browse; delete/move under date grouping; move Live Photo across albums and check counts
- Manual: JPG/RAW capture labels share unit spacing; legacy JPEG after reindex; mpg/ts probe does not hang; FileInfo video zoom; remount album panel without flicker; multi-select external open mixed/>100
- Manual: select mode right-click shared menu; SelectionPanel external open; open mpg in viewer → error + external button
- Manual: open AVI/WMV/MKV → compatibility overlay + optional external; multi-select image plugin entry runs on focused image

## Notes
- Schema pattern: append-only migration versions (v8 here; v7 remains collections).
- Repair path also calls `migrate_unique_album_files` so unique index is enforced even if `user_version` was already advanced.
- Full lap `Video.vue` player-epoch/request-id plumbing is **not** ported; UX + strategy + cancel-on-external are.
