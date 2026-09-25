// Shared across connection-owner replacement. Only the actual promise chain
// may settle this token; disposal/deadline/caller loss cannot release it.
let actual: symbol | null = null;
export type BrowserJob = { settle(): void };
export function acquireBrowserJob(): BrowserJob | null {
  if (actual !== null) return null;
  const token = Symbol("actual browser operation");
  actual = token;
  return {
    settle() {
      if (actual === token) actual = null;
    },
  };
}
