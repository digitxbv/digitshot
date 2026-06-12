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
      <button :disabled="!state.snapshot" @click="openResize">Resize…</button>
      <button :disabled="!state.snapshot" @click="copyResult">Copy</button>
      <button :disabled="!state.snapshot" @click="saveResult">Save</button>
      <button :disabled="!state.snapshot" @click="saveAsResult">Save As…</button>
      <span v-if="flashMsg" class="flash" :style="{ color: flashMsg.startsWith('Failed') ? '#ff453a' : '#30d158' }">{{ flashMsg }}</span>
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
            <v-rect v-if="selectedBlur && !exporting" :config="{
              x: selectedBlur.x, y: selectedBlur.y,
              width: selectedBlur.width, height: selectedBlur.height,
              stroke: '#0a84ff', strokeWidth: 1.5, dash: [4, 3], listening: false }" />

            <!-- Crop draft: dim rects + dashed outline -->
            <template v-if="((drafting && state.tool === 'crop' && draftRect) || pendingCrop) && !exporting">
              <v-rect v-for="(dr, i) in cropDimRects" :key="i"
                :config="{ ...dr, fill: 'rgba(0,0,0,0.45)', listening: false }" />
              <v-rect v-if="draftRect"
                :config="{
                  x: draftRect.x, y: draftRect.y,
                  width: draftRect.width, height: draftRect.height,
                  stroke: '#ffffff', strokeWidth: 1.5, dash: [6, 4],
                  listening: false,
                }" />
            </template>
          </template>

          <v-transformer ref="transformerRef" :config="{ rotateEnabled: false }" />
        </v-layer>
      </v-stage>

      <!-- Crop confirm bar -->
      <div v-if="pendingCrop" class="crop-confirm-bar">
        <button @click="applyCrop">Apply Crop</button>
        <button @click="cancelCrop">Cancel</button>
      </div>
    </div>

    <!-- Resize modal -->
    <div v-if="showResize" class="modal-overlay" @mousedown.self="showResize = false">
      <div class="modal-panel">
        <h3>Resize Image</h3>
        <div class="modal-field">
          <label>Width (px)</label>
          <input type="number" :value="resizeW" @input="onResizeWInput" min="8" />
        </div>
        <div class="modal-field">
          <label>Height (px)</label>
          <input type="number" :value="resizeH" @input="onResizeHInput" min="8" />
        </div>
        <div class="modal-field modal-check">
          <label>
            <input type="checkbox" v-model="resizeLockAspect" />
            Lock aspect ratio
          </label>
        </div>
        <p v-if="resizeError" class="modal-error">{{ resizeError }}</p>
        <div class="modal-actions">
          <button @click="applyResize">Apply</button>
          <button @click="showResize = false">Cancel</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createEditorState, cloneSnapshot, shapeId, type EditorSnapshot, type Tool, type RectShape, type BlurShape } from "./store";
import { History } from "./history";
import { fitScale, normalizeRect, clampRect, aspectResize, type Point, type Rect } from "./geometry";
import BlurPatch from "./BlurPatch.vue";
import { flattenStage, cropCanvas, scaleCanvas } from "./flatten";

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

// Crop state
const pendingCrop = ref<Rect | null>(null);

// Hides transient overlays (crop dim/outline, blur selection) while the
// stage is flattened for export — they must never bake into the output.
const exporting = ref(false);

async function exportFlatten(): Promise<HTMLCanvasElement> {
  exporting.value = true;
  try {
    const stage = stageRef.value.getNode();
    const tr = transformerRef.value.getNode();
    // flattenStage waits a render cycle, which also flushes the overlay removal
    return await flattenStage(stage, tr, scale.value);
  } finally {
    exporting.value = false;
  }
}

// Resize state
const showResize = ref(false);
const resizeW = ref(0);
const resizeH = ref(0);
const resizeLockAspect = ref(true);
const resizeError = ref("");

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

