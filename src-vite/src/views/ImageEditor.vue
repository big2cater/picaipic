<template>

  <div class="w-screen h-screen flex flex-col overflow-hidden bg-base-300 text-base-content/70">
    <!-- Title Bar -->
    <TitleBar
      v-if="showDesktopTitleBar"
      :titlebar="`${$t('msgbox.image_editor.title')} - ${shortenFilename(fileInfo?.name || '', 32)}`"
      :resizable="false"
      viewName="ImageEditor"
      class="shrink-0 z-50"
    />
    <div
      v-else
      class="h-10 shrink-0 flex items-center justify-center px-20 select-none"
      data-tauri-drag-region
    >
      <div class="min-w-0 max-w-full text-center text-sm font-medium text-base-content/70 truncate" data-tauri-drag-region>
        {{ $t('msgbox.image_editor.title') }} - {{ shortenFilename(fileInfo?.name || '', 32) }}
      </div>
    </div>

    <!-- Main Content -->
    <div v-if="fileInfo" class="flex-1 flex gap-3 p-3 min-h-0 select-none">
      <!-- Left: Image Preview -->
      <div class="flex-1 min-w-0 flex flex-col items-center justify-center gap-2">
        <div
          ref="containerRef"
          class="relative w-full flex-1 rounded-box overflow-hidden border border-base-content/5 bg-base-300/30 shadow-sm cursor-default"
          @pointerdown="handlePreviewPointerDown"
          @pointerup="handlePreviewPointerUp"
          @pointerleave="handlePreviewPointerUp"
          @pointercancel="handlePreviewPointerUp"
        >
            <transition name="fade">
              <div v-if="isProcessing" class="absolute inset-0 z-50 flex items-center justify-center bg-base-100/55 backdrop-blur-sm">
                <span class="loading loading-dots text-primary"></span>
              </div>
            </transition>

            <template v-if="imageSrc">
              <figure
                v-if="showDiffPreview && canShowDiffPreview"
                class="diff absolute inset-0 z-20 h-full w-full"
                tabindex="0"
              >
                <div class="diff-item-1 relative h-full w-full overflow-hidden">
                  <div class="absolute inset-0 overflow-hidden" :style="compareWindowStyle">
                    <img
                      :src="baseImageSrc"
                      :style="originalImageStyle"
                      class="block max-w-none"
                      draggable="false"
                    />
                  </div>
                </div>
                <div class="diff-item-2 relative h-full w-full overflow-hidden">
                  <div class="absolute inset-0 overflow-hidden" :style="compareWindowStyle">
                    <img
                      :src="displayImageSrc"
                      :style="adjustedImageStyle"
                      class="block max-w-none"
                      draggable="false"
                    />
                  </div>
                </div>
                <div class="diff-resizer"></div>
              </figure>

              <div
                v-if="showDiffPreview && canShowDiffPreview"
                class="pointer-events-none absolute z-50 rounded-box bg-base-100/80 px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-base-content/70 left-3 top-3"
              >
                {{ $t('msgbox.image_editor.original') }}
              </div>
              <div
                v-if="showDiffPreview && canShowDiffPreview"
                class="pointer-events-none absolute z-50 rounded-box bg-base-100/80 px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-base-content/70 right-3 top-3"
              >
                {{ currentPresetLabel || $t('msgbox.image_editor.adjusted') }}
              </div>
              <div
                v-if="imageReady && !(showDiffPreview && canShowDiffPreview) && currentPresetLabel"
                class="pointer-events-none absolute z-50 rounded-box bg-base-100/80 px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-base-content/70 right-3 top-3"
              >
                {{ currentPresetLabel }}
              </div>

              <img
                v-show="imageReady && !(showDiffPreview && canShowDiffPreview)"
                ref="imageRef"
                :src="displayImageSrc"
                :style="imageStyle"
                class="block"
                draggable="false"
                @load="onImageLoad"
              />
            </template>

            <div v-if="cropStatus === 1 || cropApplied"
              :class="[
                cropStatus === 1 ? 'crop-box-active' : 'crop-box-done',
                isResizing ? 'no-transition' : '',
                cropStatus === 1
                  ? (
                    cropBoxFixed
                      ? (isDragging ? 'cursor-grabbing no-transition' : 'cursor-grab')
                      : (isDragging ? 'cursor-move no-transition' : 'cursor-move')
                  )
                  : ''
              ]"
              :style="[
                cropBoxStyle,
                activeEditorTab === 'adjust' ? { pointerEvents: 'none', zIndex: 30 } : { zIndex: 40 }
              ]"
              @mousedown="cropStatus===1 ? startDrag('move', $event) : null"
              @dblclick="clickDoCrop"
            >
              <template v-if="cropStatus===1 && isDragging">
                <div class="crop-dimensions-display">
                  {{ crop.width }} x {{ crop.height }}
                </div>
                <div class="grid-lines">
                  <div class="grid-line-h grid-line-h-1"></div>
                  <div class="grid-line-h grid-line-h-2"></div>
                  <div class="grid-line-v grid-line-v-1"></div>
                  <div class="grid-line-v grid-line-v-2"></div>
                </div>
              </template>
              <template v-if="cropStatus===1 && !cropBoxFixed">
                <div class="drag-handle top-left" @mousedown.stop="startDrag('top-left', $event)"></div>
                <div class="drag-handle top" @mousedown.stop="startDrag('top', $event)"></div>
                <div class="drag-handle top-right" @mousedown.stop="startDrag('top-right', $event)"></div>
                <div class="drag-handle left" @mousedown.stop="startDrag('left', $event)"></div>
                <div class="drag-handle right" @mousedown.stop="startDrag('right', $event)"></div>
                <div class="drag-handle bottom-left" @mousedown.stop="startDrag('bottom-left', $event)"></div>
                <div class="drag-handle bottom" @mousedown.stop="startDrag('bottom', $event)"></div>
                <div class="drag-handle bottom-right" @mousedown.stop="startDrag('bottom-right', $event)"></div>
              </template>
            </div>
        </div>

      </div>

      <div
        class="w-[320px] flex flex-col gap-3 overflow-y-auto"
        :class="isProcessing ? 'pointer-events-none opacity-60' : ''"
      >
        <div class="sticky top-0 z-10 bg-base-300 border-b border-base-content/5 pb-1">
          <div role="tablist" class="sidebar-header-tabs">
            <button
              role="tab"
              :class="[
                'sidebar-header-tab',
                activeEditorTab === 'edit' ? 'tab-active' : '',
                cropStatus === 1 || isProcessing ? 'opacity-50 cursor-default' : '',
              ]"
              :disabled="cropStatus === 1 || isProcessing"
              @click="setActiveEditorTab('edit')"
            >{{ $t('msgbox.image_editor.tab_edit') }}</button>
            <button
              role="tab"
              :class="[
                'sidebar-header-tab',
                activeEditorTab === 'adjust' ? 'tab-active' : '',
                cropStatus === 1 || isProcessing ? 'opacity-50 cursor-default' : '',
              ]"
              :disabled="cropStatus === 1 || isProcessing"
              @click="setActiveEditorTab('adjust')"
            >{{ $t('msgbox.image_editor.tab_adjust') }}</button>
          </div>
        </div>

        <template v-if="activeEditorTab === 'edit'">
        <section class="rounded-box p-3 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.transform') }}</div>
            <TButton
              buttonSize="small"
              :icon="IconRestore"
              :disabled="cropStatus === 1 || !hasEditImageChanges || cropApplied"
              :tooltip="$t('msgbox.image_editor.reset')"
              @click="clickRestoreAll"
            />
          </div>

          <div class="flex gap-3">
            <TButton
              :icon="IconRotateLeft"
              :disabled="cropStatus === 1 || cropApplied"
              :tooltip="$t('msgbox.image_editor.rotate_left')"
              @click="clickRotate(-90)"
            />
            <TButton
              :icon="IconRotateRight"
              :disabled="cropStatus === 1 || cropApplied"
              :tooltip="$t('msgbox.image_editor.rotate_right')"
              @click="clickRotate(90)"
            />
            <TButton
              :icon="IconFlipHorizontal"
              :disabled="cropStatus === 1 || cropApplied"
              :tooltip="$t('msgbox.image_editor.flip_horizontal')"
              @click="clickFlipX"
            />
            <TButton
              :icon="IconFlipVertical"
              :disabled="cropStatus === 1 || cropApplied"
              :tooltip="$t('msgbox.image_editor.flip_vertical')"
              @click="clickFlipY"
            />
          </div>
        </section>

        <section class="rounded-box p-3 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.crop') }}</div>
            <TButton
              buttonSize="small"
              :icon="IconRestore"
              :disabled="cropStatus === 0 && !cropApplied"
              :tooltip="$t('msgbox.image_editor.reset')"
              @click="clearCrop"
            />
          </div>

          <div v-if="cropStatus === 0" class="flex items-center gap-2">
            <TButton
              :icon="IconCrop"
              :selected="cropApplied"
              :tooltip="cropApplied ? $t('msgbox.image_editor.restore') : $t('msgbox.image_editor.crop')"
              @click="toggleCropMode"
            />
            <div class="text-xs leading-5 text-base-content/30">
              {{ cropApplied ? $t('msgbox.image_editor.crop_applied_hint') : $t('msgbox.image_editor.crop_hint') }}
            </div>
          </div>

          <div v-else class="space-y-3">
            <div class="flex items-center gap-1">
              <TButton
                buttonSize="small"
                :icon="IconClose"
                :selected="true"
                :tooltip="$t('msgbox.image_editor.cancel_crop')"
                @click="clickCancelCrop"
              />

              <select
                :value="cropPresetSelectValue"
                class="select select-bordered select-sm flex-1 min-w-0"
                :disabled="cropBoxFixed"
                @change="onCropPresetSelectChange"
              >
                <option :value="FREE_CROP_PRESET_ID">{{ $t('msgbox.image_editor.crop_shape_custom') }}</option>
                <optgroup :label="$t('msgbox.image_editor.crop_ratio_group')">
                  <option v-for="option in ratioCropOptions" :key="option.value" :value="option.value">
                    {{ option.label }}
                  </option>
                </optgroup>
                <optgroup v-if="customCropOptions.length" :label="$t('msgbox.image_editor.crop_custom_ratio_group')">
                  <option v-for="option in customCropOptions" :key="option.value" :value="option.value">
                    {{ option.label }}
                  </option>
                </optgroup>
                <optgroup :label="$t('msgbox.image_editor.crop_photo_size_group')">
                  <option v-for="option in photoSizeCropOptions" :key="option.value" :value="option.value">
                    {{ option.label }}
                  </option>
                </optgroup>
                <optgroup :label="$t('msgbox.image_editor.options')">
                  <option :value="ADD_CUSTOM_RATIO_ID">{{ $t('msgbox.image_editor.crop_add_custom_ratio') }}</option>
                  <option :value="MANAGE_PHOTO_SIZES_ID">{{ $t('msgbox.image_editor.crop_manage_photo_sizes') }}</option>
                </optgroup>
              </select>

              <TButton
                buttonSize="small"
                :icon="IconCropLandscape"
                :disabled="cropBoxFixed"
                :tooltip="isPortrait ? $t('msgbox.image_editor.crop_shape_portrait') : $t('msgbox.image_editor.crop_shape_landscape')"
                :iconStyle="{ transform: `rotate(${isPortrait ? 90 : 0}deg)` }"
                @click="togglePortraitAndLandscape"
              />

              <TButton
                buttonSize="small"
                :icon="cropBoxFixed ? IconZoomOut : IconZoomIn"
                :tooltip="cropBoxFixed ? $t('msgbox.image_editor.zoom') : $t('msgbox.image_editor.zoom')"
                @click="toggleCropBoxFixed"
              />

              <TButton
                buttonSize="small"
                :icon="IconOk"
                :selected="true"
                :tooltip="$t('msgbox.image_editor.confirm_crop')"
                @click="clickDoCrop"
              />
            </div>

            <div v-if="cropTargetHint" class="text-[11px] leading-4 text-base-content/40">
              {{ cropTargetHint }}
            </div>
          </div>
        </section>

        <section class="rounded-box p-3 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.resize') }}</div>
            <TButton
              buttonSize="small"
              :icon="IconRestore"
              :disabled="cropStatus === 1 || !hasResizeChanges"
              :tooltip="$t('msgbox.image_editor.reset')"
              @click="resetResize"
            />
          </div>

          <div class="grid grid-cols-[1fr_auto_1fr] items-end gap-1">
            <div class="form-control w-full">
              <label class="label py-1">
                <span class="label-text text-xs font-medium opacity-70">{{ $t('msgbox.image_editor.width') }}</span>
              </label>
              <input
                v-model="resizeWidthInput"
                type="number"
                min="1"
                :max="maxResizeWidth"
                step="1"
                inputmode="numeric"
                class="input input-bordered input-sm w-full"
                :disabled="cropStatus === 1"
                @input="handleResizeWidthInput"
              />
            </div>

            <div class="pb-0.5">
              <TButton
                buttonSize="small"
                :icon="keepAspectRatio ? IconLink : IconLinkOff"
                :disabled="cropStatus === 1"
                :tooltip="$t('msgbox.image_editor.keep_aspect_ratio')"
                @click="keepAspectRatio = !keepAspectRatio"
              />
            </div>

            <div class="form-control w-full">
              <label class="label py-1">
                <span class="label-text text-xs font-medium opacity-70">{{ $t('msgbox.image_editor.height') }}</span>
              </label>
              <input
                v-model="resizeHeightInput"
                type="number"
                min="1"
                :max="maxResizeHeight"
                step="1"
                inputmode="numeric"
                class="input input-bordered input-sm w-full"
                :disabled="cropStatus === 1"
                @input="handleResizeHeightInput"
              />
            </div>
          </div>
        </section>
        </template>

        <template v-else>
        <section class="rounded-box p-3 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.histogram') }}</div>
          </div>

          <ImageHistogram
            ref="histogramRef"
            :source="histogramSource"
            :adjustments="histogramAdjustments"
            :crop="histogramCrop"
            :rotate="rotate"
            :flip-horizontal="isFlippedX"
            :flip-vertical="isFlippedY"
            :apply-adjustments="histogramApplyAdjustments"
          />
          <div
            v-if="histogramUsesHostPreview"
            class="text-[10px] text-base-content/40 leading-4"
          >{{ $t('msgbox.image_editor.histogram_host_preview_hint') }}</div>
        </section>

        <section class="rounded-box p-3 space-y-2 border border-base-content/5 shadow-sm bg-base-300/30">
          <div class="flex items-center justify-between gap-2">
            <span class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.presets.title') }}</span>
            <div class="flex items-center gap-1">
              <button type="button" class="t-button-default text-[10px]" :disabled="photoStyleBusy" @click="saveCurrentAsCustom">{{ $t('photo_style.save_as') }}</button>
              <button type="button" class="t-button-default text-[10px]" @click="showLutLibraryDialog = true">{{ $t('photo_style.lut_library_title') }}</button>
              <TButton
                buttonSize="small"
                :icon="IconSplitOn"
                :selected="showDiffPreview && canShowDiffPreview"
                :disabled="!hasAdjustmentChanges"
                :tooltip="$t('msgbox.image_editor.compare_view')"
                @click="toggleDiffPreview"
              />
              <TButton
                buttonSize="small"
                :icon="IconRestore"
                :disabled="!hasAdjustmentChanges"
                :tooltip="$t('msgbox.image_editor.reset')"
                @click.stop="resetAdjustments"
              />
            </div>
          </div>

          <div class="grid grid-cols-4 gap-1">
            <div
              v-for="option in presetOptions"
              :key="option.value"
              class="group min-w-0 cursor-pointer"
              @click="selectedPreset = option.value"
            >
              <div
                :class="[
                  'aspect-4/3 rounded-box border-2 transition-all duration-200 flex items-center justify-center overflow-hidden relative',
                  selectedPreset === option.value ? 'border-primary ring-2 ring-primary/20' : 'border-base-content/5 hover:border-base-content/20',
                ]"
              >
                <div class="w-full h-full bg-base-300 flex items-center justify-center overflow-hidden rounded-[inherit]">
                  <img
                    v-if="fileInfo.thumbnail"
                    :src="fileInfo.thumbnail"
                    class="w-full h-full rounded-box object-cover pointer-events-none"
                    :style="{ filter: presetThumbnailFilter(option.value) }"
                  />
                  <IconPalette v-else class="w-4 h-4 text-base-content/10" />
                </div>
              </div>
              <div
                class="mt-1 text-[9px] text-center truncate font-medium transition-colors uppercase tracking-tight"
                :class="selectedPreset === option.value ? 'text-primary' : 'text-base-content/70 group-hover:text-base-content'"
              >
                {{ option.label }}
              </div>
            </div>
          </div>
        </section>

        <section class="rounded-box p-3 space-y-2 border border-base-content/5 shadow-sm bg-base-300/30">
          <div class="flex items-center justify-between gap-2">
            <span class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.color_match') }}</span>
            <TButton
              buttonSize="small"
              :icon="IconRestore"
              :disabled="!hasColorMatch"
              :tooltip="$t('msgbox.image_editor.color_match_clear')"
              @click.stop="clearColorMatch"
            />
          </div>
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="t-button-default text-xs flex-1"
              :disabled="cropStatus === 1 || isProcessing || colorMatchBusy"
              @click="pickColorMatchReference"
            >{{ colorMatchReferencePath ? $t('msgbox.image_editor.color_match_change_ref') : $t('msgbox.image_editor.color_match_pick_ref') }}</button>
            <button
              v-if="colorMatchReferencePath"
              type="button"
              class="t-button-default text-xs"
              :disabled="colorMatchBusy"
              @click="clearColorMatch"
            >{{ $t('msgbox.image_editor.color_match_clear') }}</button>
          </div>
          <div v-if="colorMatchReferencePath" class="text-[10px] text-base-content/50 truncate" :title="colorMatchReferencePath">
            {{ colorMatchReferenceName }}
          </div>
          <div v-if="colorMatchError" class="text-[11px] text-error">{{ colorMatchError }}</div>
          <div v-if="colorMatchBusy" class="text-[11px] text-base-content/50 flex items-center gap-2">
            <span class="loading loading-spinner loading-xs"></span>
            {{ $t('msgbox.image_editor.color_match_computing') }}
          </div>
          <template v-if="colorMatchReferencePath">
            <div class="space-y-3 pt-1">
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ $t('msgbox.image_editor.color_match_intensity') }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="colorMatchIntensity" :min="0" :max="100" :step="1" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ colorMatchIntensity }}%</span>
                </div>
              </div>
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ $t('msgbox.image_editor.color_match_tone') }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="colorMatchTone" :min="0" :max="100" :step="1" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ colorMatchTone }}%</span>
                </div>
              </div>
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ $t('msgbox.image_editor.color_match_highlight') }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="colorMatchHighlight" :min="0" :max="100" :step="1" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ colorMatchHighlight }}%</span>
                </div>
              </div>
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ $t('msgbox.image_editor.color_match_shadow') }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="colorMatchShadow" :min="0" :max="100" :step="1" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ colorMatchShadow }}%</span>
                </div>
              </div>
              <label class="flex items-center gap-2 text-xs text-base-content/70 cursor-pointer select-none">
                <input v-model="colorMatchAutoWb" type="checkbox" class="checkbox checkbox-xs checkbox-primary" />
                {{ $t('msgbox.image_editor.color_match_auto_wb') }}
              </label>
              <button
                type="button"
                class="t-button-default text-xs w-full"
                :disabled="cropStatus === 1 || isProcessing || colorMatchBusy || colorMatchExporting"
                @click="exportColorMatchCube"
              >{{ colorMatchExporting ? $t('msgbox.image_editor.color_match_exporting_lut') : $t('msgbox.image_editor.color_match_export_lut') }}</button>
              <div class="text-[10px] text-base-content/40 leading-4">
                {{ $t('msgbox.image_editor.color_match_export_lut_hint') }}
              </div>
            </div>
          </template>
          <button
            v-else
            type="button"
            class="t-button-default text-xs w-full"
            :disabled="cropStatus === 1 || isProcessing || colorMatchExporting || !fileInfo"
            @click="exportColorMatchCube"
          >{{ colorMatchExporting ? $t('msgbox.image_editor.color_match_exporting_lut') : $t('msgbox.image_editor.color_match_export_lut_current') }}</button>
        </section>

        <section class="rounded-box p-3 space-y-2 border border-base-content/5 shadow-sm bg-base-300/30">
          <div class="flex items-center justify-between gap-2">
            <span class="text-[11px] font-bold uppercase tracking-[0.22em] text-base-content/30">{{ $t('msgbox.image_editor.adjustments') }}</span>
            <TButton
              buttonSize="small"
              :icon="IconRestore"
              :disabled="!hasAdjustmentChanges"
              :tooltip="$t('msgbox.image_editor.reset')"
              @click.stop="resetAdjustments"
            />
          </div>

          <div class="space-y-4 overflow-hidden">
            <div class="space-y-3">
              <div v-for="adj in lightSliders" :key="adj.key" class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ adj.label }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="adj.model.value" :min="adj.min" :max="adj.max" :step="adj.step" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ adj.valueDisplay }}</span>
                </div>
              </div>
            </div>

            <div class="h-px bg-base-content/5 mx-1"></div>

            <div class="space-y-3">
              <div v-for="adj in colorSliders" :key="adj.key" class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ adj.label }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="adj.model.value" :min="adj.min" :max="adj.max" :step="adj.step" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ adj.valueDisplay }}</span>
                </div>
              </div>
            </div>

            <div class="h-px bg-base-content/5 mx-1"></div>

            <div class="space-y-3">
              <div v-for="adj in effectSliders" :key="adj.key" class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ adj.label }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="adj.model.value" :min="adj.min" :max="adj.max" :step="adj.step" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ adj.valueDisplay }}</span>
                </div>
              </div>
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ $t('photo_style.filter') }}</div>
                <select v-model="selectedFilter" class="select select-bordered select-xs">
                  <option value="">{{ $t('photo_style.filter_none') }}</option>
                  <option value="grayscale">grayscale</option>
                  <option value="sepia">sepia</option>
                  <option value="invert">invert</option>
                </select>
              </div>
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-start">
                <div class="font-medium text-base-content/70 tracking-wide text-xs pt-1">{{ $t('photo_style.lut') }}</div>
                <div class="flex flex-col gap-1 min-w-0">
                  <div class="input input-bordered input-xs w-full min-w-0 truncate" :title="activeLutLabel">{{ activeLutLabel }}</div>
                  <div class="flex gap-1 min-w-0">
                    <button type="button" class="t-button-default text-[10px] flex-1 min-w-0" @click="showLutLibraryDialog = true">{{ $t('photo_style.lut_pick') }}</button>
                    <button type="button" class="t-button-default text-[10px] shrink-0" :disabled="!styleLutId" @click="clearStyleLut">{{ $t('photo_style.lut_clear') }}</button>
                  </div>
                </div>
              </div>
              <div class="grid grid-cols-[80px_minmax(0,1fr)] gap-x-4 items-center">
                <div class="font-medium text-base-content/70 tracking-wide text-xs">{{ $t('photo_style.lut_intensity') }}</div>
                <div class="flex items-center gap-2 pr-2 min-w-0">
                  <SliderInput v-model="styleLutIntensity" :min="0" :max="100" :step="1" class="flex-1 min-w-0 w-full" />
                  <span class="text-[10px] font-mono text-base-content/70 w-8 text-right shrink-0">{{ styleLutIntensity }}</span>
                </div>
              </div>
              <div v-if="photoStyleError" class="text-[11px] text-error">{{ photoStyleError }}</div>
              <div v-if="photoStyleBusy" class="text-[11px] text-base-content/50 flex items-center gap-2">
                <span class="loading loading-spinner loading-xs"></span>
                {{ $t('photo_style.computing') }}
              </div>
              <div class="flex gap-1">
                <button
                  v-if="canDeleteActiveCustomStyle"
                  type="button"
                  class="t-button-default text-[10px] text-error"
                  @click="deleteActiveCustomStyle"
                >{{ $t('photo_style.delete') }}</button>
                <button type="button" class="t-button-default text-[10px]" @click="duplicateActiveStyle">{{ $t('photo_style.copy') }}</button>
              </div>
            </div>
          </div>
        </section>
        </template>
      </div>
    </div>

    <!-- Bottom Bar -->
    <div v-if="fileInfo" class="h-14 shrink-0 flex items-center justify-end px-4 gap-2">
      <button
        class="px-4 py-1 rounded-box hover:bg-base-100 hover:text-base-content cursor-pointer text-sm mr-4"
        @click="clickCancel"
      >{{ $t('msgbox.image_editor.cancel') }}</button>

      <template v-if="effectiveSaveAsNew">
        <select v-model="combinedFormatKey" class="select select-bordered select-xs" :disabled="cropStatus===1">
          <option v-for="option in combinedFormatOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
        </select>
      </template>

        <div class="join">
          <button
            class="btn btn-sm btn-primary join-item px-4"
            :disabled="cropStatus === 1 || isProcessing"
            @click="clickSave"
          >{{ effectiveSaveAsNew ? $t('msgbox.image_editor.save_as_new') : $t('msgbox.image_editor.overwrite') }}</button>
          <div class="dropdown dropdown-top dropdown-end">
            <button
              tabindex="0"
              class="btn btn-sm btn-primary join-item border-l border-primary-content/20 px-1.5"
            :disabled="!canOverwriteOriginal || cropStatus === 1"
            >
              <IconArrowDown class="w-3 h-3" />
            </button>
            <ul tabindex="0" class="dropdown-content menu bg-base-100 rounded-box shadow-lg mb-1 p-1 text-sm w-32">
              <li>
                <a :class="config.imageEditor.saveAs === 0 ? 'active' : ''"
                   @click="config.imageEditor.saveAs = 0; closeSaveDropdown()">
                  {{ $t('msgbox.image_editor.overwrite') }}
                </a>
              </li>
              <li>
                <a :class="config.imageEditor.saveAs === 1 ? 'active' : ''"
                   @click="config.imageEditor.saveAs = 1; closeSaveDropdown()">
                  {{ $t('msgbox.image_editor.save_as_new') }}
                </a>
              </li>
            </ul>
          </div>
        </div>
    </div>
  </div>

  <MessageBox v-if="showOverwriteConfirm"
    :title="$t('msgbox.image_editor.overwrite')"
    :message="$t('msgbox.image_editor.overwrite_confirm')"
    :warningOk="true"
    :OkText="$t('msgbox.ok')"
    :cancelText="$t('msgbox.cancel')"
    @ok="handleOverwriteConfirm"
    @cancel="handleOverwriteCancel"
  />

  <PhotoSizeManageDialog
    v-if="showPhotoSizeManageDialog"
    :custom-ratios="customCropRatios"
    @cancel="showPhotoSizeManageDialog = false"
    @update:custom-ratios="onCustomCropRatiosUpdated"
  />

  <AddCustomCropRatioDialog
    v-if="showAddCustomRatioDialog"
    :existing="customCropRatios"
    @cancel="showAddCustomRatioDialog = false"
    @ok="onAddCustomCropRatio"
  />

  <LutLibraryDialog
    v-if="showLutLibraryDialog"
    :initial-selected-id="styleLutId"
    @cancel="showLutLibraryDialog = false"
    @select="onLutLibrarySelect"
  />

  <MessageBox
    v-if="showSaveStyleBox"
    :title="$t('photo_style.save_as_title')"
    :message="$t('photo_style.save_as_prompt')"
    :showInput="true"
    :inputText="saveStyleName"
    :inputPlaceholder="$t('photo_style.save_as_prompt')"
    :OkText="$t('msgbox.ok')"
    :cancelText="$t('msgbox.cancel')"
    @ok="onSaveStyleOk"
    @cancel="showSaveStyleBox = false"
  />

