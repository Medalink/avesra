# Explicit voice enrollment

## Short development setup (owner-directed activation policy)

The saved-permission revoke control remains available even when automatic
listening is inactive or saved permission could not be restored. It invokes the
same native revocation command with unchanged authority checks. This gives the
owner an explicit recovery path before replacing a saved report; a failed
restore never permits silently overwriting that report.

The owner explicitly revised Plan 001: the full independent corpus below is
release validation, not a prerequisite for personal development use. Default
setup reuses the six saved enrollment samples and records one real eight-second
owner request with live ASR, speaker comparison and streaming activity. The owner
marks the speech interval, confirms they spoke alone with quiet before and after,
and explicitly consents to development listening. Native retained evidence must
contain recognized speech, a live similarity at least the minimum saved held-out
similarity, complete correlated activity and unchanged no-output provenance.

Development derives provisional speech/overlap cutoffs from the observed speech
peak, quiet maximum and secondary-speaker maximum; quiet duration is the shorter
observed leading/trailing quiet interval. These are operating points, not measured
false-accept or overlap guarantees. A single measured activity frame is the
minimum speech quantum. Clipping allowance is the actual observed fraction.
Unseparated scores or an incomplete measured endpoint refuse setup. No release
counts are manufactured. Replay, other-speaker and overlap resistance remains
unvalidated; explicit warnings and a distinct Development admission/report retain
that fact across protected storage and restart. All ordinary actor scope grants,
protected-action approvals, current-model checks and fail-closed output evidence
still apply. The advanced corpus can produce ReleaseQualified admission later.

The private native command accepts only the retained request, interval and
consent. It never accepts scores, PCM, a policy, a pass flag or imported authority.
Settings loss, caller cancellation, deadline or changed identity withdraws both
publication and activation. Revocation and deliberate saved-permission revalidation
use the same native lifecycle for either admission kind.

## Explicit speaker calibration measurements

VoiceCheck also offers an opt-in, native-owned speaker calibration session. Starting
the session does not open the microphone. Each labelled recording still requires
an explicit press and uses the existing eight-second capture and analysis path.
Labels describe what the operator intends to present; they are never inferred
identity, acceptance or permission. Ordinary unlabelled checks remain unchanged.

The session binds its native-read candidate and owner revisions, reconciled Spark
registration, exact microphone, paired device/session, connection/action context
and pinned ASR/speaker revisions plus the optional activity adapter selection.
The separately configured [activity lane](voice-activity.md) can add transient
four-speaker activity scores to this same explicit phrase. Its option is fixed
for a measurement session; scores do not affect the speaker threshold or grants.
Mismatching or unavailable context rejects new
measurements. Cancel stops the current recording and keeps its missing measurement;
disposal, mode/context loss and explicit discard retire the exact session.
replacing a session requires discarding its predecessor. The session expires after
eight hours and holds at most 1,024 admitted recordings in memory. No transcripts,
PCM or embeddings enter it or any persisted report. Failures and missing speaker
measurements after native admission remain visible in totals. Service preflight,
session acknowledgment or registration failures before admission remain explicit
recording errors, and are not included in the admitted-recording totals.
Discarded session IDs cannot be reused while the process remains alive. Up to
128 retired IDs are retained without eviction; exceeding that metadata bound
stops new measurement sessions until restart. This also prevents a late preflight
from recreating a session discarded before its first recording was admitted.

Calibration accepts separately labelled owner and other-live-speaker recordings.
An explicit Freeze action derives the midpoint between the lowest measured owner
similarity and highest other-speaker similarity only when both classes exist and
are strictly separated. Both saved held-out comparisons must meet that threshold.
No default score or model confidence is used. This is a provisional **speaker-only**
operating point, with no claim of statistical sufficiency. It cannot activate a
profile. After freezing, further recordings are held-out measurements and cannot
retune the threshold. Owner-directed, owner-conversation, other-speaker, recorded,
assistant-playback, overlap and silence labels have separate counts, including
missing measurements. A recorded owner can match the speaker representation;
rejecting replay requires the independent echo/replay evidence, not relabelling
that comparison as a full gate pass.

