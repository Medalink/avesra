<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { command, native, type Runtime, type Settings } from "./runtime";
  let { runtime, saving, update }: { runtime: Runtime | null; saving: boolean; update: (patch: Partial<Settings>) => Promise<void> } = $props();
  type Identity = { id: string; revision: string; audio_sha256: string; metadata_sha256: string };
  type Candidate = { id: string; revision: string; state: string; identity: Identity | null; description: string | null; text: string | null; created_at_ms: number | null };
  type VoiceStatus = { selected: Identity | null; selection_revision: string | null; selection_state: string; candidates: Candidate[]; active_voice: Identity | null; active_state: string };
  type Generated = { identity: Identity; description: string; text: string; created_at_ms: number };
  type Result = { kind: "status"; value: VoiceStatus } | { kind: "candidate"; value: Generated } | { kind: "changed" };
  let panel = $state<string | null>(null);
  let status = $state<VoiceStatus | null>(null);
  let candidate = $state<Candidate | null>(null);
  let designerOpen = $state(false);
  let description = $state("");
  let busy = $state("");
  let error = $state("");
  let note = $state("");
  let previewText = $state("");
  let mounted = true;
  let operation = 0;
  const referenceText = "This is Avesra. Here is a short preview of the voice you described.";
  const selected = $derived(status?.candidates.find(c => c.identity?.id === status?.selected?.id && c.identity?.revision === status?.selected?.revision) ?? null);
  const shown = $derived(candidate ?? selected);
  const available = $derived(native && !!panel && !!runtime?.connected && !runtime?.locked);
  const count = $derived(Array.from(description).length);
  const isSelected = $derived(!!candidate?.identity && candidate.identity.id === status?.selected?.id && candidate.identity.revision === status?.selected?.revision);
  $effect(() => { const connected = runtime?.connected; const current = panel; if (current && connected) untrack(() => { if (!busy) void refresh(); }); if (!connected) untrack(() => { status = null; }); });
  async function call(value: Record<string, unknown>): Promise<Result> {
    if (!panel) throw new Error("Voice settings are not ready.");
    return command<Result>("voice_operation", { panel, command: value });
  }
  async function refresh() {
    if (!available || busy) return;
    const token = ++operation; busy = "refresh"; error = "";
    try { const result = await call({ operation: "status" }); if (mounted && token === operation && result.kind === "status") status = result.value; }
    catch (e) { if (mounted && token === operation) error = String(e); }
    finally { if (mounted && token === operation) busy = ""; }
  }
  async function change(value: Record<string, unknown>, kind: string) {
    if (!available || busy) return;
    const token = ++operation; busy = kind; error = ""; note = "";
    try {
      const result = await call(value);
      if (!mounted || token !== operation) return;
      if (result.kind === "candidate") { const c = result.value; candidate = { id: c.identity.id, revision: c.identity.revision, state: "available", identity: c.identity, description: c.description, text: c.text, created_at_ms: c.created_at_ms }; note = "Candidate generated. Preview its reference before selecting."; }
      else { candidate = null; note = "Voice setting updated."; }
      const fresh = await call({ operation: "status" });
      if (mounted && token === operation && fresh.kind === "status") status = fresh.value;
    } catch (e) {
      if (mounted && token === operation) { error = `${String(e)} No automatic retry was made. Refresh to check the current selection.`; status = null; }
    } finally { if (mounted && token === operation) busy = ""; }
  }
  async function preview() {
    if (!available || busy || !shown?.identity) return;
    const token = ++operation; busy = "preview"; error = ""; note = ""; previewText = shown.text ?? "";
    try { const result = await command<string>("preview_voice", { panel, voice: shown.identity }); if (mounted && token === operation) note = result; }
    catch (e) { if (mounted && token === operation) error = String(e); }
    finally { if (mounted && token === operation) busy = ""; }
  }
  onMount(() => {
    mounted = true;
    if (native) void command<string>("open_voice_panel").then(id => {
      if (mounted) panel = id;
      else void command("close_voice_panel", { panel: id }).catch(() => {});
    }).catch(e => { if (mounted) error = String(e); });
    return () => { mounted = false; operation++; if (panel && native) void command("close_voice_panel", { panel }).catch(() => {}); };
  });
</script>

