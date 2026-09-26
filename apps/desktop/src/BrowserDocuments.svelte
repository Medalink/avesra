<script lang="ts">
  import { onMount } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  type Reference = { id: string; revision: string };
  type Candidate = { tab: number; window: number; frame: number; document: string; url: string };
  type Phase = { state: "waiting" | "unavailable" | "expired" | "too_many" } | { state: "available"; candidates: Candidate[] } | { state: "selected"; identity: string; revision: string; candidate: Candidate };
  type Status = { request: string | null; scope: Reference | null; remaining_ms: number; phase: Phase };
  let { runtime, reference, origin }: { runtime: Runtime | null; reference: Reference; origin: string } = $props();
  let busy = $state(false), error = $state(""), status = $state<Status | null>(null), owned = $state<string | null>(null);
  let mounted = false, generation = 0, reading = false, context = "", deadline = 0;
  const enabled = $derived(native && !!runtime?.connected && !runtime.locked && !runtime.settings.paused);
  function invalidate() {
    generation++; status = null; deadline = 0;
    const request = owned; owned = null;
    if (native && request) void command("cancel_browser_documents", { request }).catch(() => {});
  }
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}:${runtime?.capture_epoch}:${runtime?.action_epoch}:${reference.id}:${reference.revision}`;
    if (context !== next) { context = next; invalidate(); }
  });
  function publish(value: Status) {
    if ((value.request && value.request !== owned) || (value.scope && (value.scope.id !== reference.id || value.scope.revision !== reference.revision))) { status = null; return; }
    const same = !!value.request && value.request === status?.request;
    deadline = same ? Math.min(deadline, performance.now() + value.remaining_ms) : performance.now() + value.remaining_ms;
    status = value;
  }
  async function refresh() {
    if (!mounted || !enabled || !owned || busy || reading) return;
    if (deadline && performance.now() >= deadline) { status = { request: owned, scope: reference, remaining_ms: 0, phase: { state: "expired" } }; return; }
    const current = generation; reading = true;
    try { const value = await command<Status>("browser_documents_status"); if (mounted && current === generation) publish(value); }
    catch (e) { if (mounted && current === generation) { status = null; error = String(e); } }
    finally { reading = false; }
  }
  async function run(work: (current:number) => Promise<string | null>) {
    if (!enabled || busy) return;
    busy = true; error = ""; const current = ++generation;
    try {
      const request = await work(current);
      if (!mounted || current !== generation) { if (request) void command("cancel_browser_documents", { request }).catch(() => {}); return; }
      owned = request; status = null; deadline = 0;
      if (request) { const value = await command<Status>("browser_documents_status"); if (mounted && current === generation) publish(value); }
    } catch (e) { if (mounted && current === generation) { status = null; error = String(e); } }
    finally { busy = false; }
  }
  function inspect() { const chosen={...reference}; return run(async current => {
    const previous = owned;
    if (previous) await command("cancel_browser_documents", { request: previous });
    if (!mounted || generation!==current) return null;
    return command<string>("inspect_browser_documents", { reference:chosen });
  }); }
  function select(index: number) {
    const request = owned;
    if (!request || performance.now() >= deadline || status?.phase.state !== "available") return;
    return run(() => command<string>("select_browser_document", { request, index }));
  }
  function cancel() { return run(async () => { if (owned) await command("cancel_browser_documents", { request: owned }); return null; }); }
  onMount(() => { mounted = true; const timer = setInterval(() => void refresh(), 500); return () => { mounted = false; clearInterval(timer); invalidate(); }; });
</script>

<!-- Approved divided settings card and owner-visible native setup metadata. -->
<div class="av-card flex flex-col gap-3 p-3.5">
  <div class="flex items-baseline justify-between gap-3"><span class="av-kicker">Browser document selection</span><span class="av-chip text-amber-200 ring-amber-400/25">Setup metadata</span></div>
  <p class="break-all font-mono text-[11px] text-zinc-300">{origin}<br />{reference.id} / {reference.revision}</p>
  <p class="av-hint">Verify owner management above, then inspect this saved Read scope. This reads tab/document identifiers and URLs only. It does not read page bodies, verify an account, or perform an action. Results expire after five seconds including discovery time.</p>
  <div class="flex justify-end gap-1.5"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || !owned} onclick={cancel}>Cancel</button><button class="av-btn av-btn-primary av-btn-sm" disabled={!enabled || busy} onclick={inspect}>{owned ? "Refresh explicitly" : "Inspect tabs"}</button></div>
  {#if status?.phase.state === "available"}
    <div class="divide-y divide-white/[0.06]">
      {#each status.phase.candidates as candidate, index (candidate.document)}
        <div class="flex items-start gap-3 py-2.5"><div class="min-w-0 flex-1"><p class="break-all text-[12.5px] text-zinc-200">{candidate.url}</p><p class="break-all font-mono text-[10.5px] leading-4 text-zinc-400">Tab {candidate.tab} / window {candidate.window}<br />Document {candidate.document}</p></div><button class="av-btn av-btn-secondary av-btn-sm" disabled={!enabled || busy} onclick={() => select(index)}>Revalidate selection</button></div>
      {:else}<p class="av-hint py-2.5">No eligible top-level documents were observed.</p>{/each}
    </div>
  {:else if status?.phase.state === "selected"}
    <p class="av-hint">Selected document metadata. To read a partial excerpt, grant this saved site in Actions and make an accepted “Read page at ORIGIN” request. Every read revalidates this exact document and browser permission; metadata alone grants no reading authority.</p>
    <p class="break-all font-mono text-[11px] text-zinc-300">{status.phase.candidate.url}<br />Reference {status.phase.identity} / {status.phase.revision}</p>
  {:else if status}
    <p class="av-hint text-amber-200" role="status">{status.phase.state === "waiting" ? "Waiting for the selected extension" : status.phase.state === "expired" ? "Document observation expired. Verify and refresh explicitly." : status.phase.state === "too_many" ? "More than 16 matching tabs; no target was guessed." : "Document metadata unavailable; no page was opened."}</p>
  {/if}
  {#if error}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
</div>
