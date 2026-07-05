<template>
  <div
    class="group/map relative w-full h-full min-h-[300px] border border-base-content/30 rounded-box overflow-hidden"
    @mouseenter="uiStore.setMapActive(true)"
    @mouseleave="uiStore.setMapActive(false)"
  >
    <div ref="mapEl" style="width:100%; height:100%;"></div>
    <div class="absolute top-2 left-2 flex bg-base-100/30 hover:bg-base-100/70 rounded-box z-500 cursor-pointer opacity-0 pointer-events-none transition-opacity duration-150 group-hover/map:opacity-100 group-hover/map:pointer-events-auto">
      <TButton
        :icon="IconZoomOut"
        :tooltip="t('map.zoom_out')"
        :disabled="zoom <= 0"
        @click="zoomOut"
      />
      <TButton
        :icon="IconZoomIn"
        :tooltip="t('map.zoom_in')"
        :disabled="zoom >= 18"
        @click="zoomIn"
      />
      <TButton
        :icon="IconMapCenter"
        :tooltip="t('map.zoom_center')"
        @click="zoomCenter"
      />
      <TButton
        :icon="config.infoPanel.mapTheme === 0 ? IconMapDefault : IconMapSatellite"
        :tooltip="t(config.infoPanel.mapTheme === 0 ? 'map.standard' : 'map.satellite')"
        @click="toggleMap"
      />
      <TButton
        v-if="showAppleMapsButton"
        :icon="IconExternal"
        :tooltip="t('file_info.open_apple_maps')"
        @click="openAppleMaps"
      />
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { config } from '@/common/config'
import { openExternalUrl } from '@/common/api'
import { isMac } from '@/common/utils'
import { useUIStore } from '@/stores/uiStore'

import { IconZoomIn, IconZoomOut, IconMapCenter, IconMapDefault, IconMapSatellite, IconExternal } from '@/common/icons'
import TButton from '@/components/TButton.vue'

import L from 'leaflet'
import 'leaflet/dist/leaflet.css'

// default marker icon
import markerIcon2x from 'leaflet/dist/images/marker-icon-2x.png'
import markerIcon from 'leaflet/dist/images/marker-icon.png'
import markerShadow from 'leaflet/dist/images/marker-shadow.png'

// Props: lat/lon from parent; no geolocation fallback
const props = defineProps({
  lat: {
    type: Number, 
    required: false,
    default: 0,
  },
  lon: {
    type: Number, 
    required: false,
    default: 0,
  },
  zoom: {
    type: Number, 
    default: 13,
  },
  label: {
    type: String,
    default: 'Lap',
  },
})

const { t } = useI18n()
const uiStore = useUIStore()

const mapTheme = [
  {
    name: 'standard',
    url: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png',
    attribution: 'OpenStreetMap', // https://osmfoundation.org/wiki/Licence/Attribution_Guidelines
  },
  {
    name: 'satellite',
    url: 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}',
    attribution: 'Powered by Esri',
  },
]

const mapEl = ref(null)
let marker = null
let map = null
let layer = null
let tileErrorFallbackTriggered = false
let zoom = ref(props.zoom)
let resizeObserver = null
const showAppleMapsButton = computed(() => isMac && validLatLon(props.lat, props.lon))

onMounted(() => {
  map = L.map(mapEl.value, {
    center: [0, 0],
    zoom: 2,
    // attributionControl: false,
    keyboard: false,
    zoomControl: false,
    maxZoom: 19,
  })
  map.attributionControl.setPrefix('')

  map.on('zoomend', () => {
    zoom.value = map.getZoom()
  })

  // delete default marker icon
  delete L.Icon.Default.prototype._getIconUrl
  // set default marker icon
  L.Icon.Default.mergeOptions({
    iconRetinaUrl: markerIcon2x,
    iconUrl: markerIcon,
    shadowUrl: markerShadow
  })

  resizeObserver = new ResizeObserver(() => {
    if (map) {
      map.invalidateSize()
    }
  })
  resizeObserver.observe(mapEl.value.parentElement)

  updateTheme()
  updateFromProps()
  window.addEventListener('keydown', handleMapKeyDown, true)
})

onBeforeUnmount(() => {
  uiStore.setMapActive(false)
  window.removeEventListener('keydown', handleMapKeyDown, true)
  if (map) map.remove()
  if (resizeObserver) resizeObserver.disconnect()
})

watch(() => [props.lat, props.lon, props.zoom], () => {
  updateFromProps()
})

function updateFromProps() {
  if (!map) return
  if (validLatLon(props.lat, props.lon)) {
    if (marker) {
      marker.setLatLng([props.lat, props.lon])
    } else {
      marker = L.marker([props.lat, props.lon]).addTo(map)
    }
    map.setView([props.lat, props.lon], zoom.value)
  } else {
    // remove marker if any and show world view
    if (marker) {
      map.removeLayer(marker)
      marker = null
    }
    map.setView([0, 0], 2)
  }
}

// validate latitude and longitude
function validLatLon(lat, lon) {
  return lat !== null && lon !== null && lat >= -90 && lat <= 90 && lon >= -180 && lon <= 180
}

function zoomIn() {
  if (map) { 
    if (zoom.value < 18) {
      map.setZoom(zoom.value + 1)
    }
  }
}

function zoomOut() {
  if (map) {
    if (zoom.value > 0) {
      map.setZoom(zoom.value - 1)
    }
  }
}

function zoomCenter() {
  zoom.value = props.zoom
  updateFromProps()
}

function toggleMap() {
  config.infoPanel.mapTheme = config.infoPanel.mapTheme === 0 ? 1 : 0;
  updateTheme();
}

function updateTheme() {
  const theme = mapTheme[Number(config.infoPanel.mapTheme)] || mapTheme[0]
  if (map) {
    if (layer) {
      map.removeLayer(layer)
      layer = null
    }
    tileErrorFallbackTriggered = false
    layer = L.tileLayer(theme.url, { attribution: theme.attribution }).addTo(map)
    layer.on('tileerror', () => {
      if (tileErrorFallbackTriggered) return
      tileErrorFallbackTriggered = true

      // If satellite tiles fail (TLS/network/provider), fall back to standard tiles.
      if (Number(config.infoPanel.mapTheme) !== 0) {
        config.infoPanel.mapTheme = 0
        updateTheme()
      }
    })
  }
}

async function openAppleMaps() {
  if (!showAppleMapsButton.value) return
  const label = props.label?.trim() || 'Lap'
  const url = `maps://?ll=${props.lat},${props.lon}&q=${encodeURIComponent(label)}`
  await openExternalUrl(url)
}

function handleMapKeyDown(event) {
  const target = event.target
  if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.isContentEditable) {
    return
  }

  if (!uiStore.mapActive || event.metaKey || event.ctrlKey || event.altKey) {
    return
  }

  if (event.key === '=') {
    event.preventDefault()
    event.stopPropagation()
    zoomIn()
  } else if (event.key === '-') {
    event.preventDefault()
    event.stopPropagation()
    zoomOut()
  }
}

</script>
