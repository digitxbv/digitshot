import { describe, it, expect } from "vitest";
import { History } from "./history";

describe("History", () => {
  it("undo returns previous state, redo returns it back", () => {
    const h = new History<string>("a");
    h.push("b");
    h.push("c");
    expect(h.undo()).toBe("b");
    expect(h.undo()).toBe("a");
    expect(h.redo()).toBe("b");
    expect(h.current).toBe("b");
  });

  it("push clears the redo stack", () => {
    const h = new History<string>("a");
    h.push("b");
    h.undo();
    h.push("c");
    expect(h.canRedo).toBe(false);
    expect(h.current).toBe("c");
  });

  it("undo/redo at the boundaries are no-ops", () => {
    const h = new History<string>("a");
    expect(h.canUndo).toBe(false);
    expect(h.undo()).toBe("a");
    expect(h.redo()).toBe("a");
  });
});
