export interface Point { x: number; y: number }
export interface Rect { x: number; y: number; width: number; height: number }

export function normalizeRect(a: Point, b: Point): Rect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(b.x - a.x),
    height: Math.abs(b.y - a.y),
  };
}

export function clampRect(r: Rect, imgW: number, imgH: number): Rect | null {
  const x = Math.max(0, r.x);
  const y = Math.max(0, r.y);
  const width = Math.min(r.x + r.width, imgW) - x;
  const height = Math.min(r.y + r.height, imgH) - y;
  if (width <= 0 || height <= 0) return null;
  return { x, y, width, height };
}

export function fitScale(imgW: number, imgH: number, maxW: number, maxH: number): number {
  return Math.min(1, maxW / imgW, maxH / imgH);
}

export function aspectResize(
  origW: number,
  origH: number,
  target: { width?: number; height?: number },
): { width: number; height: number } {
  if (target.width !== undefined) {
    return { width: target.width, height: Math.round((target.width * origH) / origW) };
  }
  if (target.height !== undefined) {
    return { width: Math.round((target.height * origW) / origH), height: target.height };
  }
  return { width: origW, height: origH };
}
