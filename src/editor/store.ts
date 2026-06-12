import { reactive } from "vue";

export type Tool = "select" | "rect" | "blur" | "crop";

export interface RectShape {
  kind: "rect";
  id: string;
  x: number; y: number; width: number; height: number;
  stroke: string;
  strokeWidth: number;
}

export interface BlurShape {
  kind: "blur";
  id: string;
  // region in image pixel coordinates
  x: number; y: number; width: number; height: number;
  pixelSize: number;
}

export type Shape = RectShape | BlurShape;

export interface EditorSnapshot {
  /** data URL or asset URL of the base bitmap */
  baseSrc: string;
  baseWidth: number;
  baseHeight: number;
  shapes: Shape[];
}

export function cloneSnapshot(s: EditorSnapshot): EditorSnapshot {
  return JSON.parse(JSON.stringify(s));
}

let nextId = 1;
export function shapeId(): string {
  return `s${nextId++}`;
}

export function createEditorState() {
  return reactive({
    tool: "select" as Tool,
    filePath: "",
    snapshot: null as EditorSnapshot | null,
    selectedId: "" as string,
    stroke: "#ff3b30",
    strokeWidth: 4,
    error: "" as string,
  });
}
