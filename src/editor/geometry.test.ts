import { describe, it, expect } from "vitest";
import { normalizeRect, clampRect, fitScale, aspectResize } from "./geometry";

describe("normalizeRect", () => {
  it("handles drags in any direction", () => {
    expect(normalizeRect({ x: 10, y: 10 }, { x: 4, y: 2 })).toEqual({
      x: 4, y: 2, width: 6, height: 8,
    });
  });
});

describe("clampRect", () => {
  it("clips a rect to image bounds", () => {
    expect(clampRect({ x: -5, y: 10, width: 30, height: 200 }, 100, 100)).toEqual({
      x: 0, y: 10, width: 25, height: 90,
    });
  });
  it("returns null when fully outside or degenerate", () => {
    expect(clampRect({ x: 200, y: 0, width: 10, height: 10 }, 100, 100)).toBeNull();
    expect(clampRect({ x: 0, y: 0, width: 0, height: 5 }, 100, 100)).toBeNull();
  });
});

describe("fitScale", () => {
  it("scales down to fit, never up", () => {
    expect(fitScale(2000, 1000, 1000, 800)).toBe(0.5);
    expect(fitScale(400, 300, 1000, 800)).toBe(1);
  });
});

describe("aspectResize", () => {
  it("derives the other dimension from aspect ratio, rounded", () => {
    expect(aspectResize(1600, 900, { width: 800 })).toEqual({ width: 800, height: 450 });
    expect(aspectResize(1600, 900, { height: 450 })).toEqual({ width: 800, height: 450 });
  });
});
