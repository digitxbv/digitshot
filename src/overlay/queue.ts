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

  return { items, add, dismiss, touch };
}
