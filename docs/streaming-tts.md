# Incremental generated-voice synthesis

Normal interruption uses [warm cancellation](audio-cancellation.md): the actual
child unwinds its hook, retires CUDA work and acknowledges the exact request
before the controller releases its permit. Uncertain retirement still terminates
the worker; a cancelled prefix never becomes a completed speech result.

Explicit editable voice tests and fixed startup greetings use preview wire
version 2 to expose this actual incremental path. Their controller waits for the
first validated private chunk, sends `StreamingReady` with a sample ceiling, and
paces bounded queued audio while the same private job continues. It no longer
collects a complete synthesized waveform before Ready. Only the actual Complete
terminal supplies the final exact sample count; a failed/truncated prefix cannot
be reported as fully spoken. Saved reference playback remains exact-length PCM.
See [generated voices](generated-voices.md) for ownership, pacing and telemetry
boundaries. Latency improvement requires a new actual native observation; the
earlier complete-waveform proof does not measure this changed path.

The required TTS path emits PCM while codec generation is still running. Splitting a complete waveform or using Qwen's simulated text-input flag does not satisfy this contract. The selected generated reference voice remains mandatory; requests cannot supply a file path, voice identity or clone reference. Output is transient24kHz mono signed16-bit PCM, bounded to30seconds and bound to one reply/utterance/session/playback epoch. Normal voice readiness remains closed until the actual synthesis/transport/playback path is qualified.

The initial adapter is tightly bound to qwen-tts0.1.1 and the inspected source hashes in the final locked image. A temporary hook on the selected model's talker observes only complete codec vectors from forward output, excluding prefill and EOS. It decodes bounded new-code chunks with up to25 left-context codes, seeded from the selected generated voice's reference codes, and removes all context audio before emission. Text/ref-text tokenization and ICL prompt preparation use the same pinned wrapper helpers. It does not call the wrapper's complete-waveform synthesis method or decode the final waveform a second time.

The model exposes full codec vectors one forward step after token sampling. Actual generation completion is separately observed from sampled EOS through a scoped wrapper around the owned talker's generation call; a token limit without EOS is truncated, never a fully spoken answer. Hooks/method overrides are removed in a finally block. One child owns generation and decoding; output pipe backpressure blocks further generation, and the supervisor's fixed deadline/cancellation can terminate that child. No arbitrary model or user function is executed as a hook.

Every emitted chunk carries contiguous sequence and exact sample count. A final terminal record declares complete or truncated and totals; it does not repeat audio. The native playback bridge must retain at most its final20ms frame until that terminal record to set final-frame semantics, reject missing/out-of-order chunks and flush on cancellation or epoch change. This source work is unqualified: first-audio latency, seam quality, reference-voice fidelity and real playback are not established by API inspection or syntax compilation.

Source evidence: inspected qwen-tts0.1.1 in immutable image5c3d411b1825c673d8bd48089a4040c5a3ab7401029316d7ea475ae589cfcbfe without importing model libraries or loading GPU artifacts. Its wrapper prepends reference codes before decode and removes reference audio; the12Hz tokenizer's chunked decoder uses25-code left context and trims context times its upsample rate. Public [Qwen source](https://github.com/QwenLM/Qwen3-TTS) is useful API context; exact package hashes in the adapter are the compatibility boundary, not floating upstream main.

The private Rust receiver holds one lane permit from admission through the final validated terminal or cancellation. It binds every chunk/terminal to the configured model, unique worker request and private session/epoch; enforces 1920-sample chunks, contiguous sequence, strict PCM decoding and matching final totals; and bounds allocation to16KiB per reply. A cancelled read cannot resume a partially consumed frame. Drop sends exact worker cancellation while retaining admission until acknowledged or the full remote lifetime has passed. This private epoch is not the paired PC's playback epoch: the controller bridge in normal-speech.md preserves both identities and enforces accepted speech before starting synthesis. Its accepted producer remains gated on qualification.

Normal speech partitions a complete accepted response into at most64 private requests of at most512 UTF-8 bytes each, without altering its text or splitting words. The controller validates the entire partition before any synthesis and retains the original reply/output authority throughout. Each successor starts only after the preceding actual Complete terminal, uses a fresh private utterance ID, and performs current authorization immediately before writing. One64-codec-chunk queue, one fixed30-second synthesis budget and the cumulative30-second PCM cap apply across every segment. No intermediate terminal finalizes the public utterance or resets Digital/Human effects and background atmosphere. Every segment must complete for public Complete; truncation, cancellation and unknown/error termination cannot start another segment or be reported as a fully spoken answer. This extends byte-limited text support within existing aggregate limits, not the supported utterance duration. Actual seam quality and throughput remain unqualified.

Synthesis has a fixed30-second request budget, a bounded3-second connection/write and2-second inter-chunk idle limit after first audio. These are unqualified availability limits, not latency promises. Native playback needs its own fixed utterance deadline with startup/prebuffer time accounted for; a30-second waveform cannot be assumed to finish within30seconds measured from the earlier generation request. Native ingress must reject late audio against both clocks and cannot extend a deadline per chunk. Per-codec decoding with25 reference/context codes is source-derived; equivalence to the upstream300-code blocks and seam/voice quality remain unproven.
