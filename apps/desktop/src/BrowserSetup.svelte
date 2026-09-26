<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import BrowserScopes from "./BrowserScopes.svelte";
  import SetupLock from "./SetupLock.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Pairing = { id: string; revision: string };
  type Confirmation = { installation: string; connection: string; session: string; challenge: string; extension: string; comparison: string };
  type Status = { automatic_reconnect: boolean; attempt: string | null; state: string; browser_app: string | null; browser_revision: string | null; browser_label: string | null; pending: Confirmation | null; selection: string | null; action_epoch: number; mode_allows_actions: boolean; scope: {state: string; reference: Pairing; origin?: string; operations?: string[]; remaining_ms?: number} | null };
  type Alias = { id: string; phrase: string; name: string; target: string; target_revision: string; available: boolean };
  type Saved = { pairing: Pairing; available: boolean; binding: null | { label: string; installation: string; browser_app: string; browser_revision: string } };
  type Selected = { revision: string; selected: null | { revision: string; pairing: Pairing; actor: string }; binding: Saved["binding"]; available: boolean };
  let expanded = $state(false), busy = $state(false), error = $state("");
  let status = $state<Status | null>(null), aliases = $state<Alias[] | null>(null), saved = $state<Saved[] | null>(null);
  let phrase = $state(""), panel = $state<string | null>(null);
  let ownedAttempt = $state<string | null>(null);
  let selections = $state<Selected[] | null>(null);
  let lifetimeReady = $state(false);
  let mounted = false, generation = 0, refreshing = false, context = "";
  const enabled = $derived(native && lifetimeReady && !!runtime?.connected && !runtime.locked);
  const reportedActive = $derived(!!status?.attempt && ["preparing", "waiting_for_extension", "awaiting_owner", "saving", "awaiting_persistence_proof", "authenticating", "authenticated_no_scopes", "closing"].includes(status.state));
  const active = $derived(!!ownedAttempt || reportedActive);
  const unavailable = $derived(status?.state === "unavailable_refresh_saved_pairings" || (!status && !!ownedAttempt));
  const stateLabel = $derived(status?.state === "authenticated_no_scopes" ? status.selection ? "Selected · paired" : "Paired" : unavailable ? "Unavailable" : reportedActive ? "Pairing session" : !status ? "Not inspected" : "Not connected");
  function invalidate() {
    const attempt = ownedAttempt; ownedAttempt = null;
    generation++; status = null; aliases = null; saved = null; selections = null; phrase = "";
    const owned = panel; panel = null;
    if (native && owned) void command("close_app_catalog", { panel: owned }).catch(() => {});
    if (native && attempt) void command("release_browser_management", { attempt }).catch(() => {});
  }
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}`;
    if (next !== context) { context = next; invalidate(); }
  });
  async function refreshStatus() {
    if (!mounted || !expanded || !enabled || busy || refreshing) return;
    const current = generation; refreshing = true;
    try { const next = await command<Status>("browser_pairing_status"); if (mounted && current === generation) status = next; }
    catch (e) { if (mounted && current === generation) { status = null; error = String(e); } }
    finally { refreshing = false; }
  }
  async function run(work: (current: number) => Promise<void>) {
    if (busy || !enabled) return;
    busy = true; error = ""; const current = ++generation;
    try { await work(current); }
    catch (e) { if (mounted && current === generation) error = String(e); }
    finally { busy = false; }
  }
  function refresh() { return run(async current => {
    try {
      if (!panel) {
        const owned = await command<string>("open_app_catalog");
        if (!mounted || current !== generation) { void command("close_app_catalog", { panel: owned }).catch(() => {}); return; }
        panel = owned;
      }
      const selected = panel;
      const names = await command<Alias[]>("app_aliases", { panel: selected });
      if (!mounted || current !== generation) return;
      aliases = names;
      if (!names.some(v => v.phrase === phrase && v.available)) phrase = "";
    } catch (e) {
      if (!mounted || current !== generation) return;
      const owned = panel; panel = null; aliases = null; phrase = ""; error = String(e);
      if (owned) void command("close_app_catalog", { panel: owned }).catch(() => {});
    }
    const next = await command<Status>("browser_pairing_status");
    if (!mounted || current !== generation) return;
    status = next;
    // Roster reads have explicit admission; no automatic storage retry loop.
    await refreshSaved(current);
  }); }
  async function refreshSaved(current: number) {
    try {
      const records = await command<Saved[]>("saved_browser_pairings");
      if (!mounted || current !== generation) return;
      const selected = await command<Selected[]>("browser_selections");
      if (mounted && current === generation) { saved = records; selections = selected; }
    } catch (e) { if (mounted && current === generation) { saved = null; selections = null; error = String(e); } }
  }
  function begin() { return run(async current => {
    saved = null; selections = null;
    const attempt = await command<string>("begin_browser_pairing", { phrase });
    if (!mounted || current !== generation) { void command("release_browser_management", { attempt }).catch(() => {}); return; }
    ownedAttempt = attempt;
    const next = await command<Status>("browser_pairing_status");
    if (mounted && current === generation) status = next;
  }); }
  function approve() { return run(async current => {
    const pending = status?.pending, attempt = status?.attempt;
    if (!pending || !attempt) return;
    await command("approve_browser_pairing", { attempt, challenge: pending.challenge });
    const next = await command<Status>("browser_pairing_status");
    if (mounted && current === generation) status = next;
  }); }
  function cancel() { return run(async current => {
    const attempt = ownedAttempt ?? status?.attempt;
    if (attempt) await command("cancel_browser_pairing", { attempt });
    if (mounted && current === generation) { status = null; ownedAttempt = null; }
  }); }
  function revoke(record: Saved) { return run(async current => {
    saved = null; selections = null;
    await command("revoke_browser_pairing", { pairing: record.pairing });
    if (!mounted || current !== generation) return;
    status = null; await refreshSaved(current);
  }); }
  function select(record: Saved) { return run(async current => {
    selections = null;
    await command("select_browser_pairing", { pairing: record.pairing });
    if (mounted && current === generation) await refreshSaved(current);
  }); }
  function connectSelected(record: Selected) { return run(async current => {
    const attempt = await command<string>("connect_selected_browser", { revision: record.revision });
    if (!mounted || current !== generation) { void command("release_browser_management", { attempt }).catch(() => {}); return; }
    ownedAttempt = attempt; saved = null; selections = null;
    const next = await command<Status>("browser_pairing_status");
    if (mounted && current === generation) status = next;
  }); }
  function clearSelection(record: Selected) { return run(async current => {
    selections = null;
    await command("clear_browser_selection", { revision: record.revision });
    if (mounted && current === generation) await refreshSaved(current);
  }); }
  async function toggle() {
    if (expanded) { expanded = false; invalidate(); }
    else { expanded = true; await refresh(); }
  }
  onMount(() => {
    mounted = true; let unlisten: (() => void) | undefined;
    if (native) void listen("settings-hidden", () => { expanded = false; invalidate(); }).then(value => { if (mounted) { unlisten = value; lifetimeReady = true; } else value(); }).catch(() => { if (mounted) { lifetimeReady = false; error = "Settings lifetime notifications are unavailable; browser setup is closed."; expanded = false; invalidate(); } });
    const timer = setInterval(() => void refreshStatus(), 1000);
    return () => {
      mounted = false; clearInterval(timer); unlisten?.(); invalidate();
    };
  });
</script>

{#if status && !status.automatic_reconnect}
  <p class="text-[12px] text-zinc-400">Automatic browser connection is off in Avesra. Connect the saved browser below to enable it. If you also disconnected the extension, use Connect in its popup.</p>
{/if}

<!-- Windows PC card: approved Settings.dc.html 478–495. -->
<div class="av-card flex flex-col gap-3 p-3.5">
  <div class="flex items-center gap-2.5">
    <span class="relative inline-flex size-2 bg-av-500"></span>
    <span class="text-[13px] font-medium text-zinc-50">Windows PC</span>
    <span class="text-[12px] text-zinc-400">This device · desktop client</span>
    <span class="flex-1"></span><span class="av-chip text-zinc-300 ring-white/15">Local</span>
  </div>
  <div class="flex flex-wrap items-center gap-1.5">
    <span class="av-chip text-zinc-300 ring-white/15">Overlay</span><span class="av-chip text-zinc-300 ring-white/15">Local controls</span>
    <span class="av-chip {status?.state === 'authenticated_no_scopes' ? 'bg-av-500/10 text-av-300 ring-av-500/40' : reportedActive || unavailable || !status ? 'bg-amber-400/10 text-amber-200 ring-amber-400/25' : 'bg-red-400/10 text-red-300 ring-red-400/25'}">Browser · {stateLabel}</span>
  </div>
  <div class="flex items-center gap-2.5"><p class="av-hint flex-1">Client inference is disabled. Browser pairing grants no page or action scope.</p><button class="av-btn av-btn-secondary av-btn-sm" disabled={!enabled || busy} onclick={toggle}>{expanded ? "Close browser setup" : "Manage browser"}</button></div>
  {#if !lifetimeReady && error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
</div>
{#if expanded}
  <div class="flex flex-col gap-3">
    <SetupLock {runtime} purpose="browser pairing, selection, site scopes and revocation" />
    <div class="av-card flex flex-col gap-3 p-3.5">
      <div class="flex items-baseline justify-between gap-3"><span class="av-kicker">Selected browser installation</span><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || active} onclick={refresh}>Refresh</button></div>
      <label class="av-label" for="browser-app">Owner-selected application name</label>
      <select id="browser-app" class="av-input av-select" bind:value={phrase} disabled={!enabled || busy || active || !aliases?.length}>
        <option value="">Choose a saved application</option>{#each aliases ?? [] as alias (alias.id)}<option value={alias.phrase} disabled={!alias.available}>{alias.phrase} · {alias.name}</option>{/each}
      </select>
      <p class="av-hint">Save its application mapping under Observation &amp; actions first. This selection is configuration; it does not prove the running browser, profile path or account.</p>
      <div class="flex justify-end gap-1.5"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!active || busy} onclick={cancel}>Disconnect / cancel</button><button class="av-btn av-btn-primary av-btn-sm" disabled={!enabled || busy || active || !phrase} onclick={begin}>Start pairing session</button></div>
      <p class="av-hint" role="status">{status?.state === "waiting_for_extension" ? "Connect from the extension popup now. Pairing expires after 45 seconds." : stateLabel}</p>
    </div>
    {#if status?.pending && status.state === "awaiting_owner"}
      <!-- Exact-scope approval layout: approved Settings.dc.html 679–705. -->
      <div class="av-card p-3.5 ring-1 ring-amber-400/25 ring-inset">
        <div class="flex items-start gap-3"><div class="flex min-w-0 flex-1 flex-col gap-0.5"><span class="text-[13px] font-medium text-zinc-50">Pair this extension installation</span><span class="av-hint">Compare the six digits in the extension popup before approving.</span></div><span class="av-chip text-amber-200 ring-amber-400/25">Pending</span></div>
        <div class="mt-2.5 break-all bg-black/30 px-3 py-2 font-mono text-[11px] leading-4 text-zinc-300">
          <strong class="block text-[22px] leading-7 tracking-widest text-zinc-50">{status.pending.comparison}</strong>
          <span class="block">Installation: {status.pending.installation}</span><span class="block">Extension: {status.pending.extension}</span>
          <span class="block">Application: {status.browser_label} · {status.browser_app}</span><span class="block">Revision: {status.browser_revision}</span>
        </div>
        <div class="mt-2.5 grid grid-cols-3 gap-3 text-[11.5px] leading-4">
          <div class="flex flex-col gap-0.5"><span class="text-[10.5px] text-zinc-400">Expected effect</span><span class="text-zinc-200">Save one installation credential</span></div>
          <div class="flex flex-col gap-0.5"><span class="text-[10.5px] text-zinc-400">Scope</span><span class="text-zinc-200">No page reading or actions</span></div>
          <div class="flex flex-col gap-0.5"><span class="text-[10.5px] text-zinc-400">Recovery</span><span class="text-zinc-200">Revoke the exact saved revision</span></div>
        </div>
        <div class="mt-3 flex items-center justify-end gap-1.5"><button class="av-btn av-btn-ghost av-btn-sm" disabled={busy} onclick={cancel}>Decline</button><button class="av-btn av-btn-primary av-btn-sm" disabled={!enabled || busy} onclick={approve}>Approve once</button></div>
      </div>
    {/if}
    <BrowserScopes {runtime} {status} />
    {#if saved !== null}
      <div class="av-card divide-y divide-white/[0.06]">
        {#each saved as record (`${record.pairing.id}:${record.pairing.revision}`)}
          <div class="flex items-start gap-3 px-3.5 py-2.5"><div class="min-w-0 flex-1"><span class="text-[12.5px] text-zinc-200">{record.binding?.label ?? "Unavailable saved record"}</span><p class="av-hint">{record.available ? "Saved; extension persistence and live connection are separate." : "Corrupt or foreign record; exact revision recovery remains available."}</p><p class="break-all font-mono text-[10.5px] leading-4 text-zinc-400">{record.pairing.id} / {record.pairing.revision}</p>{#if record.binding}<p class="break-all font-mono text-[10.5px] leading-4 text-zinc-400">Installation: {record.binding.installation}<br />Application: {record.binding.browser_app} / {record.binding.browser_revision}</p>{/if}</div><div class="flex flex-col items-end gap-1.5"><button class="av-btn av-btn-secondary av-btn-sm" disabled={!enabled || busy || active || !record.available || selections === null || selections.length !== 0} onclick={() => select(record)}>Use installation</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || active} onclick={() => revoke(record)}>Revoke</button></div></div>
        {:else}<p class="av-hint px-3.5 py-2.5">No saved browser pairings.</p>{/each}
      </div>
    {/if}
    {#if selections !== null}
      <div class="av-card divide-y divide-white/[0.06]">
        {#each selections as selected (selected.revision)}
          <div class="flex items-start gap-3 px-3.5 py-2.5"><div class="min-w-0 flex-1"><span class="text-[12.5px] text-zinc-200">{selected.available ? `Selected · ${selected.binding?.label}` : "Selected installation unavailable"}</span><p class="av-hint">{selected.available ? "Saved configuration only. Connect the selected extension explicitly; no page scopes are granted." : "Corrupt, ambiguous, missing credential or unavailable application. Clear this exact revision to recover."}</p><p class="break-all font-mono text-[10.5px] leading-4 text-zinc-400">Selection: {selected.revision}</p>{#if selected.selected}<p class="break-all font-mono text-[10.5px] leading-4 text-zinc-400">Pairing: {selected.selected.pairing.id} / {selected.selected.pairing.revision}</p>{/if}</div><div class="flex flex-col items-end gap-1.5"><button class="av-btn av-btn-secondary av-btn-sm" disabled={!enabled || busy || active || !selected.available} onclick={() => connectSelected(selected)}>Connect selected</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || active} onclick={() => clearSelection(selected)}>Clear selection</button></div></div>
        {:else}<p class="av-hint px-3.5 py-2.5">No installation selected. Choosing one does not grant page access or prove an account.</p>{/each}
      </div>
    {/if}
    {#if error}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
  </div>
{/if}
