<script lang="ts">
  import SelectFrame from "./SelectFrame.svelte";
  import { onMount, untrack } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
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
  let storageError = $state("");
  let repeating = $state(false);
  let rememberedCandidate = "";
  let hasDraft = false;
  const draftKey = "avesra.voice-designer.v1";
  let mounted = true;
  let operation = 0;
  let publication = 0;
  const referenceText = "Hello I am Avesra, A Very Effective Smart Reasoning Assistant. I am designed to help you manage your thoughts and ideas.";
  let testText = $state(referenceText);
  const testBytes = $derived(new TextEncoder().encode(testText).length);
  const testTextError = $derived(!testText.trim() ? "Enter something for Avesra to say." : testBytes > 512 ? "Shorten the text to 512 bytes or fewer." : /[\p{Cc}\p{Cs}]/u.test(testText.replace(/[\n\t]/g, "")) ? "Remove unsupported control characters from the text." : "");
  function sameVoice(a: Identity | null | undefined, b: Identity | null | undefined) {
    return !!a && !!b && a.id === b.id && a.revision === b.revision && a.audio_sha256 === b.audio_sha256 && a.metadata_sha256 === b.metadata_sha256;
  }
  const testVoiceReady = $derived(status?.selection_state === "available" && status.active_state === "available"
    && sameVoice(status.selected, status.active_voice)
    && status.candidates.some(c => c.state === "available" && sameVoice(c.identity, status?.selected)));
  const selected = $derived(status?.candidates.find(c => c.identity?.id === status?.selected?.id && c.identity?.revision === status?.selected?.revision) ?? null);
  const shown = $derived(candidate ?? selected);
  const shownReference = $derived(busy === "preview" ? previewText : shown?.text ?? "");
  const available = $derived(native && !!panel && !!runtime?.connected && !runtime?.locked);
  const count = $derived(description.length);
  const isSelected = $derived(!!candidate?.identity && candidate.identity.id === status?.selected?.id && candidate.identity.revision === status?.selected?.revision);
  const playbackBlock = $derived(!runtime?.settings.speaker ? "Choose a speaker in Audio devices to hear the preview." : runtime.settings.deafened ? "Turn off Deafen to hear the preview." : runtime.settings.paused ? "Resume Avesra to hear the preview." : "");
  $effect(() => { if (!runtime?.connected || runtime?.locked || playbackBlock) repeating = false; });
  async function volume(value: string) {
    error = "";
    try { await command("update_sound", { edit: { kind: "volume", value: Number(value) } }); }
    catch (e) { error = String(e); }
  }
  function saveDraft(edited = true) {
    if (edited) hasDraft = true;
    try {
      localStorage.setItem(draftKey, JSON.stringify({ version: 1, description, hasDescription: hasDraft, candidate: rememberedCandidate, open: designerOpen }));
      storageError = "";
    } catch { storageError = "Your voice draft could not be saved on this PC. Keep this page open to avoid losing edits."; }
  }
  function restoreDraft() {
    try {
      const raw = localStorage.getItem(draftKey);
      if (!raw) return;
      if (raw.length > 8192) throw new Error("Invalid draft");
      const draft = JSON.parse(raw);
      if (draft.version !== 1 || typeof draft.description !== "string" || draft.description.length > 4096 || typeof draft.hasDescription !== "boolean" || typeof draft.open !== "boolean" || typeof draft.candidate !== "string" || (draft.candidate !== "" && !/^[0-9a-f-]{36}:[0-9a-f-]{36}$/.test(draft.candidate))) throw new Error("Invalid draft");
      description = draft.description; hasDraft = draft.hasDescription; rememberedCandidate = draft.candidate;
    } catch { storageError = "The saved voice draft could not be restored. Saved voices can still be loaded from Spark."; }
  }
  function acceptStatus(value: VoiceStatus) {
    status = value;
    candidate = value.candidates.find(c => `${c.id}:${c.revision}` === rememberedCandidate) ?? null;
    if (!hasDraft) {
      candidate ??= value.candidates.find(c => c.identity?.id === value.selected?.id && c.identity?.revision === value.selected?.revision) ?? [...value.candidates].filter(c => c.state === "available").sort((a, b) => (b.created_at_ms ?? 0) - (a.created_at_ms ?? 0))[0] ?? null;
      if (candidate) {
        description = candidate.description ?? "";
        rememberedCandidate = `${candidate.id}:${candidate.revision}`;
        saveDraft();
      }
    } else if (rememberedCandidate && !candidate) {
      rememberedCandidate = "";
      saveDraft(false);
    }
  }
  $effect(() => { const connected = runtime?.connected; const locked = runtime?.locked; const current = panel; if (current && connected && !locked) untrack(() => { if (!busy) void refresh(); }); if (!connected || locked) untrack(() => { publication++; status = null; candidate = null; note = ""; previewText = ""; }); });
  async function call(value: Record<string, unknown>): Promise<Result> {
    if (!panel) throw new Error("Voice settings are not ready.");
    return command<Result>("voice_operation", { panel, command: value });
  }
  async function refresh() {
    if (!available || busy) return;
    const token = ++operation; const visible = publication; busy = "refresh"; error = "";
    try { const result = await call({ operation: "status" }); if (mounted && token === operation && visible === publication && result.kind === "status") acceptStatus(result.value); }
    catch (e) { if (mounted && token === operation && visible === publication) { status = null; error = String(e); } }
    finally { if (mounted && token === operation) busy = ""; }
  }
  async function change(value: Record<string, unknown>, kind: string) {
    if (!available || busy) return;
    const token = ++operation; const visible = publication; busy = kind; error = ""; note = "";
    try {
      const result = await call(value);
      if (!mounted || token !== operation || visible !== publication) return;
      if (result.kind === "candidate") { const c = result.value; candidate = { id: c.identity.id, revision: c.identity.revision, state: "available", identity: c.identity, description: c.description, text: c.text, created_at_ms: c.created_at_ms }; rememberedCandidate = `${candidate.id}:${candidate.revision}`; saveDraft(); note = "Voice generated and saved."; }
      else { if (kind === "discard") { candidate = null; rememberedCandidate = ""; saveDraft(false); } note = "Voice setting updated."; }
      const fresh = await call({ operation: "status" });
      if (!mounted || token !== operation || visible !== publication) return;
      if (fresh.kind === "status") acceptStatus(fresh.value);
      if (kind === "generate" && result.kind === "candidate") {
        if (playbackBlock) { note = `Voice saved. ${playbackBlock}`; return; }
        busy = "preview"; previewText = result.value.text;
        try {
          const playback = await command<string>("preview_voice", { panel, voice: result.value.identity });
          if (mounted && token === operation && visible === publication) note = `Voice saved. ${playback}`;
        } catch (e) {
          if (mounted && token === operation && visible === publication) { error = String(e); note = "Your voice is saved. Use Play preview to try playback again."; }
        }
      }
    } catch (e) {
      if (mounted && token === operation && visible === publication) { error = String(e); note = "Refresh status to check for a saved change before retrying."; status = null; }
    } finally { if (mounted && token === operation) busy = ""; }
  }
  async function preview(repeat = false) {
    if (!available || busy || !shown?.identity) return;
    repeating = repeat;
    const voice = shown.identity;
    const token = ++operation; const visible = publication; busy = "preview"; error = ""; note = ""; previewText = shown.text ?? "";
    try {
      do {
        const result = await command<string>("preview_voice", { panel, voice });
        if (!mounted || token !== operation || visible !== publication) break;
        note = result;
        if (!repeating) break;
        await new Promise(resolve => setTimeout(resolve, 900));
      } while (repeating && mounted && available && !playbackBlock && token === operation && visible === publication);
    }
    catch (e) { if (mounted && token === operation && visible === publication) error = String(e); }
    finally { if (mounted && token === operation) { busy = ""; repeating = false; } }
  }
  async function playText() {
    if (!available || busy || playbackBlock || !testVoiceReady || !status?.selected || testTextError) return;
    const voice = { ...status.selected }; const text = testText;
    const token = ++operation; const visible = publication; busy = "test"; error = ""; note = "";
    try {
      const result = await command<string>("preview_voice", { panel, voice, text });
      if (mounted && token === operation && visible === publication) note = result;
    }
    catch (e) { if (mounted && token === operation && visible === publication) error = String(e); }
    finally { if (mounted && token === operation) busy = ""; }
  }
  async function openPanel() {
    if (!native || busy || panel) return;
    busy = "opening"; error = "";
    try { const id = await command<string>("open_voice_panel"); if (mounted) panel = id; else void command("close_voice_panel", { panel: id }).catch(() => {}); }
    catch (e) { if (mounted) error = String(e); }
    finally { if (mounted) busy = ""; }
    if (mounted && panel && available) void refresh();
  }
  onMount(() => {
    mounted = true;
    const stopRepeatingWhenHidden = () => { if (document.hidden) repeating = false; };
    document.addEventListener("visibilitychange", stopRepeatingWhenHidden);
    restoreDraft();
    if (native) void openPanel();
    return () => { mounted = false; repeating = false; operation++; document.removeEventListener("visibilitychange", stopRepeatingWhenHidden); if (panel && native) void command("close_voice_panel", { panel }).catch(() => {}); };
  });
