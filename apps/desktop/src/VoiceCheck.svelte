<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";
  let { id, revision, runtime, blocked, onbusy }: {
    id: string; revision: string; runtime: Runtime | null; blocked: boolean; onbusy: (value: boolean) => void;
  } = $props();
  type Condition = "owner_directed_without_name" | "speaker_switch" | "ambiguous_approval" | "owner_directed" | "other_speaker" | "owner_conversation" | "recorded" | "assistant_playback" | "overlap" | "silence";
  type Counts = { condition: Condition; attempts: number; measured: number; failed_or_missing: number; speaker_matches: number; minimum_similarity: number | null; maximum_similarity: number | null; clipped_samples: number };
  type ActivityLabel = "single_speech" | "overlap_speech" | "background" | "within_pause" | "end_silence";
  type Span = { start: number; end: number; label: ActivityLabel };
  type Diagnostic = { policy: string; processed_frames: number; speech_frames: number; overlap_frames: number; unfinished: boolean; final_observation: boolean; boundaries: { kind: "start" | "end"; frame: number }[] };
  type ActivityPhase = { admitted: number; measured: number; annotated: number; failed_or_missing: number; unannotated: number; unlabelled_frames: number; classes: { label: ActivityLabel; frames: number; mismatches: number; spans: number; decision_errors: number }[] };
  type ActivityCalibration = { policy: { revision: string; speech: number; overlap: number; quiet_frames: number } | null; latest: { request: string; frames: number; held_out: boolean; diagnostic: Diagnostic | null } | null; calibration: ActivityPhase; held_out: ActivityPhase };
  type GateCount = { admitted: number; measured: number; accepted: number; missing: number };
  type Calibration = { session: string; phase: "calibration" | "held_out"; threshold: number | null; calibration_attempts: number; calibration: Counts[]; held_out: Counts[]; seconds_remaining: number; activity: ActivityCalibration; gate: { counts: GateCount[]; pending: boolean; reviewable: boolean } | null; qualified: boolean; admission: "development" | "release_qualified" | null };
  type Activity = { model_revision: string; samples: number; frames: [number, number, number, number][] };
  type Result = { request: string; candidate: string; revision: string; transcript: string; similarity: number | null; held_out_similarities: number[]; seconds: number; clipped_samples: number; total_samples: number; calibration: Calibration | null; activity: Activity | null; assistant_output_overlap: boolean };
  type Progress = { request: string; phase: "checking" | "recording" | "processing"; seconds: number };
  let request = $state<string | null>(null);
  let phase = $state<Progress["phase"]>("checking");
  let seconds = $state(0);
  let result = $state<Result | null>(null);
  let error = $state("");
  let recovery = $state("");
  let ready = $state(false);
  let calibrationSession = $state<string | null>(null);
  let calibration = $state<Calibration | null>(null);
  let condition = $state<Condition>("owner_directed");
  let reviewing = $state(false);
  let admissionConsent = $state(false);
  let development = $state(true);
  let developmentStart = $state(0);
  let developmentEnd = $state(0);
  const admitted = $derived(!!calibration?.admission || !!calibration?.qualified);
  const developmentFrames = $derived(calibration?.activity.latest?.request === result?.request ? calibration?.activity.latest?.frames ?? 0 : 0);
  const developmentIntervalValid = $derived(Number.isFinite(developmentStart) && Number.isFinite(developmentEnd) && Math.round(developmentStart / 0.08) > 0 && Math.round(developmentEnd / 0.08) > Math.round(developmentStart / 0.08) && Math.round(developmentEnd / 0.08) < developmentFrames);
  let activity = $state(false);
  let activityStreaming = $state(false);
  let liveActivity = $state<Activity | null>(null);
  let activityChart = $derived(result?.activity ?? (request ? liveActivity : null));
  let liveDiagnostic = $state<Diagnostic | null>(null);
  let diagnostic = $derived(result?.calibration?.activity.latest?.diagnostic ?? (request ? liveDiagnostic : null));
  let spans = $state<Span[]>([]);
  let spanStart = $state(0);
  let spanEnd = $state(0.8);
  let spanLabel = $state<ActivityLabel>("single_speech");
  const activityLabels: Record<ActivityLabel, string> = { single_speech: "One speaker", overlap_speech: "Overlapping speakers", background: "Quiet background", within_pause: "Pause inside the utterance", end_silence: "Silence after the utterance ended" };
  function addSpan() {
    const latest = calibration?.activity.latest;
    if (!latest || latest.request !== result?.request || request || reviewing) return;
    const start = Math.round(spanStart / 0.08), end = Math.round(spanEnd / 0.08);
    if (!Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end <= start || end > latest.frames || spans.length >= 32 || spans.some(v => start < v.end && end > v.start)) { error = "Use a nonoverlapping interval inside this recording (at most 32)."; return; }
    spans = [...spans, { start, end, label: spanLabel }].sort((a, b) => a.start - b.start);
    error = "";
  }
  async function annotateActivity() {
    const session = calibrationSession, latest = calibration?.activity.latest;
    if (!session || !latest || latest.request !== result?.request || !spans.length || request || reviewing || blocked) return;
    reviewing = true; onbusy(true); error = "";
    try {
      const value = await command<Calibration>("annotate_voice_activity", { session, request: latest.request, spans });
      if (!disposed && calibrationSession === session && value.session === session) { calibration = value; spans = []; }
    } catch (e) { if (!disposed && calibrationSession === session) error = String(e); }
    finally { reviewing = false; onbusy(false); }
  }
  function activityPath(value: Activity, rank: number) {
    return value.frames.map((scores, index) => `${index ? "L" : "M"}${(index * 400 / Math.max(1, value.samples / 1280 - 1)).toFixed(2)},${(100 - [...scores].sort((a, b) => b - a)[rank] * 100).toFixed(2)}`).join(" ");
  }
  let measurementContext = "";
  const labels: Record<Condition, string> = {
    owner_directed_without_name: "Owner request without saying Avesra", speaker_switch: "Speaker switch to another person", ambiguous_approval: "Ambiguous short approval",
    owner_directed: "Owner addressing Avesra", other_speaker: "Another live speaker",
    owner_conversation: "Owner talking to someone else", recorded: "Recorded speech",
    assistant_playback: "Actual Avesra playback", overlap: "Overlapping speakers", silence: "Silence / background",
  };
  let disposed = false;
  $effect(() => {
    const next = `${id}:${revision}:${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}:${runtime?.settings.microphone}:${runtime?.settings.explicit_mute}:${runtime?.settings.deafened}:${runtime?.settings.paused}`;
    if (measurementContext && next !== measurementContext) {
      const old = calibrationSession;
      calibrationSession = null; calibration = null;
      if (old) void command("discard_voice_calibration", { session: old }).catch(() => {});
      void cancel().catch(() => {});
    }
    measurementContext = next;
  });
  async function discardCalibration() {
    const old = calibrationSession;
    if (!old || request || reviewing) return;
    if (admitted) await command("revoke_voice_admission");
    else await command("discard_voice_calibration", { session: old });
    if (calibrationSession === old) { calibrationSession = null; calibration = null; development = true; admissionConsent = false; }
  }
  async function freezeCalibration(activityPolicy: boolean) {
    const session = calibrationSession;
    if (!session || request || reviewing || blocked) return;
    reviewing = true; onbusy(true); error = "";
    try {
      const value = await command<Calibration>("freeze_voice_calibration", { session, activity: activityPolicy });
      if (!disposed && calibrationSession === session && value.session === session) calibration = value;
    } catch (e) { if (!disposed && calibrationSession === session) error = String(e); }
    finally { reviewing = false; onbusy(false); }
  }
  const gateLabels = ["Owner directed", "Owner directed without name", "Other speaker", "Recorded speech", "Assistant playback", "Overlapping speech", "Owner conversation", "Speaker switch", "Ambiguous approval"];
  function startQualification() {
    if (request || reviewing || blocked || !ready) return;
    development = false; activity = true; activityStreaming = true;
    calibrationSession = crypto.randomUUID(); calibration = null;
    condition = "owner_directed"; admissionConsent = false; error = "";
  }
  async function startDevelopment() {
    if (request || reviewing || blocked || !ready || calibrationSession) return;
    development = true; activity = true; activityStreaming = true;
    calibrationSession = crypto.randomUUID(); calibration = null;
    condition = "owner_directed"; admissionConsent = false; error = "";
    await record();
  }
  async function reviewDevelopment() {
    const session = calibrationSession, latest = calibration?.activity.latest;
    if (!development || !session || !latest || latest.request !== result?.request || !developmentIntervalValid || !admissionConsent || request || reviewing || blocked || admitted) return;
    reviewing = true; onbusy(true); error = "";
    try {
      const value = await command<Calibration>("review_development_voice", { session, request: latest.request, start: Math.round(developmentStart / 0.08), end: Math.round(developmentEnd / 0.08), consent: true });
      if (!disposed && calibrationSession === session && value.session === session) calibration = value;
    } catch (e) { if (!disposed && calibrationSession === session) error = String(e); }
    finally { reviewing = false; onbusy(false); }
  }
  const gateProgress = $derived.by(() => {
    const counts = calibration?.gate?.counts;
    if (!counts) return null;
    const sum = (indices: number[], key: keyof GateCount) => indices.reduce((total, index) => total + (counts[index]?.[key] ?? 0), 0);
    const admitted = sum([0, 1], "admitted");
    return { positive: sum([0, 1], "measured"), admitted, accepted: sum([0, 1], "accepted"), negative: sum([2, 3, 4, 5], "measured"), conversation: sum([6], "measured"), missing: counts.reduce((total, value) => total + value.missing, 0) };
  });
  async function revokeAdmission() {
    if (request || reviewing || blocked) return;
    reviewing = true; onbusy(true); error = "";
    try { await command("revoke_voice_admission"); calibrationSession = null; calibration = null; development = true; admissionConsent = false; }
    catch (e) { error = String(e); }
    finally { reviewing = false; onbusy(false); }
  }
  async function revalidateAdmission() {
    if (request || reviewing || blocked) return;
    reviewing = true; onbusy(true); error = ""; recovery = "";
    try {
      await command("revalidate_voice_admission");
      calibrationSession = null; calibration = null;
      recovery = "Saved permission passed current revalidation. Automatic listening uses your existing controls.";
    } catch (e) { error = String(e); }
    finally { reviewing = false; onbusy(false); }
  }
  async function reviewAdmission(activate: boolean) {
    const session = calibrationSession;
    if (!session || request || reviewing || blocked || (activate && !admissionConsent)) return;
    reviewing = true; onbusy(true); error = "";
    try {
      const value = await command<Calibration>("review_voice_admission", { session, activate });
      if (!disposed && calibrationSession === session && value.session === session) calibration = value;
    } catch (e) { if (!disposed && calibrationSession === session) error = String(e); }
    finally { reviewing = false; onbusy(false); }
  }
  async function cancel() {
    const active = request;
    if (active) await command("cancel_voice_check", { request: active });
  }
  async function record() {
    if (request || reviewing || blocked || !ready || admitted) return;
    const active = crypto.randomUUID();
    const session = calibrationSession;
    request = active; phase = "checking"; seconds = 0; result = null; liveActivity = null; liveDiagnostic = null; spans = []; developmentStart = 0; developmentEnd = 0; admissionConsent = false; error = ""; onbusy(true);
    try {
      const value = await command<Result>("check_saved_voice", { request: active, id, revision, activity, activityStreaming: activity && activityStreaming, calibration: session ? { session, condition } : null });
      if (!disposed && request === active && value.request === active && value.candidate === id && value.revision === revision) {
        result = value;
        if (session && calibrationSession === session && value.calibration?.session === session) calibration = value.calibration;
      }
    } catch (e) {
      if (!disposed && request === active) error = String(e);
      if (session && !disposed && calibrationSession === session) {
        try {
          const summary = await command<Calibration | null>("voice_calibration_status", { session });
          if (!disposed && calibrationSession === session) calibration = summary;
        } catch { /* The original recording error remains visible. */ }
      }
    }
    finally { if (request === active) { request = null; onbusy(false); } }
  }
  onMount(() => {
    let unlisten: (() => void) | undefined;
    let stopActivity: (() => void) | undefined;
    if (native) void listen<{ request: string; observation: Activity & { frame_offset: number; final: boolean }; diagnostic?: Diagnostic | null }>("voice-check-activity", event => {
      const value = event.payload;
      if (!disposed && value.request === request && value.observation.frame_offset === (liveActivity?.frames.length ?? 0)) {
        liveActivity = { model_revision: value.observation.model_revision, samples: value.observation.samples, frames: [...(liveActivity?.frames ?? []), ...value.observation.frames] };
        liveDiagnostic = value.diagnostic ?? null;
      }
    }).then(stop => { if (disposed) stop(); else { stopActivity = stop; ready = !!unlisten; } }).catch(e => { error = String(e); });
    if (native) void listen<Progress>("voice-check-progress", event => {
      if (event.payload.request === request && !disposed) { phase = event.payload.phase; seconds = event.payload.seconds; }
    }).then(stop => { if (disposed) stop(); else { unlisten = stop; ready = !!stopActivity; } }).catch(e => { error = String(e); });
    return () => { disposed = true; unlisten?.(); stopActivity?.(); void cancel().catch(() => {}); if (calibrationSession && !admitted) void command("discard_voice_calibration", { session: calibrationSession }).catch(() => {}); };
  });
