<template>
  <div class="editor">
    <div class="toolbar">
      <button v-for="t in tools" :key="t.id"
        :class="{ active: state.tool === t.id }"
        @click="state.tool = t.id">{{ t.label }}</button>
      <span class="sep" />
      <button v-for="c in colors" :key="c" class="swatch"
        :style="{ background: c }"
        :class="{ active: state.stroke === c }"
        @click="state.stroke = c" />
      <select v-model.number="state.strokeWidth">
        <option :value="2">2 px</option><option :value="4">4 px</option><option :value="6">6 px</option>
      </select>
      <span class="sep" />
      <button :disabled="!canUndo" @click="undo">Undo</button>
      <button :disabled="!canRedo" @click="redo">Redo</button>
      <span class="spacer" />
      <button @click="openResize">Resize…</button>
      <button @click="copyResult">Copy</button>
      <button @click="saveResult">Save</button>
      <button @click="saveAsResult">Save As…</button>
    </div>
    <div ref="viewport" class="viewport">
      <div v-if="state.error" class="error">
        <p>{{ state.error }}</p>
        <button @click="closeWindow">Close</button>
      </div>
      <v-stage v-else-if="state.snapshot" ref="stageRef" :config="stageConfig"
        @mousedown="onMouseDown" @mousemove="onMouseMove" @mouseup="onMouseUp">
        <v-layer>
          <v-image v-if="baseImageEl" :config="{ image: baseImageEl, x: 0, y: 0 }" />
        </v-layer>
        <v-layer ref="shapesLayerRef">
          <template v-if="baseImageEl">
            <!-- Blur patches (below committed rects) -->
            <BlurPatch v-for="s in blurShapes" :key="s.id" :shape="s" :image="baseImageEl" />

            <!-- Live blur draft -->
            <v-rect v-if="drafting && state.tool === 'blur' && draftRect"
              :config="{
                x: draftRect.x, y: draftRect.y,
                width: draftRect.width, height: draftRect.height,
                fill: 'rgba(120,120,128,0.35)',
                listening: false,
              }" />

            <!-- Committed rects -->
            <v-rect v-for="s in rectShapes" :key="s.id"
              :config="{
                id: s.id,
                x: s.x, y: s.y,
                width: s.width, height: s.height,
                stroke: s.stroke, strokeWidth: s.strokeWidth,
                draggable: state.tool === 'select',
                strokeScaleEnabled: true,
              }"
              @dragend="onShapeDragEnd(s, $event)"
              @transformend="onShapeTransformEnd(s, $event)" />

            <!-- Live rect draft -->
            <v-rect v-if="drafting && state.tool === 'rect' && draftRect"
              :config="{
                x: draftRect.x, y: draftRect.y,
                width: draftRect.width, height: draftRect.height,
                stroke: state.stroke, strokeWidth: state.strokeWidth,
                dash: [6, 4],
                listening: false,
              }" />

            <!-- Blur selection outline (above everything except transformer) -->
            <v-rect v-if="selectedBlur" :config="{
              x: selectedBlur.x, y: selectedBlur.y,
              width: selectedBlur.width, height: selectedBlur.height,
              stroke: '#0a84ff', strokeWidth: 1.5, dash: [4, 3], listening: false }" />
          </template>

          <v-transformer ref="transformerRef" :config="{ rotateEnabled: false }" />
        </v-layer>
      </v-stage>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createEditorState, cloneSnapshot, shapeId, type EditorSnapshot, type Tool, type RectShape, type BlurShape } from "./store";
import { History } from "./history";
import { fitScale, normalizeRect, clampRect, type Point } from "./geometry";
import BlurPatch from "./BlurPatch.vue";

const state = createEditorState();
let history: History<EditorSnapshot> | null = null;

const tools: { id: Tool; label: string }[] = [
  { id: "select", label: "Select" },
  { id: "rect", label: "Rectangle" },
  { id: "blur", label: "Blur" },
  { id: "crop", label: "Crop" },
];

const colors = ["#ff3b30", "#0a84ff", "#30d158", "#ffd60a", "#000000"];

const baseImageEl = ref<HTMLImageElement | null>(null);
const scale = ref(1);
const viewport = ref<HTMLDivElement | undefined>(undefined);
const stageRef = ref<any>();
const shapesLayerRef = ref<any>();
const transformerRef = ref<any>();
const historyVersion = ref(0);

// Drafting state
const drafting = ref(false);
const dragStart = ref<Point | null>(null);
const dragCurrent = ref<Point | null>(null);

const draftRect = computed(() =>
  dragStart.value && dragCurrent.value
    ? normalizeRect(dragStart.value, dragCurrent.value)
    : null
);

const rectShapes = computed<RectShape[]>(() =>
  (state.snapshot?.shapes ?? []).filter((s): s is RectShape => s.kind === "rect")
);