</script>

<!-- Ported from design/mockups/Settings.dc.html 179–224; all metadata is actual. -->
<section class="flex flex-col gap-3" aria-busy={!!busy}>
  <span class="av-kicker">Voice</span>
  <div class="av-card flex items-center gap-3 p-3.5">
    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <span class="truncate text-[13px] font-medium text-zinc-50">{shown?.description ?? (status?.selected ? "Selected voice unavailable" : "No voice selected")}</span>
      <span class="av-hint">{candidate && !isSelected ? "Generated candidate · not selected" : status?.active_state === "available" ? "Selected reference · synthesis unqualified" : status ? "Voice service has no usable selected prompt" : "Pair Spark and refresh voice status"}{shown?.created_at_ms ? ` · ${new Date(shown.created_at_ms).toLocaleDateString()}` : ""}</span>
    </div>
    <button type="button" class="av-btn av-btn-secondary av-btn-sm" disabled={!available || !!busy || !shown?.identity || !runtime?.settings.speaker || runtime?.settings.deafened || runtime?.settings.paused} onclick={() => preview()}>
      <svg width="10" height="10" viewBox="0 0 24 24" aria-hidden="true"><path d="M7 4.5v15l12-7.5z" fill="currentColor"></path></svg>Preview
    </button>
    <button type="button" class="av-btn av-btn-ghost av-btn-sm" aria-expanded={designerOpen} onclick={() => { designerOpen = !designerOpen; saveDraft(false); }}>{designerOpen ? "Close designer" : "Design"}</button>
  </div>
  <div class="flex flex-wrap items-center gap-2">
    <button type="button" class={`av-btn av-btn-sm ${repeating ? "av-btn-primary" : "av-btn-secondary"}`} disabled={!repeating && (!available || !!busy || !shown?.identity || !!playbackBlock)} onclick={() => { if (repeating) repeating = false; else void preview(true); }}>{repeating ? "Stop repeating" : "Repeat preview"}</button>
    <span class="av-hint">{repeating ? "Tune below while it repeats. Stop finishes this take." : "Reuses this saved take. No voice regeneration."}</span>
  </div>
  <div class={`flex h-11 items-center gap-3 px-3 ring-1 ring-inset transition-colors ${busy === "preview" ? "bg-red-500/[0.06] ring-red-500/30" : "bg-black/20 ring-white/[0.06]"}`}>
    <span class="min-w-0 flex-1 truncate text-[12px] leading-[17px] text-zinc-300" title={shownReference}>{shownReference ? `“${shownReference}”` : "Saved reference text unavailable"}</span>
    <span class="shrink-0 font-mono text-[10.5px] text-zinc-400">{busy === "preview" ? "Preview in progress" : "Saved reference"}</span>
  </div>
  <div class="flex flex-col gap-1.5">
    <label class="av-label" for="voice-test-text">Test selected voice</label>
    <textarea id="voice-test-text" rows="2" maxlength="512" class={`av-input av-textarea resize-none${testTextError ? " av-invalid" : ""}`} bind:value={testText} disabled={!!busy} aria-invalid={!!testTextError} aria-describedby="voice-test-help voice-test-count"></textarea>
    <div class="flex flex-wrap items-center gap-2">
      <span id="voice-test-help" class={`min-w-0 flex-1 text-[11.5px] leading-4 ${testTextError ? "text-red-400" : "text-zinc-400"}`}>{testTextError || "Uses your selected voice, effects and atmosphere."}</span>
      <span id="voice-test-count" class={`shrink-0 font-mono text-[10.5px] ${testBytes > 512 ? "text-red-400" : "text-zinc-400"}`}>{testBytes} / 512 bytes</span>
      <button type="button" class="av-btn av-btn-ghost av-btn-sm" disabled={!!busy} onclick={() => { testText = referenceText; }}>Use default</button>
      <button type="button" class="av-btn av-btn-secondary av-btn-sm" disabled={!available || !!busy || !!playbackBlock || !testVoiceReady || !!testTextError} onclick={playText}>{busy === "test" ? "Synthesizing and playing…" : "Play text"}</button>
    </div>
    {#if !testVoiceReady || playbackBlock}<span class="av-hint">{!testVoiceReady ? "Select an available voice and refresh status to test it." : playbackBlock}</span>{/if}
  </div>
  {#if designerOpen}
    <div class="av-rise av-card flex flex-col gap-2.5 p-3.5">
      <label class="av-label" for="vdesc">Describe the voice</label>
      <textarea id="vdesc" rows="3" maxlength="4096" class={`av-input av-textarea resize-none${count > 280 ? " av-invalid" : ""}`} value={description} oninput={e => { description = e.currentTarget.value; saveDraft(); }} disabled={!!busy} aria-invalid={count > 280}></textarea>
      <div class="flex items-center justify-between gap-3"><span class={`text-[11.5px] leading-4 ${count > 280 ? "text-red-400" : "text-zinc-400"}`}>{count > 280 ? "Shorten the description to 280 characters or fewer." : "Used once to generate a voice. The saved preset is reused for replies."}</span><span class={`shrink-0 font-mono text-[10.5px] ${count > 280 ? "text-red-400" : "text-zinc-400"}`}>{count} / 280</span></div>
      <p class="av-hint">Reference text: “{referenceText}”</p>
      {#if status?.candidates.length}
        <label class="av-label" for="voice-candidate">Saved candidates</label>
        <SelectFrame><select id="voice-candidate" class="av-input av-select" disabled={!!busy} value={candidate ? `${candidate.id}:${candidate.revision}` : ""} onchange={e => { candidate = status?.candidates.find(c => `${c.id}:${c.revision}` === e.currentTarget.value) ?? null; rememberedCandidate = candidate ? `${candidate.id}:${candidate.revision}` : ""; if (candidate?.description) description = candidate.description; saveDraft(); }}>
          <option value="">Choose a candidate</option>{#each status.candidates as c}<option value={`${c.id}:${c.revision}`}>{c.description ?? "Unavailable candidate"} · {c.id.slice(0,8)}</option>{/each}
        </select></SelectFrame>
      {/if}
      <div class="flex items-center gap-2">
        <span class="av-hint flex-1">{candidate?.state === "unavailable" ? "This record can be discarded." : "Generate saves and plays a preview."}</span>
        {#if candidate}
          <button class="av-btn av-btn-secondary av-btn-sm" disabled={!available || !!busy || !candidate.identity || !!playbackBlock} onclick={() => preview()}>Play preview</button>
          <button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || !!busy || isSelected} onclick={() => change({ operation: "discard", id: candidate!.id, revision: candidate!.revision }, "discard")}>Discard</button>
          <button class="av-btn av-btn-primary av-btn-sm" disabled={!available || !!busy || !candidate.identity || isSelected || !status} onclick={() => change({ operation: "select", voice: candidate!.identity, expected_selection: status?.selection_revision }, "select")}>Use this voice</button>
        {/if}
        <button class="av-btn av-btn-secondary av-btn-sm" disabled={!available || !status || !!busy || !description.trim() || count > 280} onclick={() => change({ operation: "generate", text: referenceText, description }, "generate")}>{busy === "generate" ? "Generating…" : "Generate"}</button>
      </div>
    </div>
  {/if}
  {#if busy === "generate" || busy === "preview" || busy === "test"}
    <div class="flex items-center gap-3 border border-rose-500/30 bg-rose-500/5 p-3" role="status" aria-live="polite">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" class="av-spin shrink-0 text-av-400 motion-reduce:animate-none" aria-hidden="true"><path d="M12 3a9 9 0 1 1-9 9"></path></svg>
      <span class="av-hint">{busy === "generate" ? "Generating your voice… It will play automatically when ready." : busy === "test" ? "Synthesizing and playing your test text…" : "Playing preview… Your selected voice stays unchanged."}</span>
    </div>
  {/if}
  {#if shown?.identity && playbackBlock}<p class="av-hint text-amber-200">{playbackBlock}</p>{/if}
  <div class="flex items-center gap-2"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!native || !!busy || !!runtime?.locked || !runtime?.connected} onclick={() => panel ? refresh() : openPanel()}>{busy === "refresh" ? "Refreshing…" : "Refresh status"}</button>{#if status?.selection_revision}<button class="av-btn av-btn-ghost av-btn-sm" disabled={!available || !!busy} onclick={() => change({ operation: "clear", expected_selection: status!.selection_revision }, "clear")}>Clear selection</button>{/if}<span class="av-hint">Preview plays the generated reference, without selecting it.</span></div>
  {#if error}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
  {#if storageError}<p class="av-hint text-amber-200" role="alert">{storageError}</p>{/if}
  {#if note}<p class="av-hint" role="status">{note}</p>{/if}
  <div class="grid grid-cols-2 gap-5">
    <div class="flex flex-col gap-2"><div class="flex items-center justify-between"><label class="av-label" for="speed">Pace</label><span class="font-mono text-[11.5px] text-zinc-300">1.0×</span></div><input id="speed" type="range" min="0.8" max="1.5" step="0.05" class="av-range" value="1" disabled /><span class="av-hint">Streaming pace adjustment is unavailable.</span></div>
    <div class="flex flex-col gap-2"><div class="flex items-center justify-between"><label class="av-label" for="vol">Voice volume</label><span class="font-mono text-[11.5px] text-zinc-300">{runtime?.settings.speech_volume ?? 80}%</span></div><input id="vol" type="range" min="0" max="100" step="1" class="av-range" value={runtime?.settings.speech_volume ?? 80} disabled={!runtime} onchange={e => volume(e.currentTarget.value)} /><span class="av-hint">Controls the voice and its atmosphere, including during preview.</span></div>
  </div>
</section>