The UI reports speaker-threshold matches as component measurements only. It must
not call them accepted requests, successful A03/A04 trials or qualified identity.
When activity measurements are enabled, the same native session separately owns
[frame-span calibration](activity-calibration.md): one current score array,
single-use observed labels, measured provisional speech/overlap/pause cutoffs,
and held-out diagnostic boundary counts. These policies never replace the
speaker threshold or create qualified identity. Whole-record condition labels
may cover all supported conditions before speaker Freeze when activity is
enabled, while speaker Freeze still requires actual owner/other comparisons.
VAD/signal sufficiency, endpointing, overlap, echo/replay and directedness adapters,
actual grants, final authenticated review, and the plan's 100/200/200 whole-gate
holdout evidence remain required before a QualifiedProfile can be constructed.
The separate `review_voice_admission` entry point freezes the whole-gate
operating point only after the measured speaker and activity policies exist. It
uses the actual native utterance owner for subsequent independently labelled
recordings, actual ASR/speaker and reasoner-backed directedness observations,
and exact adapter/context-bound evidence. It never accepts scores, outcomes or
pass flags from the page. A separate explicit consent and final review can
promote only the required complete held-out corpus; missing/unknown negatives do
not count as measured rejections. No actual owner corpus has passed that review.

Successful review stores a bounded DPAPI-protected native evidence report,
separate from the unchanged candidate and selection files. It retains redacted
counts and native attempt IDs, the measured operating point, candidate hash,
owner/registration, native consent and exact live adapter bindings. Ordinary
restart revalidates this report before creating session-bound admission; closing
Settings preserves reviewed permission but discards unfinished calibration.
`revoke_voice_admission` immediately withdraws admission and removes the report
under a retained native writer. A direct discard command cannot silently remove
a live reviewed token while leaving its restorable report behind. Changed engine
incarnation may reuse measured evidence only when the protected report and fresh
controlled-load inspection both contain the same complete quality fingerprint
(serving image/configuration, full artifact roster, policy, GPU/runtime and helper
sources). The artifact must also match. Native restore rebinds only the live
directedness revision after that comparison, preserving the original stored
evidence and revalidating all counts and owner/device bindings. Missing fingerprints
retain exact-incarnation matching; artifact identity alone never permits reuse.
See `turn-gating.md` for the full boundary.

Entry points: optional calibration context on `check_saved_voice`,
`freeze_voice_calibration`, `voice_calibration_status` and
`discard_voice_calibration`; the existing native
recording owner alone commits measurements. There is no score/report import API.

The qualification UI starts with live activity enabled and explains the actual
sequence without asking the owner to repeat saved enrollment: record varied
owner and consenting other-speaker comparisons; annotate actual single speech,
overlap, quiet, within-utterance pause and ending silence; freeze measured speaker
and activity policies; record another representative owner utterance under the
frozen activity policy to measure signal sufficiency; then freeze the whole gate.
Only subsequent new recordings count toward the100 directed-owner,200 combined
negative and200 owner-conversation requirements, plus switch/ambiguous checks.
Review and explicit listening consent follow successful native counts. The page
must remain open; the current in-memory collection has a eight-hour deadline and
1024 admitted-recording limit. Missing trials remain visible. This is a sizeable
qualification session, not a one-recording activation shortcut.

After a transient capture/service failure, “Revalidate saved voice permission”
can replace an inactive runtime token using the protected reviewed report. It
does not recollect or rewrite evidence, clear backend uncertainty, change listening
controls or replay an accepted turn. This explicit Settings action performs fresh
owner/session/registration/model checks and respects revocation. A failed,
cancelled or hidden-window attempt stays suspended until deliberate retry; normal
device opening follows only a successful current restore and existing listening
consent. An absent or incompatible report requires actual qualification.

Native calibration profile/pairing and owner/registration-intent readers acquire
the shared owner-management admission before scheduling blocking work. The actual
blocking reader retains its owned guard even if cancellation drops its caller.
Freeze is bounded natively by that same admission; a frontend busy flag is not
the resource boundary. A completed read does not bypass the subsequent current
owner/registration/context and visible-Settings checks before measurement freeze.

## Checking saved enrollment

The guided enrollment UI saves the completed six-phrase candidate immediately after the sixth explicit recording succeeds. This only calls the existing protected native finish operation; it never selects/qualifies the candidate. If saving fails while the completed session remains readable, retain the six-phrase session and offer Save my voice. A recording failure refreshes the actual native session before deciding whether earlier phrases must be repeated. Previously saved candidates remain untouched.

