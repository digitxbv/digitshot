<template>
  <!-- SELECT / CONFIRM MODE: fullscreen veil + drag rectangle -->
  <div
    v-if="mode === 'select' || mode === 'confirm'"
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
    <div class="instructions">
      <template v-if="mode === 'confirm'">Click Start Capture, then scroll through the content</template>
      <template v-else>Drag to select ONLY the area that scrolls (avoid sidebars) — Esc to cancel</template>
    </div>

    <!-- CONFIRM MODE: floating button bar -->
    <div v-if="mode === 'confirm'" class="confirm-bar" :style="confirmBarStyle" @mousedown.stop>
      <button class="primary" @click.stop="startCapture">Start Capture</button>
      <button @click.stop="resetToSelect">Reselect</button>
      <button @click.stop="cancelAll">Cancel</button>
    </div>
  </div>

  <!-- PANEL MODE: floating control bar during capture -->
  <div v-else class="panel" :class="{ warn: tooFast }">
    <span class="rec-dot" />
    <span class="panel-text" v-if="tooFast">Too fast — scroll back a little, then continue slowly</span>
    <span class="panel-text" v-else-if="frames === 0">Starting capture…</span>
    <span class="panel-text" v-else>Scroll slowly · {{ heightPx }}px · {{ frames }} frames</span>
    <button class="done" @click="done">Done</button>
    <button @click="cancel">Cancel</button>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Mode = "select" | "confirm" | "panel";
interface Rect { x: number; y: number; width: number; height: number }

const mode = ref<Mode>("select");
const rect = ref<Rect | null>(null);
const frames = ref(0);
const heightPx = ref(0);
const tooFast = ref(false);

let dragging = false;
let startX = 0;
let startY = 0;

function onDown(e: MouseEvent) {
  dragging = true;
  startX = e.clientX;
  startY = e.clientY;
  rect.value = { x: startX, y: startY, width: 0, height: 0 };
  if (mode.value === "confirm") {
    mode.value = "select";
  }
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

function onUp() {
  if (!dragging) return;
  dragging = false;
  const r = rect.value;
  if (!r || r.width < 24 || r.height < 24) {
    rect.value = null;
    return;
  }
  mode.value = "confirm";
}

async function startCapture() {
  const r = rect.value!;
  const region = {
    x: Math.round(r.x),
    y: Math.round(r.y),
    width: Math.round(r.width),
    height: Math.round(r.height),
  };
  frames.value = 0;
  heightPx.value = 0;
  tooFast.value = false;
  // Shrink the veil into the panel BEFORE starting capture so the veil is
  // never baked into the first frames.
  mode.value = "panel";
  await invoke("position_scroll_panel", { region });
  await invoke("scroll_capture_start", { region });
}

function resetToSelect() {
  rect.value = null;
  mode.value = "select";
}

function cancelAll() {
  invoke("scroll_capture_cancel");
  reset();
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
    else if (mode.value === "confirm") resetToSelect();
    else invoke("scroll_capture_cancel"); // select mode — also hides the window
  }
}

function reset() {
  mode.value = "select";
  rect.value = null;
  dragging = false;
  frames.value = 0;
  heightPx.value = 0;
  tooFast.value = false;
}

const confirmBarStyle = computed(() => {
  const r = rect.value;
  if (!r) return {};
  const barWidth = 320;
  const barHeight = 56;
  const centerX = r.x + r.width / 2;
  const rawLeft = centerX - barWidth / 2;
  const left = Math.max(8, Math.min(rawLeft, window.innerWidth - barWidth));
  const belowY = r.y + r.height + 8;
  const top = belowY + barHeight > window.innerHeight ? r.y - barHeight : belowY;
  return {
    left: `${left}px`,
    top: `${top}px`,
  };
});

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await listen("selector-reset", reset);
  await listen<{ frames: number; height: number }>("scroll-progress", (e) => {
    frames.value = e.payload.frames;
    heightPx.value = e.payload.height;
  });
  await listen<{ too_fast: boolean }>("scroll-status", (e) => {
    tooFast.value = e.payload.too_fast;
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
.confirm-bar {
  position: absolute;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: #2c2c2e;
  border-radius: 10px;
  font: 12px -apple-system, sans-serif;
  color: #fff;
  cursor: default;
}
.confirm-bar .primary {
  background: #30d158;
  color: #1c1c1e;
  font-weight: 600;
  border: none;
  border-radius: 6px;
  padding: 4px 12px;
  font-size: 12px;
  cursor: pointer;
}
.confirm-bar button:not(.primary) {
  border: none;
  border-radius: 6px;
  padding: 4px 12px;
  font-size: 12px;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.16);
  color: #fff;
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
.panel.warn {
  background: #5c3a00;
}
.rec-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #ff453a;
  animation: pulse 1.2s infinite;
}
.panel.warn .rec-dot {
  background: #ffd60a;
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
