<template>
  <!-- SELECT MODE: fullscreen veil + drag rectangle -->
  <div
    v-if="mode === 'select'"
    class="veil"
    @mousedown="onDown"
    @mousemove="onMove"
    @mouseup="onUp"
  >
    <template v-if="rect">
      <div class="shade" :style="{ left: '0', top: '0', width: '100%', height: `${rect.y}px` }" />
      <div class="shade" :style="{ left: '0', top: `${rect.y + rect.height}px`, width: '100%', bottom: '0' }" />
      <div class="shade" :style="{ left: '0', top: `${rect.y}px`, width: `${rect.x}px`, height: `${rect.height}px` }" />
      <div class="shade" :style="{ left: `${rect.x + rect.width}px`, top: `${rect.y}px`, right: '0', height: `${rect.height}px` }" />
      <div class="marquee" :style="{ left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.width}px`, height: `${rect.height}px` }">
        <span class="size-label">{{ rect.width }} × {{ rect.height }}</span>
      </div>
    </template>
    <div v-else class="shade full" />
    <div class="instructions">Drag to select ONLY the area that scrolls (avoid sidebars) — Esc to cancel</div>
  </div>

  <!-- PANEL MODE: floating control bar during capture -->
  <div v-else class="panel">
    <span class="rec-dot" />
    <span class="panel-text" v-if="frames === 0">Starting capture…</span>
    <span class="panel-text" v-else>Now scroll the content · {{ heightPx }}px captured · {{ frames }} frames</span>
    <button class="done" @click="done">Done</button>
    <button @click="cancel">Cancel</button>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Mode = "select" | "panel";
interface Rect { x: number; y: number; width: number; height: number }

const mode = ref<Mode>("select");
const rect = ref<Rect | null>(null);
const frames = ref(0);
const heightPx = ref(0);

let dragging = false;
let startX = 0;
let startY = 0;

function onDown(e: MouseEvent) {
  dragging = true;
  startX = e.clientX;
  startY = e.clientY;
  rect.value = { x: startX, y: startY, width: 0, height: 0 };
}

function onMove(e: MouseEvent) {
  if (!dragging) return;
  rect.value = {
    x: Math.min(startX, e.clientX),
    y: Math.min(startY, e.clientY),
    width: Math.abs(e.clientX - startX),
    height: Math.abs(e.clientY - startY),
  };
}

async function onUp() {
  if (!dragging) return;
  dragging = false;
  const r = rect.value;
  if (!r || r.width < 24 || r.height < 24) {
    rect.value = null;
    return;
  }
  const region = {
    x: Math.round(r.x),
    y: Math.round(r.y),
    width: Math.round(r.width),
    height: Math.round(r.height),
  };
  frames.value = 0;
  heightPx.value = 0;
  // ORDER MATTERS: shrink the veil into the panel BEFORE starting capture,
  // or the fullscreen veil is baked into the first frames.
  mode.value = "panel";
  await invoke("position_scroll_panel", { region });
  await invoke("scroll_capture_start", { region });
}

function done() {
  invoke("scroll_capture_stop");
}

function cancel() {
  invoke("scroll_capture_cancel");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (mode.value === "panel") cancel();
    else invoke("scroll_capture_cancel"); // also hides the window
  }
}

function reset() {
  mode.value = "select";
  rect.value = null;
  dragging = false;
  frames.value = 0;
  heightPx.value = 0;
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await listen("selector-reset", reset);
  await listen<{ frames: number; height: number }>("scroll-progress", (e) => {
    frames.value = e.payload.frames;
    heightPx.value = e.payload.height;
  });
  await listen<string>("scroll-capture-error", () => {
    invoke("scroll_capture_cancel");
  });
});

onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<style scoped>
.veil {
  position: fixed;
  inset: 0;
  cursor: crosshair;
  user-select: none;
}
.shade {
  position: absolute;
  background: rgba(0, 0, 0, 0.3);
}
.shade.full {
  inset: 0;
}
.marquee {
  position: absolute;
  border: 1.5px dashed #fff;
  box-sizing: border-box;
}
.size-label {
  position: absolute;
  right: 4px;
  bottom: 4px;
  font: 11px -apple-system, sans-serif;
  color: #fff;
  background: rgba(0, 0, 0, 0.6);
  padding: 2px 6px;
  border-radius: 4px;
}
.instructions {
  position: absolute;
  top: 72px;
  left: 50%;
  transform: translateX(-50%);
  font: 14px -apple-system, sans-serif;
  color: #fff;
  background: rgba(0, 0, 0, 0.6);
  padding: 8px 18px;
  border-radius: 8px;
  pointer-events: none;
}
.panel {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background: #2c2c2e;
  border-radius: 10px;
  font: 12px -apple-system, sans-serif;
  color: #fff;
}
.rec-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #ff453a;
  animation: pulse 1.2s infinite;
}
@keyframes pulse {
  50% { opacity: 0.3; }
}
.panel-text {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
}
.panel button {
  border: none;
  border-radius: 6px;
  padding: 4px 12px;
  font-size: 12px;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.16);
  color: #fff;
}
.panel .done {
  background: #30d158;
  color: #1c1c1e;
  font-weight: 600;
}
</style>

<style>
html, body, #app {
  margin: 0;
  background: transparent !important;
}
</style>