After the six segments are saved, People & Voice ID must offer an explicit eight-second voice check against that exact candidate, without requiring recollection or changing the selected profile. This is a read-only setup operation, not identity activation. It uses the same native recording lock, media worker, frame checks, local controls and bounded capture deadline as enrollment. It requires the current local owner, visible Settings, the candidate's exact microphone/model and an authenticated current Spark session. No Windows verification is needed for this read-only check; selection, deletion, ownership and permissions retain their existing verification requirements.

Before opening the microphone, check the paired ASR and speaker services. Show checking, recording and processing progress; Cancel, navigation away, close, lock, disconnect, local control changes or device changes cancel the original attempt. Request identity and capture/connection/action epochs bind all results. End recording before inference. PCM and new embeddings remain native and transient. Return only the explicitly requested transcript, measured cosine similarity, the two existing held-out similarities and measured clipping/duration. These results stay on the setup page and are never history, planner input or a probability. Separately consented development review uses native-retained evidence; the page cannot submit scores or a pass. Never label a single matching phrase as qualified automatic recognition. The UI offers short development review with explicit unvalidated limitations; release validation remains separate.

Entry points: `check_saved_voice` (explicit bounded native check), `cancel_voice_check` (same-request cancellation), shared phrase capture (also used by `record_enrollment`), `voice_check_progress` (current attempt metadata only), paired `GET /voice-analysis` (ASR/speaker preflight), and existing `POST /voice-analysis` (transient inference). Candidate storage and profile selection are read-only/unaffected. Normal voice admission requires native Development or ReleaseQualified evidence; action scopes and protected approvals remain separate. Per the repository owner's instruction, verification uses static checks/builds and direct authorized live observations, not automated tests or an evaluation harness.

An authenticated local setup action consumes a fresh Windows verification into one native enrollment session. The session binds its opaque UUID, authenticated connection generation, microphone configuration and capture epoch. It expires after five minutes and is invalidated by Settings close, explicit cancel, lock, disconnect, device change, mute, deafen or pause. Each recording is an explicit action; setup does not unmute the microphone. The normal assistant remains unavailable throughout enrollment.

The initial collection has three prompted phrases, one natural-speech segment, and two separate held-out phrases. These counts are collection defaults, not evidence of recognition accuracy. Every segment has a new UUID, at most ten seconds of16kHz mono PCM, and a deadline covering capture plus processing of no more than30seconds. No raw audio is written to disk. A discontinuity, insufficient signal, stale result, model change or failed inference requires retry of the current segment with a new UUID.

Capture clock correlation is approximate, not a sample counter. Native-rate and resampled inputs must share one timing policy: strictly advancing nonempty packet timestamps, at most 2ms plus one source sample and1us rounding allowance of adjacent prediction error, and at most 4ms plus the same allowance of deviation from the attempt's original sample-count timeline. The anchored bound prevents repeated small gaps from accumulating indefinitely. The first admitted nonempty packet establishes the anchor; an epoch change clears all timing state. Empty callbacks do not advance it. Overflow, invalid rate/shape, backward/duplicate clocks, larger jumps, queue sequence loss and audio older than500ms remain rejected. These bounds are Avesra's capture compatibility policy, not a claim of sample-exact continuity or validated speaker quality. No silence is inserted and no missing frames are synthesized.

