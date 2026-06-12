import type Konva from "konva";

/**
 * Flattens the stage to a canvas at FULL image resolution.
 * The stage is displayed at `scale`; pixelRatio 1/scale recovers 1:1 pixels.
 * Detaches the transformer during the snapshot so handles never bake in.
 */
export async function flattenStage(
  stage: Konva.Stage,
  transformer: Konva.Transformer,
  scale: number,
): Promise<HTMLCanvasElement> {
  const prevNodes = transformer.nodes();
  transformer.nodes([]);
  await new Promise((r) => setTimeout(r, 50)); // one render cycle
  const canvas = stage.toCanvas({ pixelRatio: 1 / scale });
  transformer.nodes(prevNodes);
  return canvas;
}

export function cropCanvas(
  src: HTMLCanvasElement,
  r: { x: number; y: number; width: number; height: number },
): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = Math.round(r.width);
  out.height = Math.round(r.height);
  out.getContext("2d")!.drawImage(src, r.x, r.y, r.width, r.height, 0, 0, out.width, out.height);
  return out;
}

export function scaleCanvas(src: HTMLCanvasElement, w: number, h: number): HTMLCanvasElement {
  const out = document.createElement("canvas");
  out.width = w;
  out.height = h;
  const ctx = out.getContext("2d")!;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(src, 0, 0, w, h);
  return out;
}