</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch, type CSSProperties } from 'vue';
import { useRouter } from 'vue-router';
import { useUIStore } from '@/stores/uiStore';
import { useI18n } from 'vue-i18n';
import { config } from '@/common/config';
import { isWin, isLinux, setTheme, SCALE_VALUES, getFolderPath, getFileExtension, shortenFilename, getFullPath, combineFileName, getSelectOptions, getAssetSrc, getPreviewUrl, getThumbUrl, shouldUseBackendPreview } from '@/common/utils';
import { editImage, colorMatchPreview, exportColorMatchLut, applyPhotoStylePreview, listLutLibrary, checkFileExists, getFileInfo } from '@/common/api';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit as tauriEmit, listen } from '@tauri-apps/api/event';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

import TitleBar from '@/components/TitleBar.vue';
import MessageBox from '@/components/MessageBox.vue';
import TButton from '@/components/TButton.vue';
import SliderInput from '@/components/SliderInput.vue';
import ImageHistogram from '@/components/ImageHistogram.vue';
import PhotoSizeManageDialog from '@/components/PhotoSizeManageDialog.vue';
import AddCustomCropRatioDialog from '@/components/AddCustomCropRatioDialog.vue';
import LutLibraryDialog from '@/components/LutLibraryDialog.vue';
import {
  allPhotoStyles,
  cloneAsCustom,
  defaultPhotoStyle,
  findPhotoStyle,
  getBuiltinRecipe,
  isPhotoStyleIdentity,
  needsHostPreview,
  normalizePhotoStyles,
  sameRecipeValues,
  styleForHost,
  type AdjustRecipe,
  type PhotoStylePreset,
} from '@/common/photoStylePresets';
import {
  ADD_CUSTOM_RATIO_ID,
  BUILTIN_PHOTO_SIZE_PRESETS,
  BUILTIN_RATIO_PRESETS,
  FREE_CROP_PRESET_ID,
  MANAGE_PHOTO_SIZES_ID,
  formatRatioLabel,
  getCropAspectRatio,
  getPhotoTargetPixels,
  getPresetBaseRatio,
  migrateLegacyCropShape,
  normalizeCustomCropRatios,
  resolveCropPreset,
  type CustomCropRatio,
  type ResolvedCropPreset,
} from '@/common/photoSizePresets';

import {
  IconCrop,
  IconCropLandscape,
  IconZoomIn,
  IconZoomOut,
  IconRotateLeft,
  IconRotateRight,
  IconFlipVertical,
  IconFlipHorizontal,
  IconClose,
  IconOk,
  IconRestore,
  IconLink,
  IconLinkOff,
  IconArrowDown,
  IconPalette,
  IconSplitOn,
} from '@/common/icons';

