<script lang="ts">
  import { onMount, untrack } from "svelte";
  import ActorRegistration from "./ActorRegistration.svelte";
  import VoiceCheck from "./VoiceCheck.svelte";
  import { command, native, type Runtime } from "./runtime";
  import { ensureManagementVerification } from "./setup";
  import { latestRead } from "./latest-read";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Status = { enrollment: string; completed_segments: number; next_segment: string | null; reason: string };
  type Candidate = { id: string; revision: string; model_revision: string | null; segments: number | null; state: string };
  let status = $state<Status | null>(null);
  let candidates = $state<Candidate[]>([]);
  type Owner = { state: string; actor: string | null; revision: string | null };
  let owner = $state<Owner | null>(null);
  let ownerGeneration = 0;
  let ownerContext = "";
  let busy = $state(false);
  let progress = $state("");
  let note = $state("");
  let mounted = $state(false);
  let error = $state("");
  let generation = 0;
  let context = "";
  let candidatesLoading = $state(false);
  let candidatesLoaded = $state(false);
  let candidatesError = $state("");
  const candidateReader = latestRead(
    () => command<Candidate[]>("speaker_candidates"),
    next => { candidates = next; candidatesLoaded = true; candidatesError = ""; },
    error => { candidatesError = String(error); },
    active => { candidatesLoading = active; },
  );
  const enrollmentBlock = $derived(owner?.state !== "configured" ? "Create your owner identity first." : !runtime?.settings.microphone ? "Choose a microphone in Audio & Voice." : runtime.settings.explicit_mute ? "Unmute your microphone in Audio & Voice before starting enrollment." : runtime.settings.deafened ? "Turn off Deafen before starting enrollment." : runtime.settings.paused ? "Resume Avesra before starting enrollment." : runtime.locked ? "Unlock Windows before starting enrollment." : "");
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}`;
    if (mounted && next !== ownerContext) { ownerContext = next; ownerGeneration++; owner = null; error = ""; note = ""; if (native) untrack(() => void refreshOwner()); }
  });
  $effect(() => {
    const next = `${runtime?.capture_epoch}:${runtime?.connected}`;
    if (next !== context) { context = next; if (!busy) { generation++; status = null; } if (mounted && native) untrack(() => void refresh().catch(e => { if (mounted) error = String(e); })); }
  });
  async function refresh() {
    await candidateReader.refresh();
  }
  async function refreshOwner() {
    const current = ++ownerGeneration;
    try { const next = await command<Owner>("owner_status"); if (mounted && current === ownerGeneration) { owner = next; error = ""; } }
    catch (e) { if (current === ownerGeneration) { owner = null; error = String(e); } }
  }
  async function verifyManagement(current: number) {
    await ensureManagementVerification(() => mounted && current === ownerGeneration && !!runtime?.connected && !runtime.locked, message => progress = message);
  }
  async function createOwner() {
    if (busy) return;
    busy = true; error = ""; note = ""; const current = ++ownerGeneration;
    try { await verifyManagement(current); progress = "Creating your owner identity…"; const next = await command<Owner>("create_owner"); if (mounted && current === ownerGeneration) { owner = next; note = "Owner created. Start voice enrollment below to record your phrases."; } }
    catch (e) { if (current === ownerGeneration) { owner = null; error = String(e); } }
    finally { busy = false; progress = ""; }
  }
  async function prepare() {
    if (busy || enrollmentBlock) return;
    busy = true; error = ""; note = "";
    const current = ++generation;
    const ownerCurrent = ownerGeneration;
    try { await verifyManagement(ownerCurrent); if (!mounted || current !== generation) return; progress = "Preparing voice enrollment…"; const next = await command<Status>("begin_enrollment"); if (mounted && current === generation) status = next; }
    catch (e) { if (current === generation) error = String(e); }
    finally { busy = false; progress = ""; }
  }
  async function cancel() { generation++; status = null; await command("cancel_setup"); }
  const prompts = ["Read naturally: Avesra helps me keep track of my work and the things I want to do today.", "Read naturally: I can pause the assistant whenever I need a quiet moment to think.", "Read naturally: The next project will take several careful steps, and I want to review each result.", "Speak naturally for eight seconds about a typical part of your day.", "Held-out phrase: A clear voice carries across the room while the afternoon light changes.", "Held-out phrase: Tomorrow I may choose a different task, but today I will finish this one."];
  async function record() {
    busy = true; error = "";
    const current = ++generation;
    try { const next = await command<Status>("record_enrollment"); if (current === generation) status = next.enrollment === "unavailable" ? null : next; }
    catch (e) { if (current === generation) { error = String(e); status = null; } }
    finally { busy = false; }
  }
  async function save() {
    busy = true; error = "";
    try { await command("finish_enrollment"); status = null; await refresh(); }
    catch(e) { error = String(e); }
    finally { busy = false; }
  }
  async function remove(candidate: Candidate) {
    busy = true; error = "";
    try { await verifyManagement(ownerGeneration); await command("delete_speaker_candidate", { id: candidate.id, revision: candidate.revision }); await refresh(); }
    catch (e) { error = String(e); }
    finally { busy = false; progress = ""; }
  }
  async function select(candidate: Candidate | null) {
    busy = true; error = "";
    try { await verifyManagement(ownerGeneration); await command(candidate ? "select_speaker_candidate" : "clear_speaker_selection", candidate ? {id: candidate.id, revision: candidate.revision} : undefined); await refresh(); }
    catch (e) { error = String(e); }
    finally { busy = false; progress = ""; }
  }
  onMount(() => { mounted = true; if (native) void refresh(); return () => { mounted = false; candidateReader.dispose(); generation++; ownerGeneration++; }; });
</script>

<section class="section">
  <span class="av-kicker">Owner</span>
  <div class="av-card flex flex-col gap-3 p-3.5">
    <div class="flex items-center gap-3"><span class="grid size-9 shrink-0 place-items-center bg-[#3a5dd8]/25 font-mono text-[12px] font-medium text-[#b9c7f5]">YOU</span><div class="flex min-w-0 flex-1 flex-col"><span class="text-[13px] font-medium text-zinc-50">{owner?.state === "configured" ? "You · local owner" : owner?.state === "missing" ? "Set up the owner" : "Owner status unavailable"}</span><span class="av-hint">{owner?.state === "configured" ? "Bound to this Windows user. Voice enrollment remains unqualified." : "Create one protected owner identity after Windows verification."}</span></div><span class="av-chip text-amber-200 ring-amber-400/30">Voice setup required</span></div>
    <div class="flex items-center gap-2">
      {#if owner?.state === "missing"}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy || !runtime?.connected || runtime.locked} onclick={createOwner}>Create owner</button>{/if}
      <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy || !native} onclick={() => refreshOwner().catch(e => error = String(e))}>Refresh owner status</button>
    </div>
    <ActorRegistration {runtime} ownerReady={owner?.state === "configured"} parentBusy={busy} />
    <p class="av-hint">{candidates.some(c => c.segments === 6) ? "Your six voice segments are saved. Continue below to check your saved voice; you do not need to record enrollment again." : "Create your owner identity, then enroll your voice with six short recordings. Recording starts only when you press Record."}</p>
    <button class="av-btn av-btn-secondary self-start" disabled={!!enrollmentBlock || busy || !runtime?.connected || !!status} onclick={prepare}>{candidates.some(c => c.segments === 6) ? "Record a replacement voice" : "Start voice enrollment"}</button>
    {#if enrollmentBlock}<p class="av-hint">{enrollmentBlock}</p>{/if}
    {#if progress}<p class="av-hint" role="status">{progress}</p>{/if}
    {#if note}<p class="av-hint" role="status">{note}</p>{/if}
  </div>
  {#if status}
    <!-- Inline enrollment card follows Settings.dc.html 605–614. -->
    <div class="av-card flex flex-col gap-2.5 p-3.5">
      <span class="text-[12.5px] font-medium text-zinc-100">Owner enrollment · {status.completed_segments} / 6 segments</span>
      <p class="av-hint">{status.reason}</p>
      <p class="text-[12.5px] text-zinc-100">{prompts[status.completed_segments] ?? "Collection complete. Save this candidate for quality review."}</p>
      <p class="av-hint" role="status">{runtime?.enrollment_capture ? "Microphone recording now · eight seconds" : busy ? "Checking the speaker service or processing the explicit phrase…" : "Microphone off between recordings."}</p>
      <div class="flex items-center justify-end gap-2.5">
        <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => cancel().catch(e => error = String(e))}>Cancel</button>
        {#if status.completed_segments < 6}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy || runtime?.settings.explicit_mute} onclick={record}>Record eight seconds</button>{:else}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy} onclick={save}>Save unqualified candidate</button>{/if}
      </div>
    </div>
  {/if}
  <div class="flex items-center justify-between gap-3">
    <span class="av-kicker">Saved voice recordings</span>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!native || candidatesLoading} onclick={refresh}>{candidatesLoading ? "Loading recordings…" : "Refresh recordings"}</button>
  </div>
  {#if candidatesError}<p class="text-xs text-amber-300" role="alert">Couldn't read saved recordings: {candidatesError}. Refresh recordings to retry.</p>{:else if candidatesLoaded && candidates.length === 0}<p class="av-hint">No saved voice recordings were found on this PC.</p>{/if}
  {#each candidates as candidate (candidate.revision)}
    <div class="av-card flex flex-col gap-2.5 p-3.5">
      <span class="text-[12.5px] font-medium">Protected voice candidate · {candidate.state === "unreadable_candidate" ? "unreadable" : candidate.state === "selected_quality_unqualified" ? "selected, unqualified" : candidate.state === "selection_unavailable" ? "selection unavailable" : "quality unqualified"}</span>
      <p class="av-hint">{candidate.segments ?? "Unknown"} segments · model revision {candidate.model_revision?.slice(0, 7) ?? "unavailable"}. This candidate grants no listening or action rights. Unreadable candidates may be removed after verification.</p>
      {#if candidate.segments === 6}
        <VoiceCheck id={candidate.id} revision={candidate.revision} {runtime} blocked={busy || !!status || !!enrollmentBlock} onbusy={value => busy = value} />
      {/if}
      <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={busy} onclick={() => remove(candidate)}>Delete candidate · Windows verification required</button>
      <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={busy || candidate.state !== "candidate_quality_unqualified"} onclick={() => select(candidate)}>Select for validation · Windows verification required</button>
    </div>
  {/each}
  <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={busy} onclick={() => select(null)}>Clear profile selection · Windows verification required</button>
  <p class="av-hint">Selecting a candidate preserves the chosen revision for validation. It does not enable listening or grant owner rights. Clear selection also recovers an unreadable selection record.</p>
  <div class="av-card flex flex-col gap-2 p-3.5">
    <span class="text-[12.5px] font-medium">Automatic recognition · implementation in progress</span>
    <p class="av-hint">Saved enrollment and a voice check are available. Automatic listening still requires calibrated recognition, overlapping-speaker and playback rejection, and assistant-directed speech checks. These are application work remaining, not missing setup steps for you to repeat.</p>
  </div>
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
</section>
