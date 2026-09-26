<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";

  type Annotation = {
    scenario: string;
    version: number;
    temperature: "cold" | "warm" | "unknown";
    condition: "idle" | "observation" | "background" | "lan_impairment" | "gaming" | "unknown";
  };
  type Coverage = { host: "native" | "controller"; query_process: string; observer_loss: number; evicted: number; collector_starts: number; truncated: boolean; trace_days: number };
  type Summary = { id: string; created_ms: number; annotation: Annotation; turns: number; spans: number; controller_available: boolean; coverage: Coverage[] };
  type Stats = {
    turns: number; spans: number; timed_turns: number; counts: Record<string, number>;
    p50_ms: number | null; p95_ms: number | null; p99_ms: number | null; max_ms: number | null;
    failure_elapsed_p50_ms: number | null; successful_submission_only: boolean; provisional: boolean;
    processes: string[]; deployments: { model: string | null; image: string | null; config: string | null }[];
  };
  type Row = { scope: string; comparable: boolean; issues: string[]; baseline: Stats | null; candidate: Stats | null };
  type Comparison = { baseline: Summary; candidate: Summary; issues: string[]; rows: Row[] };
  type View = { reports: Summary[]; capacity: number; occupied_slots: number; inaccessible_slots: number; comparison: Comparison | null; export_path: string | null };
  type Operation = { kind: "list" | "delete_inaccessible" } | { kind: "compare"; baseline: string; candidate: string } | { kind: "delete" | "export"; id: string };
  let { runtime, turns }: { runtime: Runtime | null; turns: string[] } = $props();
  let reports = $state<Summary[]>([]);
  let capacity = $state(16);
  let occupiedSlots = $state(0);
  let inaccessibleSlots = $state(0);
  let confirmCleanup = $state(false);
  let loaded = $state(false);
  let selected = $state<string[]>([]);
  let scenario = $state("");
  let version = $state(1);
  let temperature = $state<Annotation["temperature"]>("unknown");
  let condition = $state<Annotation["condition"]>("unknown");
  let baseline = $state("");
  let candidate = $state("");
  let comparison = $state<Comparison | null>(null);
  let busy = $state(false);
  let message = $state("");
  let mounted = false;
  let generation = 0;
  let context = "";
  const available = $derived(native && !!runtime && !runtime.locked);
  const uniqueTurns = $derived([...new Set(turns)]);
  const annotationValid = $derived(/^[A-Za-z0-9_-]{1,48}$/.test(scenario) && Number.isInteger(version) && version >= 1 && version <= 65535);
  function contextKey() {
    return JSON.stringify([runtime?.locked, runtime?.connected, runtime?.action_epoch, runtime?.settings.owner_name?.actor]);
  }
  function invalidate() {
    generation++;
    occupiedSlots = 0; inaccessibleSlots = 0; confirmCleanup = false;
    reports = []; loaded = false; selected = []; baseline = ""; candidate = "";
    comparison = null; message = ""; scenario = ""; version = 1; temperature = "unknown"; condition = "unknown";
  }
  $effect(() => {
    const next = contextKey();
    if (next !== context) { context = next; invalidate(); }
  });
  $effect(() => {
    if (selected.some(id => !uniqueTurns.includes(id))) selected = selected.filter(id => uniqueTurns.includes(id));
  });
  onMount(() => {
    mounted = true;
    let stop: (() => void) | undefined;
    if (native) void listen("settings-hidden", invalidate).then(unlisten => {
      if (mounted) stop = unlisten; else unlisten();
    }).catch(e => { if (mounted) message = String(e); });
    return () => { mounted = false; generation++; stop?.(); };
  });
  function current(token: number, key: string) {
    return mounted && available && token === generation && key === contextKey();
  }
  function apply(view: View) {
    reports = view.reports; capacity = view.capacity; occupiedSlots = view.occupied_slots; inaccessibleSlots = view.inaccessible_slots; loaded = true; comparison = view.comparison;
    if (!reports.some(report => report.id === baseline)) baseline = "";
    if (!reports.some(report => report.id === candidate)) candidate = "";
  }
  async function operate(operation: Operation) {
    if (!mounted || !available || busy) return;
    if (operation.kind === "delete_inaccessible" && (!confirmCleanup || inaccessibleSlots < 1)) return;
    confirmCleanup = false;
    busy = true; message = ""; comparison = null;
    const token = ++generation, key = contextKey();
    try {
      const result = await command<View>("trace_cohorts", { operation });
      if (!current(token, key)) return;
      apply(result);
      if (result.export_path) message = `Saved redacted report export: ${result.export_path}`;
      else if (operation.kind === "delete_inaccessible") message = "Inaccessible saved reports deleted. Current reports, source traces and earlier exports remain.";
      else if (operation.kind === "delete") message = "Saved report deleted. Source traces and earlier exports remain.";
      else if (operation.kind === "list") message = `${reports.length} saved reports loaded.`;
    } catch (e) {
      if (current(token, key)) message = `${String(e)}${operation.kind === "delete" || operation.kind === "delete_inaccessible" || operation.kind === "export" ? " Refresh saved reports before retrying." : ""}`;
    } finally { busy = false; }
  }
  async function save() {
    if (!mounted || !available || busy || !loaded || occupiedSlots >= capacity || !annotationValid || selected.length < 1 || selected.length > 64) return;
    const annotation: Annotation = { scenario, version: Number(version), temperature, condition };
    const chosen = [...selected];
    confirmCleanup = false;
    busy = true; message = ""; comparison = null;
    const token = ++generation, key = contextKey();
    let saved: string | null = null;
    try {
      saved = await command<string>("save_trace_cohort", { annotation, turns: chosen });
      if (!current(token, key)) return;
      const result = await command<View>("trace_cohorts", { operation: { kind: "list" } });
      if (!current(token, key)) return;
      apply(result); selected = [];
      message = `Saved report ${saved} from ${chosen.length} selected accepted turns.`;
    } catch (e) {
      if (current(token, key)) message = saved ? `Report ${saved} was saved, but listing failed: ${String(e)}. Refresh saved reports.` : `${String(e)} Refresh saved reports before retrying the save.`;
    } finally { busy = false; }
  }
  function selectTurn(id: string, checked: boolean) {
    if (busy) return;
    selected = checked ? [...new Set([...selected, id])].slice(0, 64) : selected.filter(value => value !== id);
  }
  const ms = (value: number | null) => value === null ? "unavailable" : `${value.toFixed(2)} ms`;
  const annotationLabel = (value: Annotation) => `${value.scenario} v${value.version} / ${value.temperature} / ${value.condition.replaceAll("_", " ")}`;
  const reportLabel = (value: Summary) => `${annotationLabel(value.annotation)} / ${new Date(value.created_ms).toLocaleString()} / ${value.id}`;
