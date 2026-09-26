# Installed People and portrait timing observations — 2026-09-26

This record covers installed source `49cf90cf43323df4c640f28879ad63383a6ba5e0`,
with product Store 30, telemetry 8 and accepted trace query 3. It records actual
UI reads and timing export, not successful portrait creation, owner recognition,
acoustic playback, release latency or Plan003 completion. No application session,
test or build was run to write this document.

## Retained identity and evidence

Artifacts are under `E:/Dev/Avesra/artifacts/ui-parity-49cf90c/`.
`build-receipt.json` records passing Windows Static/release, ARM Static/release and
package verification, with no tests. Its deployment flag describes the packaging
step; the later `installed-native-launch.log` describes the inspected application.
`root-package-verification.json` agrees with the source and Windows package hashes.
These retained files were rehashed while documenting the evidence:

| File | SHA-256 |
| --- | --- |
| `source.tar` | `bd4c5e44c5b811c1a169d50e1bc680ef4427f1dfecb8cbd1780d34d39a6caf8c` |
| `package/avesra-desktop.exe` | `a3aa5a9ab471864d1525209130ec7bb6fd5b06d61f7f21ee8a197eaaa86ce040` |
| `package/avesra-native-host.exe` | `0526ed7a99ce6469cad8ea2f4bdf40eac7df0c5ac59a279b458c13f41adf1f81` |
| `package-49cf90c-schema30.zip` | `00eb413e93601c78794dc9f611ea24d69859227e178513b617a8d989040eac33` |
| `avesra-server-arm64` | `74458a839b0e22bc12ea98a499114c96be8f9d034519b91b4f3412ad8dff4d68` |

The launch log identifies owned PID 72128, no package identity, and non-input
desktop `AvesraInspection-b9f41441c1074323bd6b8ada85efb1d1`; the input desktop
remained `Default`. The UI explicitly reported microphone capture disabled for
background inspection. The inspection operator subsequently verified the
canonical executable path and parent PID 85408 and closed only owned PID 72128.
No other application was stopped. This closure is operator-reported; the retained
launch log is not a termination receipt.

## Observed behavior and limits

- `people-advanced-initial.png` and its DOM sidecar show an actual owner read
  failure, **Owner management is busy**, and an unavailable avatar. The saved
  six-phrase candidate was separately reported intact; that did not establish a
  current Personal avatar source.
- The real **Retry owner check** recovered the saved owner label **Eric**.
  `people-after-retry.png` and its DOM sidecar still show the avatar unavailable:
  the current owner/microphone needed saved real Personal voice observations.
  No populated portrait, redraw preview or capture was proved.
- `portrait-timings-installed.png`, its DOM sidecar and
  `app-timings-export.json` contain native `prepare` **4.438 ms** and
  `source_load` **4.084 ms**, each marked `complete`. These are completed local
  lookup phases that returned an unavailable source, not successful avatar
  derivation/publication. They overlap and must not be summed. Each is one
  observation, not a benchmark or qualified percentile.
- Filtering that export to its current native process yields **131 records:
  127 Settings frontend, 4 native, 0 Overlay**. Native rows are preparation,
  source lookup, startup and settings-store work. Zero exported frontend loss
  does not establish that an unregistered page had no losses. Settings collector
  status does not report Overlay's registration, and the actual reason for the
  missing Overlay rows was not observed.
- `final-audio-preferences-dom.json` shows the selected Razer Seiren X microphone,
  Realtek USB2.0 SPDIF output, voice volume 100%, Digital atmosphere with presence
  and background at 85%, and both chimes at 15%. The operator reported selected
  preferences unchanged. No Windows Hello, owner recording or protected
  management write was performed; the explicit timing export is diagnostic
  output. `installed.wav` is empty and its sidecar says `armed_or_incomplete`,
  so it supplies no audio completion or acoustic proof.

Later source changes, including serialized People reads and timing registration
before webview creation, require a new installed observation. This schema-8 run
does not validate the optional portrait capture UI/native observer or telemetry 9.

## Future manual repetition

Follow [the noninterrupting evidence procedure](../background-evidence.md) with a
fresh source/build identity and an owned diagnostic process. Keep the input
desktop unchanged, microphone disabled and physical output silent. Do not reuse
another running application's session or inject direct private IPC.

1. Open actual People controls, retain the initial card and Advanced state, and
   record any read error before using a visible Retry control. Distinguish the
   owner account, saved candidate and current Personal source.
2. Hide and reopen Settings through its actual controls; revisit People and
   Advanced. Observe whether reads recover without an extra click and whether
   old state is withdrawn. Do not unlock, record, redraw or save protected data.
3. Use the actual App timings **Read timings** and **Export** controls. Separate
   current-process Settings, Overlay and native records, and retain the original
   outcomes. An empty cohort is missing evidence, not a zero-duration result.
4. Recheck selected preferences, retain screenshots/DOM and export metadata, then
   verify and close only the owned diagnostic process. Real owner speech and
   acoustic proof require separately authorized ordinary application use.
