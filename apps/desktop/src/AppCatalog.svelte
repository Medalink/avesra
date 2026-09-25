<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SetupLock from "./SetupLock.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Alias = { id: string; revision: string; phrase: string; name: string; detail: string; available: boolean; last_success_ms: number | null };
  type Candidate = { id: string; name: string; source: string; detail: string; arguments: string; packaged: boolean; working_directory: string | null; selectable: boolean; window_hints: { id: string; class: string; title: string; process_id: number }[]; selected_hint: string | null; hints_complete: boolean };
  type Scan = { candidates: Candidate[]; skipped: number; truncated: boolean; unavailable_sources: string[] };
  let panel = $state<string | null>(null);
  let aliases = $state<Alias[] | null>(null);
  let scan = $state<Scan | null>(null);
  let selected = $state("");
  let phrase = $state("");
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  let mounted = false;
  let generation = 0;
  let context = "";
  function invalidate() { generation++; panel = null; aliases = null; scan = null; selected = ""; }
  const candidate = $derived(scan?.candidates.find(v => v.id === selected));
  const enabled = $derived(native && runtime?.connected && !runtime.locked);
  function observedDate(value: number | null): string | null {
    if (value === null || !Number.isSafeInteger(value) || value <= 0) return null;
    const date = new Date(value);
    return Number.isFinite(date.getTime()) ? date.toLocaleDateString() : null;
  }
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}`;
    if (next !== context) {
      context = next; invalidate();
    }
  });
  async function run(work: (current: number) => Promise<void>, mutation = false) {
    if (busy) return;
    busy = true; error = ""; notice = "";
    const current = ++generation;
    try { await work(current); }
    catch (e) { if (mounted && current === generation) { error = mutation ? `${String(e)} Refresh saved mappings to check whether the change was saved.` : String(e); if (mutation) aliases = null; } }
    finally { busy = false; }
  }
  async function open(current: number): Promise<string | null> {
    if (panel) return panel;
    const value = await command<string>("open_app_catalog");
    if (!mounted || current !== generation) { void command("close_app_catalog", { panel: value }).catch(() => {}); return null; }
    panel = value; return value;
  }
  function refresh() { return run(async current => {
    const active = await open(current); if (!active) return;
    try {
      const value = await command<Alias[]>("app_aliases", { panel: active });
      if (mounted && current === generation) aliases = value;
    } catch (e) {
      if (current === generation) { aliases = null; panel = null; scan = null; selected = ""; void command("close_app_catalog", { panel: active }).catch(() => {}); }
      throw e;
    }
  }); }
  function discover() { return run(async current => {
    const active = await open(current); if (!active) return;
    scan = null; selected = "";
    const value = await command<Scan>("scan_app_catalog", { panel: active });
    if (mounted && current === generation) scan = value;
  }); }
  function folder() { return run(async current => {
    if (!panel || !candidate) return;
    const value = await command<Candidate>("choose_app_folder", { panel, candidate: candidate.id });
    if (mounted && current === generation && scan) scan = { ...scan, candidates: scan.candidates.map(v => v.id === value.id ? value : v) };
  }); }
  function executable() { return run(async current => {
    const active = await open(current); if (!active) return;
    const value = await command<Candidate>("choose_app_executable", { panel: active });
    if (mounted && current === generation) {
      scan = scan ? { ...scan, candidates: [...scan.candidates, value] } : { candidates: [value], skipped: 0, truncated: false, unavailable_sources: [] };
      selected = value.id;
    }
  }); }
  function observeWindows() { return run(async current => {
    if (!panel || !candidate) return;
    const value = await command<Candidate>("observe_app_windows", { panel, candidate: candidate.id });
    if (mounted && current === generation && scan) scan = { ...scan, candidates: scan.candidates.map(v => v.id === value.id ? value : v) };
  }); }
  function chooseWindow(hint: string) { return run(async current => {
    if (!panel || !candidate) return;
    const value = await command<Candidate>("choose_app_window", { panel, candidate: candidate.id, hint: hint || null });
    if (mounted && current === generation && scan) scan = { ...scan, candidates: scan.candidates.map(v => v.id === value.id ? value : v) };
  }); }
  function remember() { return run(async current => {
    if (!panel || !candidate) return;
    const value = await command<Alias[]>("remember_app", { panel, candidate: candidate.id, phrase });
    if (mounted && current === generation) { aliases = value; notice = "Application name saved. Action permissions remain separate."; phrase = ""; }
  }, true); }
  function forget(alias: Alias) { return run(async current => {
    if (!panel) return;
    const value = await command<Alias[]>("forget_app_alias", { panel, id: alias.id, revision: alias.revision });
    if (mounted && current === generation) { aliases = value; notice = "Name forgotten. Existing tasks and permissions are unchanged."; }
  }, true); }
  onMount(() => {
    mounted = true;
    let unlisten: (() => void) | undefined;
    if (native) void (async () => {
      const stop = await listen("settings-hidden", invalidate);
      if (!mounted) { stop(); return; }
      unlisten = stop;
      if (runtime?.connected && !runtime.locked) await refresh();
    })().catch(e => { if (mounted) error = String(e); });
    return () => {
      mounted = false; generation++; unlisten?.();
      if (panel) void command("close_app_catalog", { panel }).catch(() => {});
    };
  });
</script>

<section class="flex flex-col gap-2">
  <div class="flex items-center justify-between">
    <span class="av-kicker">Learned names</span>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={refresh}>Refresh</button>
  </div>
  <!-- Approved Settings.dc.html 710–730: fixed phrase column, divided rows. -->
  <div class="av-card flex flex-col divide-y divide-white/[0.06]">
    {#each aliases ?? [] as alias (alias.id)}
      <div class="flex items-center gap-3 px-3.5 py-2.5">
        <span class="w-[120px] shrink-0 truncate font-mono text-[12px] text-av-300" title={alias.phrase}>“{alias.phrase}”</span>
        <div class="flex min-w-0 flex-1 flex-col">
          <span class="truncate text-[12.5px] text-zinc-100">{alias.name}</span>
          <span class="av-hint truncate" title={alias.detail}>{alias.detail}</span>
        </div>
        <span class="shrink-0 text-[11px] text-zinc-400" title={observedDate(alias.last_success_ms) ? "Your last native-observed successful opening; current app state is not checked" : "Last successful launch is unavailable"}>{observedDate(alias.last_success_ms) ?? (alias.available ? "Unverified" : "Unavailable")}</span>
        <button class="av-iconbtn size-7 hover:bg-red-500/10 hover:text-red-300" disabled={!enabled || busy} onclick={() => forget(alias)} aria-label={`Forget ${alias.phrase}`} title="Forget">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"></path></svg>
        </button>
      </div>
    {:else}
      <p class="av-hint px-3.5 py-2.5">{aliases ? "No application names have been saved." : "Saved application names are unavailable. Connect Spark and refresh."}</p>
    {/each}
  </div>
  <p class="av-hint">Remember an exact application for a name. This does not allow screen observation or grant permission to launch it.</p>
  <div class="av-card flex flex-col gap-3 p-3.5">
    <div class="flex items-center justify-between gap-3">
      <span class="text-[12.5px] text-zinc-100">Choose an application</span>
      <div class="flex items-center gap-1.5"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={executable}>Choose executable</button>
      <button class="av-btn av-btn-secondary av-btn-sm" disabled={!enabled || busy} onclick={discover}>{busy ? "Working…" : "Find applications"}</button></div>
    </div>
    {#if scan}
      <label class="flex flex-col gap-1.5"><span class="av-hint">Native application entry</span>
        <select class="av-input av-select w-full" bind:value={selected} disabled={busy}>
          <option value="">Choose an exact entry</option>
          {#each scan.candidates as item}<option value={item.id}>{item.name} · {item.source.replaceAll("_", " ")} · {item.detail}</option>{/each}
        </select>
      </label>
      <p class="av-hint">{scan.candidates.length} entries · {scan.skipped} skipped{scan.truncated ? " · scan limit reached" : ""}. Choices expire after two minutes.</p>
      {#if scan.unavailable_sources.length}<p class="av-hint text-amber-200">Discovery incomplete. Unavailable sources: {scan.unavailable_sources.join(", ")}.</p>{/if}
    {/if}
    {#if candidate}
      <div class="flex flex-col gap-1 text-[12px] text-zinc-300">
        <span class="break-all">{candidate.detail}</span>
        {#if candidate.packaged}<span class="av-hint">Exact installed package version. A working folder and executable arguments do not apply.</span>
        {:else}<span class="break-all">Arguments: {candidate.arguments || "None"}</span>
        <span class="break-all">Working folder: {candidate.working_directory || "Choose a folder"}</span>{/if}
      </div>
      {#if !candidate.selectable}<p class="av-hint text-amber-200">This native registration requires an environment that is not supported yet.</p>{/if}
      {#if !candidate.packaged && !candidate.working_directory}<button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={busy || !enabled || !candidate.selectable} onclick={folder}>Choose working folder</button>{/if}
      <div class="flex flex-col gap-2">
        <div class="flex items-center justify-between gap-3"><span class="av-hint">Expected application window</span><button class="av-btn av-btn-secondary av-btn-sm" disabled={busy || !enabled || !candidate.selectable || (!candidate.packaged && !candidate.working_directory)} onclick={observeWindows}>Observe open windows</button></div>
        <p class="av-hint">Open the chosen app yourself, then select its main window. This observes only matching application windows and does not focus them. Window choices expire after 30 seconds.</p>
        {#if candidate.window_hints.length}
          <select class="av-input av-select w-full" aria-label="Expected application window" value={candidate.selected_hint ?? ""} disabled={busy} onchange={e => chooseWindow(e.currentTarget.value)}>
            <option value="">No window hint · opening remains unavailable</option>
            {#each candidate.window_hints as hint}<option value={hint.id}>{hint.title || "Untitled window"} · {hint.class} · PID {hint.process_id}</option>{/each}
          </select>
          {#if !candidate.hints_complete}<p class="av-hint text-amber-200">Window discovery was incomplete. Only the listed native observations can be selected.</p>{/if}
        {:else}<p class="av-hint">No eligible window is selected. Saving a name alone cannot verify opening.</p>{/if}
      </div>
      <label class="flex flex-col gap-1.5"><span class="av-hint">Name to remember</span><input class="av-input w-full" bind:value={phrase} maxlength="64" placeholder="e.g. editor" disabled={busy} /></label>
      <SetupLock {runtime} purpose="application mapping" />
      <p class="av-hint">Create the local owner in People & Voice ID first. Verify here after reviewing the entry, then choose Remember. A fresh verification is required to forget a mapping.</p>
      <button class="av-btn av-btn-primary av-btn-sm self-end" disabled={busy || !enabled || !candidate.selectable || (!candidate.packaged && !candidate.working_directory) || !phrase.trim()} onclick={remember}>Remember application</button>
    {:else if aliases?.length}
      <SetupLock {runtime} purpose="application mapping" />
    {/if}
  </div>
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
  {#if notice}<p class="av-hint" role="status">{notice}</p>{/if}
</section>
