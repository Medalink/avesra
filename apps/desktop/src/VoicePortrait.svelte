<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native } from "./runtime";
  import { ensureManagementVerification } from "./setup";
  import { decodeAvatar } from "./speaker-profiles";
  let { context, visible, blocked, connected, locked, onbusy, onrefresh }: { context: string; visible: boolean; blocked: boolean; connected: boolean; locked: boolean; onbusy: (busy: boolean) => void; onrefresh: () => Promise<void> } = $props();
  type Slot = "pending" | "captured" | "missing";
  type Summary = {version: 1; session: string; remaining_ms: number; batches: number; captured_slots: number; slots: Slot[]; words: string[]};
  type Progress = {version: 1; session: string; request: string; batch: number; received_samples: number; slot: number; phase: "observing" | "slot_complete"; captured_slots: number};
  const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
  const integer = (value: unknown, maximum: number): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= maximum;
  function decode(value: unknown): Summary {
    const v = value as Summary;
    if (!v || v.version !== 1 || typeof v.session !== "string" || !uuid.test(v.session) || /^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(v.session)
      || !integer(v.remaining_ms, 60000) || !v.remaining_ms || !integer(v.batches, 2) || !integer(v.captured_slots, 20)
      || !Array.isArray(v.slots) || v.slots.length !== 20 || v.slots.some(s => !["pending", "captured", "missing"].includes(s))
      || v.slots.filter(s => s === "captured").length !== v.captured_slots
      || v.slots.some((s, i) => i < v.batches * 10 ? s === "pending" : s !== "pending")
      || !Array.isArray(v.words) || v.words.length !== 20 || v.words.some(w => typeof w !== "string" || !w.trim() || w.length > 32 || /[\u0000-\u001f\u007f]/.test(w))) throw new Error("The portrait observation response is incompatible.");
    return v;
  }
  let mounted = false, generation = 0, oldContext = "", sessionContext = "", expires = 0;
  let stopProgress: (() => void) | undefined, expiryTimer: ReturnType<typeof setTimeout> | undefined;
  let ready = $state(false), working = $state(false), saving = $state(false), summary = $state<Summary | null>(null), active = $state<string | null>(null), progress = $state<Progress | null>(null), error = $state(""), note = $state("");
  const allowed = $derived(native && ready && visible && connected && !locked && !blocked && !working);
  function current(operation: number, key: string) { return mounted && visible && connected && !locked && operation === generation && key === context; }
  function cancelNative(session: string) { void command("cancel_voice_portrait", {session}).catch(() => {}); }
  function clear(message = "") {
    generation++;
    if (expiryTimer) clearTimeout(expiryTimer);
    expiryTimer = undefined;
    if (summary) cancelNative(summary.session);
    summary = null; sessionContext = ""; active = null; progress = null; error = ""; note = message;
  }
  $effect(() => {
    const key = `${context}:${visible}:${connected}:${locked}`;
    if (key !== oldContext) { oldContext = key; untrack(() => clear()); }
  });
  function keep(value: Summary, started: number) {
    expires = Math.min(expires, started + value.remaining_ms);
    if (performance.now() >= expires) { cancelNative(value.session); throw new Error("The portrait session expired. Begin again when ready."); }
    summary = value;
    if (expiryTimer) clearTimeout(expiryTimer);
    expiryTimer = setTimeout(() => clear("The portrait session expired. No automatic recording was started."), Math.max(0, expires - performance.now()));
  }
  async function begin() {
    if (!allowed || summary) return;
    working = true; onbusy(true); error = ""; note = "";
    const operation = ++generation, key = context;
    try {
      await ensureManagementVerification(() => current(operation, key), message => { if (current(operation, key)) note = message; });
      if (!current(operation, key)) return;
      const started = performance.now();
      const raw = await command<unknown>("begin_voice_portrait");
      let value: Summary;
      try { value = decode(raw); } catch (e) { const id = (raw as Partial<Summary> | null)?.session; if (typeof id === "string" && uuid.test(id)) cancelNative(id); throw e; }
      if (!current(operation, key)) { cancelNative(value.session); return; }
      if (value.batches !== 0) { cancelNative(value.session); throw new Error("The new portrait session was not empty."); }
      expires = started + value.remaining_ms; sessionContext = key; keep(value, started); note = "Choose Record batch when you are ready. Normal listening continues.";
    } catch (e) { if (current(operation, key)) { error = String(e); note = ""; } }
    finally { working = false; onbusy(false); }
  }
  async function record() {
    const saved = summary;
    if (!allowed || !saved || saved.batches >= 2 || performance.now() >= expires) return;
    working = true; onbusy(true); error = ""; note = "";
    const operation = generation, key = context, request = crypto.randomUUID(), started = performance.now();
    active = request; progress = null;
    try {
      const value = decode(await command<unknown>("record_voice_portrait", {session: saved.session, request}));
      if (!current(operation, key)) return;
      if (value.session !== saved.session || value.batches !== saved.batches + 1 || value.words.some((w, i) => w !== saved.words[i])) throw new Error("The portrait batch source changed.");
      keep(value, started); note = value.batches === 2 ? "Both batches finished. Review the observed slots before saving." : "First batch finished. Record the second batch when ready.";
    } catch (e) { if (current(operation, key)) { clear(); error = String(e); } }
    finally { if (active === request) active = null; working = false; onbusy(false); }
  }
  async function save() {
    const saved = summary;
    if (!allowed || !saved || saved.batches !== 2 || !saved.captured_slots || performance.now() >= expires) return;
    working = true; saving = true; onbusy(true); error = ""; note = "";
    const operation = generation, key = context;
    try {
      const value = decodeAvatar(await command<unknown>("save_voice_portrait", {session: saved.session}));
      if (!current(operation, key)) return;
      if (value.state !== "ready_with_portrait" || value.parameters?.version !== 2) throw new Error("The saved portrait response could not be read. Refresh the saved avatar before trying again.");
      clear("Optional portrait saved. This is not a speaker-verification result.");
    } catch (e) { if (current(operation, key)) { clear(); error = String(e); } }
    finally {
      // Cancellation can race an already-committed native save. Reconcile the
      // same visible source after actual settlement, without restoring a session.
      if (mounted && visible && connected && !locked && key === context) {
        try { await onrefresh(); }
        catch { if (mounted && visible && key === context) error = "The saved avatar could not be refreshed. Reopen this section to read its current state."; }
      }
      saving = false; working = false; onbusy(false);
    }
  }
  onMount(() => {
    mounted = true;
    if (native) void listen<Progress>("voice-portrait-progress", ({payload: p}) => {
      if (!mounted || !visible || locked || !connected || !summary || !active || context !== sessionContext || performance.now() >= expires || p?.version !== 1 || p.session !== summary.session || p.request !== active
        || p.batch !== summary.batches || !integer(p.received_samples,128000) || !integer(p.slot,19) || Math.floor(p.slot / 10) !== p.batch || !integer(p.captured_slots,20) || p.captured_slots < summary.captured_slots || p.captured_slots > (p.batch + 1) * 10 || !["observing","slot_complete"].includes(p.phase)) return;
      if (progress && p.received_samples < progress.received_samples) return;
      progress = p;
    }).then(stop => { if (mounted) {stopProgress = stop; ready = true;} else stop(); }).catch(() => { if (mounted) error = "Portrait progress is unavailable. No observation was started."; });
    return () => { mounted = false; ready = false; stopProgress?.(); clear(); };
  });
