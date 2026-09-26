<script lang="ts">
  import SelectFrame from "./SelectFrame.svelte";
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Teaching from "./Teaching.svelte";
  import ConversationHistory from "./ConversationHistory.svelte";
  import SetupLock from "./SetupLock.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Entry = { id: string; revision: string; source: { task: string; turn: string } | null; accepted_source?:{turn:string;revision:string}|null; content: { kind: "named_fact"; key:string; value:string } | { kind: "fact"; value: string } | { kind: "routine"; name: string; disabled: boolean }; corrected: boolean; created_ms: number };
  type View = { entry: Entry; validated: boolean };
  type Snapshot = { aliases:{id:string;revision:string;phrase:string}[]; sequence: number; memories: View[]; tasks: { task: string; outcome: string | null; payload: { kind: string }; created_ms: number }[] };
  type DeleteView = { proposal: string; memory: string; revision: string; key: string };
  let pendingDelete = $state<DeleteView | null>(null);
  let panel = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let task = $state("");
  let kind = $state("fact");
  let value = $state("");
  let editing = $state<Entry | null>(null);
  let formOpen = $state(false);
  let disabled = $state(false);
  let busy = $state(false);
  let historyBusy = $state(false);
  let historySource = $state<{ memory: string; revision: string } | null>(null);
  let error = $state("");
  let notice = $state("");
  let mounted = false;
  let generation = 0;
  let context = "";
  const enabled = $derived(native && runtime?.connected && !runtime.locked);
  const bytes = $derived(new TextEncoder().encode(value).length);
  const limit = $derived(editing?.content.kind === "named_fact" ? 512 : kind === "fact" ? 1024 : 256);
  const valid = $derived(value.trim().length > 0 && bytes <= limit && !/[\x00-\x08\x0b-\x1f\x7f]/.test(value));
  function invalidate() { historySource = null; const old = panel; generation++; panel = null; snapshot = null; pendingDelete = null; editing = null; formOpen = false; task = ""; value = ""; if (old && native) void command("close_action_panel", { panel: old }).catch(() => {}); }
  $effect(() => { const next = `${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}:${runtime?.capture_epoch}:${runtime?.settings.owner_name?.actor}`; if (next !== context) { context = next; invalidate(); } });
  function apply(next: Snapshot) { if (Number.isSafeInteger(next.sequence) && next.sequence > 0 && (!snapshot || next.sequence >= snapshot.sequence)) snapshot = next; }
  async function run(work: (current: number) => Promise<void>) {
    if (busy || historyBusy || !enabled) return;
    busy = true; historySource = null; error = ""; notice = "";
    const current = ++generation;
    try { await work(current); } catch (e) { if (mounted && current === generation) error = `${String(e)} Refresh before retrying a change.`; } finally { busy = false; }
  }
  function refresh() { return run(async current => {
    if (!panel) { const id = await command<string>("open_action_panel"); if (!mounted || current !== generation) { void command("close_action_panel", { panel: id }).catch(() => {}); return; } panel = id; }
    const result = await command<{ snapshot: Snapshot }>("action_status", { panel });
    const deletion = await command<DeleteView | null>("memory_forget_status", { panel });
    if (mounted && current === generation) { apply(result.snapshot); pendingDelete = deletion; }
  }); }
  function resolveDelete(approve: boolean) { return run(async current => {
    if (!panel || !pendingDelete) return;
    await command(approve ? "approve_memory_forget" : "cancel_memory_forget", { panel, proposal: pendingDelete.proposal });
    if (mounted && current === generation) { pendingDelete = null; notice = approve ? "Approval sent. The original request will report the result." : "Deletion cancelled."; }
  }); }
  function save() { return run(async current => {
    if (!panel || !valid || (!editing && !task)) return;
    const change = editing ? { kind: "correct", id: editing.id, revision: editing.revision, text: value, disabled: kind === "routine" && disabled } : kind === "fact" ? { kind: "save_fact", task, value } : { kind: "save_routine", task, name: value };
    const result = await command<Snapshot>("change_private_memory", { panel, change });
    if (mounted && current === generation) { apply(result); editing = null; formOpen = false; value = ""; disabled = false; notice = "Saved with its original source; notification delivery is tracked separately."; }
  }); }
  function remove(entry: Entry) { return run(async current => {
    if (!panel) return;
    const result = await command<Snapshot>("change_private_memory", { panel, change: { kind: "delete", id: entry.id, revision: entry.revision } });
    if (mounted && current === generation) { apply(result); if (editing?.id === entry.id) { editing = null; formOpen = false; value = ""; } notice = "Deleted this memory, its revision content and dependent learning-event text. Original task history and backups remain separate."; }
  }); }
  function entryText(entry: Entry) { return entry.content.kind === "named_fact" ? `${entry.content.key}: ${entry.content.value}` : entry.content.kind === "fact" ? entry.content.value : entry.content.name; }
  async function openForm() {
    if (!enabled || busy || historyBusy) return;
    formOpen = true; const current = generation;
    await tick();
    if (mounted && current === generation && formOpen) document.getElementById("memory-value")?.focus();
  }
  function edit(entry: Entry) { editing = entry; kind = entry.content.kind === "routine" ? "routine" : "fact"; value = entry.content.kind !== "routine" ? entry.content.value : entry.content.name; disabled = entry.content.kind === "routine" && entry.content.disabled; void openForm(); }
  onMount(() => {
    mounted = true; const stops: (() => void)[] = [];
    if (native) void listen("settings-hidden", invalidate).then(value => { if (mounted) stops.push(value); else value(); }).catch(e => { if (mounted) error = String(e); });
    for (const event of ["memory-forget-changed", "private-memory-changed"]) {
      if (native) void listen(event, () => { if (enabled && !busy) void refresh(); }).then(stop => { if (mounted) stops.push(stop); else stop(); }).catch(e => { if (mounted) error = String(e); });
    }
    if (enabled) void refresh();
    return () => { mounted = false; generation++; for (const stop of stops) stop(); if (panel) void command("close_action_panel", { panel }).catch(() => {}); };
  });
