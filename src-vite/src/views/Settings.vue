<template>
  <div class="w-screen h-screen flex flex-col bg-base-300 text-base-content/70 overflow-hidden">
    <!-- Title Bar -->
    <TitleBar :titlebar="$t('sidebar.settings')" :resizable="false" viewName="Settings" class="shrink-0 z-50" />

    <div class="flex flex-1 overflow-hidden relative">
      <!-- Sidebar -->
      <div class="w-40 m-1 p-2 bg-base-200/30 flex flex-col rounded-box overflow-y-auto shrink-0 select-none">
        <div
          v-for="(tab, index) in settingsTabs"
          :key="index"
          :class="[
            'px-3 py-2 rounded-box cursor-pointer transition-all duration-200 font-medium flex items-center',
            config.settings.tabIndex === index 
              ? 'bg-base-100 text-primary' 
              : 'hover:text-base-content hover:bg-base-100/30'
          ]"
          @click="config.settings.tabIndex = index"
        >
          {{ $t(tab) }}
        </div>
      </div>

      <!-- Main Content -->
      <div class="p-2 mr-1 mb-2 flex-1 overflow-y-auto scrollbar-hide bg-base-300 cursor-default select-none">
          
        <!-- General Tab -->
        <div v-if="config.settings.tabIndex === 0" class="flex flex-col space-y-2">
          
          <!-- languange -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.general.section_language') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.select_language') }}</div>
                <div v-if="config.settings.language !== 'en'" class="text-xs text-base-content/30">Select language</div>
              </div>
              <select class="select  select-bordered select-sm min-w-32" v-model="config.settings.language">
                <option v-for="(lang, index) in languages" :key="index" :value="lang.value">{{ lang.label }}</option>
              </select>
            </div>
          </div>

          <!-- appearance -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.general.section_appearance') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.appearance') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.appearance">
                <option v-for="(item, index) in appearanceOptions" :key="index" :value="item.value">{{ item.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.theme') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="currentTheme">
                <option v-for="(option, index) in themeOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.font_size') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.scale">
                <option v-for="(option, index) in scaleOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
          </div>

          <!-- external app -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.image_view.section_external_apps') }}</span>
            </div>
            <div class="flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="min-w-0 flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.external_image_editor') }}</div>
                <div class="text-xs text-base-content/30 truncate" :title="config.settings.externalImageAppPath || ''">
                  {{ externalImageAppName }}
                </div>
              </div>
              <div class="shrink-0 flex items-center gap-1">
                <button 
                  class="btn btn-sm btn-ghost min-w-20 rounded-box bg-base-100 border border-base-content/30 text-base-content/70 hover:text-base-content" 
                  @click="selectExternalApp('image')"
                >
                  {{ $t('settings.image_view.choose_app') }}
                </button>
                <TButton v-if="config.settings.externalImageAppPath"
                  :icon="IconTrash"
                  :buttonSize="'small'"
                  :tooltip="$t('settings.image_view.clear_app')"
                  @click="clearExternalApp('image')"
                />
              </div>
            </div>
            <div class="flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="min-w-0 flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.external_video_app') }}</div>
                <div class="text-xs text-base-content/30 truncate" :title="config.settings.externalVideoAppPath || ''">
                  {{ externalVideoAppName }}
                </div>
              </div>
              <div class="shrink-0 flex items-center gap-1">
                <button 
                  class="btn btn-sm btn-ghost min-w-20 rounded-box bg-base-100 border border-base-content/30 text-base-content/70 hover:text-base-content" 
                  @click="selectExternalApp('video')"
                >
                  {{ $t('settings.image_view.choose_app') }}
                </button>
                <TButton v-if="config.settings.externalVideoAppPath"
                  :icon="IconTrash"
                  :buttonSize="'small'"
                  :tooltip="$t('settings.image_view.clear_app')"
                  @click="clearExternalApp('video')"
                />
              </div>
            </div>
          </div>

          <!-- display -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.general.section_interface') }}</span>
            </div>
            <div class="flex items-center justify-between p-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.show_button_text') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.showButtonText" />
            </div>
            <div class="flex items-center justify-between p-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.show_tool_tip') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.showToolTip" />
            </div>
            <div class="flex items-center justify-between p-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.show_status_bar') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.showStatusBar" />
            </div>
            <div class="flex items-center justify-between p-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.general.auto_check_updates') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.autoCheckUpdates" />
            </div>
          </div>

        </div>

        <!-- View Tab -->
        <div v-else-if="config.settings.tabIndex === 1" class="flex flex-col space-y-2">

          <!-- grid view -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.view.section_layout') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.view.style') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.grid.style">
                <option v-for="(option, index) in gridStyleOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.view.scaling') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.grid.scaling" :disabled="config.settings.grid.style !== 0 && config.settings.grid.style !== 1">
                <option v-for="(option, index) in gridScalingOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.view.label_primary') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.grid.labelPrimary" :disabled="config.settings.grid.style !== 0">
                  <option v-for="(option, index) in gridLabelOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.view.label_secondary') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.grid.labelSecondary" :disabled="config.settings.grid.style !== 0">
                  <option v-for="(option, index) in gridLabelOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.view.date_grouping') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.view.date_grouping_hint') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.grid.dateGrouping">
                <option v-for="(option, index) in dateGroupingOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.filmstrip_view.preview_position') }}</div>
                <!-- <div class="text-xs text-base-content/30">{{ $t('settings.filmstrip_view.preview_position_hint') }}</div> -->
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.grid.previewPosition">
                  <option v-for="(option, index) in filmStripViewPreviewPositionOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
          </div>

          <!-- preview -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.image_view.section_viewing') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.mouse_wheel') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.mouseWheelMode">
                <option v-for="(item, index) in wheelOptions" :key="index" :value="item.value">
                  {{ item.label }}
                </option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.navigator_view') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-32" v-model="config.settings.navigatorViewMode">
                  <option v-for="(option, index) in navigatorViewModeOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.navigator_view__size') }}</div>
              </div>
                <select class="select select-bordered select-sm min-w-32" v-model="config.settings.navigatorViewSize">
                  <option v-for="(option, index) in navigatorViewSizeOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.slide_show_transition') }}</div>
              </div>
                <select class="select select-bordered select-sm min-w-32" v-model="config.settings.slideShowTransition">
                  <option v-for="(option, index) in slideShowTransitionOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 h-8 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.auto_play_video') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.autoPlayVideo" />
            </div>
            <div class="flex items-center justify-between px-1 h-8 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_view.loop_video') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.loopVideo" />
            </div>
          </div>

        </div>

        <!-- Library Tab -->
        <div v-else-if="config.settings.tabIndex === 2" class="flex flex-col space-y-2">

          <!-- album -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.library.section_album') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.library.show_subfolder_files') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.library.show_subfolder_files_hint') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.showSubfolderFiles" />
            </div>
          </div>

          <!-- sorting -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.library.section_sorting') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.library.folder_sort') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.library.folder_sort_hint') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-40" v-model="config.settings.folderSort">
                <option v-for="option in folderSortOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.library.calendar_sort') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.library.calendar_sort_hint') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-40" v-model="config.settings.calendarSort">
                <option v-for="option in calendarSortOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.library.category_sort') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.library.category_sort_hint') }}</div>
              </div>
              <select class="select select-bordered select-sm min-w-40" v-model="config.settings.categorySort">
                <option v-for="option in categorySortOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
          </div>

          <!-- storage -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.database.section_storage') }}</span>
            </div>

            <!-- current location -->
            <div class="flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="min-w-0 flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.database.current_location') }}</div>
                <div class="text-xs text-base-content/30 truncate" :title="dbStorageDir || ''">
                  {{ hasCustomDbStorage ? (dbStorageDir || '-') : $t('settings.database.system_default') }}
                </div>
              </div>
              <div class="shrink-0 flex items-center gap-2">
                <button
                  class="btn btn-sm btn-ghost rounded-box bg-base-100 border border-base-content/30 text-base-content/70 hover:text-base-content"
                  :disabled="isChangingDbStorage"
                  @click="selectDbStorageDir"
                >
                  {{ isChangingDbStorage ? $t('tooltip.loading') : $t('settings.database.change_location') }}
                </button>
                <TButton
                  v-if="hasCustomDbStorage"
                  :icon="IconRestore"
                  :buttonSize="'small'"
                  :disabled="isChangingDbStorage"
                  :tooltip="$t('settings.database.restore_default_location')"
                  @click="restoreDefaultDbStorageDir"
                />
              </div>
            </div>

            <!-- backup / restore buttons -->
            <div class="flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.database.backup_title') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.database.backup_hint') }}</div>
              </div>
              <button
                class="btn btn-sm btn-ghost rounded-box bg-base-100 border border-base-content/30 text-base-content/70 hover:text-base-content"
                @click="showBackupDialog = true"
              >
                {{ $t('settings.database.backup') }}
              </button>
            </div>

            <div class="flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.database.restore_title') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.database.restore_hint') }}</div>
              </div>
              <button
                class="btn btn-sm btn-ghost rounded-box bg-base-100 border border-base-content/30 text-base-content/70 hover:text-base-content"
                @click="showRestoreDialog = true"
              >
                {{ $t('settings.database.restore') }}
              </button>
            </div>
          </div>
        </div>

        <!-- Image Search Tab -->
        <div v-else-if="config.settings.tabIndex === 3" class="flex flex-col overflow-hidden space-y-2">

          <!-- image search -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.image_search.search_image') }}</span>
            </div>
            <div class="flex items-start justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="min-w-0 flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_search.search_model') }}</div>
                <div class="text-xs text-base-content/30">
                  {{ imageSearchModelHint }}
                </div>
              </div>
              <select
                class="select select-bordered select-sm min-w-36 shrink-0"
                :value="config.settings.imageSearch.model"
                :disabled="isDownloadingMultilingualModel"
                @change="onImageSearchModelChange"
              >
                <option
                  v-for="option in imageSearchModelOptions"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </option>
              </select>
            </div>
            <div v-if="isDownloadingMultilingualModel" class="px-1 pt-1 space-y-1">
              <div class="flex items-center justify-between text-xs text-base-content/30">
                <span>{{ $t('settings.image_search.downloading_multilingual_model') }}</span>
                <span>{{ multilingualModelDownloadSizeText }}</span>
              </div>
              <div class="flex items-center gap-2">
                <progress
                  class="progress progress-primary h-1.5 flex-1"
                  :value="multilingualModelDownloadProgress"
                  max="100"
                ></progress>
                <button
                  class="btn btn-ghost btn-xs h-6 min-h-0 w-6 p-0 text-base-content/30 hover:text-base-content"
                  :title="$t('msgbox.cancel')"
                  :aria-label="$t('msgbox.cancel')"
                  @click="cancelMultilingualModelDownload"
                >
                  <IconClose class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          </div>

          <!-- find similar -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.image_search.find_similar') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div>{{ $t('settings.image_search.similarity') }}</div>
                <div class="text-xs text-base-content/30">{{ $t('settings.image_search.similarity_hint') }}</div>
              </div>
                <select class="select select-bordered select-sm min-w-32" v-model="config.settings.imageSearch.thresholdIndex">
                  <option v-for="(option, index) in similarityOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
          </div>

          <!-- face recognition -->
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('settings.face_recognition.title') }}</span>
            </div>
            <div class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div class="flex items-center">
                  <div>{{ $t('settings.face_recognition.enable') }}</div>
                  <span class="ml-2 px-1.5 h-5 inline-flex items-center rounded-box text-[10px] font-semibold tracking-[0.08em] text-warning border border-warning/30 bg-warning/10 cursor-default">
                    BETA
                  </span>
                </div>
                <div class="text-xs text-base-content/30">{{ $t('settings.face_recognition.beta_hint') }}</div>
              </div>
              <input type="checkbox" class="toggle toggle-primary toggle-sm" v-model="config.settings.face.enabled" />
            </div>
            <div v-if="config.settings.face.enabled" class="flex items-center justify-between px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200">
              <div class="flex flex-col gap-0.5 text-sm leading-5">
                <div class="flex items-center">
                  <div>{{ $t('settings.face_recognition.similarity') }}</div>
                  <span class="ml-2 px-1.5 h-5 inline-flex items-center rounded-box text-[10px] font-semibold tracking-[0.08em] text-warning border border-warning/30 bg-warning/10 cursor-default">
                    BETA
                  </span>
                </div>
                <div class="text-xs text-base-content/30">{{ $t('settings.face_recognition.cluster_threshold_hint') }}</div>
              </div>
                <select class="select select-bordered select-sm min-w-32" v-model="config.settings.face.clusterThresholdIndex" :disabled="!config.settings.face.enabled">
                  <option v-for="(option, index) in faceClusterOptions" :key="index" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
          </div>
        </div>

        <!-- Shortcuts Tab -->
        <div v-else-if="config.settings.tabIndex === 4" class="flex flex-col space-y-2">
          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center justify-between gap-4">
              <div class="min-w-0 flex flex-col gap-0.5">
                <span class="font-bold uppercase text-[10px] tracking-widest text-base-content/30">{{ pluginText('pluginStoreLocation') }}</span>
                <span class="text-xs text-base-content/30">{{ pluginText('pluginStoreHint') }}</span>
              </div>
              <div class="shrink-0 flex items-center gap-1">
                <TButton
                  :icon="IconFolder"
                  :buttonSize="'small'"
                  :disabled="!aiPluginStoreInfo?.path"
                  :tooltip="pluginText('openPluginStore')"
                  @click="openPluginPath(aiPluginStoreInfo?.path)"
                />
                <TButton
                  :icon="IconFolder"
                  :buttonSize="'small'"
                  :disabled="isChangingAiPluginStore || Boolean(aiPluginStoreInfo?.envOverride)"
                  :tooltip="pluginText('changePluginStore')"
                  @click="chooseAiPluginStoreDir"
                />
                <TButton
                  :icon="IconRestore"
                  :buttonSize="'small'"
                  :disabled="isChangingAiPluginStore || !aiPluginStoreInfo?.usingCustom || Boolean(aiPluginStoreInfo?.envOverride)"
                  :tooltip="pluginText('resetPluginStore')"
                  @click="resetAiPluginStoreLocation"
                />
              </div>
            </div>
            <div v-if="aiPluginStoreInfo" class="space-y-1 text-xs">
              <div class="min-w-0 flex gap-2 px-1">
                <span class="shrink-0 text-base-content/30">{{ pluginText('activePath') }}</span>
                <span class="truncate" :title="aiPluginStoreInfo.path">{{ aiPluginStoreInfo.path }}</span>
              </div>
              <div v-if="aiPluginStoreInfo.defaultPath" class="min-w-0 flex gap-2 px-1">
                <span class="shrink-0 text-base-content/30">{{ pluginText('defaultPath') }}</span>
                <span class="truncate" :title="aiPluginStoreInfo.defaultPath">{{ aiPluginStoreInfo.defaultPath }}</span>
              </div>
              <div v-if="aiPluginStoreInfo.envOverride" class="px-1 text-warning/80 break-all">
                {{ pluginText('pluginStoreEnvOverride', { path: aiPluginStoreInfo.envOverride }) }}
              </div>
              <div v-else class="px-1 text-base-content/30">
                {{ pluginText('pluginStoreChangeWarning') }}
              </div>
            </div>
            <div v-else class="px-1 py-2 text-sm text-base-content/30">
              {{ $t('tooltip.loading') }}
            </div>
          </div>

          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ pluginText('trustedPublishers') }}</span>
            </div>
            <div v-if="aiPluginTrustedPublishers.length === 0" class="px-1 text-xs text-base-content/30">
              {{ pluginText('noTrustedPublishers') }}
            </div>
            <div v-else class="space-y-1">
              <div
                v-for="tp in aiPluginTrustedPublishers"
                :key="tp.publisher"
                class="flex items-center justify-between gap-2 px-1 py-0.5 rounded-box bg-base-100/30"
              >
                <div class="min-w-0 flex flex-col">
                  <span class="text-xs text-base-content/60 truncate">{{ tp.publisher }}</span>
                  <span class="text-[10px] text-base-content/30 truncate" :title="tp.publicKey">{{ tp.publicKey.slice(0, 32) }}...</span>
                </div>
                <TButton
                  :icon="IconTrash"
                  :buttonSize="'small'"
                  :tooltip="pluginText('removeTrustedPublisher')"
                  @click="removeAiPluginTrustedPublisher(tp.publisher)"
                />
              </div>
            </div>
          </div>

          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center justify-between gap-4">
              <div class="flex items-center gap-2 text-base-content/30">
                <span class="font-bold uppercase text-[10px] tracking-widest">{{ pluginText('hostEnvironment') }}</span>
              </div>
              <TButton
                :icon="IconRefresh"
                :buttonSize="'small'"
                :disabled="isLoadingAiPluginHostEnvironment"
                :tooltip="pluginText('refreshHostEnvironment')"
                @click="loadAiPluginHostEnvironment"
              />
            </div>

            <div v-if="aiPluginHostEnvironment" class="space-y-2 text-xs">
              <div class="grid grid-cols-1 md:grid-cols-2 gap-x-4 gap-y-1">
                <div class="min-w-0 flex gap-2">
                  <span class="shrink-0 text-base-content/30">{{ pluginText('platform') }}</span>
                  <span class="truncate">{{ aiPluginHostEnvironment.platform }}</span>
                </div>
                <div class="min-w-0 flex gap-2">
                  <span class="shrink-0 text-base-content/30">{{ pluginText('candidateBackends') }}</span>
                  <span class="truncate">{{ formatPluginBackends(aiPluginHostEnvironment.candidateBackends) }}</span>
                </div>
              </div>

              <div v-if="aiPluginHostEnvironment.gpus.length > 0" class="flex flex-wrap gap-1">
                <span
                  v-for="gpu in aiPluginHostEnvironment.gpus"
                  :key="`${gpu.vendor}:${gpu.name}`"
                  class="max-w-full px-1.5 py-0.5 rounded-box bg-base-100/40 border border-base-content/10 text-base-content/50 truncate"
                  :title="`${gpu.name} - ${formatPluginBackends(gpu.backendCandidates)}`"
                >
                  {{ gpu.name }} · {{ gpu.vendor }} · {{ formatPluginBackends(gpu.backendCandidates) }}
                </span>
              </div>
              <div v-else class="text-base-content/30">
                {{ pluginText('noGpusDetected') }}
              </div>

              <div v-if="aiPluginHostEnvironment.pythonRuntimes?.length" class="space-y-1">
                <span class="font-bold uppercase text-[10px] tracking-widest text-base-content/30">{{ pluginText('pythonRuntimes') }}</span>
                <div class="flex flex-wrap gap-1">
                  <span
                    v-for="runtime in aiPluginHostEnvironment.pythonRuntimes"
                    :key="runtime.id"
                    class="max-w-full px-1.5 py-0.5 rounded-box border truncate"
                    :class="runtime.available ? 'bg-base-100/40 border-base-content/10 text-base-content/50' : 'bg-warning/10 border-warning/20 text-warning'"
                    :title="runtime.error || runtime.python"
                  >
                    {{ runtime.label }} / {{ runtime.version || runtime.python }}
                  </span>
                </div>
              </div>

              <div v-if="aiPluginHostEnvironment.probeError" class="text-warning/80 break-all">
                {{ aiPluginHostEnvironment.probeError }}
              </div>
            </div>
            <div v-else class="px-1 py-2 text-sm text-base-content/30">
              {{ isLoadingAiPluginHostEnvironment ? $t('tooltip.loading') : pluginText('hostEnvironmentUnavailable') }}
            </div>
          </div>

          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center justify-between gap-4">
              <div class="flex items-center gap-2 text-base-content/30">
                <span class="font-bold uppercase text-[10px] tracking-widest">{{ pluginText('directories') }}</span>
              </div>
              <div class="shrink-0 flex items-center gap-1">
                <TButton
                  :icon="IconRefresh"
                  :buttonSize="'small'"
                  :disabled="isLoadingAiPlugins"
                  :tooltip="pluginText('refresh')"
                  @click="loadAiPluginPanel(true)"
                />
                <TButton
                  :icon="IconDownload"
                  :buttonSize="'small'"
                  :disabled="isLoadingAiPlugins"
                  :tooltip="pluginText('installPackage')"
                  @click="chooseAiPluginPackage"
                />
                <TButton
                  :icon="IconAdd"
                  :buttonSize="'small'"
                  :disabled="isLoadingAiPlugins"
                  :tooltip="pluginText('addDirectory')"
                  @click="chooseAiPluginDirectory"
                />
              </div>
            </div>

            <div v-if="aiPluginRegistryPaths.length === 0" class="px-1 py-3 text-sm text-base-content/30">
              {{ pluginText('noDirectories') }}
            </div>
            <div
              v-for="path in aiPluginRegistryPaths"
              :key="path"
              class="min-h-9 flex items-center justify-between gap-3 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200"
            >
              <div class="min-w-0 flex items-center gap-2 text-sm">
                <IconFolder class="w-4 h-4 shrink-0 text-base-content/30" />
                <span class="truncate" :title="path">{{ path }}</span>
              </div>
              <TButton
                :icon="IconTrash"
                :buttonSize="'small'"
                :disabled="isLoadingAiPlugins"
                :tooltip="pluginText('removeDirectory')"
                @click="removeAiPluginDirectory(path)"
              />
            </div>
          </div>

          <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
            <div class="flex items-center justify-between gap-4">
              <div class="flex items-center gap-2 text-base-content/30">
                <span class="font-bold uppercase text-[10px] tracking-widest">{{ pluginText('installedPlugins') }}</span>
              </div>
              <div class="shrink-0 flex items-center gap-2 text-xs text-base-content/30">
                <span>{{ pluginSummaryText }}</span>
              </div>
            </div>

            <div v-if="isLoadingAiPlugins" class="px-1 py-3 text-sm text-base-content/30">
              {{ $t('tooltip.loading') }}
            </div>
            <div v-else-if="aiPlugins.length === 0" class="px-1 py-3 text-sm text-base-content/30">
              {{ pluginText('noPlugins') }}
            </div>
            <div v-else class="space-y-2">
              <div
                v-for="plugin in aiPlugins"
                :key="plugin.manifestPath || plugin.path"
                class="p-2 rounded-box bg-base-100/30 border border-base-content/10 space-y-2"
              >
                <div class="flex items-start justify-between gap-3">
                  <button
                    class="min-w-0 flex-1 flex items-start gap-2 text-left"
                    type="button"
                    @click="toggleAiPluginExpanded(plugin)"
                  >
                    <IconRight
                      class="mt-1 w-3.5 h-3.5 shrink-0 text-base-content/30 transition-transform"
                      :class="{ 'rotate-90': isAiPluginExpanded(plugin) }"
                    />
                    <div class="min-w-0 space-y-0.5">
                    <div class="flex items-center gap-2 min-w-0">
                      <div class="font-medium text-base-content truncate">
                        {{ plugin.name || pluginText('unnamedPlugin') }}
                      </div>
                      <span class="shrink-0 text-[10px] px-1.5 h-5 inline-flex items-center rounded-box bg-base-300/70 text-base-content/50">
                        {{ plugin.version || '-' }}
                      </span>
                    </div>
                    <div class="text-xs text-base-content/30 truncate" :title="plugin.id || plugin.manifestPath">
                      {{ plugin.id || plugin.manifestPath }}
                    </div>
                    <div class="text-xs text-base-content/30 truncate" :title="plugin.path">
                      {{ plugin.path }}
                    </div>
                  </div>
                  </button>
                  <div class="shrink-0 flex items-center gap-1">
                    <span
                      class="px-2 h-6 inline-flex items-center rounded-box text-xs font-medium"
                      :class="pluginStateClass(plugin)"
                    >
                      {{ pluginStateText(plugin) }}
                    </span>
                    <TButton
                      v-if="plugin.id && plugin.entry?.kind === 'local-http'"
                      :icon="IconPlay"
                      :buttonSize="'small'"
                      :disabled="Boolean(aiPluginRuntimeLoading[plugin.id])"
                      :tooltip="pluginText('start')"
                      @click="startAiPluginRuntime(plugin)"
                    />
                    <TButton
                      v-if="plugin.id && plugin.entry?.kind === 'local-http'"
                      :icon="IconPause"
                      :buttonSize="'small'"
                      :disabled="Boolean(aiPluginRuntimeLoading[plugin.id])"
                      :tooltip="pluginText('stop')"
                      @click="stopAiPluginRuntime(plugin)"
                    />
                    <TButton
                      v-if="plugin.id && plugin.entry?.kind === 'local-http'"
                      :icon="IconRestore"
                      :buttonSize="'small'"
                      :disabled="Boolean(aiPluginRuntimeLoading[plugin.id])"
                      :tooltip="pluginText('restart')"
                      @click="restartAiPluginRuntime(plugin)"
                    />
                    <TButton
                      v-if="plugin.id"
                      :icon="IconRefresh"
                      :buttonSize="'small'"
                      :disabled="Boolean(aiPluginStatusLoading[plugin.id])"
                      :tooltip="pluginText('refreshStatus')"
                      @click="refreshAiPluginStatus(plugin)"
                    />
                    <TButton
                      v-if="plugin.id"
                      :icon="IconInformation"
                      :buttonSize="'small'"
                      :disabled="Boolean(aiPluginDiagnosticsLoading[plugin.id])"
                      :tooltip="pluginText('diagnostics')"
                      @click="refreshAiPluginDiagnostics(plugin)"
                    />
                    <TButton
                      v-if="plugin.id"
                      :icon="IconFileInfo"
                      :buttonSize="'small'"
                      :disabled="Boolean(aiPluginLogsLoading[plugin.id])"
                      :tooltip="pluginText('logs')"
                      @click="refreshAiPluginLogs(plugin)"
                    />
                    <TButton
                      v-if="plugin.id"
                      :icon="IconTrash"
                      :buttonSize="'small'"
                      :disabled="isLoadingAiPlugins || Boolean(aiPluginRuntimeLoading[plugin.id])"
                      :tooltip="pluginText('uninstallPlugin')"
                      @click="uninstallInstalledAiPlugin(plugin)"
                    />
                  </div>
                </div>

                <div v-if="isAiPluginExpanded(plugin)" class="space-y-2">
                <div v-if="plugin.publisher || plugin.entry || plugin.install" class="grid grid-cols-1 md:grid-cols-2 gap-x-4 gap-y-1 text-xs">
                  <div v-if="plugin.publisher" class="min-w-0 flex gap-2">
                    <span class="shrink-0 text-base-content/30">{{ pluginText('publisher') }}</span>
                    <span class="truncate">{{ plugin.publisher }}</span>
                  </div>
                  <div v-if="plugin.entry" class="min-w-0 flex gap-2">
                    <span class="shrink-0 text-base-content/30">{{ pluginText('entry') }}</span>
                    <span class="truncate" :title="plugin.entry.baseUrl || plugin.entry.startCommand || ''">
                      {{ plugin.entry.kind }}{{ plugin.entry.baseUrl ? ` - ${plugin.entry.baseUrl}` : '' }}
                    </span>
                  </div>
                  <div v-if="pluginRuntimeUrl(plugin)" class="min-w-0 flex gap-2">
                    <span class="shrink-0 text-base-content/30">Runtime</span>
                    <span class="truncate" :title="pluginRuntimeUrl(plugin)">
                      {{ pluginRuntimeUrl(plugin) }}
                    </span>
                  </div>
                  <div v-if="plugin.install" class="min-w-0 flex gap-2">
                    <span class="shrink-0 text-base-content/30">{{ pluginText('install') }}</span>
                    <span class="truncate" :title="plugin.install.command || ''">
                      {{ formatAiPluginInstall(plugin.install) }}
                    </span>
                  </div>
                </div>

                <div v-if="pluginStorageRows(plugin).length" class="space-y-1 rounded-box bg-base-300/30 border border-base-content/10 p-2 text-xs">
                  <div class="flex items-center justify-between gap-3">
                    <div class="text-[10px] uppercase tracking-widest text-base-content/30">
                      {{ pluginText('storage') }}
                    </div>
                    <button
                      v-if="plugin.storage?.storeDir"
                      class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                      :title="plugin.storage.storeDir"
                      @click="openPluginPath(plugin.storage.storeDir)"
                    >
                      {{ pluginText('openPluginStore') }}
                    </button>
                  </div>
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-1">
                    <div
                      v-for="row in pluginStorageRows(plugin)"
                      :key="row.key"
                      class="min-w-0 flex items-center justify-between gap-2 rounded-box bg-base-100/20 border border-base-content/5 px-2 py-1"
                    >
                      <div class="min-w-0 flex items-center gap-1.5">
                        <IconFolder class="w-3.5 h-3.5 shrink-0 text-base-content/30" />
                        <span class="shrink-0 text-base-content/30">{{ row.label }}</span>
                        <span class="truncate text-base-content/50" :title="row.path">{{ row.path }}</span>
                      </div>
                      <button
                        class="shrink-0 px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        :title="row.path"
                        @click="openPluginPath(row.path)"
                      >
                        {{ pluginText('open') }}
                      </button>
                    </div>
                  </div>
                </div>

                <div class="space-y-1 rounded-box bg-base-300/30 border border-base-content/10 p-2 text-xs">
                  <div class="flex items-center justify-between gap-3">
                    <div class="text-[10px] uppercase tracking-widest text-base-content/30">
                      Privacy
                    </div>
                    <button
                      v-if="plugin.permissionGrant"
                      class="px-1.5 py-0.5 rounded-box border border-warning/20 bg-warning/10 text-warning hover:bg-warning/20"
                      :title="pluginText('revokePrivacyGrantHint')"
                      @click="revokePluginPrivacyGrant(plugin)"
                    >
                      {{ pluginText('revokePrivacyGrant') }}
                    </button>
                  </div>
                  <div class="text-base-content/55">
                    {{ pluginPrivacySummary(plugin) }}
                  </div>
                  <div class="text-base-content/35">
                    Domains: {{ pluginAllowedDomainsText(plugin) }}
                  </div>
                  <div class="text-base-content/35">
                    {{ pluginPermissionGrantSummary(plugin) }}
                  </div>
                </div>

                <div v-if="plugin.runtime || plugin.installProfiles?.length || plugin.smokeTest" class="space-y-1 rounded-box bg-base-300/30 border border-base-content/10 p-2 text-xs">
                  <div class="flex items-center justify-between gap-3">
                    <div class="text-[10px] uppercase tracking-widest text-base-content/30">
                      {{ pluginText('runtimeProfiles') }}
                    </div>
                    <span class="shrink-0 text-base-content/40">
                      {{ pluginRecommendedProfileText(plugin) }}
                    </span>
                  </div>
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-x-4 gap-y-1">
                    <div v-if="plugin.runtime" class="min-w-0 flex gap-2">
                      <span class="shrink-0 text-base-content/30">{{ pluginText('runtime') }}</span>
                      <span class="truncate" :title="plugin.runtime.notes || ''">
                        {{ formatAiPluginRuntime(plugin.runtime) }}
                      </span>
                    </div>
                    <div v-if="plugin.smokeTest" class="min-w-0 flex gap-2">
                      <span class="shrink-0 text-base-content/30">{{ pluginText('smokeTest') }}</span>
                      <span class="truncate" :title="formatAiPluginSmokeTest(plugin.smokeTest)">
                        {{ formatAiPluginSmokeTest(plugin.smokeTest) }}
                      </span>
                    </div>
                  </div>
                  <div v-if="plugin.installProfiles?.length" class="space-y-1">
                    <div
                      v-for="profile in visiblePluginProfiles(plugin)"
                      :key="profile.id"
                      class="flex flex-wrap items-center gap-1"
                    >
                      <span
                        class="max-w-full px-1.5 py-0.5 rounded-box border text-base-content/50 truncate"
                        :class="profileBadgeClass(plugin, profile)"
                        :title="`${formatAiPluginInstallProfile(profile)} - ${profileEnvironmentStatus(plugin, profile).title}`"
                      >
                        {{ formatAiPluginInstallProfile(profile) }}
                        <span class="opacity-70">/ {{ profileEnvironmentStatus(plugin, profile).label }}</span>
                      </span>
                      <select
                        v-if="profileRuntimeBindingOptions(profile).length > 1"
                        v-show="isAiPluginAdvancedExpanded(plugin)"
                        class="select select-bordered select-xs max-w-56 bg-base-100/40 text-base-content/60"
                        :value="selectedRuntimeBindingKey(plugin, profile)"
                        @change="setRuntimeBindingSelection(plugin, profile, ($event.target as HTMLSelectElement).value)"
                      >
                        <option
                          v-for="(binding, bindingIndex) in profileRuntimeBindingOptions(profile)"
                          :key="runtimeBindingKey(binding, bindingIndex)"
                          :value="runtimeBindingKey(binding, bindingIndex)"
                        >
                          {{ bindingOptionLabel(plugin, profile, binding) }}
                        </option>
                      </select>
                      <span
                        v-if="selectedRuntimeBinding(plugin, profile)?.scope && (!isAiPluginAdvancedExpanded(plugin) || profileRuntimeBindingOptions(profile).length <= 1)"
                        class="max-w-full px-1.5 py-0.5 rounded-box border truncate"
                        :class="runtimeBindingBadgeClass(selectedRuntimeBinding(plugin, profile))"
                        :title="selectedRuntimeBinding(plugin, profile)?.python || selectedRuntimeBinding(plugin, profile)?.root || selectedRuntimeBinding(plugin, profile)?.notes || ''"
                      >
                        {{ formatRuntimeBinding(selectedRuntimeBinding(plugin, profile)) }}
                      </span>
                      <div
                        v-if="profileRuntimePathChip(plugin, profile)"
                        class="max-w-full inline-flex items-center gap-1 px-1.5 py-0.5 rounded-box border truncate"
                        :class="runtimeBindingBadgeClass(selectedRuntimeBinding(plugin, profile))"
                        :title="profile.resolvedRuntimeDir || ''"
                      >
                        <IconFolder class="w-3 h-3 shrink-0 opacity-70" />
                        <span class="truncate">{{ shortRuntimePath(plugin, profile) }}</span>
                        <button
                          v-if="profile.resolvedRuntimeDir"
                          class="shrink-0 px-1 rounded hover:bg-base-content/10"
                          :title="pluginText('openRuntimeFolder')"
                          @click="openPluginPath(profile.resolvedRuntimeDir || undefined)"
                        >{{ pluginText('open') }}</button>
                        <span
                          v-if="condensedRuntimeVersions(plugin, profile)"
                          class="opacity-70 truncate border-l border-base-content/10 pl-1"
                          :title="condensedRuntimeVersions(plugin, profile)"
                        >{{ condensedRuntimeVersions(plugin, profile) }}</span>
                      </div>
                      <button
                        v-if="isAiPluginAdvancedExpanded(plugin) && selectedRuntimeBinding(plugin, profile)?.python"
                        class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        :disabled="Boolean(aiPluginProfileActionLoading[profileActionKey(plugin, profile, 'probe')])"
                        :title="selectedRuntimeBinding(plugin, profile)?.python || ''"
                        @click="probeAiPluginProfileRuntime(plugin, profile)"
                      >
                        {{ pluginText('probeRuntime') }}
                      </button>
                      <button
                        v-if="isAiPluginAdvancedExpanded(plugin)"
                        class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        :disabled="Boolean(aiPluginProfileActionLoading[profileActionKey(plugin, profile, 'setup')])"
                        @click="setupAiPluginProfile(plugin, profile)"
                      >
                        {{ pluginText('setupProfile') }}
                      </button>
                      <button
                        v-if="plugin.install?.command"
                        class="px-1.5 py-0.5 rounded-box border border-warning/20 bg-warning/10 text-warning hover:bg-warning/20"
                        :disabled="Boolean(aiPluginProfileActionLoading[profileActionKey(plugin, profile, 'runSetup')])"
                        :title="plugin.install.command"
                        @click="runAiPluginProfileSetup(plugin, profile)"
                      >
                        {{ pluginText('runSetupProfile') }}
                      </button>
                      <button
                        v-if="isAiPluginAdvancedExpanded(plugin)"
                        class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        :disabled="Boolean(aiPluginProfileActionLoading[profileActionKey(plugin, profile, 'verify')])"
                        @click="verifyAiPluginProfile(plugin, profile)"
                      >
                        {{ pluginText('verifyProfile') }}
                      </button>
                      <button
                        class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        :disabled="Boolean(aiPluginProfileActionLoading[profileActionKey(plugin, profile, 'smoke')])"
                        @click="runAiPluginProfileSmokeTest(plugin, profile)"
                      >
                        {{ pluginText('smokeTestProfile') }}
                      </button>
                      <div
                        v-if="aiPluginProfileActionLoading[profileActionKey(plugin, profile, 'smoke')]"
                        class="w-full mt-1 rounded-box bg-base-300/40 border border-primary/15 px-2 py-1 text-[11px] text-primary/70"
                      >
                        <div class="flex items-center justify-between gap-2">
                          <span class="truncate">{{ smokeTestRunningText(plugin) }}</span>
                          <span class="shrink-0">{{ pluginText('smokeTestRunningBadge') }}</span>
                        </div>
                        <progress class="progress progress-primary w-full mt-1"></progress>
                      </div>
                      <div
                        v-if="profile.setupJob"
                        class="w-full mt-1 rounded-box bg-base-300/40 border border-base-content/10 px-2 py-1 text-[11px] text-base-content/45"
                      >
                        <div class="flex items-center justify-between gap-2">
                          <span class="truncate" :title="profile.setupJob.message || ''">
                            {{ formatSetupJobSummary(profile.setupJob) }}
                          </span>
                          <div class="flex items-center gap-1.5 shrink-0">
                            <span>{{ Math.round(profile.setupJob.progress || 0) }}%</span>
                            <button
                              v-if="profile.setupJob.status === 'running' && aiPluginSetupRunningFor"
                              class="px-1.5 py-0.5 rounded-box border border-warning/20 bg-warning/10 text-warning hover:bg-warning/20"
                              @click="cancelAiPluginProfileSetup"
                            >
                              {{ pluginText('cancelSetup') }}
                            </button>
                          </div>
                        </div>
                        <progress
                          v-if="profile.setupJob.status === 'running'"
                          class="progress progress-primary w-full mt-1"
                          :value="profile.setupJob.progress || 0"
                          max="100"
                        ></progress>
                        <div v-if="profile.setupJob.log?.length" class="mt-1 space-y-1">
                          <div class="flex items-center justify-end">
                            <TButton
                              :icon="IconCopy"
                              :buttonSize="'small'"
                              :tooltip="pluginText('copySetupLog')"
                              @click="copyAiPluginSetupLog(profile)"
                            />
                          </div>
                          <div class="max-h-32 overflow-auto font-mono whitespace-pre-wrap break-all select-text cursor-text">
                            {{ profile.setupJob.log.join('\n') }}
                          </div>
                        </div>
                      </div>
                      <div
                        v-if="profileRuntimeProbeResult(plugin, profile)"
                        class="w-full mt-1 rounded-box bg-base-300/40 border px-2 py-1 text-[11px] space-y-1"
                        :class="runtimeProbeCardClass(profileRuntimeProbeResult(plugin, profile))"
                      >
                        <div class="flex items-center justify-between gap-2">
                          <span
                            class="shrink-0 px-1.5 py-0.5 rounded-box border"
                            :class="runtimeProbeBadgeClass(profileRuntimeProbeResult(plugin, profile))"
                          >
                            {{ runtimeProbeStatusLabel(profileRuntimeProbeResult(plugin, profile)) }}
                          </span>
                          <span class="min-w-0 truncate text-base-content/35" :title="runtimeProbeCacheTitle(profileRuntimeProbeResult(plugin, profile))">
                            {{ runtimeProbeCacheText(profileRuntimeProbeResult(plugin, profile)) }}
                          </span>
                        </div>
                        <div class="text-base-content/55">
                          {{ runtimeProbeSummary(profileRuntimeProbeResult(plugin, profile), profile.backend) }}
                        </div>
                        <div class="space-y-1.5 text-base-content/40">
                          <div
                            v-for="group in runtimeProbeDetailGroups(profileRuntimeProbeResult(plugin, profile))"
                            :key="group.groupKey"
                            class="space-y-0.5"
                          >
                            <div class="text-[10px] font-semibold uppercase tracking-wider text-base-content/30">
                              {{ pluginText(group.groupKey) }}
                            </div>
                            <div class="grid grid-cols-1 md:grid-cols-2 gap-x-3 gap-y-0.5">
                              <div
                                v-for="item in group.items"
                                :key="item.label"
                                class="min-w-0 flex gap-1"
                              >
                                <span class="shrink-0 text-base-content/30">{{ item.label }}</span>
                                <span
                                  class="truncate"
                                  :class="item.tone === 'ok' ? 'text-success/70' : item.tone === 'bad' ? 'text-error/70' : ''"
                                  :title="item.title || item.value"
                                >{{ item.value }}</span>
                              </div>
                            </div>
                          </div>
                        </div>
                        <div
                          v-if="profileRuntimeConflicts(profile).length"
                          class="space-y-0.5 border-t border-warning/20 pt-1"
                        >
                          <div
                            v-for="conflict in profileRuntimeConflicts(profile)"
                            :key="conflict.package"
                            :class="conflict.kind === 'unprobed'
                              ? 'text-base-content/40'
                              : 'text-warning/90'"
                            :title="conflict.kind === 'unprobed' ? '' : conflict.message"
                          >
                            <span class="mr-0.5">{{ conflict.kind === 'unprobed' ? '○' : '⚠' }}</span>{{ conflict.message }}
                          </div>
                          <div
                            v-if="profileRuntimeConflicts(profile).some((c) => c.kind === 'version_mismatch' || c.kind === 'missing')"
                            class="text-primary/70 font-medium"
                          >
                            <span class="mr-0.5">→</span>{{ pluginText('runtimeConflictAdvice') }}
                          </div>
                        </div>
                        <div
                          v-if="runtimeProbeAdvice(plugin, profile, profileRuntimeProbeResult(plugin, profile)).length"
                          class="space-y-0.5 border-t border-base-content/10 pt-1"
                        >
                          <div
                            v-for="(advice, idx) in runtimeProbeAdvice(plugin, profile, profileRuntimeProbeResult(plugin, profile))"
                            :key="idx"
                            :class="advice.kind === 'action'
                              ? 'text-primary/70 font-medium'
                              : 'text-base-content/35'"
                          >
                            <span v-if="advice.kind === 'action'" class="mr-0.5">→</span>{{ advice.text }}
                          </div>
                        </div>
                      </div>
                    </div>
                    <button
                      v-if="hiddenPluginProfileCount(plugin) > 0 || isAiPluginAdvancedExpanded(plugin)"
                      class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/30 text-base-content/40 hover:text-base-content"
                      @click="toggleAiPluginAdvanced(plugin)"
                    >
                      {{ pluginText('advancedRuntimeOptions') }}{{ hiddenPluginProfileCount(plugin) > 0 && !isAiPluginAdvancedExpanded(plugin) ? ` (${hiddenPluginProfileCount(plugin)})` : '' }}
                    </button>
                  </div>
                  <div v-if="pluginSmokeResultProfiles(plugin).length > 0" class="space-y-1">
                    <div
                      v-for="profile in pluginSmokeResultProfiles(plugin)"
                      :key="`smoke:${profile.id}`"
                      class="max-h-36 overflow-auto rounded-box bg-base-300/60 p-2 font-mono text-[11px] leading-4 text-base-content/50 whitespace-pre-wrap break-all"
                    >
                      {{ formatAiPluginStatus(aiPluginProfileSmokeResults[profileResultKey(plugin, profile)].result || aiPluginProfileSmokeResults[profileResultKey(plugin, profile)]) }}
                    </div>
                  </div>
                </div>

                <div v-if="plugin.capabilities.length > 0" class="flex flex-wrap gap-1">
                  <span
                    v-for="capability in plugin.capabilities"
                    :key="capability.id"
                    class="max-w-full px-2 py-1 rounded-box bg-primary/10 border border-primary/20 text-xs text-primary inline-flex items-center gap-1"
                    :title="`${capability.id} - ${capability.kind}`"
                  >
                    {{ capability.name || capability.id }}
                    <span class="text-primary/60">/ {{ capability.kind }}</span>
                    <button
                      class="ml-1 px-1 rounded-box text-primary/60 hover:text-primary hover:bg-primary/10"
                      :title="pluginText('testInvoke')"
                      :aria-label="pluginText('testInvoke')"
                      :disabled="Boolean(aiPluginInvokeLoading[`${plugin.id}:${capability.id}`])"
                      @click.stop="testInvokeAiPluginCapability(plugin, capability)"
                    >
                      {{ pluginText('test') }}
                    </button>
                  </span>
                </div>
                <div v-else class="text-xs text-base-content/30">
                  {{ pluginText('noCapabilities') }}
                </div>

                <div v-if="plugin.contributes?.menus?.length" class="flex flex-wrap gap-1">
                  <span
                    v-for="menu in plugin.contributes.menus"
                    :key="menu.id"
                    class="max-w-full px-2 py-1 rounded-box bg-base-100/60 border border-base-content/10 text-xs text-base-content/50 inline-flex items-center gap-1"
                    :title="`${menu.id} - ${menu.placements.join(', ')}`"
                  >
                    {{ menu.label || menu.id }}
                    <span class="text-base-content/30">/ {{ menu.contexts.join(', ') }}</span>
                  </span>
                </div>

                <div v-if="plugin.validation.errors.length > 0" class="space-y-1">
                  <div
                    v-for="error in plugin.validation.errors"
                    :key="error"
                    class="text-xs text-error"
                  >
                    {{ error }}
                  </div>
                </div>
                <div v-if="plugin.validation.warnings.length > 0" class="space-y-1">
                  <div
                    v-for="warning in plugin.validation.warnings"
                    :key="warning"
                    class="text-xs text-warning"
                  >
                    {{ warning }}
                  </div>
                </div>

                <div v-if="plugin.taskStates?.length" class="space-y-1 rounded-box bg-base-300/30 border border-base-content/10 p-2 text-xs">
                  <div class="text-[10px] uppercase tracking-widest text-base-content/30">
                    {{ pluginText('recentTasks') }}
                  </div>
                  <div
                    v-for="task in plugin.taskStates"
                    :key="task.taskId"
                    class="flex items-start justify-between gap-2 rounded-box bg-base-100/30 border border-base-content/5 px-2 py-1.5 text-base-content/45"
                  >
                    <div class="min-w-0 flex-1 space-y-1" :title="formatPluginTaskTitle(task)">
                      <div class="flex min-w-0 items-center gap-1.5">
                        <span
                          class="shrink-0 px-1.5 h-5 inline-flex items-center rounded-box border text-[10px] font-medium"
                          :class="pluginTaskBadgeClass(task)"
                        >
                          {{ pluginTaskStatusLabel(task) }}
                        </span>
                        <span class="min-w-0 truncate text-base-content/60">
                          {{ task.capabilityId }}
                        </span>
                        <span v-if="typeof task.progress === 'number'" class="shrink-0 tabular-nums">
                          {{ task.progress }}%
                        </span>
                        <span class="shrink-0">
                          {{ pluginTaskOutputCount(task) }}
                        </span>
                      </div>
                      <div v-if="formatPluginTaskDetail(task)" class="truncate text-[11px] text-base-content/35">
                        {{ formatPluginTaskDetail(task) }}
                      </div>
                    </div>
                    <div class="shrink-0 flex items-center gap-1">
                      <button
                        class="inline-flex h-6 w-6 items-center justify-center rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content disabled:opacity-40"
                        :title="pluginText('refreshTask')"
                        :disabled="Boolean(aiPluginTaskLoading[`${plugin.id}:${task.taskId}`])"
                        @click="refreshAiPluginTask(plugin, task)"
                      >
                        <IconRefresh class="h-3.5 w-3.5" />
                      </button>
                      <button
                        v-if="task.status === 'succeeded' && !task.adopted"
                        class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        @click="discardAiPluginTask(plugin, task)"
                      >
                        {{ pluginText('discardTask') }}
                      </button>
                      <button
                        v-if="task.retryable"
                        class="px-1.5 py-0.5 rounded-box border border-base-content/10 bg-base-100/40 text-base-content/40 hover:text-base-content"
                        @click="retryAiPluginTaskFromLedger(plugin, task)"
                      >
                        {{ pluginText('retryTask') }}
                      </button>
                      <button
                        v-if="['queued', 'running', 'cancelling'].includes(task.status)"
                        class="px-1.5 py-0.5 rounded-box border border-warning/20 bg-warning/10 text-warning hover:bg-warning/20"
                        @click="cancelAiPluginTaskFromLedger(plugin, task)"
                      >
                        {{ pluginText('cancelTask') }}
                      </button>
                    </div>
                  </div>
                </div>

                <div v-if="aiPluginStatuses[plugin.id]?.status" class="text-xs text-base-content/40 break-all">
                  {{ formatAiPluginStatus(aiPluginStatuses[plugin.id].status) }}
                </div>
                <div v-else-if="aiPluginStatuses[plugin.id]?.error" class="text-xs text-base-content/30 break-all">
                  {{ aiPluginStatuses[plugin.id].error }}
                </div>
                <div
                  v-if="aiPluginStartupIssue(plugin)"
                  class="space-y-1 rounded-box border border-warning/20 bg-warning/10 p-2 text-xs text-warning/90"
                >
                  <div class="flex items-center justify-between gap-2">
                    <div class="min-w-0 font-medium truncate" :title="aiPluginStartupIssue(plugin)?.error || ''">
                      {{ aiPluginStartupIssue(plugin)?.error }}
                    </div>
                    <span class="shrink-0 text-[10px] uppercase tracking-wider opacity-70">
                      {{ aiPluginStartupIssueLabel(aiPluginStartupIssue(plugin)) }}
                    </span>
                  </div>
                  <div v-if="aiPluginStartupIssue(plugin)?.advice?.length" class="space-y-0.5 text-warning/80">
                    <div
                      v-for="advice in aiPluginStartupIssue(plugin)?.advice"
                      :key="advice"
                    >
                      {{ advice }}
                    </div>
                  </div>
                  <div v-if="aiPluginStartupIssue(plugin)?.logTail" class="space-y-1">
                    <div class="flex items-center justify-between gap-2 text-[11px] text-warning/70">
                      <span class="truncate" :title="aiPluginStartupIssue(plugin)?.logTail?.path">
                        {{ aiPluginStartupIssue(plugin)?.logTail?.name || 'start.log' }}
                      </span>
                      <span class="shrink-0">{{ formatFileSize(aiPluginStartupIssue(plugin)?.logTail?.bytes || 0) }}</span>
                    </div>
                    <pre class="max-h-32 overflow-auto rounded-box bg-base-300/60 p-2 whitespace-pre-wrap break-all font-mono text-[11px] leading-4 text-base-content/55 select-text cursor-text">{{ aiPluginStartupIssue(plugin)?.logTail?.content || pluginText('emptyLog') }}</pre>
                  </div>
                </div>

                <div v-if="aiPluginDiagnostics[plugin.id]" class="space-y-1">
                  <div class="text-[10px] uppercase tracking-widest text-base-content/30">
                    {{ pluginText('diagnostics') }}
                  </div>
                  <div v-if="aiPluginDiagnostics[plugin.id].diagnostics" class="max-h-36 overflow-auto rounded-box bg-base-300/60 p-2 font-mono text-[11px] leading-4 text-base-content/50 whitespace-pre-wrap break-all">
                    {{ formatAiPluginStatus(aiPluginDiagnostics[plugin.id].diagnostics) }}
                  </div>
                  <div v-else class="text-xs text-base-content/30 break-all">
                    {{ aiPluginDiagnostics[plugin.id].error || pluginText('diagnosticsUnavailable') }}
                  </div>
                </div>

                <div v-if="aiPluginLogs[plugin.id]" class="space-y-1">
                  <div class="text-[10px] uppercase tracking-widest text-base-content/30">
                    {{ pluginText('logs') }}
                  </div>
                  <div v-if="aiPluginLogs[plugin.id].files.length > 0" class="space-y-1">
                    <div
                      v-for="file in aiPluginLogs[plugin.id].files"
                      :key="file.path"
                      class="rounded-box bg-base-300/60 p-2"
                    >
                      <div class="mb-1 flex items-center justify-between gap-2 text-[11px] text-base-content/30">
                        <span class="truncate" :title="file.path">{{ file.name }}</span>
                        <span class="shrink-0">{{ formatFileSize(file.bytes) }}</span>
                      </div>
                      <pre class="max-h-32 overflow-auto whitespace-pre-wrap break-all font-mono text-[11px] leading-4 text-base-content/50">{{ file.content || pluginText('emptyLog') }}</pre>
                    </div>
                  </div>
                  <div v-else class="text-xs text-base-content/30 break-all">
                    {{ aiPluginLogs[plugin.id].error || pluginText('noLogs') }}
                  </div>
                </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Shortcuts Tab -->
        <div v-else-if="config.settings.tabIndex === 5" class="flex flex-col space-y-2">
          <div
            v-for="section in shortcutSections"
            :key="section.key"
            class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm"
          >
            <div class="flex items-center gap-2 text-base-content/30">
              <span class="font-bold uppercase text-[10px] tracking-widest">{{ section.title }}</span>
            </div>
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-4 gap-y-1">
              <div
                v-for="item in section.items"
                :key="item.actionId"
                class="min-h-9 flex items-center justify-between gap-4 px-1 rounded-box hover:bg-base-100/10 transition-colors duration-200"
              >
                <div class="min-w-0 text-sm leading-5 truncate">{{ item.label }}</div>
                <div class="shrink-0 flex items-center gap-1">
                  <span
                    v-for="(key, keyIndex) in item.keys"
                    :key="`${item.actionId}-${keyIndex}-${key}`"
                    class="min-w-7 h-7 px-2 inline-flex items-center justify-center rounded-box border border-base-content/10 bg-base-100/40 text-xs font-semibold text-base-content/30 shadow-sm"
                  >
                    {{ key }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- About Tab -->
        <div v-else-if="config.settings.tabIndex === 6" class="py-2">
            <SettingsAbout />
        </div>

      </div>
    </div>

    <MessageBox
      v-if="showChangeDbStorageDialog"
      :title="$t('settings.database.prechange_title')"
      :message="$t('settings.database.prechange_message')"
      :OkText="$t('settings.database.change_location_confirm')"
      :cancelText="$t('msgbox.cancel')"
      @ok="chooseDbStorageDir"
      @cancel="showChangeDbStorageDialog = false"
    />

    <MessageBox
      v-if="showResetDbStorageDialog"
      :title="$t('settings.database.restore_default_confirm_title')"
      :message="$t('settings.database.restore_default_confirm_message')"
      :OkText="$t('settings.database.restore_default_confirm_ok')"
      :cancelText="$t('msgbox.cancel')"
      @ok="confirmResetDbStorageDir"
      @cancel="showResetDbStorageDialog = false"
    />

    <BackupDialog
      v-if="showBackupDialog"
      @done="showBackupDialog = false"
      @cancel="showBackupDialog = false"
    />

    <RestoreDialog
      v-if="showRestoreDialog"
      @done="onRestoreDone"
      @cancel="showRestoreDialog = false"
    />
    <UninstallModeDialog
      v-if="uninstallModeDialog.show"
      :plugin="uninstallModeDialog.plugin"
      :path="uninstallModeDialog.path"
      @resolve="resolveUninstallMode"
    />
  </div>
</template>

<script setup lang="ts">

import { ref, watch, computed, onMounted, onUnmounted } from 'vue';
import { LogicalSize } from '@tauri-apps/api/dpi';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit } from '@tauri-apps/api/event';
import { ask, open as openDialog } from '@tauri-apps/plugin-dialog';
import { useI18n } from 'vue-i18n';
import { config, libConfig } from '@/common/config';
import {
  getExternalAppDisplayName,
  getDbStorageDir,
  changeDbStorageDir,
  resetDbStorageDir,
  isFaceIndexing,
  isUsingCustomDbStorage,
  getImageSearchModelStatus,
  setImageSearchModel,
  downloadMultilingualImageSearchModel,
  cancelMultilingualImageSearchModelDownload,
  listenImageSearchModelDownloadProgress,
  getAiPluginRegistry,
  getAiPluginStoreInfo,
  setAiPluginStoreDir,
  resetAiPluginStoreDir,
  grantAiPluginPermissions,
  getAiPluginHostEnvironment,
  probeAiPluginPythonRuntime,
  installAiPluginPackage,
  uninstallAiPlugin,
  registerAiPluginPath,
  unregisterAiPluginPath,
  listAiPlugins,
  getAiPluginStatus,
  getAiPluginDiagnostics,
  getAiPluginLogs,
  revealPath,
  startAiPlugin,
  stopAiPlugin,
  markAiPluginProfileSetupNeeded,
  previewAiPluginProfileSetupCommand,
  runAiPluginProfileSetupCommand,
  smokeTestAiPlugin,
  invokeAiPluginCapability,
  discardAiPluginTaskOutputs,
  retryAiPluginTask,
  cancelAiPluginTask,
  cancelAiPluginSetup,
  getAiPluginTask,
  revokeAiPluginPermissions,
  listTrustedPublishers,
  trustPublisher,
  removeTrustedPublisher,
} from '@/common/api';
import { formatFileSize, isLinux, isMac, setTheme, SCALE_VALUES } from '@/common/utils';
import { getShortcutLabels, ShortcutActionId, ShortcutPlatform } from '@/common/shortcuts';
import { useToast } from '@/common/toast';
import { usePluginStore } from '@/stores/pluginStore';
import { IconAdd, IconClose, IconCopy, IconDownload, IconFolder, IconPause, IconPlay, IconRefresh, IconTrash, IconRestore, IconInformation, IconFileInfo, IconRight } from '@/common/icons';