</script>
<section class="flex flex-col gap-3 border-b border-white/[0.06] pb-3 mb-3">
  <div><p class="m-0 text-[12.5px] font-medium">Optional voice portrait</p><p class="av-hint mt-1">Save optional features from two short batches of your voice. Normal listening continues. These timed prompts are not a word-recognition or identity test.</p></div>
  {#if summary}
    <div class="grid grid-cols-2 gap-1 sm:grid-cols-4">{#each summary.words as word, i}<div class={`flex items-center justify-between gap-2 px-2 py-1.5 text-[11px] ring-1 ring-inset ${active && progress?.slot === i ? "bg-av-500/10 ring-av-500/40" : "ring-white/[0.06]"}`}><span>{word}</span><span class="text-zinc-500">{summary.slots[i]}</span></div>{/each}</div>
    <p class="av-hint">{summary.captured_slots} of 20 slots captured · {summary.batches} of 2 batches finished. Missing means no usable feature was captured; it does not mean a word was incorrect.</p>
    {#if active}<p class="av-hint" role="status">Observing batch {summary.batches + 1}{progress ? ` · ${(progress.received_samples / 16000).toFixed(1)} of 8 seconds` : " · waiting for existing input"}.</p>{/if}
    <div class="flex flex-wrap gap-2"><button class="av-btn av-btn-secondary av-btn-sm" disabled={!allowed || summary.batches >= 2} onclick={record}>Record batch {Math.min(2, summary.batches + 1)}</button><button class="av-btn av-btn-primary av-btn-sm" disabled={!allowed || summary.batches !== 2 || !summary.captured_slots} onclick={save}>Save portrait</button><button class="av-btn av-btn-ghost av-btn-sm" onclick={() => clear(saving ? "Cancellation requested. The saved avatar will be refreshed when saving settles." : "Portrait cancellation requested.")}>Cancel</button></div>
  {:else}<button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!allowed} onclick={begin}>{working ? "Working…" : "Prepare portrait"}</button>{/if}
  {#if note}<p class="av-hint" role="status">{note}</p>{/if}
  {#if error}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
</section>
