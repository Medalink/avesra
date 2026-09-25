import type { SignalFrame } from "./Signal.svelte";

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const positive = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v > 0;
const finite = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v) && v >= 0;

// Display observation only. No authority, audio queue, or retained history.
export class PlaybackSignal {
  frame: SignalFrame | null = null;
  private epoch = 0;
  private allowed = false;
  private replyAllowed = false;
  private purpose: "preview" | "reply" | null = null;
  private output: string | null = null;
  private closed = false;
  private retiredEpoch = 0;
  private sequence = 0;
  private expires = 0;
  private calibration: { offset: number; expires: number } | null = null;

  context(epoch: number, allowed: boolean, replyAllowed: boolean) {
    if (epoch !== this.epoch) {
      this.epoch = epoch;
      this.output = null;
      this.purpose = null;
      this.closed = epoch <= this.retiredEpoch;
      this.frame = null;
    }
    this.allowed = allowed;
    this.replyAllowed = replyAllowed;
    if (!allowed || (this.purpose === "reply" && !replyAllowed)) { this.frame = null; this.closed = true; }
  }
  uncalibrated() { this.calibration = null; this.frame = null; }
  calibrate(nativeTime: unknown, start: number, end: number) {
    if (!finite(nativeTime) || end < start || end - start > 250) {
      this.uncalibrated();
      return;
    }
    this.calibration = { offset: start - nativeTime, expires: start + 30_000 };
    // Never recalculate an already displayed frame's deadline.
    this.expire(end);
  }
  expire(now: number) {
    if (!this.calibration || now >= this.calibration.expires || now >= this.expires) this.frame = null;
  }
  accept(value: unknown, now: number, visible: boolean) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return;
    const e = value as Record<string, unknown>;
    if (!positive(e.epoch) || !positive(e.sequence) || e.sequence <= this.sequence ||
      typeof e.output !== "string" || !uuid.test(e.output) || e.output === "00000000-0000-0000-0000-000000000000") return;
    if (e.type === "clear") {
      if (Object.keys(e).length !== 4 || (e.epoch === this.epoch && this.output !== null && e.output !== this.output)) return;
      this.sequence = e.sequence;
      this.retiredEpoch = Math.max(this.retiredEpoch, e.epoch);
      if (this.epoch <= e.epoch) {
        if (this.epoch === e.epoch) this.output = e.output;
        this.closed = true;
        this.frame = null;
      }
      return;
    }
    if (e.epoch !== this.epoch || (this.output !== null && e.output !== this.output)) return;
    if (e.type !== "sample" || Object.keys(e).length !== 8 || (e.purpose !== "preview" && e.purpose !== "reply") ||
      !finite(e.submittedAt) || !finite(e.expiresAt) || Math.abs(e.expiresAt - e.submittedAt - 250) > 0.001 ||
      !Array.isArray(e.samples) || e.samples.length !== 32 ||
      e.samples.some(v => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 1)) return;
    if (this.purpose !== null && this.purpose !== e.purpose) return;
    this.purpose = e.purpose;
    // Consume ordering even when this event cannot be displayed.
    this.sequence = e.sequence;
    this.output = e.output;
    const clock = this.calibration;
    if (this.closed || !this.allowed || (e.purpose === "reply" && !this.replyAllowed) || !visible || !clock || now >= clock.expires) return;
    const deadline = e.expiresAt + clock.offset;
    if (now >= deadline || deadline > now + 250) return;
    this.expires = Math.min(deadline, clock.expires);
    this.frame = { kind: "speaking", source: "assistant", samples: e.samples, sequence: e.sequence,
      capturedAt: e.submittedAt, playbackEpoch: e.epoch, outputId: e.output, purpose: e.purpose };
  }
}
