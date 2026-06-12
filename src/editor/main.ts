import { createApp } from "vue";
import VueKonva from "vue-konva";
import EditorApp from "./EditorApp.vue";

createApp(EditorApp).use(VueKonva).mount("#app");
