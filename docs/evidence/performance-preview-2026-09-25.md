# Native performance and task-view inspection — 2026-09-25

This follows the owner's [noninterrupting evidence procedure](../background-evidence.md).
It is actual production UI observation, not an automated test or qualified owner
workflow. The earlier [default/custom media](background-preview-2026-09-25.md)
remains intact with its original binary and source identity.

## Exact source and checks

Frozen archive `artifacts/performance-proof-20260925/source.tar` SHA256:
`5D3A2D503523089A55FADDF511A024D8EBCDC99F80963AE16A19E0F5487881FB`.
Independent source review passed after shortening metadata lock scope, taking the
finish timestamp before that lock, and rechecking original sample freshness after
instrumentation. Windows formatting/locked Clippy passed in 6.87s; extension
TypeScript and Svelte passed with zero errors/warnings. Full Windows release
passed in 3m06s. Shared core/server source is unchanged from the separately passed
C3 ARM checks; no redundant ARM build was run for this native/UI-only slice.

Installed canonical executable: 25,786,368 bytes, SHA256
`3B5B69FED9E14CC293BF4B6F3758BAB3970CEE3FBBB197789F7DF6A8FC17F9E4`;
bundled frontend `index-CJSYiFfH.js`. The prior proven executable is preserved in
`artifacts/background-proof-stable-20260925/proven-avesra-desktop.exe`.

## Actual silent inspection

No existing Avesra instance was running. The helper verified the normal physical
voice store and started exact PID 72804 on
`AvesraInspection-0cdd1598b81c4824a0f26bb6073f96bd`. Input desktop remained `Default`;
the debugger listened only on `127.0.0.1:9475`. This build includes the diagnostic
automatic-microphone suppression guard as well as unconditional hardware silencing.
No microphone, Hello prompt, grant, app effect or system-volume action was invoked.

The actual Performance tab first showed zero observations. From the real Audio &
Voice controls, Play text then submitted the unchanged default phrase. UI progressed
to **Final preview samples submitted to the output device**. Native recording
finalized with 238,080 stereo frames at 24 kHz, 9.92 seconds,
`termination: stream_disposed`, `hardware_silent: true`. Digital effects/music and
saved device selections stayed unchanged. Device enumeration had changed since the
earlier proof: headphones were now available, but the saved SPDIF endpoint remained
present. No new device selection or OS default change was performed.

The Performance tab then rendered six complete stage observations, zero failures,
zero missing values, zero capacity loss and zero remaining active output sessions:

| Actual measured stage | Time |
| --- | ---: |
| Admitted output session | 16,632.2 ms |
| Device/session/pairing preparation | 116.9 ms |
| Request to validated audio-ready response | 6,528.2 ms |
| Session to first submitted speech | 6,816.3 ms |
| Final frame to confirmed submission | 195.7 ms |
| Submission acknowledgment to estimated drain | 1,857.8 ms |

Each row correctly states one observation and provisional percentiles. These are
observed stage timings, not a latency qualification. The large pre-ready interval
exposed a concrete source bottleneck: `synthesized_pcm` collected the complete
utterance before `voice_preview::stream` sent Ready. Direct TTS first-chunk timing
was hidden behind this collector. A subsequent streaming correction requires its
own matching protocol build and actual before/after observation; this record does
not claim that correction has passed.

## Storage and read-only actions view

Before launch, consistent SQLite backup-API copies preserved both normal databases,
including WAL contents, under `artifacts/action-integration-proof-20260925/`:

- `native-actions-before-schema11.sqlite`, SHA256
  `d8769851a8404566ace6853a47230eccdf87631f679037c1132db2bbe9507d76`.
- `avesra-before-schema11.sqlite`, SHA256
  `8707a321a6dd04878c70153f90762d6323f18d389b929a1591b04758c8a61d83`.

Both application `schema_version` values were 10 before launch and 11 afterward;
both new permission tables had zero rows. SQLite `PRAGMA user_version` is unrelated
and remained zero. The real Observation & Actions page read the owner and durable
task projection successfully, listed actual outputs, showed protected setup and
reported no accepted action tasks. No grant was created and no effect was exercised.
This proves migration/startup and the empty read view, not action execution.

## Retained media

Local directory: `artifacts/e2e-20260925/performance-ui/`.

- `01-empty-performance.png`, `02-playing-default.png`, `03-default-complete.png`,
  `04-measured-performance.png`, `05-actual-action-status.png`,
  `06-first-speech-measurement.png`; genuine production WebView screenshots.
- `performance-observed.json`: actual rendered DOM text with the measurements.
- `native-preview.wav`, SHA256
  `08A8E0337879BEAFB496B95339404C521740AEB1E2BEED1911EEE292B04CD451`.
- `native-preview-proof.mp4`, SHA256
  `900977E1E25CE24678F9A9AF6CD27ADF54A7469DE462AEF4D09AF367321F4FAF`.
  It contains 167 timestamped real WebView frames and the matching native audio,
  offset 22.016130209s in a 35.184s sampled application video.
- Original frames, capture manifest, native sidecar, mux metadata and launch logs.

Only the verified owned PID was stopped after finalization. Automatic owner
recognition, accepted reasoning/actions, physical acoustic delivery, OW1-OW3 and
Plan001 completion remain unproven.
