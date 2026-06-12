<template>
  <v-image ref="nodeRef" :config="config" />
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch, nextTick } from "vue";
import Konva from "konva";
import type { BlurShape } from "./store";

const props = defineProps<{ shape: BlurShape; image: HTMLImageElement }>();
const nodeRef = ref();

const config = computed(() => ({
  id: props.shape.id,
  x: props.shape.x,
  y: props.shape.y,
  width: props.shape.width,
  height: props.shape.height,
  image: props.image,
  crop: {
    x: props.shape.x,
    y: props.shape.y,
    width: props.shape.width,
    height: props.shape.height,
  },
  filters: [Konva.Filters.Pixelate],
  pixelSize: props.shape.pixelSize,
  listening: true,
  draggable: false,
}));

function recache() {
  const node: Konva.Image = nodeRef.value?.getNode();
  if (node) {
    node.cache();
    node.getLayer()?.batchDraw();
  }
}

onMounted(async () => {
  await nextTick();
  recache();
});
watch(config, async () => {
  await nextTick();
  recache();
});
</script>
