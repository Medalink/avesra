# Actual streaming preview correction — 2026-09-25

The [prior measured native run](performance-preview-2026-09-25.md) exposed a
6,528.2-ms wait for Ready and 6,816.3-ms wait for first submitted speech. The server
collected the full synthesized utterance before sending Ready, hiding the private
TTS service's early chunks. Passing builds had not revealed this behavior.

## Correction and exact verification

Preview wire version 2 distinguishes fixed-length saved references from bounded
unknown-length synthesis. Typed text and startup greetings can send Ready after
their first validated TTS chunk. A bounded queue preserves backpressure and the
original synthesis lifetime; exact Complete totals are required before the held
final frame and End. Failure after streaming begins may leave an already-spoken
prefix; it must not report successful completion. Effects/music and the native
output ownership path are preserved.

Independent source review found no remaining actionable issues. Frozen archive:
`artifacts/streaming-preview-proof-20260925/source.tar`, SHA256
`29CA2D506A36124802E215A8ADC73AA90E0E022C05B9976831B0FD72B3515BA5`.
Windows formatting/locked Clippy passed in 12.34s; extension TypeScript and Svelte
passed with zero errors/warnings; full release passed in 3m57s. ARM static/release
passed in 5.14s/36.56s using the same pinned compiler and bounded resources as the
earlier evidence. No automated tests or fabricated qualification were used.
Final comparison of primary application/crate/service source against the verified
snapshot found no substantive differences; 33 unchanged files differed only in
CRLF/LF endings. The final working diff passed the whitespace check.

Installed matching binaries:

- Canonical desktop: 25,872,384 bytes, SHA256
  `20BB7DA12B8DDD7E1CA9B608135D654FF3F1331FCC2FED5F99885F41109FF8D4`.
- Existing controller path: SHA256
  `7b144ddf84dbd2226b68971b41c0822fc93c33922ccb642a35911dfe5cbc7ec7`.
  Atomic replacement and restart retained the transient unit, same TLS/config
  store, MemoryMax=1G, CPUQuota=200% and NoNewPrivileges=yes; observed PID 1828953.
  Installed-path hash and pinned-certificate health passed. The earlier binary
  and unit definition are preserved. No other model service was restarted.

General health remains voice unavailable/actions disabled. This preview correction
does not construct owner qualification or activate automatic acceptance.

## Two actual native UI runs

Following [the saved procedure](../background-evidence.md), each recording used a
fresh explicitly owned production instance on a separate `AvesraInspection-*`
desktop. Launch logs verified the normal physical voice store, no package identity,
and unchanged `Default` input desktop. Native diagnostic mode suppressed automatic
microphone capture and startup greeting, recorded post-mixer samples, and zeroed
physical output. No owner input, microphone, Hello, grant or PC effect was invoked.

The default run clicked the actual **Play text** control with the default 120-byte
phrase. The custom run entered the following text through the real textarea/input
event, then clicked the same button:

> This is a custom voice test. I am speaking the words you typed, while the recording runs quietly in the background.

Both UI runs reached **Final preview samples submitted to the output device**.
Both native WAVs finalized with `stream_disposed` and `hardware_silent: true`.
Digital atmosphere, voice volume 80%, voice presence 85%, background 85% and the
existing valid SPDIF selection were preserved; no voice was regenerated or selected.

| Observed stage | Previous default | Streaming default | Streaming custom |
| --- | ---: | ---: | ---: |
| Preparation | 116.9 ms | 97.0 ms | 104.6 ms |
| Request to validated Ready | 6,528.2 ms | 134.2 ms | 126.7 ms |
| Session to first submitted speech | 6,816.3 ms | 397.0 ms | 407.6 ms |
| Complete admitted output session | 16,632.2 ms | 10,301.5 ms | 8,703.5 ms |
| Final submission acknowledgment | 195.7 ms | 184.4 ms | 190.9 ms |
| Estimated drain | 1,857.8 ms | 1,871.8 ms | 1,866.9 ms |
| Recorded mixed output | 9.92 s | 10.00 s | 8.39 s |

Each new process showed six complete stage observations, zero failures/missing
timings/capacity losses and zero active output sessions afterward. These are two
individual observations with different texts, not population percentiles, acoustic
latency proof, measured underrun counts or a release-latency qualification.

Actual Models & Drivers now reports configured ASR/speaker/TTS revisions and loaded
but unqualified status. The current ASR/speaker services advertise batch-only;
TTS advertises streaming. Reasoning status is not inspected here; screen,
decision and memory drivers remain explicitly not integrated. Setup alone cannot
turn those unfinished integrations into a working automatic assistant.

## Retained artifacts

Directory: `artifacts/e2e-20260925/streaming-preview/`.

| Artifact | SHA256 |
| --- | --- |
| `native-default.wav` | `07662BBBCD980762D4D8B462D5DB4B12ED90B4244A1AD89D5973D899D6C5FD8A` |
| `native-custom.wav` | `F27E00275DEC74326DC3B1939BA7810BE8A719A11725E6B8887705D0F965889B` |
| `default-proof.mp4` | `7F6EE53B2BA25CED967C12CBFDD3CFBED55A49D0034253F8D89E8140BFF8DDC9` |
| `custom-proof.mp4` | `03BF2E894651679EEF81B1AC5A251D17F5D8E62725EAFCF69770329A53EE0E96` |

WAVs contain 240,000 and 201,360 stereo float32 frames at 24 kHz. Each video uses
142 genuine timestamped WebView frames, encoded at 10 fps with repeated sampled
frames, and its matching native audio. Audio offsets are 16.422337770s and
15.571226835s; video durations are 30.024s and 30.016s. Original frames, capture
manifests, WAV sidecars, mux metadata and launch logs are preserved.

Screenshots `01`–`10` show actual default/custom UI and performance states;
`11-real-model-status.png` and `model-status.txt` preserve actual model status.
`default-performance.txt` and `custom-performance.txt` preserve rendered timings.
Only exact owned PIDs 60608 and 83860 were stopped after finalization. The app was
left closed and the inspection listener removed.

This proves the default/editable preview from production Settings through paired
controller, real TTS chunks and native effects/music output. It does not prove
physical acoustic output, automatic owner recognition, accepted reasoning/actions,
OW1-OW3, calibration quality or full Plan001 completion. No independent listening
audition is claimed. Source calibration additions in this build passed review and
static/build checks, but were not invoked with an owner microphone or labels.