const router = useRouter();
const fileInfo = ref<any>(null);
const initialImageSrc = ref('');

const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[locale.value] as any);

const uiStore = useUIStore();
const appWindow = getCurrentWebviewWindow();
const showDesktopTitleBar = isWin || isLinux;

function sendToParent(payload: Record<string, any>) {
  void tauriEmit('message-from-image-editor', payload).catch((error) => {
    console.error('Failed to notify parent from image editor:', error);
  });
}

async function closeEditorWindow() {
  try {
    await appWindow.close();
  } catch (error) {
    try {
      await appWindow.destroy();
    } catch (destroyError) {
      console.error('Failed to close image editor window:', error, destroyError);
    }
    return;
  }

  await new Promise(resolve => window.setTimeout(resolve, 100));
  try {
    if (await appWindow.isVisible()) {
      await appWindow.destroy();
    }
  } catch {
    // The window is already closed.
  }
}

async function loadFileInfo(fileId: number) {
  try {
    imageReady.value = false;
    const file = await getFileInfo(fileId);
    if (file) {
      file.thumbnail = getThumbUrl(file.id);
      fileInfo.value = file;
      newFileName.value = file.name?.substring(0, file.name.lastIndexOf('.')) || file.name || '';
      const src = getPreviewUrl(file);
      initialImageSrc.value = typeof src === 'string' ? src : '';
    }
  } catch {
    await closeEditorWindow();
  }
}

const isProcessing = ref(false);
const imageReady = ref(false);
const activeEditorTab = ref<'edit' | 'adjust'>('edit');

const containerRef = ref<HTMLElement | null>(null);
const containerRect = ref<DOMRect | null>(null);
const containerBounds = ref({ top: 0, left: 0, width: 0, height: 0 });
const containerPadding = 5;

const imageRef = ref<HTMLImageElement | null>(null);
const imageRect = ref<DOMRect | null>(null);
const imageRectOriginal = ref<DOMRect | null>(null);
const imageSrc = ref('');
const imageWidth = ref(0);
const imageHeight = ref(0);
const isRawFile = computed(() => Number(fileInfo.value?.file_type || 0) === 3);
const normalizeRotate = (value: number) => {
  const normalized = Number(value || 0) % 360;
  return normalized < 0 ? normalized + 360 : normalized;
};
const initialDisplayRotate = computed(() => normalizeRotate(Number(fileInfo.value?.rotate || 0)));
const isPortraitForRotation = (width: number, height: number, rotation: number) => {
  const normalized = normalizeRotate(rotation);
  return normalized % 180 !== 0 ? width > height : height > width;
};
const usesBackendPreview = computed(() =>
  shouldUseBackendPreview(
    fileInfo.value?.name || fileInfo.value?.file_path || '',
    Number(fileInfo.value?.file_type || 0)
  )
);

const enableTransition = ref(false);
const position = ref({ left: 0, top: 0 });
const isFlippedX = ref(false);
const isFlippedY = ref(false);
const scale = ref(1);
const rotate = ref(0);
const showDiffPreview = ref(false);
const showOriginalWhilePressed = ref(false);
const brightness = ref(0);
const contrast = ref(0);
const saturation = ref(100);
const hue = ref(0);
const blur = ref(0);
const selectedFilter = ref('');
const selectedPreset = ref('natural');
const autoPresetValues = ref<AdjustmentValues | null>(null);
let isApplyingPreset = false;
let skipNextCustomPresetLoad = false;
let autoPresetRequestId = 0;

// Traditional global Lab color match (追色)
const colorMatchReferencePath = ref('');
const colorMatchIntensity = ref(100);
const colorMatchTone = ref(50);
const colorMatchHighlight = ref(80);
const colorMatchShadow = ref(80);
const colorMatchAutoWb = ref(true);
const colorMatchBusy = ref(false);
const colorMatchExporting = ref(false);
const colorMatchError = ref('');
const colorMatchPreviewUrl = ref('');
let colorMatchPreviewRequestId = 0;
let colorMatchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

// Unified adjust recipes (presets + manual + host effects/LUT)
const showLutLibraryDialog = ref(false);
const showSaveStyleBox = ref(false);
const saveStyleName = ref('');
const photoStyleBusy = ref(false);
const photoStyleError = ref('');
const photoStylePreviewUrl = ref('');
let photoStylePreviewRequestId = 0;
let lastHostPreviewFingerprint = '';
let lastHostPreviewBytes: Uint8Array | null = null;
let photoStyleDebounceTimer: ReturnType<typeof setTimeout> | null = null;
const lutLibraryCache = ref<any[]>([]);

// Host-only fields (CSS base uses brightness/contrast/saturation/hue/blur/selectedFilter)
const styleHighlights = ref(0);
const styleShadows = ref(0);
const styleFade = ref(0);
const styleVignette = ref(0);
const styleGrain = ref(0);
const styleLutId = ref('');
const styleLutIntensity = ref(100);

function ensurePhotoStyleConfig() {
  const editor = config.imageEditor as any;
  if (!Array.isArray(editor.photoStyles)) editor.photoStyles = [];
  else editor.photoStyles = normalizePhotoStyles(editor.photoStyles);
  // Migrate legacy activePhotoStyleId -> selected preset if still present
  if (!editor.activePhotoStyleId) editor.activePhotoStyleId = 'natural';
  // Normalize old builtin style ids
  const legacyMap: Record<string, string> = {
    'builtin-standard': 'natural',
    'builtin-vivid': 'vivid',
    'builtin-natural': 'muted',
    'builtin-portrait': 'portrait',
    'builtin-landscape': 'landscape',
    'builtin-mono': 'bw',
    'builtin-nostalgic': 'nostalgic',
    'builtin-cinematic': 'cinematic',
  };
  const active = String(editor.activePhotoStyleId || '');
  if (legacyMap[active]) editor.activePhotoStyleId = legacyMap[active];
}
ensurePhotoStyleConfig();

const customPhotoStyles = computed<AdjustRecipe[]>(() => {
  return normalizePhotoStyles((config.imageEditor as any).photoStyles);
});

const activeLutLabel = computed(() => {
  if (!styleLutId.value) return (localeMsg.value as any).photo_style?.lut_none || 'No LUT';
  const hit = lutLibraryCache.value.find((x: any) => x.id === styleLutId.value);
  return hit?.name || styleLutId.value;
});

function currentPreviewGeometry() {
  const hasCrop = cropApplied.value && crop.value.width > 0 && crop.value.height > 0;
  return {
    rotate: rotate.value || 0,
    flipHorizontal: !!isFlippedX.value,
    flipVertical: !!isFlippedY.value,
    fullWidth: imageWidth.value || 0,
    fullHeight: imageHeight.value || 0,
    crop: hasCrop
      ? {
          x: crop.value.left,
          y: crop.value.top,
          width: crop.value.width,
          height: crop.value.height,
        }
      : null,
  };
}

function currentWorkingStyle(): AdjustRecipe {
  const id = selectedPreset.value;
  const named = findPhotoStyle(id, customPhotoStyles.value);
  return defaultPhotoStyle({
    id: named?.id || (id.startsWith('custom-') ? id : 'custom'),
    name: named?.name || (localeMsg.value.msgbox.image_editor.presets.custom as string) || 'Custom',
    builtIn: named?.builtIn ?? false,
    brightness: brightness.value,
    contrast: contrast.value,
    saturation: saturation.value,
    hue: hue.value,
    blur: blur.value,
    filter: (selectedFilter.value as any) || '',
    highlights: styleHighlights.value,
    shadows: styleShadows.value,
    fade: styleFade.value,
    vignette: styleVignette.value,
    grain: styleGrain.value,
    lutId: styleLutId.value,
    lutIntensity: styleLutIntensity.value,
  });
}

function applyRecipeValues(recipe: AdjustRecipe | PhotoStylePreset) {
  brightness.value = recipe.brightness;
  contrast.value = recipe.contrast;
  saturation.value = recipe.saturation;
  hue.value = recipe.hue;
  blur.value = recipe.blur ?? 0;
  selectedFilter.value = recipe.filter || '';
  styleHighlights.value = recipe.highlights ?? 0;
  styleShadows.value = recipe.shadows ?? 0;
  styleFade.value = recipe.fade ?? 0;
  styleVignette.value = recipe.vignette ?? 0;
  styleGrain.value = recipe.grain ?? 0;
  styleLutId.value = recipe.lutId || '';
  styleLutIntensity.value = recipe.lutIntensity ?? 100;
}

function persistCustomStyles(list: AdjustRecipe[]) {
  (config.imageEditor as any).photoStyles = normalizePhotoStyles(list);
}

function saveCurrentAsCustom() {
  const cur = currentWorkingStyle();
  saveStyleName.value = `${cur.name} custom`;
  showSaveStyleBox.value = true;
}

function onSaveStyleOk(name: string) {
  showSaveStyleBox.value = false;
  const n = String(name || '').trim() || 'Custom';
  const style = currentWorkingStyle();
  const custom = cloneAsCustom(style, n);
  persistCustomStyles([custom, ...customPhotoStyles.value]);
  skipNextCustomPresetLoad = true;
  selectedPreset.value = custom.id;
  (config.imageEditor as any).activePhotoStyleId = custom.id;
}

function duplicateActiveStyle() {
  const custom = cloneAsCustom(currentWorkingStyle());
  persistCustomStyles([custom, ...customPhotoStyles.value]);
  skipNextCustomPresetLoad = true;
  selectedPreset.value = custom.id;
  (config.imageEditor as any).activePhotoStyleId = custom.id;
}

const canDeleteActiveCustomStyle = computed(() => {
  const id = selectedPreset.value;
  if (!id || id === 'auto' || id === 'custom' || id === 'natural') return false;
  const hit = customPhotoStyles.value.find((s) => s.id === id);
  return !!hit && !hit.builtIn;
});

function deleteActiveCustomStyle() {
  const id = selectedPreset.value;
  if (!canDeleteActiveCustomStyle.value) return;
  persistCustomStyles(customPhotoStyles.value.filter((s) => s.id !== id));
  selectedPreset.value = 'natural';
  (config.imageEditor as any).activePhotoStyleId = 'natural';
}

function clearStyleLut() {
  styleLutId.value = '';
  schedulePhotoStylePreview();
}

function onLutLibrarySelect(id: string) {
  showLutLibraryDialog.value = false;
  styleLutId.value = id;
  schedulePhotoStylePreview();
  void refreshLutCache();
}

async function refreshLutCache() {
  try {
    lutLibraryCache.value = (await listLutLibrary()) || [];
  } catch {
    /* ignore */
  }
}

function invalidateHostPreviewClientCache() {
  lastHostPreviewFingerprint = '';
  lastHostPreviewBytes = null;
}

function revokePhotoStylePreviewUrl() {
  if (photoStylePreviewUrl.value) {
    try { URL.revokeObjectURL(photoStylePreviewUrl.value); } catch { /* ignore */ }
    photoStylePreviewUrl.value = '';
  }
}

let namedCustomPersistTimer: ReturnType<typeof setTimeout> | null = null;

function flushPersistNamedCustom() {
  if (namedCustomPersistTimer) {
    clearTimeout(namedCustomPersistTimer);
    namedCustomPersistTimer = null;
  }
  try { flushPersistNamedCustom(); } catch { /* ignore */ }
  const style = currentWorkingStyle();
  if (style.builtIn) return;
  if (!customPhotoStyles.value.some((s) => s.id === style.id)) return;
  const next = customPhotoStyles.value.map((s) => (
    s.id === style.id
      ? { ...style, updatedAt: s.updatedAt || style.updatedAt }
      : s
  ));
  persistCustomStyles(next);
}

function schedulePersistNamedCustom() {
  if (namedCustomPersistTimer) {
    clearTimeout(namedCustomPersistTimer);
    namedCustomPersistTimer = null;
  }
  namedCustomPersistTimer = setTimeout(() => {
    flushPersistNamedCustom();
  }, 400);
}

/** Interactive host preview long-edge: scale with viewport, keep work small on narrow sidebars. */
const previewMaxEdge = computed(() => {
  const w = Number(containerBounds.value?.width || 0);
  const h = Number(containerBounds.value?.height || 0);
  const edge = Math.max(w, h);
  if (edge <= 0) return 1000;
  // Decode/work budget for slider ticks; clamped for quality vs cost.
  return Math.round(Math.min(1400, Math.max(720, edge * 1.15)));
});

function schedulePhotoStylePreview() {
  if (photoStyleDebounceTimer) {
    clearTimeout(photoStyleDebounceTimer);
    photoStyleDebounceTimer = null;
  }
  if (namedCustomPersistTimer) {
    clearTimeout(namedCustomPersistTimer);
    namedCustomPersistTimer = null;
  }
  photoStyleDebounceTimer = setTimeout(() => { void runPhotoStylePreview(); }, 280);
}

async function runPhotoStylePreview() {
  // Shared host adjust preview: color-match (optional) then photo-style (optional), matching edit_image order.
  if (!fileInfo.value?.file_path) return;
  const style = currentWorkingStyle();
  const hasMatch = !!colorMatchReferencePath.value;
  const styleNeedsHost = needsHostPreview(style) && !isPhotoStyleIdentity(style);

  if (!hasMatch && !styleNeedsHost) {
    revokePhotoStylePreviewUrl();
    // keep colorMatchPreviewUrl managed by color-match path when only match params change without host style
    photoStyleBusy.value = false;
    photoStyleError.value = '';
    return;
  }

  // When only CSS style (no host fields) but color match is on, color-match runner owns the canvas.
  if (hasMatch && !styleNeedsHost) {
    revokePhotoStylePreviewUrl();
    photoStyleBusy.value = false;
    photoStyleError.value = '';
    // Ensure match preview is fresh without style bake.
    scheduleColorMatchPreview();
    return;
  }

  const requestId = ++photoStylePreviewRequestId;
  photoStyleBusy.value = true;
  photoStyleError.value = '';
  try {
    const hostStyle = styleForHost(style);
    const geometry = currentPreviewGeometry();
    const fingerprint = JSON.stringify({
      path: fileInfo.value.file_path,
      maxEdge: previewMaxEdge.value,
      hasMatch,
      ref: colorMatchReferencePath.value || '',
      intensity: colorMatchIntensity.value,
      tone: colorMatchTone.value,
      autoWb: colorMatchAutoWb.value,
      hi: colorMatchHighlight.value,
      sh: colorMatchShadow.value,
      style: hostStyle,
      geometry,
    });
    let bytes: Uint8Array | ArrayBuffer;
    if (lastHostPreviewFingerprint === fingerprint && lastHostPreviewBytes) {
      bytes = lastHostPreviewBytes;
    } else if (hasMatch) {
      // Combined: match then style (same as save).
      colorMatchBusy.value = true;
      bytes = await colorMatchPreview({
        sourceFilePath: fileInfo.value.file_path,
        referenceFilePath: colorMatchReferencePath.value,
        orientation: fileInfo.value.e_orientation || 1,
        maxEdge: previewMaxEdge.value,
        intensity: colorMatchIntensity.value / 100,
        tonePreservation: colorMatchTone.value / 100,
        autoWb: colorMatchAutoWb.value,
        highlightProtection: colorMatchHighlight.value / 100,
        shadowProtection: colorMatchShadow.value / 100,
        photoStyle: hostStyle,
        geometry,
      });
    } else {
      bytes = await applyPhotoStylePreview({
        sourceFilePath: fileInfo.value.file_path,
        maxEdge: previewMaxEdge.value,
        style: hostStyle,
        geometry,
      });
    }
    if (requestId !== photoStylePreviewRequestId) return;
    const arr = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes as ArrayBuffer);
    lastHostPreviewFingerprint = fingerprint;
    lastHostPreviewBytes = arr;
    const url = URL.createObjectURL(new Blob([arr], { type: 'image/jpeg' }));
    if (hasMatch) {
      revokeColorMatchPreviewUrl();
      colorMatchPreviewUrl.value = url;
      revokePhotoStylePreviewUrl();
    } else {
      revokePhotoStylePreviewUrl();
      photoStylePreviewUrl.value = url;
    }
  } catch (error: any) {
    if (requestId !== photoStylePreviewRequestId) return;
    const msg = String(error?.message || error || (localeMsg.value as any).photo_style?.failed || 'failed');
    photoStyleError.value = msg;
    if (hasMatch) {
      colorMatchError.value = msg;
      revokeColorMatchPreviewUrl();
    }
    revokePhotoStylePreviewUrl();
  } finally {
    if (requestId === photoStylePreviewRequestId) {
      photoStyleBusy.value = false;
      if (hasMatch) colorMatchBusy.value = false;
    }
  }
}

