<template>
  <div v-if="region" class="ring" :class="{ lost }" :style="{
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
const lost = ref(false);

onMounted(async () => {
  await listen<Region>("scroll-region", (e) => {
    region.value = e.payload;
    lost.value = false;
  });
  await listen<{ state: string }>("scroll-status", (e) => {
    lost.value = e.payload.state === "lost";
  });
});
</script>

<style scoped>
.ring {
  position: fixed;
  box-shadow: 0 0 0 3px #30d158; /* green: captured */
  border-radius: 2px;
  animation: ringpulse 1.6s ease-in-out infinite;
  pointer-events: none;
}
.ring.lost {
  box-shadow: 0 0 0 4px #ff453a; /* red: tracking lost */
  animation: none;
}
@keyframes ringpulse {
  50% { box-shadow: 0 0 0 3px rgba(48, 209, 88, 0.45); }
}
</style>

<style>
html, body, #app { margin: 0; background: transparent !important; }
</style>
