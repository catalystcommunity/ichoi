export interface SubscriptionOptions {
  signal?: AbortSignal;
}

interface Listener<T> {
  handler: (event: T) => void;
  signal?: AbortSignal;
  abort?: () => void;
}

export interface EventRouterStats {
  eventTypes: number;
  listeners: number;
}

/** A typed, in-memory event router. It does not use DOM events, so wire and data events do not
 * accidentally cross document or component boundaries. */
export class EventRouter<Events extends object> {
  private readonly buckets = new Map<keyof Events, Set<Listener<unknown>>>();

  on<K extends keyof Events>(
    type: K,
    handler: (event: Events[K]) => void,
    options: SubscriptionOptions = {},
  ): () => void {
    if (options.signal?.aborted) return () => undefined;
    let bucket = this.buckets.get(type);
    if (!bucket) {
      bucket = new Set();
      this.buckets.set(type, bucket);
    }
    const listener: Listener<Events[K]> = { handler, signal: options.signal };
    bucket.add(listener as Listener<unknown>);
    let active = true;
    const off = () => {
      if (!active) return;
      active = false;
      const current = this.buckets.get(type);
      current?.delete(listener as Listener<unknown>);
      if (current?.size === 0) this.buckets.delete(type);
      if (listener.signal && listener.abort) {
        listener.signal.removeEventListener("abort", listener.abort);
      }
    };
    if (options.signal) {
      listener.abort = off;
      options.signal.addEventListener("abort", off, { once: true });
    }
    return off;
  }

  emit<K extends keyof Events>(type: K, event: Events[K]): void {
    const bucket = this.buckets.get(type);
    if (!bucket) return;
    let firstError: unknown;
    for (const listener of [...bucket]) {
      if (listener.signal?.aborted) continue;
      try {
        (listener.handler as (value: Events[K]) => void)(event);
      } catch (error) {
        firstError ??= error;
      }
    }
    if (firstError !== undefined) throw firstError;
  }

  /** Remove aborted listeners and empty buckets. Normal disposal remains the primary cleanup. */
  sweep(): number {
    let removed = 0;
    for (const [type, bucket] of this.buckets) {
      for (const listener of bucket) {
        if (listener.signal?.aborted) {
          bucket.delete(listener);
          removed += 1;
        }
      }
      if (bucket.size === 0) this.buckets.delete(type);
    }
    return removed;
  }

  clear(): void {
    for (const bucket of this.buckets.values()) {
      for (const listener of bucket) {
        if (listener.signal && listener.abort) {
          listener.signal.removeEventListener("abort", listener.abort);
        }
      }
    }
    this.buckets.clear();
  }

  stats(): EventRouterStats {
    let listeners = 0;
    for (const bucket of this.buckets.values()) listeners += bucket.size;
    return { eventTypes: this.buckets.size, listeners };
  }
}

/** Own all event handlers created by one component or store lifetime. */
export class EventScope {
  private readonly controller = new AbortController();

  get signal(): AbortSignal {
    return this.controller.signal;
  }

  on<Events extends object, K extends keyof Events>(
    router: EventRouter<Events>,
    type: K,
    handler: (event: Events[K]) => void,
  ): () => void {
    return router.on(type, handler, { signal: this.signal });
  }

  dispose(): void {
    this.controller.abort();
  }
}