const hasColorMatch = computed(() => !!colorMatchReferencePath.value);
const colorMatchReferenceName = computed(() => {
  const p = colorMatchReferencePath.value;
  if (!p) return '';
  const parts = p.replace(/\\/g, '/').split('/');
  return parts[parts.length - 1] || p;
});
const baseImageSrc = computed(() => imageSrc.value);
/** Host-baked raster currently shown (color-match and/or photo-style). */
const hostPreviewUrl = computed(() => colorMatchPreviewUrl.value || photoStylePreviewUrl.value || '');
/** True when host JPEG already includes base color/style fields (save path uses photoStyle). */
const hostBakesBaseColor = computed(() => {
  if (showOriginalWhilePressed.value) return false;
  if (photoStylePreviewUrl.value) return true;
  // Combined match+style is stored on colorMatchPreviewUrl (style blob cleared).
  if (colorMatchPreviewUrl.value) {
    const style = currentWorkingStyle();
    return needsHostPreview(style) && !isPhotoStyleIdentity(style);
  }
  return false;
});
/** Any host preview blob is on screen (pure match and/or style). */
const usesHostPreviewRaster = computed(() => {
  if (showOriginalWhilePressed.value) return false;
  return !!hostPreviewUrl.value;
});
const displayImageSrc = computed(() => {
  if (showOriginalWhilePressed.value) return imageSrc.value;
  if (hostPreviewUrl.value) return hostPreviewUrl.value;
  return imageSrc.value;
});

let containerResizeObserver: ResizeObserver | null = null;
let unlistenUpdateFile: (() => void) | null = null;
const isResizing = ref(false);
type AdjustmentValues = {
  brightness: number;
  contrast: number;
  saturation: number;
  hue: number;
  blur: number;
  filter: string;
  highlights?: number;
  shadows?: number;
  fade?: number;
  vignette?: number;
  grain?: number;
  lutId?: string;
  lutIntensity?: number;
};

const histogramRef = ref<InstanceType<typeof ImageHistogram> | null>(null);
// Prefer host preview raster when present so histogram matches on-screen grade (match/style bake).
const histogramUsesHostPreview = computed(() => usesHostPreviewRaster.value);
const histogramSource = computed(() => {
  if (histogramUsesHostPreview.value) {
    return hostPreviewUrl.value || imageSrc.value || fileInfo.value?.thumbnail || '';
  }
  return imageSrc.value || fileInfo.value?.thumbnail || '';
});
/** Map editor crop (imageWidth/Height space) into host-preview raster space (long-edge maxEdge). */
function mapCropToHostPreview(
  cropRect: { left: number; top: number; width: number; height: number },
  srcW: number,
  srcH: number,
  maxEdge: number,
) {
  const sw = Math.max(1, srcW);
  const sh = Math.max(1, srcH);
  const longEdge = Math.max(sw, sh);
  const scale = longEdge > maxEdge ? maxEdge / longEdge : 1;
  const x = Math.max(0, Math.round(cropRect.left * scale));
  const y = Math.max(0, Math.round(cropRect.top * scale));
  const width = Math.max(1, Math.round(cropRect.width * scale));
  const height = Math.max(1, Math.round(cropRect.height * scale));
  const outW = Math.max(1, Math.round(sw * scale));
  const outH = Math.max(1, Math.round(sh * scale));
  return {
    x: Math.min(x, Math.max(0, outW - 1)),
    y: Math.min(y, Math.max(0, outH - 1)),
    width: Math.min(width, Math.max(1, outW - x)),
    height: Math.min(height, Math.max(1, outH - y)),
  };
}

const histogramCrop = computed(() => {
  if (!(cropApplied.value && crop.value.width > 0 && crop.value.height > 0)) return null;
  // Host color preview now applies crop before grade; the raster is already the crop region.
  if (histogramUsesHostPreview.value) return null;
  return {
    x: crop.value.left,
    y: crop.value.top,
    width: crop.value.width,
    height: crop.value.height,
  };
});
// Host preview already bakes base color/style; only stack CSS blur (host photoStyle never bakes blur).
const histogramApplyAdjustments = computed(() => {
  // Pure match raster still needs CSS refine; style-baked raster only stacks blur.
  if (hostBakesBaseColor.value) return blur.value > 0;
  return true;
});
const histogramAdjustments = computed<AdjustmentValues>(() => {
  if (hostBakesBaseColor.value) {
    return {
      brightness: 0,
      contrast: 0,
      saturation: 100,
      hue: 0,
      blur: blur.value,
      filter: '',
    };
  }
  return {
    brightness: brightness.value,
    contrast: contrast.value,
    saturation: saturation.value,
    hue: hue.value,
    blur: blur.value,
    filter: selectedFilter.value,
  };
});


function buildAdjustmentFilter(values: AdjustmentValues) {
  return `
    brightness(${100 + values.brightness}%)
    contrast(${100 + values.contrast}%)
    blur(${values.blur}px)
    hue-rotate(${values.hue}deg)
    saturate(${values.saturation}%)
    ${values.filter === 'grayscale' ? 'grayscale(100%)' : ''}
    ${values.filter === 'sepia' ? 'sepia(100%)' : ''}
    ${values.filter === 'invert' ? 'invert(100%)' : ''}
  `;
}

const imageStyle = computed((): CSSProperties => {
  const hostRaster = usesHostPreviewRaster.value;
  // Host color preview already applied flip/rotate/crop; only pan/zoom remain in CSS.
  const rot = hostRaster ? 0 : rotate.value;
  const fx = hostRaster ? false : isFlippedX.value;
  const fy = hostRaster ? false : isFlippedY.value;
  const hasCrop = cropApplied.value && crop.value.width > 0 && crop.value.height > 0;
  // When host baked crop, display box should match crop region (save output aspect).
  const dispW = hostRaster && hasCrop ? crop.value.width : imageWidth.value;
  const dispH = hostRaster && hasCrop ? crop.value.height : imageHeight.value;
  return {
    display: 'block',
    width: `${dispW}px`,
    height: `${dispH}px`,
    maxWidth: 'none',
    maxHeight: 'none',
    position: 'absolute',
    filter: showOriginalWhilePressed.value
      ? 'none'
      : (hostBakesBaseColor.value
        ? (blur.value > 0 ? `blur(${blur.value}px)` : 'none')
        : adjustmentFilter.value),
    transform: `
    translate(${position.value.left}px, ${position.value.top}px)
    rotate(${rot}deg)
    scaleX(${fx ? -1 : 1})
    scaleY(${fy ? -1 : 1})
    scale(${scale.value})
  `,
    transition: enableTransition.value ? 'transform 0.3s ease' : 'none',
    backfaceVisibility: 'hidden',
    willChange: 'transform, filter',
  };
});
/** Shared compare window (crop region when applied, else full frame). */
const compareHasCrop = computed(
  () => cropApplied.value && crop.value.width > 0 && crop.value.height > 0,
);
const compareWindowStyle = computed((): CSSProperties => {
  // Fill the diff pane; inner content is positioned in image pixel space then scaled by CSS transform on img.
  return {
    width: '100%',
    height: '100%',
  };
});

// Diff "before": full base image, offset so the crop window shows the same region as "after".
const originalImageStyle = computed((): CSSProperties => {
  const hasCrop = compareHasCrop.value;
  const hostRaster = usesHostPreviewRaster.value;
  // When comparing host bake that already includes flip/rotate, base still needs CSS geometry.
  const rot = rotate.value;
  const fx = isFlippedX.value;
  const fy = isFlippedY.value;
  // Map editor pan/zoom into the compare window. Use crop size as the layout unit when cropped.
  const cropX = hasCrop ? crop.value.left : 0;
  const cropY = hasCrop ? crop.value.top : 0;
  return {
    display: 'block',
    width: `${imageWidth.value}px`,
    height: `${imageHeight.value}px`,
    maxWidth: 'none',
    maxHeight: 'none',
    position: 'absolute',
    left: '50%',
    top: '50%',
    filter: 'none',
    // Center the layout window, then shift by -crop so the crop rect fills the window, then pan/zoom/rot.
    transform: `
      translate(-50%, -50%)
      translate(${position.value.left}px, ${position.value.top}px)
      scale(${scale.value})
      rotate(${rot}deg)
      scaleX(${fx ? -1 : 1})
      scaleY(${fy ? -1 : 1})
      translate(${-cropX}px, ${-cropY}px)
    `,
    transformOrigin: 'top left',
    transition: enableTransition.value ? 'transform 0.3s ease' : 'none',
    willChange: 'transform',
  };
});

// Diff "after": host bake already includes geometry when host preview is on; otherwise CSS geometry like main view.
const adjustedImageStyle = computed((): CSSProperties => {
  const hasCrop = compareHasCrop.value;
  const hostRaster = usesHostPreviewRaster.value;
  if (hostRaster) {
    // Host JPEG is already flip/rotate/crop baked; size to crop (or full) and only pan/zoom.
    const w = hasCrop ? crop.value.width : imageWidth.value;
    const h = hasCrop ? crop.value.height : imageHeight.value;
    return {
      display: 'block',
      width: `${w}px`,
      height: `${h}px`,
      maxWidth: 'none',
      maxHeight: 'none',
      position: 'absolute',
      left: '50%',
      top: '50%',
      filter: hostBakesBaseColor.value
        ? (blur.value > 0 ? `blur(${blur.value}px)` : 'none')
        : adjustmentFilter.value,
      transform: `
        translate(-50%, -50%)
        translate(${position.value.left}px, ${position.value.top}px)
        scale(${scale.value})
      `,
      transition: enableTransition.value ? 'transform 0.3s ease' : 'none',
      willChange: 'transform, filter',
    };
  }
  // CSS-only after: same crop window trick as original, with adjustment filter.
  const cropX = hasCrop ? crop.value.left : 0;
  const cropY = hasCrop ? crop.value.top : 0;
  return {
    display: 'block',
    width: `${imageWidth.value}px`,
    height: `${imageHeight.value}px`,
    maxWidth: 'none',
    maxHeight: 'none',
    position: 'absolute',
    left: '50%',
    top: '50%',
    filter: adjustmentFilter.value,
    transform: `
      translate(-50%, -50%)
      translate(${position.value.left}px, ${position.value.top}px)
      scale(${scale.value})
      rotate(${rotate.value}deg)
      scaleX(${isFlippedX.value ? -1 : 1})
      scaleY(${isFlippedY.value ? -1 : 1})
      translate(${-cropX}px, ${-cropY}px)
    `,
    transformOrigin: 'top left',
    transition: enableTransition.value ? 'transform 0.3s ease' : 'none',
    willChange: 'transform, filter',
  };
});
const canShowDiffPreview = computed(() => activeEditorTab.value === 'adjust' && hasAdjustmentChanges.value);
const currentPresetLabel = computed(() => {
  if (showOriginalWhilePressed.value) {
    return presetOptions.value.find(o => o.value === 'natural')?.label || '';
  }
  if (usesHostPreviewRaster.value) {
    const pe = localeMsg.value.msgbox.image_editor;
    if (colorMatchPreviewUrl.value && photoStylePreviewUrl.value) {
      // combined path stores only on colorMatch url; still show generic adjusted
      return pe.adjusted || '';
    }
    if (colorMatchPreviewUrl.value) {
      // may be pure match or match+style combined (style blob cleared)
      const style = currentWorkingStyle();
      if (needsHostPreview(style) && !isPhotoStyleIdentity(style)) {
        return pe.compare_host_grade || pe.adjusted || '';
      }
      return pe.color_match || pe.adjusted || '';
    }
    return pe.compare_host_grade || pe.adjusted || '';
  }
  const key = resolvePresetKey(getCurrentAdjustmentValues());
  return presetOptions.value.find(o => o.value === key)?.label || '';
});
const adjustmentFilter = computed(() => {
  return buildAdjustmentFilter({
    brightness: brightness.value,
    contrast: contrast.value,
    saturation: saturation.value,
    hue: hue.value,
    blur: blur.value,
    filter: selectedFilter.value,
  });
});

const cropStatus = ref(0);
const cropApplied = ref(false);
const isPortrait = ref(false);
const cropBoxFixed = ref(false);

const cropBox = ref({ left: 0, top: 0, width: 0, height: 0 });
const crop = ref({ left: 0, top: 0, width: 0, height: 0 });

const isDragging = ref(false);
const dragHandle = ref('');
const dragStartX = ref(0);
const dragStartY = ref(0);

const cropBoxStyle = computed(() => ({
  top: `${cropBox.value.top}px`,
  left: `${cropBox.value.left}px`,
  width: `${cropBox.value.width}px`,
  height: `${cropBox.value.height}px`,
}));

