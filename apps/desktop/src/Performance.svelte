<script lang="ts">
  import PerformanceTable from "./PerformanceTable.svelte";
  import AppTimings from "./AppTimings.svelte";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import AcceptedTraces from "./AcceptedTraces.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Row = { operation: string; stage: string; counts: { complete: number; failed: number; withdrawn: number; abandoned: number; missing: number }; timed: number; p50_ms: number | null; p95_ms: number | null; p99_ms: number | null; max_ms: number | null; provisional: boolean };
  type Snapshot = { uptime_seconds: number; capacity: number; retained: number; evicted: number; observer_loss: number; active_outputs: number; silent_diagnostic: boolean; summaries: Row[]; gates: Record<string,number> };
  const names: Record<string, string> = { preview: "Voice preview", greeting: "Startup greeting", activity: "Live activity check", output_session: "Admitted output session", preparation: "Device, session and pairing preparation", remote_ready: "Request to validated audio-ready response", first_submitted_speech: "Session to first submitted speech", final_submission: "Final frame to confirmed submission", estimated_drain: "Submission acknowledgment to estimated drain", activity_session: "Capture and response session", activity_exchange: "Packet send to validated response", capture_age: "Oldest sample age before send" };
  let snapshot = $state<Snapshot | null>(null);
  type Endpoint = { p50: number | null; p95: number | null; measured: number; total: number; failed: number; truncated: boolean; sourceCount: number; reason: string | null };
  let endpoint = $state<Endpoint | null>(null);
  let traceDetails = $state(false);
  const tableRows = $derived((snapshot?.summaries ?? []).map(row => ({ key: `${row.operation}/${row.stage}`, stage: `${names[row.operation] ?? row.operation}: ${names[row.stage] ?? row.stage}`, source: "Unavailable · PC observer", detail: `${row.timed} timed; ${row.counts.complete} complete; ${row.counts.failed} failed; ${row.counts.withdrawn} withdrawn; ${row.counts.abandoned} abandoned; ${row.counts.missing} missing timing`, p50: row.p50_ms, p95: row.p95_ms, p99: row.p99_ms, max: row.max_ms, errors: String(row.counts.failed) })));
  let busy = $state(false);
  let error = $state("");
  let mounted = false;
  let generation = 0;
  let visible = $state(false);
  let eventsReady = $state(false);
  let reopenPending = false;
  function clear() { generation++; snapshot = null; error = ""; endpoint = null; reopenPending = false; }
  function hide() { visible = false; clear(); }
  $effect(() => { if (runtime?.locked) clear(); });
  const ms = (value: number | null) => value === null ? "Unavailable" : `${value.toFixed(1)} ms`;
  async function refresh(explicit = false) {
    if (busy || !mounted || (!visible && !explicit) || !eventsReady || !native || !runtime || runtime.locked) return;
    busy = true; error = "";
    const current = ++generation;
    try { const result = await command<Snapshot>("performance_snapshot"); if (mounted && !runtime?.locked && generation === current) { visible = true; snapshot = result; } }
    catch (e) { if (mounted && !runtime?.locked && generation === current) error = String(e); }
    finally { busy = false; if (reopenPending) { reopenPending = false; void refresh(); } }
  }
  onMount(() => {
    mounted = true; visible = !document.hidden;
    let stopHidden: (() => void) | undefined;
    let stopShown: (() => void) | undefined;
    function reopen(nativeShown = false) {
      if (!mounted || (!nativeShown && document.hidden)) return;
      visible = true;
      if (!eventsReady) return;
      if (busy) reopenPending = true;
      else void refresh();
    }
    const visibilityChanged = () => { if (document.hidden) hide(); else reopen(); };
    document.addEventListener("visibilitychange", visibilityChanged);
    const focused = () => reopen();
    window.addEventListener("focus", focused);
    if (native) void (async () => {
      const hidden = await listen("settings-hidden", hide);
      if (!mounted) { hidden(); return; }
      stopHidden = hidden;
      const shown = await listen("settings-shown", () => reopen(true));
      if (!mounted) { shown(); return; }
      stopShown = shown; eventsReady = true;
      if (visible) void refresh();
    })().catch(() => { stopHidden?.(); stopShown?.(); if (mounted) { eventsReady = false; hide(); error = "Performance view visibility is unavailable. Reload this app window to try again."; } });
    return () => { mounted = false; hide(); eventsReady = false; stopHidden?.(); stopShown?.(); document.removeEventListener("visibilitychange", visibilityChanged); window.removeEventListener("focus", focused); };
  });
</script>

