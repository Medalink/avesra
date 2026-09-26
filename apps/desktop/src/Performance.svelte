<script lang="ts">
  import AppTimings from "./AppTimings.svelte";
  import { onMount } from "svelte";
  import AcceptedTraces from "./AcceptedTraces.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Row = { operation: string; stage: string; counts: { complete: number; failed: number; withdrawn: number; abandoned: number; missing: number }; timed: number; p50_ms: number | null; p95_ms: number | null; p99_ms: number | null; max_ms: number | null; provisional: boolean };
  type Snapshot = { uptime_seconds: number; capacity: number; retained: number; evicted: number; observer_loss: number; active_outputs: number; silent_diagnostic: boolean; summaries: Row[]; gates: Record<string,number> };
  const names: Record<string, string> = { preview: "Voice preview", greeting: "Startup greeting", activity: "Live activity check", output_session: "Admitted output session", preparation: "Device, session and pairing preparation", remote_ready: "Request to validated audio-ready response", first_submitted_speech: "Session to first submitted speech", final_submission: "Final frame to confirmed submission", estimated_drain: "Submission acknowledgment to estimated drain", activity_session: "Capture and response session", activity_exchange: "Packet send to validated response", capture_age: "Oldest sample age before send" };
  let snapshot = $state<Snapshot | null>(null);
  let busy = $state(false);
  let error = $state("");
  let mounted = false;
  let generation = 0;
  $effect(() => { if (runtime?.locked) { generation++; snapshot = null; } });
  const ms = (value: number | null) => value === null ? "Unavailable" : `${value.toFixed(1)} ms`;
  async function refresh() {
    if (busy || !native || !runtime || runtime.locked) return;
    busy = true; error = "";
    const current = ++generation;
    try { const result = await command<Snapshot>("performance_snapshot"); if (mounted && generation === current) snapshot = result; }
    catch (e) { if (mounted && generation === current) error = String(e); }
    finally { busy = false; }
  }
  onMount(() => { mounted = true; void refresh(); return () => { mounted = false; generation++; }; });
</script>

<section class="section">
  <div class="flex items-center justify-between"><span class="av-kicker">Observed performance</span><button class="av-btn av-btn-ghost av-btn-sm" disabled={busy || !native || !runtime || runtime.locked} onclick={refresh}>{busy ? "Reading…" : "Refresh"}</button></div>
  <p class="av-hint">Actual voice, preview, greeting and activity measurements from this companion process. Opening this view starts no recording, playback or model request.</p>
  {#if snapshot}
    <p class="av-hint">{snapshot.retained} / {snapshot.capacity} retained stage observations · {snapshot.evicted} evicted · {snapshot.observer_loss} observer capacity losses · {snapshot.active_outputs} active output sessions. Process age: {Math.floor(snapshot.uptime_seconds / 60)} minutes.</p>
    {#if snapshot.silent_diagnostic}<p class="av-hint text-amber-200">Silent diagnostic mode: output is silenced at the hardware buffer; any submission measurements are taken before silencing.</p>{/if}
    <div class="av-card p-4"><span class="av-kicker">Voice gate totals · this process</span><p class="av-hint">Enum-only outcomes; no rejected utterance identifiers or content are retained. These counts are not labeled accuracy measurements.</p>{#each Object.entries(snapshot.gates) as [reason,count]}<p class="av-hint">{reason.replaceAll("_"," ")}: {count}</p>{/each}</div>
    {#if snapshot.summaries.length === 0}<div class="empty"><h2>No timed observations yet</h2><p class="av-hint">Voice gate totals above include observed preacceptance outcomes; batch checks are outside these local timing summaries.</p></div>{/if}
    {#each snapshot.summaries as row}
      <div class="av-card p-4">
        <span class="av-kicker">{names[row.operation] ?? row.operation}</span><h3 class="mt-1">{names[row.stage] ?? row.stage}</h3>
        <div class="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-4">
          {#each [["Median", row.p50_ms], ["p95", row.p95_ms], ["p99", row.p99_ms], ["Maximum", row.max_ms]] as metric}<div><span class="av-hint">{metric[0]}</span><p class="font-mono text-sm">{ms(metric[1] as number | null)}</p></div>{/each}
        </div>
        <p class="av-hint mt-2">{row.timed} timed observations · {row.counts.complete} complete · {row.counts.failed} failed · {row.counts.withdrawn} withdrawn · {row.counts.abandoned} abandoned · {row.counts.missing} missing timing{row.provisional ? " · provisional (fewer than 30 timed observations)" : ""}.</p>
      </div>
    {/each}
  {:else}<p class="av-hint">Refresh in unlocked Settings to read local measurements.</p>{/if}
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
  <p class="av-hint">Percentiles include measured failures and abandoned operations, not only successes. A failed duration is time to failure. Missing timing is never zero. Recent observations are bounded and clear on restart.</p>
  <p class="av-hint">Submitted speech and estimated drain do not prove audible delivery. Request timing includes transport and controller work; it is not pure GPU inference time. Owner accuracy, accepted-request latency, server queue time and release qualification remain unmeasured here.</p>
</section>
<AcceptedTraces {runtime} />

<AppTimings {runtime} />