const baseOutputWidth = computed(() => {
  if ((cropStatus.value === 1 || cropApplied.value) && crop.value.width > 0) {
    return crop.value.width;
  }
  return rotate.value % 180 !== 0 ? imageHeight.value : imageWidth.value;
});
const baseOutputHeight = computed(() => {
  if ((cropStatus.value === 1 || cropApplied.value) && crop.value.height > 0) {
    return crop.value.height;
  }
  return rotate.value % 180 !== 0 ? imageWidth.value : imageHeight.value;
});
const resizeWidthInput = ref('');
const resizeHeightInput = ref('');
const keepAspectRatio = ref(true);
const resizeAspectRatio = computed(() => {
  if (!baseOutputWidth.value || !baseOutputHeight.value) return 1;
  return baseOutputWidth.value / baseOutputHeight.value;
});
const parsedResizeWidth = computed(() => {
  const width = Number.parseInt(resizeWidthInput.value, 10);
  return Number.isFinite(width) && width > 0 ? width : null;
});
const parsedResizeHeight = computed(() => {
  const height = Number.parseInt(resizeHeightInput.value, 10);
  return Number.isFinite(height) && height > 0 ? height : null;
});
const maxResizeWidth = computed(() => Math.max(1, baseOutputWidth.value));
const maxResizeHeight = computed(() => Math.max(1, baseOutputHeight.value));
const hasResizeChanges = computed(() => {
  return (
    resizeWidthInput.value !== String(baseOutputWidth.value) ||
    resizeHeightInput.value !== String(baseOutputHeight.value) ||
    !keepAspectRatio.value
  );
});
const resizeOutput = computed(() => {
  const widthInput = parsedResizeWidth.value;
  const heightInput = parsedResizeHeight.value;
  const baseWidth = baseOutputWidth.value;
  const baseHeight = baseOutputHeight.value;
  const ratio = resizeAspectRatio.value || 1;

  if (!widthInput && !heightInput) {
    return { width: baseWidth, height: baseHeight, hasResize: false };
  }

  const buildResizeResult = (width: number, height: number) => ({
    width,
    height,
    hasResize: width !== baseWidth || height !== baseHeight,
  });

  if (keepAspectRatio.value) {
    if (widthInput && heightInput) {
      return buildResizeResult(widthInput, heightInput);
    }
    if (widthInput) {
      return buildResizeResult(widthInput, Math.max(1, Math.round(widthInput / ratio)));
    }
    if (heightInput) {
      return buildResizeResult(Math.max(1, Math.round(heightInput * ratio)), heightInput);
    }
  }

  return buildResizeResult(widthInput || baseWidth, heightInput || baseHeight);
});
const hasEditImageChanges = computed(() =>
  normalizeRotate(rotate.value) !== initialDisplayRotate.value ||
  isFlippedX.value ||
  isFlippedY.value
);
const presets: Record<string, AdjustmentValues> = {
  // Keep natural as identity for auto/reset fallbacks; full recipes live in photoStylePresets.
  natural: { brightness: 0, contrast: 0, saturation: 100, hue: 0, blur: 0, filter: '', highlights: 0, shadows: 0, fade: 0, vignette: 0, grain: 0, lutId: '', lutIntensity: 100 },
};

function sameAdjustmentValues(a: AdjustmentValues, b: AdjustmentValues) {
  return sameRecipeValues(
    {
      brightness: a.brightness,
      contrast: a.contrast,
      saturation: a.saturation,
      hue: a.hue,
      blur: a.blur,
      filter: (a.filter || '') as any,
      highlights: a.highlights ?? 0,
      shadows: a.shadows ?? 0,
      fade: a.fade ?? 0,
      vignette: a.vignette ?? 0,
      grain: a.grain ?? 0,
      lutId: a.lutId || '',
      lutIntensity: a.lutIntensity ?? 100,
    },
    {
      brightness: b.brightness,
      contrast: b.contrast,
      saturation: b.saturation,
      hue: b.hue,
      blur: b.blur,
      filter: (b.filter || '') as any,
      highlights: b.highlights ?? 0,
      shadows: b.shadows ?? 0,
      fade: b.fade ?? 0,
      vignette: b.vignette ?? 0,
      grain: b.grain ?? 0,
      lutId: b.lutId || '',
      lutIntensity: b.lutIntensity ?? 100,
    },
  );
}

function resolvePresetKey(values: AdjustmentValues) {
  if (autoPresetValues.value && sameAdjustmentValues(autoPresetValues.value, values)) {
    return 'auto';
  }
  for (const recipe of allPhotoStyles(customPhotoStyles.value)) {
    if (sameAdjustmentValues(recipe as AdjustmentValues, values)) {
      return recipe.id;
    }
  }
  return 'custom';
}

function getCurrentAdjustmentValues(): AdjustmentValues {
  return {
    brightness: brightness.value,
    contrast: contrast.value,
    saturation: saturation.value,
    hue: hue.value,
    blur: blur.value,
    filter: selectedFilter.value,
    highlights: styleHighlights.value,
    shadows: styleShadows.value,
    fade: styleFade.value,
    vignette: styleVignette.value,
    grain: styleGrain.value,
    lutId: styleLutId.value,
    lutIntensity: styleLutIntensity.value,
  };
}

function getConfiguredCustomPreset(): AdjustmentValues {
  const c = config.imageEditor.custom as any;
  return {
    brightness: Number(c?.brightness ?? 0),
    contrast: Number(c?.contrast ?? 0),
    saturation: Number(c?.saturation ?? 100),
    hue: Number(c?.hue ?? 0),
    blur: Number(c?.blur ?? 0),
    filter: String(c?.filter ?? ''),
    highlights: Number(c?.highlights ?? 0),
    shadows: Number(c?.shadows ?? 0),
    fade: Number(c?.fade ?? 0),
    vignette: Number(c?.vignette ?? 0),
    grain: Number(c?.grain ?? 0),
    lutId: String(c?.lutId ?? ''),
    lutIntensity: Number(c?.lutIntensity ?? 100),
  };
}

function persistCustomPreset(values = getCurrentAdjustmentValues()) {
  config.imageEditor.custom = {
    brightness: values.brightness,
    contrast: values.contrast,
    saturation: values.saturation,
    hue: values.hue,
    blur: values.blur,
    filter: values.filter,
    highlights: values.highlights ?? 0,
    shadows: values.shadows ?? 0,
    fade: values.fade ?? 0,
    vignette: values.vignette ?? 0,
    grain: values.grain ?? 0,
    lutId: values.lutId || '',
    lutIntensity: values.lutIntensity ?? 100,
  } as any;
}

function applyAdjustmentValues(values: AdjustmentValues) {
  brightness.value = values.brightness;
  contrast.value = values.contrast;
  saturation.value = values.saturation;
  hue.value = values.hue;
  blur.value = values.blur;
  selectedFilter.value = values.filter;
  styleHighlights.value = values.highlights ?? 0;
  styleShadows.value = values.shadows ?? 0;
  styleFade.value = values.fade ?? 0;
  styleVignette.value = values.vignette ?? 0;
  styleGrain.value = values.grain ?? 0;
  styleLutId.value = values.lutId || '';
  styleLutIntensity.value = values.lutIntensity ?? 100;
}

const presetOptions = computed(() => {
  const pe = localeMsg.value.msgbox.image_editor.presets as Record<string, string>;
  const ps = (localeMsg.value as any).photo_style || {};
  const labelFor = (id: string, fallback: string) => {
    if (pe[id]) return pe[id];
    if (id === 'portrait') return pe.portrait || ps.portrait || fallback;
    if (id === 'landscape') return pe.landscape || ps.landscape || fallback;
    if (id === 'nostalgic') return pe.nostalgic || ps.nostalgic || fallback;
    return fallback;
  };
  const items: { value: string; label: string }[] = [
    { value: 'auto', label: pe.auto },
  ];
  for (const recipe of allPhotoStyles(customPhotoStyles.value)) {
    items.push({ value: recipe.id, label: labelFor(recipe.id, recipe.name) });
  }
  items.push({ value: 'custom', label: pe.custom });
  return items;
});
const lightSliders = computed(() => [
  {
    key: 'brightness',
    label: localeMsg.value.msgbox.image_editor.brightness,
    model: brightness,
    min: -100,
    max: 100,
    step: 1,
    valueDisplay: `${brightness.value > 0 ? '+' : ''}${brightness.value}`,
  },
  {
    key: 'contrast',
    label: localeMsg.value.msgbox.image_editor.contrast,
    model: contrast,
    min: -100,
    max: 100,
    step: 1,
    valueDisplay: `${contrast.value > 0 ? '+' : ''}${contrast.value}`,
  },
]);
const colorSliders = computed(() => [
  {
    key: 'saturation',
    label: localeMsg.value.msgbox.image_editor.saturation,
    model: saturation,
    min: 0,
    max: 200,
    step: 1,
    valueDisplay: `${saturation.value}%`,
  },
  {
    key: 'hue',
    label: localeMsg.value.msgbox.image_editor.hue_rotate,
    model: hue,
    min: -180,
    max: 180,
    step: 1,
    valueDisplay: `${hue.value > 0 ? '+' : ''}${hue.value}`,
  },
  {
    key: 'blur',
    label: localeMsg.value.msgbox.image_editor.blur,
    model: blur,
    min: 0,
    max: 20,
    step: 1,
    valueDisplay: `${blur.value}`,
  },
]);
const effectSliders = computed(() => [
  {
    key: 'highlights',
    label: (localeMsg.value as any).photo_style?.highlights || localeMsg.value.msgbox.image_editor.highlights || 'Highlights',
    model: styleHighlights,
    min: -100,
    max: 100,
    step: 1,
    valueDisplay: `${styleHighlights.value > 0 ? '+' : ''}${styleHighlights.value}`,
  },
  {
    key: 'shadows',
    label: (localeMsg.value as any).photo_style?.shadows || localeMsg.value.msgbox.image_editor.shadows || 'Shadows',
    model: styleShadows,
    min: -100,
    max: 100,
    step: 1,
    valueDisplay: `${styleShadows.value > 0 ? '+' : ''}${styleShadows.value}`,
  },
  {
    key: 'fade',
    label: (localeMsg.value as any).photo_style?.fade || 'Fade',
    model: styleFade,
    min: 0,
    max: 100,
    step: 1,
    valueDisplay: `${styleFade.value}`,
  },
  {
    key: 'vignette',
    label: (localeMsg.value as any).photo_style?.vignette || 'Vignette',
    model: styleVignette,
    min: 0,
    max: 100,
    step: 1,
    valueDisplay: `${styleVignette.value}`,
  },
  {
    key: 'grain',
    label: (localeMsg.value as any).photo_style?.grain || 'Grain',
    model: styleGrain,
    min: 0,
    max: 100,
    step: 1,
    valueDisplay: `${styleGrain.value}`,
  },
]);

const hasAdjustmentChanges = computed(() => {
  const p = getBuiltinRecipe('natural') || presets.natural;
  return (
    brightness.value !== p.brightness ||
    contrast.value !== p.contrast ||
    saturation.value !== p.saturation ||
    hue.value !== p.hue ||
    blur.value !== p.blur ||
    selectedFilter.value !== (p.filter || '') ||
    styleHighlights.value !== (p.highlights || 0) ||
    styleShadows.value !== (p.shadows || 0) ||
    styleFade.value !== (p.fade || 0) ||
    styleVignette.value !== (p.vignette || 0) ||
    styleGrain.value !== (p.grain || 0) ||
    !!styleLutId.value ||
    hasColorMatch.value
  );
});

const showPhotoSizeManageDialog = ref(false);
const showAddCustomRatioDialog = ref(false);

function ensureCropPresetConfig() {
  const editor = config.imageEditor as any;
  if (!Array.isArray(editor.customCropRatios)) {
    editor.customCropRatios = [];
  } else {
    editor.customCropRatios = normalizeCustomCropRatios(editor.customCropRatios);
  }

  const hasPresetId = typeof editor.cropPresetId === 'string' && editor.cropPresetId.length > 0;
  if (!hasPresetId) {
    editor.cropPresetId = migrateLegacyCropShape(editor.cropShape);
  }

  const resolved = resolveCropPreset(editor.cropPresetId, editor.customCropRatios);
  editor.cropPresetId = resolved.id;
}

ensureCropPresetConfig();

// Prefer the persisted array as-is after ensure/normalize; avoid cloning on every reactive read.
const customCropRatios = computed<CustomCropRatio[]>(() => {
  const raw = (config.imageEditor as any).customCropRatios;
  return Array.isArray(raw) ? (raw as CustomCropRatio[]) : [];
});

const activeCropPreset = computed<ResolvedCropPreset>(() =>
  resolveCropPreset((config.imageEditor as any).cropPresetId, customCropRatios.value),
);

const cropPresetSelectValue = computed(() => activeCropPreset.value.id);

// Cache aspect once per preset/orientation change (used heavily while dragging crop handles).
const activeCropAspectRatio = computed<number | null>(() => {
  const base = getPresetBaseRatio(activeCropPreset.value);
  if (!base) return null;
  return getCropAspectRatio(base.ratioW, base.ratioH, isPortrait.value);
});

const ratioCropOptions = computed(() => {
  const portrait = isPortrait.value;
  return BUILTIN_RATIO_PRESETS.map((preset) => ({
    value: preset.id,
    label: formatRatioLabel(preset.ratioW, preset.ratioH, portrait),
  }));
});

const customCropOptions = computed(() => {
  const portrait = isPortrait.value;
  return customCropRatios.value.map((preset) => ({
    value: preset.id,
    label: `${preset.name} (${formatRatioLabel(preset.ratioW, preset.ratioH, portrait)})`,
  }));
});

const photoSizeCropOptions = computed(() => {
  const portrait = isPortrait.value;
  const names = localeMsg.value.msgbox.image_editor.photo_sizes;
  return BUILTIN_PHOTO_SIZE_PRESETS.map((preset) => {
    const name = names[preset.nameKey] || preset.nameKey;
    const ratio = formatRatioLabel(preset.pxW, preset.pxH, portrait);
    return {
      value: preset.id,
      label: `${name} (${ratio})`,
    };
  });
});

const cropTargetHint = computed(() => {
  const preset = activeCropPreset.value;
  if (preset.kind !== 'photo') return '';
  const target = getPhotoTargetPixels(preset, isPortrait.value);
  const title = localeMsg.value.msgbox.image_editor.crop_target_size;
  return `${title}: ${target.width} × ${target.height} px @ ${preset.dpi} DPI · ${preset.cmW}×${preset.cmH} cm`;
});

function setCropPresetId(presetId: string) {
  const resolved = resolveCropPreset(presetId, customCropRatios.value);
  (config.imageEditor as any).cropPresetId = resolved.id;
  // Keep legacy field roughly aligned for older readers.
  (config.imageEditor as any).cropShape = resolved.kind === 'free' ? 0 : 1;
}

function applyPhotoTargetResize(preset: ResolvedCropPreset) {
  if (preset.kind !== 'photo') return;
  const target = getPhotoTargetPixels(preset, isPortrait.value);
  keepAspectRatio.value = true;
  resizeWidthInput.value = String(target.width);
  resizeHeightInput.value = String(target.height);
}

function onCropPresetSelectChange(event: Event) {
  const value = String((event.target as HTMLSelectElement | null)?.value || FREE_CROP_PRESET_ID);

  if (value === MANAGE_PHOTO_SIZES_ID) {
    showPhotoSizeManageDialog.value = true;
    return;
  }
  if (value === ADD_CUSTOM_RATIO_ID) {
    showAddCustomRatioDialog.value = true;
    return;
  }

  setCropPresetId(value);
  applyPhotoTargetResize(activeCropPreset.value);
  onChangeCropShape();
}

function onCustomCropRatiosUpdated(next: CustomCropRatio[]) {
  (config.imageEditor as any).customCropRatios = normalizeCustomCropRatios(next);
  // Drop selection if the active custom preset was deleted.
  const resolved = resolveCropPreset((config.imageEditor as any).cropPresetId, customCropRatios.value);
  setCropPresetId(resolved.id);
  if (cropStatus.value === 1) {
    onChangeCropShape();
  }
}

function onAddCustomCropRatio(ratio: CustomCropRatio) {
  const next = [...customCropRatios.value, ratio];
  (config.imageEditor as any).customCropRatios = next;
  showAddCustomRatioDialog.value = false;
  setCropPresetId(ratio.id);
  onChangeCropShape();
}

const newFileName = ref('');

const fileFormatOptions = computed(() => getSelectOptions(localeMsg.value.msgbox.image_editor.format_options));
const fileQualityOptions = computed(() => getSelectOptions(localeMsg.value.msgbox.image_editor.quality_options));
const outputFormatValues = ['jpg', 'png', 'webp'] as const;