// Crop dim rects: four strips covering area outside the draft rect (image coords)
const cropDimRects = computed<Array<{ x: number; y: number; width: number; height: number }>>(() => {
  const snap = state.snapshot;
  const dr = draftRect.value ?? pendingCrop.value;
  if (!snap || !dr) return [];
  const W = snap.baseWidth;
  const H = snap.baseHeight;
  const rects = [
    // top
    { x: 0, y: 0, width: W, height: dr.y },
    // bottom
    { x: 0, y: dr.y + dr.height, width: W, height: H - (dr.y + dr.height) },
    // left
    { x: 0, y: dr.y, width: dr.x, height: dr.height },
    // right
    { x: dr.x + dr.width, y: dr.y, width: W - (dr.x + dr.width), height: dr.height },
  ];
  return rects.filter((r) => r.width > 0 && r.height > 0);
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
  },
  // post flush: the Konva node for a freshly drawn shape must exist before findOne
  { flush: "post" }
);

// Clear pending crop and draft when switching away from crop tool
watch(
  () => state.tool,
  (newTool, oldTool) => {
    if (oldTool === "crop" && newTool !== "crop") {
      pendingCrop.value = null;
      resetDraft();
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
    const pt = pointerInImage();
    if (!pt) return;
    // Reset any existing pending crop before starting a new draft
    pendingCrop.value = null;
    dragStart.value = pt;
    dragCurrent.value = pt;
    drafting.value = true;
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

  if (state.tool === "crop") {
    const snap = state.snapshot;
    const dr = draftRect.value;
    if (!snap || !dr) {
      resetDraft();
      return;
    }
    const clamped = clampRect(dr, snap.baseWidth, snap.baseHeight);
    if (!clamped || clamped.width < 8 || clamped.height < 8) {
      resetDraft();
      pendingCrop.value = null;
      return;
    }
    pendingCrop.value = clamped;
    // Keep dragStart/dragCurrent so draftRect (and dim overlay) still renders;
    // just stop the "active drawing" state
    drafting.value = false;
    return;
  }

  resetDraft();
}

function cancelCrop() {
  pendingCrop.value = null;
  resetDraft();
}

async function applyCrop() {
  if (!pendingCrop.value) return;
  const region = pendingCrop.value;
  const canvas = await exportFlatten();
  const out = cropCanvas(canvas, region);
  pendingCrop.value = null;
  resetDraft();
  state.tool = "select";
  replaceBase(out);
}

function replaceBase(canvas: HTMLCanvasElement) {
  const dataUrl = canvas.toDataURL("image/png");
  state.snapshot!.baseSrc = dataUrl;
  state.snapshot!.baseWidth = canvas.width;
  state.snapshot!.baseHeight = canvas.height;
  state.snapshot!.shapes = []; // annotations are baked into the new base
  state.selectedId = "";
  commit();
}

function openResize() {
  if (!state.snapshot) return;
  resizeW.value = state.snapshot.baseWidth;
  resizeH.value = state.snapshot.baseHeight;
  resizeLockAspect.value = true;
  resizeError.value = "";
  showResize.value = true;
}

function onResizeWInput(e: Event) {
  const val = parseInt((e.target as HTMLInputElement).value, 10);
  if (isNaN(val)) return;
  resizeW.value = val;
  if (resizeLockAspect.value && state.snapshot) {
    resizeH.value = aspectResize(state.snapshot.baseWidth, state.snapshot.baseHeight, { width: val }).height;
  }
}

function onResizeHInput(e: Event) {
  const val = parseInt((e.target as HTMLInputElement).value, 10);
  if (isNaN(val)) return;
  resizeH.value = val;
  if (resizeLockAspect.value && state.snapshot) {
    resizeW.value = aspectResize(state.snapshot.baseWidth, state.snapshot.baseHeight, { height: val }).width;
  }
}

async function applyResize() {
  const snap = state.snapshot;
  if (!snap) return;
  const origW = snap.baseWidth;
  const origH = snap.baseHeight;
  const w = Math.round(resizeW.value);
  const h = Math.round(resizeH.value);
  if (!Number.isInteger(w) || !Number.isInteger(h) || w < 8 || h < 8 || w > origW * 4 || h > origH * 4) {
    resizeError.value = "Dimensions must be between 8px and 4× the original size.";
    return;
  }
  resizeError.value = "";
  const canvas = await exportFlatten();
  showResize.value = false;
  replaceBase(scaleCanvas(canvas, w, h));
}

function closeWindow() {
  getCurrentWindow().close();
}

// Flash toast
const flashMsg = ref("");
let flashTimer: ReturnType<typeof setTimeout> | null = null;
function flash(msg: string) {
  if (flashTimer !== null) clearTimeout(flashTimer);
  flashMsg.value = msg;
  flashTimer = setTimeout(() => { flashMsg.value = ""; flashTimer = null; }, 1500);
}

async function exportPngBase64(): Promise<string> {
  const canvas = await exportFlatten();
  return canvas.toDataURL("image/png").split(",")[1];
}

async function copyResult() {
  try {
    await invoke("copy_image_data", { base64Png: await exportPngBase64() });
    flash("Copied");
  } catch (e) {
    flash("Failed: " + String(e));
  }
}

async function saveResult() {
  try {
    await invoke("save_png", { path: state.filePath, base64Png: await exportPngBase64() });
    flash("Saved");
  } catch (e) {
    flash("Failed: " + String(e));
  }
}

async function saveAsResult() {
  try {
    const target = await save({
      defaultPath: state.filePath.replace(/\.png$/, " edited.png"),
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (!target) return;
    await invoke("save_png", { path: target, base64Png: await exportPngBase64() });
    flash("Saved");
  } catch (e) {
    flash("Failed: " + String(e));
  }
}

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
    if (pendingCrop.value) {
      cancelCrop();
    } else if (drafting.value) {
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
.flash {
  font-size: 12px;
}
.viewport {
  flex: 1;
  background: #1c1c1e;
  display: grid;
  place-items: center;
  overflow: auto;
  position: relative;
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

/* Crop confirm bar */
.crop-confirm-bar {
  position: absolute;
  bottom: 16px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 8px;
  background: #2c2c2e;
  border-radius: 8px;
  padding: 8px 12px;
  z-index: 10;
}
.crop-confirm-bar button {
  background: rgba(255, 255, 255, 0.12);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
}
.crop-confirm-bar button:hover {
  background: rgba(255, 255, 255, 0.22);
}

/* Resize modal */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal-panel {
  background: #2c2c2e;
  border-radius: 12px;
  padding: 20px;
  width: 300px;
  color: white;
}
.modal-panel h3 {
  margin: 0 0 16px;
  font-size: 15px;
  font-weight: 600;
}
.modal-field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
  font-size: 13px;
}
.modal-field label {
  color: rgba(255, 255, 255, 0.75);
}
.modal-field input[type="number"] {
  background: rgba(255, 255, 255, 0.1);
  color: white;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  padding: 4px 8px;
  width: 90px;
  font-size: 13px;
}
.modal-check {
  justify-content: flex-start;
  gap: 8px;
}
.modal-check label {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.modal-error {
  color: #ff3b30;
  font-size: 12px;
  margin: 0 0 12px;
}
.modal-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.modal-actions button {
  background: rgba(255, 255, 255, 0.12);
  color: white;
  border: none;
  border-radius: 6px;
  padding: 6px 14px;
  font-size: 13px;
  cursor: pointer;
}
.modal-actions button:hover {
  background: rgba(255, 255, 255, 0.22);
}
</style>

<style>
html, body, #app {
  margin: 0;
  background: #1c1c1e;
}
</style>
