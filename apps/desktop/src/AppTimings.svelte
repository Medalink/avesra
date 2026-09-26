<script lang="ts">
  import PerformanceTable from "./PerformanceTable.svelte";
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
      return { key, row: rows[0], outcome: rows[0].outcome, count: rows.length, complete: rows.filter(r => r.outcome === "complete").length, p50: percentile(.5), p95: percentile(.95), p99: percentile(.99), max: values[values.length-1] };
    });
  });
  const tableRows = $derived(groups.map(group => ({ key: group.key, stage: `${group.row.operation.name ?? group.row.operation.kind} / ${group.row.stage} / ${group.outcome}`.replaceAll("_", " "), source: `${group.row.origin.kind === "frontend" ? `UI ${group.row.origin.window ?? "window"}` : "Native"} · this PC`, detail: `${group.key}; ${group.count} attempts; separate outcome and page-clock cohort`, p50: group.p50, p95: group.p95, p99: group.p99, max: group.max, errors: group.outcome === "failed" ? String(group.count) : "0" })));
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
  <div class="flex items-center justify-between gap-2">
    <span class="av-kicker">App timings</span>
    <div class="flex gap-2"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!native || busy || !runtime || runtime.locked} onclick={() => read()}>{busy ? "Reading…" : "Read timings"}</button><button class="av-btn av-btn-secondary av-btn-sm" disabled={!native || busy || !runtime || runtime.locked} onclick={() => read(true)}>Export</button></div>
  </div>
  <p class="av-hint">This Windows user's installation · current native process · each outcome and page clock kept separate. These measurements do not prove work on Spark.</p>
  <PerformanceTable rows={tableRows} label="App operation timings" empty={busy ? "Reading retained app timings…" : snapshot ? "No retained timings from this process." : "Read timings to inspect retained app observations."} />
  <p class="av-hint">Successful latency and time to failure remain separate. Descriptive p95/p99 values do not certify release targets.</p>
  <details>
    <summary class="cursor-pointer text-[12px] text-zinc-400">Cohorts, collection &amp; retention</summary>
    <div class="mt-2 flex flex-col gap-2">
      <p class="av-hint">This page timing collector: {frontend.registration}. {frontend.pending} buffered | {frontend.unreported_loss} unreported losses. A failed page registration requires reopening this app window; no automatic observer retry occurs.</p>
      {#if snapshot}
        <p class="av-hint">{snapshot.records.length} / {snapshot.capacity} retained | {snapshot.retention_days} days maximum | {snapshot.evicted} evicted | {snapshot.native_observer_loss} native observer losses | {snapshot.frontend_reported_loss} frontend-reported losses.</p>
        <p class="av-hint">The export includes retained older processes. Exact build fingerprint and profile identity are unavailable; package version alone cannot establish comparable builds. Model attribution is unavailable for these local operations; the table identifies their observer origin.</p>
        {#each groups as group (group.key)}<p class="av-hint break-all">{group.key.replaceAll("_", " ")}: {group.count} {group.outcome} attempts{group.count < 30 ? " | provisional" : ""}.</p>{/each}
      {/if}
      <p class="av-hint">Each outcome has separate percentiles. Failure and abandonment timings are time to that outcome, not successful latency. UI waits are not worker retirement; next-frame observations are not physical display presentation. All percentiles are descriptive retained-sample summaries; sample count never certifies a tail-latency target. Internal timing coverage remains partial.</p>
    </div>
  </details>
  {#if message}<p class="av-hint break-all" role="status">{message}</p>{/if}
</section>