function getSelectedOutputFormat() {
  return outputFormatValues[config.imageEditor.format] || outputFormatValues[0];
}

const combinedFormatKey = computed({
  get: () => {
    if (config.imageEditor.format !== 0) return String(config.imageEditor.format);
    return `0-${config.imageEditor.quality}`;
  },
  set: (key: string) => {
    if (key.includes('-')) {
      const [f, q] = key.split('-').map(Number);
      config.imageEditor.format = f;
      config.imageEditor.quality = q;
    } else {
      config.imageEditor.format = Number(key);
      config.imageEditor.quality = 0;
    }
  },
});

const combinedFormatOptions = computed(() => {
  const fmt = fileFormatOptions.value;
  const qual = fileQualityOptions.value;
  const items: { value: string; label: string }[] = [];
  // JPEG with quality levels
  items.push({ value: '0-0', label: `${fmt[0].label} (${qual[0].label})` });
  items.push({ value: '0-1', label: `${fmt[0].label} (${qual[1].label})` });
  items.push({ value: '0-2', label: `${fmt[0].label} (${qual[2].label})` });
  // PNG, WebP — no quality variants
  for (let i = 1; i < fmt.length; i++) {
    items.push({ value: String(i), label: fmt[i].label });
  }
  return items;
});

const canOverwriteOriginal = computed(() => {
  const ext = getFileExtension(fileInfo.value?.name || fileInfo.value?.file_path || '').toLowerCase();
  return ['jpg', 'jpeg', 'png', 'webp'].includes(ext);
});
const effectiveSaveAsNew = computed(() => config.imageEditor.saveAs === 1 || !canOverwriteOriginal.value);

const showOverwriteConfirm = ref(false);

const handleOverwriteConfirm = () => {
  showOverwriteConfirm.value = false;

  if (!canOverwriteOriginal.value) {
    return;
  }

  const originalPath = fileInfo.value.file_path;
  const ext = getFileExtension(fileInfo.value.name).toLowerCase();
  const outputFormat = (ext === 'jpg' || ext === 'jpeg') ? 'jpg' : ext;

  executeSave({
    destFilePath: originalPath,
    outputFormat,
  });
};

const handleOverwriteCancel = () => {
  showOverwriteConfirm.value = false;
  isProcessing.value = false;
};

const handleResizeWidthInput = () => {
  const width = parsedResizeWidth.value;
  if (!width) return;

  const clampedWidth = Math.min(maxResizeWidth.value, Math.max(1, width));
  if (clampedWidth !== width) {
    resizeWidthInput.value = String(clampedWidth);
  }

  if (!keepAspectRatio.value) return;
  resizeHeightInput.value = String(Math.min(maxResizeHeight.value, Math.max(1, Math.round(clampedWidth / resizeAspectRatio.value))));
};

const handleResizeHeightInput = () => {
  const height = parsedResizeHeight.value;
  if (!height) return;

  const clampedHeight = Math.min(maxResizeHeight.value, Math.max(1, height));
  if (clampedHeight !== height) {
    resizeHeightInput.value = String(clampedHeight);
  }

  if (!keepAspectRatio.value) return;
  resizeWidthInput.value = String(Math.min(maxResizeWidth.value, Math.max(1, Math.round(clampedHeight * resizeAspectRatio.value))));
};

const resetResize = () => {
  resizeWidthInput.value = String(baseOutputWidth.value);
  resizeHeightInput.value = String(baseOutputHeight.value);
  keepAspectRatio.value = true;
};

watch(
  () => [baseOutputWidth.value, baseOutputHeight.value],
  ([width, height]) => {
    resizeWidthInput.value = width > 0 ? String(width) : '';
    resizeHeightInput.value = height > 0 ? String(height) : '';
  },
  { immediate: true }
);

watch(
  () => keepAspectRatio.value,
  (enabled) => {
    if (!enabled || !parsedResizeWidth.value) return;
    resizeHeightInput.value = String(Math.max(1, Math.round(parsedResizeWidth.value / resizeAspectRatio.value)));
  }
);

watch(
  () => fileInfo.value?.file_path,
  () => {
    invalidateHostPreviewClientCache();
  },
);

watch(
  [cropApplied, () => crop.value.left, () => crop.value.top, () => crop.value.width, () => crop.value.height, isFlippedX, isFlippedY, rotate],
  () => {
    invalidateHostPreviewClientCache();
    schedulePhotoStylePreview();
    scheduleColorMatchPreview();
  },
);

watch(selectedPreset, () => {
  flushPersistNamedCustom();
  if (selectedPreset.value !== 'auto' && selectedPreset.value !== 'custom') {
    (config.imageEditor as any).activePhotoStyleId = selectedPreset.value;
  }

  if (selectedPreset.value === 'custom') {
    autoPresetRequestId++;
    if (skipNextCustomPresetLoad) {
      skipNextCustomPresetLoad = false;
      schedulePhotoStylePreview();
      return;
    }

    const custom = getConfiguredCustomPreset();
    isApplyingPreset = true;
    applyAdjustmentValues(custom);
    nextTick(() => {
      isApplyingPreset = false;
      schedulePhotoStylePreview();
    });
    return;
  }

  if (selectedPreset.value === 'auto') {
    applyAutoPreset();
    return;
  }

  const recipe = findPhotoStyle(selectedPreset.value, customPhotoStyles.value);
  if (!recipe) return;
  autoPresetRequestId++;
  isApplyingPreset = true;
  applyRecipeValues(recipe);
  nextTick(() => {
    isApplyingPreset = false;
    schedulePhotoStylePreview();
  });
});

watch(activeEditorTab, (tab) => {
  if (tab !== 'adjust') {
    showDiffPreview.value = false;
    showOriginalWhilePressed.value = false;
    return;
  }
});



watch(
  [
    colorMatchReferencePath,
    colorMatchIntensity,
    colorMatchTone,
    colorMatchHighlight,
    colorMatchShadow,
    colorMatchAutoWb,
  ],
  () => {
    scheduleColorMatchPreview();
  },
);

watch(histogramSource, () => {
  autoPresetValues.value = null;
});


watch(
  [
    brightness, contrast, saturation, hue, blur, selectedFilter,
    styleHighlights, styleShadows, styleFade, styleVignette, styleGrain,
    styleLutId, styleLutIntensity,
  ],
  () => {
  if (isApplyingPreset) return;

  const currentValues = getCurrentAdjustmentValues();
  const resolvedPreset = resolvePresetKey(currentValues);

  if (resolvedPreset === 'natural') {
    showDiffPreview.value = false;
  }

  if (resolvedPreset === 'custom') {
    persistCustomPreset(currentValues);
  }

  if (selectedPreset.value !== 'custom' && selectedPreset.value !== 'auto') {
    const p = findPhotoStyle(selectedPreset.value, customPhotoStyles.value);
    if (!p) {
      skipNextCustomPresetLoad = true;
      selectedPreset.value = 'custom';
      schedulePhotoStylePreview();
      return;
    }
    // Named custom: update recipe values in place (stable order). Builtins: drift to anonymous custom slot.
    if (!p.builtIn) {
      schedulePersistNamedCustom();
    } else if (!sameAdjustmentValues(p as AdjustmentValues, currentValues)) {
      skipNextCustomPresetLoad = true;
      selectedPreset.value = 'custom';
    }
  }

  schedulePhotoStylePreview();
});

watch(
  [brightness, contrast, saturation, hue, blur, selectedFilter, styleHighlights, styleShadows, styleFade, styleVignette, styleGrain, styleLutId, styleLutIntensity, () => resizeOutput.value.width, () => resizeOutput.value.height],
  () => {
    if (!fileInfo.value?.file_path) return;
    uiStore.setActiveAdjustments(fileInfo.value.file_path, {
      brightness: brightness.value,
      contrast: contrast.value,
      saturation: saturation.value,
      hue: hue.value,
      blur: blur.value,
      filter: selectedFilter.value || null,
      highlights: styleHighlights.value,
      shadows: styleShadows.value,
      fade: styleFade.value,
      vignette: styleVignette.value,
      grain: styleGrain.value,
      lutId: styleLutId.value || null,
      lutIntensity: styleLutIntensity.value,
      resize: resizeOutput.value.hasResize ? {
        width: resizeOutput.value.width,
        height: resizeOutput.value.height,
      } : null,
    });
  },
  { immediate: true }
);

watch(() => config.settings.language, (newLanguage) => {
  locale.value = newLanguage;
});

watch(() => config.settings.appearance, (newAppearance) => {
  setTheme(newAppearance, newAppearance === 0 ? config.settings.lightTheme : config.settings.darkTheme);
});

watch(() => config.settings.lightTheme, (newLightTheme) => {
  setTheme(config.settings.appearance, newLightTheme);
});

watch(() => config.settings.darkTheme, (newDarkTheme) => {
  setTheme(config.settings.appearance, newDarkTheme);
});

watch(() => Number(config.settings.scale || 1), (newScale) => {
  const normalizedScale = SCALE_VALUES.find((item) => item === newScale) ?? 1;
  document.documentElement.style.fontSize = `${normalizedScale * 16}px`;
});

onMounted(async () => {
  void refreshLutCache();

  window.addEventListener('keydown', handleKeyDown);
  uiStore.pushInputHandler('EditImage');
  ensureCropPresetConfig();
  activeEditorTab.value = config.imageEditor.tab === 'adjust' ? 'adjust' : 'edit';

  const query = router.currentRoute.value.query;
  const fileId = Number(query.fileId || 0);
  if (fileId > 0) {
    await loadFileInfo(fileId);
  }

  if (!fileInfo.value) {
    await closeEditorWindow();
    return;
  }

  isProcessing.value = true;
  initEditImage();

  unlistenUpdateFile = await listen('update-file', async (event: any) => {
    const newFileId = Number(event?.payload?.fileId || 0);
    if (newFileId > 0 && newFileId !== Number(fileInfo.value?.id || 0)) {
      await loadFileInfo(newFileId);
      if (fileInfo.value) {
        initEditImage();
      }
    }
  });

  containerResizeObserver = new ResizeObserver(() => {
    isResizing.value = true;
    enableTransition.value = false;
    cropBoxFixed.value = false;
    containerRect.value = containerRef.value?.getBoundingClientRect() || null;
    if (containerRect.value) {
      containerBounds.value = {
        left: containerRect.value.left + containerPadding,
        top: containerRect.value.top + containerPadding,
        width: containerRect.value.width - containerPadding * 2,
        height: containerRect.value.height - containerPadding * 2,
      };
      autoFitVisualArea();
    }
    if (cropStatus.value === 1 || cropApplied.value) {
      requestAnimationFrame(() => {
        imageRectOriginal.value = imageRef.value?.getBoundingClientRect() || null;
        updateCropBoxFromCrop();
        enableTransition.value = true;
        isResizing.value = false;
      });
    } else {
      enableTransition.value = true;
      isResizing.value = false;
    }
  });
  if (containerRef.value) {
    containerResizeObserver.observe(containerRef.value);
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  uiStore.removeInputHandler('EditImage');
  if (containerResizeObserver) {
    containerResizeObserver.disconnect();
    containerResizeObserver = null;
  }
  if (unlistenUpdateFile) {
    unlistenUpdateFile();
    unlistenUpdateFile = null;
  }
  if (colorMatchDebounceTimer) {
    clearTimeout(colorMatchDebounceTimer);
    colorMatchDebounceTimer = null;
  }
  colorMatchPreviewRequestId += 1;
  revokeColorMatchPreviewUrl();
  if (photoStyleDebounceTimer) {
    clearTimeout(photoStyleDebounceTimer);
    photoStyleDebounceTimer = null;
  }
  photoStylePreviewRequestId += 1;
  revokePhotoStylePreviewUrl();
});

const onImageLoad = async () => {
  await nextTick();

  // Color-match preview is intentionally downscaled; do not overwrite source dimensions used by crop/save.
  if (
    !colorMatchPreviewUrl.value
    && !photoStylePreviewUrl.value
    && imageRef.value
    && imageRef.value.naturalWidth > 0
    && imageRef.value.naturalHeight > 0
  ) {
    imageWidth.value = imageRef.value.naturalWidth;
    imageHeight.value = imageRef.value.naturalHeight;
    isPortrait.value = isPortraitForRotation(imageWidth.value, imageHeight.value, rotate.value);
  }

  autoFitVisualArea();

  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      enableTransition.value = true;
      imageReady.value = true;
      isProcessing.value = false;
    });
  });
};

const initEditImageLoadingId = ref(0);

const initEditImage = async () => {
  initEditImageLoadingId.value++;
  const loadingId = initEditImageLoadingId.value;

  if (usesBackendPreview.value) {
    if (initialImageSrc.value) {
      imageSrc.value = initialImageSrc.value;
    }

    void (async () => {
      try {
        if (loadingId !== initEditImageLoadingId.value) return;
        const previewSrc = getPreviewUrl(fileInfo.value.id, fileInfo.value.file_path);
        if (previewSrc) {
          imageSrc.value = previewSrc;
        }
      } catch {
        if (loadingId !== initEditImageLoadingId.value) return;
      }
    })();
  } else {
    imageSrc.value = getAssetSrc(fileInfo.value.file_path);
  }

  imageWidth.value = fileInfo.value.width;
  imageHeight.value = fileInfo.value.height;
  isPortrait.value = isPortraitForRotation(imageWidth.value, imageHeight.value, initialDisplayRotate.value);
  if (isRawFile.value || !canOverwriteOriginal.value) {
    config.imageEditor.saveAs = 1;
  }

  containerRect.value = containerRef.value?.getBoundingClientRect() || null;
  if (!containerRect.value) return;

  containerBounds.value = {
    left: containerRect.value.left + containerPadding,
    top: containerRect.value.top + containerPadding,
    width: containerRect.value.width - containerPadding * 2,
    height: containerRect.value.height - containerPadding * 2,
  };

  enableTransition.value = false;

  if (uiStore.activeAdjustments.filePath === fileInfo.value.file_path) {
    const adj = uiStore.activeAdjustments;
    rotate.value = initialDisplayRotate.value;
    isFlippedX.value = false;
    isFlippedY.value = false;
    brightness.value = adj.brightness || 0;
    contrast.value = adj.contrast || 0;
    saturation.value = adj.saturation ?? 100;
    hue.value = adj.hue || 0;
    blur.value = adj.blur || 0;
    selectedFilter.value = adj.filter || '';
    styleHighlights.value = adj.highlights || 0;
    styleShadows.value = adj.shadows || 0;
    styleFade.value = adj.fade || 0;
    styleVignette.value = adj.vignette || 0;
    styleGrain.value = adj.grain || 0;
    styleLutId.value = adj.lutId || '';
    styleLutIntensity.value = adj.lutIntensity ?? 100;
    const restoredPreset = resolvePresetKey(getCurrentAdjustmentValues());
    if (restoredPreset === 'custom') {
      skipNextCustomPresetLoad = true;
    }
    selectedPreset.value = restoredPreset;
    schedulePhotoStylePreview();
  } else {
    rotate.value = initialDisplayRotate.value;
    isFlippedX.value = false;
    isFlippedY.value = false;
    const preferred = String((config.imageEditor as any).activePhotoStyleId || 'natural');
    if (preferred && preferred !== 'natural' && preferred !== 'custom' && preferred !== 'auto'
      && findPhotoStyle(preferred, customPhotoStyles.value)) {
      skipNextCustomPresetLoad = false;
      selectedPreset.value = preferred;
    } else {
      resetAdjustments();
    }
  }

};

