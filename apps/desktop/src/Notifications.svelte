<script lang="ts">
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Description = { kind: "fact"; value: string } | { kind: "routine_candidate"; name: string } | { kind: "verified_action"; operation: string };
  type Event = { id: string; kind: "learning" | "action"; at_ms: number; description: Description | null };
  let rows = $state<Event[]>([]), busy = $state(false), error = $state("");
  let generation = 0;
  $effect(() => { const key = `${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}`; void key; generation++; rows = []; });
  async function refresh() {
    if (busy || !native) return;
    const token = generation; busy = true; error = "";
    try { const values = await command<Event[]>("notification_status"); if (token === generation) rows = values; }
    catch (e) { if (token === generation) error = String(e); }
    finally { busy = false; }
  }
</script>
<section class="flex flex-col gap-2">
  <div class="flex items-center justify-between gap-3"><span class="av-kicker">Committed events</span><button class="av-btn av-btn-secondary av-btn-sm" disabled={!native || busy || !runtime?.connected || runtime.locked} onclick={refresh}>{busy ? "Reading…" : "Inspect recent events"}</button></div>
  <p class="av-hint">Inspection never replays a sound. Voice questions about the last announcement use that exact batch, even when newer events stayed silent.</p>
  {#if error}<p class="av-hint text-red-300" role="alert">{error}</p>{/if}
  {#each rows as row (row.id)}<div class="border-l border-white/10 pl-3"><p class="av-hint">{new Date(row.at_ms).toLocaleString()} · {row.kind}</p><p class="text-sm text-zinc-200">{!row.description ? "Content deleted" : row.description.kind === "fact" ? `Explicitly saved fact: ${row.description.value}` : row.description.kind === "routine_candidate" ? `Unvalidated routine candidate: ${row.description.name}` : row.description.operation}</p></div>{/each}
</section>
