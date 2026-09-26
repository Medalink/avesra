# Personal conversation during native playback

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