import TitleBar from '@/components/TitleBar.vue';
import SettingsAbout from '@/components/SettingsAbout.vue';
import MessageBox from '@/components/MessageBox.vue';
import BackupDialog from '@/components/BackupDialog.vue';
import RestoreDialog from '@/components/RestoreDialog.vue';
import TButton from '@/components/TButton.vue';
import UninstallModeDialog from '@/components/UninstallModeDialog.vue';
import {
  pluginAllowedDomains,
  buildPluginPermissionGrantRequest,
  missingPluginPermissionFlags,
  pluginPermissionGrant as getPluginPermissionGrant,
  pluginPermissions as getPluginPermissions,
} from '@/common/pluginRuntime';

/// i18n
const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[config.settings.language] as any);
const toast = useToast();
const pluginStore = usePluginStore();
const shortcutPlatform: ShortcutPlatform = isMac ? 'mac' : (isLinux ? 'linux' : 'windows');
const settingsTabs = [
  'settings.general.title',
  'settings.view.title',
  'settings.library.title',
  'settings.image_search.title',
  'settings.plugins.title',
  'settings.shortcuts.title',
  'settings.about.title',
];

const appWindow = getCurrentWebviewWindow()
let gridSizeEmitTimer: number | null = null;
const SETTINGS_BASE_WIDTH = 600;
const SETTINGS_BASE_HEIGHT = 620;
const dbStorageDir = ref('');
const isChangingDbStorage = ref(false);
const hasCustomDbStorage = ref(false);
const showChangeDbStorageDialog = ref(false);
const showResetDbStorageDialog = ref(false);
const showBackupDialog = ref(false);
const showRestoreDialog = ref(false);
const isDownloadingMultilingualModel = ref(false);
const isCancelingMultilingualModelDownload = ref(false);
const multilingualModelDownloadProgress = ref(0);
const multilingualModelDownloadedBytes = ref(0);
const multilingualModelTotalBytes = ref(0);
const isMultilingualModelAvailable = ref(false);
let unlistenImageSearchModelDownloadProgress: (() => void) | null = null;
const aiPluginRegistryPaths = ref<string[]>([]);
const aiPluginTrustedPublishers = ref<Array<{ publisher: string; publicKey: string; trustedAt: string }>>([]);
const aiPlugins = ref<AiPluginSummary[]>([]);
const aiPluginStoreInfo = ref<AiPluginStoreInfo | null>(null);
const expandedAiPluginKeys = ref<Record<string, boolean>>({});
const advancedAiPluginKeys = ref<Record<string, boolean>>({});
const aiPluginHostEnvironment = ref<AiPluginHostEnvironment | null>(null);
const aiPluginStatuses = ref<Record<string, AiPluginStatus>>({});
const aiPluginDiagnostics = ref<Record<string, AiPluginDiagnostics>>({});
const aiPluginLogs = ref<Record<string, AiPluginLogs>>({});
const aiPluginStatusLoading = ref<Record<string, boolean>>({});
const aiPluginDiagnosticsLoading = ref<Record<string, boolean>>({});
const aiPluginLogsLoading = ref<Record<string, boolean>>({});
const aiPluginRuntimeLoading = ref<Record<string, boolean>>({});
const aiPluginInvokeLoading = ref<Record<string, boolean>>({});
const aiPluginTaskLoading = ref<Record<string, boolean>>({});
const aiPluginProfileActionLoading = ref<Record<string, boolean>>({});
const aiPluginSetupRunningFor = ref<{ pluginId: string; profileId: string } | null>(null);
const uninstallModeDialog = ref({ show: false, plugin: '', path: '' });
let uninstallModeResolver: ((mode: 'code_only' | 'code_and_data' | 'cancel') => void) | null = null;
const aiPluginProfileSmokeResults = ref<Record<string, AiPluginSmokeTestResult>>({});
const aiPluginRuntimeProbeResults = ref<Record<string, AiPluginPythonRuntimeProbeResult>>({});
const aiPluginRuntimeBindingSelection = ref<Record<string, string>>({});
const isLoadingAiPlugins = ref(false);
const isRefreshingAiPlugins = ref(false);
const isLoadingAiPluginHostEnvironment = ref(false);
const isChangingAiPluginStore = ref(false);

