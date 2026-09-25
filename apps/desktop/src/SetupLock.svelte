<script lang="ts">
  import { onMount } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Status = { local_authentication: string; authentication_seconds_remaining: number; enrollment: string; reason: string };
  let status = $state<Status | null>(null);
  let busy = $state(false);
  let error = $state("");
  let mounted = true;
  let refreshing = false;
  let operationGeneration = 0;
  let context = "";
  $effect(() => {
    const next = `${runtime?.capture_epoch}:${runtime?.connected}:${runtime?.locked}`;
    if (next !== context) { context = next; operationGeneration++; status = null; }
  });
  const unlocked = $derived(status?.local_authentication === "verified");
  async function refresh() {
    if (!native || busy || refreshing) return;
    refreshing = true;
    const generation = operationGeneration;
    try { const next = await command<Status>("setup_status"); if (mounted && generation === operationGeneration) status = next; }
    catch (e) { if (mounted && generation === operationGeneration) error = String(e); }
    finally { refreshing = false; }
  }
  async function unlock() {
    busy = true; error = "";
    const generation = ++operationGeneration;
    try { const next = await command<Status>("verify_setup"); if (mounted && generation === operationGeneration) status = next; }
    catch (e) { if (mounted && generation === operationGeneration) error = String(e); }
    finally { busy = false; }
  }
  async function lock() {
    operationGeneration++; status = null;
    await command("cancel_setup"); await refresh();
  }
  onMount(() => {
    mounted = true; void refresh();
    const interval = setInterval(() => void refresh(), 2000);
    return () => { mounted = false; operationGeneration++; clearInterval(interval); if (native) void command("cancel_setup").catch(() => {}); };
  });
</script>

<!-- Approved People & Voice ID lock banner: Settings.dc.html 552–566. -->
<div class="flex items-center gap-3 bg-white/[0.03] px-3.5 py-3 ring-1 ring-white/10 ring-inset">
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="shrink-0 text-zinc-300" aria-hidden="true"><rect x="5" y="11" width="14" height="10"></rect><path d="M8 11V8a4 4 0 0 1 8 0v3"></path></svg>
  <span class="min-w-0 flex-1 text-[12.5px] leading-[18px] text-zinc-300">
    {unlocked ? `Windows verified · ${status?.authentication_seconds_remaining}s remaining. Voice enrollment still requires service and quality checks.` : "Voice identity management is locked. Verify with Windows Hello here. It locks again when Settings closes."}
  </span>
  {#if unlocked}
    <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => lock().catch(e => error = String(e))}>Lock</button>
  {:else}
    <button class="av-btn av-btn-primary av-btn-sm" disabled={busy || !runtime?.connected || status?.local_authentication !== "available"} onclick={unlock}>{busy ? "Verifying…" : "Unlock with Windows Hello"}</button>
  {/if}
</div>
{#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
