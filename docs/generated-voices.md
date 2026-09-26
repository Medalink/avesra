# Generated voice candidates and explicit preview

## Editable voice test

Audio & Voice provides an explicit **Test selected voice** action with an editable
text area, initially containing the default sentence: `Hello I am Avesra, A Very
Effective Smart Reasoning Assistant. I am designed to help you manage your thoughts
and ideas.` **Use default** restores it. Editing, resetting, mounting or refreshing
never plays audio; **Play text** speaks a snapshot of the entered text using the
currently selected and active voice, with the existing native effects/atmosphere.
The draft lives only in the mounted panel; it is not saved as voice metadata,
history or a generated candidate. Existing candidate reference previews and repeat
preview retain their saved-take behavior, including after Generate.

The explicit native `preview_voice` command accepts optional `text`, carried as
optional `test_text` in `PreviewRequest` on `/voice-preview`. Missing text means
reference playback; supplied text must be nonblank, at most 512 UTF-8 bytes, and
contain no control characters except newline/tab. Empty or invalid supplied text
is rejected, never replaced by the default or reference. Native and server validate
independently. The UI shows the byte limit and disables playback for invalid text,
unavailable selected/active voice, current busy work or unavailable output.
Every response echoes the complete original request, including exact test text.
Preview wire version 2 is required on both ends; no retry or reference fallback
to an older protocol is allowed.

Test text and startup greeting are mutually exclusive. `/startup-greeting` remains
fixed-shape and rejects test text; normal accepted replies retain their separate
authority. Test synthesis verifies exact selected and active voice identities,
never temporarily selects a candidate, and uses the same private synthesis path
and shared voice-operation exclusion as the greeting. It retains the existing
30-second private synthesis deadline, 30-second PCM cap and complete-terminal
requirement. Test text and greetings now emit `StreamingReady` only after the
first validated private codec chunk, without waiting for the complete waveform.
Its maximum sample count is a ceiling, never a promised duration. A bounded
64-codec-chunk queue connects the original private job to paced 20-ms public
frames; queue pressure never renews the synthesis deadline. The original private
job remains owned through its terminal or retained cancellation cleanup even if
the public consumer disappears. The visible Settings panel, paired
session, request identity, original playback epoch, cancellation, local output
ownership and Ready/Play/Audio/End/Submitted checks remain mandatory. No microphone,
owner qualification, action admission, voice regeneration or new playback renderer
is introduced. Raw test text is not logged or persisted.

The renderer retains the final 20-ms frame until a validated Complete terminal
establishes the actual total; only exact sequence/offset/terminal correlation can
mark final submission or drain. Failed/truncated synthesis stops the stream and
does not finalize it; an already-played prefix cannot be retracted. Public pacing
has a fixed origin, at most 100-ms lateness and at least 10-ms packet spacing.
Native ingress checks its fixed 32-second playback deadline, cumulative sample
ceiling and bounded queue. Cancellation or lost permissions stop both sides.
Reference previews still use exact-length `Ready` and their saved PCM. A
`StreamingReady` on that route or an exact-length `Ready` on synthesis rejects.
Performance remote-ready measures connection, admission and first validated
codec availability; first submitted speech is the separate native device-submission
observation. Neither boundary proves acoustic playback or completed speech.

Entry points: VoiceDesigner Play text and native `preview_voice` are affected;
`PreviewRequest`, server `/voice-preview` and echoed native response validation
cover the wire. Existing candidate Generate/autoplay/Preview/Repeat omit text and
remain reference-only. Native `startup_greeting::run` and `/startup-greeting` remain
fixed greeting only; `speech::speak` and `/normal-speech` are unaffected. Validation
uses Windows/frontend and ARM server static/release checks under the owner's
no-automated-tests override; live playback is reported separately.

## Startup greeting