const blurShapes = computed<BlurShape[]>(() =>
  (state.snapshot?.shapes ?? []).filter((s): s is BlurShape => s.kind === "blur")
);

const selectedBlur = computed<BlurShape | null>(() => {
  if (!state.selectedId) return null;
  const shape = state.snapshot?.shapes.find((s) => s.id === state.selectedId);
  return shape?.kind === "blur" ? shape : null;
});

const stageConfig = computed(() => {
  const snap = state.snapshot;
  if (!snap) return { width: 0, height: 0, scaleX: 1, scaleY: 1 };
  return {
    width: snap.baseWidth * scale.value,
    height: snap.baseHeight * scale.value,
    scaleX: scale.value,
    scaleY: scale.value,
  };
});

const canUndo = computed(() => {
  void historyVersion.value;
  return history?.canUndo ?? false;
});

const canRedo = computed(() => {
  void historyVersion.value;
  return history?.canRedo ?? false;
});

// Used in Task 8 — keep declaration here so downstream tasks can reference it
function pointerInImage(): Point | null {
  const stage = stageRef.value?.getNode();
  const pos = stage?.getPointerPosition();
  return pos ? { x: pos.x / scale.value, y: pos.y / scale.value } : null;
}

function commit() {
  history!.push(cloneSnapshot(state.snapshot!));
  historyVersion.value++;
}

function undo() {
  if (history && history.canUndo) {
    state.snapshot = cloneSnapshot(history.undo());
    state.selectedId = "";
    historyVersion.value++;
  }
}

function redo() {
  if (history && history.canRedo) {
    state.snapshot = cloneSnapshot(history.redo());
    state.selectedId = "";
    historyVersion.value++;
  }
}

function fitToViewport() {
  const snap = state.snapshot;
  if (!snap || !viewport.value) return;
  const maxW = viewport.value.clientWidth - 32;
  const maxH = viewport.value.clientHeight - 32;
  scale.value = fitScale(snap.baseWidth, snap.baseHeight, maxW, maxH);
}

async function loadFile(path: string) {
  state.filePath = path;
  state.error = "";
  const assetUrl = convertFileSrc(path);
  const img = new Image();
  img.onload = () => {
    state.snapshot = {
      baseSrc: assetUrl,
      baseWidth: img.naturalWidth,
      baseHeight: img.naturalHeight,
      shapes: [],
    };
    baseImageEl.value = img;
    history = new History(cloneSnapshot(state.snapshot));
    historyVersion.value = 0;
    state.selectedId = "";
    fitToViewport();
  };
  img.onerror = () => {
    state.error = "Could not load capture — the file may have been moved or deleted.";
  };
  img.src = assetUrl;
}

// Watcher for baseSrc changes (Task 9 — undo/redo with different base images)
watch(
  () => state.snapshot?.baseSrc,
  (newSrc) => {
    if (!newSrc || newSrc === baseImageEl.value?.src) return;
    const img = new Image();
    img.onload = () => {
      baseImageEl.value = img;
      fitToViewport();
    };
    img.src = newSrc;
  }
);

// Transformer watcher
watch(
  () => state.selectedId,
  () => {
    const tr = transformerRef.value?.getNode();
    if (!tr) return;
    const stage = stageRef.value?.getNode();
    if (!stage) {
      tr.nodes([]);
      return;
    }
    const shape = state.snapshot?.shapes.find((s) => s.id === state.selectedId);
    if (shape && shape.kind === "rect") {
      const node = stage.findOne("#" + state.selectedId);
      tr.nodes(node ? [node] : []);
    } else {
      tr.nodes([]);
    }
  }
);

function resetDraft() {
  drafting.value = false;
  dragStart.value = null;
  dragCurrent.value = null;
}

function onShapeDragEnd(s: RectShape, e: any) {
  s.x = e.target.x();
  s.y = e.target.y();
  commit();
}

function onShapeTransformEnd(s: RectShape, e: any) {
  const node = e.target;
  s.x = node.x();
  s.y = node.y();
  s.width = Math.max(4, node.width() * node.scaleX());
  s.height = Math.max(4, node.height() * node.scaleY());
  node.scaleX(1);
  node.scaleY(1);
  commit();
}

function onMouseDown(e: any) {
  // Drawing tools — handle before select logic
  if (state.tool === "rect" || state.tool === "blur") {
    const pt = pointerInImage();
    if (!pt) return;
    dragStart.value = pt;
    dragCurrent.value = pt;
    drafting.value = true;
    return;
  }

  if (state.tool === "crop") {
    // Task 9
    return;
  }

  // Select tool logic
  if (state.tool !== "select") return;
  const target = e.target;
  const stage = stageRef.value?.getNode();
  // Transformer anchors must not change the selection
  if (target.getParent()?.className === "Transformer") return;
  const id = target !== stage ? target.id() : "";
  if (id && state.snapshot?.shapes.some((s) => s.id === id)) {
    state.selectedId = id;
  } else {
    // stage, base image, or anything unselectable -> deselect
    state.selectedId = "";
  }
}