Windows exposes separate device-frame positions and QPC time correlations; the current CPAL adapter supplies the latter but does not expose native discontinuity flags/frame positions. See [Microsoft's GetBuffer contract](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer). Full native frame-loss evidence and calibrated speaker qualification remain separate from this bounded capture timing check.

Before admitting the first packet of an attempt, allow at most250ms from its first nonempty callback for a valid capture/callback clock correlation. Discard that startup pre-roll without filling sample buffers, feeding the resampler or creating a timeline anchor. A valid packet must have a capture time no later than its callback and be at most500ms old. Once established, an invalid correlation fails the attempt immediately; it never silently restarts or skips audio inside a recording. If no valid clock arrives within250ms, fail. Both capture paths share this startup policy, reset on epoch replacement, and remain subject to the original12second deadline. The no-drop rule above applies after admission of the first valid packet.

Only the native/controller adapter may provide derived192-dimensional speaker embeddings with exact configured model revision and observed duration. Prompted/natural embeddings form a normalized mean; held-out vectors remain separate and contribute measured cosine similarities. Scores are similarities, never probabilities. Overlap detection and calibrated operating thresholds are separate required evidence. Missing evidence keeps the result a candidate and cannot set enrolled or voice-ready. No default numeric acceptance threshold is invented.

Candidate persistence contains derived representations, model/profile revisions, the microphone configuration, collection counts, duration and measured held-out similarities. It uses per-user Windows DPAPI and atomic publication; generic settings, history and diagnostic exports cannot return its vectors. Re-enrollment creates a new revision. Revocation/removal requires fresh local verification and must invalidate active turns before removing the exact selected revision. Additional people start with no grants. No voice match can create or change ownership.

Fresh local verification can select one exact protected candidate for subsequent validation. Selection verifies the candidate/schema/model/microphone, writes a separate DPAPI reference with atomic create-only publication, and never modifies its derived vectors. Replacing a selection requires an explicit verified clear first; an unreadable selection can be cleared without decrypting it and does not hide candidate records. Deleting the selected candidate clears its reference before removal. Selection changes invalidate epochs and withdraw enrolled/voice readiness; selecting is not qualification, ownership or action permission. No selection has been performed during development. Candidate and selection schema2 bind the stable WASAPI endpoint ID, not its display label. Legacy name-bound candidate/selection records remain unqualified and require explicit deletion/recollection; no name-to-device guess is made.

Implementation status: native prompted recording now uses the single media worker, current acknowledged control session and pinned TLS speaker route. Each explicit press checks service metadata before opening the selected device, captures eight seconds, rejects frame loss/old frames, closes capture before inference and consumes only a same-session/epoch/segment/model reply. The frontend sees capture state and progress, never PCM or vectors. Cancel, close and stale setup advance the control epoch to cancel server compute. A normal recording advances and rebinds only the idle enrollment session, preserving prior derived segments. All this remains uninvoked: live device/transport/held-out quality and calibrated activation are not proven; no owner enrollment or authentication prompt has been executed during development.

The paired speaker route is an inference boundary, not an enrollment/ownership grant. It accepts only bounded PCM and opaque correlation under a revocable device credential. An operator-configured private Unix socket and immutable speaker revision select the deployment; requests cannot choose a model/path. The companion invokes it only from native prompted collection after local verification. Returned embeddings never enter webview events or generic diagnostics. One active inference, request-body deadlines, a15second inference budget and cancellation on route abandonment bound retention. A loaded service remains unqualified and cannot activate normal listening.

Deployment uses `speaker-deployment.json` in the private controller directory with exact `socket` and `model_revision` fields. Missing configuration disables `/speaker`; it never falls back to another host/model. The route requires an active authenticated control session, current capture epoch, explicit permissive modes and a fresh request UUID. It reserves the UUID in a bounded replay window and creates an independent worker cancellation ID. Live control changes cancel admitted work via a session watch; response validation repeats session/epoch/model. The route is ARM64 compiled and deployed in the scoped preflight controller, but has not been called with paired media.

The native capture gate carries an immutable12second monotonic deadline for each enrollment attempt; callbacks stop independently of the command future or UI responsiveness. During transient PCM collection the command does not perform synchronous UI-thread queries. Settings close and local controls cancel through the native epoch publisher. Cleanup checks its expected epoch atomically, so an old recording guard cannot cancel a newer session.

Windows session monitoring starts closed. During native setup the owned Settings HWND registers this-session WTS notifications on its event thread, then queries `WTSSessionInfoEx`; only an active, explicitly unlocked session opens the OS gate. Failed registration, an unknown query result, or a disconnected session stays visibly unavailable. Lock, logoff and session disconnect invalidate both epochs, pending verification/recording and connection generation, stop media, and abort the control connection. Unlock/reconnect notifications requery OS state. The first verified unlock per app launch also schedules the single saved-pairing reconnect described in product.md; subsequent notifications only clear the OS gate. They never unmute, restore a setup proof or qualify speech. Hidden Settings retains the hook; destruction unregisters and removes the subclass before releasing its context. No native window getter runs while the runtime state mutex is held.

This follows the [WTS session-state contract](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoex_level1_w) and [same-thread subclass requirement](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass). The notification path is compiled source only; actual lock/unlock, logoff and teardown behavior have not been exercised during the owner's gaming restriction.

Recording failures must distinguish actual five-minute expiry from local cancellation and microphone failure. The shared native media worker preserves a bounded, non-sensitive failure reason (device open, stream interruption, clock discontinuity, invalid timestamp, queue overflow or deadline). Enrollment returns that reason from the current runtime instead of labelling every invalidated session expired. No PCM, device credentials or raw driver errors are exposed. A new explicit recording clears the prior failure. The same native capture failure reasons apply to both native-rate and resampled microphones; gates remain closed on failure.
