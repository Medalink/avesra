# Explicit speech-activity measurements

The native Performance view measures only actual explicit stream session/exchange
durations and original oldest-sample age. It retains no PCM or model scores and
does not infer qualification; see `performance.md` for bounded process-local
retention, failure/abandonment and host-local timing semantics.

## Incremental explicit check contract

An additional opt-in `activity-streaming.config.json` configuration with
`activity_streaming: true` permits the
existing private `stream` request lifecycle on the activity lane. Batch `infer`
remains available. One original 30-second stream owner retains the NeMo speaker
cache, FIFO and feature context; cancellation/expiry terminates that same child.
Every chunk is ordered, at most 3,200 samples in 160-sample units, with a cumulative 160,000-sample
limit. Idle gaps, lost chunks, changed epoch or attempted replacement fail closed.

The adapter emits immutable contiguous score blocks, with the first frame index,
cumulative received samples, exact model revision and final marker. It processes
48 feature frames per chunk with 56 frames of right context and up to eight of
left context, through `forward_streaming_step`. Only stable STFT centers enter
the feature queue. End-of-input flushes real remaining features and trims only
model padding; it never manufactures missing scores or speech endpoints.

Paired `GET /voice-activity-stream` upgrades to a WebSocket after authenticated
native capture permission checks. Public stream version 2 is required in Start,
Acknowledgment and StreamReply; older peers reject, with no downgrade. The private
model stream remains version 1. A correlated acknowledgment precedes an explicit
VoiceCheck's microphone capture. Continuous Personal capture may already be
running when the next bounded transport window opens. The same eight-second
explicit VoiceCheck capture supplies
200-ms packets to this stream through a bounded native queue. Each packet and
reply retain request/session/capture epoch, sequence and immutable sample/frame
offsets. Permission loss, original deadline or queue overflow cancels the stream.
Every packet carries required `captured_age_ms`, rounded up from the original
oldest native frame's monotonic age immediately before sending. Reject future
capture or age above 500 ms. This includes PCM buffered while the previous actual
stream retires and the next one opens; do not discard it or stamp it fresh.
The server anchors one capture origin from its first packet receipt minus that
native age assertion. Each packet also carries `elapsed_since_ack_ms`, floored
from the actual native acknowledgment receipt to the same send observation.
The server freezes a conservative transit uncertainty equal to its interval
from acknowledgment send initiation to first packet receipt minus that native
elapsed time. This bounds both acknowledgment-out and packet-in transit without
counting microphone startup or sample gathering twice. An impossible, decreasing,
or over-lifetime native elapsed value fails. The fixed uncertainty is subtracted
from every projected capture timestamp, never waived or renewed.
Later receipt-minus-age claims must agree with the fixed
origin plus immutable sample offset within 100 ms of transport/clock tolerance.
The earlier of those two times, minus that fixed uncertainty, feeds the private
model stream and must still be at most 500 ms old. A slow initial handoff can
therefore conservatively fail even when native PCM itself is fresh. The packet
end only bounds forward pacing (100 ms tolerance)
and never renews oldest-sample age. This is an authenticated native age assertion
projected to the server clock, not synchronized-clock proof of one-way network
latency. Native freshness remains checked before every send. Original public and
private stream deadlines, cumulative sample limits, retained cancellation owners
and terminal correlation are unchanged.
No diagnostic request opens an additional recording or enables normal listening.

VoiceCheck offers batch or live measurements explicitly. The live option displays
validated score blocks during recording, then correlates the complete observation
with the existing ASR/speaker result. Calibration context includes this choice;
switching modes requires a new session. The original batch behavior is preserved.
No VAD/overlap threshold or endpoint hangover is guessed. Native component
calibration can freeze measured cutoffs; the separate whole-gate review described
in `turn-gating.md` can create a qualified profile only after its required held-out
measurements. Service scores or the generated-audio proof below cannot set
readiness. The batch proof does not establish streaming correctness.

The optional `activity` audio lane measures the explicitly recorded VoiceCheck
phrase. It uses NeMo 3.0.0's installed `SortformerEncLabelModel.forward` on a
bounded in-memory 16 kHz mono tensor, never a temporary WAV or dataset manifest.
The selected artifact is NVIDIA `diar_streaming_sortformer_4spk-v2.1`, revision
`cd03eee90fbec18297ac31b8c21546e596b7f71c`, file
`diar_streaming_sortformer_4spk-v2.1.nemo`, 471367680 bytes, SHA256
`8abd32832159c6ac1148c926b7276f35ba34582c444e559dce1f1253fea42ef8`.
These values were read from the publisher's immutable metadata. The isolated
runtime observation below verifies this exact artifact and one generated-audio
inference; detector quality remains unqualified.