async function getAutoPresetValues() {
  if (autoPresetValues.value) return autoPresetValues.value;
  autoPresetValues.value = await histogramRef.value?.getAutoPresetValues() || presets.natural;
  return autoPresetValues.value;
}

async function applyAutoPreset() {
  const requestId = ++autoPresetRequestId;
  try {
    const values = await getAutoPresetValues();
    if (requestId !== autoPresetRequestId || selectedPreset.value !== 'auto') return;
    isApplyingPreset = true;
    applyAdjustmentValues({
      ...values,
      highlights: 0,
      shadows: 0,
      fade: 0,
      vignette: 0,
      grain: 0,
      lutId: '',
      lutIntensity: 100,
    });
  } finally {
    if (isApplyingPreset && requestId === autoPresetRequestId) {
      nextTick(() => {
        isApplyingPreset = false;
        schedulePhotoStylePreview();
      });
    }
  }
}

function presetThumbnailFilter(presetKey: string) {
  if (presetKey === 'custom') {
    return buildAdjustmentFilter({
      brightness: brightness.value,
      contrast: contrast.value,
      saturation: saturation.value,
      hue: hue.value,
      blur: blur.value,
      filter: selectedFilter.value,
    });
  }
  if (presetKey === 'auto') {
    const p = autoPresetValues.value || presets.natural;
    return buildAdjustmentFilter(p);
  }
  const recipe = findPhotoStyle(presetKey, customPhotoStyles.value);
  if (!recipe) return '';
  return buildAdjustmentFilter({
    brightness: recipe.brightness,
    contrast: recipe.contrast,
    saturation: recipe.saturation,
    hue: recipe.hue,
    blur: recipe.blur || 0,
    filter: recipe.filter || '',
  });
}

const resetAdjustments = () => {
  const p = getBuiltinRecipe('natural') || defaultPhotoStyle({ id: 'natural', name: 'Original', builtIn: true });
  applyRecipeValues(p);
  selectedPreset.value = 'natural';
  (config.imageEditor as any).activePhotoStyleId = 'natural';
  showDiffPreview.value = false;
  showOriginalWhilePressed.value = false;
  revokePhotoStylePreviewUrl();
  invalidateHostPreviewClientCache();
  photoStyleError.value = '';
};

function setActiveEditorTab(tab: 'edit' | 'adjust') {
  if (cropStatus.value === 1) return;
  activeEditorTab.value = tab;
  config.imageEditor.tab = tab;
}

function handlePreviewPointerDown() {
  if (activeEditorTab.value !== 'adjust' || showDiffPreview.value || !hasAdjustmentChanges.value) return;
  showOriginalWhilePressed.value = true;
}

function handlePreviewPointerUp() {
  showOriginalWhilePressed.value = false;
}

function toggleDiffPreview() {
  if (!hasAdjustmentChanges.value) return;
  showOriginalWhilePressed.value = false;
  showDiffPreview.value = !showDiffPreview.value;
}

const clickStartCrop = () => {
  cropStatus.value = 1;
  cropApplied.value = false;
  cropBoxFixed.value = false;
  initCropBox();
};

const toggleCropMode = () => {
  if (cropStatus.value === 1) {
    clearCrop();
    return;
  }

  if (cropApplied.value) {
    clickRestoreCrop();
    return;
  }

  if (cropStatus.value === 0) {
    clickStartCrop();
    return;
  }
};

const clearCrop = () => {
  cropStatus.value = 0;
  cropApplied.value = false;
  cropBoxFixed.value = false;
  crop.value = { left: 0, top: 0, width: 0, height: 0 };
  cropBox.value = { left: 0, top: 0, width: 0, height: 0 };
  autoFitVisualArea();
};

const clickRestoreAll = () => {
  if (cropStatus.value === 1 || cropApplied.value) return;

  rotate.value = initialDisplayRotate.value;
  isFlippedX.value = false;
  isFlippedY.value = false;
  isPortrait.value = isPortraitForRotation(imageWidth.value, imageHeight.value, initialDisplayRotate.value);
  autoFitVisualArea();
};

const clickCancelCrop = () => {
  cropStatus.value = 0;
  cropApplied.value = false;
  crop.value = { left: 0, top: 0, width: 0, height: 0 };
  cropBox.value = { left: 0, top: 0, width: 0, height: 0 };
  autoFitVisualArea();
};

const clickRestoreCrop = () => {
  cropStatus.value = 1;
  cropBoxFixed.value = false;
  autoFitVisualArea();
};

const autoFitVisualArea = () => {
  if (cropApplied.value) {
    fitCropBoxToContainer();
  } else {
    fitImageToContainer();
  }
};

const clickDoCrop = () => {
  cropApplied.value = true;
  cropBoxFixed.value = false;
  fitCropBoxToContainer();

  cropStatus.value = 0;
};

const togglePortraitAndLandscape = () => {
  isPortrait.value = !isPortrait.value;
  applyPhotoTargetResize(activeCropPreset.value);
  initCropBox();
};

const toggleCropBoxFixed = () => {
  cropBoxFixed.value = !cropBoxFixed.value;
  cropBoxFixed.value ? fitCropBoxToContainer() : fitImageToContainer();
};

const onChangeCropShape = () => {
  initCropBox();
};

const refreshCropLayoutRects = () => {
  containerRect.value = containerRef.value?.getBoundingClientRect() || null;
  imageRect.value = imageRef.value?.getBoundingClientRect() || null;
  return !!(imageRect.value && containerRect.value);
};

const initCropBox = () => {
  if (!refreshCropLayoutRects()) return;

  const aspectRatio = activeCropAspectRatio.value;
  if (aspectRatio) {
    let newWidth;
    let newHeight;
    if (imageRect.value!.width / imageRect.value!.height > aspectRatio) {
      newHeight = imageRect.value!.height;
      newWidth = newHeight * aspectRatio;
    } else {
      newWidth = imageRect.value!.width;
      newHeight = newWidth / aspectRatio;
    }

    const imageLeft = imageRect.value!.left - containerRect.value!.left;
    const imageTop = imageRect.value!.top - containerRect.value!.top;

    cropBox.value = {
      left: imageLeft + (imageRect.value!.width - newWidth) / 2,
      top: imageTop + (imageRect.value!.height - newHeight) / 2,
      width: newWidth,
      height: newHeight,
    };
  } else {
    cropBox.value = {
      left: imageRect.value!.left - containerRect.value!.left,
      top: imageRect.value!.top - containerRect.value!.top,
      width: imageRect.value!.width,
      height: imageRect.value!.height,
    };
  }

  updateCropFromCropBox({ refreshRects: false });
};

/**
 * Map on-screen crop box → source image pixels.
 * During drag, pass refreshRects:false and reuse cached image/container rects
 * (layout does not change while resizing the box).
 */
const updateCropFromCropBox = (options: { refreshRects?: boolean } = {}) => {
  if (cropBox.value.width === 0 || cropBox.value.height === 0) {
    crop.value = { left: 0, top: 0, width: 0, height: 0 };
    return;
  }

  if (options.refreshRects !== false) {
    refreshCropLayoutRects();
  }
  if (!imageRect.value || !containerRect.value) return;

  const imgWidth = rotate.value % 180 === 0 ? imageWidth.value : imageHeight.value;
  const imgHeight = rotate.value % 180 === 0 ? imageHeight.value : imageWidth.value;
  if (imageRect.value.width <= 0 || imageRect.value.height <= 0) return;

  const scaleX = imgWidth / imageRect.value.width;
  const scaleY = imgHeight / imageRect.value.height;

  crop.value = {
    left: Math.round(scaleX * (cropBox.value.left + containerRect.value.left - imageRect.value.left)),
    top: Math.round(scaleY * (cropBox.value.top + containerRect.value.top - imageRect.value.top)),
    width: Math.round(scaleX * cropBox.value.width),
    height: Math.round(scaleY * cropBox.value.height),
  };
};

const updateCropBoxFromCrop = () => {
  if (crop.value.width === 0 || crop.value.height === 0) {
    cropBox.value = { left: 0, top: 0, width: 0, height: 0 };
    return;
  }

  imageRect.value = imageRectOriginal.value;
  if (!imageRect.value || !containerRect.value) return;

  const imgWidth = rotate.value % 180 === 0 ? imageWidth.value : imageHeight.value;
  const imgHeight = rotate.value % 180 === 0 ? imageHeight.value : imageWidth.value;

  const scaleX = imgWidth / imageRect.value.width;
  const scaleY = imgHeight / imageRect.value.height;

  if (scaleX === 0 || scaleY === 0) return;

  cropBox.value = {
    left: (crop.value.left / scaleX) - containerRect.value.left + imageRect.value.left,
    top: (crop.value.top / scaleY) - containerRect.value.top + imageRect.value.top,
    width: crop.value.width / scaleX,
    height: crop.value.height / scaleY,
  };
};

const scaleFit = (imgWidth: number, imgHeight: number) => {
  scale.value = Math.min(containerBounds.value.width / imgWidth, containerBounds.value.height / imgHeight);
};

const fitImageToContainer = () => {
  containerRect.value = containerRef.value?.getBoundingClientRect() || null;
  if (!containerRect.value) return;

  position.value = {
    left: (containerRect.value.width - imageWidth.value) / 2,
    top: (containerRect.value.height - imageHeight.value) / 2,
  };

  rotate.value % 180 !== 0
    ? scaleFit(imageHeight.value, imageWidth.value)
    : scaleFit(imageWidth.value, imageHeight.value);

  updateCropBoxFromCrop();
};

const fitCropBoxToContainer = () => {
  if (!containerBounds.value || !containerRect.value) return;

  imageRectOriginal.value = imageRect.value;
  const oldScale = scale.value;

  scale.value = Math.min(
    (containerBounds.value.width / cropBox.value.width) * oldScale,
    (containerBounds.value.height / cropBox.value.height) * oldScale,
  );

  position.value = {
    left: position.value.left + (containerRect.value.width / 2 - (cropBox.value.left + cropBox.value.width / 2)) * scale.value / oldScale,
    top: position.value.top + (containerRect.value.height / 2 - (cropBox.value.top + cropBox.value.height / 2)) * scale.value / oldScale,
  };

  const newCropBoxWidth = cropBox.value.width * scale.value / oldScale;
  const newCropBoxHeight = cropBox.value.height * scale.value / oldScale;
  cropBox.value = {
    left: (containerRect.value.width - newCropBoxWidth) / 2,
    top: (containerRect.value.height - newCropBoxHeight) / 2,
    width: newCropBoxWidth,
    height: newCropBoxHeight,
  };

  imageRef.value?.addEventListener('transitionend', updateCropFromCropBox, { once: true });
};

const clickRotate = (degree: number) => {
  rotate.value += degree;
  isPortrait.value = !isPortrait.value;
  scaleFit(
    rotate.value % 180 !== 0 ? imageHeight.value : imageWidth.value,
    rotate.value % 180 !== 0 ? imageWidth.value : imageHeight.value,
  );
};

const clickFlipX = () => {
  rotate.value % 180 !== 0
    ? isFlippedY.value = !isFlippedY.value
    : isFlippedX.value = !isFlippedX.value;
};

const clickFlipY = () => {
  rotate.value % 180 !== 0
    ? isFlippedX.value = !isFlippedX.value
    : isFlippedY.value = !isFlippedY.value;
};

const startDrag = (handle: string, event: MouseEvent) => {
  isDragging.value = true;
  dragHandle.value = handle;
  dragStartX.value = event.clientX;
  dragStartY.value = event.clientY;

  if (cropBoxFixed.value && dragHandle.value === 'move') {
    enableTransition.value = false;
  }

  // Snapshot layout once: getBoundingClientRect is expensive if called every mousemove.
  refreshCropLayoutRects();
  const initialCropBoxData = { ...cropBox.value };
  const initialImagePosition = { ...position.value };
  const dragContainerRect = containerRect.value;
  const dragImageRect = imageRect.value;
  const initialImageRect = dragImageRect;
  const aspectRatio = activeCropAspectRatio.value;
  const imgBoundsLeft = dragImageRect && dragContainerRect
    ? dragImageRect.left - dragContainerRect.left
    : 0;
  const imgBoundsTop = dragImageRect && dragContainerRect
    ? dragImageRect.top - dragContainerRect.top
    : 0;
  const imgBoundsRight = dragImageRect ? imgBoundsLeft + dragImageRect.width : 0;
  const imgBoundsBottom = dragImageRect ? imgBoundsTop + dragImageRect.height : 0;

  let rafId = 0;
  let pendingClientX = event.clientX;
  let pendingClientY = event.clientY;

  const applyDrag = (clientX: number, clientY: number) => {
    if (!isDragging.value || !initialImageRect || !dragContainerRect) return;

    const dx = clientX - dragStartX.value;
    const dy = clientY - dragStartY.value;

    if (cropBoxFixed.value && dragHandle.value === 'move') {
      const initialImageLeft = initialImageRect.left - dragContainerRect.left;
      const initialImageRight = initialImageLeft + initialImageRect.width;
      const maxDx = cropBox.value.left - initialImageLeft;
      const minDx = (cropBox.value.left + cropBox.value.width) - initialImageRight;
      const clampedDx = Math.max(minDx, Math.min(dx, maxDx));

      const initialImageTop = initialImageRect.top - dragContainerRect.top;
      const initialImageBottom = initialImageTop + initialImageRect.height;
      const maxDy = cropBox.value.top - initialImageTop;
      const minDy = (cropBox.value.top + cropBox.value.height) - initialImageBottom;
      const clampedDy = Math.max(minDy, Math.min(dy, maxDy));

      position.value.left = initialImagePosition.left + clampedDx;
      position.value.top = initialImagePosition.top + clampedDy;
    } else if (dragHandle.value === 'move') {
      let newLeft = initialCropBoxData.left + dx;
      let newTop = initialCropBoxData.top + dy;

      if (newLeft < imgBoundsLeft) newLeft = imgBoundsLeft;
      if (newTop < imgBoundsTop) newTop = imgBoundsTop;
      if (newLeft + initialCropBoxData.width > imgBoundsRight) newLeft = imgBoundsRight - initialCropBoxData.width;
      if (newTop + initialCropBoxData.height > imgBoundsBottom) newTop = imgBoundsBottom - initialCropBoxData.height;

      cropBox.value.left = newLeft;
      cropBox.value.top = newTop;
    } else {
      let left = initialCropBoxData.left;
      let top = initialCropBoxData.top;
      let width = initialCropBoxData.width;
      let height = initialCropBoxData.height;

      if (dragHandle.value.includes('right')) width += dx;
      if (dragHandle.value.includes('left')) {
        width -= dx;
        left += dx;
      }
      if (dragHandle.value.includes('bottom')) height += dy;
      if (dragHandle.value.includes('top')) {
        height -= dy;
        top += dy;
      }

      if (aspectRatio) {
        if (dragHandle.value.includes('left') || dragHandle.value.includes('right')) {
          height = width / aspectRatio;
        } else {
          width = height * aspectRatio;
        }
        if (dragHandle.value.includes('top')) {
          top = initialCropBoxData.top + (initialCropBoxData.height - height);
        }
        if (dragHandle.value.includes('left')) {
          left = initialCropBoxData.left + (initialCropBoxData.width - width);
        }
      }

      if (
        width >= 10 &&
        height >= 10 &&
        left >= imgBoundsLeft &&
        top >= imgBoundsTop &&
        left + width <= imgBoundsRight + 0.1 &&
        top + height <= imgBoundsBottom + 0.1
      ) {
        cropBox.value.left = left;
        cropBox.value.top = top;
        cropBox.value.width = width;
        cropBox.value.height = height;
      }
    }

    // Reuse cached layout rects while dragging (box moves within fixed image frame).
    updateCropFromCropBox({ refreshRects: false });
  };

  const doDrag = (e: MouseEvent) => {
    pendingClientX = e.clientX;
    pendingClientY = e.clientY;
    if (rafId) return;
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      applyDrag(pendingClientX, pendingClientY);
    });
  };

  const stopDrag = () => {
    if (rafId) {
      cancelAnimationFrame(rafId);
      rafId = 0;
      applyDrag(pendingClientX, pendingClientY);
    }
    if (cropBoxFixed.value && dragHandle.value === 'move') {
      // Image moved under a fixed crop box — remeasure after transform settles.
      refreshCropLayoutRects();
      updateCropFromCropBox({ refreshRects: false });
      enableTransition.value = true;
    }
    isDragging.value = false;
    window.removeEventListener('mousemove', doDrag);
    window.removeEventListener('mouseup', stopDrag);
  };

  window.addEventListener('mousemove', doDrag);
  window.addEventListener('mouseup', stopDrag);
};

