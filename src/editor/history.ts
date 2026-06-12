export class History<T> {
  private past: T[] = [];
  private future: T[] = [];
  private present: T;

  constructor(initial: T) {
    this.present = initial;
  }

  get current(): T {
    return this.present;
  }
  get canUndo(): boolean {
    return this.past.length > 0;
  }
  get canRedo(): boolean {
    return this.future.length > 0;
  }

  push(next: T): void {
    this.past.push(this.present);
    this.present = next;
    this.future = [];
  }

  undo(): T {
    const prev = this.past.pop();
    if (prev !== undefined) {
      this.future.push(this.present);
      this.present = prev;
    }
    return this.present;
  }

  redo(): T {
    const next = this.future.pop();
    if (next !== undefined) {
      this.past.push(this.present);
      this.present = next;
    }
    return this.present;
  }
}