The existing ASR runtime contains the concrete model API. The activity lane is a
separate same-UID private Unix-socket child with its own inference/cancellation
owner, reusing the pinned NeMo environment. It has no permission authority. Batch
configuration advertises nonstreaming; the opt-in streaming configuration adds
the incremental path described above. Loading
must verify the artifact digest before deserializing it. The fixed low-latency
Sortformer geometry is chunk6/right7/FIFO188/update144/cache188 at 80 ms per frame;
it is a reproducible configuration, not a locally measured recognition policy.
Loading also verifies NeMo 3.0.0, a 160-sample feature hop, encoder/module
subsampling of eight and four output speakers before serving observations.

Configuration is opt-in: rebuild the audio source using `Dockerfile.asr`, run a
separate same-UID supervisor with `activity.config.json` and its private socket,
and supply controller `activity-deployment.json` containing that absolute socket
path and the exact `model_revision`. Use the existing explicit administration
load operation. The model directory must be mounted read-only, the cache private,
and the inference container offline with bounded resources as for ASR. This
configuration is not created automatically by the application. The isolated
runtime observation below did not change controller or existing ASR configuration.

`GET /voice-activity` supplies authenticated health for explicit preflight.
`POST /voice-analysis` version2 accepts an optional `activity: true`; omitted/false
retains the base analysis shape, including its required nullable typed timing
receipt ([voice-timing.md](voice-timing.md)). Explicit activity requests fail if the
lane is absent/unready and never fall back to invented clean/unknown scores.
The same PCM is processed concurrently with ASR/speaker under the existing
15-second inference budget and current paired capture permission. Cancellation
targets all admitted workers. No additional microphone window is opened.

Returned activity evidence binds the pinned model revision, actual input sample
count and contiguous 80 ms frames, each with four finite scores in [0,1]. At most
160000 input samples / 125 frames are accepted. Padding beyond the input is
discarded only after the model supplies every required frame; short output is an
error. Scores are model activity outputs, not calibrated probabilities, speaker
identity, overlap decisions, clean audio or accepted requests. The native bridge
requires exact requested activity presence, revision and sample count.

VoiceCheck displays strongest and second-strongest activity scores over the
phrase without applying a guessed threshold. The optional lane can supply actual
speech/overlap calibration observations once installed. The shared native
utterance owner now uses frozen measured policies for explicit qualification and
normal endpoint capture; the activity model alone supplies neither identity nor
permission. Acoustic echo/replay rejection, directed intent, grants and final
owner review remain separate gates. Speaker-only
calibration sessions bind whether this exact activity adapter was requested;
changing it requires a new session. Raw score arrays remain transient on the
explicit setup page and are not appended to retained calibration summaries.

Entry points: activity service configuration/load/infer; Rust AudioClient activity
validation; authenticated health and optional voice-analysis; native
`check_saved_voice` opt-in; VoiceCheck display; shared native utterance ownership
and qualified normal capture. Enrollment and unlabelled checks without the option
retain their existing behavior.

Normal capture and whole-gate live qualification require authenticated activity
health with the exact pinned revision, `state: loaded_unqualified`, and
`streaming: true`. This state means a loaded measurement service, never qualified
owner permission. The UI's live selection and native preflight require that same
streaming capability; a healthy batch-only lane is insufficient and there is no
batch fallback. Deployment must explicitly point the controller at the verified
streaming service after its model is loaded. Recorded service proof describes its
observed run only; idle release or a later configuration change requires fresh
health/load verification before another session.

