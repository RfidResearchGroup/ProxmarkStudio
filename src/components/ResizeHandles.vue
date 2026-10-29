<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

type ResizeDirection =
  | 'East'
  | 'North'
  | 'NorthEast'
  | 'NorthWest'
  | 'South'
  | 'SouthEast'
  | 'SouthWest'
  | 'West'

const appWindow = getCurrentWindow()

const handles: Array<{ direction: ResizeDirection; class: string }> = [
  { direction: 'North', class: 'resize-handle--north' },
  { direction: 'South', class: 'resize-handle--south' },
  { direction: 'East', class: 'resize-handle--east' },
  { direction: 'West', class: 'resize-handle--west' },
  { direction: 'NorthEast', class: 'resize-handle--north-east' },
  { direction: 'NorthWest', class: 'resize-handle--north-west' },
  { direction: 'SouthEast', class: 'resize-handle--south-east' },
  { direction: 'SouthWest', class: 'resize-handle--south-west' },
]

function startResize(event: MouseEvent, direction: ResizeDirection) {
  event.preventDefault()
  void appWindow.startResizeDragging(direction)
}
</script>

<template>
  <div class="resize-layer" aria-hidden="true">
    <div
      v-for="handle in handles"
      :key="handle.direction"
      class="resize-handle"
      :class="handle.class"
      @mousedown="startResize($event, handle.direction)"
    />
  </div>
</template>

<style scoped>
.resize-layer {
  position: fixed;
  inset: 0;
  z-index: 80;
  pointer-events: none;
}

.resize-handle {
  position: absolute;
  pointer-events: auto;
  background: transparent;
}

.resize-handle--north,
.resize-handle--south {
  left: 8px;
  right: 8px;
  height: 4px;
  cursor: ns-resize;
}

.resize-handle--north {
  top: 0;
}
.resize-handle--south {
  bottom: 0;
}

.resize-handle--east,
.resize-handle--west {
  top: 8px;
  bottom: 8px;
  width: 4px;
  cursor: ew-resize;
}

.resize-handle--east {
  right: 0;
}
.resize-handle--west {
  left: 0;
}

.resize-handle--north-east,
.resize-handle--north-west,
.resize-handle--south-east,
.resize-handle--south-west {
  width: 10px;
  height: 10px;
}

.resize-handle--north-east {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}
.resize-handle--north-west {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}
.resize-handle--south-east {
  bottom: 0;
  right: 0;
  cursor: nwse-resize;
}
.resize-handle--south-west {
  bottom: 0;
  left: 0;
  cursor: nesw-resize;
}
</style>
