# Explicit activity frame calibration and diagnostic endpoints

This is an in-memory component measurement workflow inside the existing native
VoiceCheck calibration session. Component Freeze alone never creates a
QualifiedProfile, sets readiness, opens automatic capture, or approves a spoken
request. Its measured policy also feeds the shared native utterance owner used
by the separate whole-gate qualification and normal producer. See
`turn-gating.md` and `utterance-ownership.md` for those additional gates.

Only the native recorder can provide activity scores. After a successfully
completed explicit check, native state retains at most that latest request's
125 four-score frames, bound to the existing candidate/owner/registration,
microphone, device/session/action context and exact activity model/mode. Starting
another recording replaces the retained frame array; an unannotated recording
remains visible in totals. Discard/context loss/two-hour expiry removes it.
Neither PCM nor transcripts nor embeddings are retained by this workflow.

The operator submits one bounded set of nonoverlapping half-open frame intervals
for that exact request: `single_speech`, `overlap_speech`, `background`,
`within_pause`, or `end_silence`. A frame is 80 ms. Background is quiet unrelated
to an utterance, within-pause is quiet inside an intended utterance, and end-silence
is quiet following its actual end. Labels express operator observations; the
application never infers them from a model score or a whole-record speaker label.
At most 32 intervals are accepted, and unlabelled frames are excluded explicitly.
No score values, cutoffs, revisions or pass flags are accepted from the page.
Annotation is single-use and freezes that recording's labels; the page can inspect
its score plot but cannot edit submitted labels to improve results.

Before recording, annotation, and Freeze, native state verifies its bound context;
annotation and Freeze refresh protected candidate/owner and authenticated Spark
registration. Concurrent recording/review or stale/retired request IDs reject.
Successful recordings, annotations, missing results and unannotated results are
accounted separately for calibration and held-out phases.

An explicit activity Freeze derives provisional operating points only when the
labelled observations strictly separate:

- Speech cutoff: midpoint of the highest strongest-channel quiet score and the
  lowest strongest-channel score labelled single or overlapping speech.
- Overlap cutoff: midpoint of the highest second-channel non-overlap score and
  the lowest second-channel overlapping-speech score.
- End-quiet duration: integer midpoint, rounded up, between the longest labelled
  within-utterance pause and shortest labelled ending silence, requiring the
  former to be strictly shorter. Both durations must contain observed frames.

Missing classes, ambiguous score ranges or nonseparating pause durations reject
Freeze. There are no default cutoffs or quiet durations. These are provisional
component policies, not statistically sufficient qualification. Their native UUID
and operating points are immutable for the session. Only recordings admitted
after Freeze contribute to held-out counts; prior captures cannot be relabelled
as held-out evidence. Held-out labels report frame mismatches and quiet-span
decision mismatches without retuning the policy.

Explicit checks admitted under that frozen policy consume the actual ordered
score stream to display tentative speech starts and end decisions. A start is a
speech-threshold crossing from idle; an end is the measured number of consecutive
quiet frames after speech. The display also reports provisional speech/overlap
frame counts. These observations do not drive the microphone, ASR submission,
TurnGate, actions or normal voice producer. At the eight-second recording cutoff,
an unfinished span stays unfinished; the cutoff is never reported as an endpoint.
Batch checks can show the same diagnostic result after completion. Live checks
show it alongside the existing immutable activity blocks.

All state remains bounded: at most 1,024 admitted recordings under the existing
session owner, one retained raw score array, class totals/ranges, and at most 125
diagnostic boundary events per recording. Reports contain counts and operating
points, not retained speech. Endpoint, overlap and owner quality still require
real held-out observations, independent echo/replay/directness evidence, actual
voice consent/grants, and the plan's full 100/200/200 whole-gate evidence.

Entry points: existing `check_saved_voice` activity recording, exact-request
`annotate_voice_activity`, activity scope on `freeze_voice_calibration`, existing
status/discard commands, and the explicit VoiceCheck frame editor/diagnostic view.