Source inspection and static/build checks cannot establish GPU/runtime compatibility
or measured detector quality. No automated fixtures or harness are part of this
implementation. Publisher grounding: [model card and output shape](https://huggingface.co/nvidia/diar_streaming_sortformer_4spk-v2.1),
[pinned artifact](https://huggingface.co/nvidia/diar_streaming_sortformer_4spk-v2.1/tree/cd03eee90fbec18297ac31b8c21546e596b7f71c).

## Isolated runtime observation, 2026-09-25

On Spark, the exact artifact above was downloaded at a maximum 5 MiB/s into
`/home/medalink/.cache/huggingface/avesra/diar_streaming_sortformer_4spk-v2.1-cd03eee`,
then its size and SHA256 were verified. Source archive SHA256
`16f6522f4ecd0ff3342efd944cb0ef6649bce22e2c205c8e0a2977958d950344`
was layered over existing NeMo image
`sha256:9256e738088d2531674c6ff02ec02e229cad056208ff4e136474d85a83be2188`,
with no dependency changes. Resulting image:
`sha256:f0539ab96aedb5edc552d554485aca7034b59215959d58d9dd9233a99a675c70`.

Separate container `avesra-activity-20260925` runs as UID/GID 1000, offline,
with two CPUs, 8 GiB memory/no additional swap, all capabilities dropped and
no-new-privileges. Its private socket is
`/home/medalink/.local/share/avesra-build/activity-20260925/run/activity.sock`.
The existing administration load succeeded and health reported
`loaded_unqualified`, `streaming: false`, `permission_authority: false`.
No existing service was stopped or restarted, and no controller deployment
configuration was written for this observation.

One actual request used only the already generated custom-voice service PCM at
`/home/medalink/.local/share/avesra-build/evidence-20260925/custom-service-16k.pcm`,
SHA256 `ce816e5c0570e605ea615d739e7e8d648de3db2e2c5c264c96b59e9e7fa2fa04`.
The 99,840 samples (6.24 seconds) produced exactly 78 frames of four finite scores
in [0,1], through the real supervisor and model forward path. Service inference
took 652.978 ms (654.347 ms request wall time); post-request health reported one
successful inference and no active work. Observed idle container memory was
1.997 GiB; host available memory remained about 35.5 GiB with 49 MiB swap used.

The private observation file is
`/home/medalink/.local/share/avesra-build/activity-20260925/generated-voice-activity.json`,
SHA256 `fc945784de17c493ee224674b49164afeff462317c959756f899f0c2da734212`.
This establishes a bounded batch tensor/service path on the actual Spark GPU.
It is not microphone capture, owner calibration, overlap rejection, continuous
endpointing, controller/native end-to-end proof, or an A02–A07 qualification pass.

## Isolated streaming observation, 2026-09-25

After independent source review and Windows static/frontend plus ARM static/release
checks of frozen archive
`d754e1b4f3c957c716468dfa02d39527f5e949a0502613d8ddcdb24c72019e43`,
the unchanged NeMo base was layered with Python source archive SHA256
`24d452039958b3bdc5506a036bcd8f5a0eee1887e595a8edd48b2a8ec1695194`.
The resulting image is
`sha256:4fe867b6a5c71ea1fc7b6da3ff4be122bab2401e17837451de394ad2c9249457`.
Separate container `avesra-activity-stream-20260925` retains the same isolated
UID, offline network, two-CPU/8-GiB limits and read-only model mount described
above. Its socket and private deployment example are under
`/home/medalink/.local/share/avesra-build/activity-stream-20260925`.
The working batch container and controller configuration were not changed by
these observations.

Manual requests streamed only the already generated custom-voice PCM, paced at
its actual 16 kHz rate with at most 3,200 samples per chunk:

| Generated input | Input samples | Replies / score frames | First score | Maximum round trip | Total wall time |
| --- | ---: | ---: | ---: | ---: | ---: |
| Full 6.24-second recording | 99,840 | 32 / 78 | 1.344874 s | 351.718 ms | 6.284229 s |
| First 6 seconds of the same recording | 96,000 | 30 / 75 | 1.223849 s | 83.294 ms | 6.044281 s |

Every response matched the original request/session/epoch, ordered chunk and
frame offsets, exact cumulative samples and pinned revision. All four scores per
frame were finite and within [0,1]. The six-second prefix exercised the partial
final model block: the final response contained 15 new frames, including its
three-frame tail, with exactly 75 total frames. No missing input or scores were
filled with synthetic values.

A separately opened stream on the same generated input was cancelled between
chunks. Cancellation acknowledged `cancelled` in 205.584 ms; subsequent health
reported `unavailable` and not busy, confirming child retirement. Another stream
used an original 2,500-ms deadline; a second chunk completed at 1,223.691 ms.
At 2,801.327 ms, health reported unavailable/not busy, before that second chunk's
two-second idle expiry. This observed the original deadline without renewal.
The isolated candidate was explicitly reloaded afterward and left idle with
`loaded_unqualified`, `streaming: true`, `permission_authority: false`, and two
successful completed inferences. Host available memory remained about 33 GiB.

Private evidence is copied locally under `artifacts/e2e-20260925/activity-streaming/`:
`full-6_24s.json` SHA256
`42ac2315e5a995ef064c8b5c30df1b07258e457965a11be5fc3fd95891d3cb6b`,
`prefix-6s.json` SHA256
`b047d29b2ad3eee6c3c53596f209e47e54093471b9b1e12a949427e81dc4c965`,
and `lifecycle.json` SHA256
`57f2244154ac1505677973aef4315a5566131bf55bad04fca247f767254751ce`.
These are actual private-service/GPU observations, not microphone, native UI,
paired WebSocket end-to-end, owner quality, continuous endpointing or A06 proof.