<!-- Ported from design/mockups/Settings.dc.html 179–224; all metadata is actual. -->
<section class="flex flex-col gap-3">
  <span class="av-kicker">Voice</span>
  <div class="av-card flex items-center gap-3 p-3.5">
    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <span class="truncate text-[13px] font-medium text-zinc-50">{shown?.description ?? (status?.selected ? "Selected voice unavailable" : "No voice selected")}</span>
      <span class="av-hint">{candidate && !isSelected ? "Generated candidate · not selected" : status?.active_state === "available" ? "Selected reference · synthesis unqualified" : status ? "Voice service has no usable selected prompt" : "Pair Spark and refresh voice status"}{shown?.created_at_ms ? ` · ${new Date(shown.created_at_ms).toLocaleDateString()}` : ""}</span>
    </div>
    <button type="button" class="av-btn av-btn-secondary av-btn-sm" disabled={!available || !!busy || !shown?.identity || !runtime?.settings.speaker || runtime?.settings.deafened || runtime?.settings.paused} onclick={preview}>
      <svg width="10" height="10" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 4.5v15l12-7.5z" fill="currentColor"></path></svg>Preview
    </button>
    <button type="button" class="av-btn av-btn-ghost av-btn-sm" aria-expanded={designerOpen} onclick={() => designerOpen = !designerOpen}>{designerOpen ? "Close designer" : "Design"}</button>
  </div>
  {#if previewText}<div class="av-card flex items-center gap-3 px-3.5 py-2.5"><span class="min-w-0 flex-1 text-[12px] leading-[17px] text-zinc-300">“{previewText}”</span><span class="shrink-0 font-mono text-[10.5px] text-zinc-400">{busy === "preview" ? "Reference preview" : "Generated reference"}</span></div>{/if}
  {#if designerOpen}
    <div class="av-rise av-card flex flex-col gap-2.5 p-3.5">
      <label class="av-label" for="vdesc">Describe the voice</label>
      <textarea id="vdesc" rows="3" class="av-input" bind:value={description} disabled={!!busy} aria-invalid={count > 1024}></textarea>
      <div class="flex items-center justify-between gap-3"><span class="av-hint">Describe tone, pace and character. Creates a generated reference.</span><span class="font-mono text-[10.5px] text-zinc-400">{count}/1024</span></div>
      <p class="av-hint">Reference text: “{referenceText}”</p>
      {#if status?.candidates.length}
        <label class="av-label" for="voice-candidate">Saved candidates</label>
        <select id="voice-candidate" class="av-input" disabled={!!busy} value={candidate ? `${candidate.id}:${candidate.revision}` : ""} onchange={e => { candidate = status?.candidates.find(c => `${c.id}:${c.revision}` === e.currentTarget.value) ?? null; if (candidate?.description) description = candidate.description; }}>
          <option value="">Choose a candidate</option>{#each status.candidates as c}<option value={`${c.id}:${c.revision}`}>{c.description ?? "Unavailable candidate"} · {c.id.slice(0,8)}</option>{/each}
        </select>
      {/if}
      <div class="flex items-center gap-2">
        <span class="av-hint flex-1">{busy === "generate" ? "Generating candidate…" : candidate?.state === "unavailable" ? "This record can be discarded." : "Preview keeps your current selection."}</span>
        {#if candidate}
          <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || !!busy || isSelected} onclick={() => change({ operation: "discard", id: candidate!.id, revision: candidate!.revision }, "discard")}>Discard</button>
          <button class="av-btn av-btn-primary av-btn-sm" disabled={!available || !!busy || !candidate.identity || isSelected || !status} onclick={() => change({ operation: "select", voice: candidate!.identity, expected_selection: status?.selection_revision }, "select")}>Use this voice</button>
        {/if}
        <button class="av-btn av-btn-secondary av-btn-sm" disabled={!available || !!busy || !description.trim() || count > 1024} onclick={() => change({ operation: "generate", text: referenceText, description }, "generate")}>{busy === "generate" ? "Generating…" : "Generate"}</button>
      </div>
    </div>
  {/if}
  <div class="flex items-center gap-2"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || !!busy} onclick={refresh}>{busy === "refresh" ? "Refreshing…" : "Refresh status"}</button>{#if status?.selection_revision}<button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || !!busy} onclick={() => change({ operation: "clear", expected_selection: status!.selection_revision }, "clear")}>Clear selection</button>{/if}<span class="av-hint">Preview plays the generated reference, without selecting it.</span></div>
  {#if error}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
  {#if note}<p class="av-hint" role="status">{note}</p>{/if}
  <div class="grid grid-cols-2 gap-5">
    <div class="flex flex-col gap-2"><div class="flex items-center justify-between"><label class="av-label" for="speed">Pace</label><span class="font-mono text-[11.5px] text-zinc-300">1.0×</span></div><input id="speed" type="range" min="0.8" max="1.5" step="0.05" class="av-range" value="1" disabled /><span class="av-hint">Streaming pace adjustment is unavailable.</span></div>
    <div class="flex flex-col gap-2"><div class="flex items-center justify-between"><label class="av-label" for="vol">Voice volume</label><span class="font-mono text-[11.5px] text-zinc-300">{runtime?.settings.speech_volume ?? 80}%</span></div><input id="vol" type="range" min="0" max="100" step="1" class="av-range" value={runtime?.settings.speech_volume ?? 80} disabled={!runtime || saving || !!busy} onchange={e => update({ speech_volume: Number(e.currentTarget.value) })} /></div>
  </div>
</section>