</script>

<section class="section">
  <div class="flex items-center justify-between gap-2">
    <span class="av-kicker">Private facts & routines</span>
    <div class="flex items-center gap-2">
      <button type="button" class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || historyBusy} onclick={refresh}>{busy ? "Working…" : "Refresh"}</button>
      <button type="button" class="av-btn av-btn-secondary" aria-expanded={formOpen} aria-controls="memory-editor" disabled={!enabled || busy || historyBusy} onclick={openForm}>{editing ? "Continue correction" : value || task ? "Continue draft" : "Add"}</button>
    </div>
  </div>
  <p class="av-hint">Say “Remember that my coffee preference is black coffee” to save a named fact from your accepted request. You can also save a fact or one-step routine from a verified task below. Facts are your assertions; only your records appear here.</p>
  <SetupLock {runtime} purpose="private memory" />
  {#if pendingDelete}<div class="row"><div><h3>Delete the saved fact “{pendingDelete.key}”?</h3><p class="av-hint">This approves only the exact saved revision requested in your current conversation. Conversation history and backups are unchanged.</p></div><div class="flex gap-2"><button class="av-btn av-btn-secondary" disabled={!enabled || busy || historyBusy} onclick={() => resolveDelete(true)}>Approve deletion</button><button class="av-btn av-btn-ghost" disabled={!enabled || busy || historyBusy} onclick={() => resolveDelete(false)}>Cancel</button></div></div>{/if}
  {#if formOpen}
    <div id="memory-editor" class="av-card flex flex-col gap-3 p-3.5">
      <div class="flex items-center justify-between gap-2">
        <h3 class="text-[13px] font-medium text-zinc-100">{editing ? "Correct saved memory" : "Add from a verified task"}</h3>
        <button type="button" class="av-btn av-btn-ghost av-btn-sm" disabled={busy || historyBusy} onclick={() => formOpen = false}>Close form</button>
      </div>
  {#if !editing}<label class="av-label" for="memory-task">Verified source task</label><SelectFrame><select id="memory-task" class="av-input av-select" bind:value={task} disabled={!enabled || busy || historyBusy}><option value="">Choose a successful task</option>{#each snapshot?.tasks.filter(t => t.outcome === "success") ?? [] as item}<option value={item.task}>{item.payload.kind.replaceAll("_", " ")} · {new Date(item.created_ms).toLocaleString()}</option>{/each}</select></SelectFrame>
    <label class="av-label" for="memory-kind">Save as</label><SelectFrame><select id="memory-kind" class="av-input av-select" bind:value={kind} disabled={!enabled || busy || historyBusy}><option value="fact">Explicit private fact</option><option value="routine">Named one-step routine</option></select></SelectFrame>
  {/if}
  <label class="av-label" for="memory-value">{kind === "fact" ? "Fact" : "Routine name"}</label><textarea id="memory-value" class="av-input av-textarea" rows="3" bind:value disabled={!enabled || busy || historyBusy}></textarea>
  <p class="av-hint">{bytes} / {limit} UTF-8 bytes. {kind === "routine" ? "Use a short unique name with letters, numbers, spaces, apostrophes or hyphens. Say ‘Run routine NAME’ in a newly accepted request; current action permissions are still required." : "Keep this brief. Do not save passwords, credentials or complete messages."}</p>
  {#if editing && kind === "routine"}<label class="av-hint"><input type="checkbox" bind:checked={disabled} disabled={busy || historyBusy} /> Disable this routine</label>{/if}
  <div class="flex gap-2"><button class="av-btn av-btn-secondary" disabled={!enabled || busy || historyBusy || !panel || !valid || (!editing && !task)} onclick={save}>{editing ? "Save correction" : "Save with source"}</button>{#if editing}<button class="av-btn av-btn-ghost" disabled={busy || historyBusy} onclick={() => { editing = null; value = ""; disabled = false; formOpen = false; }}>Cancel edit</button>{/if}</div>
    </div>
  {/if}
  <div class="flex flex-col gap-2" aria-label="Saved facts and routines">
    {#if !snapshot}
      <div class="border border-dashed border-white/15 px-4 py-6 text-center text-[12.5px] text-zinc-400">Refresh to inspect your durable records.</div>
    {:else if snapshot.memories.length === 0}
      <div class="border border-dashed border-white/15 px-4 py-6 text-center text-[12.5px] text-zinc-400">No saved facts or routines for the current owner.</div>
    {:else}
      {#each snapshot.memories as item (item.entry.id)}
        <div class="bg-white/[0.03] px-3.5 py-3 ring-1 ring-inset ring-white/[0.07]">
          <div class="flex items-start gap-3">
            <span class="av-chip mt-px shrink-0 {item.entry.content.kind === 'routine' ? 'text-zinc-200 ring-white/20' : 'bg-av-500/10 text-av-300 ring-av-500/40'}">{item.entry.content.kind === "routine" ? "Routine" : "Stated"}</span>
            <div class="flex min-w-0 flex-1 flex-col gap-1">
              <span class="break-words text-[13px] leading-[19px] text-zinc-100">{entryText(item.entry)}</span>
              <span class="text-[11px] text-zinc-400">{item.entry.source ? "Verified task source" : item.entry.accepted_source ? "Accepted request source" : "Source unavailable"} · {new Date(item.entry.created_ms).toLocaleString()}</span>
              <span class="text-[11px] text-zinc-400">{item.entry.content.kind !== "routine" ? "Explicit owner assertion" : item.entry.content.disabled ? "Disabled routine" : item.validated ? "Verified invocation" : "Candidate · not yet invoked"}{item.entry.corrected ? " · corrected revision" : ""}</span>
              <details class="text-[11px] text-zinc-400">
                <summary class="cursor-pointer">Source details</summary>
                {#if item.entry.source}<p class="mt-1 break-all">Task {item.entry.source.task} · turn {item.entry.source.turn}</p>{:else if item.entry.accepted_source}<p class="mt-1 break-all">Accepted turn {item.entry.accepted_source.turn} · revision {item.entry.accepted_source.revision}</p>{:else}<p class="mt-1">Source unavailable.</p>{/if}
              </details>
              {#if item.entry.content.kind === "named_fact" && item.entry.accepted_source}<button type="button" class="av-btn av-btn-ghost av-btn-sm self-start" disabled={!enabled || busy || historyBusy || !panel} onclick={() => historySource = { memory: item.entry.id, revision: item.entry.revision }}>Inspect accepted source</button>{/if}
            </div>
            <div class="flex shrink-0 items-center gap-0.5">
              <button type="button" class="av-iconbtn size-7" aria-label={`Correct ${entryText(item.entry)}`} title="Correct" disabled={!enabled || busy || historyBusy} onclick={() => edit(item.entry)}><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" aria-hidden="true"><path d="M4 20h4L19 9l-4-4L4 16z"></path><path d="m13.5 6.5 4 4"></path></svg></button>
              <button type="button" class="av-iconbtn size-7 hover:bg-red-500/10 hover:text-red-300" aria-label={`Delete ${entryText(item.entry)}`} title="Delete" disabled={!enabled || busy || historyBusy} onclick={() => remove(item.entry)}><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"></path></svg></button>
            </div>
          </div>
        </div>
      {/each}
    {/if}
  </div>
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}{#if notice}<p class="av-hint" role="status">{notice}</p>{/if}
</section>

<ConversationHistory {runtime} {panel} source={historySource} blocked={busy} onbusy={value => historyBusy = value} />

<Teaching {runtime} {panel} aliases={snapshot?.aliases ?? []} />
