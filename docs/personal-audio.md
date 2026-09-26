# Personal conversation during native playback

Normal startup retries only definitely read-only audio health preflight and local
management contention while the same connected, unlocked, unmuted, unpaused,
non-deafened session/capture epoch remains eligible. One attempt is outstanding;
backoff is 2, 5, 15, then 30 seconds, capped at 30 seconds thereafter. The visible
status names the waiting reason, without repeated error notifications. HTTP
temporary-unavailability/busy/timeout and validated busy/unloaded lane states are
retryable; invalid metadata, unsupported versions/revisions, authentication,
owner/storage failures and attempted registration uncertainty are not. ASR and
activity health preflight occurs before possible registration mutation. A failed
registration is never blindly repeated; reconnect/owner management must reconcile
its actual status and original native registration intent. Current controls cancel
eligibility without renewing any accepted-turn or capture deadline.
Audio metadata follows the controller's exact cancellation contract: ASR/speaker
require `terminate_process`; TTS/activity additionally permit
`cooperative_reset_or_terminate` (including their non-streaming configurations).
This describes observed worker cleanup capability, not permission or readiness.
Interrupted bounded health-response transport is retryable; malformed JSON,
oversized bodies and invalid typed metadata are blocked.

Personal intent is the explicit provisional native conversation policy. Startup
does not inspect or require a strict directedness model that this admission kind
never calls. Its profile uses a fixed native policy revision and stores no invented
directedness model/artifact/incarnation. Development and ReleaseQualified admission
still require their exact observed directedness binding. Current Windows owner,
paired actor registration, saved voice source, ASR/speaker revisions and streaming
activity readiness remain required; reasoning availability is checked by the
ordinary accepted planner when an actual request needs it.

Personal admission follows its live native session instead of expiring after
twelve hours. Its private profile lifetime has no wall-clock deadline; only the
Personal kind permits that representation. This does not make saved identity or
speech permanent authority: every existing owner, selected device, model, session,
epoch and grant check still applies, as do mute, deafen, pause, lock, revocation,
disconnect and diagnostic-mode capture suppression. No periodic timer restarts
capture or discards an unfinished utterance. Restart restores actual protected
observations through the existing current-owner revalidation path.

Development and ReleaseQualified profiles retain their existing finite deadlines,
including the bounded collection period and twelve-hour reviewed/restored lifetime.
Original utterance freshness, accepted-turn deadlines, learning bounds and private
transport deadlines are unchanged. Personal recognition remains provisional;
continuous lifetime is not a release qualification claim.

Personal capture may continue while Avesra speaks. It uses native post-format
stereo mixed output (speech, effects and background) as a provisional echo
reference. This is not `NoOutput`, acoustic-clean proof, release qualification,
or an output-device audibility measurement. Other admission modes keep their
existing quiet/output rules. Overlapping speech cannot teach the owner profile;
Personal overlap admission also requires the existing owner voice anchor.

The output callback maps its predicted playback timestamp to monotonic `Instant`
using the callback timestamp and entry instant, plus the sample offset. The
microphone already maps its capture timestamp to the same process clock. Raw
stream clocks are not subtracted across devices. The complete callback DAC horizon
(including its last sample) is bounded to
500 ms; a missing/out-of-bound clock closes that output attempt before submission.
Cancelled or incomplete output remains unknown for that 500 ms horizon plus
200 ms acoustic search tail, including unreported partial reference chunks;
missing mapping, missing sequence, discontinuity, overflow or missing reference
coverage is Unknown. Submission telemetry retains its original timestamp.

Off-callback native code retains at most one second of actual mixed PCM and a
bounded output interval history. A per-capture Input binds the capture epoch,
checks ordered 20 ms native frames and owns provisional delay/gain state. It
searches 0–200 ms acoustic delay at 1 ms spacing with local one-sample refinement,
uses the actual stereo channels,
and subtracts a bounded two-channel linear estimate. Strong reference correlation
is required to fit/update the path; otherwise established coefficients are frozen
so independent near-end speech does not train the echo path. Output epoch changes
reset the learned path even when the input owner remains continuous. No inferred
clean
silence replaces microphone samples. The actual computed residual is returned;
unknown reference returns original microphone samples marked unknown.

Per-frame metadata distinguishes output overlap, known reference coverage and
residual dominated by the estimated echo. Native voice processing retains this
metadata under its original capture owner and checks the completed span. Bounded
per-frame hardware-projected capture times remain attached to retained PCM across
eight-second transport windows; completed endpoints never renew their timestamps. UI or
model text cannot create it. This lightweight room model cannot guarantee echo
removal, double-talk separation or replay rejection. Nonlinear speakers, long
reverberation and clock jitter can cause missed interruptions or false activity;
actual headphone and room-speaker runs remain required evidence.

Entry points: PlaybackReference carries callback-owned sequence/played_at;
playback_signal ingests before display throttling and records open/retire;
MediaWorker::personal_input creates a native epoch-bound processor; normal
Personal capture consumes its samples/metadata. No callbacks allocate or lock for
this addition, no microphone data persists, and no browser command exposes it.
Source checks/builds are allowed; automated tests and live playback are excluded
from this implementation task by the owner's instruction.

Clock semantics: CPAL documents playback as predicted device delivery and does
not guarantee cross-stream timestamp origins:
https://docs.rs/cpal/latest/cpal/struct.OutputStreamTimestamp.html
https://docs.rs/cpal/latest/cpal/struct.StreamInstant.html