</script>

<section class="av-card p-3 mt-3" aria-label="Saved trace comparisons">
  <strong>Saved trace comparisons</strong>
  <p class="av-hint">Save actual retained accepted turns with your own scenario and condition annotations. These labels do not prove the scenario ran or the model was cold or warm. Saving starts no capture, inference or playback.</p>
  <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || busy} onclick={() => operate({ kind: "list" })}>{busy ? "Working…" : "Refresh saved reports"}</button>
  <p class="av-hint">{loaded ? `${occupiedSlots} of ${capacity} report slots used; ${reports.length} accessible and ${inaccessibleSlots} inaccessible.` : "Refresh saved reports before saving or comparing."} Each saved report is limited to 4 MiB. Reports remain until explicitly deleted; a full store refuses new saves. Export any evidence you want to keep before deleting a report.</p>
  <p class="av-hint">Only reports for the current owner, owner revision, device and paired server are listed. Inaccessible reports from earlier bindings also occupy slots; their IDs and contents are hidden.</p>
  {#if loaded && inaccessibleSlots > 0}
    {#if confirmCleanup}
      <div class="av-card p-3 mt-2" role="group" aria-label="Confirm inaccessible report deletion">
        <p class="av-hint">Permanently delete {inaccessibleSlots} inaccessible saved reports? This cannot be undone. Current reports, source traces and earlier exports will remain.</p>
        <div class="flex flex-wrap gap-2">
          <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || busy} onclick={() => operate({ kind: "delete_inaccessible" })}>Confirm permanent deletion</button>
          <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy} onclick={() => confirmCleanup = false}>Cancel</button>
        </div>
      </div>
    {:else}
      <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || busy} onclick={() => confirmCleanup = true}>Delete inaccessible saved reports</button>
    {/if}
  {/if}
  <fieldset disabled={!available || busy}>
    <legend class="av-hint">Owner annotations</legend>
    <div class="flex flex-wrap gap-3">
      <label class="av-hint">Scenario ID <input class="av-input" bind:value={scenario} maxlength="48" placeholder="conversation-idle" aria-describedby="cohort-scenario-help" /></label>
      <label class="av-hint">Scenario version <input class="av-input" type="number" bind:value={version} min="1" max="65535" step="1" /></label>
      <label class="av-hint">Declared state <select bind:value={temperature}><option value="unknown">Unknown</option><option value="cold">Cold</option><option value="warm">Warm</option></select></label>
      <label class="av-hint">Declared condition <select bind:value={condition}><option value="unknown">Unknown</option><option value="idle">Idle</option><option value="observation">Observation</option><option value="background">Background work</option><option value="lan_impairment">LAN impairment</option><option value="gaming">Gaming</option></select></label>
    </div>
    <p id="cohort-scenario-help" class="av-hint">Scenario ID: 1–48 ASCII letters, digits, hyphens or underscores. Version: 1–65,535. Unknown annotations remain unknown.</p>
    <details>
      <summary>Select accepted turns ({selected.length}/64 selected; {uniqueTurns.length} currently listed)</summary>
      {#if uniqueTurns.length === 0}<p class="av-hint">Refresh accepted traces above to list native retained turns.</p>{/if}
      <div class="max-h-64 overflow-y-auto">
        {#each uniqueTurns as turn (turn)}
          <label class="av-hint flex gap-2 break-all"><input type="checkbox" checked={selected.includes(turn)} disabled={busy || (!selected.includes(turn) && selected.length >= 64)} onchange={event => selectTurn(turn, event.currentTarget.checked)} />{turn}</label>
        {/each}
      </div>
    </details>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!loaded || occupiedSlots >= capacity || !annotationValid || selected.length < 1 || selected.length > 64} onclick={save}>Save selected turns</button>
  </fieldset>
  <p class="av-hint">The native owner rechecks selected IDs against current retained records. A selected turn is not proof of an independent scenario repetition. Missing or expired records cannot become zero-duration measurements.</p>
  {#if loaded && reports.length === 0}<p class="av-hint">No saved reports for the current owner and device.</p>{/if}
  {#each reports as report (report.id)}
    <div class="av-card p-2 mt-2">
      <p class="av-hint break-all">{reportLabel(report)}</p>
      <p class="av-hint">{report.turns} selected unique accepted turns · {report.spans} span attempts · controller {report.controller_available ? "available at save" : "unavailable at save"}.</p>
      <details>
        <summary>Saved query coverage</summary>
        {#each report.coverage as coverage}
          <p class="av-hint break-all">{coverage.host}: {coverage.observer_loss} observer losses · {coverage.evicted} expired/evicted records · {coverage.collector_starts} collector starts · {coverage.trace_days}-day retention · query {coverage.truncated ? "truncated" : "not truncated"} · query process {coverage.query_process}.</p>
        {/each}
        <p class="av-hint">Query process identity is separate from the original processes in stage records. Loss and eviction totals are cumulative query coverage counters, not failures attributed to selected turns. An apparently complete query does not prove a complete scenario corpus.</p>
      </details>
      <div class="flex flex-wrap gap-2">
        <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || busy} onclick={() => operate({ kind: "export", id: report.id })}>Export report</button>
        <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || busy} onclick={() => operate({ kind: "delete", id: report.id })}>Delete saved report</button>
      </div>
    </div>
  {/each}
  <fieldset disabled={!available || busy || !loaded} class="mt-3">
    <legend class="av-hint">Choose two saved reports</legend>
    <div class="flex flex-wrap gap-3">
      <label class="av-hint">Baseline <select bind:value={baseline} onchange={() => comparison = null}><option value="">Select baseline</option>{#each reports as report}<option value={report.id}>{reportLabel(report)}</option>{/each}</select></label>
      <label class="av-hint">Candidate <select bind:value={candidate} onchange={() => comparison = null}><option value="">Select candidate</option>{#each reports as report}<option value={report.id}>{reportLabel(report)}</option>{/each}</select></label>
      <button class="av-btn av-btn-ghost av-btn-sm" disabled={!baseline || !candidate || baseline === candidate} onclick={() => operate({ kind: "compare", baseline, candidate })}>Compare reports</button>
    </div>
  </fieldset>
  {#if comparison}
    <div class="mt-3">
      <p class="av-hint break-all">Baseline: {reportLabel(comparison.baseline)} · {comparison.baseline.turns} selected turns.</p>
      <p class="av-hint break-all">Candidate: {reportLabel(comparison.candidate)} · {comparison.candidate.turns} selected turns.</p>
      {#if comparison.issues.length}<p class="av-hint">Comparison limitations:</p><ul class="av-hint">{#each comparison.issues as issue}<li>{issue}</li>{/each}</ul>{/if}
      {#if comparison.rows.length === 0}<p class="av-hint">No observed host/stage rows. This is missing evidence, not zero latency.</p>{/if}
      {#each comparison.rows as row}
        <div class="av-card p-3 mt-2">
          <strong>{row.scope}</strong>
          <p class="av-hint">Not comparable in this report format: historical build/profile provenance is unavailable. Retained descriptive values remain visible.</p>
          {#if row.issues.length}<ul class="av-hint">{#each row.issues as issue}<li>{issue}</li>{/each}</ul>{/if}
          <div class="grid gap-3 md:grid-cols-2">
            {#each [{ label: "Baseline", stats: row.baseline, summary: comparison.baseline }, { label: "Candidate", stats: row.candidate, summary: comparison.candidate }] as side}
              <div>
                <strong>{side.label}</strong>
                {#if side.stats}
                  <p class="av-hint">{side.stats.turns}/{side.summary.turns} selected unique turns observed · {Math.max(0, side.summary.turns - side.stats.turns)} selected turns without this stage · {side.stats.spans} span attempts · {side.stats.timed_turns} timed unique turns{side.stats.provisional ? " · provisional (fewer than 30 timed unique turns)" : ""}.</p>
                  <p class="av-hint">{side.stats.successful_submission_only ? "Successful post-quiet-endpoint submission latency" : "All-outcome elapsed durations"}, weighted by span attempts: attempt-level p50 {ms(side.stats.p50_ms)} / p95 {ms(side.stats.p95_ms)} / p99 {ms(side.stats.p99_ms)} / max {ms(side.stats.max_ms)}.</p>
                  {#if side.stats.successful_submission_only}<p class="av-hint">Non-success attempt-level elapsed p50: {ms(side.stats.failure_elapsed_p50_ms)}. This is time to failure or other non-success, not successful submission latency.</p>{/if}
                  <p class="av-hint">Outcomes: {Object.entries(side.stats.counts).map(([name, count]) => `${name}: ${count}`).join(" · ") || "unavailable"}.</p>
                  <details>
                    <summary>Original process and deployment metadata</summary>
                    <p class="av-hint break-all">Processes: {side.stats.processes.join(", ") || "unavailable"}</p>
                    {#each side.stats.deployments as deployment}<p class="av-hint break-all">Model {deployment.model ?? "unavailable"} · image {deployment.image ?? "unavailable"} · config {deployment.config ?? "unavailable"}</p>{/each}
                    {#if side.stats.deployments.length === 0}<p class="av-hint">Deployment metadata unavailable.</p>{/if}
                  </details>
                {:else}<p class="av-hint">No stage records for these {side.summary.turns} selected turns. Duration unavailable.</p>{/if}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
  {#if message}<p class="av-hint break-all" role="status">{message}</p>{/if}
  <p class="av-hint">Tail percentiles remain descriptive and are not independently qualified at this sample size.</p>
  <p class="av-hint">Percentiles are weighted by measured span attempts; timed unique turns describe coverage, not the percentile denominator. Repeated attempts are not independent scenario runs. Measurements retain their original host and stage; clocks are never subtracted across hosts. Observer loss, eviction and truncation can hide records. Cumulative query losses are possible coverage gaps, not failures attributed to selected turns. Missing build/profile provenance prevents a comparable claim. Submission timing has up to 20 ms reference-block granularity and does not prove acoustic audibility, correctness or usefulness. This panel chooses no winner and grants no release qualification, promotion or rollback.</p>
</section>
