import * as p from "./protocol.js";
import * as r from "./reading.js";
import { acquireBrowserJob, type BrowserJob } from "./browser-job.js";
import { ObservationAuthority } from "./authority.js";
import { beginPageExcerpt, finishPageExcerpt, type Extracted } from "./page-excerpt.js";

// Survives connection-owner disposal, but never persists or retains page text.
// The lease also excludes metadata until durable native acknowledgement.
let outbox: { settlement: r.Settlement; actual: BrowserJob } | null = null;
export function acknowledgeRead(context: r.Context | null, pairing: p.Pairing) {
  if (context && outbox && p.equal(pairing, outbox.settlement.context.pairing)
    && r.sameContext(context, outbox.settlement.context)) {
    outbox.actual.settle();
    outbox = null;
  }
}
export function pendingSettlement(pairing: p.Pairing): r.Settlement | null {
  return outbox && p.equal(pairing, outbox.settlement.context.pairing) ? outbox.settlement : null;
}

type Job = {
  request: r.Request; owner: ReturnType<ObservationAuthority["snapshot"]>;
  deadline: number; withdrawn: boolean; sent: boolean;
  extracted: Extracted | null; reply: r.Reply | null;
};

function text(value: unknown, maximum: number): value is string {
  return typeof value === "string" && new TextEncoder().encode(value).length <= maximum
    && !/[\p{Cc}\p{Cs}]/u.test(value.replace(/[\n\t]/g, ""));
}
function extracted(value: unknown, request: r.Request): value is Extracted {
  if (p.object(value, ["started", "state", "request", "url", "guard_absent"])) {
    return value.started === false && value.state === "unavailable" && value.guard_absent === true
      && value.request === request.context.request && value.url === request.document.url;
  }
  if (p.object(value, ["started", "state"])) {
    return (value.started === "unknown" && value.state === "unavailable")
      || (value.started === true && ["changed", "unavailable"].includes(String(value.state)));
  }
  if (p.object(value, ["started", "state", "dom_revision"])) {
    return value.started === true && value.state === "empty" && p.counter(value.dom_revision);
  }
  if (!p.object(value, ["started", "state", "dom_revision", "title", "blocks", "truncated", "excluded_content"])
    || value.started !== true || value.state !== "excerpt" || !p.counter(value.dom_revision)
    || !text(value.title, 256) || !Array.isArray(value.blocks) || !value.blocks.length
    || value.blocks.length > Math.min(16, request.message_limit) || typeof value.truncated !== "boolean"
    || typeof value.excluded_content !== "boolean") return false;
  let bytes = 0;
  for (const block of value.blocks) {
    if (!text(block, 512) || !block.trim()) return false;
    bytes += new TextEncoder().encode(block).length;
  }
  return bytes <= 4096;
}
function result<T>(values: chrome.scripting.InjectionResult<T>[], job: Job): unknown {
  if (values.length !== 1 || values[0].frameId !== 0 || values[0].documentId !== job.request.document.document) {
    throw new Error("Read document result mismatch");
  }
  return values[0].result;
}

export class ReadJob {
  private disposed = false;
  private job: Job | null = null;
  private readonly seen = new Set<string>();
  private expiry: ReturnType<typeof setTimeout> | undefined;
  // Retained until every actual API/cleanup promise returns; never raced away.
  private running: Promise<void> | null = null;
  constructor(private readonly authority: ObservationAuthority, private readonly connected: () => boolean,
    private readonly revision: () => number, private readonly failed: () => void) {}

  invalidate() {
    clearTimeout(this.expiry); this.expiry = undefined;
    if (this.job) { this.job.withdrawn = true; this.job.extracted = null; this.job.reply = null; }
  }
  dispose() { this.disposed = true; this.invalidate(); }
  private current(job: Job) {
    const context = job.request.context;
    return !this.disposed && this.connected() && this.job === job && !job.withdrawn
      && performance.now() < job.deadline && this.authority.current(job.owner)
      && this.revision() === context.observation_revision
      && job.owner?.selection === context.selection && job.owner.action_epoch === context.source.action_epoch;
  }
  private check(job: Job) { if (!this.current(job)) throw new Error("Read authority withdrawn"); }
  private arm(job: Job) {
    clearTimeout(this.expiry);
    const expire = () => {
      if (this.job !== job || job.withdrawn) return;
      const remaining = job.deadline - performance.now();
      if (remaining > 0) { this.expiry = setTimeout(expire, Math.ceil(remaining)); return; }
      this.invalidate();
    };
    this.expiry = setTimeout(expire, Math.max(1, Math.ceil(job.deadline - performance.now())));
  }
  observe(value: r.Request | null, session: string, generation: number, pairing: p.Pairing) {
    if (!value) { this.invalidate(); return; }
    if (value.context.browser_session !== session || value.context.browser_generation !== generation
      || !p.equal(value.context.pairing, pairing)) throw new Error("Read transport changed");
    if (this.job?.request.context.request === value.context.request) {
      const job = this.job;
      if (!r.sameContext(job.request.context, value.context) || job.request.origin !== value.origin
        || JSON.stringify(job.request.document) !== JSON.stringify(value.document)
        || job.request.message_limit !== value.message_limit) throw new Error("Read request changed");
      job.deadline = Math.min(job.deadline, performance.now() + value.remaining_ms);
      if (!this.current(job)) this.invalidate();
      else this.arm(job);
      return;
    }
    if (this.seen.has(value.context.request) || this.seen.size >= 64) throw new Error("Read identity reused or exhausted");
    this.seen.add(value.context.request);
    if (this.running || outbox) throw new Error("Read owner occupied");
    const job: Job = { request: value, owner: this.authority.snapshot(), deadline: performance.now() + value.remaining_ms,
      withdrawn: false, sent: false, extracted: null, reply: null };
    this.job = job;
    this.check(job);
    this.arm(job);
    const actual = acquireBrowserJob();
    if (!actual) throw new Error("Actual browser owner occupied");
    this.running = this.run(job, actual).catch(() => {
      // Unknown actual completion retains the module-global lease indefinitely.
      this.invalidate();
      if (!this.disposed) this.failed();
    }).finally(() => { this.running = null; });
  }

