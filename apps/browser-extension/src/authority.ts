import type { Authority } from "./protocol.js";

// Transport context only: no method creates an origin grant or page capability.
// Future semantic work must also hold native-issued scope/document authority.
export class ObservationAuthority {
  private latest: Authority | null = null;
  private history: Authority | null = null;
  private revision = 0;
  private deadline = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private disposed = false;
  private seen = false;
  constructor(private readonly expired: () => void) {}
  private invalidate() {
    this.latest = null;
    if (!Number.isSafeInteger(this.revision + 1)) { this.disposed = true; throw new Error("Observation ownership exhausted"); }
    this.revision++;
  }
  observe(value: Authority | null) {
    if (this.disposed || (this.deadline !== 0 && performance.now() >= this.deadline)) throw new Error("Native authority expired");
    if ((this.seen && this.history?.selection !== value?.selection) || (this.history && (!value || value.action_epoch < this.history.action_epoch))) throw new Error("Native authority rolled back");
    const changed = this.history?.selection !== value?.selection || this.history?.action_epoch !== value?.action_epoch || this.history?.mode_allowed !== value?.mode_allowed;
    // Withdrawal is synchronous and precedes publishing any replacement context.
    if (changed) this.invalidate();
    this.history = value ? { ...value } : null;
    this.seen = true;
    this.latest = value?.mode_allowed ? { ...value } : null;
    this.deadline = performance.now() + 3000;
    clearTimeout(this.timer);
    const expire = () => {
      if (this.disposed) return;
      const remaining = this.deadline - performance.now();
      if (remaining > 0) { this.timer = setTimeout(expire, Math.ceil(remaining)); return; }
      this.latest = null; this.disposed = true; this.expired();
    };
    this.timer = setTimeout(expire, 3000);
  }
  // Snapshots are invalidated by epoch/mode changes, disconnect or missing status;
  // recheck immediately before and after every future asynchronous operation.
  snapshot() {
    if (this.disposed || performance.now() >= this.deadline || !this.latest) return null;
    return { ...this.latest, observation_revision: this.revision };
  }
  current(value: ReturnType<ObservationAuthority["snapshot"]>) {
    const actual = this.snapshot();
    return !!value && !!actual && value.selection === actual.selection && value.action_epoch === actual.action_epoch && value.observation_revision === actual.observation_revision;
  }
  dispose() { this.latest = null; this.disposed = true; clearTimeout(this.timer); }
}
