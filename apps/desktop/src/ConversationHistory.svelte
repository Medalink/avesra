<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";
  type Selection = { memory: string; revision: string };
  type Response = { kind: "answer" | "needs_input"; text: string } | { kind: "proposal"; action: { kind: "launch_app"; alias: string } | { kind: "set_volume"; percent: number } };
  type Provenance = { kind: "model" | "native_mailbox" | "native_memory" | "native_clock" | "native_observation" | "native_events" };
  type Planner = { request: string; state: string; owner_revision: string; registration_revision: string; reply: { revision: string; response: Response; provenance: Provenance } | null; server_fingerprint: null };
  type Row = { id: string; revision: string; created_ms: number; state: string; text: string; source: { device: string; session: string; utterance: string }; planner: Planner | null; linked_task: { task: string; state: string } | null; playback_outcome: null };
  type Page = { rows: Row[]; scanned_window_complete: boolean; has_older: boolean };
  type SourceView = { memory: string; revision: string; key: string; current_value: string; corrected: boolean; original: Row; correction: Row | null };
  type Query = { kind: "page"; cursor: string | null } | { kind: "memory_source"; memory: string; revision: string };
  type View = { reader: string; cursor: string | null; remaining_ms: number; page: Page | null; source: SourceView | null };
  let { runtime, panel, source, blocked = false, onbusy }: { runtime: Runtime | null; panel: string | null; source: Selection | null; blocked?: boolean; onbusy?: (value: boolean) => void } = $props();
  let reader = $state<string | null>(null);
  let cursor = $state<string | null>(null);
  let page = $state<Page | null>(null);
  let sourceView = $state<SourceView | null>(null);
  let busy = $state(false);
  let message = $state("");
  let mounted = false;
  let generation = 0;
  let context = "";
  let expiry: ReturnType<typeof setTimeout> | undefined;
  const enabled = $derived(!blocked && native && !!runtime?.connected && !runtime.locked && !!panel);
  function contextKey() {
    return JSON.stringify([panel, runtime?.locked, runtime?.connected, runtime?.action_epoch, runtime?.capture_epoch, runtime?.settings.owner_name?.actor]);
  }
  function clear() {
    generation++; clearTimeout(expiry); expiry = undefined;
    reader = null; cursor = null; page = null; sourceView = null; message = "";
  }
  $effect(() => {
    const next = contextKey();
    if (context !== next) { context = next; clear(); }
  });
  $effect(() => {
    const selected = source;
    untrack(() => {
      if (selected) void inspect({ kind: "memory_source", ...selected });
      else if (sourceView) clear();
    });
  });
  function current(token: number, key: string) {
    return mounted && enabled && token === generation && key === contextKey();
  }
  async function inspect(query: Query, fresh = false) {
    if (!mounted || !enabled || busy || !panel) return;
    if (fresh) clear();
    busy = true; onbusy?.(true); message = ""; page = null; sourceView = null;
    const token = ++generation, key = contextKey(), started = performance.now();
    try {
      const result = await command<View>("inspect_conversation_history", { panel, reader, query });
      if (!current(token, key)) return;
      const remaining = result.remaining_ms - (performance.now() - started);
      if (!Number.isFinite(remaining) || remaining <= 0 || result.remaining_ms > 60_000) {
        clear(); message = "History verification expired. Verify with Windows Hello, then open history or inspect the source again."; return;
      }
      clearTimeout(expiry);
      reader = result.reader; cursor = result.cursor; page = result.page; sourceView = result.source;
      expiry = setTimeout(() => {
        if (mounted && token === generation && key === contextKey()) { clear(); message = "History verification expired. Verify with Windows Hello, then open history or inspect the source again."; }
      }, remaining);
    } catch (e) {
      if (current(token, key)) { clear(); message = `${String(e)} Verify with Windows Hello again, then open history or inspect the source again.`; }
    } finally { busy = false; onbusy?.(false); }
  }
  onMount(() => {
    mounted = true;
    const stops: (() => void)[] = [];
    for (const event of ["settings-hidden", "private-memory-changed"]) {
      if (native) void listen(event, clear).then(stop => { if (mounted) stops.push(stop); else stop(); }).catch(e => { if (mounted) message = String(e); });
    }
    return () => { mounted = false; clear(); for (const stop of stops) stop(); onbusy?.(false); };
  });
</script>

