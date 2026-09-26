<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";
  type Selection = { memory: string; revision: string };
  type Response = { kind: "answer" | "needs_input"; text: string } | { kind: "proposal"; action: { kind: "launch_app"; alias: string } | { kind: "set_volume"; percent: number } };
  type Provenance = { kind: "model" | "native_mailbox" | "native_memory" | "native_clock" | "native_observation" | "native_events" };
  type Planner = { content_deleted: boolean; request: string; state: string; owner_revision: string; registration_revision: string; reply: { revision: string; response: Response; provenance: Provenance } | null; server_fingerprint: null };
  type Row = { id: string; revision: string; created_ms: number; state: string; text: string; source: { device: string; session: string; utterance: string }; planner: Planner | null; linked_task: { task: string; state: string } | null; playback_outcome: null };
  type Page = { rows: Row[]; scanned_window_complete: boolean; has_older: boolean };
  type SourceView = { memory: string; revision: string; key: string; current_value: string; corrected: boolean; original: Row; correction: Row | null };
  type SearchQuery = { kind: "text"; text: string } | { kind: "turn" | "task" | "app"; id: string };
  type Coverage = { indexed_through: number; highwater: number; complete: boolean; processed: number; budget_exhausted: boolean };
  type Query = { kind: "search"; query: SearchQuery } | { kind: "search_more"; cursor: string } | { kind: "index" } | { kind: "page"; cursor: string | null } | { kind: "memory_source"; memory: string; revision: string };
  type View = { search_cursor: string | null; coverage: Coverage | null; reader: string; cursor: string | null; remaining_ms: number; page: Page | null; source: SourceView | null };
  type DeletionPreview = { ticket: string; selected: string; affected: { turn: string; revision: string; created_ms: number; response_already_deleted: boolean }[]; remaining_ms: number };
  type Deleted = { deletion: string; selected: string; responses_removed: number; wal_truncated: boolean };
  let { runtime, panel, source, blocked = false, onbusy }: { runtime: Runtime | null; panel: string | null; source: Selection | null; blocked?: boolean; onbusy?: (value: boolean) => void } = $props();
  let reader = $state<string | null>(null);
  let cursor = $state<string | null>(null);
  let page = $state<Page | null>(null);
  let pageKind = $state<"history" | "search" | null>(null);
  let searchCursor = $state<string | null>(null);
  let coverage = $state<Coverage | null>(null);
  let indexBatch = $state(false);
  let searchMode = $state<SearchQuery["kind"]>("text");
  let searchText = $state("");
  let searchLabel = $state("");
  const searchBytes = $derived(new TextEncoder().encode(searchText).length);
  const searchTerms = $derived(searchText.match(/[\p{Alphabetic}\p{N}]+/gu) ?? []);
  const searchValid = $derived(searchMode === "text"
    ? searchBytes <= 256 && searchTerms.length >= 1 && searchTerms.length <= 8
    : /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(searchText.trim()) && searchText.trim() !== "00000000-0000-0000-0000-000000000000");
  let sourceView = $state<SourceView | null>(null);
  let busy = $state(false);
  let message = $state("");
  let pendingDeletion = $state<DeletionPreview | null>(null);
  let deletionNotice = $state("");
  let withdrawals = 0;
  let deletionExpiry: ReturnType<typeof setTimeout> | undefined;
  const deletionTarget = $derived(pendingDeletion ? page?.rows.find(row => row.id === pendingDeletion?.selected) ?? null : null);
  let mounted = false;
  let generation = 0;
  let context = "";
  let expiry: ReturnType<typeof setTimeout> | undefined;
  const enabled = $derived(!blocked && native && !!runtime?.connected && !runtime.locked && !!panel);
  function contextKey() {
    return JSON.stringify([panel, runtime?.locked, runtime?.connected, runtime?.action_epoch, runtime?.capture_epoch, runtime?.settings.owner_name?.actor]);
  }
  function clear() {
    withdrawals++; deletionNotice = ""; clearContent();
  }
  function cancelDeletion() {
    clearTimeout(deletionExpiry); deletionExpiry = undefined; pendingDeletion = null;
  }
  function clearContent() {
    cancelDeletion();
    generation++; clearTimeout(expiry); expiry = undefined;
    reader = null; cursor = null; page = null; sourceView = null; message = "";
    searchCursor = null; coverage = null; indexBatch = false; pageKind = null; searchText = ""; searchLabel = ""; searchMode = "text";
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
  function search() {
    if (!searchValid) return;
    const query: SearchQuery = searchMode === "text" ? { kind: "text", text: searchText } : { kind: searchMode, id: searchText.trim() };
    void inspect({ kind: "search", query });
  }
  async function inspect(query: Query, fresh = false) {
    if (!mounted || !enabled || busy || !panel) return;
    if (fresh) clear();
    cancelDeletion(); deletionNotice = "";
    busy = true; onbusy?.(true); message = ""; page = null; sourceView = null;
    const token = ++generation, key = contextKey(), started = performance.now();
    const label = query.kind === "search" ? (query.query.kind === "text" ? `All terms: ${query.query.text}` : `Exact ${query.query.kind} ID: ${query.query.id}`) : searchLabel;
    cursor = null; searchCursor = null; coverage = null; indexBatch = false; pageKind = null;
    try {
      const result = await command<View>("inspect_conversation_history", { panel, reader, query });
      if (!current(token, key)) return;
      const remaining = result.remaining_ms - (performance.now() - started);
      if (!Number.isFinite(remaining) || remaining <= 0 || result.remaining_ms > 60_000) {
        clear(); message = "History verification expired. Verify with Windows Hello, then open history or inspect the source again."; return;
      }
      clearTimeout(expiry);
      reader = result.reader; cursor = result.cursor; page = result.page; sourceView = result.source;
      searchCursor = result.search_cursor; coverage = result.coverage; indexBatch = query.kind === "index";
      pageKind = result.page ? (query.kind === "search" || query.kind === "search_more" ? "search" : "history") : null;
      searchLabel = pageKind === "search" ? label : "";
      if (query.kind === "index") message = "Index batch finished. Review its coverage, then run a new search.";
      const lease = reader, withdrawal = withdrawals;
      expiry = setTimeout(() => {
        if (mounted && reader === lease && withdrawal === withdrawals && key === contextKey()) { clear(); message = "History verification expired. Verify with Windows Hello, then reopen history."; }
      }, remaining);
    } catch (e) {
      if (current(token, key)) {
        // Search/index failures can retain the same unrenewed reader, including a stale search cursor.
        if (!reader || !["search", "search_more", "index"].includes(query.kind)) clear();
        message = String(e);
      }
    } finally { busy = false; onbusy?.(false); }
  }
  function eligible(row: Row) {
    if (row.linked_task || !row.planner) return false;
    if (row.planner.content_deleted) return true;
    return row.planner.state === "replied" && ["answered", "waiting_input"].includes(row.state)
      && row.planner.reply?.provenance.kind === "model" && row.planner.reply.response.kind !== "proposal";
  }
  async function prepareDeletion(row: Row) {
    if (!mounted || !enabled || busy || !panel || !reader || sourceView || !page?.rows.includes(row) || !eligible(row)) return;
    cancelDeletion(); message = ""; deletionNotice = "";
    busy = true; onbusy?.(true);
    // Keep the original reader expiry active while preparation runs.
    const token = generation, key = contextKey(), started = performance.now();
    try {
      const result = await command<DeletionPreview>("prepare_conversation_deletion", { panel, reader, turn: row.id, revision: row.revision });
      if (!current(token, key)) return;
      const remaining = result.remaining_ms - (performance.now() - started);
      if (!Number.isFinite(remaining) || remaining <= 0 || result.remaining_ms > 30_000) {
        clear(); message = "Deletion confirmation expired. Verify with Windows Hello and reopen history before preparing again."; return;
      }
      if (result.selected !== row.id || !result.affected.some(item => item.turn === row.id && item.revision === row.revision)) throw new Error("Deletion preview did not match the selected revision.");
      pendingDeletion = result;
      deletionExpiry = setTimeout(() => {
        if (mounted && token === generation && key === contextKey()) {
          clear(); message = "Deletion confirmation expired. Verify with Windows Hello and reopen history before preparing again.";
        }
      }, remaining);
    } catch (e) {
      if (current(token, key)) { cancelDeletion(); message = String(e); }
    } finally { busy = false; onbusy?.(false); }
  }
  async function confirmDeletion() {
    if (!mounted || !enabled || busy || !panel || !pendingDeletion || !deletionTarget) return;
    const ticket = pendingDeletion.ticket, selected = pendingDeletion.selected;
    const key = contextKey(), withdrawal = withdrawals;
    busy = true; onbusy?.(true);
    // Remove cached content before the mutation; its commit event can arrive before this await.
    clearContent(); deletionNotice = "";
    try {
      const result = await command<Deleted>("confirm_conversation_deletion", { panel, ticket });
      if (!mounted || !enabled || withdrawal !== withdrawals || key !== contextKey()) return;
      if (result.selected !== selected) { deletionNotice = "Deletion returned an unexpected identity. Reopen verified history to inspect the current store."; return; }
      deletionNotice = `Selected accepted text deleted. ${result.responses_removed} stored model response${result.responses_removed === 1 ? "" : "s"} removed. Later independently accepted text remains. ${result.wal_truncated ? "Current-store WAL cleanup completed." : "Deletion committed; current-store WAL cleanup did not complete."} Offline backups and filesystem snapshots are unchanged; this is not a forensic-erasure guarantee.`;
    } catch (e) {
      if (mounted && enabled && withdrawal === withdrawals && key === contextKey()) deletionNotice = `${String(e)} Reopen verified history to inspect the durable outcome before retrying.`;
    } finally { busy = false; onbusy?.(false); }
  }
  onMount(() => {
    mounted = true;
    const stops: (() => void)[] = [];
    for (const event of ["settings-hidden", "private-memory-changed"]) {
      if (native) void listen(event, clear).then(stop => { if (mounted) stops.push(stop); else stop(); }).catch(e => { if (mounted) message = String(e); });
    }
    if (native) void listen("conversation-history-changed", clearContent).then(stop => { if (mounted) stops.push(stop); else stop(); }).catch(e => { if (mounted) message = String(e); });
    return () => { mounted = false; clear(); for (const stop of stops) stop(); onbusy?.(false); };
  });
</script>

{#snippet record(row: Row, title: string, allowDeletion = false)}
  <article class="av-card p-3 mt-2">
    <strong>{title}</strong>
    <p class="av-hint">{new Date(row.created_ms).toLocaleString()} · recorded state: {row.state}</p>
    <p class="whitespace-pre-wrap break-words">{row.text}</p>
    {#if row.planner}
      <p class="av-hint">Stored planner state: {row.planner.state}</p>
      {#if row.planner.content_deleted}
        <p class="av-hint">Stored planner request, dialogue and model response were deleted. This independently accepted original text remains.</p>
      {:else if row.planner.reply}
        <p class="av-hint">Stored response · {row.planner.reply.response.kind.replaceAll("_", " ")} · provenance: {row.planner.reply.provenance.kind.replaceAll("_", " ")}</p>
        {#if row.planner.reply.response.kind === "proposal"}
          <p class="whitespace-pre-wrap break-words">{row.planner.reply.response.action.kind === "launch_app" ? `Proposed opening app: ${row.planner.reply.response.action.alias}` : `Proposed volume: ${row.planner.reply.response.action.percent}%`}</p>
        {:else}<p class="whitespace-pre-wrap break-words">{row.planner.reply.response.text}</p>{/if}
      {:else}<p class="av-hint">Stored planner response unavailable.</p>{/if}
    {:else}<p class="av-hint">Stored planner record unavailable.</p>{/if}
    {#if row.linked_task}<p class="av-hint break-all">Linked task {row.linked_task.task} · recorded state: {row.linked_task.state}</p>{:else}<p class="av-hint">No linked task recorded.</p>{/if}
    <p class="av-hint">Playback outcome unavailable. Stored text or a proposal does not prove speech playback or action success.</p>
    {#if allowDeletion && eligible(row)}
      <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || !reader || !!pendingDeletion} onclick={() => prepareDeletion(row)}>Prepare deletion</button>
    {/if}
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
  <p class="av-hint">Inspect original accepted requests and stored responses for the current owner and device. Historical text may contain outdated or deleted fact values; it is not current memory. This view cannot replay a reply or rerun a stored proposal.</p>
  <p class="av-hint">Verify with Windows Hello above. Access expires within 60 seconds; searching and paging do not extend it.</p>
  <div class="flex flex-wrap gap-2">
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => inspect({ kind: "page", cursor: null }, true)}>{busy ? "Working…" : "Open history"}</button>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || pageKind !== "history" || !reader || !cursor || !page?.has_older} onclick={() => inspect({ kind: "page", cursor })}>Older accepted turns</button>
    {#if page || sourceView || coverage}<button class="av-btn av-btn-ghost av-btn-sm" onclick={clear}>Clear displayed history</button>{/if}
  </div>
  {#if !panel}<p class="av-hint">Refresh private memory above to open the current Settings panel.</p>{/if}
  <fieldset disabled={!enabled || busy} class="av-card p-3 mt-3">
    <legend class="av-hint">Search protected history</legend>
    <p class="av-hint">Search your requests and saved replies. All entered words must match.</p>
    <div class="flex flex-wrap gap-2">
      <label class="av-hint">Search by <select class="av-select" bind:value={searchMode}><option value="text">Accepted and response text</option><option value="turn">Exact turn ID</option><option value="task">Exact task ID</option><option value="app">Exact linked app ID</option></select></label>
      <label class="av-hint">{searchMode === "text" ? "Search terms" : "Exact UUID"}<input class="av-input" bind:value={searchText} maxlength="256" placeholder={searchMode === "text" ? "Words that must all match" : "00000000-0000-0000-0000-000000000000"} /></label>
      <button class="av-btn av-btn-ghost av-btn-sm" disabled={!searchValid} onclick={search}>Search history</button>
      <button class="av-btn av-btn-ghost av-btn-sm" disabled={pageKind !== "search" || !reader || !searchCursor} onclick={() => { if (searchCursor) void inspect({ kind: "search_more", cursor: searchCursor }); }}>More search results</button>
    </div>
    {#if searchMode === "app"}<p class="av-hint">Exact app IDs match validated linked application or project-prompt tasks. This does not search the installed-app catalog or every mention of an app.</p>{/if}
    <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => inspect({ kind: "index" })}>Index older history</button>
    <p class="av-hint">Index older history to include it in searches. Each click processes one batch.</p>
    <details>
      <summary>Search details</summary>
      <p class="av-hint">Text search accepts 1–8 alphanumeric terms and at most 256 UTF-8 bytes ({searchBytes} entered). Punctuation separates terms; raw search operators and wildcards are not used. Saved responses do not prove playback.</p>
      <p class="av-hint">Each indexing batch processes at most 200 headers and 2 MiB. It never loops automatically or renews the original Windows Hello proof. Index changes or new source content can invalidate search pagination; start a new search if requested.</p>
    </details>
  </fieldset>
  {#if coverage}
    <div class="av-card p-3 mt-2">
      <p class="av-hint">{coverage.complete ? "Indexing is complete for this view." : "Some older history is not indexed yet; it may contain matches."}{coverage.budget_exhausted ? " This batch reached its limit. Choose Index older history to continue." : ""}</p>
      <details><summary>Index coverage details</summary><p class="av-hint">Indexed through {coverage.indexed_through} · captured high-water {coverage.highwater}. These are progress markers, not matching-record counts. {#if indexBatch}{coverage.processed} headers processed in this batch.{/if}</p></details>
    </div>
  {:else}<p class="av-hint">Search coverage is unavailable until a search or index batch returns. An empty search must not be read as complete history coverage.</p>{/if}
  {#if pendingDeletion && deletionTarget}
    <div class="av-card p-3 mt-3" role="group" aria-label="Confirm selected history deletion">
      <strong>Confirm deletion of this accepted text</strong>
      <p class="whitespace-pre-wrap break-words">{deletionTarget.text}</p>
      <p class="av-hint break-all">Selected turn {deletionTarget.id} · revision {deletionTarget.revision} · {new Date(deletionTarget.created_ms).toLocaleString()}</p>
      <p class="av-hint">Delete the selected accepted text and the stored planner requests, dialogue and model responses listed below. Later independently accepted user text remains. Previously deleted responses remain deleted.</p>
      <ul class="av-hint">
        {#each pendingDeletion.affected as affected (affected.turn)}
          <li class="break-all">Stored response for turn {affected.turn} · turn revision {affected.revision} · {new Date(affected.created_ms).toLocaleString()} · {affected.response_already_deleted ? "already deleted; no response content remains to remove" : "response content will be removed"}{affected.turn === pendingDeletion.selected ? " · selected accepted text will also be removed" : " · accepted text is not removed by this selection"}.</li>
        {/each}
      </ul>
      <p class="av-hint">This confirmation expires within 30 seconds and cannot outlive the original Windows Hello proof. This deletes eligible content from the current product store and retrieval; it does not rewrite offline backups or promise forensic erasure. Task and memory-source history deletion is not supported here.</p>
      <div class="flex flex-wrap gap-2">
        <button class="av-btn av-btn-secondary av-btn-sm" disabled={!enabled || busy} onclick={confirmDeletion}>Confirm selected text and response deletion</button>
        <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy} onclick={cancelDeletion}>Cancel deletion</button>
      </div>
    </div>
  {/if}
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
    {#if pageKind === "search"}
      <p class="av-hint break-words">Search: {searchLabel}</p>
      <p class="av-hint">{page.rows.length} matching accepted turns on this search page.</p>
      {#if page.rows.length === 0}<p class="av-hint">No matches on this indexed page.{searchCursor ? " More search results remain to inspect." : ""} {coverage?.complete ? "Coverage is limited to the captured index high-water mark." : "Index coverage is incomplete or unavailable; this does not prove older history has no matches."}</p>{/if}
      {#if !searchCursor}<p class="av-hint">End of results for this search and its captured index coverage.</p>{/if}
    {:else}
      <p class="av-hint">{page.rows.length} matching accepted turns in this scanned window. {page.scanned_window_complete ? "The frozen history scan is exhausted." : "The scan is bounded; older windows remain to inspect."}</p>
      {#if page.rows.length === 0}<p class="av-hint">{page.has_older ? "No matching turns in this sparse window. Continue with Older accepted turns; older history may remain." : "No matching turns in this window and no older history remains."}</p>{/if}
      {#if !page.has_older}<p class="av-hint">End of older accepted history for this frozen view.</p>{/if}
    {/if}
    {#each page.rows as row (row.id)}{@render record(row, "Historical accepted request", true)}{/each}
  {/if}
  {#if deletionNotice}<p class="av-hint break-words" role="status">{deletionNotice}</p>{/if}
  {#if message}<p class="av-hint break-words" role="status">{message}</p>{/if}
</section>