type AiPluginValidation = {
  valid: boolean;
  errors: string[];
  warnings: string[];
};

type AiPluginStoreInfo = {
  path: string;
  configuredPath?: string | null;
  defaultPath: string;
  envOverride?: string | null;
  usingCustom: boolean;
};

type AiPluginCapability = {
  id: string;
  kind: string;
  name: string;
  version?: string;
  inputs?: unknown[];
  outputs?: unknown[];
  parameters?: unknown;
};

type AiPluginEntry = {
  kind: string;
  baseUrl?: string;
  startCommand?: string;
  statusPath?: string;
  healthPath?: string;
};

type AiPluginInstall = {
  kind: string;
  command?: string;
  estimatedDiskMb?: number;
  requiresAdmin: boolean;
};

type AiPluginRuntime = {
  kind: string;
  cudaApiCompatible: boolean;
  notes?: string;
};

type AiPluginRuntimeBinding = {
  scope: string;
  kind?: string;
  id?: string;
  label?: string;
  python?: string;
  root?: string;
  requirements?: string;
  notes?: string;
};

type RuntimeConflict = {
  package: string;
  declaredSpec: string;
  installedVersion: string;
  available: boolean;
  kind: string;
  message: string;
};

type AiPluginInstallProfile = {
  id: string;
  backend: string;
  label?: string;
  supportLevel: string;
  derivedFrom?: string;
  envDir?: string;
  requirements?: string;
  runtimeBinding?: AiPluginRuntimeBinding;
  runtimeBindings?: AiPluginRuntimeBinding[];
  notes?: string;
  state?: AiPluginProfileState;
  setupJob?: AiPluginSetupJob;
  runtimeProbeState?: AiPluginRuntimeProbeState;
  runtimeProbeStates?: AiPluginRuntimeProbeState[];
  runtimeConflicts?: RuntimeConflict[];
};

type AiPluginSmokeTest = {
  command?: string;
  capability?: string;
  timeoutMs?: number;
};

type AiPluginMenuContribution = {
  id: string;
  label: string;
  capability: string;
  contexts: string[];
  placements: string[];
  icon?: string;
  order: number;
};

type AiPluginContributes = {
  menus: AiPluginMenuContribution[];
};

type AiPluginNetworkPermissions = {
  runtime: boolean;
  setupDownloads: boolean;
  uploadSelectedFiles: boolean;
  uploadOutputs: boolean;
  allowedDomains: string[];
};

type AiPluginPermissions = {
  readSelectedFiles: boolean;
  writeOutputDir: boolean;
  writeSourceFiles: boolean;
  launchChildProcesses: boolean;
  network: AiPluginNetworkPermissions;
};

type AiPluginPermissionGrant = {
  pluginId: string;
  runtimeNetwork: boolean;
  setupDownloads: boolean;
  uploadSelectedFiles: boolean;
  uploadOutputs: boolean;
  allowedDomains: string[];
  updatedAt?: string;
};

type AiPluginSummary = {
  id: string;
  name: string;
  version: string;
  publisher?: string;
  path: string;
  manifestPath: string;
  platformSupported: boolean;
  validation: AiPluginValidation;
  permissions: AiPluginPermissions;
  permissionGrant?: AiPluginPermissionGrant | null;
  runtimes?: string[];
  runtime?: AiPluginRuntime;
  entry?: AiPluginEntry;
  install?: AiPluginInstall;
  storage?: AiPluginStorage;
  installProfiles?: AiPluginInstallProfile[];
  smokeTest?: AiPluginSmokeTest;
  capabilities: AiPluginCapability[];
  contributes?: AiPluginContributes;
  taskStates?: AiPluginTaskState[];
};

type AiPluginStorage = {
  storeDir: string;
  codeDir: string;
  dataDir: string;
  modelDir: string;
  modelDirs?: string[];
  logDir: string;
  configPath: string;
  runtimeDir: string;
  runtimeDirs?: string[];
  cacheDir: string;
  outputDir: string;
};

type AiPluginStatus = {
  pluginId: string;
  reachable: boolean;
  managed?: boolean;
  url?: string;
  status?: unknown;
  error?: string;
  errorCode?: string;
  errorDomain?: string;
  errorDetails?: any;
  logTail?: AiPluginLogFile;
  advice?: string[];
};

type AiPluginDiagnostics = {
  pluginId: string;
  reachable: boolean;
  url?: string;
  diagnostics?: unknown;
  error?: string;
};

type AiPluginLogFile = {
  path: string;
  name: string;
  bytes: number;
  content: string;
};

type AiPluginLogs = {
  pluginId: string;
  files: AiPluginLogFile[];
  error?: string;
};

type AiPluginSmokeTestResult = {
  pluginId: string;
  profileId: string;
  backend: string;
  capability: string;
  reachable: boolean;
  url?: string;
  passed: boolean;
  durationMs?: number;
  result?: unknown;
  error?: string;
  startupStatus?: AiPluginStatus;
};

type AiPluginPythonRuntimeProbeResult = {
  python: string;
  backend?: string;
  available: boolean;
  durationMs: number;
  result?: any;
  error?: string;
  state?: AiPluginRuntimeProbeState;
};

type AiPluginRuntimeProbeState = {
  pluginId: string;
  profileId: string;
  backend: string;
  capability: string;
  status: string;
  available: boolean;
  probedAt: string;
  stale?: boolean;
  staleReason?: string;
  durationMs?: number;
  error?: string;
  result?: any;
  runtimeBinding?: AiPluginRuntimeBinding;
  fingerprint?: any;
};

type AiPluginProfileState = {
  pluginId: string;
  profileId: string;
  backend: string;
  capability: string;
  status: string;
  verified: boolean;
  updatedAt: string;
  setupAttempted?: boolean;
  setupJobId?: string;
  durationMs?: number;
  error?: string;
  result?: unknown;
  runtimeBinding?: AiPluginRuntimeBinding;
};

type AiPluginSetupJob = {
  id: string;
  pluginId: string;
  profileId: string;
  backend: string;
  capability: string;
  status: string;
  createdAt: string;
  updatedAt: string;
  progress: number;
  message?: string;
  error?: string;
  log?: string[];
};

type AiPluginTaskState = {
  pluginId: string;
  capabilityId: string;
  taskId: string;
  status: string;
  createdAt: string;
  updatedAt: string;
  taskDir: string;
  outputDir: string;
  resultPolicy?: string;
  adopted?: boolean;
  outputs?: any[];
  progress?: number;
  message?: string;
  error?: string;
  errorCode?: string;
  errorDomain?: string;
  errorDetails?: any;
  retryable?: boolean;
  requestSnapshot?: any;
  pluginStatus?: any;
  pluginStatusError?: string;
};

type AiPluginSetupPreview = {
  pluginId: string;
  profileId: string;
  backend: string;
  capability: string;
  command: string;
  commandPath: string;
  workingDir: string;
  envDir?: string;
  envPath?: string;
  requirements?: string;
  requirementsPath?: string;
  runtimeBinding?: AiPluginRuntimeBinding;
  environment: Record<string, string>;
  warnings: string[];
  errors: string[];
};

type AiPluginHostGpu = {
  name: string;
  vendor: string;
  backendCandidates: string[];
};

type AiPluginPythonRuntime = {
  id: string;
  label: string;
  scope: string;
  python: string;
  root?: string;
  source: string;
  version?: string;
  available: boolean;
  error?: string;
};

type AiPluginHostEnvironment = {
  os: string;
  arch: string;
  platform: string;
  gpus: AiPluginHostGpu[];
  candidateBackends: string[];
  pythonRuntimes: AiPluginPythonRuntime[];
  probeError?: string;
};

const onRestoreDone = () => {
  showRestoreDialog.value = false;
  emit('libraries-changed');
};

const languages = [
  { label: 'English', value: 'en' },
  { label: '中文', value: 'zh' },
];

const appearanceOptions = computed(() => {
  const options = localeMsg.value.settings.general.appearance_options;
  return Array.from({ length: options.length }, (_, i) => ({
    label: options[i],
    value: i,
  }));
});

// Define the theme options
const themeOptions = computed(() => {
  const options = config.settings.appearance === 0 
    ? localeMsg.value.settings.general.theme_options_light 
    : localeMsg.value.settings.general.theme_options_dark;

  const result = [];
  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }
  return result;
});

