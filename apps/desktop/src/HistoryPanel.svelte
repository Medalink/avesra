<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SetupLock from "./SetupLock.svelte";
  import ConversationHistory from "./ConversationHistory.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  let panel = $state<string | null>(null);
  let busy = $state(false);
  let error = $state("");
  let mounted = false;
  let generation = 0;
  let context = "";
  const enabled = $derived(native && !!runtime?.connected && !runtime.locked);
  function contextKey() {
    return JSON.stringify([runtime?.locked, runtime?.connected, runtime?.action_epoch, runtime?.capture_epoch, runtime?.settings.owner_name?.actor]);
  }
  function close(id: string) {
    void command("close_action_panel", { panel: id }).catch(() => {});
  }
  function invalidate() {
    generation++;
    const old = panel;
    panel = null; error = "";
    if (old && native) close(old);
  }
  $effect(() => {
    const next = contextKey();
    if (next !== context) { context = next; invalidate(); }
  });
  async function prepare() {
    if (!mounted || !enabled || busy || panel) return;
    busy = true; error = "";
    const token = ++generation, key = contextKey();
    try {
      const id = await command<string>("open_action_panel");
      if (!mounted || !enabled || token !== generation || key !== contextKey()) { close(id); return; }
      panel = id;
    } catch (e) {
      if (mounted && token === generation && key === contextKey()) error = String(e);
    } finally { busy = false; }
  }
  onMount(() => {
    mounted = true;
    let stop: (() => void) | undefined;
    if (native) void listen("settings-hidden", invalidate).then(unlisten => {
      if (!mounted) { unlisten(); return; }
      stop = unlisten;
      void prepare();
    }).catch(e => { if (mounted) error = String(e); });
    return () => { mounted = false; invalidate(); stop?.(); };
  });
</script>

<section class="section">
  <SetupLock {runtime} purpose="accepted history inspection" />
  {#if !panel}
    <p class="av-hint">Prepare the current Settings panel, then verify with Windows Hello and choose Open history. Preparing does not read conversation content.</p>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={prepare}>{busy ? "Preparing…" : "Prepare history"}</button>
  {/if}
  {#if error}<p class="av-hint break-all" role="status">{error}</p>{/if}
</section>
{#if panel}<ConversationHistory {runtime} {panel} source={null} />{/if}