</script>

<div class="flex flex-col gap-2.5 border-t border-white/10 pt-3">
  <span class="text-[12.5px] font-medium">Try automatic voice interaction</span>
  {#if development && !runtime?.voice_ready}
    <p class="av-hint">Keep your six saved phrases. Record one eight-second check, mark when you spoke, then allow development listening. The full release-validation corpus is optional below.</p>
    <p class="text-[12.5px] text-zinc-100">Start with a brief quiet pause, say “Avesra, tell me the time,” then stay quiet until recording ends.</p>
    {#if !calibrationSession}<button class="av-btn av-btn-primary av-btn-sm self-start" disabled={!!request || reviewing || blocked || !ready || !runtime?.connected || runtime.locked} onclick={startDevelopment}>Record my eight-second check</button>{/if}
    {#if calibrationSession && !admitted}
      {#if result && developmentFrames}
        <p class="av-hint">Mark the interval when you actually spoke. Leave the quiet before and after outside this interval; don't use the model scores as your label.</p>
        <div class="flex flex-wrap gap-2">
          <label class="av-hint flex flex-col gap-1">Speech starts (seconds)<input class="av-input w-28" type="number" min="0.08" max="7.92" step="0.08" bind:value={developmentStart} disabled={!!request || reviewing} /></label>
          <label class="av-hint flex flex-col gap-1">Speech ends (seconds)<input class="av-input w-28" type="number" min="0.08" max="7.92" step="0.08" bind:value={developmentEnd} disabled={!!request || reviewing} /></label>
        </div>
        <label class="av-hint flex items-start gap-2"><input type="checkbox" bind:checked={admissionConsent} disabled={!!request || reviewing} />I spoke alone during the marked interval, with quiet before and after. Allow automatic listening for my own testing. Speaker identification, replay rejection and overlap handling are not release-validated; I can mute, pause or revoke this permission.</label>
        <button class="av-btn av-btn-primary av-btn-sm self-start" disabled={!!request || reviewing || blocked || !developmentIntervalValid || !admissionConsent} onclick={reviewDevelopment}>{reviewing ? "Checking permission…" : "Allow development listening"}</button>
      {/if}
      <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={!!request || reviewing || blocked} onclick={() => discardCalibration().catch(e => error = String(e))}>Discard this check</button>
    {/if}
  {/if}
  {#if calibration?.admission === "development"}<p class="av-hint" role="status">Development listening is enabled. This is permission for your testing, not a release-quality validation pass.</p>{/if}
  {#if !runtime?.voice_ready}<button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!!request || reviewing || blocked || !runtime?.connected || !!calibrationSession} onclick={revalidateAdmission}>Revalidate saved voice permission</button><p class="av-hint">For existing listening permission after a service or device error. This checks saved evidence; it cannot replace the first live check or undo revocation.</p>{/if}
  {#if recovery}<p class="av-hint" role="status">{recovery}</p>{/if}
  <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!!request || reviewing || blocked} onclick={revokeAdmission}>Revoke saved voice permission</button>
  <p class="av-hint">Use this to remove an earlier listening permission, including one that could not be restored, before enabling a replacement.</p>
  {#if !development}<p class="av-hint">{calibrationSession ? `Present the selected condition: ${labels[condition]}. Each press records eight seconds; use new speech for held-out measurements.` : "Press Record, then read the sentence below. Recording stops automatically after eight seconds."}</p>{/if}
  {#if calibrationSession && condition === "assistant_playback"}<p class="av-hint">This explicit check records and plays “Avesra, tell me the time.” using the selected active voice. Stay quiet before and after playback. Native measured quiet starts the sound; playback that does not finish within the recording remains a missing trial. No action is executed.</p>{/if}
  {#if !development && !calibrationSession}<p class="text-[12.5px] text-zinc-100">“Avesra, tell me what I have planned for today and help me choose what to work on next.”</p>{/if}
  {#if !development}
  <label class="av-hint flex items-center gap-2"><input type="checkbox" bind:checked={activity} disabled={!!request || reviewing || !!calibrationSession} />Include speech activity measurements</label>
  {#if activity}<label class="av-hint flex items-center gap-2"><input type="checkbox" bind:checked={activityStreaming} disabled={!!request || reviewing || !!calibrationSession} />Show live activity while recording (requires streaming service)</label>{/if}
  <p class="av-hint">Requires the optional activity service on Spark. Reports raw speech and possible overlap scores; it does not decide who is speaking or enable listening. Choose before starting a measurement session.</p>
  {/if}
  <details class="border border-white/10 p-3">
    <summary class="av-hint cursor-pointer">Advanced: full release validation</summary>
    <div class="mt-2 flex flex-col gap-2">
      <p class="av-hint">Keep your six saved enrollment phrases. This separate qualification measures when automatic listening may accept a request. It needs another consenting speaker and the paired speech, live activity and reasoning services. Measurements stay in memory while this page remains open, up to eight hours; raw recordings and transcripts are not saved.</p>
      <p class="av-hint">This optional release-validation corpus takes at least about 67 minutes of recording, plus processing and setup. It is not needed for development listening. Closing this page, changing its bound setup or restarting the app discards unfinished measurements. Completed reviewed qualification is saved separately and revalidated on restart.</p>
      <ol class="av-hint list-decimal space-y-1 pl-5">
        <li>Record varied owner speech and another live speaker, including representative short requests.</li>
        <li>Label actual single speech, overlapping speech, quiet, a pause inside speech and silence after speech. Save the labels before the next recording.</li>
        <li>Freeze both speaker threshold and activity policy, then record one more owner request under the frozen policy to measure signal sufficiency.</li>
        <li>Freeze the whole-gate operating point. Collect the required new labelled requests and negative cases shown below; earlier calibration recordings do not count.</li>
        <li>When native review passes, give listening consent and enable automatic voice interaction.</li>
      </ol>
      {#if !calibrationSession}
        <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!!request || reviewing || blocked || !ready} onclick={startQualification}>Start voice qualification</button>
        <p class="av-hint">Starting selects live activity measurement for this session; it does not open the microphone until you press Record.</p>
      {:else if !development}
        <label class="av-hint flex flex-col gap-1">Condition to record
          <select class="av-select" bind:value={condition} disabled={!!request || reviewing}>
            {#each Object.entries(labels) as [value, label]}
              <option value={value} disabled={(value === "assistant_playback" && !calibration?.gate) || (value === "silence" && !!calibration?.gate) || (!activity && calibration?.phase !== "held_out" && value !== "owner_directed" && value !== "other_speaker")}>{label}</option>
            {/each}
          </select>
        </label>
        <p class="av-hint">{calibration?.phase === "held_out" ? "Threshold frozen. New recordings measure held-out speaker matches without retuning." : "Record varied owner speech and a consenting other live speaker first. Freeze derives a provisional midpoint only if the observed scores separate. One pair does not establish reliability."}</p>
        <div class="flex flex-wrap gap-2">
          <button class="av-btn av-btn-secondary av-btn-sm" disabled={!!request || reviewing || blocked || !calibration || calibration.phase === "held_out"} onclick={() => freezeCalibration(false)}>{reviewing ? "Reviewing…" : "Freeze speaker threshold"}</button>
          <button class="av-btn av-btn-secondary av-btn-sm" disabled={!!request || reviewing} onclick={() => discardCalibration().catch(e => error = String(e))}>Discard measurements</button>
        </div>
        {#if calibration}
          <p class="av-hint">{calibration.calibration_attempts} admitted calibration recordings · threshold {calibration.threshold === null ? "not frozen" : calibration.threshold.toFixed(4)} · {Math.ceil(calibration.seconds_remaining / 60)} minutes remained at the last result.</p>
          {#each (calibration.phase === "held_out" ? calibration.held_out : calibration.calibration).filter(row => row.attempts) as row}
            <p class="av-hint">{labels[row.condition]}: {row.measured} measured / {row.attempts} admitted · {row.failed_or_missing} failed or missing{calibration.phase === "held_out" ? ` · ${row.speaker_matches} speaker-threshold matches` : ""} · scores {row.minimum_similarity?.toFixed(3) ?? "unavailable"}–{row.maximum_similarity?.toFixed(3) ?? "unavailable"}.</p>
          {/each}
        {/if}
        {#if calibration && activityStreaming}
          <div class="flex flex-col gap-2 border-t border-white/10 pt-2">
            <span class="av-hint">Whole-gate qualification</span>
            <p class="av-hint">This uses the actual native utterance owner, speaker check, directedness model and output-absence evidence. Start with measured quiet, present exactly one labelled utterance, then leave ending silence. A cutoff or multiple utterances cannot count as a complete observation. No recordings before this Freeze count toward the whole-gate corpus.</p>
            {#if !calibration.gate && !calibration.qualified}
              <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!!request || reviewing || blocked || calibration.threshold === null || !calibration.activity.policy} onclick={() => reviewAdmission(false)}>Freeze whole-gate operating point</button>
            {/if}
            {#if calibration.gate}
              <p class="av-hint">Requires 100 measured owner requests (at least 95% of admitted requests accepted, including a request without saying Avesra), 200 combined other/recorded/playback/overlap attempts with every class represented, 200 owner conversations, and speaker-switch and ambiguous-approval observations. Negative attempts must never be accepted. Native output absence does not establish external replay resistance.</p>
              {#if gateProgress}<p class="av-hint" role="status">Owner requests: {gateProgress.positive}/100 measured, {gateProgress.accepted}/{gateProgress.admitted} admitted requests accepted. Combined negatives: {gateProgress.negative}/200. Owner conversations: {gateProgress.conversation}/200. Missing observations: {gateProgress.missing}.</p>{/if}
              {#each calibration.gate.counts as count, index}
                <p class="av-hint">{gateLabels[index]}: {count.measured}/{count.admitted} measured · {count.accepted} native accepts · {count.missing} missing.</p>
              {/each}
              <label class="av-hint flex items-center gap-2"><input type="checkbox" bind:checked={admissionConsent} disabled={!!request || reviewing} />After native review passes, allow this qualified owner and microphone to start automatic listening.</label>
              <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!!request || reviewing || blocked || !admissionConsent || !calibration.gate.reviewable} onclick={() => reviewAdmission(true)}>Review evidence and enable listening</button>
            {/if}
            {#if calibration.qualified}<p class="av-hint">Native review passed for this exact context. Its protected evidence is revalidated on restart. Revoke automatic voice permission to remove it.</p>{/if}
          </div>
        {/if}
        {#if activity && calibration}
          <details class="border-t border-white/10 pt-2">
            <summary class="av-hint cursor-pointer">Activity frame calibration and tentative endpoints</summary>
            <div class="mt-2 flex flex-col gap-2">
              <p class="av-hint">Label what you actually observed in each interval; do not infer labels from the model scores alone. Labels are rounded to 80-ms frames. Unknown or unlabelled frames stay excluded. This is separate from the speaker comparison and cannot enable listening.</p>
              {#if calibration.activity.latest?.request === result?.request && calibration.activity.latest}
                <p class="av-hint">This recording belongs to {calibration.activity.latest.held_out ? "held-out measurement" : "calibration"}. Starting another recording replaces its retained frames; unannotated results remain counted.</p>
                <div class="flex flex-wrap items-end gap-2">
                  <label class="av-hint flex flex-col">Start (seconds)<input class="av-input w-24" type="number" min="0" max="8" step="0.08" bind:value={spanStart} disabled={!!request || reviewing} /></label>
                  <label class="av-hint flex flex-col">End (seconds)<input class="av-input w-24" type="number" min="0" max="8" step="0.08" bind:value={spanEnd} disabled={!!request || reviewing} /></label>
                  <label class="av-hint flex flex-col">Observed condition<select class="av-select" bind:value={spanLabel} disabled={!!request || reviewing}>{#each Object.entries(activityLabels) as [value, label]}<option value={value}>{label}</option>{/each}</select></label>
                  <button class="av-btn av-btn-secondary av-btn-sm" disabled={!!request || reviewing} onclick={addSpan}>Add interval</button>
                </div>
                {#each spans as span, index}
                  <div class="flex items-center gap-2"><span class="av-hint">{(span.start * 0.08).toFixed(2)}–{(span.end * 0.08).toFixed(2)} s · {activityLabels[span.label]}</span><button class="av-btn av-btn-secondary av-btn-sm" disabled={reviewing} onclick={() => spans = spans.filter((_, i) => i !== index)}>Remove</button></div>
                {/each}
                <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!spans.length || !!request || reviewing || blocked} onclick={annotateActivity}>Save labels once</button>
              {/if}
              <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={!!request || reviewing || blocked || !!calibration.activity.policy || !!calibration.activity.latest} onclick={() => freezeCalibration(true)}>Freeze provisional activity policy</button>
              <p class="av-hint">Freeze needs labelled single speech, overlapping speech and quiet, plus pauses shorter than ending silence. It derives measured cutoffs only when classes separate. One set of labels does not establish reliability; held-out recordings never retune the policy.</p>
              {#if calibration.activity.policy}<p class="av-hint">Frozen speech {calibration.activity.policy.speech.toFixed(4)} · overlap {calibration.activity.policy.overlap.toFixed(4)} · ending quiet {(calibration.activity.policy.quiet_frames * 0.08).toFixed(2)} s.</p>{/if}
              {#each ["calibration", "held_out"] as phase}
                {@const value = phase === "calibration" ? calibration.activity.calibration : calibration.activity.held_out}
                <p class="av-hint">{phase === "calibration" ? "Calibration" : "Held-out"}: {value.measured}/{value.admitted} measured recordings · {value.annotated} annotated · {value.failed_or_missing} failed or missing · {value.unannotated} unannotated · {value.unlabelled_frames} unlabelled frames.</p>
                {#each value.classes.filter(v => v.frames) as row}<p class="av-hint">{activityLabels[row.label]}: {row.frames} labelled frames{phase === "held_out" ? ` · ${row.mismatches} frame mismatches · ${row.decision_errors} quiet-span decision mismatches` : ""}.</p>{/each}
              {/each}
            </div>
          </details>
        {/if}
        <p class="av-hint">Readiness errors before admission appear below and are not counted. Cancel keeps an admitted recording as missing; Discard or closing this page removes unfinished measurements. Whole-gate checks use actual speech, speaker, directedness, overlap and native output evidence. Unknown evidence cannot count as a successful negative. A recorded owner may still match your voice; failed replay cases cannot be hidden or relabelled to enable listening.</p>
      {:else}<p class="av-hint">Finish or discard the development check above before starting a separate release-validation session.</p>{/if}
    </div>
  </details>
  <div class="flex items-center gap-2">
    {#if !development || (calibrationSession && !admitted)}<button class="av-btn av-btn-secondary av-btn-sm" disabled={!!request || reviewing || blocked || !ready || !runtime?.connected || runtime.locked || admitted} onclick={record}>{development ? "Record check again" : calibrationSession && condition === "assistant_playback" ? "Record and play rejection trial" : calibrationSession ? "Record labelled measurement" : "Record an eight-second check"}</button>{/if}
    {#if request}<button class="av-btn av-btn-secondary av-btn-sm" onclick={() => cancel().catch(e => error = String(e))}>Cancel</button>{/if}
  </div>
  {#if request}
    <div class="flex flex-col gap-1.5" role="status" aria-live="polite">
      <span class="av-hint">{phase === "checking" ? "Checking Spark speech services · microphone off…" : phase === "recording" ? `Speak now · ${seconds} / 8 seconds` : "Recording complete · Spark is transcribing and comparing your voice…"}</span>
      {#if phase === "recording"}<progress class="h-1.5 w-full accent-[#ac315b]" value={seconds} max={8} aria-label="Recording progress"></progress>{:else}<progress class="h-1.5 w-full accent-[#ac315b]" aria-label="Processing voice check"></progress>{/if}
    </div>
  {/if}
  {#if error}<p class="text-xs text-amber-300" role="alert">{error}</p>{/if}
  {#if result}
    <div class="flex flex-col gap-2 border border-white/10 bg-black/15 p-3" role="status">
      <span class="text-[12.5px] font-medium">What Spark heard</span>
      <p class="text-[12.5px] text-zinc-100">{result.transcript || "No speech was transcribed."}</p>
      {#if result.assistant_output_overlap}<p class="av-hint">Native speech output submissions overlapped this measured endpoint. This is rejection evidence only, not echo cancellation or external replay protection.</p>{/if}
      <p class="av-hint">{result.transcript && result.similarity !== null ? development ? "Your microphone reached Spark. Review the speech interval and give permission above to try automatic listening." : "Your microphone reached Spark and speech was transcribed. Review the advanced measurement results above." : "Speech could not be fully checked. Check your selected microphone in Audio & Voice, then try speaking again."}</p>
      <details><summary class="av-hint cursor-pointer">Technical comparison details</summary><p class="av-hint mt-2">Voice similarity: {result.similarity === null ? "not enough speech" : result.similarity.toFixed(3)} · saved held-out phrases: {result.held_out_similarities.map(v => v.toFixed(3)).join(", ")}</p><p class="av-hint">{result.seconds} seconds captured · {result.clipped_samples} clipped samples of {result.total_samples.toLocaleString()}. Similarity is a comparison score, not a confidence percentage.</p></details>

    </div>
  {/if}
      {#if activityChart}
        <div class="flex flex-col gap-1">
          <span class="av-hint">Speech activity · raw scores from 0 to 1</span>
          <svg viewBox="0 0 400 100" class="h-24 w-full border border-white/10" role="img" aria-label="Strongest and second-strongest speaker activity scores over this recording">
            {#if calibration?.activity.latest?.request === result?.request && !request}
              {#each spans as span}<rect x={span.start * 4} y="0" width={(span.end - span.start) * 4} height="100" fill="#ac315b" opacity="0.2" />{/each}
            {/if}
            <path d={activityPath(activityChart, 0)} fill="none" stroke="#e7a2b7" stroke-width="1.5" />
            <path d={activityPath(activityChart, 1)} fill="none" stroke="#9ebfe7" stroke-width="1.5" />
          </svg>
          <p class="av-hint">0–{(activityChart.samples / 16000).toFixed(1)} seconds · pink: strongest activity · blue: second strongest. Each point covers 80 ms. The plot shows raw scores; any frozen provisional policy is reported separately below. These scores do not identify the owner and are not saved in measurement summaries.</p>
        </div>
      {/if}
  {#if diagnostic}
    <div class="flex flex-col gap-1 border border-white/10 p-3">
      <span class="av-hint">Provisional activity diagnostics · {diagnostic.processed_frames} frames observed</span>
      <p class="av-hint">{diagnostic.speech_frames} speech-threshold frames · {diagnostic.overlap_frames} overlap-threshold frames. These measurements do not accept speech or control recording.</p>
      {#each diagnostic.boundaries as boundary}<p class="av-hint">Tentative {boundary.kind === "start" ? "start" : "end decision"} at {(boundary.frame * 0.08).toFixed(2)} seconds.</p>{/each}
      {#if diagnostic.final_observation && diagnostic.unfinished}<p class="av-hint">Speech span unfinished at the recording cutoff. No endpoint was manufactured.</p>{/if}
    </div>
  {/if}
  <p class="av-hint">The check sends this phrase to your paired Spark and keeps no recording. It does not execute the spoken request. Automatic listening starts only after the separate permission step.</p>
</div>
