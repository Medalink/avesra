/** Coalesce repeated read requests without overlapping native store operations. */
export function latestRead<T>(
  read: () => Promise<T>,
  accept: (value: T) => void,
  reject: (error: unknown) => void,
  loading: (active: boolean) => void,
) {
  let pending = false;
  let running: Promise<void> | null = null;
  let disposed = false;
  async function drain() {
    if (disposed) return;
    loading(true);
    try {
      while (pending && !disposed) {
        pending = false;
        try {
          const value = await read();
          if (!disposed && !pending) accept(value);
        } catch (error) {
          if (!disposed && !pending) reject(error);
        }
      }
    } finally {
      running = null;
      if (!disposed) loading(false);
    }
  }
  return {
    refresh() {
      if (disposed) return Promise.resolve();
      pending = true;
      running ??= Promise.resolve().then(drain);
      return running;
    },
    dispose() { disposed = true; pending = false; },
  };
}