  // Withdrawal only permits exact-document metadata cleanup. It never revives
  // content authority, changes target, or grants a missing host permission.
  private async verify(job: Job, cleanup = false) {
    const check = () => {
      if (!cleanup) this.check(job);
      else if (!this.current(job)) { job.withdrawn = true; job.extracted = null; job.reply = null; }
    };
    const request = job.request, document = request.document;
    check();
    const permitted = await chrome.permissions.contains({ origins: [request.origin + "/*"] }); check();
    if (!permitted) throw new Error("Read permission unavailable");
    const tab = await chrome.tabs.get(document.tab); check();
    const usable = (value: chrome.tabs.Tab) => value.id === document.tab && value.windowId === document.window
      && value.url === document.url && !value.incognito && !value.discarded && !value.frozen
      && value.status === "complete" && !value.pendingUrl;
    if (!usable(tab)) throw new Error("Read tab changed");
    const target = { tabId: document.tab, frameId: 0, documentId: document.document };
    const frame = await chrome.webNavigation.getFrame(target); check();
    if (!frame || frame.documentId !== document.document || frame.url !== document.url || frame.errorOccurred
      || frame.documentLifecycle !== "active" || frame.frameType !== "outermost_frame" || frame.parentFrameId !== -1) {
      throw new Error("Read document changed");
    }
    const again = await chrome.tabs.get(document.tab); check();
    if (!usable(again)) throw new Error("Read tab changed");
    const stillPermitted = await chrome.permissions.contains({ origins: [request.origin + "/*"] }); check();
    if (!stillPermitted) throw new Error("Read permission removed");
  }
  private async run(job: Job, actual: BrowserJob) {
    let injected = false, settled = false;
    let outcome: r.Outcome = { state: "unavailable" };
    try {
      await this.verify(job); this.check(job);
      injected = true; // Rejection or loss after this point cannot prove absence.
      const values = await chrome.scripting.executeScript({
        target: { tabId: job.request.document.tab, documentIds: [job.request.document.document] },
        world: "ISOLATED", func: beginPageExcerpt,
        args: [{ request: job.request.context.request, url: job.request.document.url,
          maxBlocks: Math.min(16, job.request.message_limit), budgetMs: Math.max(1, Math.floor(job.deadline - performance.now())) }],
      });
      const value = result(values, job);
      if (!extracted(value, job.request)) throw new Error("Malformed read result");
      if (value.started === false) settled = true; // Correlated explicit absent-guard proof only.
      if (this.current(job)) job.extracted = value;
      this.check(job);
      await this.verify(job); this.check(job);
    } catch {
      job.extracted = null;
    }

    if (injected && !settled) {
      // Do not abandon any cleanup promise, including after disposal/deadline.
      // If current validation fails, this separate pass permits cleanup only.
      await this.verify(job, true);
      const values = await chrome.scripting.executeScript({
        target: { tabId: job.request.document.tab, documentIds: [job.request.document.document] },
        world: "ISOLATED", func: finishPageExcerpt,
        args: [job.request.context.request, job.request.document.url],
      });
      const finish = result(values, job);
      if (!p.object(finish, ["state", "request", "url", "dom_revision", "unchanged"])
        || finish.state !== "settled" || finish.request !== job.request.context.request
        || finish.url !== job.request.document.url || !p.counter(finish.dom_revision)
        || typeof finish.unchanged !== "boolean") throw new Error("Read cleanup unknown");
      settled = true;
      // Finish is the final awaited operation: the DOM guard spans every real
      // permission/document check. Only synchronous freshness checks follow.
      const value = job.extracted;
      if (this.current(job) && value?.started === true && finish.unchanged) {
        if (value.state === "excerpt" && value.dom_revision === finish.dom_revision) {
          outcome = { state: "excerpt", excerpt: { coverage: "partial", document: job.request.document,
            dom_revision: value.dom_revision, title: value.title, blocks: value.blocks,
            truncated: value.truncated, excluded_content: value.excluded_content } };
        } else if (value.state === "empty" && value.dom_revision === finish.dom_revision) outcome = { state: "empty" };
      } else if (value?.started === true) outcome = { state: "changed" };
    }
    // No injection occurred, or exact cleanup/absence has completed. Every API
    // above has returned. Transfer only metadata and the exclusion lease.
    if (injected && !settled) throw new Error("Read ownership unknown");
    job.extracted = null;
    if (this.current(job)) job.reply = { context: job.request.context, outcome };
    if (outbox) throw new Error("Settlement owner occupied");
    outbox = { settlement: { context: job.request.context, kind: "actual_job_settled" }, actual };
  }
  takeReply(): r.Reply | null {
    const job = this.job;
    if (!job || !this.current(job) || job.sent || !job.reply) { if (job && !this.current(job)) this.invalidate(); return null; }
    job.sent = true;
    const reply = job.reply; job.reply = null;
    return reply;
  }
}
