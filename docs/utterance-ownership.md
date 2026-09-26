# Native utterance ownership

One native capture owner consumes contiguous 16 kHz PCM and the corresponding
immutable 80 ms activity frames. It binds the exact native context and measured
activity policy for its lifetime. The same segmenter is used by qualification
and normal voice; a diagnostic decision is not a second endpoint algorithm.

PCM arrives in native 20 ms frames, with contiguous sequence and a first-sample
timestamp no older than 500 ms. Activity may lag capture; it cannot lead the
captured samples, change model revision, repeat frames, or append after EOF.
Storage is bounded to one ten-second utterance, 200 ms pre-roll and two seconds
of detector lookahead. These are resource bounds, not guessed speech cutoffs.
Exceeding a bound cancels the owner, never emits an endpoint or renews a budget.

A speech crossing under the frozen measured policy starts a fresh native UUID.
Only the measured quiet-frame run completes it. The completed value owns its
PCM, exact context, policy revision, capture times and signal/overlap counts;
it cannot be deserialized or recreated from UI start/end indices. An original
ten-second utterance limit cancels unfinished speech and suppresses new starts
until an observed quiet run. EOF cancels unfinished speech; it is not silence.
Each successful endpoint is consumed once. The caller retains actual capture,
model and inference admission until their real cleanup completes.

Qualification can inspect this same ownership without granting permission.
Normal capture additionally needs a current native qualified profile and voice
consent grant. Segment extraction does not establish echo/replay, directedness,
speaker identity or action permission. Those remain separate evidence checks;
unknown evidence abstains. No raw capture or rejected text is archived.

Entry points: native activity calibration consumes ordered model frames;
native voice consumes actual capture and model frames; neither frontend frame
annotations nor persisted reports are endpoint-producing entry points.
