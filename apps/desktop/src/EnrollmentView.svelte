<script lang="ts">
  import { onMount, untrack, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import OwnerName from "./OwnerName.svelte";
  import ActorRegistration from "./ActorRegistration.svelte";
  import YourVoiceCard from "./YourVoiceCard.svelte";
  import VoiceCheck from "./VoiceCheck.svelte";
  import { command, native, type Runtime } from "./runtime";
  import { ensureManagementVerification } from "./setup";
  import { latestRead } from "./latest-read";
  import { readSpeakerStore, unavailableAvatar, type VoiceAvatar, type SpeakerCandidate as Candidate } from "./speaker-profiles";
  import { personalVoiceView, type RegistrationView } from "./owner-setup";
  let { runtime, navigate, control }: { runtime: Runtime | null; navigate: (section: string, target?: string) => void; control: (value: string) => Promise<void> } = $props();
  type Status = { enrollment: string; completed_segments: number; next_segment: string | null; reason: string };
  let status = $state<Status | null>(null);
  let candidates = $state<Candidate[]>([]);
  let avatar = $state<VoiceAvatar>(unavailableAvatar());
  let visible = $state(true);
  let readEpoch = 0;
  let eventsReady = false;
  let sourceContext = "";
  let diagnosticCandidate = $state<Candidate | null>(null);
  let diagnostics: HTMLDivElement | undefined = $state();
  let recordingTools: HTMLDetailsElement | undefined = $state();
  function readContext() { return `${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}:${runtime?.capture_epoch}:${runtime?.settings.microphone}:${visible}:${readEpoch}`; }
  function clearVoiceView() { readEpoch++; avatar = unavailableAvatar(); candidates = []; diagnosticCandidate = null; candidatesLoaded = false; storageDirectory = ""; }
  const boundCandidate = $derived(avatar.candidate ? candidates.find(c => c.id === avatar.candidate?.id && c.revision === avatar.candidate?.revision && c.segments === 6) : undefined);
  const listening = $derived(!!runtime?.connected && !runtime.locked && runtime.voice_ready && !runtime.settings.explicit_mute && !runtime.settings.deafened && !runtime.settings.paused);
  const listeningLabel = $derived(!runtime?.connected ? "Disconnected" : runtime.locked ? "Locked" : runtime.settings.paused ? "Paused" : runtime.settings.deafened ? "Deafened" : runtime.settings.explicit_mute ? "Muted" : "Not listening");
  async function openVoiceTool(test: boolean) {
    if (test) diagnosticCandidate = boundCandidate ?? null;
    advanced = true;
    await tick();
    if (!mounted || !visible) return;
    if (test) diagnostics?.scrollIntoView({ block: "nearest" });
    else if (recordingTools) { recordingTools.open = true; recordingTools.scrollIntoView({ block: "nearest" }); }
  }
  type Owner = { state: string; actor: string | null; revision: string | null };
  let owner = $state<Owner | null>(null);
  const displayName = $derived(owner?.state === "configured" && runtime?.settings.owner_name?.actor === owner.actor ? runtime.settings.owner_name.name : "You");
  let ownerLoading = $state(true), ownerError = $state("");
  let registration = $state<RegistrationView>({state: "waiting", detail: ""});
  let registrationBusy = $state(false);
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
  let storageDirectory = $state("");
  let checkingSavedVoice = $state(false), savedVoiceCheckResult = $state("");
  const savedVoice = $derived(candidates.find(c => c.state === "selected_quality_unqualified" && c.segments === 6) ?? candidates.find(c => c.segments === 6));
  const checkCandidate = $derived(diagnosticCandidate ?? savedVoice);
  const savedVoiceReadError = $derived(candidatesError || (candidatesLoaded && candidates.length > 0 && !savedVoice ? "Your saved voice was found, but Avesra couldn't read it. Keep your recordings and retry the check; you do not need to record six new phrases." : ""));
  const ownerComplete = $derived(owner?.state === "configured");
  const voice = $derived(personalVoiceView(runtime));
  let ownsEnrollment = false;
  let advanced = $state(false);
  const unavailable = $derived(!runtime?.connected ? "Connect your Spark to continue setup." : runtime.locked ? "Unlock Windows to continue setup." : runtime.settings.paused ? "Resume Avesra before continuing setup." : "");
  const acting = $derived(busy || registrationBusy);
  const audioAction = $derived(runtime?.settings.paused ? { label: "Resume Avesra", value: "resume" } : runtime?.settings.deafened ? { label: "Turn off Deafen", value: "undeafen" } : runtime?.settings.explicit_mute ? { label: "Unmute microphone", value: "unmute" } : null);
  function restoreAudio() { if (audioAction) void control(audioAction.value).catch(e => error = String(e)); else navigate("audio"); }
  const candidateReader = latestRead(
    async () => {
      const key = readContext();
      if (!eventsReady || !visible || runtime?.locked) return { key, value: null, error: "" };
      try { return { key, value: await readSpeakerStore(), error: "" }; }
      catch (e) { return { key, value: null, error: String(e) }; }
    },
    next => {
      if (!mounted || !visible || runtime?.locked || next.key !== readContext()) return;
      if (next.value) { candidates = next.value.candidates; avatar = next.value.avatar; storageDirectory = next.value.storage_directory; candidatesLoaded = true; candidatesError = ""; }
      else { avatar = unavailableAvatar(next.error || undefined); candidatesError = next.error; }
    },
    error => { candidatesError = String(error); },
    active => { candidatesLoading = active; },
  );
  const enrollmentBlock = $derived(owner?.state !== "configured" ? "Create your owner identity first." : !runtime?.settings.microphone ? "Choose a microphone in Audio & Voice." : runtime.settings.explicit_mute ? "Unmute your microphone in Audio & Voice before starting enrollment." : runtime.settings.deafened ? "Turn off Deafen before starting enrollment." : runtime.settings.paused ? "Resume Avesra before starting enrollment." : runtime.locked ? "Unlock Windows before starting enrollment." : "");
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}:${visible}`;
    if (mounted && next !== ownerContext) { ownerContext = next; ownerGeneration++; owner = null; error = ""; note = ""; if (native && visible && !runtime?.locked) untrack(() => void refreshOwner()); }
  });
  $effect(() => {
    const next = `${runtime?.capture_epoch}:${runtime?.action_epoch}:${runtime?.connected}:${runtime?.locked}:${runtime?.settings.microphone}:${visible}`;
    if (next !== context) { context = next;
      const source = `${runtime?.action_epoch}:${runtime?.connected}:${runtime?.locked}:${runtime?.settings.microphone}:${visible}`;
      if (source !== sourceContext) { sourceContext = source; untrack(clearVoiceView); }
      else untrack(() => { readEpoch++; avatar = unavailableAvatar(); }); if (!busy) { generation++; status = null; } if (mounted && native && visible && !runtime?.locked) untrack(() => void refresh().catch(e => { if (mounted) error = String(e); })); }
  });
  async function refresh() {
    await candidateReader.refresh();
  }
  async function checkSavedVoice() {
    if (checkingSavedVoice) return;
    checkingSavedVoice = true; savedVoiceCheckResult = "";
    await refresh();
    if (!mounted) return;
    const checked = new Date().toLocaleTimeString();
    savedVoiceCheckResult = savedVoiceReadError ? `Checked at ${checked}. The saved voice could not be read; see the error above.`
      : savedVoice ? `Checked at ${checked}. Found your saved voice: all 6 phrases are intact. You do not need to record again.`
      : `Checked at ${checked}. No saved voices were found in this Windows account's Avesra folder. If you already recorded six phrases, do not repeat them; the folder checked is shown below.`;
    checkingSavedVoice = false;
  }
  function registrationChanged(next: RegistrationView) {
    const confirmed = untrack(() => registration.state) !== "registered" && next.state === "registered";
    registration = next;
    if (confirmed && mounted && native) untrack(() => void refresh());
  }
  async function refreshOwner() {
    const current = ++ownerGeneration;
    ownerLoading = true; ownerError = "";
    try { const next = await command<Owner>("owner_status"); if (mounted && current === ownerGeneration) owner = next; }
    catch (e) { if (current === ownerGeneration) { owner = null; ownerError = String(e); } }
    finally { if (current === ownerGeneration) ownerLoading = false; }
  }
  async function verifyManagement(current: number) {
    await ensureManagementVerification(() => mounted && current === ownerGeneration && !!runtime?.connected && !runtime.locked, message => progress = message);
  }
  async function createOwner() {
    if (busy) return;
    busy = true; error = ""; note = ""; const current = ++ownerGeneration;
    try { await verifyManagement(current); progress = "Creating your owner account…"; const next = await command<Owner>("create_owner"); if (mounted && current === ownerGeneration) { owner = next; note = "Owner account saved. Continue with Spark registration below."; } }
    catch (e) { if (current === ownerGeneration) { owner = null; error = String(e); } }
    finally { busy = false; progress = ""; }
  }
  async function prepare(replacement = false) {
    if (busy || enrollmentBlock) return;
    busy = true; error = ""; note = "";
    const current = ++generation;
    const ownerCurrent = ownerGeneration;
    try {
      if (!replacement) {
        progress = "Checking for your saved voice…";
        await refresh();
        if (!mounted || current !== generation) return;
        if (savedVoiceReadError || !candidatesLoaded) return;
        if (candidates.length) { note = "Your six voice phrases are already saved. No new recording is needed."; return; }
      }
      await verifyManagement(ownerCurrent); if (!mounted || current !== generation) return; progress = "Preparing voice enrollment…"; ownsEnrollment = true; const next = await command<Status>("begin_enrollment"); if (mounted && current === generation) status = next;
    }
    catch (e) { if (current === generation) error = String(e); }
    finally { busy = false; progress = ""; }
  }
  async function cancel() { generation++; status = null; await command("cancel_setup"); ownsEnrollment = false; }
  const prompts = ["Read naturally: Avesra helps me keep track of my work and the things I want to do today.", "Read naturally: I can pause the assistant whenever I need a quiet moment to think.", "Read naturally: The next project will take several careful steps, and I want to review each result.", "Speak naturally for eight seconds about a typical part of your day.", "Held-out phrase: A clear voice carries across the room while the afternoon light changes.", "Held-out phrase: Tomorrow I may choose a different task, but today I will finish this one."];
  async function record() {
    busy = true; error = "";
    const current = ++generation;
    try {
      const next = await command<Status>("record_enrollment");
      if (!mounted || current !== generation) return;
      status = next.enrollment === "unavailable" ? null : next; if (!status) ownsEnrollment = false;
      if (status?.completed_segments === 6) { progress = "Saving your six voice phrases…"; await command("finish_enrollment"); ownsEnrollment = false; if (mounted && current === generation) { status = null; note = "All six voice phrases are saved."; await refresh(); } }
    }
    catch (e) {
      if (current === generation) {
        error = String(e);
        try { const next = await command<Status>("setup_status"); if (mounted && current === generation) status = next.enrollment === "unavailable" ? null : next; if (!status) ownsEnrollment = false; }
        catch { if (current === generation) status = null; }
      }
    }
    finally { busy = false; progress = ""; }
  }
  async function save() {
    busy = true; error = "";
    try { await command("finish_enrollment"); ownsEnrollment = false; status = null; await refresh(); }
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
  onMount(() => {
    mounted = true; visible = !document.hidden;
    const visibilityChanged = () => { visible = !document.hidden; clearVoiceView(); };
    document.addEventListener("visibilitychange", visibilityChanged);
    let stopHidden: (() => void) | undefined;
    if (native) void listen("settings-hidden", () => { visible = false; clearVoiceView(); ownerGeneration++; owner = null; }).then(stop => { if (mounted) { stopHidden = stop; eventsReady = true; if (visible) void refresh(); } else stop(); }).catch(() => { if (mounted) { visible = false; clearVoiceView(); } });
    const refreshSavedVoice = () => { if (native && mounted && eventsReady && !document.hidden) { visible = true; void refresh(); } };
    refreshSavedVoice();
    window.addEventListener("focus", refreshSavedVoice);
    return () => { mounted = false; stopHidden?.(); document.removeEventListener("visibilitychange", visibilityChanged); clearVoiceView(); window.removeEventListener("focus", refreshSavedVoice); candidateReader.dispose(); generation++; ownerGeneration++; if (native && ownsEnrollment) void command("cancel_setup").catch(() => {}); };
  });
</script>
<section class="section">
  <YourVoiceCard {avatar} candidate={boundCandidate} name={displayName} owner={ownerComplete} {listening} paused={listeningLabel} loading={candidatesLoading} blocked={!native || !visible || !!runtime?.locked || acting} {prompts} ontest={() => void openVoiceTool(true)} onenroll={() => void openVoiceTool(false)} />
  {#if error && !advanced}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
  <details class="av-card p-3.5" bind:open={advanced}>
    <summary class="cursor-pointer text-[12.5px] font-medium">Advanced voice tools</summary>
    <p class="av-hint my-3">Optional owner management, saved recordings and diagnostics. These are not required to start talking.</p>
    {#if advanced}
    <OwnerName {runtime} />
    <p class="av-hint">{voice.reason}</p>
    <div class="flex flex-wrap gap-2 pt-1">
      {#if !runtime?.connected}<button class="av-btn av-btn-primary av-btn-sm" onclick={() => navigate("profiles", "spark-pairing")}>Connect Spark</button>{/if}
      {#if audioAction}<button class="av-btn av-btn-secondary av-btn-sm" disabled={!native} onclick={restoreAudio}>{audioAction.label}</button>{:else}<button class="av-btn av-btn-secondary av-btn-sm" disabled={!native || !runtime} onclick={() => control("mute").catch(e => error = String(e))}>Mute microphone</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={!native || !runtime} onclick={() => control("pause").catch(e => error = String(e))}>Pause Avesra</button>{/if}
      <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => navigate("audio")}>Audio &amp; Voice</button>
    </div>
  <div class="av-card px-3.5">
    <div class="flex flex-col gap-2 py-3">
      <div class="flex items-center gap-3"><span class="min-w-0 flex-1 text-[13px] font-medium text-zinc-100">Create your owner account</span><span class="av-chip {ownerComplete ? 'text-emerald-300 ring-emerald-400/30' : 'text-zinc-300 ring-white/15'}">{ownerLoading ? "Checking…" : ownerComplete ? "Complete" : ownerError ? "Check failed" : "Action needed"}</span></div>
      <p class="av-hint ml-7">{ownerLoading ? "Reading your saved owner account…" : ownerComplete ? "Your owner account is saved for this Windows user." : ownerError ? `Couldn't read your owner account. ${ownerError}` : "Create an owner account for this Windows user. Windows will ask you to verify."}</p>
      {#if !ownerLoading && !ownerComplete}<button class="av-btn av-btn-primary av-btn-sm ml-7 self-start" disabled={acting || !!unavailable || !native} onclick={ownerError || owner?.state !== "missing" ? refreshOwner : createOwner}>{ownerError || owner?.state !== "missing" ? "Retry owner check" : "Create owner account"}</button>{/if}
    </div>
    <ActorRegistration {runtime} ownerReady={ownerComplete} parentBusy={busy} onstate={registrationChanged} onbusy={value => registrationBusy = value} />
    <div class="flex flex-col gap-2 border-t border-white/10 py-3">
      <div class="flex items-center gap-3"><span class="min-w-0 flex-1 text-[13px] font-medium text-zinc-100">Save your voice</span><span class="av-chip {savedVoice && !savedVoiceReadError ? 'text-emerald-300 ring-emerald-400/30' : 'text-zinc-300 ring-white/15'}">{checkingSavedVoice || (!candidatesLoaded && !savedVoiceReadError) ? "Checking…" : savedVoiceReadError ? "Check failed" : savedVoice ? "Complete · 6 of 6" : "Action needed"}</span></div>
      <p class="av-hint ml-7">{savedVoiceReadError || (savedVoice ? "All six phrases are saved on this PC. No new recording is needed." : !candidatesLoaded ? "Reading your saved voice…" : registration.state !== "registered" ? "Complete the owner and Spark steps above, then record six short phrases." : "Record six eight-second phrases. Avesra saves your voice after the last one.")}</p>
      {#if !savedVoiceReadError && candidatesLoaded && !savedVoice && registration.state === "registered" && !status}
        {#if enrollmentBlock}<p class="av-hint ml-7 text-amber-200">{enrollmentBlock}</p><button class="av-btn av-btn-secondary av-btn-sm ml-7 self-start" disabled={acting} onclick={restoreAudio}>{audioAction?.label ?? "Choose microphone"}</button>{:else}<button class="av-btn av-btn-primary av-btn-sm ml-7 self-start" disabled={acting || candidatesLoading || !!unavailable} onclick={() => prepare()}>Record my voice</button>{/if}
      {/if}
      <button class="av-btn av-btn-ghost av-btn-sm ml-7 self-start" disabled={acting || candidatesLoading || checkingSavedVoice || !native} onclick={checkSavedVoice}>{checkingSavedVoice ? "Checking saved voice…" : savedVoiceReadError ? "Retry saved voice check" : "Check saved voice"}</button>
      {#if checkingSavedVoice || savedVoiceCheckResult}<p class="av-hint ml-7" role="status">{checkingSavedVoice ? "Reading your saved voice from this PC…" : savedVoiceCheckResult}</p>{/if}
      {#if candidatesLoaded && !savedVoice && storageDirectory}<p class="av-hint ml-7 break-all">Folder checked: {storageDirectory}</p>{/if}
    </div>
  </div>
  {#if progress || note}<p class="av-hint" role="status">{progress || note}</p>{/if}
  {#if error}<div class="flex flex-col gap-1 border border-amber-400/25 p-3" role="alert"><span class="text-[12.5px] font-medium text-amber-200">This step couldn't finish</span><p class="av-hint">{error}</p><p class="av-hint">{status ? "Your completed phrases are still in this session. Retry the current step below." : "Your previously saved owner and voice are unchanged. Retry the step you were completing."}</p></div>{/if}
  {#if status}
    <div class="av-card flex flex-col gap-3 p-3.5">
      <h2 class="text-[14px] font-medium">{status.completed_segments === 6 ? "Save your voice" : `Phrase ${status.completed_segments + 1} of 6`}</h2>
      <progress class="h-1.5 w-full accent-[#ac315b]" value={status.completed_segments} max={6} aria-label="Saved enrollment phrases"></progress>
      <p class="text-[13px] leading-relaxed text-zinc-100">{prompts[status.completed_segments]?.replace("Held-out phrase: ", "Read naturally: ") ?? "All six phrases have been recorded. Save them to finish this step."}</p>
      <p class="av-hint" role="status">{runtime?.enrollment_capture ? "Recording now — speak naturally for eight seconds." : busy ? progress || "Checking the microphone and processing your phrase…" : "Microphone off. Press Record when you're ready."}</p>
      <div class="flex flex-wrap gap-2">
        {#if status.completed_segments < 6}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy || runtime?.settings.explicit_mute} onclick={record}>Record phrase {status.completed_segments + 1}</button>{:else}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy} onclick={save}>Save my voice</button>{/if}
        <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => cancel().catch(e => error = String(e))}>Cancel recording session</button>
      </div>
    </div>
  {/if}
  {#if checkCandidate}
    <div class="av-card p-3.5" bind:this={diagnostics}>
      <h2 class="text-[12.5px] font-medium">Optional voice diagnostics</h2>
      {#if checkCandidate.id !== boundCandidate?.id || checkCandidate.revision !== boundCandidate?.revision}<p class="av-hint">Saved candidate diagnostics; this candidate is not currently linked to the displayed avatar.</p>{/if}
      {#if enrollmentBlock}<p class="av-hint mt-2 text-amber-200">{enrollmentBlock}</p><button class="av-btn av-btn-secondary av-btn-sm mt-2" disabled={acting} onclick={restoreAudio}>{audioAction?.label ?? "Open audio settings"}</button>{/if}
      <VoiceCheck id={checkCandidate.id} revision={checkCandidate.revision} {runtime} blocked={acting || !!status || !!enrollmentBlock || !ownerComplete || registration.state !== "registered"} onbusy={value => busy = value} />
    </div>
  {/if}
  <details class="av-card p-3.5" bind:this={recordingTools}>
    <summary class="cursor-pointer text-[12.5px] font-medium">Manage saved voice · advanced</summary>
    <p class="av-hint mt-3">These options replace or remove saved setup. They are not required for normal listening.</p>
    <div class="my-3 flex flex-wrap gap-2"><button class="av-btn av-btn-secondary av-btn-sm" disabled={acting || !!enrollmentBlock || !!unavailable || !!status} onclick={() => prepare(true)}>Record a new voice…</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={acting || candidatesLoading || checkingSavedVoice || !native} onclick={checkSavedVoice}>{checkingSavedVoice ? "Checking saved voice…" : "Reload saved voices"}</button></div>
    {#if storageDirectory}<p class="av-hint break-all">Saved voice folder: {storageDirectory}</p>{/if}
    {#each candidates as candidate, i (candidate.revision)}
      <div class="flex flex-col gap-2 border-t border-white/10 py-3">
        <span class="text-[12.5px] font-medium">Saved voice {i + 1} · {candidate.segments === 6 ? "6 phrases" : "Could not read"}{candidate.state === "selected_quality_unqualified" ? " · selected" : ""}</span>
        <p class="av-hint">Selecting a voice stores your recognition preference. Native listening status above shows when it is in use.</p>
        {#if candidate.read_error}<p class="av-hint text-amber-200">{candidate.read_error}. The saved file has not been changed.</p>{/if}
        <div class="flex flex-wrap gap-2"><button class="av-btn av-btn-secondary av-btn-sm" disabled={acting || candidate.state !== "candidate_quality_unqualified"} onclick={() => select(candidate)}>Select this saved voice</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={acting} onclick={() => remove(candidate)}>Delete saved voice…</button></div>
      </div>
    {/each}
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={acting} onclick={() => select(null)}>Clear saved voice selection…</button>
    <p class="av-hint mt-2">Changes ask for Windows verification when needed.</p>
  </details>
    {/if}
  </details>
</section>