function onMouseMove(_e: any) {
  if (drafting.value) {
    const pt = pointerInImage();
    if (pt) dragCurrent.value = pt;
  }
}

function onMouseUp(_e: any) {
  if (!drafting.value) return;

  if (state.tool === "rect") {
    const dr = draftRect.value;
    if (!dr || dr.width < 4 || dr.height < 4) {
      resetDraft();
      return;
    }
    const id = shapeId();
    state.snapshot!.shapes.push({
      kind: "rect",
      id,
      ...dr,
      stroke: state.stroke,
      strokeWidth: state.strokeWidth,
    });
    commit();
    state.tool = "select";
    state.selectedId = id;
    resetDraft();
    return;
  }

  if (state.tool === "blur") {
    const dr = draftRect.value;
    if (!dr) {
      resetDraft();
      return;
    }
    const clamped = clampRect(dr, state.snapshot!.baseWidth, state.snapshot!.baseHeight);
    if (!clamped || clamped.width < 4 || clamped.height < 4) {
      resetDraft();
      return;
    }
    state.snapshot!.shapes.push({
      kind: "blur",
      id: shapeId(),
      ...clamped,
      pixelSize: Math.max(8, Math.round(Math.min(clamped.width, clamped.height) / 12)),
    });
    commit();
    state.tool = "select";
    resetDraft();
    return;
  }

  resetDraft();
}

function closeWindow() {
  getCurrentWindow().close();
}

function openResize() { console.warn("not implemented"); }
function copyResult() { console.warn("not implemented"); }
function saveResult() { console.warn("not implemented"); }
function saveAsResult() { console.warn("not implemented"); }

function handleKeyDown(e: KeyboardEvent) {
  if (e.metaKey && !e.shiftKey && e.key.toLowerCase() === "z") {
    e.preventDefault();
    undo();
  } else if (e.metaKey && e.shiftKey && e.key.toLowerCase() === "z") {
    e.preventDefault();
    redo();
  } else if (e.key === "Delete" || e.key === "Backspace") {
    if (state.selectedId && state.snapshot) {
      const idx = state.snapshot.shapes.findIndex((s) => s.id === state.selectedId);
      if (idx !== -1) state.snapshot.shapes.splice(idx, 1);
      state.selectedId = "";
      commit();
    }
  } else if (e.key === "Escape") {
    if (drafting.value) {
      resetDraft();
    } else if (state.selectedId) {
      state.selectedId = "";
    } else {
      closeWindow();
    }
  }
}

onMounted(() => {
  const pathParam = new URLSearchParams(location.search).get("path");
  if (pathParam) loadFile(pathParam);

  getCurrentWebviewWindow().listen<{ path: string }>("editor-load", (e) => {
    loadFile(e.payload.path);
  });

  window.addEventListener("resize", fitToViewport);
  window.addEventListener("keydown", handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener("resize", fitToViewport);
  window.removeEventListener("keydown", handleKeyDown);
});

// Expose for Task 8 drawing tools
defineExpose({ pointerInImage, commit, state, scale });
</script>

<style scoped>
.editor {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
}
.toolbar {
  background: #2c2c2e;
  height: 44px;
  display: flex;
  flex-direction: row;
  gap: 6px;
  padding: 0 10px;
  align-items: center;
  flex-shrink: 0;
}
.toolbar button {
  background: rgba(255, 255, 255, 0.12);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
}
.toolbar button.active {
  background: rgba(255, 255, 255, 0.35);
}
.toolbar button:disabled {
  opacity: 0.4;
  cursor: default;
}
.swatch {
  width: 18px !important;
  height: 18px !important;
  padding: 0 !important;
  border-radius: 50% !important;
  min-width: 18px;
}
.swatch.active {
  outline: 2px solid white;
  outline-offset: 1px;
}
.toolbar select {
  background: rgba(255, 255, 255, 0.12);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 4px 6px;
  font-size: 12px;
  cursor: pointer;
}
.sep {
  display: inline-block;
  width: 1px;
  height: 20px;
  background: rgba(255, 255, 255, 0.2);
}
.spacer {
  flex: 1;
}
.viewport {
  flex: 1;
  background: #1c1c1e;
  display: grid;
  place-items: center;
  overflow: auto;
}
.error {
  color: white;
  text-align: center;
}
.error button {
  background: rgba(255, 255, 255, 0.12);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
}
</style>

<style>
html, body, #app {
  margin: 0;
  background: #1c1c1e;
}
</style>
