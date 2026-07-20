---
name: change-smart-albums
description: Rule-based Smart Albums (智能相册) — LibraryState definitions + server-side SQL evaluation.
last_updated: 2026-07-19
---

# Change smart albums / 智能相册

## When to use

- Add/edit smart album rule fields, operators, or evaluation SQL
- Change SmartAlbumList/Edit UI or Content smart query source

## Touchpoints

| Area | Path |
|------|------|
| Rule SQL |  , , ,  |
| IPC |   / , ,  |
| Persistence |  +  selection (, ) |
| UI | , , Home sidebar entry |
| Content | , , fetch via  |
| i18n | ,  |

## Rules

- Definitions are **JSON in LibraryState** (not SQLite tables). Evaluation is **server-side SQL** only.
- Require ≥1 rule; match mode  (AND) or  (OR). Max 20 rules in editor.
- Always AND search-exclusion + exclude Live companion videos ().
- Core fields: favorite, rating, name, file_type, extension, dates, size, orientation, tag, person, has_gps, camera, lens.
- Size values &lt; 100000 treated as **MB** (×1e6 to bytes).
- Opening a smart album updates cached  (and first-file  when available).

## Verify

-  / 
> picaipic@1.1.0 build D:ilab\PicAiPic\src-vite
> vite build --config vite.config.js