{#snippet record(row: Row, title: string)}
  <article class="av-card p-3 mt-2">
    <strong>{title}</strong>
    <p class="av-hint">{new Date(row.created_ms).toLocaleString()} · recorded state: {row.state}</p>
    <p class="whitespace-pre-wrap break-words">{row.text}</p>
    {#if row.planner}
      <p class="av-hint">Stored planner state: {row.planner.state}</p>
      {#if row.planner.reply}
        <p class="av-hint">Stored response · {row.planner.reply.response.kind.replaceAll("_", " ")} · provenance: {row.planner.reply.provenance.kind.replaceAll("_", " ")}</p>
        {#if row.planner.reply.response.kind === "proposal"}
          <p class="whitespace-pre-wrap break-words">{row.planner.reply.response.action.kind === "launch_app" ? `Proposed opening app: ${row.planner.reply.response.action.alias}` : `Proposed volume: ${row.planner.reply.response.action.percent}%`}</p>
        {:else}<p class="whitespace-pre-wrap break-words">{row.planner.reply.response.text}</p>{/if}
      {:else}<p class="av-hint">Stored planner response unavailable.</p>{/if}
    {:else}<p class="av-hint">Stored planner record unavailable.</p>{/if}
    {#if row.linked_task}<p class="av-hint break-all">Linked task {row.linked_task.task} · recorded state: {row.linked_task.state}</p>{:else}<p class="av-hint">No linked task recorded.</p>{/if}
    <p class="av-hint">Playback outcome unavailable. Stored text or a proposal does not prove speech playback or action success.</p>
    <details>
      <summary>Historical record identities</summary>
      <p class="av-hint break-all">Turn {row.id} · revision {row.revision}</p>
      <p class="av-hint break-all">Device {row.source.device} · session {row.source.session} · utterance {row.source.utterance}</p>
      {#if row.planner}<p class="av-hint break-all">Planner request {row.planner.request} · historical owner revision {row.planner.owner_revision} · historical registration revision {row.planner.registration_revision}{row.planner.reply ? ` · reply revision ${row.planner.reply.revision}` : ""}</p>{/if}
      <p class="av-hint">Historical paired-server identity unavailable.</p>
    </details>
  </article>
{/snippet}

<section class="section" aria-label="Protected accepted conversation history">
  <div class="av-kicker">Accepted conversation history</div>
  <p class="av-hint">Inspect original accepted requests and stored responses for the current owner and device. Historical text may contain outdated or deleted fact values; it is not current memory. This view cannot replay a reply or perform an action.</p>
  <p class="av-hint">Verify with Windows Hello above before opening. The reader uses that proof's original lifetime of at most 60 seconds; reading more pages does not renew it.</p>
  <div class="flex flex-wrap gap-2">
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => inspect({ kind: "page", cursor: null }, true)}>{busy ? "Reading…" : "Open history"}</button>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || !reader || !cursor || !page?.has_older} onclick={() => inspect({ kind: "page", cursor })}>Older accepted turns</button>
    {#if page || sourceView}<button class="av-btn av-btn-ghost av-btn-sm" onclick={clear}>Clear displayed history</button>{/if}
  </div>
  {#if !panel}<p class="av-hint">Refresh private memory above to open the current Settings panel.</p>{/if}
  {#if sourceView}
    <div class="av-card p-3 mt-3">
      <strong>Current saved fact at inspection</strong>
      <p class="whitespace-pre-wrap break-words">{sourceView.key}: {sourceView.current_value}</p>
      <p class="av-hint break-all">Memory {sourceView.memory} · current revision {sourceView.revision}</p>
      {#if sourceView.corrected && !sourceView.correction}<p class="av-hint">Corrected through protected Settings. No accepted-turn correction source is recorded.</p>{/if}
    </div>
    {@render record(sourceView.original, "Original accepted source (historical)")}
    {#if sourceView.correction}{@render record(sourceView.correction, "Accepted correction source (historical)")}{/if}
  {/if}
  {#if page}
    <p class="av-hint">{page.rows.length} matching accepted turns in this scanned window. {page.scanned_window_complete ? "The frozen history scan is exhausted." : "The scan is bounded; older windows remain to inspect."}</p>
    {#if page.rows.length === 0}<p class="av-hint">{page.has_older ? "No matching turns in this sparse window. Continue with Older accepted turns; older history may remain." : "No matching turns in this window and no older history remains."}</p>{/if}
    {#each page.rows as row (row.id)}{@render record(row, "Historical accepted request")}{/each}
    {#if !page.has_older}<p class="av-hint">End of older accepted history for this frozen view.</p>{/if}
  {/if}
  {#if message}<p class="av-hint break-all" role="status">{message}</p>{/if}
</section>
