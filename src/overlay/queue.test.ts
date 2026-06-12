import { describe, it, expect } from "vitest";
import { createQueue } from "./queue";

describe("capture queue", () => {
  it("prepends new captures, newest first", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    expect(q.items.map((i) => i.path)).toEqual(["/b.png", "/a.png"]);
  });

  it("caps visible items, dropping the oldest", () => {
    const q = createQueue(2);
    q.add("/a.png");
    q.add("/b.png");
    q.add("/c.png");
    expect(q.items.map((i) => i.path)).toEqual(["/c.png", "/b.png"]);
  });

  it("dismiss removes by path", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    q.dismiss("/a.png");
    expect(q.items.map((i) => i.path)).toEqual(["/b.png"]);
  });

  it("touch bumps version for cache-busting after edits", () => {
    const q = createQueue(5);
    q.add("/a.png");
    const before = q.items[0].version;
    q.touch("/a.png");
    expect(q.items[0].version).toBe(before + 1);
  });

  it("re-adding an existing path moves it to front instead of duplicating", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    q.add("/a.png");
    expect(q.items.map((i) => i.path)).toEqual(["/a.png", "/b.png"]);
  });
});

describe("queue persistence", () => {
  it("serialize round-trips through restore", () => {
    const q = createQueue(5);
    q.add("/a.png");
    q.add("/b.png");
    q.touch("/b.png");
    const data = q.serialize();
    const q2 = createQueue(5);
    q2.restore(data);
    expect(q2.items).toEqual(q.items.map((i) => ({ ...i })));
  });

  it("restore drops entries beyond max and tolerates garbage", () => {
    const q = createQueue(2);
    q.restore([
      { path: "/a.png", version: 1 },
      { path: "/b.png", version: 0 },
      { path: "/c.png", version: 0 },
    ]);
    expect(q.items.length).toBe(2);
    const q2 = createQueue(5);
    q2.restore("nonsense" as unknown as []);
    expect(q2.items.length).toBe(0);
  });
});
