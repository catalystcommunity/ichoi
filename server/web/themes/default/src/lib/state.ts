import BasicDataService from "@catalystcommunity/basic-data-service";

export type Accessor<T> = () => T;
export interface Observable { subscribe(listener: () => void): () => void }

/** Explicit state cells. Zest owns storage and change delivery. */
export class State implements Observable {
  private data = new BasicDataService<{ state: unknown }>();
  cell<T>(key: string, initial: T): [Accessor<T>, (next: T | ((current: T) => T)) => void] {
    this.data.set("state", key, initial);
    const get = () => this.data.get("state", key) as T;
    return [get, next => this.data.set("state", key,
      typeof next === "function" ? (next as (current: T) => T)(get()) : next)];
  }
  subscribe(listener: () => void): () => void { return this.data.subscribe(listener); }
}

/** Observe named inputs. Coalesce synchronous writes and ignore unchanged selections. */
export function observe<T extends readonly unknown[]>(sources: Observable[], select: () => T,
  changed: (value: T, previous: T | undefined) => void): () => void {
  let active = true, queued = false;
  let previous: T | undefined;
  const schedule = () => {
    if (queued || !active) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      if (!active) return;
      const next = select();
      if (previous && next.length === previous.length && next.every((v, i) => Object.is(v, previous![i]))) return;
      const old = previous;
      previous = next;
      changed(next, old);
    });
  };
  const removals = sources.map(source => source.subscribe(schedule));
  schedule();
  return () => { active = false; removals.forEach(remove => remove()); };
}
