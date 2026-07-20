import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'
import { createPinia } from 'pinia'
import piniaPersistedState from 'pinia-plugin-persistedstate'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import router from '@/common/router'
import App from '@/App.vue'
import { useConfigStore } from '@/stores/configStore'
import '@/assets/app.css'

// I18n
import en from '@/locales/en.json'
import zh from '@/locales/zh.json'

// Create the app instance
const app = createApp(App)

// Create Pinia store and use the persisted state plugin
const pinia = createPinia()
pinia.use(piniaPersistedState)
app.use(pinia) // Use Pinia
const config = useConfigStore() // Use the config store

// Create the I18n instance
const i18n = createI18n({
  legacy: false, // Disable legacy mode
  locale: config.settings.language, // Use language setting from config store
  fallbackLocale: "en",
  messages: {
    en,
    zh,
  },
})

// Set up global properties
app.config.globalProperties.$invoke = invoke

// Use the router and i18n
app.use(router)
app.use(i18n)

// Mount the app
app.mount('#app')
console.log('App mounted', app)

// Listen for events
listen('settings-appearance-changed', (event) => {
  config.setAppearance(event.payload)
})
listen('settings-lightTheme-changed', (event) => {
  config.setLightTheme(event.payload)
})
listen('settings-darkTheme-changed', (event) => {
  config.setDarkTheme(event.payload)
})
listen('settings-scale-changed', (event) => {
  config.setScale(event.payload)
})
listen('settings-externalImageAppPath-changed', (event) => {
  config.setExternalImageAppPath(event.payload)
})
listen('settings-externalImageAppName-changed', (event) => {
  config.setExternalImageAppName(event.payload)
})
listen('settings-externalVideoAppPath-changed', (event) => {
  config.setExternalVideoAppPath(event.payload)
})
listen('settings-externalVideoAppName-changed', (event) => {
  config.setExternalVideoAppName(event.payload)
})
listen('settings-language-changed', (event) => {
  config.setLanguage(event.payload)
})
listen('settings-showButtonText-changed', (event) => {
  config.setShowButtonText(event.payload)
})
listen('settings-showToolTip-changed', (event) => {
  config.setShowToolTip(event.payload)
})
listen('settings-showStatusBar-changed', (event) => {
  config.setShowStatusBar(event.payload)
})
listen('settings-autoCheckUpdates-changed', (event) => {
  config.setAutoCheckUpdates(event.payload)
})
listen('settings-debugMode-changed', (event) => {
  config.setDebugMode(event.payload)
})
listen('settings-settingsTabIndex-changed', (event) => {
  config.setSettingsTabIndex(event.payload)
})
listen('settings-folderSort-changed', (event) => {
  config.setFolderSort(event.payload)
})
listen('settings-calendarSort-changed', (event) => {
  config.setCalendarSort(event.payload)
})
listen('settings-categorySort-changed', (event) => {
  config.setCategorySort(event.payload)
})
listen('settings-showSubfolderFiles-changed', (event) => {
  config.setShowSubfolderFiles(event.payload)
})
listen('settings-importAiPromptsToComments-changed', (event) => {
  config.setImportAiPromptsToComments(event.payload)
  // Keep Rust scan flag in sync (best-effort; Home also syncs on boot).
  import('@/common/api').then(({ setImportAiPrompts }) => {
    void setImportAiPrompts(event.payload !== false)
  }).catch(() => {})
})
listen('settings-gridSize-changed', (event) => {
  config.setGridSize(event.payload)
})
listen('settings-gridStyle-changed', (event) => {
  config.setGridStyle(event.payload)
})
listen('settings-gridScaling-changed', (event) => {
  config.setGridScaling(event.payload)
})
listen('settings-gridLabelPrimary-changed', (event) => {
  config.setGridLabelPrimary(event.payload)
})
listen('settings-gridLabelSecondary-changed', (event) => {
  config.setGridLabelSecondary(event.payload)
})
listen('settings-gridDateGrouping-changed', (event) => {
  config.setGridDateGrouping(event.payload)
})
listen('settings-gridMediaBadges-changed', (event) => {
  config.setGridMediaBadges(event.payload)
})
listen('settings-filmStripViewPreviewPosition-changed', (event) => {
  config.setFilmStripViewPreviewPosition(event.payload)
})
listen('settings-mouseWheelMode-changed', (event) => {
  config.setMouseWheelMode(event.payload)
})
listen('settings-mediaViewerBackgroundMode-changed', (event) => {
  config.setMediaViewerBackgroundMode(event.payload)
})
listen('settings-navigatorViewMode-changed', (event) => {
  config.setNavigatorViewMode(event.payload)
})
listen('settings-autoPlayVideo-changed', (event) => {
  config.setAutoPlayVideo(event.payload)
})
listen('settings-loopVideo-changed', (event) => {
  config.settings.loopVideo = event.payload
})
listen('settings-navigatorViewMode-changed', (event) => {
  config.setNavigatorViewMode(event.payload)
})
listen('settings-navigatorViewSize-changed', (event) => {
  config.setNavigatorViewSize(event.payload)
})
listen('settings-slideShowTransition-changed', (event) => {
  config.setSlideShowTransition(event.payload)
})
// listen('settings-showComment-changed', (event) => {
//   config.setShowComment(event.payload)
// })
listen('settings-imageSearchThresholdIndex-changed', (event) => {
  config.setImageSearchThresholdIndex(event.payload)
})
listen('settings-imageSearchLimit-changed', (event) => {
  config.setImageSearchLimit(event.payload)
})
listen('settings-faceClusterThresholdIndex-changed', (event) => {
  config.setFaceClusterThresholdIndex(event.payload)
})
listen('settings-faceEnabled-changed', (event) => {
  config.setFaceEnabled(event.payload)
})
listen('libraries-changed', () => {
  config.notifyLibrariesChanged()
})
