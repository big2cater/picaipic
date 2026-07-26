/**
 * Config Store - Global application configuration
 */
import { defineStore } from 'pinia';

export const useConfigStore = defineStore('configStore', {
  state: () => ({
    main: {
      sidebarIndex: 0,            // toolbar index
      maxLibraryCount: 20,        // max library count
      selectionChunkSize: 200,    // virtual list fetch chunk size
    },

    content: {
      filmStripPaneHeight: 160,   // film strip pane height (px)
    },

    leftPanel: {
      show: true,                 // show left pane
      width: 320,                 // left pane width
    },

    rightPanel: {
      show: false,                // show right panel
      width: 360,                 // panel width in px
      mode: 'info',               // right panel mode ('info' | 'dedup')
    },

    infoPanel: {
      showPreview: true,         // show preview thumbnail
      previewMode: 'thumbnail',  // preview section mode ('thumbnail' | 'histogram')
      previewScale: 1,           // preview thumbnail scale (1, 0.5, 0.25)
      histogramChannels: 15,     // histogram channel mask (L=1, R=2, G=4, B=8; 0=none, 15=all)
      showBasicInfo: true,       // show basic info
      showMetadata: true,        // show metadata
      showMap: true,             // show map
      mapTheme: 0,               // 0: standard, 2: satellite
    },

    search: {
      maxSearchHistory: 20,     // max search history
      fileType: 0,              // bitmask: 0 all, 1 image, 2 video, 4 raw, 8 live/motion still
      sortType: 0,              // sort type (default to time)
      sortOrder: 0,             // sort order(0: ascending, 1: descending)
    },

    calendar: {
      isMonthly: true,    // display monthly or daily calendar
    },

    mediaViewer: {
      isZoomFit: true,      // true: zoom to fit container; false: original size(scale = 1)
      isPinned: true,       // pinned mode
      // 0: theme, 1: black, 2: white, 3: gray, 4: checker
      backgroundMode: 0,
    },

    video: {
      muted: false,           // video muted
      volume: 1.0,            // video volume (0.0-1.0)
    },

    imageEditor: {
      tab: 'edit',               // image editor active tab ('edit' | 'adjust')
      custom: {
        brightness: 0,
        contrast: 0,
        saturation: 100,
        hue: 0,
        blur: 0,
        filter: '',
        highlights: 0,
        shadows: 0,
        fade: 0,
        vignette: 0,
        grain: 0,
        lutId: '',
        lutIntensity: 100,
      },
      // Legacy numeric crop shape kept for older persisted configs; prefer cropPresetId.
      cropShape: 0,
      // Crop preset id: 'free' | ratio-* | photo-* | custom-*
      cropPresetId: 'free',
      // User-defined favorite ratios (app-wide, not per-library)
      customCropRatios: [],
      // Photo styles (Panasonic-like): user customs only; builtins in photoStylePresets.ts
      photoStyles: [],
      activePhotoStyleId: 'natural',
      saveAs: 0,                // image editor save as (0: Overwrite existing file, 1: Save as new file)
      format: 0,                // image editor format (0: JPEG, 1: PNG, 2: WEBP)
      quality: 0,               // jpeg quality (0: High, 1: Medium, 2: Low), [90, 80, 60]
    },

    // Collage free-canvas project drafts (app-wide; geometry + style only)
    collage: {
      freeDrafts: [],
    },

    // Batch action chain templates ("一键动作") + optional import of saveAs outputs
    batchProcess: {
      templates: [],
      importToLibrary: false,
    },

    // Photo frame / 相框: user-saved full option presets + last export prefs
    photoFrame: {
      presets: [],
      importToLibrary: false,
      lastTemplateId: 'classic-white',
      lastPresetId: '',
    },

    // Photo print layout (冲印排版): custom papers + custom packing styles
    printLayout: {
      customPapers: [],
      customLayouts: [],
      dpi: 300,
      background: '#ffffff',
      showGuides: true,
      importToLibrary: false,
    },

    imageViewer: {
      isSplit: false,           // split view (legacy mirror of splitCount > 1)
      splitCount: 1,            // 1 | 2 | 4 pane comparison
      isSyncViewport: false,    // sync viewport
      isFullScreen: false,      // native fullscreen in image viewer window
    },

    libraryChangedVersion: 0,

    collectionTray: {
      expanded: true,
      height: 180,
    },

    settings: {
      tabIndex: 0,               // settings tab index (0: general, 1: view, 2: library, 3: image search, 4: shortcuts, 5: about)

      // general settings
      language: 'en',             // default language
      appearance: 1,              // appearance (0: light; 1: dark)
      // Theme index (v1.4): 0 default, 1 retro, 2 cmyk, 3 black hole — see setTheme / isBlackHoleTheme
      lightTheme: 0,
      darkTheme: 0,
      // v1.5: dynamic theme distortion intensity (0=off 0.5=subtle 1=standard 1.5=intense)
      dynamicThemeIntensity: 1,
      scale: 1,                   // root font-size scale
      showButtonText: true,       // show button text
      showToolTip: true,          // show button tooltip
      showStatusBar: true,        // show status bar
      showCollections: true,     // show collections tray under left sidebar
      autoCheckUpdates: true,      // automatically check for updates
      debugMode: false,           // debug mode

      // navigation settings
      folderSort: 0,              // folder_sort_options: 0=name asc, 1=name desc, 2=date asc(oldest first), 3=date desc(newest first)
      calendarSort: 0,            // 0=taken asc, 1=taken desc, 2=created asc, 3=created desc, 4=modified asc, 5=modified desc
      categorySort: 0,            // category_sort_options: 0=name asc, 1=name desc, 2=count asc, 3=count desc
      showSubfolderFiles: false,  // show subfolder files (in album folder view)
      // Import AI generation prompts from PNG metadata into empty comments on scan
      importAiPromptsToComments: true,
      
      // grid view settings
      thumbnailSize: 512,         // thumbnail image size (small: 128, medium: 256, large: 512, extra large: 1024)
      grid: {
        size: 160,               // grid size, range 120-360
        style: 0,                // 0: card view, 1: tile view, 2: justified view, 3: masonry view
        showFilmStrip: false,    // show filmstrip view
        scaling: 1,              // 0: Fit Entire Image, 1: Crop to Fill, 2: Stretch to Fill
        labelPrimary: 1,         // card view: primary label (1: Name)
        labelSecondary: 3,       // card view: secondary label (3: Dimension)
        previewPosition: 0,      // filmstrip view: preview position (0: top, 1: bottom, 2: left, 3: right)
        dateGrouping: 0,         // show date groups: 0: none, 1: day, 2: month
        // Overlay media-info badges on thumbnails (format / capture settings)
        mediaBadges: {
          format: false,
          iso: false,
          shutter: false,
          aperture: false,
          focal: false,
          exposure: false,
        },
      },
      
      // image view settings
      mouseWheelMode: 1,         // 0: previous/next, 1: zoom in/out (default)
      slideShowInterval: 1,      // slide show interval in seconds [1, 3, 5, 10, 30, 60]
      slideShowTransition: 0,    // 0: Slide, 1: Fade, 2: None
      navigatorViewMode: 0,      // 0: Auto, 1: Always show, 2: Always hide
      navigatorViewSize: 240,    // navigator view size (160, 240, 320, 400)
      autoPlayVideo: true,       // auto play video
      loopVideo: false,          // loop video (only effective when autoPlayVideo is off)
      // showComment: false,        // show comment
      externalImageAppPath: '',    // external image app path
      externalImageAppName: '',    // external image app display name
      externalVideoAppPath: '',    // external video app path
      externalVideoAppName: '',    // external video app display name

      // image search settings
      imageSearch: {
        // 0: bundled bilingual text (resources int8 EN+CN). 1 reserved / cloud override observation.
        model: 0,
        // Default High (1 → thr 0.24): abs floor thr*0.85=0.204, thr_cap 40.
        // Histogram-calibrated after owner search_similar logs (2026-07-23): absent
        // concepts max≈0.21 so Low (0.16) floods false positives; VH (0.28) is optional.
        // Existing installs keep their saved index until user changes Settings.
        thresholdIndex: 1,
        // Ranked top-K after floor (host soft-caps at 200). 1000 dumped half-library noise.
        limit: 50,
      },
      
      // face recognition settings
      face: {
        enabled: false, // enable face recognition in image search
        // Cluster threshold index: 0=Very High, 1=High, 2=Medium, 3=Low
        clusterThresholdIndex: 2, // Default: Medium
        // Graph build: auto (exact small-n / HNSW large-n) | exact | fast (HNSW; blocked fallback)
        clusterMode: 'auto',
      },
    },
  }),

  getters: {
    // Image search settings_thr (higher = stricter).
    // Text search primary cut = max(0.16, thr*0.85); thr_cap VH30/H40/M50/L200.
    // Similar-from-file (image→image) uses a separate host ladder (floors ~0.62–0.88, caps 12–100)
    // because CLIP image-image scores sit far above text-image. Same UI index for both.
    // Relative top1*0.85 is empty-fallback only for text. Smart tags share the text ladder.
    // Calibrated 2026-07-23 from owner search_similar histograms (~100–300 embeds):
    //   strong (bird/landscape) max ≈ 0.25–0.28; >0.28 rare; nothing useful >0.30
    //   concept tags (portrait/family w/ people) max ≈ 0.23–0.26
    //   absent concept max ≈ 0.21 (empty is correct)
    // [Very High, High, Medium, Low]
    imageSearchThresholds: () => [0.28, 0.24, 0.20, 0.16],
    
    // Cluster threshold values: cosine distance (lower = stricter, higher = looser)
    // [Very High, High, Medium, Low]
    faceClusterThresholds: () => [0.35, 0.45, 0.55, 0.65],
  },

  actions: {
    // general settings
    setAppearance(appearance) {
      this.settings.appearance = appearance;
    },
    setLightTheme(lightTheme) {
      const n = Number(lightTheme);
      this.settings.lightTheme = (Number.isFinite(n) && n >= 0 && n <= 3) ? Math.floor(n) : 0;
    },
    setDarkTheme(darkTheme) {
      const n = Number(darkTheme);
      this.settings.darkTheme = (Number.isFinite(n) && n >= 0 && n <= 3) ? Math.floor(n) : 0;
    },
    setScale(scale) {
      this.settings.scale = scale;
    },
    setExternalImageAppPath(externalImageAppPath) {
      this.settings.externalImageAppPath = externalImageAppPath;
    },
    setExternalImageAppName(externalImageAppName) {
      this.settings.externalImageAppName = externalImageAppName;
    },
    setExternalVideoAppPath(externalVideoAppPath) {
      this.settings.externalVideoAppPath = externalVideoAppPath;
    },
    setExternalVideoAppName(externalVideoAppName) {
      this.settings.externalVideoAppName = externalVideoAppName;
    },
    setLanguage(language) {
      this.settings.language = language;
    },
    setShowButtonText(showButtonText) {
      this.settings.showButtonText = showButtonText;
    },
    setShowToolTip(showToolTip) {
      this.settings.showToolTip = showToolTip;
    },
    setShowStatusBar(showStatusBar) {
      this.settings.showStatusBar = showStatusBar;
    },
    setAutoCheckUpdates(autoCheckUpdates) {
      this.settings.autoCheckUpdates = autoCheckUpdates;
    },
    setDebugMode(debugMode) {
      this.settings.debugMode = debugMode;
    },
    setSettingsTabIndex(tabIndex) {
      this.settings.tabIndex = tabIndex;
    },
    setFolderSort(folderSort) {
      this.settings.folderSort = folderSort;
    },
    setCalendarSort(calendarSort) {
      this.settings.calendarSort = calendarSort;
    },
    setCategorySort(categorySort) {
      this.settings.categorySort = categorySort;
    },
    setShowSubfolderFiles(showSubfolderFiles) {
      this.settings.showSubfolderFiles = showSubfolderFiles;
    },
    setShowCollections(showCollections) {
      this.settings.showCollections = !!showCollections;
    },
    setImportAiPromptsToComments(importAiPromptsToComments) {
      this.settings.importAiPromptsToComments = importAiPromptsToComments !== false;
    },

    // video settings
    setVideoMuted(videoMuted) {
      this.video.muted = videoMuted;
    },
    setVideoVolume(videoVolume) {
      this.video.volume = videoVolume;
    },

    // grid view settings
    setGridSize(gridSize) {
      this.settings.grid.size = gridSize;
    },
    setGridStyle(gridStyle) {
      this.settings.grid.style = gridStyle;
    },
    setGridScaling(gridScaling) {
      this.settings.grid.scaling = gridScaling;
    },
    setGridLabelPrimary(gridLabelPrimary) {
      this.settings.grid.labelPrimary = gridLabelPrimary;
    },
    setGridLabelSecondary(gridLabelSecondary) {
      this.settings.grid.labelSecondary = gridLabelSecondary;
    },
    setGridDateGrouping(dateGrouping) {
      this.settings.grid.dateGrouping = dateGrouping;
    },
    setGridMediaBadges(mediaBadges) {
      const next = mediaBadges && typeof mediaBadges === 'object' ? mediaBadges : {};
      const normalized = {
        format: !!next.format,
        iso: !!next.iso,
        shutter: !!next.shutter,
        aperture: !!next.aperture,
        focal: !!next.focal,
        exposure: !!next.exposure,
      };
      // Avoid replacing the object when values are unchanged. Settings.vue deep-watches
      // mediaBadges and re-emits; both main and settings windows listen on the same event,
      // so a replace-every-time path causes an infinite emit/apply loop (UI flicker / hang).
      const cur = this.settings.grid.mediaBadges;
      if (
        cur &&
        cur.format === normalized.format &&
        cur.iso === normalized.iso &&
        cur.shutter === normalized.shutter &&
        cur.aperture === normalized.aperture &&
        cur.focal === normalized.focal &&
        cur.exposure === normalized.exposure
      ) {
        return;
      }
      this.settings.grid.mediaBadges = normalized;
    },
    setShowFilmStrip(showFilmStrip) {
      this.settings.grid.showFilmStrip = showFilmStrip;
    },

    // image view settings
    setFilmStripViewPreviewPosition(filmStripViewPreviewPosition) {
      this.settings.grid.previewPosition = filmStripViewPreviewPosition;
    },
    setMediaViewerBackgroundMode(backgroundMode) {
      const mode = Number(backgroundMode);
      this.mediaViewer.backgroundMode = Number.isFinite(mode)
        ? Math.max(0, Math.min(4, Math.trunc(mode)))
        : 0;
    },
    setMouseWheelMode(mouseWheelMode) {
      this.settings.mouseWheelMode = mouseWheelMode;
    },
    setSlideShowInterval(slideShowInterval) {
      this.settings.slideShowInterval = slideShowInterval;
    },
    setSlideShowTransition(slideShowTransition) {
      this.settings.slideShowTransition = slideShowTransition;
    },
    setAutoPlayVideo(autoPlayVideo) {
      this.settings.autoPlayVideo = autoPlayVideo;
    },
    setNavigatorViewMode(navigatorViewMode) {
      this.settings.navigatorViewMode = navigatorViewMode;
    },
    setNavigatorViewSize(navigatorViewSize) {
      this.settings.navigatorViewSize = navigatorViewSize;
    },
    // setShowComment(showComment) {
    //   this.settings.showComment = showComment;
    // },
    // image search settings
    setImageSearchModel(model) {
      const n = Number(model);
      // Product: bundled bilingual is 0. Value 1 only if cloud Multilingual path is used later.
      if (!Number.isFinite(n)) {
        this.settings.imageSearch.model = 0;
        return;
      }
      this.settings.imageSearch.model = n === 1 ? 1 : 0;
    },
    setImageSearchThresholdIndex(imageSearchThresholdIndex) {
      // HTML <select> / event payload may be string; host thr ladder needs a numeric index.
      const n = Number(imageSearchThresholdIndex);
      this.settings.imageSearch.thresholdIndex = Number.isFinite(n) ? n : 1;
    },
    setImageSearchLimit(imageSearchLimit) {
      const n = Number(imageSearchLimit);
      this.settings.imageSearch.limit = Number.isFinite(n) && n > 0 ? n : 50;
    },

    // face recognition settings
    setFaceEnabled(enabled) {
      if (!this.settings.face) {
        this.settings.face = { enabled, clusterThresholdIndex: 2, clusterMode: 'auto' };
      } else {
        this.settings.face.enabled = enabled;
      }
    },
    setFaceClusterThresholdIndex(index) {
      if (!this.settings.face) {
        this.settings.face = { enabled: true, clusterThresholdIndex: index, clusterMode: 'auto' };
      } else {
        this.settings.face.clusterThresholdIndex = index;
      }
    },
    setFaceClusterMode(mode) {
      const allowed = ['auto', 'exact', 'fast'];
      const next = allowed.includes(mode) ? mode : 'auto';
      if (!this.settings.face) {
        this.settings.face = { enabled: true, clusterThresholdIndex: 2, clusterMode: next };
      } else {
        this.settings.face.clusterMode = next;
      }
    },

    notifyLibrariesChanged() {
      this.libraryChangedVersion++;
    },

  },
  persist: true
});