const currentTheme = computed({
  get() {
    return config.settings.appearance === 0 ? config.settings.lightTheme : config.settings.darkTheme;
  },
  set(value) {
    config.settings.appearance === 0 ? config.settings.lightTheme = value : config.settings.darkTheme = value;
  }
});

const scaleOptions = computed(() => {
  const options = localeMsg.value.settings.general.font_size_options;
  const values = [0.8, 0.9, 1, 1.1, 1.2];
  return values.map((value, index) => ({
    value,
    label: options[index] ?? String(value),
  }));
});

const folderSortOptions = computed(() => {
  const options = localeMsg.value.settings.library.folder_sort_options || [];
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

const calendarSortOptions = computed(() => {
  const options = localeMsg.value.settings.library.calendar_sort_options || [];
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

const categorySortOptions = computed(() => {
  const options = localeMsg.value.settings.library.category_sort_options || [];
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

const externalImageAppName = computed(() =>
  String(config.settings.externalImageAppName || '') || localeMsg.value.settings.image_view.external_app_not_selected
);

const externalVideoAppName = computed(() =>
  String(config.settings.externalVideoAppName || '') || localeMsg.value.settings.image_view.external_app_not_selected
);

// Define the wheel options using computed to react to language changes
const wheelOptions = computed(() => {
  const options = localeMsg.value.settings.image_view.mouse_wheel_options; // returns an array
  return [
    { label: options[0], value: 0 },  // 0: previous / next
    { label: options[1], value: 1 },  // 1: zoom in / out
  ];
});

// Define the grid scaling options
const gridScalingOptions = computed(() => {
  const options = localeMsg.value.settings.view.scaling_options;
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

// Define the grid style options
const gridStyleOptions = computed(() => {
  const options = localeMsg.value.settings.view.style_options;
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

// Define the grid label options
const gridLabelOptions = computed(() => {
  const options = localeMsg.value.settings.view.label_options;
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

// Define the navigator view mode options
const navigatorViewModeOptions = computed(() => {
  const options = localeMsg.value.settings.image_view.navigator_view_options;
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

// Define the navigator view size options
const navigatorViewSizeOptions = computed(() => {
  const options = localeMsg.value.settings.image_view.navigator_view_size_options;
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: parseInt(options[i].split('(')[1].split('px')[0]) });
  }

  return result;
});

const slideShowTransitionOptions = computed(() => {
  const options = localeMsg.value.settings.image_view.slide_show_transition_options;
  const result = [];

  for (let i = 0; i < options.length; i++) {
    result.push({ label: options[i], value: i });
  }

  return result;
});

const dateGroupingOptions = computed(() => {
  const options = localeMsg.value.settings.view.date_grouping_options;
  return options.map((label: string, i: number) => ({ label, value: i }));
});

const filmStripViewPreviewPositionOptions = computed(() => {
  const options = localeMsg.value.settings.filmstrip_view.preview_position_options;
  return options.map((label, i) => ({ label, value: i }));
});

// Define the similarity options
const similarityOptions = computed(() => {
  const options = localeMsg.value.settings.image_search.similarity_options;
  // Use getter to retrieve thresholds
  const values = config.imageSearchThresholds ?? [0.8, 0.6, 0.4, 0.25]; 
  // Map index dummy as the value since v-model is thresholdIndex
  return values.map((val, i) => ({ label: options[i], value: i }));
});

const imageSearchModelOptions = computed(() => {
  const options = localeMsg.value.settings.image_search.search_model_options || ['Default', 'Multilingual model'];
  return options.map((label: string, i: number) => ({ label, value: i }));
});

const imageSearchModelHint = computed(() => {
  return Number(config.settings.imageSearch.model || 0) === 1
    ? localeMsg.value.settings.image_search.multilingual_model_hint
    : localeMsg.value.settings.image_search.default_model_hint;
});

const multilingualModelDownloadSizeText = computed(() => {
  const downloaded = multilingualModelDownloadedBytes.value;
  const total = multilingualModelTotalBytes.value;
  if (total > 0) {
    return `${formatFileSize(downloaded)} / ${formatFileSize(total)}`;
  }
  return formatFileSize(downloaded);
});

const syncImageSearchModelStatus = async () => {
  const status = await getImageSearchModelStatus();
  if (!status) return;

  isMultilingualModelAvailable.value = Boolean(status.multilingualAvailable);
  if (Number(config.settings.imageSearch.model || 0) === 1 && !isMultilingualModelAvailable.value) {
    config.settings.imageSearch.model = 0;
    await setImageSearchModel(0);
    return;
  }

  try {
    await setImageSearchModel(config.settings.imageSearch.model || 0);
  } catch (error) {
    console.error('Failed to activate image search model:', error);
    config.settings.imageSearch.model = 0;
    await setImageSearchModel(0);
  }
};

// Define the face cluster threshold options
const faceClusterOptions = computed(() => {
  const options = localeMsg.value.settings.face_recognition?.cluster_threshold_options || 
    ['Very High', 'High', 'Medium', 'Low'];
  // Map index as value since v-model is clusterThresholdIndex
  return options.map((label: string, i: number) => ({ label, value: i }));
});

const pluginMessages = computed(() => localeMsg.value.settings.plugins || {});

const pluginSummaryText = computed(() => {
  const valid = aiPlugins.value.filter((plugin) => plugin.validation.valid).length;
  const invalid = aiPlugins.value.length - valid;
  return pluginText('summary')
    .replace('{valid}', String(valid))
    .replace('{invalid}', String(invalid));
});

function pluginText(key: string, params?: Record<string, string | number | null | undefined>) {
  const fallback: Record<string, string> = {
    title: 'Plugins',
    pluginStoreLocation: 'Plugin storage',
    pluginStoreHint: 'ZIP plugins, models, logs, outputs, and shared runtimes are stored here.',
    changePluginStore: 'Change storage',
    resetPluginStore: 'Use default storage',
    activePath: 'Active',
    defaultPath: 'Default',
    pluginStoreEnvOverride: 'Environment variable PICAIPIC_PLUGIN_STORE_DIR is active: {path}',
    pluginStoreChangeWarning: 'Changing this path does not move existing plugins or runtimes automatically.',
    pluginStoreChanged: 'Plugin storage updated.',
    pluginStoreReset: 'Plugin storage reset to default.',
    directories: 'Plugin directories',
    addDirectory: 'Add plugin directory',
    installPackage: 'Install plugin package',
    installPackageSuccess: 'Plugin package installed.',
    trustPublisherTitle: 'Trust this publisher?',
    trustPublisher: 'Trust publisher',
    trustPublisherMessage: 'Publisher: {publisher}\nPublic key: {key}\n\nThis plugin is signed but the publisher is not in your trusted list. Trust this publisher to allow installation?',
    trustPublisherSuccess: 'Publisher trusted. Retrying install...',
    trustedPublishers: 'Trusted publishers',
    removeTrustedPublisher: 'Remove',
    noTrustedPublishers: 'No trusted publishers yet.',
    installPackageWarnings: 'Plugin package installed with warnings.',
    installPackageMissingModelsTitle: 'Model files needed',
    installPackageMissingModelsMessage: 'Plugin: {plugin}\nModel folder: {modelDir}\n\nMissing required model files:\n{models}\n\nOpen the model folder now?',
    installPackageModelsReady: 'Plugin package installed. Model folder is ready.',
    revokePrivacyGrant: 'Revoke authorization',
    revokePrivacyGrantHint: 'Clear saved permission for setup downloads and other plugin network/privacy grants.',
    revokePrivacyGrantTitle: 'Revoke plugin authorization?',
    revokePrivacyGrantMessage: 'Plugin: {plugin}\n\nClear saved setup-download and privacy/network authorization for this plugin? You can grant it again next time setup needs it.',
    revokePrivacyGrantSuccess: 'Plugin authorization revoked.',
    uninstallPlugin: 'Uninstall plugin',
    uninstallPluginConfirmTitle: 'Uninstall plugin?',
    uninstallPluginConfirmMessage: 'Plugin: {plugin}\nPath: {path}\n\nThis stops the plugin, removes the installed package copy from the user plugin directory, and unregisters it from PicAiPic. Development plugin directories are not deleted.',
    uninstallPluginSuccess: 'Plugin uninstalled.',
    uninstallSuccessCodeOnly: 'Plugin code removed. Data and runtimes kept.',
    uninstallSuccessCodeAndData: 'Plugin fully removed (code, data, runtimes). Shared runtimes kept.',
    removeDirectory: 'Remove directory',
    refresh: 'Refresh plugins',
    refreshStatus: 'Refresh status',
    diagnostics: 'Diagnostics',
    diagnosticsUnavailable: 'Diagnostics unavailable.',
    logs: 'Logs',
    storage: 'Storage',
    pluginCode: 'Code',
    pluginModels: 'Models',
    pluginLogs: 'Log files',
    pluginRuntime: 'Runtime',
    pluginOutputs: 'Outputs',
    open: 'Open',
    openRuntimeFolder: 'Open runtime folder',
    openPluginStore: 'Open store',
    openPathFailed: 'Failed to open path.',
    noLogs: 'No logs found.',
    emptyLog: 'Empty log.',
    start: 'Start plugin',
    stop: 'Stop plugin',
    restart: 'Restart plugin',
    test: 'Test',
    testInvoke: 'Test invoke',
    hostEnvironment: 'Host AI environment',
    refreshHostEnvironment: 'Refresh host AI environment',
    hostEnvironmentUnavailable: 'Host AI environment unavailable.',
    platform: 'Platform',
    candidateBackends: 'Backends',
    pythonRuntimes: 'Python runtimes',
    noGpusDetected: 'No GPU detected; CPU fallback is available.',
    runtime: 'Runtime',
    runtimeProfiles: 'Runtime profiles',
    selectedRuntimeProfile: 'Selected runtime',
    advancedRuntimeOptions: 'Advanced',
    smokeTest: 'Smoke test',
    recommendedProfile: 'Recommended: {profile}',
    noRecommendedProfile: 'No matching profile',
    profileReady: 'ready',
    profileMissingDependency: 'missing dependency',
    profileMissingModel: 'missing model',
    profileMissingSource: 'missing source',
    profileFailed: 'failed',
    profileVerified: 'verified',
    profileNeedsInstall: 'not installed',
    profileInstalling: 'installing',
    profileNeedsDiagnostics: 'needs diagnostics',
    profileNeedsVerification: 'needs verification',
    profileNotDetected: 'not detected',
    profileFallback: 'fallback',
    setupProfile: 'Setup',
    runSetupProfile: 'Run setup',
    runSetupConfirmTitle: 'Run plugin setup command?',
    runSetupConfirmMessage: 'Plugin: {plugin}\nProfile: {profile}\nBackend: {backend}\nCapability: {capability}\nRuntime: {runtime}\nPython: {python}\nCommand: {command}\nWorking dir: {workingDir}\nEnv dir: {envDir}\nRequirements: {requirements}\n{warnings}\nThis runs the setup script declared by the plugin. It may install dependencies or change files inside the plugin runtime environment. After it completes, run Smoke to verify the profile before using it.',
    runSetupPreviewFailed: 'Setup preview failed.',
    runSetupPreviewBlocked: 'Setup command cannot run until preview errors are fixed.',
    runSetupPreviewWarnings: 'Warnings:\n{warnings}\n',
    runtimeBinding: '{scope} runtime',
    runtimeBindingNone: 'runtime not declared',
    probeRuntime: 'Probe',
    probeRuntimeSuccess: 'Runtime probe completed.',
    probeRuntimeFailed: 'Runtime probe failed.',
    probeCachePassed: 'Probe passed',
    probeCacheFailed: 'Probe failed',
    probeCacheStale: 'Probe stale',
    probeCacheUnknown: 'Probe unknown',
    probeCacheNotPersisted: 'not cached',
    probeCacheNotProbed: 'not probed',
    probeCacheRecorded: 'cached {time}',
    probeStaleReason: 'Stale reason',
    probeStalePythonMissing: 'Python path missing',
    probeStaleFingerprintChanged: 'Runtime files changed',
    probeStaleTtlExpired: 'Cache expired',
    probeStaleInvalidTime: 'Probe time invalid',
    probeStaleUnknown: 'unknown',
    probeFingerprint: 'Fingerprint',
    probeTarget: 'Target',
    probeDuration: 'Duration',
    probeBinding: 'Binding',
    probeDevice: 'Device',
    probeBackendVersion: 'Backend',
    probeProviders: 'Providers',
    probeGroupGeneral: 'General',
    probeGroupPython: 'Python',
    probeGroupTorch: 'torch',
    probeGroupBackends: 'Backends',
    probeGroupOnnx: 'ONNX Runtime',
    probeGroupPackages: 'Packages',
    probePythonPlatform: 'Platform',
    probeTorchCuda: 'CUDA',
    probeTorchHip: 'HIP',
    probeTorchDeviceCount: 'Devices',
    probeTorchMps: 'MPS',
    probeBackendAvailable: 'available',
    probeBackendNotAvailable: 'not available',
    probeBackendProbeOk: 'probe ok',
    probeBackendProbeFailed: 'probe failed',
    probeSummaryReady: '{backend} runtime is available. Run Smoke before using it.',
    probeSummaryFailed: '{backend} runtime is not available.',
    probeSummaryStale: 'Cached probe cannot be trusted: {reason}.',
    probeAdviceRunProbe: 'Run Probe again for the selected runtime binding.',
    probeAdviceSelectRuntime: 'Select a runtime binding with an existing Python executable.',
    probeAdviceFingerprintChanged: 'The Python executable, venv config, requirements, or binding changed since the last probe.',
    probeAdviceTtlExpired: 'The cached probe is old; refresh it before invoking this plugin.',
    probeAdviceSmoke: 'Run Smoke next to record this profile as verified.',
    probeAdviceTimeout: 'The probe timed out; start with CPU or check whether importing torch hangs in this environment.',
    probeAdviceBackendMissing: '{backend} was requested but the probe did not find an available backend.',
    probeAdviceRunSetup: 'Run setup or install the missing Python packages for this binding.',
    probeAdviceOpenDiagnostics: 'Refresh Diagnostics and Logs for more detail.',
    runtimeConflictAdvice: 'Switch to a plugin-private runtime, or re-run Setup to fix.',
    probeAdviceTorchImportError: 'torch is installed but failed to import — check the error detail below.',
    probeAdviceInstallTorch: 'Install torch for this Python environment (pip install torch or run setup).',
    probeAdviceInstallOnnx: 'Install onnxruntime for this Python environment to use ONNX-based backends.',
    probeAdviceInstallDirectML: 'Install torch-directml for this Python environment (pip install torch-directml).',
    probeAdviceDirectMLInitFailed: 'torch-directml is installed but the DirectML device could not initialize.',
    probeAdviceGpuDeviceCountZero: 'No GPU devices detected by torch — check your GPU driver or whether another process is occupying the GPU.',
    probeAdviceTensorProbeFailed: 'GPU backend is available but a test tensor operation failed — the driver or runtime may be mismatched.',
    probeAdviceOom: 'GPU out of memory during the probe — close other GPU applications or reduce analysis size.',
    runSetupSuccess: 'Setup command completed. Run Smoke to verify this profile.',
    runSetupFailed: 'Setup command failed.',
    runSetupCancelled: 'Setup command was cancelled.',
    copySetupLog: 'Copy setup log',
    copySetupLogSuccess: 'Setup log copied.',
    copySetupLogFailed: 'Failed to copy setup log.',
    cancelSetup: 'Cancel',
    verifyProfile: 'Verify',
    verifyProfileFailed: 'Profile diagnostics failed.',
    smokeTestProfile: 'Smoke',
    setupProfileQueued: 'Runtime setup state recorded. Run Verify or Smoke next.',
    verifyProfileSuccess: 'Profile diagnostics refreshed.',
    smokeTestSuccess: 'Smoke test passed. Profile verified.',
    smokeTestFailed: 'Smoke test failed.',
    smokeTestRunning: 'Running smoke test. This may start the plugin and can take up to {seconds}s.',
    smokeTestRunningBadge: 'running',
    recentTasks: 'Recent tasks',
    taskStatusQueued: 'Queued',
    taskStatusRunning: 'Running',
    taskStatusCancelling: 'Cancelling',
    taskStatusSucceeded: 'Succeeded',
    taskStatusFailed: 'Failed',
    taskStatusCancelled: 'Cancelled',
    taskStatusImported: 'Imported',
    taskStatusDiscarded: 'Cleaned',
    discardTask: 'Discard',
    discardTaskSuccess: 'Task outputs discarded.',
    retryTask: 'Retry',
    retryTaskSuccess: 'Task retry started.',
    cancelTask: 'Cancel',
    cancelTaskSuccess: 'Task cancellation requested.',
    installedPlugins: 'Installed plugins',
    noDirectories: 'No custom plugin directories registered.',
    noPlugins: 'No plugins discovered.',
    unnamedPlugin: 'Unnamed plugin',
    valid: 'Valid',
    invalid: 'Invalid',
    unsupported: 'Unsupported',
    running: 'Running',
    staleRuntime: 'Stale service',
    stopped: 'Not running',
    statusUnknown: 'Status unknown',
    publisher: 'Publisher',
    entry: 'Entry',
    install: 'Install',
    requiresAdmin: 'admin',
    noCapabilities: 'No capabilities declared.',
    chooseFile: 'Choose file',
    addSuccess: 'Plugin directory added.',
    removeSuccess: 'Plugin directory removed.',
    startSuccess: 'Plugin started.',
    startFailed: 'Plugin did not start.',
    stopSuccess: 'Plugin stopped.',
    restartSuccess: 'Plugin restarted.',
    restartFailed: 'Plugin did not restart.',
    invokeSuccess: 'Capability invoked.',
    summary: '{valid} valid / {invalid} with issues',
  };
  let value = pluginMessages.value[key] || fallback[key] || key;
  if (params) {
    for (const [name, replacement] of Object.entries(params)) {
      value = value.replaceAll(`{${name}}`, String(replacement ?? ''));
    }
  }
  return value;
}

function pluginStateText(plugin: AiPluginSummary) {
  if (!plugin.validation.valid) return pluginText('invalid');
  if (!plugin.platformSupported) return pluginText('unsupported');

  const status = aiPluginStatuses.value[plugin.id];
  if (plugin.entry?.kind === 'local-http') {
    if (!status) return pluginText('statusUnknown');
    if (status.reachable && status.managed) return pluginText('running');
    if (status.reachable) return pluginText('staleRuntime');
    return pluginText('stopped');
  }

  if (status?.reachable && status?.managed !== false) return pluginText('running');
  if (status?.reachable) return pluginText('staleRuntime');
  if (status?.error) return pluginText('stopped');
  return pluginText('valid');
}

function pluginStateClass(plugin: AiPluginSummary) {
  if (!plugin.validation.valid || !plugin.platformSupported) {
    return 'bg-error/10 text-error border border-error/20';
  }

  const status = aiPluginStatuses.value[plugin.id];
  if (plugin.entry?.kind === 'local-http') {
    if (!status) {
      return 'bg-base-300/70 text-base-content/50 border border-base-content/10';
    }
    if (status.reachable && status.managed) {
      return 'bg-success/10 text-success border border-success/20';
    }
    if (status.reachable) {
      return 'bg-warning/10 text-warning border border-warning/20';
    }
    return 'bg-warning/10 text-warning border border-warning/20';
  }

  if (status?.reachable && status?.managed !== false) {
    return 'bg-success/10 text-success border border-success/20';
  }
  if (status?.reachable) {
    return 'bg-warning/10 text-warning border border-warning/20';
  }
  if (status?.error) {
    return 'bg-warning/10 text-warning border border-warning/20';
  }
  return 'bg-base-300/70 text-base-content/50 border border-base-content/10';
}

function aiPluginKey(plugin: AiPluginSummary) {
  return plugin.id || plugin.manifestPath || plugin.path;
}

function isAiPluginExpanded(plugin: AiPluginSummary) {
  return Boolean(expandedAiPluginKeys.value[aiPluginKey(plugin)]);
}

function setAiPluginExpanded(plugin: AiPluginSummary, expanded: boolean) {
  expandedAiPluginKeys.value = {
    ...expandedAiPluginKeys.value,
    [aiPluginKey(plugin)]: expanded,
  };
}

function toggleAiPluginExpanded(plugin: AiPluginSummary) {
  setAiPluginExpanded(plugin, !isAiPluginExpanded(plugin));
}

function isAiPluginAdvancedExpanded(plugin: AiPluginSummary) {
  return Boolean(advancedAiPluginKeys.value[aiPluginKey(plugin)]);
}

function toggleAiPluginAdvanced(plugin: AiPluginSummary) {
  advancedAiPluginKeys.value = {
    ...advancedAiPluginKeys.value,
    [aiPluginKey(plugin)]: !isAiPluginAdvancedExpanded(plugin),
  };
}

function pluginRuntimeUrl(plugin: AiPluginSummary) {
  const statusUrl = plugin.id ? aiPluginStatuses.value[plugin.id]?.url : '';
  if (!statusUrl || !statusUrl.startsWith('http')) return '';

  try {
    const url = new URL(statusUrl);
    return `${url.protocol}//${url.host}`;
  } catch {
    return statusUrl;
  }
}

function pluginPrivacyRisks(plugin: AiPluginSummary) {
  const permissions = getPluginPermissions(plugin);
  const risks: string[] = [];
  if (permissions.network.setupDownloads) risks.push('setup downloads');
  if (permissions.network.runtime) risks.push('runtime network');
  if (permissions.network.uploadSelectedFiles) risks.push('upload selected files');
  if (permissions.network.uploadOutputs) risks.push('upload outputs');
  return risks;
}

function pluginPrivacySummary(plugin: AiPluginSummary) {
  const permissions = getPluginPermissions(plugin);
  const risks = pluginPrivacyRisks(plugin);
  if (!permissions) return 'No declared permissions.';
  if (risks.length === 0) return 'Local-only declared. No network or upload permissions.';
  return `Declared: ${risks.join(', ')}.`;
}

function pluginAllowedDomainsText(plugin: AiPluginSummary) {
  const domains = pluginAllowedDomains(plugin);
  if (!domains.length) return 'No declared domains';
  return domains.join(', ');
}

function pluginPermissionGrantSummary(plugin: AiPluginSummary) {
  const grant = getPluginPermissionGrant(plugin);
  if (!grant) return 'No saved privacy authorization.';
  const granted: string[] = [];
  if (grant.setupDownloads) granted.push('setup downloads');
  if (grant.runtimeNetwork) granted.push('runtime network');
  if (grant.uploadSelectedFiles) granted.push('upload selected files');
  if (grant.uploadOutputs) granted.push('upload outputs');
  if (!granted.length) return 'Saved authorization is empty.';
  return `Authorized: ${granted.join(', ')}.`;
}

async function revokePluginPrivacyGrant(plugin: AiPluginSummary) {
  if (!plugin.id) return;
  const confirmed = await ask(
    pluginText('revokePrivacyGrantMessage', { plugin: plugin.name || plugin.id }),
    {
      title: pluginText('revokePrivacyGrantTitle'),
      kind: 'warning',
      okLabel: pluginText('revokePrivacyGrant'),
      cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
    },
  );
  if (!confirmed) return;

  try {
    await revokeAiPluginPermissions(plugin.id);
    await loadAiPluginPanel(false);
    toast.success(pluginText('revokePrivacyGrantSuccess'));
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

function formatAiPluginStatus(status: unknown) {
  if (!status) return '';
  if (typeof status === 'string') return status;
  try {
    return JSON.stringify(status);
  } catch {
    return String(status);
  }
}

function aiPluginStatusError(status: AiPluginStatus | null | undefined) {
  if (!status) return '';
  if (status.error) return status.error;
  const payload: any = status.status || {};
  return payload?.error?.message || payload?.lastError || payload?.reason || '';
}

function aiPluginStartupIssue(plugin: AiPluginSummary) {
  if (!plugin?.id) return null;
  const status = aiPluginStatuses.value[plugin.id];
  if (!status || status.reachable) return null;
  const code = String(status.errorCode || '').toLowerCase();
  const domain = String(status.errorDomain || '').toLowerCase();
  if (domain !== 'runtime' && !code.includes('startup') && !code.includes('start_command')) {
    return null;
  }
  if (!status.error && !status.logTail && !(status.advice || []).length) return null;
  return status;
}

function aiPluginStartupIssueLabel(status: AiPluginStatus | null | undefined) {
  if (!status) return '';
  return [status.errorDomain, status.errorCode].filter(Boolean).join(' / ') || pluginText('startFailed');
}

function formatPluginBackends(backends?: string[]) {
  if (!Array.isArray(backends) || backends.length === 0) return 'cpu';
  return backends.join(' / ');
}

function formatAiPluginInstall(install: AiPluginInstall) {
  const parts = [install.kind || pluginText('install')];
  if (install.command) parts.push(install.command);
  if (install.estimatedDiskMb) parts.push(formatFileSize(install.estimatedDiskMb * 1024 * 1024));
  if (install.requiresAdmin) parts.push(pluginText('requiresAdmin'));
  return parts.join(' - ');
}

function pluginStorageRows(plugin: AiPluginSummary) {
  const storage = plugin.storage;
  if (!storage) return [];
  const modelDirs = Array.isArray(storage.modelDirs)
    ? storage.modelDirs.filter(Boolean)
    : [];
  const modelRows = modelDirs.length > 0
    ? modelDirs.map((path, index) => ({
        key: `models-${index}`,
        label: modelDirs.length === 1 ? pluginText('pluginModels') : `${pluginText('pluginModels')} ${index + 1}`,
        path,
      }))
    : [{ key: 'models', label: pluginText('pluginModels'), path: storage.modelDir }];
  const runtimeDirs = Array.isArray(storage.runtimeDirs)
    ? storage.runtimeDirs.filter(Boolean)
    : [];
  const runtimeRows = runtimeDirs.length > 0
    ? runtimeDirs.map((path, index) => ({
        key: `runtime-${index}`,
        label: runtimeDirs.length === 1 ? pluginText('pluginRuntime') : `${pluginText('pluginRuntime')} ${index + 1}`,
        path,
      }))
    : [{ key: 'runtime', label: pluginText('pluginRuntime'), path: storage.runtimeDir }];
  return [
    { key: 'code', label: pluginText('pluginCode'), path: storage.codeDir || plugin.path },
    ...modelRows,
    { key: 'logs', label: pluginText('pluginLogs'), path: storage.logDir },
    ...runtimeRows,
    { key: 'outputs', label: pluginText('pluginOutputs'), path: storage.outputDir },
  ].filter((row) => row.path);
}

async function openPluginPath(path?: string) {
  if (!path) return;
  try {
    await revealPath(path);
  } catch (error: any) {
    toast.error(error?.message || String(error) || pluginText('openPathFailed'));
  }
}

async function chooseAiPluginStoreDir() {
  if (isChangingAiPluginStore.value || aiPluginStoreInfo.value?.envOverride) return;

  const result = await openDialog({
    title: pluginText('changePluginStore'),
    multiple: false,
    directory: true,
  });

  if (!result || Array.isArray(result)) return;

  try {
    isChangingAiPluginStore.value = true;
    aiPluginStoreInfo.value = await setAiPluginStoreDir(result);
    await loadAiPluginPanel(true);
    toast.success(pluginText('pluginStoreChanged'));
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    isChangingAiPluginStore.value = false;
  }
}

async function resetAiPluginStoreLocation() {
  if (isChangingAiPluginStore.value || aiPluginStoreInfo.value?.envOverride) return;

  try {
    isChangingAiPluginStore.value = true;
    aiPluginStoreInfo.value = await resetAiPluginStoreDir();
    await loadAiPluginPanel(true);
    toast.success(pluginText('pluginStoreReset'));
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    isChangingAiPluginStore.value = false;
  }
}

async function promptInstalledPluginModels(installResult: any) {
  const modelDir = installResult?.storage?.modelDir;
  const modelDirs = Array.isArray(installResult?.storage?.modelDirs)
    ? installResult.storage.modelDirs.filter(Boolean)
    : [];
  const modelFiles = Array.isArray(installResult?.modelFiles) ? installResult.modelFiles : [];
  const missingRequired = modelFiles.filter((model: any) => model?.required && !model?.exists);
  if (!modelDir || modelFiles.length === 0) return;

  if (missingRequired.length === 0) {
    toast.success(pluginText('installPackageModelsReady'));
    return;
  }

  const modelList = missingRequired
    .map((model: any) => `- ${model.name || model.id || 'model'}\n  ${model.path}`)
    .join('\n');
  const firstMissingDir = missingRequired[0]?.path
    ? String(missingRequired[0].path).replace(/[\\/][^\\/]*$/, '')
    : '';
  const openDir = firstMissingDir || modelDirs[0] || modelDir;
  const openFolder = await ask(
    pluginText('installPackageMissingModelsMessage')
      .replace('{plugin}', installResult?.pluginId || '')
      .replace('{modelDir}', openDir)
      .replace('{models}', modelList),
    {
      title: pluginText('installPackageMissingModelsTitle'),
      kind: 'warning',
      okLabel: pluginText('pluginModels'),
      cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
    },
  );
  if (openFolder) {
    await openPluginPath(openDir);
  }
}

const PROFILE_SUPPORT_RANK: Record<string, number> = {
  official: 0,
  derived: 1,
  experimental: 2,
  fallback: 3,
};

function pluginRecommendedProfile(plugin: AiPluginSummary) {
  const profiles = plugin.installProfiles || [];
  if (profiles.length === 0) return null;

  const detected = new Set((aiPluginHostEnvironment.value?.candidateBackends || []).map((backend) => backend.toLowerCase()));
  const matches = profiles.filter((profile) => detected.has(String(profile.backend || '').toLowerCase()));
  const candidates = matches.length > 0 ? matches : profiles.filter((profile) => profile.backend === 'cpu');
  const sorted = [...candidates].sort((a, b) => {
    const rankA = PROFILE_SUPPORT_RANK[a.supportLevel] ?? 10;
    const rankB = PROFILE_SUPPORT_RANK[b.supportLevel] ?? 10;
    return rankA - rankB;
  });
  return sorted[0] || null;
}

function pluginStartProfile(plugin: AiPluginSummary) {
  const profiles = plugin.installProfiles || [];
  if (profiles.length === 0) return null;
  const verified = profiles.find((profile) => {
    const status = profileEnvironmentStatus(plugin, profile).level;
    return status === 'verified' || status === 'ready';
  });
  const needsVerification = profiles.find((profile) => (
    profileEnvironmentStatus(plugin, profile).level === 'needsVerification'
  ));
  return verified || pluginRecommendedProfile(plugin) || needsVerification || profiles[0] || null;
}

function pluginRecommendedProfileText(plugin: AiPluginSummary) {
  const profile = pluginRecommendedProfile(plugin);
  if (!profile) return pluginText('noRecommendedProfile');
  return pluginText('recommendedProfile').replace('{profile}', profile.label || profile.backend || profile.id);
}

function visiblePluginProfiles(plugin: AiPluginSummary) {
  const profiles = plugin.installProfiles || [];
  if (isAiPluginAdvancedExpanded(plugin)) return profiles;
  const selected = pluginStartProfile(plugin) || pluginRecommendedProfile(plugin) || profiles[0];
  if (!selected) return [];
  return profiles.filter((profile) => profile.id === selected.id);
}

function hiddenPluginProfileCount(plugin: AiPluginSummary) {
  const total = plugin.installProfiles?.length || 0;
  return Math.max(0, total - visiblePluginProfiles(plugin).length);
}

function formatAiPluginRuntime(runtime: AiPluginRuntime) {
  const parts = [runtime.kind || pluginText('runtime')];
  if (runtime.cudaApiCompatible) parts.push('CUDA API compatible');
  return parts.join(' - ');
}

function formatAiPluginInstallProfile(profile: AiPluginInstallProfile) {
  const parts = [
    profile.label || profile.backend || profile.id,
    profile.supportLevel,
  ];
  if (profile.derivedFrom) parts.push(`from ${profile.derivedFrom}`);
  return parts.filter(Boolean).join(' - ');
}

function formatRuntimeBinding(binding?: AiPluginRuntimeBinding) {
  if (!binding?.scope) return pluginText('runtimeBindingNone');
  if (binding.label) return `${binding.label} (${binding.scope})`;
  if (binding.id) return `${binding.id} (${binding.scope})`;
  return pluginText('runtimeBinding').replace('{scope}', binding.scope);
}

function runtimeBindingBadgeClass(binding?: AiPluginRuntimeBinding) {
  const scope = String(binding?.scope || '').toLowerCase();
  if (scope === 'shared') {
    return 'border-success/25 bg-success/10 text-success';
  }
  if (scope === 'plugin') {
    return 'border-info/25 bg-info/10 text-info';
  }
  if (scope === 'external') {
    return 'border-warning/25 bg-warning/10 text-warning';
  }
  return 'border-base-content/10 bg-base-100/30 text-base-content/35';
}

function runtimeBindingKey(binding: AiPluginRuntimeBinding, index = 0) {
  return binding.id || `${binding.scope || 'runtime'}:${index}`;
}

function profileRuntimePathChip(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  if (!selectedRuntimeBinding(plugin, profile)?.scope) return false;
  return Boolean(profile.resolvedRuntimeDir || condensedRuntimeVersions(plugin, profile));
}

function shortRuntimePath(_plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const p = profile.resolvedRuntimeDir;
  if (!p) return '';
  const parts = p.split(/[\\/]/).filter(Boolean);
  if (parts.length === 0) return p;
  return parts.slice(-2).join('\\');
}

function condensedRuntimeVersions(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const result = profileRuntimeProbeResult(plugin, profile);
  const data = result?.result;
  if (!data) return '';
  const parts: string[] = [];
  const pyVer = data.python?.version;
  if (pyVer) parts.push(`Python ${pyVer}`);
  const pkgs = data.packages || {};
  const names: Array<'torch' | 'numpy' | 'opencv-python' | 'rawpy'> = ['torch', 'numpy', 'opencv-python', 'rawpy'];
  for (const name of names) {
    const info = pkgs[name];
    if (info?.available && info?.version) {
      const label = name === 'opencv-python' ? 'cv2' : name;
      parts.push(`${label} ${info.version}`);
    }
  }
  return parts.join(' · ');
}

function discoveredRuntimeBindingsForProfile(profile: AiPluginInstallProfile) {
  const declaredRequirements = profile.runtimeBinding?.requirements || profile.requirements;
  return (aiPluginHostEnvironment.value?.pythonRuntimes || [])
    .filter((runtime) => runtime.available)
    .map((runtime) => ({
      scope: runtime.scope || 'external',
      kind: 'python',
      id: `discovered:${runtime.id}`,
      label: runtime.label,
      python: runtime.python,
      root: runtime.root,
      requirements: declaredRequirements,
      notes: `${runtime.source}${runtime.version ? ` - ${runtime.version}` : ''}`,
    } as AiPluginRuntimeBinding));
}

function profileRuntimeBindingOptions(profile: AiPluginInstallProfile) {
  const bindings: AiPluginRuntimeBinding[] = [];
  const seen = new Set<string>();
  const pushBinding = (binding?: AiPluginRuntimeBinding) => {
    if (!binding?.scope) return;
    const key = runtimeBindingKey(binding, bindings.length);
    if (seen.has(key)) return;
    seen.add(key);
    bindings.push(binding);
  };
  pushBinding(profile.runtimeBinding);
  for (const binding of profile.runtimeBindings || []) {
    pushBinding(binding);
  }
  for (const binding of discoveredRuntimeBindingsForProfile(profile)) {
    pushBinding(binding);
  }
  return bindings;
}

function runtimeBindingSelectionKey(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  return `${aiPluginKey(plugin)}:${profile.id}:runtimeBinding`;
}

function selectedRuntimeBinding(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const persisted = profilePersistentState(plugin, profile)?.runtimeBinding;
  const options = profileRuntimeBindingOptions(profile);
  const selectedKey = aiPluginRuntimeBindingSelection.value[runtimeBindingSelectionKey(plugin, profile)];
  if (selectedKey) {
    return options.find((binding, index) => runtimeBindingKey(binding, index) === selectedKey) || persisted || options[0];
  }
  if (persisted) {
    return options.find((binding) => binding.id && binding.id === persisted.id) || persisted;
  }
  return options[0];
}

function selectedRuntimeBindingId(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  return selectedRuntimeBinding(plugin, profile)?.id || undefined;
}

function selectedRuntimeBindingKey(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const selected = selectedRuntimeBinding(plugin, profile);
  const options = profileRuntimeBindingOptions(profile);
  const index = options.findIndex((binding) => binding === selected || (binding.id && binding.id === selected?.id));
  return selected ? runtimeBindingKey(selected, Math.max(index, 0)) : '';
}

function setRuntimeBindingSelection(plugin: AiPluginSummary, profile: AiPluginInstallProfile, key: string) {
  aiPluginRuntimeBindingSelection.value = {
    ...aiPluginRuntimeBindingSelection.value,
    [runtimeBindingSelectionKey(plugin, profile)]: key,
  };
}

function runtimeProbeKey(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const binding = selectedRuntimeBinding(plugin, profile);
  return `${aiPluginKey(plugin)}:${profile.id}:${profile.backend}:${binding?.python || binding?.id || 'runtime'}`;
}

function profileRuntimeProbeResult(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const runtimeResult = aiPluginRuntimeProbeResults.value[runtimeProbeKey(plugin, profile)];
  if (runtimeResult) return runtimeResult;
  // Find the probe state matching the currently selected binding
  const selectedBinding = selectedRuntimeBinding(plugin, profile);
  const allStates = profile.runtimeProbeStates || (profile.runtimeProbeState ? [profile.runtimeProbeState] : []);
  const state = matchProbeStateByBinding(allStates, selectedBinding) || profile.runtimeProbeState;
  if (!state) return undefined;
  return {
    python: state.runtimeBinding?.python || '',
    backend: state.backend,
    available: state.available && !state.stale && state.status === 'passed',
    durationMs: state.durationMs || 0,
    result: state.result,
    error: state.stale ? `stale: ${state.staleReason || 'unknown'}` : state.error,
    state,
  } as AiPluginPythonRuntimeProbeResult;
}

function profileRuntimeConflicts(profile: AiPluginInstallProfile): RuntimeConflict[] {
  return profile.runtimeConflicts || [];
}

function matchProbeStateByBinding(
  states: AiPluginRuntimeProbeState[],
  binding?: AiPluginRuntimeBinding,
): AiPluginRuntimeProbeState | undefined {
  if (!binding || states.length === 0) return undefined;
  // Match by python path first, then by binding id
  if (binding.python) {
    const byPython = states.find((s) => s.runtimeBinding?.python === binding.python);
    if (byPython) return byPython;
  }
  if (binding.id) {
    const byId = states.find((s) => s.runtimeBinding?.id === binding.id);
    if (byId) return byId;
  }
  return undefined;
}

function bindingProbeStatus(
  plugin: AiPluginSummary,
  profile: AiPluginInstallProfile,
  binding: AiPluginRuntimeBinding,
): { label: string; tone: 'ok' | 'bad' | 'stale' | 'none' } {
  // Check live probe results first
  const liveKey = `${aiPluginKey(plugin)}:${profile.id}:${profile.backend}:${binding.python || binding.id || 'runtime'}`;
  const liveResult = aiPluginRuntimeProbeResults.value[liveKey];
  if (liveResult) {
    if (liveResult.state?.stale) return { label: pluginText('probeCacheStale'), tone: 'stale' };
    if (runtimeProbeAvailable(liveResult)) return { label: pluginText('probeCachePassed'), tone: 'ok' };
    if (liveResult.error || liveResult.state?.status === 'failed') return { label: pluginText('probeCacheFailed'), tone: 'bad' };
  }
  // Check persisted probe states from backend
  const allStates = profile.runtimeProbeStates || (profile.runtimeProbeState ? [profile.runtimeProbeState] : []);
  const state = matchProbeStateByBinding(allStates, binding);
  if (state) {
    if (state.stale) return { label: pluginText('probeCacheStale'), tone: 'stale' };
    if (state.available && state.status === 'passed') return { label: pluginText('probeCachePassed'), tone: 'ok' };
    if (state.status === 'failed' || state.error) return { label: pluginText('probeCacheFailed'), tone: 'bad' };
  }
  return { label: pluginText('probeCacheNotProbed'), tone: 'none' };
}

function bindingOptionLabel(
  plugin: AiPluginSummary,
  profile: AiPluginInstallProfile,
  binding: AiPluginRuntimeBinding,
) {
  const status = bindingProbeStatus(plugin, profile, binding);
  const marker = status.tone === 'ok' ? ' ✓' : status.tone === 'bad' ? ' ✗' : status.tone === 'stale' ? ' ⟳' : '';
  return `${formatRuntimeBinding(binding)}${marker}`;
}

function probeBackendAvailable(result: AiPluginPythonRuntimeProbeResult | undefined, backend: string) {
  if (!result?.result) return false;
  const normalized = String(backend || '').toLowerCase();
  if (normalized === 'cpu') return true;
  return Boolean(result.result?.backends?.[normalized]?.available);
}

function runtimeProbeAvailable(result?: AiPluginPythonRuntimeProbeResult) {
  return Boolean(result?.available || probeBackendAvailable(result, result?.backend || ''));
}

function runtimeProbeStatusLabel(result?: AiPluginPythonRuntimeProbeResult) {
  if (!result) return '';
  if (result.state?.stale) return pluginText('probeCacheStale');
  if (runtimeProbeAvailable(result)) return pluginText('probeCachePassed');
  if (result.error || result.state?.status === 'failed') return pluginText('probeCacheFailed');
  return result.state?.status || pluginText('probeCacheUnknown');
}

function runtimeProbeCardClass(result?: AiPluginPythonRuntimeProbeResult) {
  if (!result) return 'border-base-content/10';
  if (result.state?.stale) return 'border-warning/20';
  if (runtimeProbeAvailable(result)) return 'border-success/20';
  return 'border-error/20';
}

function runtimeProbeBadgeClass(result?: AiPluginPythonRuntimeProbeResult) {
  if (!result) return 'border-base-content/10 bg-base-100/40 text-base-content/45';
  if (result.state?.stale) return 'border-warning/20 bg-warning/10 text-warning';
  if (runtimeProbeAvailable(result)) return 'border-success/20 bg-success/10 text-success';
  return 'border-error/20 bg-error/10 text-error';
}

function parseRuntimeProbeTimestamp(value?: string) {
  if (!value) return 0;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

function formatRuntimeProbeTime(value?: string) {
  const timestamp = parseRuntimeProbeTimestamp(value);
  return timestamp ? new Date(timestamp).toLocaleString() : '';
}

function runtimeProbeCacheText(result?: AiPluginPythonRuntimeProbeResult) {
  const state = result?.state;
  if (!state?.probedAt) return pluginText('probeCacheNotPersisted');
  const time = formatRuntimeProbeTime(state.probedAt);
  if (state.stale) {
    return `${pluginText('probeCacheStale')}: ${runtimeProbeStaleReasonLabel(state.staleReason)}`;
  }
  return pluginText('probeCacheRecorded').replace('{time}', time || state.probedAt);
}

function runtimeProbeCacheTitle(result?: AiPluginPythonRuntimeProbeResult) {
  const state = result?.state;
  if (!state) return '';
  return [
    state.probedAt ? `${pluginText('probeCacheRecorded').replace('{time}', formatRuntimeProbeTime(state.probedAt) || state.probedAt)}` : '',
    state.stale ? `${pluginText('probeStaleReason')}: ${runtimeProbeStaleReasonLabel(state.staleReason)}` : '',
    state.fingerprint ? `${pluginText('probeFingerprint')}: ${formatAiPluginStatus(state.fingerprint)}` : '',
  ].filter(Boolean).join('\n');
}

function runtimeProbeStaleReasonLabel(reason?: string) {
  const labels: Record<string, string> = {
    python_missing: pluginText('probeStalePythonMissing'),
    fingerprint_changed: pluginText('probeStaleFingerprintChanged'),
    ttl_expired: pluginText('probeStaleTtlExpired'),
    invalid_probed_at: pluginText('probeStaleInvalidTime'),
  };
  return labels[String(reason || '')] || reason || pluginText('probeStaleUnknown');
}

function runtimeProbeBackendInfo(result?: AiPluginPythonRuntimeProbeResult, backendOverride?: string) {
  const backend = String(backendOverride || result?.backend || '').toLowerCase();
  if (!backend || !result?.result?.backends) return null;
  return result.result.backends[backend] || null;
}

function runtimeProbeSummary(result: AiPluginPythonRuntimeProbeResult | undefined, backend: string) {
  if (!result) return '';
  const backendInfo = runtimeProbeBackendInfo(result, backend);
  const target = backend || result.backend || pluginText('runtime');
  if (result.state?.stale) {
    return pluginText('probeSummaryStale').replace('{reason}', runtimeProbeStaleReasonLabel(result.state.staleReason));
  }
  if (runtimeProbeAvailable(result)) {
    return pluginText('probeSummaryReady').replace('{backend}', target);
  }
  const backendError = backendInfo?.probe?.error || backendInfo?.error;
  const packageError = result.result?.packages?.torch?.error || result.result?.packages?.torchDirectML?.error || result.result?.packages?.onnxruntime?.error;
  return backendError || result.error || packageError || pluginText('probeSummaryFailed').replace('{backend}', target);
}

function runtimeProbeDetails(result?: AiPluginPythonRuntimeProbeResult) {
  if (!result) return [];
  const data = result.result || {};
  const binding = result.state?.runtimeBinding;
  const backendInfo = runtimeProbeBackendInfo(result);
  const torch = data.torch || data.packages?.torch;
  const details: { label: string; value: string; title?: string }[] = [];
  if (result.backend) details.push({ label: pluginText('probeTarget'), value: result.backend });
  if (result.durationMs !== undefined) details.push({ label: pluginText('probeDuration'), value: `${result.durationMs}ms` });
  if (binding) details.push({ label: pluginText('probeBinding'), value: formatRuntimeBinding(binding), title: binding.python || binding.root || binding.notes || '' });
  if (result.python || binding?.python) details.push({ label: 'Python', value: data.python?.version || result.python || binding?.python || '', title: result.python || binding?.python || '' });
  if (torch?.version) details.push({ label: 'torch', value: String(torch.version) });
  if (backendInfo?.device) details.push({ label: pluginText('probeDevice'), value: String(backendInfo.device) });
  if (backendInfo?.version) details.push({ label: pluginText('probeBackendVersion'), value: String(backendInfo.version) });
  if (backendInfo?.providers?.length) details.push({ label: pluginText('probeProviders'), value: backendInfo.providers.join(', '), title: backendInfo.providers.join('\n') });
  return details.filter((detail) => detail.value);
}

type ProbeDetailItem = { label: string; value: string; title?: string; tone?: 'ok' | 'bad' | 'neutral' };
type ProbeDetailGroup = { groupKey: string; items: ProbeDetailItem[] };

function runtimeProbeDetailGroups(result?: AiPluginPythonRuntimeProbeResult): ProbeDetailGroup[] {
  if (!result) return [];
  const data = result.result || {};
  const binding = result.state?.runtimeBinding;
  const groups: ProbeDetailGroup[] = [];

  // General
  const general: ProbeDetailItem[] = [];
  if (result.backend) general.push({ label: pluginText('probeTarget'), value: result.backend });
  if (result.durationMs !== undefined) general.push({ label: pluginText('probeDuration'), value: `${result.durationMs}ms` });
  if (binding) general.push({ label: pluginText('probeBinding'), value: formatRuntimeBinding(binding), title: binding.python || binding.root || binding.notes || '' });
  if (general.length) groups.push({ groupKey: 'probeGroupGeneral', items: general });

  // Python
  const pyItems: ProbeDetailItem[] = [];
  const py = data.python;
  if (py?.version || result.python || binding?.python) {
    pyItems.push({ label: 'Python', value: py?.version || result.python || binding?.python || '', title: result.python || binding?.python || '' });
  }
  if (py?.platform) pyItems.push({ label: pluginText('probePythonPlatform'), value: String(py.platform), title: String(py.platform) });
  if (pyItems.length) groups.push({ groupKey: 'probeGroupPython', items: pyItems });

  // torch
  const torchItems: ProbeDetailItem[] = [];
  const torch = data.torch || data.packages?.torch;
  if (torch?.version) torchItems.push({ label: 'torch', value: String(torch.version) });
  if (torch?.cudaVersion !== undefined && torch?.cudaVersion !== null) torchItems.push({ label: pluginText('probeTorchCuda'), value: String(torch.cudaVersion) });
  if (torch?.hipVersion !== undefined && torch?.hipVersion !== null) torchItems.push({ label: pluginText('probeTorchHip'), value: String(torch.hipVersion) });
  if (torch?.cudaDeviceCount !== undefined) torchItems.push({ label: pluginText('probeTorchDeviceCount'), value: String(torch.cudaDeviceCount) });
  if (torch?.mpsAvailable !== undefined) torchItems.push({ label: pluginText('probeTorchMps'), value: torch.mpsAvailable ? '✓' : '✗', tone: torch.mpsAvailable ? 'ok' : 'neutral' });
  if (torchItems.length) groups.push({ groupKey: 'probeGroupTorch', items: torchItems });

  // Backends
  const backends = data.backends || {};
  const backendOrder = ['cuda', 'rocm', 'directml', 'mps', 'openvino', 'cpu'];
  const backendItems: ProbeDetailItem[] = [];
  for (const backendId of backendOrder) {
    const info = backends[backendId];
    if (!info) continue;
    const available = Boolean(info.available);
    const parts: string[] = [];
    if (info.version) parts.push(String(info.version));
    if (info.deviceCount !== undefined && info.deviceCount > 0) parts.push(`${info.deviceCount} device${info.deviceCount !== 1 ? 's' : ''}`);
    if (info.device) parts.push(String(info.device));
    if (info.probe?.ok === true) parts.push(pluginText('probeBackendProbeOk'));
    if (info.probe?.ok === false) parts.push(pluginText('probeBackendProbeFailed'));
    const value = available
      ? (parts.length ? `✓ ${parts.join(' · ')}` : '✓')
      : (parts.length ? `✗ ${parts.join(' · ')}` : '✗');
    backendItems.push({
      label: backendId,
      value,
      title: info.probe?.error || (available ? '' : pluginText('probeBackendNotAvailable')),
      tone: available ? (info.probe?.ok === false ? 'bad' : 'ok') : 'neutral',
    });
  }
  if (backendItems.length) groups.push({ groupKey: 'probeGroupBackends', items: backendItems });

  // ONNX Runtime
  const onnxItems: ProbeDetailItem[] = [];
  const onnx = data.onnxruntime;
  if (onnx?.available) {
    if (onnx.version) onnxItems.push({ label: 'onnxruntime', value: String(onnx.version) });
    if (onnx.providers?.length) onnxItems.push({ label: pluginText('probeProviders'), value: onnx.providers.join(', '), title: onnx.providers.join('\n') });
  } else if (onnx) {
    onnxItems.push({ label: 'onnxruntime', value: pluginText('probeBackendNotAvailable'), title: onnx.error || '', tone: 'bad' });
  }
  if (onnxItems.length) groups.push({ groupKey: 'probeGroupOnnx', items: onnxItems });

  // Packages
  const pkgItems: ProbeDetailItem[] = [];
  const packages = data.packages || {};
  const pkgOrder = ['torch', 'torchDirectML', 'onnxruntime', 'numpy', 'opencv-python', 'rawpy'];
  for (const pkgName of pkgOrder) {
    const info = packages[pkgName];
    if (!info) continue;
    if (info.available) {
      pkgItems.push({ label: pkgName, value: info.version || pluginText('probeBackendAvailable'), tone: 'ok' });
    } else {
      pkgItems.push({ label: pkgName, value: pluginText('probeBackendNotAvailable'), title: info.error || '', tone: 'bad' });
    }
  }
  if (pkgItems.length) groups.push({ groupKey: 'probeGroupPackages', items: pkgItems });

  return groups;
}

type ProbeAdvice = { text: string; kind: 'action' | 'diagnostic' };

function runtimeProbeAdvice(
  plugin: AiPluginSummary,
  profile: AiPluginInstallProfile,
  result?: AiPluginPythonRuntimeProbeResult,
): ProbeAdvice[] {
  if (!result) return [];
  const advice: ProbeAdvice[] = [];
  const state = result.state;
  const binding = selectedRuntimeBinding(plugin, profile);
  const backend = String(profile.backend || result.backend || '').toLowerCase();
  const backendInfo = runtimeProbeBackendInfo(result, backend);
  const data = result.result || {};
  const packages = data.packages || {};
  const torchPkg = packages.torch;
  const onnxPkg = packages.onnxruntime;
  const directmlPkg = packages.torchDirectML;
  const torchInfo = data.torch;

  // --- Stale cache ---
  if (state?.stale) {
    advice.push({ kind: 'action', text: pluginText('probeAdviceRunProbe') });
    if (state.staleReason === 'python_missing') advice.push({ kind: 'action', text: pluginText('probeAdviceSelectRuntime') });
    if (state.staleReason === 'fingerprint_changed') advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceFingerprintChanged') });
    if (state.staleReason === 'ttl_expired') advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceTtlExpired') });
    return advice;
  }

  // --- Tensor probe explicitly failed — takes priority over "available" ---
  if (backend !== 'cpu' && backendInfo?.probe?.ok === false) {
    const probeError = backendInfo.probe.error || '';
    advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceTensorProbeFailed') });
    if (probeError) advice.push({ kind: 'diagnostic', text: probeError });
    if (probeError.toLowerCase().includes('out of memory') || probeError.toLowerCase().includes('oom')) {
      advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceOom') });
    }
    advice.push({ kind: 'action', text: pluginText('probeAdviceRunProbe') });
    return advice;
  }

  // --- Available: next step is Smoke ---
  if (runtimeProbeAvailable(result)) {
    advice.push({ kind: 'action', text: pluginText('probeAdviceSmoke') });
    return advice;
  }

  // --- No Python binding selected ---
  if (!binding?.python) {
    advice.push({ kind: 'action', text: pluginText('probeAdviceSelectRuntime') });
  }

  // --- Probe script timed out ---
  if (result.error?.toLowerCase().includes('timed out')) {
    advice.push({ kind: 'action', text: pluginText('probeAdviceTimeout') });
  }

  // --- torch not installed or import failed ---
  if (torchPkg?.available === false) {
    if (torchPkg.error && !torchPkg.error.includes("No module named 'torch'")) {
      // torch exists but import crashed
      advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceTorchImportError') });
      advice.push({ kind: 'diagnostic', text: torchPkg.error });
    } else {
      advice.push({ kind: 'action', text: pluginText('probeAdviceInstallTorch') });
    }
    advice.push({ kind: 'action', text: pluginText('probeAdviceRunSetup') });
  }

  // --- ONNX Runtime not installed ---
  if (onnxPkg?.available === false) {
    if (backend === 'directml' || backendInfo?.providers === undefined) {
      advice.push({ kind: 'action', text: pluginText('probeAdviceInstallOnnx') });
    }
  }

  // --- DirectML specific failure ---
  if (backend === 'directml') {
    if (directmlPkg?.available === false) {
      advice.push({ kind: 'action', text: pluginText('probeAdviceInstallDirectML') });
    }
    if (directmlPkg?.available === true && backendInfo && !backendInfo.available) {
      advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceDirectMLInitFailed') });
    }
  }

  // --- Backend not available ---
  if (backend !== 'cpu' && backendInfo && !backendInfo.available) {
    // GPU device count is zero — driver or hardware issue
    if ((backend === 'cuda' || backend === 'rocm') && torchInfo?.cudaDeviceCount === 0) {
      advice.push({ kind: 'diagnostic', text: pluginText('probeAdviceGpuDeviceCountZero') });
    } else {
      advice.push({ kind: 'action', text: pluginText('probeAdviceBackendMissing').replace('{backend}', backend) });
    }
  }

  // --- Fallback ---
  if (advice.length === 0) {
    advice.push({ kind: 'action', text: pluginText('probeAdviceOpenDiagnostics') });
  }
  return advice;
}

function formatRuntimeProbeResult(result?: AiPluginPythonRuntimeProbeResult) {
  if (!result) return '';
  const parts = [
    runtimeProbeStatusLabel(result),
    `${result.durationMs ?? 0}ms`,
  ];
  if (result.state?.status) parts.push(result.state.status);
  parts.push(...runtimeProbeDetails(result).map((detail) => `${detail.label} ${detail.value}`));
  if (result.error) parts.push(result.error);
  return parts.filter(Boolean).join(' / ');
}

function pluginTaskStatusLabel(task: AiPluginTaskState) {
  const status = String(task.status || '').toLowerCase();
  const labels: Record<string, string> = {
    queued: pluginText('taskStatusQueued'),
    running: pluginText('taskStatusRunning'),
    cancelling: pluginText('taskStatusCancelling'),
    succeeded: pluginText('taskStatusSucceeded'),
    completed: pluginText('taskStatusSucceeded'),
    failed: pluginText('taskStatusFailed'),
    error: pluginText('taskStatusFailed'),
    cancelled: pluginText('taskStatusCancelled'),
    canceled: pluginText('taskStatusCancelled'),
    imported: pluginText('taskStatusImported'),
    discarded: pluginText('taskStatusDiscarded'),
  };
  return labels[status] || task.status || '-';
}

function pluginTaskBadgeClass(task: AiPluginTaskState) {
  const status = String(task.status || '').toLowerCase();
  if (['succeeded', 'completed', 'imported'].includes(status)) {
    return 'bg-success/10 border-success/20 text-success';
  }
  if (['failed', 'error'].includes(status)) {
    return 'bg-error/10 border-error/20 text-error';
  }
  if (['queued', 'running', 'cancelling'].includes(status)) {
    return 'bg-warning/10 border-warning/20 text-warning';
  }
  return 'bg-base-300/70 border-base-content/10 text-base-content/45';
}

function pluginTaskOutputCount(task: AiPluginTaskState) {
  const count = task.outputs?.length || 0;
  return `${count} ${count === 1 ? 'output' : 'outputs'}`;
}

function formatPluginTaskDetail(task: AiPluginTaskState) {
  const details = [
    task.message || '',
    task.errorCode || '',
    task.retryable ? 'retryable' : '',
    task.pluginStatus?.status ? `plugin:${task.pluginStatus.status}` : '',
    task.pluginStatusError ? 'plugin status error' : '',
  ];
  return details.filter(Boolean).join(' / ');
}

function formatPluginTaskTitle(task: AiPluginTaskState) {
  return [
    task.taskId,
    task.outputDir,
    task.errorCode,
    task.errorDomain,
    task.error,
    typeof task.progress === 'number' ? `${task.progress}%` : '',
    task.message,
    task.pluginStatusError,
    task.pluginStatus ? formatAiPluginStatus(task.pluginStatus) : '',
  ].filter(Boolean).join(' - ');
}

function formatAiPluginSmokeTest(smokeTest: AiPluginSmokeTest) {
  const parts = [];
  if (smokeTest.command) parts.push(smokeTest.command);
  if (smokeTest.capability) parts.push(smokeTest.capability);
  if (smokeTest.timeoutMs) parts.push(`${Math.round(smokeTest.timeoutMs / 1000)}s`);
  return parts.join(' - ') || pluginText('smokeTest');
}

function smokeTestRunningText(plugin: AiPluginSummary) {
  const seconds = Math.round((plugin.smokeTest?.timeoutMs || 120000) / 1000);
  return pluginText('smokeTestRunning', { seconds });
}

function formatSetupJobSummary(job: AiPluginSetupJob) {
  const status = job.status || 'needsVerify';
  const message = job.message || '';
  return [pluginText('setupProfile'), status, message].filter(Boolean).join(' - ');
}

function formatSetupPreviewWarnings(preview: AiPluginSetupPreview) {
  if (!preview.warnings?.length) return '';
  return pluginText('runSetupPreviewWarnings').replace('{warnings}', preview.warnings.join('\n'));
}

function formatSetupPreviewMessage(
  plugin: AiPluginSummary,
  profile: AiPluginInstallProfile,
  preview: AiPluginSetupPreview,
) {
  const profileLabel = profile.label || profile.id || profile.backend;
  return pluginText('runSetupConfirmMessage')
    .replace('{plugin}', plugin.name || plugin.id)
    .replace('{profile}', profileLabel)
    .replace('{backend}', preview.backend || profile.backend || '')
    .replace('{capability}', preview.capability || '')
    .replace('{runtime}', formatRuntimeBinding(preview.runtimeBinding || selectedRuntimeBinding(plugin, profile)))
    .replace('{python}', preview.runtimeBinding?.python || selectedRuntimeBinding(plugin, profile)?.python || '')
    .replace('{command}', preview.command || '')
    .replace('{workingDir}', preview.workingDir || '')
    .replace('{envDir}', preview.envPath || preview.envDir || '')
    .replace('{requirements}', preview.requirementsPath || preview.requirements || '')
    .replace('{warnings}', formatSetupPreviewWarnings(preview));
}

function profileBadgeClass(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const environmentStatus = profileEnvironmentStatus(plugin, profile);
  if (environmentStatus.level === 'ready' || environmentStatus.level === 'verified') {
    return 'bg-success/10 border-success/20 text-success';
  }
  if (environmentStatus.level === 'failed') {
    return 'bg-error/10 border-error/20 text-error';
  }
  if (environmentStatus.level === 'missing' || environmentStatus.level === 'notDetected') {
    return 'bg-base-100/50 border-base-content/10 text-base-content/35';
  }

  const recommended = pluginRecommendedProfile(plugin);
  if (recommended?.id === profile.id) {
    return 'bg-success/10 border-success/20 text-success';
  }
  if (profile.supportLevel === 'official') {
    return 'bg-primary/10 border-primary/20 text-primary';
  }
  if (profile.supportLevel === 'derived') {
    return 'bg-info/10 border-info/20 text-info';
  }
  if (profile.supportLevel === 'fallback') {
    return 'bg-base-100/50 border-base-content/10';
  }
  return 'bg-warning/10 border-warning/20 text-warning';
}

function pluginDiagnosticStatus(plugin: AiPluginSummary) {
  const diagnostics = plugin.id ? aiPluginDiagnostics.value[plugin.id]?.diagnostics : null;
  const statusFromDiagnostics = (diagnostics as any)?.status;
  if (statusFromDiagnostics) return statusFromDiagnostics;
  return plugin.id ? aiPluginStatuses.value[plugin.id]?.status : null;
}

function profileResultKey(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  return `${aiPluginKey(plugin)}:${profile.id}`;
}

function pluginSmokeResultProfiles(plugin: AiPluginSummary) {
  return (plugin.installProfiles || []).filter((profile) =>
    Boolean(aiPluginProfileSmokeResults.value[profileResultKey(plugin, profile)])
  );
}

function hydrateProfileSmokeResults(plugins: AiPluginSummary[]) {
  const next = { ...aiPluginProfileSmokeResults.value };
  for (const plugin of plugins) {
    for (const profile of plugin.installProfiles || []) {
      const state = profile.state;
      if (!state || !state.result) continue;
      next[profileResultKey(plugin, profile)] = {
        pluginId: state.pluginId,
        profileId: state.profileId,
        backend: state.backend,
        capability: state.capability,
        reachable: true,
        url: undefined,
        passed: Boolean(state.verified),
        durationMs: state.durationMs,
        result: state.result,
        error: state.error,
      };
    }
  }
  aiPluginProfileSmokeResults.value = next;
}

function profilePersistentState(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  return profile.state || null;
}

function profileBackendSatisfiedByStatus(profile: AiPluginInstallProfile, status: any) {
  const torch = status?.environment?.torch || status?.torch;
  const backend = String(profile.backend || '').toLowerCase();
  if (backend === 'cpu') return Boolean(torch?.available || status?.ready);
  if (backend === 'rocm') return Boolean(torch?.rocmAvailable || torch?.hipVersion);
  if (backend === 'cuda') return Boolean(torch?.cudaAvailable && !torch?.hipVersion);
  if (backend === 'directml') return Boolean(torch?.directmlAvailable || status?.environment?.directml?.available);
  if (backend === 'openvino') return Boolean(status?.environment?.openvino?.available);
  if (backend === 'mps') return Boolean(torch?.mpsAvailable || status?.environment?.mps?.available);
  return false;
}

function profileEnvironmentStatus(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  const smokeResult = aiPluginProfileSmokeResults.value[profileResultKey(plugin, profile)];
  if (smokeResult) {
    if (smokeResult.passed) {
      return {
        level: 'verified',
        label: pluginText('profileVerified'),
        title: `Smoke test passed in ${smokeResult.durationMs ?? 0}ms.`,
      };
    }
    return {
      level: 'failed',
      label: pluginText('profileFailed'),
      title: smokeResult.error || pluginText('smokeTestFailed'),
    };
  }

  const persistentState = profilePersistentState(plugin, profile);
  if (persistentState) {
    if (persistentState.status === 'verified') {
      return {
        level: 'verified',
        label: pluginText('profileVerified'),
        title: `Smoke test passed at ${persistentState.updatedAt}.`,
      };
    }
    if (persistentState.status === 'failed') {
      return {
        level: 'failed',
        label: pluginText('profileFailed'),
        title: persistentState.error || pluginText('smokeTestFailed'),
      };
    }
    if (persistentState.status === 'installing') {
      return {
        level: 'needsVerification',
        label: pluginText('profileInstalling'),
        title: 'Runtime setup task has been recorded and is waiting for verification.',
      };
    }
    if (persistentState.status === 'needsVerify') {
      return {
        level: 'needsVerification',
        label: pluginText('profileNeedsVerification'),
        title: 'Runtime setup state is recorded. Run Verify or Smoke to validate this profile.',
      };
    }
  }

  const probeResult = profileRuntimeProbeResult(plugin, profile);
  if (probeResult) {
    if (probeResult.available || probeBackendAvailable(probeResult, profile.backend)) {
      return {
        level: 'needsVerification',
        label: pluginText('profileNeedsVerification'),
        title: formatRuntimeProbeResult(probeResult),
      };
    }
    return {
      level: 'failed',
      label: pluginText('profileFailed'),
      title: formatRuntimeProbeResult(probeResult),
    };
  }

  const status = pluginDiagnosticStatus(plugin) as any;
  const backend = String(profile.backend || '').toLowerCase();
  const detectedBackends = new Set((aiPluginHostEnvironment.value?.candidateBackends || []).map((item) => item.toLowerCase()));
  const detected = backend === 'cpu' || detectedBackends.has(backend);

  if (status) {
    if (status.ready && profileBackendSatisfiedByStatus(profile, status)) {
      return {
        level: 'ready',
        label: pluginText('profileReady'),
        title: 'Plugin diagnostics report this runtime is available.',
      };
    }

    if (status.reason === 'dependency_missing') {
      return {
        level: 'missing',
        label: pluginText('profileMissingDependency'),
        title: status?.environment?.torch?.error || 'Plugin diagnostics report missing runtime dependencies.',
      };
    }

    if (status.reason === 'model_missing') {
      return {
        level: 'missing',
        label: pluginText('profileMissingModel'),
        title: 'Plugin diagnostics report missing model files.',
      };
    }

    if (status.reason === 'source_missing') {
      return {
        level: 'missing',
        label: pluginText('profileMissingSource'),
        title: 'Plugin diagnostics report missing source backend.',
      };
    }

    if (profileBackendSatisfiedByStatus(profile, status)) {
      return {
        level: 'needsVerification',
        label: pluginText('profileNeedsVerification'),
        title: 'Runtime appears installed, but smoke test has not been recorded.',
      };
    }

    if (status.reason || status.lastError) {
      return {
        level: 'failed',
        label: pluginText('profileFailed'),
        title: status.lastError || status.reason,
      };
    }
  }

  if (!detected) {
    return {
      level: 'notDetected',
      label: pluginText('profileNotDetected'),
      title: `Host probe did not detect backend: ${backend}.`,
    };
  }

  if (profile.supportLevel === 'fallback') {
    return {
      level: 'fallback',
      label: pluginText('profileFallback'),
      title: 'Fallback profile is available if faster runtimes fail.',
    };
  }

  return {
    level: 'needsDiagnostics',
    label: pluginText('profileNeedsDiagnostics'),
    title: 'Start the plugin or refresh diagnostics to verify this environment.',
  };
}

function profileActionKey(plugin: AiPluginSummary, profile: AiPluginInstallProfile, action: string) {
  return `${aiPluginKey(plugin)}:${profile.id}:${action}`;
}

async function withProfileActionLoading(
  plugin: AiPluginSummary,
  profile: AiPluginInstallProfile,
  action: string,
  task: () => Promise<void>,
) {
  const key = profileActionKey(plugin, profile, action);
  if (aiPluginProfileActionLoading.value[key]) return;
  aiPluginProfileActionLoading.value = {
    ...aiPluginProfileActionLoading.value,
    [key]: true,
  };
  try {
    await task();
  } finally {
    aiPluginProfileActionLoading.value = {
      ...aiPluginProfileActionLoading.value,
      [key]: false,
    };
  }
}

async function probeAiPluginProfileRuntime(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  await withProfileActionLoading(plugin, profile, 'probe', async () => {
    const binding = selectedRuntimeBinding(plugin, profile);
    if (!binding?.python) {
      toast.error(pluginText('runtimeBindingNone'));
      return;
    }
    setAiPluginExpanded(plugin, true);
    try {
      const result = await probeAiPluginPythonRuntime({
        python: binding.python,
        backend: profile.backend,
        pluginId: plugin.id,
        profileId: profile.id,
        capability: plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '',
        runtimeBinding: binding,
      });
      aiPluginRuntimeProbeResults.value = {
        ...aiPluginRuntimeProbeResults.value,
        [runtimeProbeKey(plugin, profile)]: result,
      };
      if (result?.available) {
        toast.success(pluginText('probeRuntimeSuccess'));
      } else {
        toast.error(result?.error || pluginText('probeRuntimeFailed'));
      }
      await loadAiPluginPanel(false);
    } catch (error: any) {
      toast.error(error?.message || String(error) || pluginText('probeRuntimeFailed'));
    }
  });
}

async function setupAiPluginProfile(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  await withProfileActionLoading(plugin, profile, 'setup', async () => {
    if (!plugin.id) return;
    setAiPluginExpanded(plugin, true);
    const capability = plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '';
    const state = await markAiPluginProfileSetupNeeded(plugin.id, {
      profileId: profile.id,
      backend: profile.backend,
      capability,
      runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
      runtimeBinding: selectedRuntimeBinding(plugin, profile),
    });
    if (state) {
      toast.info(pluginText('setupProfileQueued'));
      await loadAiPluginPanel(false);
    }
  });
}

async function runAiPluginProfileSetup(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  await withProfileActionLoading(plugin, profile, 'runSetup', async () => {
    if (!plugin.id || !plugin.install?.command) return;
    const permissions = getPluginPermissions(plugin);
    if (permissions.network?.setupDownloads) {
      const missing = missingPluginPermissionFlags(plugin, { setupDownloads: true });
      if (missing.length > 0) {
        const confirmed = await ask(
          `Plugin: ${plugin.name || plugin.id}\nProfile: ${profile.label || profile.id}\n\nThis setup may access the network to download dependencies or model files.\nDeclared domains: ${pluginAllowedDomainsText(plugin)}\n\nAllow setup downloads for this plugin?`,
          {
            title: 'Allow plugin setup downloads?',
            kind: 'warning',
            okLabel: 'Allow',
            cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
          },
        );
        if (!confirmed) return;
        await grantAiPluginPermissions(
          plugin.id,
          buildPluginPermissionGrantRequest(plugin, { setupDownloads: true }),
        );
      }
    }
    setAiPluginExpanded(plugin, true);
    const capability = plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '';
    let preview: AiPluginSetupPreview;
    try {
      preview = await previewAiPluginProfileSetupCommand(plugin.id, {
        profileId: profile.id,
        backend: profile.backend,
        capability,
        runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
        runtimeBinding: selectedRuntimeBinding(plugin, profile),
      });
    } catch (error: any) {
      toast.error(error?.message || String(error) || pluginText('runSetupPreviewFailed'));
      return;
    }

    if (preview.errors?.length) {
      toast.error(`${pluginText('runSetupPreviewBlocked')}\n${preview.errors.join('\n')}`);
      return;
    }

    const confirmed = await ask(formatSetupPreviewMessage(plugin, profile, preview), {
      title: pluginText('runSetupConfirmTitle'),
      kind: 'warning',
      okLabel: pluginText('runSetupProfile'),
      cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
    });
    if (!confirmed) return;

    // Start a polling loop to refresh setup job state while the command runs
    let pollStop = false;
    const pollInterval = setInterval(async () => {
      if (pollStop) return;
      await loadAiPluginPanel(false, true);
    }, 2000);

    // Track which plugin+profile is running setup so cancellation can find the job id
    aiPluginSetupRunningFor.value = { pluginId: plugin.id, profileId: profile.id };

    try {
      const state = await runAiPluginProfileSetupCommand(plugin.id, {
        profileId: profile.id,
        backend: profile.backend,
        capability,
        runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
        runtimeBinding: selectedRuntimeBinding(plugin, profile),
        allowCommandExecution: true,
      });
      if (state) {
        const status = String(state.status || '').toLowerCase();
        if (status === 'cancelled') {
          toast.info(pluginText('runSetupCancelled'));
        } else {
          toast.success(pluginText('runSetupSuccess'));
        }
        await loadAiPluginPanel(false, true);
      }
    } catch (error: any) {
      const msg = String(error?.message || error || '');
      if (msg.toLowerCase().includes('cancel')) {
        toast.info(pluginText('runSetupCancelled'));
      } else {
        toast.error(msg || pluginText('runSetupFailed'));
      }
      await loadAiPluginPanel(false, true);
    } finally {
      pollStop = true;
      clearInterval(pollInterval);
      aiPluginSetupRunningFor.value = null;
      await loadAiPluginPanel(false, true);
    }
  });
}

async function cancelAiPluginProfileSetup() {
  const running = aiPluginSetupRunningFor.value;
  if (!running?.pluginId || !running?.profileId) return;
  // Find the running setup job from the current plugin list
  const plugin = aiPlugins.value.find((p: any) => p?.id === running.pluginId);
  const profile = plugin?.installProfiles?.find((p: any) => p?.id === running.profileId);
  const jobId = profile?.setupJob?.id;
  if (!jobId) return;
  try {
    await cancelAiPluginSetup(jobId);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

async function copyAiPluginSetupLog(profile: AiPluginInstallProfile) {
  const text = profile?.setupJob?.log?.join('\n') || '';
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    toast.success(pluginText('copySetupLogSuccess'));
  } catch (error: any) {
    toast.error(error?.message || String(error) || pluginText('copySetupLogFailed'));
  }
}

async function verifyAiPluginProfile(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  await withProfileActionLoading(plugin, profile, 'verify', async () => {
    if (!plugin.id) return;
    setAiPluginExpanded(plugin, true);

    const status = await startAiPlugin(plugin.id, {
      profileId: profile.id,
      backend: profile.backend,
      capability: plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '',
      runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
      runtimeBinding: selectedRuntimeBinding(plugin, profile),
    });
    if (status) {
      aiPluginStatuses.value = {
        ...aiPluginStatuses.value,
        [plugin.id]: status,
      };
      pluginStore.setStatus(plugin.id, status);
    }

    const diagnostics = await getAiPluginDiagnostics(plugin.id);
    if (diagnostics) {
      aiPluginDiagnostics.value = {
        ...aiPluginDiagnostics.value,
        [plugin.id]: diagnostics,
      };
    }
    if (status?.reachable) {
      toast.success(pluginText('verifyProfileSuccess'));
    } else {
      toast.error(aiPluginStatusError(status) || pluginText('verifyProfileFailed'));
    }
  });
}

async function runAiPluginProfileSmokeTest(plugin: AiPluginSummary, profile: AiPluginInstallProfile) {
  await withProfileActionLoading(plugin, profile, 'smoke', async () => {
    if (!plugin.id) return;
    setAiPluginExpanded(plugin, true);
    const capability = plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '';
    if (!capability) {
      toast.error(pluginText('noCapabilities'));
      return;
    }

    const result = await smokeTestAiPlugin(plugin.id, {
      profileId: profile.id,
      backend: profile.backend,
      capability,
      runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
      runtimeBinding: selectedRuntimeBinding(plugin, profile),
    });
    if (!result) {
      toast.error(pluginText('smokeTestFailed'));
      return;
    }

    aiPluginProfileSmokeResults.value = {
      ...aiPluginProfileSmokeResults.value,
      [profileResultKey(plugin, profile)]: result,
    };

    const startupStatus = result.startupStatus;
    if (startupStatus) {
      aiPluginStatuses.value = {
        ...aiPluginStatuses.value,
        [plugin.id]: startupStatus,
      };
      pluginStore.setStatus(plugin.id, startupStatus);
    } else {
      const status = await getAiPluginStatus(plugin.id);
      if (status) {
        aiPluginStatuses.value = {
          ...aiPluginStatuses.value,
          [plugin.id]: status,
        };
        pluginStore.setStatus(plugin.id, status);
      }
    }

    if (result.passed) {
      toast.success(pluginText('smokeTestSuccess'));
    } else {
      toast.error(result.error || pluginText('smokeTestFailed'));
    }
    await loadAiPluginPanel(false);
  });
}

function shortPath(path: string) {
  if (!path) return '';
  return path.split(/[\\/]/).filter(Boolean).pop() || path;
}

function getPluginOutputPath(result: any) {
  const payload = result?.result || result;
  const outputs = Array.isArray(payload?.outputs) ? payload.outputs : [];
  const output = outputs.find((item: any) => item?.kind === 'image' && item?.path) || outputs.find((item: any) => item?.path);
  return output?.path ? String(output.path) : '';
}

function getPluginTaskId(result: any) {
  return String(result?.taskId || result?.result?.taskId || '');
}

function pluginTaskErrorMessage(task: any) {
  return task?.error || task?.pluginStatus?.error?.message || `Plugin task ended with status ${task?.status || 'unknown'}.`;
}

async function waitForPluginTaskOutput(pluginId: string, invokeResult: any) {
  let outputPath = getPluginOutputPath(invokeResult);
  if (outputPath) return outputPath;

  const taskId = getPluginTaskId(invokeResult) || String(invokeResult?.taskState?.taskId || '');
  if (!taskId) return '';

  for (let attempt = 0; attempt < 120; attempt += 1) {
    const response = await getAiPluginTask(pluginId, taskId);
    const task = {
      ...(response?.state || {}),
      pluginStatus: response?.pluginStatus,
      pluginStatusError: response?.pluginStatusError,
    };
    outputPath = getPluginOutputPath({ result: { outputs: task.outputs || response?.pluginStatus?.outputs || response?.pluginStatus?.state?.outputs || [] } });
    updateAiPluginTaskInPanel(pluginId, task);
    if (task.status === 'succeeded' && outputPath) return outputPath;
    if (['failed', 'cancelled', 'canceled'].includes(String(task.status || '').toLowerCase())) {
      throw new Error(pluginTaskErrorMessage(task));
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  throw new Error('Plugin task did not finish in time.');
}

async function discardAiPluginTask(plugin: AiPluginSummary, task: AiPluginTaskState) {
  if (!plugin.id || !task.taskId) return;
  try {
    await discardAiPluginTaskOutputs(plugin.id, {
      taskId: task.taskId,
      deleteTaskDir: true,
    });
    toast.success(pluginText('discardTaskSuccess'));
    await loadAiPluginPanel(false);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

async function retryAiPluginTaskFromLedger(plugin: AiPluginSummary, task: AiPluginTaskState) {
  if (!plugin.id || !task.taskId) return;
  try {
    await retryAiPluginTask(plugin.id, task.taskId);
    toast.success(pluginText('retryTaskSuccess'));
    await loadAiPluginPanel(false);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

function updateAiPluginTaskInPanel(pluginId: string, nextTask: AiPluginTaskState) {
  aiPlugins.value = aiPlugins.value.map((plugin) => {
    if (plugin.id !== pluginId) return plugin;
    const taskStates = (plugin.taskStates || []).map((task) => (
      task.taskId === nextTask.taskId ? { ...task, ...nextTask } : task
    ));
    return { ...plugin, taskStates };
  });
  pluginStore.plugins = aiPlugins.value;
}

async function refreshAiPluginTask(plugin: AiPluginSummary, task: AiPluginTaskState) {
  if (!plugin.id || !task.taskId) return;
  const key = `${plugin.id}:${task.taskId}`;
  if (aiPluginTaskLoading.value[key]) return;
  aiPluginTaskLoading.value = {
    ...aiPluginTaskLoading.value,
    [key]: true,
  };
  try {
    const response = await getAiPluginTask(plugin.id, task.taskId);
    const nextTask = {
      ...(response?.state || task),
      pluginStatus: response?.pluginStatus,
      pluginStatusError: response?.pluginStatusError,
    };
    updateAiPluginTaskInPanel(plugin.id, nextTask);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginTaskLoading.value = {
      ...aiPluginTaskLoading.value,
      [key]: false,
    };
  }
}

async function cancelAiPluginTaskFromLedger(plugin: AiPluginSummary, task: AiPluginTaskState) {
  if (!plugin.id || !task.taskId) return;
  try {
    await cancelAiPluginTask(plugin.id, task.taskId);
    toast.info(pluginText('cancelTaskSuccess'));
    await loadAiPluginPanel(false);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

async function loadAiPluginPanel(refreshStatus = false, silent = false) {
  if (silent) {
    if (isRefreshingAiPlugins.value || isLoadingAiPlugins.value) return;
    isRefreshingAiPlugins.value = true;
  } else {
    if (isLoadingAiPlugins.value) return;
    isLoadingAiPlugins.value = true;
  }

  try {
    const [registry, plugins, hostEnvironment, storeInfo, trustedPublishers] = await Promise.all([
      getAiPluginRegistry(),
      listAiPlugins(),
      getAiPluginHostEnvironment(),
      getAiPluginStoreInfo(),
      listTrustedPublishers(),
    ]);
    aiPluginRegistryPaths.value = registry?.registeredPaths || [];
    aiPluginTrustedPublishers.value = trustedPublishers || [];
    aiPlugins.value = Array.isArray(plugins) ? plugins : [];
    aiPluginStoreInfo.value = storeInfo || null;
    hydrateProfileSmokeResults(aiPlugins.value);
    const nextExpanded = { ...expandedAiPluginKeys.value };
    for (const plugin of aiPlugins.value) {
      if (!plugin.validation.valid || plugin.validation.warnings.length > 0 || !plugin.platformSupported) {
        nextExpanded[aiPluginKey(plugin)] = true;
      }
    }
    expandedAiPluginKeys.value = nextExpanded;
    aiPluginHostEnvironment.value = hostEnvironment || null;
    pluginStore.plugins = aiPlugins.value;
    pluginStore.loaded = true;

    if (refreshStatus) {
      await refreshVisibleAiPluginStatuses();
    }
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    if (silent) {
      isRefreshingAiPlugins.value = false;
    } else {
      isLoadingAiPlugins.value = false;
    }
  }
}

async function loadAiPluginHostEnvironment() {
  if (isLoadingAiPluginHostEnvironment.value) return;
  isLoadingAiPluginHostEnvironment.value = true;

  try {
    aiPluginHostEnvironment.value = await getAiPluginHostEnvironment();
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    isLoadingAiPluginHostEnvironment.value = false;
  }
}

async function refreshVisibleAiPluginStatuses() {
  const statusablePlugins = aiPlugins.value.filter((plugin) => plugin.id && plugin.validation.valid);
  await Promise.all(statusablePlugins.map((plugin) => refreshAiPluginStatus(plugin, false)));
}

async function refreshAiPluginStatus(plugin: AiPluginSummary, showErrors = true) {
  if (!plugin.id || aiPluginStatusLoading.value[plugin.id]) return;

  aiPluginStatusLoading.value = {
    ...aiPluginStatusLoading.value,
    [plugin.id]: true,
  };

  try {
    const status = await getAiPluginStatus(plugin.id);
    if (status) {
      aiPluginStatuses.value = {
        ...aiPluginStatuses.value,
        [plugin.id]: status,
      };
      pluginStore.setStatus(plugin.id, status);
    }
  } catch (error: any) {
    if (showErrors) {
      toast.error(error?.message || String(error));
    }
  } finally {
    aiPluginStatusLoading.value = {
      ...aiPluginStatusLoading.value,
      [plugin.id]: false,
    };
  }
}

async function refreshAiPluginDiagnostics(plugin: AiPluginSummary) {
  if (!plugin.id || aiPluginDiagnosticsLoading.value[plugin.id]) return;
  setAiPluginExpanded(plugin, true);

  aiPluginDiagnosticsLoading.value = {
    ...aiPluginDiagnosticsLoading.value,
    [plugin.id]: true,
  };

  try {
    const diagnostics = await getAiPluginDiagnostics(plugin.id);
    if (diagnostics) {
      aiPluginDiagnostics.value = {
        ...aiPluginDiagnostics.value,
        [plugin.id]: diagnostics,
      };
    }
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginDiagnosticsLoading.value = {
      ...aiPluginDiagnosticsLoading.value,
      [plugin.id]: false,
    };
  }
}

async function refreshAiPluginLogs(plugin: AiPluginSummary) {
  if (!plugin.id || aiPluginLogsLoading.value[plugin.id]) return;
  setAiPluginExpanded(plugin, true);

  aiPluginLogsLoading.value = {
    ...aiPluginLogsLoading.value,
    [plugin.id]: true,
  };

  try {
    const logs = await getAiPluginLogs(plugin.id);
    if (logs) {
      aiPluginLogs.value = {
        ...aiPluginLogs.value,
        [plugin.id]: logs,
      };
    }
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginLogsLoading.value = {
      ...aiPluginLogsLoading.value,
      [plugin.id]: false,
    };
  }
}

async function startAiPluginRuntime(plugin: AiPluginSummary) {
  if (!plugin.id || aiPluginRuntimeLoading.value[plugin.id]) return;
  aiPluginRuntimeLoading.value = {
    ...aiPluginRuntimeLoading.value,
    [plugin.id]: true,
  };

  try {
    const permissions = getPluginPermissions(plugin);
    if (permissions.network?.runtime) {
      const missing = missingPluginPermissionFlags(plugin, { runtimeNetwork: true });
      if (missing.length > 0) {
        const confirmed = await ask(
          `Plugin: ${plugin.name || plugin.id}\n\nThis plugin declares runtime network access.\nDeclared domains: ${pluginAllowedDomainsText(plugin)}\n\nAllow runtime network access for this plugin?`,
          {
            title: 'Allow plugin network access?',
            kind: 'warning',
            okLabel: 'Allow',
            cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
          },
        );
        if (!confirmed) return;
        await grantAiPluginPermissions(
          plugin.id,
          buildPluginPermissionGrantRequest(plugin, { runtimeNetwork: true }),
        );
      }
    }
    const profile = pluginStartProfile(plugin);
    const status = await startAiPlugin(plugin.id, profile ? {
      profileId: profile.id,
      backend: profile.backend,
      capability: plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '',
      runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
      runtimeBinding: selectedRuntimeBinding(plugin, profile),
    } : undefined);
    if (status) {
      aiPluginStatuses.value = {
        ...aiPluginStatuses.value,
        [plugin.id]: status,
      };
      pluginStore.setStatus(plugin.id, status);
    }
    if (status?.reachable) {
      toast.success(pluginText('startSuccess'));
    } else {
      toast.error(aiPluginStatusError(status) || pluginText('startFailed'));
    }
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginRuntimeLoading.value = {
      ...aiPluginRuntimeLoading.value,
      [plugin.id]: false,
    };
  }
}

async function stopAiPluginRuntime(plugin: AiPluginSummary) {
  if (!plugin.id || aiPluginRuntimeLoading.value[plugin.id]) return;
  aiPluginRuntimeLoading.value = {
    ...aiPluginRuntimeLoading.value,
    [plugin.id]: true,
  };

  try {
    const status = await stopAiPlugin(plugin.id);
    if (status) {
      aiPluginStatuses.value = {
        ...aiPluginStatuses.value,
        [plugin.id]: status,
      };
      pluginStore.setStatus(plugin.id, status);
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
    await refreshAiPluginStatus(plugin, false);
    await pluginStore.loadPlugins(true);
    toast.success(pluginText('stopSuccess'));
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginRuntimeLoading.value = {
      ...aiPluginRuntimeLoading.value,
      [plugin.id]: false,
    };
  }
}

async function restartAiPluginRuntime(plugin: AiPluginSummary) {
  if (!plugin.id || aiPluginRuntimeLoading.value[plugin.id]) return;
  aiPluginRuntimeLoading.value = {
    ...aiPluginRuntimeLoading.value,
    [plugin.id]: true,
  };

  try {
    await stopAiPlugin(plugin.id);
    await new Promise((resolve) => setTimeout(resolve, 300));
    const profile = pluginStartProfile(plugin);
    const status = await startAiPlugin(plugin.id, profile ? {
      profileId: profile.id,
      backend: profile.backend,
      capability: plugin.smokeTest?.capability || plugin.capabilities?.[0]?.id || '',
      runtimeBindingId: selectedRuntimeBindingId(plugin, profile),
      runtimeBinding: selectedRuntimeBinding(plugin, profile),
    } : undefined);
    if (status) {
      aiPluginStatuses.value = {
        ...aiPluginStatuses.value,
        [plugin.id]: status,
      };
      pluginStore.setStatus(plugin.id, status);
    }
    await refreshAiPluginStatus(plugin, false);
    await pluginStore.loadPlugins(true);
    if (status?.reachable) {
      toast.success(pluginText('restartSuccess'));
    } else {
      toast.error(aiPluginStatusError(status) || pluginText('restartFailed'));
    }
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginRuntimeLoading.value = {
      ...aiPluginRuntimeLoading.value,
      [plugin.id]: false,
    };
  }
}

async function testInvokeAiPluginCapability(plugin: AiPluginSummary, capability: AiPluginCapability) {
  const key = `${plugin.id}:${capability.id}`;
  if (!plugin.id || !capability.id || aiPluginInvokeLoading.value[key]) return;

  aiPluginInvokeLoading.value = {
    ...aiPluginInvokeLoading.value,
    [key]: true,
  };

  try {
    const result = await invokeAiPluginCapability(plugin.id, capability.id, {
      inputs: {},
      parameters: {},
    });
    console.log('invokeAiPluginCapability result:', result);
    toast.success(pluginText('invokeSuccess'));
    await refreshAiPluginStatus(plugin, false);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginInvokeLoading.value = {
      ...aiPluginInvokeLoading.value,
      [key]: false,
    };
  }
}

async function chooseAiPluginDirectory() {
  const result = await openDialog({
    title: pluginText('addDirectory'),
    multiple: false,
    directory: true,
  });

  if (!result || Array.isArray(result)) return;

  try {
    const registry = await registerAiPluginPath(result);
    aiPluginRegistryPaths.value = registry?.registeredPaths || [];
    await loadAiPluginPanel(true);
    toast.success(pluginText('addSuccess'));
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

async function chooseAiPluginPackage() {
  if (isLoadingAiPlugins.value) return;
  const result = await openDialog({
    title: pluginText('installPackage'),
    multiple: false,
    directory: false,
    filters: [{ name: 'PicAiPic Plugin Package', extensions: ['zip'] }],
  });

  if (!result || Array.isArray(result)) return;

  try {
    await installAiPluginPackageWithTrust(result);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

async function installAiPluginPackageWithTrust(packagePath: string) {
  try {
    const installResult = await installAiPluginPackage(packagePath);
    aiPluginRegistryPaths.value = installResult?.registeredPaths || [];
    await loadAiPluginPanel(true);
    const warnings = [
      ...(installResult?.packageWarnings || []),
      ...(installResult?.validation?.warnings || []),
    ];
    if (warnings.length > 0) {
      console.warn('Plugin package installed with warnings:', warnings);
    }
    toast.success(pluginText('installPackageSuccess'));
    await promptInstalledPluginModels(installResult);
  } catch (error: any) {
    const msg = error?.message || String(error);
    // Check for trust-required error: TRUST_REQUIRED:<publisher>:<publicKey>:<pluginId>
    if (msg.startsWith('TRUST_REQUIRED:')) {
      const parts = msg.split(':');
      const publisher = parts[1] || 'unknown';
      const publicKey = parts[2] || '';
      const confirmed = await ask(
        pluginText('trustPublisherMessage')
          .replace('{publisher}', publisher)
          .replace('{key}', publicKey.slice(0, 24) + '...'),
        {
          title: pluginText('trustPublisherTitle'),
          kind: 'warning',
          okLabel: pluginText('trustPublisher'),
          cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
        },
      );
      if (!confirmed) return;
      await trustPublisher(publisher, publicKey);
      aiPluginTrustedPublishers.value = await listTrustedPublishers();
      toast.success(pluginText('trustPublisherSuccess'));
      // Retry install after trusting.
      return installAiPluginPackageWithTrust(packagePath);
    }
    throw error;
  }
}

async function uninstallInstalledAiPlugin(plugin: AiPluginSummary) {
  if (!plugin.id || isLoadingAiPlugins.value) return;

  const mode = await requestUninstallMode(plugin.name || plugin.id, plugin.path || '');
  if (mode === 'cancel') return;

  try {
    aiPluginRuntimeLoading.value = {
      ...aiPluginRuntimeLoading.value,
      [plugin.id]: true,
    };
    const result = await uninstallAiPlugin(plugin.id, mode);
    aiPluginRegistryPaths.value = result?.registeredPaths || [];
    const { [plugin.id]: _removedStatus, ...remainingStatuses } = aiPluginStatuses.value;
    const { [plugin.id]: _removedDiagnostics, ...remainingDiagnostics } = aiPluginDiagnostics.value;
    const { [plugin.id]: _removedLogs, ...remainingLogs } = aiPluginLogs.value;
    aiPluginStatuses.value = remainingStatuses;
    aiPluginDiagnostics.value = remainingDiagnostics;
    aiPluginLogs.value = remainingLogs;
    aiPlugins.value = aiPlugins.value.filter((item) => item.id !== plugin.id);
    pluginStore.removePlugin(plugin.id);
    await loadAiPluginPanel(true);
    toast.success(
      mode === 'code_and_data'
        ? pluginText('uninstallSuccessCodeAndData')
        : pluginText('uninstallSuccessCodeOnly'),
    );
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    aiPluginRuntimeLoading.value = {
      ...aiPluginRuntimeLoading.value,
      [plugin.id]: false,
    };
  }
}

function requestUninstallMode(
  plugin: string,
  path: string,
): Promise<'code_only' | 'code_and_data' | 'cancel'> {
  uninstallModeDialog.value = { show: true, plugin, path };
  return new Promise((resolve) => {
    uninstallModeResolver = resolve;
  });
}

function resolveUninstallMode(result: { mode: 'code_only' | 'code_and_data' | 'cancel' }) {
  uninstallModeDialog.value.show = false;
  uninstallModeResolver?.(result.mode);
  uninstallModeResolver = null;
}

async function removeAiPluginDirectory(path: string) {
  try {
    const registry = await unregisterAiPluginPath(path);
    aiPluginRegistryPaths.value = registry?.registeredPaths || [];
    await loadAiPluginPanel(true);
    toast.success(pluginText('removeSuccess'));
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

async function removeAiPluginTrustedPublisher(publisher: string) {
  try {
    aiPluginTrustedPublishers.value = await removeTrustedPublisher(publisher);
  } catch (error: any) {
    toast.error(error?.message || String(error));
  }
}

type ShortcutDisplayItem = {
  actionId: ShortcutActionId;
  labelKey: string;
  keys?: string[];
};

const shortcutDisplaySections: Array<{ key: string; items: ShortcutDisplayItem[] }> = [
  {
    key: 'global',
    items: [
      { actionId: 'app.sidebar.toggle', labelKey: 'toggle_sidebar' },
      { actionId: 'app.preferences', labelKey: 'open_settings' },
      { actionId: 'app.scale.increase', labelKey: 'font_increase' },
      { actionId: 'app.scale.decrease', labelKey: 'font_decrease' },
      { actionId: 'app.scale.reset', labelKey: 'font_reset' },
      { actionId: 'app.search', labelKey: 'search' },
    ],
  },
  {
    key: 'image_browsing',
    items: [
      { actionId: 'view.previous', labelKey: 'previous_image' },
      { actionId: 'view.next', labelKey: 'next_image' },
      { actionId: 'view.first', labelKey: 'first_image' },
      { actionId: 'view.last', labelKey: 'last_image' },
      { actionId: 'view.quickPreview', labelKey: 'quick_preview' },
      { actionId: 'view.close', labelKey: 'close_viewer' },
      { actionId: 'file.openNewWindow', labelKey: 'open_new_window' },
      { actionId: 'file.openExternalApp', labelKey: 'open_external_app' },
      { actionId: 'file.editImage', labelKey: 'edit_image' },
      { actionId: 'file.searchSimilar', labelKey: 'search_similar' },
    ],
  },
  {
    key: 'viewing',
    items: [
      { actionId: 'view.zoomIn', labelKey: 'zoom_in' },
      { actionId: 'view.zoomOut', labelKey: 'zoom_out' },
      { actionId: 'view.zoomFit', labelKey: 'zoom_fit' },
      { actionId: 'slideshow.toggle', labelKey: 'toggle_slideshow' },
    ],
  },
  {
    key: 'file_actions',
    items: [

      { actionId: 'file.rename', labelKey: 'rename_file' },
      { actionId: 'file.moveTo', labelKey: 'move_within_library' },
      { actionId: 'file.moveToFolder', labelKey: 'move_to_folder' },
      { actionId: 'file.copy', labelKey: 'copy_file' },
      { actionId: 'file.paste', labelKey: 'paste_file' },
      { actionId: 'file.reveal', labelKey: 'reveal_in_file_manager' },
      { actionId: 'file.trash', labelKey: 'move_to_trash' },
    ],
  },
  {
    key: 'selection',
    items: [
      { actionId: 'file.selectAll', labelKey: 'select_all' },
      { actionId: 'file.selectNone', labelKey: 'select_none' },
      { actionId: 'file.invertSelection', labelKey: 'invert_selection' },
    ],
  },
  {
    key: 'metadata',
    items: [
      { actionId: 'meta.favorite', labelKey: 'toggle_favorite' },
      { actionId: 'meta.rating.clear', labelKey: 'set_clear_rating', keys: ['0 ~ 5'] },
      { actionId: 'meta.tag', labelKey: 'edit_tags' },
      { actionId: 'meta.comment', labelKey: 'edit_comment' },
      { actionId: 'meta.rotate', labelKey: 'rotate' },
      { actionId: 'meta.info', labelKey: 'show_info' },
    ],
  },
];

const shortcutSections = computed(() => {
  const shortcutMessages = localeMsg.value.settings.shortcuts;
  return shortcutDisplaySections.map((section) => ({
    key: section.key,
    title: shortcutMessages.sections[section.key],
    items: section.items
      .map((item) => ({
        actionId: item.actionId,
        label: shortcutMessages.actions[getShortcutActionLabelKey(item)],
        keys: item.keys ?? getDisplayShortcutKeys(item.actionId),
      }))
      .filter((item) => item.keys.length > 0),
  }));
});

function getShortcutActionLabelKey(item: ShortcutDisplayItem): string {
  if (item.actionId === 'file.reveal' && shortcutPlatform === 'mac') {
    return 'reveal_in_finder';
  }
  return item.labelKey;
}

function getDisplayShortcutKeys(actionId: ShortcutActionId): string[] {
  const labels = getShortcutLabels(actionId, shortcutPlatform);
  const label = getPreferredShortcutLabel(actionId, labels);
  return splitShortcutLabel(label);
}

function getPreferredShortcutLabel(actionId: ShortcutActionId, labels: string[]): string {
  if (actionId === 'app.scale.increase') {
    return labels.find((label) => label.includes('+')) || labels[0] || '';
  }
  return labels[0] || '';
}

function splitShortcutLabel(label: string): string[] {
  if (!label) return [];
  if (shortcutPlatform === 'mac') {
    return splitMacShortcutLabel(label);
  }

  let normalized = label
    .replace(/←/g, 'Left')
    .replace(/→/g, 'Right')
    .replace(/↑/g, 'Up')
    .replace(/↓/g, 'Down');

  normalized = normalized
    .replace(/\+\+$/, '+Plus')
    .replace(/\+=$/, '+=')
    .replace(/\+-$/, '+Minus')
    .replace(/\+0$/, '+0')
    .replace(/\+,/g, '+Comma');

  return normalized
    .split('+')
    .filter(Boolean)
    .map((key) => {
      key = key.trim();
      if (key === 'Plus') return '+';
      if (key === 'Minus') return '-';
      if (key === 'Comma') return ',';
      if (key === 'Del') return 'Delete';
      return key;
    });
}

function splitMacShortcutLabel(label: string): string[] {
  const modifierKeys = new Set(['⌘', '⌥', '⇧', '⌃']);
  const keys: string[] = [];
  let remaining = label;

  while (remaining.length > 0 && modifierKeys.has(remaining[0])) {
    keys.push(remaining[0]);
    remaining = remaining.slice(1);
  }

  if (remaining.length > 0) {
    keys.push(remaining);
  }

  return keys;
}

const onImageSearchModelChange = async (event: Event) => {
  const select = event.target as HTMLSelectElement;
  const nextModel = Number(select.value || 0);
  const previousModel = Number(config.settings.imageSearch.model || 0);

  if (nextModel !== 1) {
    try {
      await setImageSearchModel(nextModel);
      config.settings.imageSearch.model = nextModel;
    } catch (error) {
      select.value = String(previousModel);
      toast.error(error?.message || String(error));
    }
    return;
  }

  if (isMultilingualModelAvailable.value) {
    try {
      await setImageSearchModel(nextModel);
      config.settings.imageSearch.model = nextModel;
    } catch (error) {
      select.value = String(previousModel);
      toast.error(error?.message || String(error));
    }
    return;
  }

  select.value = String(previousModel);
  const shouldDownload = await ask(
    localeMsg.value.settings.image_search.multilingual_model_download_message,
    {
      title: localeMsg.value.settings.image_search.multilingual_model_download_title,
      kind: 'info',
      okLabel: localeMsg.value.settings.image_search.download,
      cancelLabel: localeMsg.value.msgbox?.cancel || 'Cancel',
    },
  );

  if (!shouldDownload) {
    return;
  }

  await startMultilingualModelDownload(previousModel);
};

const startMultilingualModelDownload = async (previousModel: number) => {
  if (isDownloadingMultilingualModel.value) return;

  isDownloadingMultilingualModel.value = true;
  isCancelingMultilingualModelDownload.value = false;
  multilingualModelDownloadProgress.value = 0;
  multilingualModelDownloadedBytes.value = 0;
  multilingualModelTotalBytes.value = 0;

  try {
    await downloadMultilingualImageSearchModel();
    isDownloadingMultilingualModel.value = false;
    isMultilingualModelAvailable.value = true;
    await setImageSearchModel(1);
    config.settings.imageSearch.model = 1;
    multilingualModelDownloadProgress.value = 100;
    if (multilingualModelTotalBytes.value > 0) {
      multilingualModelDownloadedBytes.value = multilingualModelTotalBytes.value;
    }
    toast.success(localeMsg.value.settings.image_search.multilingual_model_download_success);
  } catch (error) {
    if (isCancelingMultilingualModelDownload.value || String(error).includes('Download canceled')) {
      isCancelingMultilingualModelDownload.value = false;
      isDownloadingMultilingualModel.value = false;
      config.settings.imageSearch.model = previousModel;
      multilingualModelDownloadProgress.value = 0;
      multilingualModelDownloadedBytes.value = 0;
      multilingualModelTotalBytes.value = 0;
      return;
    }
    isDownloadingMultilingualModel.value = false;
    config.settings.imageSearch.model = previousModel;
    toast.error(error?.message || localeMsg.value.settings.image_search.multilingual_model_download_failed);
  }
};

const cancelMultilingualModelDownload = async () => {
  if (!isDownloadingMultilingualModel.value) return;

  isCancelingMultilingualModelDownload.value = true;
  isDownloadingMultilingualModel.value = false;
  multilingualModelDownloadProgress.value = 0;
  multilingualModelDownloadedBytes.value = 0;
  multilingualModelTotalBytes.value = 0;
  await cancelMultilingualImageSearchModelDownload();
};

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown);
  if (typeof config.settings.tabIndex !== 'number' || config.settings.tabIndex < 0 || config.settings.tabIndex >= settingsTabs.length) {
    config.settings.tabIndex = 0;
  }
  if (typeof config.settings.imageSearch.model !== 'number') {
    config.settings.imageSearch.model = 0;
  }
  unlistenImageSearchModelDownloadProgress = await listenImageSearchModelDownloadProgress((event: any) => {
    const progress = Number(event?.payload?.progress ?? 0);
    multilingualModelDownloadProgress.value = Math.max(0, Math.min(100, progress));
    multilingualModelDownloadedBytes.value = Math.max(0, Number(event?.payload?.downloadedBytes ?? 0));
    multilingualModelTotalBytes.value = Math.max(0, Number(event?.payload?.totalBytes ?? 0));
  });
  await syncImageSearchModelStatus();
  await loadAiPluginPanel(true);
  applyWindowScale(Number(config.settings.scale || 1));
  dbStorageDir.value = (await getDbStorageDir()) || '';
  hasCustomDbStorage.value = await isUsingCustomDbStorage();

  if (config.settings.externalImageAppPath) {
    try {
      config.settings.externalImageAppName = await getExternalAppDisplayName(config.settings.externalImageAppPath);
    } catch {
      config.settings.externalImageAppName = '';
    }
  }

  if (config.settings.externalVideoAppPath) {
    try {
      config.settings.externalVideoAppName = await getExternalAppDisplayName(config.settings.externalVideoAppPath);
    } catch {
      config.settings.externalVideoAppName = '';
    }
  }
  
  // Show window after mount
  await appWindow.show();
});

onUnmounted(() => {
  if (isDownloadingMultilingualModel.value) {
    void cancelMultilingualImageSearchModelDownload();
  }
  if (gridSizeEmitTimer) {
    clearTimeout(gridSizeEmitTimer);
    gridSizeEmitTimer = null;
  }
  if (unlistenImageSearchModelDownloadProgress) {
    unlistenImageSearchModelDownloadProgress();
    unlistenImageSearchModelDownloadProgress = null;
  }
  document.documentElement.style.fontSize = '';
  window.removeEventListener('keydown', handleKeyDown);
});

// general settings
watch(() => config.settings.tabIndex, (newValue) => {
  emit('settings-settingsTabIndex-changed', newValue);
});
watch(() => config.settings.appearance, (newValue) => {
  setTheme(newValue, newValue === 0 ? config.settings.lightTheme : config.settings.darkTheme);
  emit('settings-appearance-changed', newValue);
});
watch(() => config.settings.lightTheme, (newValue) => {
  setTheme(config.settings.appearance, newValue);
  emit('settings-lightTheme-changed', newValue);
});
watch(() => config.settings.darkTheme, (newValue) => {
  setTheme(config.settings.appearance, newValue);
  emit('settings-darkTheme-changed', newValue);
});
watch(() => config.settings.scale, (newValue) => {
  applyWindowScale(Number(newValue || 1));
  updateSettingsWindowSize(Number(newValue || 1));
  emit('settings-scale-changed', newValue);
});
watch(() => config.settings.externalImageAppPath, (newValue) => {
  emit('settings-externalImageAppPath-changed', newValue);
});
watch(() => config.settings.externalImageAppName, (newValue) => {
  emit('settings-externalImageAppName-changed', newValue);
});
watch(() => config.settings.externalVideoAppPath, (newValue) => {
  emit('settings-externalVideoAppPath-changed', newValue);
});
watch(() => config.settings.externalVideoAppName, (newValue) => {
  emit('settings-externalVideoAppName-changed', newValue);
});
watch(() => config.settings.language, (newValue) => {
  locale.value = newValue;
  emit('settings-language-changed', newValue);
});
watch(() => config.settings.showButtonText, (newValue) => {
  emit('settings-showButtonText-changed', newValue);
});
watch(() => config.settings.showToolTip, (newValue) => {
  emit('settings-showToolTip-changed', newValue);
});
watch(() => config.settings.showStatusBar, (newValue) => {
  emit('settings-showStatusBar-changed', newValue);
});
watch(() => config.settings.autoCheckUpdates, (newValue) => {
  emit('settings-autoCheckUpdates-changed', newValue);
});
// watch(() => config.settings.showComment, (newValue) => {
//   emit('settings-showComment-changed', newValue);
// });
watch(() => config.settings.debugMode, (newValue) => {
  emit('settings-debugMode-changed', newValue);
});
watch(() => config.settings.folderSort, (newValue) => {
  emit('settings-folderSort-changed', newValue);
});
watch(() => config.settings.calendarSort, (newValue) => {
  emit('settings-calendarSort-changed', newValue);
});
watch(() => config.settings.categorySort, (newValue) => {
  emit('settings-categorySort-changed', newValue);
});
watch(() => config.settings.showSubfolderFiles, (newValue) => {
  emit('settings-showSubfolderFiles-changed', newValue);
});

// grid view settings
watch(() => config.settings.grid.size, (newValue: number) => {
  if (gridSizeEmitTimer) {
    clearTimeout(gridSizeEmitTimer);
  }

  gridSizeEmitTimer = window.setTimeout(() => {
    emit('settings-gridSize-changed', newValue);
    gridSizeEmitTimer = null;
  }, 100);
});
watch(() => config.settings.grid.style, (newValue) => {
  emit('settings-gridStyle-changed', newValue);
});
watch(() => config.settings.grid.scaling, (newValue) => {
  emit('settings-gridScaling-changed', newValue);
});
watch(() => config.settings.grid.labelPrimary, (newValue) => {
  emit('settings-gridLabelPrimary-changed', newValue);
});
watch(() => config.settings.grid.labelSecondary, (newValue) => {
  emit('settings-gridLabelSecondary-changed', newValue);
});
watch(() => config.settings.grid.previewPosition, (newValue) => {
  emit('settings-filmStripViewPreviewPosition-changed', newValue);
});
watch(() => config.settings.grid.dateGrouping, (newValue) => {
  emit('settings-gridDateGrouping-changed', newValue);
});

// image viewer settings
watch(() => config.settings.mouseWheelMode, (newValue) => {
  emit('settings-mouseWheelMode-changed', newValue);
});
watch(() => config.settings.navigatorViewMode, (newValue) => {
  emit('settings-navigatorViewMode-changed', newValue);
});
watch(() => config.settings.navigatorViewSize, (newValue) => {
  emit('settings-navigatorViewSize-changed', newValue);
});
watch(() => config.settings.slideShowTransition, (newValue) => {
  emit('settings-slideShowTransition-changed', newValue);
});
watch(() => config.settings.autoPlayVideo, (newValue) => {
  emit('settings-autoPlayVideo-changed', newValue);
});
watch(() => config.settings.loopVideo, (newValue) => {
  emit('settings-loopVideo-changed', newValue);
});

// image search settings
watch(() => config.settings.imageSearch.model, (newValue) => {
  emit('settings-imageSearchModel-changed', newValue);
});
watch(() => config.settings.imageSearch.thresholdIndex, (newValue) => {
  emit('settings-imageSearchThresholdIndex-changed', newValue);
});
watch(() => config.settings.imageSearch.limit, (newValue) => {
  emit('settings-imageSearchLimit-changed', newValue);
});

// face settings
watch(() => config.settings.face.enabled, (newValue) => {
  emit('settings-faceEnabled-changed', newValue);
});
watch(() => config.settings.face.clusterThresholdIndex, (newValue) => {
  emit('settings-faceClusterThresholdIndex-changed', newValue);
});

// Handle keyboard shortcuts
function handleKeyDown(event: KeyboardEvent) {
  const navigationKeys = ['Tab', 'Escape'];
  
  // Disable default behavior for certain keys
  if (navigationKeys.includes(event.key)) {
    event.preventDefault();
  }

  switch (event.key) {
    case 'Tab':
      config.settings.tabIndex += 1;
      config.settings.tabIndex = config.settings.tabIndex % settingsTabs.length;
      break;
    case 'Escape':
      // Close the topmost dialog first
      if (showBackupDialog.value) { showBackupDialog.value = false; return; }
      if (showRestoreDialog.value) { showRestoreDialog.value = false; return; }
      if (showChangeDbStorageDialog.value) { showChangeDbStorageDialog.value = false; return; }
      if (showResetDbStorageDialog.value) { showResetDbStorageDialog.value = false; return; }
      appWindow.close(); // Close the window
      break;
  }
}

async function selectDbStorageDir() {
  if (Number(libConfig.index.status || 0) === 1) {
    toast.error(localeMsg.value.settings?.database?.busy_library_indexing || 'Cannot change the data location while library indexing is running.');
    return;
  }

  const faceIndexState = await isFaceIndexing();
  if (Array.isArray(faceIndexState) && faceIndexState[0] === true) {
    toast.error(localeMsg.value.settings?.database?.busy_face_indexing || 'Cannot change the data location while face indexing is running.');
    return;
  }

  showChangeDbStorageDialog.value = true;
}

async function chooseDbStorageDir() {
  showChangeDbStorageDialog.value = false;

  const result = await openDialog({
    title: localeMsg.value.settings?.database?.change_location || 'Move data to another folder',
    multiple: false,
    directory: true,
  });

  if (!result || Array.isArray(result) || isChangingDbStorage.value) return;

  try {
    isChangingDbStorage.value = true;
    const newPath = await changeDbStorageDir(result);
    dbStorageDir.value = String(newPath || result);
    hasCustomDbStorage.value = true;
    toast.success(localeMsg.value.settings?.database?.change_success || 'Library data has been moved successfully');
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    isChangingDbStorage.value = false;
  }
}

async function restoreDefaultDbStorageDir() {
  if (Number(libConfig.index.status || 0) === 1) {
    toast.error(localeMsg.value.settings?.database?.busy_library_indexing || 'Cannot change the data location while library indexing is running.');
    return;
  }

  const faceIndexState = await isFaceIndexing();
  if (Array.isArray(faceIndexState) && faceIndexState[0] === true) {
    toast.error(localeMsg.value.settings?.database?.busy_face_indexing || 'Cannot change the data location while face indexing is running.');
    return;
  }

  showResetDbStorageDialog.value = true;
}

async function confirmResetDbStorageDir() {
  showResetDbStorageDialog.value = false;

  try {
    isChangingDbStorage.value = true;
    const newPath = await resetDbStorageDir();
    dbStorageDir.value = String(newPath || '');
    hasCustomDbStorage.value = false;
    toast.success(localeMsg.value.settings?.database?.restore_default_success || 'Library data has been moved back to the default location');
  } catch (error: any) {
    toast.error(error?.message || String(error));
  } finally {
    isChangingDbStorage.value = false;
  }
}

async function selectExternalApp(kind: 'image' | 'video') {
  const result = await openDialog({
    title: kind === 'image'
      ? localeMsg.value.settings.image_view.external_image_editor
      : localeMsg.value.settings.image_view.external_video_app,
    multiple: false,
    directory: false,
    ...(isMac
      ? {
          defaultPath: '/Applications',
          filters: [{ name: 'Applications', extensions: ['app'] }],
        }
      : {}),
  });

  if (!result || Array.isArray(result)) return;
  let displayName = '';
  try {
    displayName = await getExternalAppDisplayName(result);
  } catch {}

  if (kind === 'image') {
    config.settings.externalImageAppPath = result;
    config.settings.externalImageAppName = displayName;
  } else {
    config.settings.externalVideoAppPath = result;
    config.settings.externalVideoAppName = displayName;
  }
}

function clearExternalApp(kind: 'image' | 'video') {
  if (kind === 'image') {
    config.settings.externalImageAppPath = '';
    config.settings.externalImageAppName = '';
  } else {
    config.settings.externalVideoAppPath = '';
    config.settings.externalVideoAppName = '';
  }
}

function normalizeScale(value: number) {
  return SCALE_VALUES.find((item) => item === Number(value)) ?? 1;
}

function applyWindowScale(scale: number) {
  const normalizedScale = normalizeScale(scale);
  document.documentElement.style.fontSize = `${normalizedScale * 16}px`;
}

async function updateSettingsWindowSize(scale: number) {
  const normalizedScale = normalizeScale(scale);
  const width = Math.round(SETTINGS_BASE_WIDTH * normalizedScale);
  const height = Math.round(SETTINGS_BASE_HEIGHT * normalizedScale);
  const size = new LogicalSize(width, height);

  await appWindow.setMinSize(size);
  await appWindow.setSize(size);
}

</script>