function handleKeyDown(event: KeyboardEvent) {
  if (!uiStore.isInputActive('EditImage')) return;

  switch (event.key) {
    case 'ArrowLeft':
      if (activeEditorTab.value === 'adjust' && !isProcessing.value && cropStatus.value !== 1) {
        movePresetSelection(-1);
        event.preventDefault();
        event.stopPropagation();
      }
      break;
    case 'ArrowRight':
      if (activeEditorTab.value === 'adjust' && !isProcessing.value && cropStatus.value !== 1) {
        movePresetSelection(1);
        event.preventDefault();
        event.stopPropagation();
      }
      break;
    case 'Enter':
      if (isProcessing.value) break;
      if (cropStatus.value === 1) {
        clickDoCrop();
      } else {
        clickSave();
      }
      event.preventDefault();
      event.stopPropagation();
      break;
    case 'Escape':
      if (showDiffPreview.value) {
        showDiffPreview.value = false;
      } else if (cropStatus.value === 1) {
        clickCancelCrop();
      } else {
        clickCancel();
      }
      event.preventDefault();
      event.stopPropagation();
      break;
    case ' ':
      if (cropStatus.value === 1) {
        toggleCropBoxFixed();
        event.preventDefault();
        event.stopPropagation();
      }
      break;
    default:
      break;
  }
}

const clickCancel = async () => {
  if (uiStore.activeAdjustments.filePath === fileInfo.value?.file_path) {
    uiStore.clearActiveAdjustments();
  }
  await closeEditorWindow();
};

function closeSaveDropdown() {
  (document.activeElement as HTMLElement)?.blur();
}

function movePresetSelection(direction: number) {
  const currentIndex = presetOptions.value.findIndex(option => option.value === selectedPreset.value);
  if (currentIndex === -1) return;
  const nextIndex = Math.max(0, Math.min(presetOptions.value.length - 1, currentIndex + direction));
  selectedPreset.value = presetOptions.value[nextIndex].value;
}

const setEditParams = (overrides: { fileName?: string; destFilePath?: string; outputFormat?: string } = {}) => {
  let name = overrides.fileName || newFileName.value;
  let outputFormat = overrides.outputFormat || getSelectedOutputFormat();

  let destFilePath = overrides.destFilePath;
  if (!destFilePath) {
    destFilePath = getFullPath(getFolderPath(fileInfo.value.file_path), combineFileName(name, outputFormat));
  }

  return {
    sourceFilePath: fileInfo.value.file_path,
    destFilePath,
    outputFormat,
    quality: [90, 80, 60][config.imageEditor.quality] || 80,
    orientation: fileInfo.value.e_orientation || 1,
    flipHorizontal: isFlippedX.value,
    flipVertical: isFlippedY.value,
    rotate: rotate.value,
    crop: {
      x: crop.value.left,
      y: crop.value.top,
      width: crop.value.width,
      height: crop.value.height,
    },
    resize: {
      width: resizeOutput.value.hasResize ? resizeOutput.value.width : null,
      height: resizeOutput.value.hasResize ? resizeOutput.value.height : null,
    },
    filter: selectedFilter.value || null,
    brightness: brightness.value !== 0 ? brightness.value : null,
    contrast: contrast.value !== 0 ? contrast.value : null,
    blur: blur.value > 0 ? blur.value : null,
    hue_rotate: hue.value !== 0 ? hue.value : null,
    saturation: saturation.value !== 100 ? saturation.value / 100.0 : null,
    colorMatch: colorMatchReferencePath.value
      ? {
          referenceFilePath: colorMatchReferencePath.value,
          intensity: colorMatchIntensity.value / 100,
          tonePreservation: colorMatchTone.value / 100,
          autoWb: colorMatchAutoWb.value,
          highlightProtection: colorMatchHighlight.value / 100,
          shadowProtection: colorMatchShadow.value / 100,
        }
      : null,
    photoStyle: (() => {
      const style = currentWorkingStyle();
      if (isPhotoStyleIdentity(style)) return null;
      // Host photoStyle bakes base+effects+LUT; use whenever non-identity so save matches preview.
      return styleForHost(style);
    })(),
  };
};

function revokeColorMatchPreviewUrl() {
  if (colorMatchPreviewUrl.value) {
    try {
      URL.revokeObjectURL(colorMatchPreviewUrl.value);
    } catch {
      /* ignore */
    }
    colorMatchPreviewUrl.value = '';
  }
}

function clearColorMatch() {
  colorMatchReferencePath.value = '';
  colorMatchError.value = '';
  colorMatchBusy.value = false;
  colorMatchPreviewRequestId += 1;
  if (colorMatchDebounceTimer) {
    clearTimeout(colorMatchDebounceTimer);
    colorMatchDebounceTimer = null;
  }
  revokeColorMatchPreviewUrl();
  invalidateHostPreviewClientCache();
  schedulePhotoStylePreview();
}

async function pickColorMatchReference() {
  const selected = await openDialog({
    multiple: false,
    filters: [{
      name: 'Images',
      extensions: ['jpg', 'jpeg', 'png', 'webp', 'tif', 'tiff', 'bmp', 'heic', 'heif', 'jxl'],
    }],
  });
  if (!selected) return;
  colorMatchReferencePath.value = String(selected);
  colorMatchError.value = '';
  scheduleColorMatchPreview();
}

function scheduleColorMatchPreview() {
  if (colorMatchDebounceTimer) {
    clearTimeout(colorMatchDebounceTimer);
    colorMatchDebounceTimer = null;
  }
  if (!colorMatchReferencePath.value || !fileInfo.value?.file_path) {
    revokeColorMatchPreviewUrl();
    return;
  }
  colorMatchDebounceTimer = setTimeout(() => {
    void runColorMatchPreview();
  }, 280);
}

async function runColorMatchPreview() {
  if (!colorMatchReferencePath.value || !fileInfo.value?.file_path) return;
  const style = currentWorkingStyle();
  // If host style fields are active, one combined host call keeps preview == save order.
  if (needsHostPreview(style) && !isPhotoStyleIdentity(style)) {
    await runPhotoStylePreview();
    return;
  }
  const requestId = ++colorMatchPreviewRequestId;
  colorMatchBusy.value = true;
  colorMatchError.value = '';
  try {
    const geometry = currentPreviewGeometry();
    const fingerprint = JSON.stringify({
      path: fileInfo.value.file_path,
      maxEdge: previewMaxEdge.value,
      hasMatch: true,
      ref: colorMatchReferencePath.value || '',
      intensity: colorMatchIntensity.value,
      tone: colorMatchTone.value,
      autoWb: colorMatchAutoWb.value,
      hi: colorMatchHighlight.value,
      sh: colorMatchShadow.value,
      style: null,
      geometry,
    });
    let bytes: Uint8Array | ArrayBuffer;
    if (lastHostPreviewFingerprint === fingerprint && lastHostPreviewBytes) {
      bytes = lastHostPreviewBytes;
    } else {
      bytes = await colorMatchPreview({
        sourceFilePath: fileInfo.value.file_path,
        referenceFilePath: colorMatchReferencePath.value,
        orientation: fileInfo.value.e_orientation || 1,
        maxEdge: previewMaxEdge.value,
        intensity: colorMatchIntensity.value / 100,
        tonePreservation: colorMatchTone.value / 100,
        autoWb: colorMatchAutoWb.value,
        highlightProtection: colorMatchHighlight.value / 100,
        shadowProtection: colorMatchShadow.value / 100,
        geometry,
      });
    }
    if (requestId !== colorMatchPreviewRequestId) return;
    const arr = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes as ArrayBuffer);
    lastHostPreviewFingerprint = fingerprint;
    lastHostPreviewBytes = arr;
    const blob = new Blob([arr], { type: 'image/jpeg' });
    const url = URL.createObjectURL(blob);
    revokeColorMatchPreviewUrl();
    colorMatchPreviewUrl.value = url;
    // Pure match preview supersedes style-only blob.
    revokePhotoStylePreviewUrl();
  } catch (error: any) {
    if (requestId !== colorMatchPreviewRequestId) return;
    colorMatchError.value = String(error?.message || error || localeMsg.value.msgbox.image_editor.color_match_failed);
    revokeColorMatchPreviewUrl();
  } finally {
    if (requestId === colorMatchPreviewRequestId) {
      colorMatchBusy.value = false;
    }
  }
}

async function exportColorMatchCube() {
  // Single-image style LUT: prefer the selected reference/style image; otherwise current photo.
  const stylePath = colorMatchReferencePath.value || fileInfo.value?.file_path || '';
  if (!stylePath) {
    colorMatchError.value = localeMsg.value.msgbox.image_editor.color_match_need_ref;
    return;
  }
  if (colorMatchExporting.value) return;

  const styleName = stylePath.replace(/\\/g, '/').split('/').pop() || 'style';
  const stem = String(styleName)
    .replace(/\.[^.]+$/, '')
    .replace(/[<>:"/\\|?*]+/g, '_')
    .trim() || 'style';
  const dest = await saveDialog({
    defaultPath: `${stem}_style_33.cube`,
    filters: [{ name: 'Cube LUT', extensions: ['cube'] }],
  });
  if (!dest) return;

  colorMatchExporting.value = true;
  colorMatchError.value = '';
  try {
    await exportColorMatchLut({
      sourceFilePath: stylePath,
      destFilePath: String(dest),
      lutSize: 33,
    });
  } catch (error: any) {
    colorMatchError.value = String(
      error?.message || error || localeMsg.value.msgbox.image_editor.color_match_export_failed,
    );
  } finally {
    colorMatchExporting.value = false;
  }
}

const executeSave = async (overrides: { fileName?: string; destFilePath?: string; outputFormat?: string } = {}) => {
  isProcessing.value = true;
  let success = false;
  const savedFilePath = overrides.destFilePath || fileInfo.value.file_path;
  const saveAsNew = savedFilePath !== fileInfo.value.file_path;
  try {
    success = await editImage(setEditParams(overrides));
  } finally {
    isProcessing.value = false;
    if (success) {
      uiStore.updateFileVersion(fileInfo.value.file_path);
      if (savedFilePath !== fileInfo.value.file_path) {
        uiStore.updateFileVersion(savedFilePath);
      }
      if (uiStore.activeAdjustments.filePath === fileInfo.value.file_path) {
        uiStore.clearActiveAdjustments();
      }
      sendToParent({ type: 'success', saveAsNew, filePath: savedFilePath });
    } else {
      sendToParent({ type: 'failed' });
    }
  }
};

const clickSave = async () => {
  if (cropStatus.value === 1 || isProcessing.value) return;

  if (effectiveSaveAsNew.value) {
    isProcessing.value = true;
    try {
      const folderPath = getFolderPath(fileInfo.value.file_path);
      const ext = getSelectedOutputFormat();
      const baseName = newFileName.value;

      let counter = 1;
      let candidateName = `${baseName}_${counter}`;
      let candidatePath = getFullPath(folderPath, combineFileName(candidateName, ext));

      while (await checkFileExists(candidatePath)) {
        counter++;
        candidateName = `${baseName}_${counter}`;
        candidatePath = getFullPath(folderPath, combineFileName(candidateName, ext));
      }

      await executeSave({
        fileName: candidateName,
        destFilePath: candidatePath,
      });
    } catch {
      isProcessing.value = false;
      sendToParent({ type: 'failed' });
    }
  } else {
    showOverwriteConfirm.value = true;
  }
};

</script>

<style scoped>
.crop-box-active {
  position: absolute;
  border: 1px solid #fff;
  box-shadow: 0 0 0 9999px color-mix(in srgb, var(--color-base-200) 80%, transparent);
  box-sizing: border-box;
  will-change: transform;
  transition: all 0.3s ease;
}

.crop-box-done {
  position: absolute;
  box-shadow: 0 0 0 9999px var(--color-base-200);
  box-sizing: border-box;
  will-change: transform;
}

.no-transition {
  transition: none !important;
}

.drag-handle {
  position: absolute;
  width: 10px;
  height: 10px;
  background-color: #fff;
  border: 1px solid #000;
  box-sizing: border-box;
}

.top-left {
  top: -5px;
  left: -5px;
  cursor: nwse-resize;
}

.top {
  top: -5px;
  left: 50%;
  transform: translateX(-50%);
  cursor: ns-resize;
}

.top-right {
  top: -5px;
  right: -5px;
  cursor: nesw-resize;
}

.left {
  top: 50%;
  left: -5px;
  transform: translateY(-50%);
  cursor: ew-resize;
}

.right {
  top: 50%;
  right: -5px;
  cursor: ew-resize;
}

.bottom-left {
  bottom: -5px;
  left: -5px;
  cursor: nesw-resize;
}

.bottom {
  bottom: -5px;
  left: 50%;
  transform: translateX(-50%);
  cursor: ns-resize;
}

.bottom-right {
  bottom: -5px;
  right: -5px;
  cursor: nwse-resize;
}

.grid-line-h,
.grid-line-v {
  position: absolute;
  background-color: rgba(255, 255, 255, 0.2);
}

.grid-line-h {
  width: 100%;
  height: 1px;
  left: 0;
}

.grid-line-v {
  width: 1px;
  height: 100%;
  top: 0;
}

.grid-line-h-1 {
  top: 33.33%;
}

.grid-line-h-2 {
  top: 66.66%;
}

.grid-line-v-1 {
  left: 33.33%;
}

.grid-line-v-2 {
  left: 66.66%;
}

.crop-dimensions-display {
  position: absolute;
  bottom: 10px;
  left: 50%;
  transform: translateX(-50%);
  background-color: rgba(0, 0, 0, 0.5);
  color: #aaa;
  padding: 2px 8px;
  border-radius: 14px;
  font-size: 12px;
  white-space: nowrap;
}
</style>
