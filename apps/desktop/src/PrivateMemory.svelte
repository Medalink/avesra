<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SetupLock from "./SetupLock.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Entry = { id: string; revision: string; source: { task: string; turn: string }; content: { kind: "fact"; value: string } | { kind: "routine"; name: string; disabled: boolean }; corrected: boolean; created_ms: number };
  type View = { entry: Entry; validated: boolean };
  type Snapshot = { sequence: number; memories: View[]; tasks: { task: string; outcome: string | null; payload: { kind: string }; created_ms: number }[] };
  let panel = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let task = $state("");
  let kind = $state("fact");
  let value = $state("");
  let editing = $state<Entry | null>(null);
  let disabled = $state(false);
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  let mounted = false;
  let generation = 0;
  let context = "";
  const enabled = $derived(native && runtime?.connected && !runtime.locked);
  const bytes = $derived(new TextEncoder().encode(value).length);
  const valid = $derived(value.trim().length > 0 && bytes <= (kind === "fact" ? 1024 : 256) && !/[\x00-\x08\x0b-\x1f\x7f]/.test(value));
  function invalidate() { const old = panel; generation++; panel = null; snapshot = null; editing = null; task = ""; value = ""; if (old && native) void command("close_action_panel", { panel: old }).catch(() => {}); }
  $effect(() => { const next = `${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}`; if (next !== context) { context = next; invalidate(); } });
  function apply(next: Snapshot) { if (Number.isSafeInteger(next.sequence) && next.sequence > 0 && (!snapshot || next.sequence >= snapshot.sequence)) snapshot = next; }
  async function run(work: (current: number) => Promise<void>) {
    if (busy || !enabled) return;
    busy = true; error = ""; notice = "";
    const current = ++generation;
    try { await work(current); } catch (e) { if (mounted && current === generation) error = `${String(e)} Refresh before retrying a change.`; } finally { busy = false; }
  }
  function refresh() { return run(async current => {
    if (!panel) { const id = await command<string>("open_action_panel"); if (!mounted || current !== generation) { void command("close_action_panel", { panel: id }).catch(() => {}); return; } panel = id; }
    const result = await command<{ snapshot: Snapshot }>("action_status", { panel });
    if (mounted && current === generation) apply(result.snapshot);
  }); }
  function save() { return run(async current => {
    if (!panel || !valid || (!editing && !task)) return;
    const change = editing ? { kind: "correct", id: editing.id, revision: editing.revision, text: value, disabled: kind === "routine" && disabled } : kind === "fact" ? { kind: "save_fact", task, value } : { kind: "save_routine", task, name: value };
    const result = await command<Snapshot>("change_private_memory", { panel, change });
    if (mounted && current === generation) { apply(result); editing = null; value = ""; disabled = false; notice = "Saved with its original task source. No sound was announced."; }
  }); }
  function remove(entry: Entry) { return run(async current => {
    if (!panel) return;
    const result = await command<Snapshot>("change_private_memory", { panel, change: { kind: "delete", id: entry.id, revision: entry.revision } });
    if (mounted && current === generation) { apply(result); if (editing?.id === entry.id) { editing = null; value = ""; } notice = "Deleted this memory, its revision content and dependent learning-event text. Original task history and backups remain separate."; }
  }); }
  function edit(entry: Entry) { editing = entry; kind = entry.content.kind; value = entry.content.kind === "fact" ? entry.content.value : entry.content.name; disabled = entry.content.kind === "routine" && entry.content.disabled; }
  onMount(() => {
    mounted = true; let stop: (() => void) | undefined;
    if (native) void listen("settings-hidden", invalidate).then(value => { if (mounted) stop = value; else value(); }).catch(e => { if (mounted) error = String(e); });
    if (enabled) void refresh();
    return () => { mounted = false; generation++; stop?.(); if (panel) void command("close_action_panel", { panel }).catch(() => {}); };
  });
</script>

<section class="section">
  <div class="flex items-center justify-between"><span class="av-kicker">Private facts & routines</span><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={refresh}>{busy ? "Working…" : "Refresh"}</button></div>
  <p class="av-hint">Save an explicit fact or one-step routine from a verified task. Facts are your assertions; a task source does not prove their text. Only your records appear here.</p>
  <SetupLock {runtime} purpose="private memory" />
  {#if !editing}<label class="av-label" for="memory-task">Verified source task</label><select id="memory-task" class="av-select" bind:value={task} disabled={!enabled || busy}><option value="">Choose a successful task</option>{#each snapshot?.tasks.filter(t => t.outcome === "success") ?? [] as item}<option value={item.task}>{item.payload.kind.replaceAll("_", " ")} · {new Date(item.created_ms).toLocaleString()}</option>{/each}</select>
    <label class="av-label" for="memory-kind">Save as</label><select id="memory-kind" class="av-select" bind:value={kind} disabled={!enabled || busy}><option value="fact">Explicit private fact</option><option value="routine">Named one-step routine</option></select>
  {/if}
  <label class="av-label" for="memory-value">{kind === "fact" ? "Fact" : "Routine name"}</label><textarea id="memory-value" class="av-textarea" rows="3" bind:value disabled={!enabled || busy}></textarea>
  <p class="av-hint">{bytes} / {kind === "fact" ? 1024 : 256} UTF-8 bytes. {kind === "routine" ? "Use a short unique name with letters, numbers, spaces, apostrophes or hyphens. Say ‘Run routine NAME’ in a newly accepted request; current action permissions are still required." : "Keep this brief. Do not save passwords, credentials or complete messages."}</p>
  {#if editing && kind === "routine"}<label class="av-hint"><input type="checkbox" bind:checked={disabled} disabled={busy} /> Disable this routine</label>{/if}
  <div class="flex gap-2"><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !panel || !valid || (!editing && !task)} onclick={save}>{editing ? "Save correction" : "Save with source"}</button>{#if editing}<button class="av-btn av-btn-ghost" disabled={busy} onclick={() => { editing = null; value = ""; disabled = false; }}>Cancel edit</button>{/if}</div>
  <div class="line"></div>
  {#if !snapshot}<p class="av-hint">Refresh to inspect your durable records.</p>{:else if snapshot.memories.length === 0}<p class="av-hint">No saved facts or routines for the current owner.</p>{:else}{#each snapshot.memories as item}<div class="row"><div><h3>{item.entry.content.kind === "fact" ? item.entry.content.value : item.entry.content.name}</h3><p class="av-hint">{item.entry.content.kind === "fact" ? "Explicit fact" : item.entry.content.disabled ? "Disabled routine" : item.validated ? "Routine · verified invocation" : "Routine candidate · not yet invoked"}{item.entry.corrected ? " · corrected revision" : ""} · {new Date(item.entry.created_ms).toLocaleString()}</p><p class="av-hint">Source task {item.entry.source.task}</p></div><div class="flex gap-2"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => edit(item.entry)}>Correct</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => remove(item.entry)}>Delete</button></div></div>{/each}{/if}
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}{#if notice}<p class="av-hint" role="status">{notice}</p>{/if}
</section>