[36mvite v8.0.16 [32mbuilding client environment for production...[36m[39m
[2K
transforming...✓ 698 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                               1.82 kB │ gzip:   0.80 kB
dist/assets/image-file-D_-fXNSN.png          11.44 kB
dist/assets/icon-CV4SW8kY.png                78.25 kB
dist/assets/ImageViewer-D1pctEYY.css          0.06 kB │ gzip:   0.06 kB
dist/assets/ManageLibraries-B9Gkgz4L.css      0.06 kB │ gzip:   0.08 kB
dist/assets/TitleBar-BMpljF7P.css             0.11 kB │ gzip:   0.08 kB
dist/assets/MessageBox-DEqYj2L_.css           0.47 kB │ gzip:   0.17 kB
dist/assets/Content-a-Gjc6F4.css              0.96 kB │ gzip:   0.46 kB
dist/assets/StatusBar-DCbChZ1u.css            1.49 kB │ gzip:   0.41 kB
dist/assets/ImageEditor-CnzDeish.css          1.75 kB │ gzip:   0.55 kB
dist/assets/leaflet-vh-t_kPv.css             15.09 kB │ gzip:   6.36 kB
dist/assets/Video-D6B9mToZ.css               47.39 kB │ gzip:  12.53 kB
dist/assets/index--5m1NK3c.css              224.19 kB │ gzip:  32.97 kB
dist/assets/plus-92SBLVGP.js                  0.33 kB │ gzip:   0.27 kB
dist/assets/arrow-down-DSHUiX-D.js            0.36 kB │ gzip:   0.28 kB
dist/assets/video-play-Bz4sGhz9.js            0.37 kB │ gzip:   0.31 kB
dist/assets/error-HtsEHbiq.js                 0.39 kB │ gzip:   0.30 kB
dist/assets/icon-CUnxReGt.js                  0.39 kB │ gzip:   0.30 kB
dist/assets/more-DMPQp__i.js                  0.40 kB │ gzip:   0.27 kB
dist/assets/map-default-DBT0TKAo.js           0.49 kB │ gzip:   0.35 kB
dist/assets/user-DUcWXTQx.js                  0.52 kB │ gzip:   0.36 kB
dist/assets/update-ph9WQ0vq.js                0.52 kB │ gzip:   0.36 kB
dist/assets/folder-CNxD0MIi.js                0.52 kB │ gzip:   0.35 kB
dist/assets/refresh-jGKXmb4Y.js               0.55 kB │ gzip:   0.38 kB
dist/assets/edit-CtLyNpca.js                  0.58 kB │ gzip:   0.38 kB
dist/assets/folders-BBQeU-CM.js               0.60 kB │ gzip:   0.37 kB
dist/assets/restore-6fDUo9Ch.js               0.64 kB │ gzip:   0.42 kB
dist/assets/eye-off-D0gOaKVd.js               0.64 kB │ gzip:   0.43 kB
dist/assets/folder-heart-mgvwao7Y.js          0.69 kB │ gzip:   0.43 kB
dist/assets/zoom-out-B2l4mS6v.js              0.74 kB │ gzip:   0.34 kB
dist/assets/rename-BMSUv2Qz.js                0.82 kB │ gzip:   0.47 kB
dist/assets/calendar-day-BrXwh1sh.js          0.98 kB │ gzip:   0.43 kB
dist/assets/information-DV6lnm6q.js           1.14 kB │ gzip:   0.58 kB
dist/assets/star-filled-DqxuOCOH.js           1.21 kB │ gzip:   0.54 kB
dist/assets/Library-Cmas23Vi.js               1.34 kB │ gzip:   0.77 kB
dist/assets/heart-fill-WGuMqSxX.js            1.48 kB │ gzip:   0.60 kB
dist/assets/layout-split-542F5nXB.js          1.55 kB │ gzip:   0.60 kB
dist/assets/smartTags-D1C_pFv7.js             1.82 kB │ gzip:   0.95 kB
dist/assets/TitleBar-OjByAlRI.js              2.35 kB │ gzip:   1.01 kB
dist/assets/Location-D-k7y54L.js              3.03 kB │ gzip:   1.25 kB
dist/assets/Camera-BUSxXdxD.js                4.05 kB │ gzip:   1.50 kB
dist/assets/TButton-U2H-9GpZ.js               4.59 kB │ gzip:   1.93 kB
dist/assets/updater-CyjCrcoI.js               5.42 kB │ gzip:   1.93 kB
dist/assets/Favorite-BFibfaqR.js              6.03 kB │ gzip:   2.14 kB
dist/assets/pluginRuntime-CxCUFw76.js         6.58 kB │ gzip:   2.51 kB
dist/assets/MessageBox-CrVecttI.js            6.79 kB │ gzip:   2.93 kB
dist/assets/Tag-BnopUfXB.js                   7.03 kB │ gzip:   2.83 kB
dist/assets/Calendar-QgBnyTvD.js              8.18 kB │ gzip:   2.96 kB
dist/assets/ContextMenu-Bwqr3cL-.js           8.72 kB │ gzip:   3.02 kB
dist/assets/Person-CrEI2sZv.js                9.00 kB │ gzip:   3.47 kB
dist/assets/ImageSearch-Ch7A0qP0.js           9.89 kB │ gzip:   3.00 kB
dist/assets/ManageLibraries-Ci40RaS0.js      10.68 kB │ gzip:   3.97 kB
dist/assets/MapHeatmapView-BBeeMY8m.js       11.97 kB │ gzip:   4.91 kB
dist/assets/SmartAlbumList-DAniy5JA.js       13.41 kB │ gzip:   4.14 kB
dist/assets/Home-DewVwVLH.js                 15.68 kB │ gzip:   6.33 kB
dist/assets/SliderInput-DrqhGPt2.js          17.17 kB │ gzip:   6.76 kB
dist/assets/ImageViewer-Nd9KLI0W.js          25.84 kB │ gzip:   8.54 kB
dist/assets/vue-draggable-plus-DRQcSvJf.js   41.78 kB │ gzip:  14.79 kB
dist/assets/MoveTo-DTZwuDAR.js               42.82 kB │ gzip:  12.88 kB
dist/assets/ImageEditor-DkhRkwZd.js          49.61 kB │ gzip:  14.20 kB
dist/assets/StatusBar-6xaF3IAi.js            81.60 kB │ gzip:  23.34 kB
dist/assets/Settings-ZHsHyRZT.js            145.50 kB │ gzip:  33.78 kB
dist/assets/leaflet-DQhprtKH.js             149.99 kB │ gzip:  43.83 kB
dist/assets/Video-BAznyMBS.js               282.26 kB │ gzip:  82.90 kB
dist/assets/Content-CJ-DFQLH.js             314.01 kB │ gzip:  89.50 kB
dist/assets/index-rH3RuX2J.js               414.63 kB │ gzip: 138.58 kB

[32m✓ built in 1.10s[39m
- Manual: create album (favorite is true AND rating ≥ 4) → open → list matches; edit/delete; switch library isolation