<section class="flex flex-col gap-3" aria-label="Performance">
  <div class="flex items-center gap-3 bg-white/[0.04] px-3 py-2 ring-1 ring-white/10 ring-inset">
    <span class="av-chip text-zinc-300 ring-white/20">OBSERVED</span>
    <span class="flex-1 text-[12px] text-zinc-300">{busy ? "Reading local measurements…" : "Real retained observations. No release gate is established."}</span>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy || !eventsReady || !native || !runtime || runtime.locked} onclick={() => refresh(true)}>Refresh</button>
  </div>
  <div class="grid grid-cols-3 gap-2">
    <div class="av-card flex flex-col gap-1 p-3">
      <span class="text-[11px] text-zinc-400">Endpoint → first submitted speech</span>
      <span class="font-mono text-[18px] text-zinc-50">{endpoint?.p50 == null ? "—" : ms(endpoint.p50)} <span class="text-[11px] text-zinc-400">p50</span></span>
      <span class="font-mono text-[11.5px] text-zinc-300">p95 {endpoint?.p95 == null ? "—" : ms(endpoint.p95)} · n {endpoint && !endpoint.reason ? endpoint.measured : "—"}</span>
      <span class="text-[11px] text-zinc-400">{endpoint ? endpoint.reason ?? "One retained cohort · quiet tail excluded" : "Read accepted traces below"}</span>
    </div>
    {#each ["Speech end → first verified action", "Speech end → verified completion"] as label}
      <div class="av-card flex flex-col gap-1 p-3">
        <span class="text-[11px] text-zinc-400">{label}</span>
        <span class="font-mono text-[18px] text-zinc-50">— <span class="text-[11px] text-zinc-400">p50</span></span>
        <span class="font-mono text-[11.5px] text-zinc-300">p95 — · n —</span>
        <span class="text-[11px] text-zinc-400">Exact endpoint metric unavailable</span>
      </div>
    {/each}
  </div>
  <PerformanceTable rows={tableRows} empty={busy ? "Reading local stage observations…" : snapshot ? "No timed observations yet" : "Local measurements unavailable. Refresh in unlocked Settings."} />
  <div class="flex items-center gap-2">
    <span class="av-hint flex-1">Local preview, greeting and activity stages · ms unless marked s · failures included. No measured bottleneck attribution.</span>
    <button class="av-btn av-btn-secondary av-btn-sm" onclick={() => { traceDetails = true; }}>Traces &amp; compare</button>
  </div>
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
  {#if snapshot?.silent_diagnostic}<p class="av-hint text-amber-200">Silent diagnostic mode: submission is measured before hardware-buffer silencing.</p>{/if}
  <details class="av-card px-3.5 py-2">
    <summary class="cursor-pointer text-[12px] text-zinc-300">Measurement scope &amp; voice outcomes</summary>
    <div class="mt-3 flex flex-col gap-2">
      {#if snapshot}
        <p class="av-hint">{snapshot.retained} / {snapshot.capacity} retained · {snapshot.evicted} evicted · {snapshot.observer_loss} observer losses · {snapshot.active_outputs} active outputs · process age {Math.floor(snapshot.uptime_seconds / 60)} minutes.</p>
        <span class="av-kicker">Voice gate totals · this process</span>
        {#each Object.entries(snapshot.gates) as [reason,count]}<p class="av-hint">{reason.replaceAll("_"," ")}: {count}</p>{/each}
        {#each snapshot.summaries as row}<p class="av-hint">{names[row.operation] ?? row.operation} / {names[row.stage] ?? row.stage}: {row.timed} timed · {row.counts.complete} complete · {row.counts.failed} failed · {row.counts.withdrawn} withdrawn · {row.counts.abandoned} abandoned · {row.counts.missing} missing timing{row.provisional ? " · provisional" : ""}.</p>{/each}
      {/if}
      {#if endpoint}<p class="av-hint">Endpoint card: {endpoint.total} retained attempts across {endpoint.sourceCount} process/deployment identities; {endpoint.failed} failed. {endpoint.reason ?? `${endpoint.measured} completed attempts in one retained cohort; current-build and profile identity remain unproven.`} Other outcomes remain in trace details.{endpoint.truncated ? " The query is truncated; this is a partial retained cohort." : " This is bounded retained-query coverage, not a complete run."}</p>{/if}
      <p class="av-hint">Local stage percentiles include failed and abandoned durations; time to failure is not successful latency. Missing timing is never zero. Gate totals are not accuracy measurements; batch checks are outside local timing summaries. Local stages clear on restart.</p>
      <p class="av-hint">Endpoint card percentiles include successful submissions only, after the quiet tail and minimum capture span, at up to 20 ms block granularity. They do not measure user-speech-end latency, usefulness or audibility. Counts are attempts, not unique turns. All p95/p99 summaries are descriptive; none certifies a target.</p>
      <p class="av-hint">Request timing includes transport and controller work, not pure GPU time. Submitted speech and estimated drain do not prove audible delivery. Model identity is unavailable in local stage summaries. Opening this view starts no recording, playback or model work.</p>
    </div>
  </details>
  <details class="av-card px-3.5 py-2" bind:open={traceDetails}>
    <summary class="cursor-pointer text-[12px] text-zinc-300">Accepted traces, resources &amp; saved comparisons</summary>
    <div class="mt-3"><AcceptedTraces {runtime} onEndpoint={(value) => { endpoint = mounted && visible && !runtime?.locked ? value : null; }} /></div>
  </details>
  <details class="av-card px-3.5 py-2">
    <summary class="cursor-pointer text-[12px] text-zinc-300">App timings &amp; export</summary>
    <div class="mt-3"><AppTimings {runtime} /></div>
  </details>
</section>
