<template>
  <div class="stack">
    <div
      v-for="item in displayItems"
      :key="item.path"
      class="card"
      :class="{ dev: isDev }"
      @click="edit(item)"
    >
      <img :src="src(item)" alt="" draggable="false" />
      <div class="actions" @click.stop>
        <button title="Edit" @click="edit(item)">Edit</button>
        <button title="Copy" @click="copy(item)">Copy</button>
        <button title="Show in Finder" @click="reveal(item)">Finder</button>
        <button title="Dismiss" class="dismiss" @click="dismiss(item)">✕</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, watch } from "vue";

const isDev = import.meta.env.DEV;
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createQueue, type CaptureItem } from "./queue";

const CARD_W = 224;
const CARD_H = 140;
const GAP = 10;
const PAD = 8; // room for shadows

const queue = createQueue(5);
const displayItems = computed(() => [...queue.items].reverse());

function src(item: CaptureItem) {
  return convertFileSrc(item.path) + "?v=" + item.version;
}

async function syncWindow() {
  await nextTick();
  const n = queue.items.length;
  if (n === 0) {
    await invoke("hide_overlay");
    return;
  }
  const height = n * CARD_H + (n - 1) * GAP + PAD * 2;
  await invoke("resize_overlay", { width: CARD_W + PAD * 2, height });
  await invoke("show_overlay");
}

watch(() => queue.items.length, syncWindow);

function edit(item: CaptureItem) {
  invoke("open_editor", { path: item.path });
}
function copy(item: CaptureItem) {
  invoke("copy_image", { path: item.path });
}
function reveal(item: CaptureItem) {
  invoke("reveal_in_finder", { path: item.path });
}
function dismiss(item: CaptureItem) {
  queue.dismiss(item.path);
}

onMounted(async () => {
  await listen<{ path: string }>("capture-taken", (e) => {
    queue.add(e.payload.path);
    syncWindow();
  });
  await listen<{ path: string }>("capture-updated", (e) => {
    queue.touch(e.payload.path);
  });
});
</script>

<style scoped>
.stack {
  position: fixed;
  inset: 0;
  padding: 8px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 10px;
}
.card {
  position: relative;
  width: 224px;
  height: 140px;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 4px 18px rgba(0, 0, 0, 0.45);
  background: #1c1c1e;
  cursor: pointer;
  border: 1px solid rgba(255, 255, 255, 0.18);
}
.card img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.actions {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  gap: 4px;
  padding: 6px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.75));
  opacity: 0;
  transition: opacity 120ms ease;
}
.card:hover .actions {
  opacity: 1;
}
.actions button {
  flex: 1;
  font-size: 11px;
  padding: 4px 0;
  border: none;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.16);
  color: #fff;
  cursor: pointer;
}
.actions button:hover {
  background: rgba(255, 255, 255, 0.3);
}
.actions .dismiss {
  flex: 0 0 28px;
}
.card.dev {
  border-color: rgba(255, 159, 10, 0.85);
}
</style>
