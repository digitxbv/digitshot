<template>
  <div v-if="region" class="ring" :style="{
    left: `${region.x}px`,
    top: `${region.y}px`,
    width: `${region.width}px`,
    height: `${region.height}px`,
  }" />
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";

interface Region { x: number; y: number; width: number; height: number }
const region = ref<Region | null>(null);

onMounted(async () => {
  await listen<Region>("scroll-region", (e) => {
    region.value = e.payload;
  });
});
</script>

<style scoped>
.ring {
  position: fixed;
  /* Border ring lives OUTSIDE the region via box-shadow so it is never
     captured into the frames. */
  box-shadow: 0 0 0 3px #ff453a;
  border-radius: 2px;
  animation: ringpulse 1.6s ease-in-out infinite;
  pointer-events: none;
}
@keyframes ringpulse {
  50% { box-shadow: 0 0 0 3px rgba(255, 69, 58, 0.45); }
}
</style>

<style>
html, body, #app { margin: 0; background: transparent !important; }
</style>
