import * as p from "./protocol.js";
import { acquireBrowserJob } from "./browser-job.js";
import { ObservationAuthority } from "./authority.js";

export type Candidate = { tab: number; window: number; frame: 0; document: string; url: string };
export type Request = { request: string; scope: p.Pairing; origin: string; remaining_ms: number; mode: { kind: "discover" } | { kind: "revalidate"; candidate: Candidate } };
export type Outcome = { state: "available"; candidates: Candidate[] } | { state: "unavailable" | "expired" | "too_many" };
export type Reply = { request: string; scope: p.Pairing; observation_revision: number; outcome: Outcome };
export function candidate(value: unknown, origin: string): value is Candidate {
  if (!p.object(value, ["tab", "window", "frame", "document", "url"]) || !nativeId(value.tab) || !nativeId(value.window) || value.frame !== 0 || !p.id(value.document) || typeof value.url !== "string" || value.url.length > 2048 || !/^[\x21-\x7e]+$/.test(value.url)) return false;
  try { const url = new URL(value.url); return url.href === value.url && !url.username && !url.password && url.origin === origin && p.origin(url.origin); } catch { return false; }
}
export function request(value: unknown): value is Request {
  if (!p.object(value, ["request", "scope", "origin", "remaining_ms", "mode"]) || !p.id(value.request) || !p.pairing(value.scope) || !p.origin(value.origin) || !p.counter(value.remaining_ms) || value.remaining_ms > 5000) return false;
  return (p.object(value.mode, ["kind"]) && value.mode.kind === "discover") || (p.object(value.mode, ["kind", "candidate"]) && value.mode.kind === "revalidate" && candidate(value.mode.candidate, value.origin));
}
function sameCandidate(a: Candidate, b: Candidate) { return a.tab === b.tab && a.window === b.window && a.frame === b.frame && a.document === b.document && a.url === b.url; }
function nativeId(value: unknown): value is number { return p.counter(value) && value <= 2147483647; }
type Job = { request: Request; deadline: number; revision: number; owner: ReturnType<ObservationAuthority["snapshot"]>; session: string; generation: number; reply: Reply | null; sent: boolean };
export class Documents {
  private disposed = false;
  private revision = 1;
  private job: Job | null = null;
  private seen = new Set<string>();
  observationRevision() { return this.revision; }
  constructor(private readonly authority: ObservationAuthority, private readonly connected: () => boolean, private readonly exhausted: () => void) {}
  invalidate() {
    if (!Number.isSafeInteger(this.revision + 1)) { this.dispose(); this.exhausted(); return; }
    this.revision++;
    if (this.job) this.job.reply = null;
  }
  private current(job: Job) {
    return !this.disposed && this.connected() && this.job === job && this.revision === job.revision && performance.now() < job.deadline && this.authority.current(job.owner);
  }
  observe(value: Request | null, session: string, generation: number) {
    if (!value) { this.job = null; return; }
    if (this.job?.request.request === value.request) {
      const job = this.job;
      if (!p.equal(job.request.scope, value.scope) || job.request.origin !== value.origin || JSON.stringify(job.request.mode) !== JSON.stringify(value.mode) || job.session !== session || job.generation !== generation) throw new Error("Document request changed");
      job.deadline = Math.min(job.deadline, performance.now() + value.remaining_ms);
      return;
    }
    if (this.seen.has(value.request) || this.seen.size >= 64) throw new Error("Document request identity reused or exhausted");
    this.seen.add(value.request);
    const owner = this.authority.snapshot();
    if (!owner) return;
    const job: Job = { request: value, deadline: performance.now() + value.remaining_ms, revision: this.revision, owner, session, generation, reply: null, sent: false };
    this.job = job;
    const actual = acquireBrowserJob();
    if (!actual) { this.publish(job, { state: "unavailable" }); return; }
    void this.run(job).then(outcome => this.publish(job, outcome)).catch(() => this.publish(job, { state: "unavailable" }))
      .finally(() => actual.settle());
  }
  private publish(job: Job, outcome: Outcome) {
    if (this.current(job)) job.reply = { request: job.request.request, scope: job.request.scope, observation_revision: job.revision, outcome };
  }
  private check(job: Job) { if (!this.current(job)) throw new Error("Document ownership expired"); }
  private async tab(job: Job, id: number): Promise<Candidate> {
    this.check(job);
    const tab = await chrome.tabs.get(id); this.check(job);
    if (!nativeId(tab.id) || tab.id !== id || !nativeId(tab.windowId) || tab.incognito || tab.discarded || tab.frozen || tab.status !== "complete" || tab.pendingUrl) throw new Error("Tab unavailable");
    const frame = await chrome.webNavigation.getFrame({ tabId: id, frameId: 0 }); this.check(job);
    if (!frame || frame.errorOccurred || frame.documentLifecycle !== "active" || frame.frameType !== "outermost_frame" || frame.parentFrameId !== -1 || !p.id(frame.documentId) || frame.url !== tab.url) throw new Error("Document unavailable");
    const result: Candidate = { tab: id, window: tab.windowId, frame: 0, document: frame.documentId, url: frame.url };
    if (!candidate(result, job.request.origin)) throw new Error("Origin unavailable");
    // Installed @types omits this Chrome 106+ field. Structural extra property
    // on a named object preserves the actual documented API without an any cast.
    const exact = { tabId: id, frameId: 0, documentId: frame.documentId };
    const again = await chrome.webNavigation.getFrame(exact); this.check(job);
    const currentTab = await chrome.tabs.get(id); this.check(job);
    if (!again || again.documentId !== result.document || again.url !== result.url || again.documentLifecycle !== "active" || again.errorOccurred || again.frameType !== "outermost_frame" || currentTab.id !== id || currentTab.windowId !== result.window || currentTab.url !== result.url || currentTab.incognito || currentTab.discarded || currentTab.frozen || currentTab.status !== "complete" || currentTab.pendingUrl) throw new Error("Document changed");
    return result;
  }
  private async run(job: Job): Promise<Outcome> {
    this.check(job);
    if (!await chrome.permissions.contains({ origins: [job.request.origin + "/*"] })) return { state: "unavailable" };
    this.check(job);
    let candidates: Candidate[];
    if (job.request.mode.kind === "revalidate") {
      const value = await this.tab(job, job.request.mode.candidate.tab); this.check(job);
      if (!sameCandidate(value, job.request.mode.candidate)) return { state: "unavailable" };
      candidates = [value];
    } else {
      const tabs = await chrome.tabs.query({ url: job.request.origin + "/*" }); this.check(job);
      if (tabs.length > 16) return { state: "too_many" };
      candidates = [];
      for (const tab of tabs) {
        if (!nativeId(tab.id)) return { state: "unavailable" };
        const value = await this.tab(job, tab.id); this.check(job);
        if (candidates.some(v => v.tab === value.tab || v.document === value.document)) return { state: "unavailable" };
        candidates.push(value);
      }
    }
    if (!await chrome.permissions.contains({ origins: [job.request.origin + "/*"] })) return { state: "unavailable" };
    this.check(job);
    return { state: "available", candidates };
  }
  takeReply(): Reply | null {
    const job = this.job;
    if (!job || !this.current(job) || job.sent || !job.reply) return null;
    job.sent = true;
    const reply = job.reply; job.reply = null;
    return reply;
  }
  dispose() { this.disposed = true; this.job = null; }
}