An explicit native `--capture-output` diagnostic process suppresses this automatic
greeting before preparation/synthesis so the first recording belongs to the
operator's selected preview. Ordinary launches retain the behavior below. See
[background evidence](background-evidence.md) and [silent output recording](playback.md#explicit-silent-output-recording).

Once per desktop process launch, after the saved Spark connection is acknowledged,
Avesra may synthesize `Hello.` or `Hello, <remembered owner name>.` using the exact
selected and active voice. This is an explicitly requested product greeting,
independent of enrollment, microphone mute, normal replies and Settings visibility.
No selected/active voice, unavailable output, deafen, pause or lock means silence.
Wait at most 60 seconds for the initial connection; an output control change or
disconnect cancels the attempt. Do not retry on reconnect, unlock, page changes,
voice selection, a second-instance launch, or synthesis failure.

The authenticated `/startup-greeting` route accepts only the fixed greeting shape,
with an optional bounded plain name (at most 80 Unicode characters / 320 bytes).
It never accepts arbitrary reply text or invokes the planner. The name must come
from owner-scoped local memory; absent, invalid or unavailable memory falls back
to `Hello.`. Never infer a name from the Windows account or a voice reference.
The server constructs the sentence, verifies the voice is selected and active,
and holds the same voice-operation exclusion as preview. Synthesis is bounded to
30 seconds and 30 seconds of PCM, with the incremental `StreamingReady` behavior
above. Truncation cannot finalize the already-started greeting.
StreamingReady/Play/Audio/End/Submitted transport, output gain, final-frame
holdback, draining and cancellation rules apply. `/voice-preview` rejects greeting
requests and `/startup-greeting` rejects reference-preview requests.

People > Your name provides the initial explicit owner-name memory producer.
The native `remember_owner_name` command resolves the authenticated local owner
and persists a bounded name with that actor UUID in local SQLite settings. It
does not change ownership, enroll a voice or grant actions. Empty input forgets
the name. General `save_settings` cannot replace this memory; old settings default
to no name. Startup revalidates the protected owner identity before using it.
Conversational extraction of names remains part of the future memory pipeline.
The startup `/voices` status read carries an optional playback epoch and uses
output permission, so microphone-only changes cannot cancel greeting preparation.
This alternative binding is legal only for Status; mutations keep their existing
capture/session binding. Replies echo the optional playback epoch for correlation.
The generated-voice TTS deployment enables `tts_streaming` for synthesis of the
fixed greeting. Enabling that private capability does not qualify normal speech,
owner recognition or microphone capture; the controller still owns admission.

Entry points: native `startup_greeting::run` is the sole automatic producer;
`preview::greet` owns native output; `/startup-greeting` owns fixed synthesis.
Explicit `preview_voice` and `/voice-preview` retain reference behavior when text
is absent, and support the separately explicit editable test described above.
Normal `speech::speak` and `/normal-speech` retain accepted-reply requirements.
Validation follows Plan 001's no-automated-tests override: compile both Windows
desktop and Unix server, format checks, and report live playback separately.

2026-09-25 voice-atmosphere audition: the owner explicitly authorized playing Digital and Human and requires approval of both before completion. The owner's latest exact reference text is "Hello I am Avesra, A Very Effective Smart Reasoning Assistant. I am designed to help you manage your thoughts and ideas." It supersedes the earlier Hello Eric greeting for newly generated previews. Existing candidates retain their original reference text. Explicit Repeat preview replays the same saved take without regeneration; Stop repeating prevents the next take. Both sound presets render the same candidate through the native mixer; changing a sound preset does not select a different generated voice. Master volume and the atmosphere controls remain available during an admitted preview. This authorization is for the requested auditions and does not enable microphone capture or general voice readiness.

The owner's2026-09-25 Generate attempt authorizes fixing and invoking that explicit generated-voice operation. Live service deployment and generated-candidate evidence are recorded in `operations.md`. Historical no-generation statements below describe earlier checkpoints; they do not override this later request. No playback or microphone capture was performed by the agent.

## Connected but unavailable recovery

Voice designer navigation must preserve the local description draft, designer open/closed state and reviewed candidate reference across section changes and app restarts. Store only bounded UI metadata in this PC's WebView local storage; never store audio, credentials, trusted availability or permission state there. Restoration resolves the remembered candidate against a fresh authenticated Spark status response. It does not select a voice, generate, or play anything. Deleted or foreign-server candidate references cannot enable actions. If no draft exists, recover the most recent available saved candidate (or selected voice) and its description from Spark. An intentionally emptied draft remains empty. Storage failures are visible while in-memory editing continues. The candidate row exposes an explicit Play preview action alongside Use this voice.

The owner explicitly requests Generate to generate and preview in one action. After successful generation, the same still-visible/current panel may invoke the existing native preview lease for that exact returned candidate. Show live Generating and Playing preview indicators and an accessible status announcement. Do not auto-select the voice. Navigation, disconnect or context withdrawal prevents delayed autoplay. If no output device is selected, sound is deafened/paused, or playback fails, retain the saved candidate and report the specific reason. Manual Preview remains available for replay; restoring a draft or refreshing status must never autoplay. This later requirement supersedes the historical separate-button-only generation flow below.

Connection alone must not enable Generate. The native voice-status request must first succeed; failed status clears cached availability and disables generation until an explicit refresh succeeds. A successful status proves access to the configured voice store, not overall assistant readiness. The native client distinguishes authentication/context, busy, unavailable-service and timeout HTTP failures with fixed actionable messages, without rendering arbitrary server bodies. Mutations are never automatically retried. The UI adds reconciliation guidance only once and retains the user's description. Entry points: VoiceDesigner refresh/generate (availability and recovery), native connection::voice_operation (HTTP categorization); paired server /voices and private voice operations retain their existing admission, deadlines and storage contracts. Validate with static/build checks and real service metadata under the owner's no-tests override.

Voice design produces a new generated reference asset from an explicit description and reference text. It never accepts uploaded impersonation references or request-supplied filesystem paths. A configured private Spark voice store assigns candidate/revision UUIDs, pins VoiceDesign/Base model revisions, records the exact generated WAV content digest, and preserves bounded metadata needed to recreate the Base clone prompt. A second digest binds canonical UTF8 JSON of the complete metadata (sorted keys, compact separators, excluding only the digest itself from identity), including reference text, description, models, geometry and candidate identity. Changed reference text therefore cannot reuse an audio-only prompt identity. Candidates remain separate from the selected voice. Generation, selection, preview and discard are explicit native Settings actions; none qualifies owner recognition or grants actions.

The store has a bounded candidate count and bytes per record. Publication writes/flushes a temporary file, atomically publishes immutable audio, then publishes its metadata. Interrupted or corrupt entries remain identifiable for exact deletion. Selection is an atomically replaced reference to an existing verified candidate/content digest; callers supply the observed selection-file revision for comparison, so stale UI cannot replace a newer selection. Deletion never silently removes the selected voice. A corrupt selection can be explicitly cleared by its observed file digest without interpreting it as a trusted candidate. Raw ambient audio is never persisted by this store; these files contain only explicit generated reference assets.

The TTS lane loads a selected reference through its configured private store. Selection prepares a new clone prompt before committing the selected reference and switching the live prompt. Cancellation/restart recovers from the durable reference. Every synthesis request and emitted chunk/terminal must carry the exact selected candidate/revision/content digest; old voice output is rejected after a selection change. The store is not a network API and its same-UID service commands are not owner authority. The paired controller/native setup path must bind each effect to the current authenticated Settings session and explicit intent.

Preview is a separate bounded native setup playback lease, available before normal owner/voice readiness only after an explicit action in the visible native Settings window and current unlocked paired-session validation. It grants no microphone permission, cannot set enrolled/voice_ready, and is revoked by deafen/pause/lock/disconnect/output-device or voice-selection changes. Candidate identity, session/generation and playback epoch survive the TTS/private/public/native boundaries. Slow generation and native playback have separate fixed deadlines; late chunks never extend either. Cancellation stops local output first and signals exact remote work.

UI follows Settings.dc.html Audio/Voice: current-preset card with Preview and Design toggle, inline description/count, Discard/Use this voice/Generate actions, Pace and Voice volume. Only actual candidate/selection/status data supplies metadata; no prototype name/date, animated preview bars or sample sentence is reported as runtime output. The current streaming driver cannot adjust pace, so that control remains explicitly unavailable for streaming until a supported implementation exists. No voice generation, selection, playback or visual inspection is authorized during the owner's gaming restriction.

Initial source checkpoint implements private `create_voice`, `voice_status`, `select_voice`, `clear_voice` and `discard_voice` child operations through existing bounded supervisor admission. `voice_store` is an explicit operator-configured absolute private directory and is incompatible with the legacy fixed `voice_preset` option. TTS startup attempts to restore the selected reference; corrupt selection or failed prompt preparation leaves no active prompt while preserving loaded-model metadata/status/clear recovery. It never silently deletes the selected record. Generation/prompt creation and persisted candidate handling remain uninvoked. Status distinguishes missing/corrupt selected assets from the actually loaded prompt. The private storage baseline was followed by controller, native lease and UI integration below. No image/configuration/store directory was created or altered by this source change.

Streaming synthesis now requires the exact selected identity, including both content digests. Child, supervisor and typed Rust receiver validate that identity on request and every chunk/terminal; the legacy fixed-path preset cannot satisfy this streaming identity contract. The prompt store revalidates durable audio/metadata before use. No loaded/unqualified or selected state implies empirical voice quality, completed owner qualification or permission to play sound.

The paired `/voices` controller route now exposes bounded typed status/generate/select/clear/discard commands, with one admitted operation, current authenticated control-session/epoch binding, request replay rejection and cancellation on session changes/revocation. It uses separately configured TTS/VoiceDesign private deployments and never accepts paths. Metadata commands do not require microphone permission; a distinct output permission owns the preview transport. Native Settings context validation and UI integration are implemented below; no command was invoked. Private `preview_voice` returns the exact generated reference asset without changing durable/live selection, and the typed client strictly validates identity, rate and PCM bounds. This is a generated-reference preview, not proof of Base-model synthesized voice quality or completed playback.

## Generated-reference preview transport

`/voice-preview` is a bounded authenticated WSS output route, separate from normal conversation and microphone ingress. Its version1 start binds paired device/control-session, positive playback epoch, unique request UUID and exact candidate identity. It subscribes to output permission, so mic mute alone does not cancel it. A shared single voice-operation admission slot excludes concurrent selection/clear/generation through the paired controller while preview owns its asset; no temporary selection is made. Private TTS `preview_voice` verifies and returns the existing generated reference, with no new synthesis or owner acceptance.

After asset validation the server sends Ready with the exact sample count. Native must explicitly reply Play only after its current output lease/device can receive frames. Audio consists of ordered 480-sample24kHz mono frames (last may be shorter), each bound to request/session/playback/candidate with exact sequence and sample offset. Server pacing follows a20ms/frame timeline, permitting at most100ms (five frames) of catch-up before aborting; native retains its independent500ms queue-age and sequence bounds. Native must independently enforce receipt age and source sequence, preserve final-frame holdback until End confirms exact counts, and use its own fixed playback deadline. End means transport completion, not completed device output. Native Submitted means only final samples were submitted to the device; it never claims audible delivery.

Socket close/cancel, output epoch changes, deafen/pause, credential revocation and fixed time limits cancel the route and its private command. Asset load has31seconds, Play admission3seconds, transfer31seconds and final native receipt2seconds; total route lifetime is70seconds. All sends have500ms deadlines. At most30seconds of explicit generated reference PCM is retained; this is never ambient microphone material. The companion now implements explicit visible Settings context validation and a separate native playback lease for this route. The approved Audio & Voice designer now consumes native candidate/status and preview operations; neither endpoint nor playback has been invoked or deployed.

Generated assistant voice configuration does not require Windows Hello: it is an explicit ordinary Audio & Voice action. Windows Hello remains required for speaker identity and grant management. Native preview admission snapshots the local context before querying Settings visibility and rechecks it under the native lock; a close/context change cannot transfer a prior button press into a new session. Its ongoing output-only lease ignores microphone-only transitions.

The native preview command waits for the same acknowledged control-session/output epoch and exact preview-device-ready identity before Play, applies the saved assistant gain, preserves source age from its Play-time timeline, and holds the last frame for an exact End. Its guarded cancellation advances only output epoch, closes local permission first and signals remote session cancellation; dropped native futures retain the same synchronous local cleanup. Final submission is followed by the bounded native device drain described in playback.md. None of this enables normal voice/owner readiness.

The Audio & Voice panel registers a native UUID context on mount and unregisters that exact context on section exit; late cleanup cannot revoke a newer panel. Explicit operations require the current panel, visible Settings, unlocked paired session and a native serialized slot shared with preview. Closing/replacing that context invalidates capture-bound management and output-bound preview epochs before returning. Microphone mute alone leaves an admitted output lease valid. Native management uses a dedicated bounded35second network deadline and512KiB response limit, strict candidate/status shape validation and exact request/session/epoch reply correlation. A failed or timed-out mutation is never retried automatically; the UI refreshes status to reconcile possible durable selection changes.

The designer ports the approved p-3.5 current-preset card, inline description/counter and Discard/Use this voice/Generate row, and side-by-side gap-5 Pace/Voice volume controls. Saved candidates and corrupt-selection recovery use the same square controls. The actual generated reference text is supplied in bounded metadata and displayed when previewing; unavailable records expose exact deletion only. Pace retains the approved0.8 to1.5 presentation but is disabled because streaming adjustment is unsupported. Voice volume is native local gain captured at preview start and is disabled while an operation is active. Status refresh is explicit/on-connection only, never an error retry loop; possibly committed mutations require status reconciliation before reuse. Frontend operation ownership survives capture-only changes and preview's own output epoch transitions. No visual/live fidelity or voice-quality proof is inferred from static checks.

The designer preserves the prototype280-character presentation limit and av-textarea/av-select styles; transport contracts independently permit bounded Unicode scalar counts (512 reference,1024 description) for other native callers. Disconnect/lock invalidates frontend publication while the actual operation retains busy ownership until its future exits. Microphone/output epoch changes alone do not reset that UI ownership.
