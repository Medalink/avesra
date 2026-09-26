import * as p from "./protocol.js";
import * as r from "./reading.js";
import { acquireBrowserJob, type BrowserJob } from "./browser-job.js";
import { ObservationAuthority } from "./authority.js";
import { beginPageExcerpt, finishPageExcerpt, type Extracted } from "./page-excerpt.js";
import { probe } from "./provider.js";
import { observeXReady, type XObservation, type XInputObservation } from "./x-ready.js";
import {MailboxStream,observeGmail,type Observation as GmailObservation,type Ack as MailboxAck} from "./gmail-job.js";
import { XDocument } from "./x-document.js";
import type { Candidate } from "./documents.js";

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
  request: r.Request; document:Candidate|null; creation:XDocument|null; owner: ReturnType<ObservationAuthority["snapshot"]>;
  deadline: number; withdrawn: boolean; sent: boolean;
  mailbox:MailboxStream|null;
  extracted: Extracted | XObservation | XInputObservation | GmailObservation | null; reply: r.Reply | null;
  focusTab:boolean; focusWindow:boolean; focusVacancy:boolean; focused:boolean; focusUncertain:boolean;
};

function text(value: unknown, maximum: number): value is string {
  return typeof value === "string" && new TextEncoder().encode(value).length <= maximum
    && !/[\p{Cc}\p{Cs}]/u.test(value.replace(/[\n\t]/g, ""));
}
function extracted(value: unknown, request: r.Request, document:Candidate): value is Extracted {
  if (p.object(value,["started","state","dom_revision","provider","complete","choices"])) {
    return value.started === true && value.state === "provider_inspection" && (request.mode.kind === "provider_inspection" || request.mode.kind === "x_ready" || request.mode.kind === "inbox")
      && value.provider === (request.mode.kind === "x_ready" ? "x" : request.mode.kind === "inbox" ? "gmail" : request.mode.provider) && probe({provider:value.provider,scope:"provider_header",document,
        dom_revision:value.dom_revision,complete:value.complete,choices:value.choices});
  }
  if (p.object(value, ["started", "state", "request", "url", "guard_absent"])) {
    return value.started === false && value.state === "unavailable" && value.guard_absent === true
      && value.request === request.context.request && value.url === document.url;
  }
  if (p.object(value, ["started", "state"])) {
    return (value.started === "unknown" && value.state === "unavailable")
      || (value.started === true && ["changed", "unavailable"].includes(String(value.state)));
  }
  if (p.object(value, ["started", "state", "dom_revision"])) {
    return request.mode.kind === "excerpt" && value.started === true && value.state === "empty" && p.counter(value.dom_revision);
  }
  if (!p.object(value, ["started", "state", "dom_revision", "title", "blocks", "truncated", "excluded_content"])
    || request.mode.kind !== "excerpt" || value.started !== true || value.state !== "excerpt" || !p.counter(value.dom_revision)
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
  if (values.length !== 1 || values[0].frameId !== 0 || values[0].documentId !== job.document!.document) {
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

  creationEvent(event:Parameters<XDocument["event"]>[0]):boolean { return this.job?.creation?.event(event)??false; }
  expectedUpdate(tab:number,change:object,info:chrome.tabs.Tab):boolean {
    const job=this.job,document=job?.document;
    return !!job&&!!document&&this.current(job)&&job.request.mode.kind==="x_ready"&&tab===document.tab
      &&info.id===document.tab&&info.windowId===document.window&&info.url===document.url&&!info.incognito
      &&!info.discarded&&!info.frozen&&info.status==="complete"&&!info.pendingUrl
      &&Object.keys(change).length>0&&Object.keys(change).every(key=>key==="title"||key==="favIconUrl");
  }
  invalidate() {
    clearTimeout(this.expiry); this.expiry = undefined;
    if (this.job) { this.job.mailbox?.withdraw(); this.job.withdrawn = true; this.job.extracted = null; this.job.reply = null; }
  }
  dispose() { this.disposed = true; this.invalidate(); }
  // Only the two exact expected focus notifications may belong to this job.
  // Navigation/tab replacement and any other activation still withdraw it.
  expectedActivation(tab:number,window:number):boolean {
    const job=this.job;
    if(!job||!this.current(job)||!job.focusTab||job.request.mode.kind!=="x_ready"
      ||tab!==job.document!.tab||window!==job.document!.window)return false;
    job.focusTab=false;return true;
  }
  expectedFocus(window:number):boolean {
    const job=this.job;
    if(!job||!this.current(job)||!job.focusWindow||job.request.mode.kind!=="x_ready")return false;
    if(window===chrome.windows.WINDOW_ID_NONE&&job.focusVacancy){job.focusVacancy=false;return true;}
    if(window!==job.document!.window)return false;
    job.focusWindow=false;return true;
  }
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
        || job.request.message_limit !== value.message_limit || JSON.stringify(job.request.mode) !== JSON.stringify(value.mode)) throw new Error("Read request changed");
      job.deadline = Math.min(job.deadline, performance.now() + value.remaining_ms);
      if (!this.current(job)) this.invalidate();
      else this.arm(job);
      return;
    }
    if (this.seen.has(value.context.request) || this.seen.size >= 64) throw new Error("Read identity reused or exhausted");
    this.seen.add(value.context.request);
    if (this.running || outbox) throw new Error("Read owner occupied");
    const job: Job = { request: value, document:value.document, creation:null, owner: this.authority.snapshot(), deadline: performance.now() + value.remaining_ms,
      withdrawn: false, sent: false, mailbox:null, extracted: null, reply: null,focusTab:false,focusWindow:false,focusVacancy:false,focused:false,focusUncertain:false };
    this.job = job;
    if(value.mode.kind==="inbox")job.mailbox=new MailboxStream(value.context,()=>this.current(job));
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
    const request = job.request, document = job.document;
    if(!document)throw new Error("Document unavailable");
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
      if(!job.document){
        if(job.request.mode.kind!=="x_ready")throw new Error("Unselected read document");
        job.creation=new XDocument(()=>this.check(job));
        job.document=await job.creation.create();this.check(job);
      }
      await this.verify(job); this.check(job);
      injected = true; // Rejection or loss after this point cannot prove absence.
      const values = await chrome.scripting.executeScript({
        target: { tabId: job.document!.tab, documentIds: [job.document!.document] },
        world: "ISOLATED", func: beginPageExcerpt,
        args: [{ request: job.request.context.request, url: job.document!.url,
          maxBlocks: Math.min(16, job.request.message_limit), budgetMs: Math.max(1, Math.floor(job.deadline - performance.now())),
          provider: job.request.mode.kind === "provider_inspection" ? job.request.mode.provider : job.request.mode.kind === "x_ready" ? "x" : job.request.mode.kind === "inbox" ? "gmail" : null,
          inbox:job.request.mode.kind==="inbox",
          xReady:job.request.mode.kind==="x_ready" }],
      });
      const value = result(values, job);
      if (!extracted(value, job.request, job.document!)) throw new Error("Malformed read result");
      if (value.started === false) settled = true; // Correlated explicit absent-guard proof only.
      if (this.current(job)) job.extracted = value;
      this.check(job);
      await this.verify(job); this.check(job);
      if(job.request.mode.kind==="inbox"){
        const observed=await chrome.scripting.executeScript({target:{tabId:job.document!.tab,documentIds:[job.document!.document]},world:"ISOLATED",func:observeGmail,args:[job.request.context.request,job.document!.url,job.request.mode.account]});
        this.check(job);const value=result(observed,job);
        if(!p.object(value,["started","state","dom_revision","account","incomplete"]) || value.started!==true || value.state!=="inbox" || !p.counter(value.dom_revision) || value.account!==job.request.mode.account || !["account_unverified","inbox_membership_unverified","provider_unsupported"].includes(String(value.incomplete)))throw new Error("Gmail semantics unavailable");
        job.extracted=value as GmailObservation;
      }
      if(job.request.mode.kind==="x_ready"){
        const observed=await chrome.scripting.executeScript({target:{tabId:job.document!.tab,documentIds:[job.document!.document]},world:"ISOLATED",func:observeXReady,
          args:[job.request.context.request,job.document!.url,job.request.mode.account]});
        this.check(job);const ready=result(observed,job);
        if(p.object(ready,["started","state","dom_revision","account","reason"])&&ready.started===true&&ready.state==="x_needs_input"
          &&p.counter(ready.dom_revision)&&ready.account===job.request.mode.account&&["login_required","account_mismatch","unsupported_page"].includes(String(ready.reason))){
          job.extracted=ready as XInputObservation;
        }else{
        if(!p.object(ready,["started","state","dom_revision","account"])||ready.started!==true||ready.state!=="x_ready"
          ||!p.counter(ready.dom_revision)||ready.account!==job.request.mode.account)throw new Error("X account or compose state unavailable");
        job.extracted=ready as XObservation;
        // Mutation is one-shot and held by the original actual job/deadline.
        job.focusTab=true;
        job.focusUncertain=true;
        const active=await chrome.tabs.update(job.document!.tab,{active:true});
        if(!active||active.id!==job.document!.tab||active.windowId!==job.document!.window||!active.active)throw new Error("X activation uncertain");
        job.focusUncertain=false;this.check(job);
        job.focusWindow=true;job.focusVacancy=true;
        job.focusUncertain=true;
        const focused=await chrome.windows.update(job.document!.window,{focused:true});
        if(focused.id!==job.document!.window||!focused.focused)throw new Error("X focus uncertain");
        job.focusUncertain=false;this.check(job);
        await this.verify(job);this.check(job);
        const tab=await chrome.tabs.get(job.document!.tab);this.check(job);
        const window=await chrome.windows.get(job.document!.window);this.check(job);
        if(!tab.active||tab.windowId!==window.id||!window.focused)throw new Error("X focus changed");
        const confirmed=await chrome.scripting.executeScript({target:{tabId:job.document!.tab,documentIds:[job.document!.document]},world:"ISOLATED",func:observeXReady,
          args:[job.request.context.request,job.document!.url,job.request.mode.account]});
        this.check(job);
        if(JSON.stringify(result(confirmed,job))!==JSON.stringify(ready))throw new Error("X readiness changed after focus");
        job.focused=true;
        }
      }
    } catch {
      job.extracted = null;
    }

    if(job.creation&&!await job.creation.settle())throw new Error("Created browser work settlement unknown");

    if (injected && !settled) {
      // Do not abandon any cleanup promise, including after disposal/deadline.
      // If current validation fails, this separate pass permits cleanup only.
      await this.verify(job, true);
      const values = await chrome.scripting.executeScript({
        target: { tabId: job.document!.tab, documentIds: [job.document!.document] },
        world: "ISOLATED", func: finishPageExcerpt,
        args: [job.request.context.request, job.document!.url],
      });
      const finish = result(values, job);
      if (!p.object(finish, ["state", "request", "url", "dom_revision", "unchanged"])
        || finish.state !== "settled" || finish.request !== job.request.context.request
        || finish.url !== job.document!.url || !p.counter(finish.dom_revision)
        || typeof finish.unchanged !== "boolean") throw new Error("Read cleanup unknown");
      settled = true;
      // Finish is the final awaited operation: the DOM guard spans every real
      // permission/document check. Only synchronous freshness checks follow.
      const value = job.extracted;
      if (this.current(job) && value?.started === true && finish.unchanged) {
        if (value.state === "excerpt" && value.dom_revision === finish.dom_revision) {
          outcome = { state: "excerpt", excerpt: { coverage: "partial", document: job.document!,
            dom_revision: value.dom_revision, title: value.title, blocks: value.blocks,
            truncated: value.truncated, excluded_content: value.excluded_content } };
        } else if (value.state === "inbox" && value.dom_revision===finish.dom_revision && job.mailbox) {
          outcome={state:"inbox",terminal:{document:job.document!,dom_revision:value.dom_revision,account:value.account,...job.mailbox.terminal(),incomplete:value.incomplete}};
        } else if (value.state === "x_ready" && value.dom_revision === finish.dom_revision && job.focused) {
          outcome={state:"x_ready",ready:{document:job.document!,dom_revision:value.dom_revision,account:value.account,focused:true,created:job.request.document===null}};
        } else if (value.state === "x_needs_input" && value.dom_revision === finish.dom_revision) {
          outcome={state:"x_needs_input",evidence:{document:job.document!,dom_revision:value.dom_revision,account:value.account,reason:value.reason,created:job.request.document===null}};
        } else if (value.state === "empty" && value.dom_revision === finish.dom_revision) outcome = { state: "empty" };
        else if (value.state === "provider_inspection" && value.dom_revision === finish.dom_revision) {
          outcome = {state:"provider_inspection",probe:{provider:value.provider,scope:"provider_header",document:job.document!,
            dom_revision:value.dom_revision,complete:value.complete,choices:value.choices}};
        }
      } else if (value?.started === true) outcome = { state: "changed" };
    }
    // No injection occurred, or exact cleanup/absence has completed. Every API
    // above has returned. Transfer only metadata and the exclusion lease.
    if (injected && !settled) throw new Error("Read ownership unknown");
    if(job.focusUncertain)throw new Error("Browser focus settlement unknown");
    job.extracted = null;
    job.focusTab=false;job.focusWindow=false;job.focusVacancy=false;
    if (this.current(job)) job.reply = { context: job.request.context, outcome };
    if (outbox) throw new Error("Settlement owner occupied");
    outbox = { settlement: { context: job.request.context, kind: "actual_job_settled" }, actual };
  }
  takeMailboxChunk() {return this.job?.mailbox?.take()??null;}
  mailboxWaiting():boolean {return this.job?.mailbox?.waiting()??false;}
  acknowledgeMailbox(ack:MailboxAck|null) {this.job?.mailbox?.acknowledge(ack);}
  takeReply(): r.Reply | null {
    const job = this.job;
    if (!job || !this.current(job) || job.sent || !job.reply) { if (job && !this.current(job)) this.invalidate(); return null; }
    job.sent = true;
    const reply = job.reply; job.reply = null;
    return reply;
  }
}
