import { reactive } from "vue";

export interface CaptureItem {
  path: string;
  version: number;
}

export function createQueue(max: number) {
  const items = reactive<CaptureItem[]>([]);

  function add(path: string) {
    const existing = items.findIndex((i) => i.path === path);
    if (existing !== -1) items.splice(existing, 1);
    items.unshift({ path, version: 0 });
    if (items.length > max) items.length = max;
  }

  function dismiss(path: string) {
    const i = items.findIndex((it) => it.path === path);
    if (i !== -1) items.splice(i, 1);
  }

  function touch(path: string) {
    const item = items.find((it) => it.path === path);
    if (item) item.version++;
  }

  function serialize(): CaptureItem[] {
    return items.map((i) => ({ ...i }));
  }

  function restore(data: unknown) {
    items.length = 0;
    if (!Array.isArray(data)) return;
    for (const entry of data.slice(0, max)) {
      const e = entry as { path?: unknown; version?: unknown };
      if (e && typeof e.path === "string" && typeof e.version === "number") {
        items.push({ path: e.path, version: e.version });
      }
    }
  }

  return { items, add, dismiss, touch, serialize, restore };
}
