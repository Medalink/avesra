<script lang="ts">
  import { timingAvailability } from "./app-timing";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Row = { process: string; package_version: string; build_fingerprint: string | null; operation: { kind: string; name?: string }; stage: string; outcome: string; origin: { kind: string; window?: string; page?: string }; duration_us: number };
  type Snapshot = { version: number; scope: string; process: string; capacity: number; retention_days: number; evicted: number; native_observer_loss: number; frontend_reported_loss: number; records: Row[] };
  let snapshot = $state<Snapshot | null>(null), busy = $state(false), message = $state("");
  let frontend = $state(timingAvailability());
  let mounted = false, generation = 0;
  function clear() { generation++; snapshot = null; message = ""; }
  $effect(() => { if (runtime?.locked) clear(); });
  const groups = $derived.by(() => {
    const grouped = new Map<string, Row[]>();
    for (const row of snapshot?.records ?? []) {
      if (row.process !== snapshot?.process) continue;
      const key = [row.origin.kind, row.origin.window ?? "", row.origin.page ?? "", row.operation.kind, row.operation.name ?? "", row.stage, row.outcome].join(" / ");
      const rows = grouped.get(key) ?? []; rows.push(row); grouped.set(key, rows);
    }
    return [...grouped].map(([key, rows]) => {
      const values = rows.map(row => row.duration_us / 1000).sort((a,b) => a-b);
      const percentile = (p: number) => values[Math.ceil(values.length * p) - 1];
      return { key, outcome: rows[0].outcome, count: rows.length, complete: rows.filter(r => r.outcome === "complete").length, p50: percentile(.5), p95: percentile(.95), p99: percentile(.99), max: values[values.length-1] };
    });
  });
  async function read(exportFile = false) {
    if (!native || !runtime || runtime.locked || busy) return;
    busy = true; message = ""; frontend = timingAvailability(); const token = ++generation;
    try {
      if (exportFile) { const path = await command<string>("export_app_timings"); if (mounted && token === generation) message = `Export saved: ${path}`; }
      else { const value = await command<Snapshot>("app_timing_snapshot"); if (mounted && token === generation) snapshot = value; }
    } catch (e) { if (mounted && token === generation) message = String(e); }
    finally { busy = false; }
  }
  onMount(() => {
    mounted = true; let stop: (() => void) | undefined;
    if (native) void listen("settings-hidden", clear).then(value => { if (mounted) stop = value; else value(); });
    return () => { mounted = false; clear(); stop?.(); };
  });
</script>
<section class="section">
  <span class="av-kicker">App timings</span>
  <p class="av-hint">Local measurements for this Windows user's installation. These are separate from accepted conversations and do not prove work on Spark.</p>
  <div class="flex gap-2"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!native || busy || !runtime || runtime.locked} onclick={() => read()}>Read app timings</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={!native || busy || !runtime || runtime.locked} onclick={() => read(true)}>Export retained timings</button></div>
  <p class="av-hint">This page timing collector: {frontend.registration}. {frontend.pending} buffered � {frontend.unreported_loss} unreported losses. A failed page registration requires reopening this app window; no automatic observer retry occurs.</p>
  {#if snapshot}
    <p class="av-hint">{snapshot.records.length} / {snapshot.capacity} retained · {snapshot.retention_days} days maximum · {snapshot.evicted} evicted · {snapshot.native_observer_loss} native observer losses · {snapshot.frontend_reported_loss} frontend-reported losses.</p>
    <p class="av-hint">Rows below show this native process only, separating frontend page clocks. The export also contains retained older processes. Exact build fingerprint and profile identity are unavailable; package version alone cannot establish comparable builds.</p>
    {#if groups.length === 0}<p class="av-hint">No retained timings from this process.</p>{/if}
    {#each groups as group (group.key)}
      <div class="av-card p-3"><p class="av-hint break-all">{group.key.replaceAll("_", " ")}</p><p class="font-mono text-xs">p50 {group.p50.toFixed(1)} · p95 {group.p95.toFixed(1)} · p99 {group.p99.toFixed(1)} · max {group.max.toFixed(1)} ms</p><p class="av-hint">{group.count} measured attempts · {group.complete} complete · {group.count-group.complete} other outcomes{group.count<30 ? " · provisional" : ""}</p></div>
    {/each}
  {/if}
  <p class="av-hint">Each outcome has separate percentiles. Failure and abandonment timings are time to that outcome, not successful latency. UI waits are not worker retirement; next-frame observations are not physical display presentation. All percentiles are descriptive retained-sample summaries, including p95/p99; sample count never certifies a tail-latency target. Internal timing coverage remains partial.</p>
  {#if message}<p class="av-hint break-all" role="status">{message}</p>{/if}
</section>
