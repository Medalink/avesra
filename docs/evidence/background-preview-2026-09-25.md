# Actual background preview evidence — 2026-09-25

The owner explicitly requested screenshots and audio/video without interruption,
then requested retaining the procedure for future proof. The reproducible manual
procedure is [background-evidence.md](../background-evidence.md). No automated test
suite, fixture, mocked UI, forged qualification or owner recording was used.

## Exact build and deployment

- Source baseline: `1704d2badf40f5521de372d162d2f420e6bdcb6c` plus the frozen dirty
  source in `artifacts/background-proof-stable-20260925/integrated-source.tar`.
  Archive SHA256: `F3EBD75CEDC35AAA95B6D3DF6C8B3295DEE040CCD372DEB03280D260C53977C1`.
- Windows static gate passed: workspace formatting/locked Clippy, extension
  TypeScript, Svelte zero errors and zero warnings. Full production build passed
  in 3m48s with one Cargo job, BelowNormal priority and the warm sibling target.
  The frozen snapshot used the existing identical locked dependency directories;
  pnpm automatic dependency reinstall was disabled for those junctions after its
  initial no-TTY reinstall attempt was rejected. No dependency/version changed.
- Canonical executable `target/release/avesra-desktop.exe`: 25,172,992 bytes,
  SHA256 `C9A8988A44C92A89EBF2C22F5EB8E52C807858C7EA7E465AF94BF621A2F2F839`.
  Bundled frontend: `index-CL7q0kPS.js`. No existing Avesra process was present
  when installing or starting these explicitly owned inspection instances.
- ARM static/release gates passed in 5.41s/34.70s using pinned compiler image
  `rust@sha256:8fa55b2f3ddf97471ab6a767bfa3f37e6bad0986ba823e75fea57e2a2a5c3073`,
  read-only source, two CPUs, 8 GiB and no GPU. Remote snapshot/log:
  `/home/medalink/.local/share/avesra-build/background-proof-integrated-20260925`.
- Deployed controller SHA256:
  `cf13e5cdbc18a31ac7e509b4559ad45207a983c7be493014c3c88519583db12b`,
  observed PID 1632653. The existing transient unit retains MemoryMax=1G,
  CPUQuota=200%, NoNewPrivileges=yes and the same private TLS/configuration store.
  Pinned-TLS health passed and still reports general voice unavailable/actions
  disabled. That honest general status is independent of explicit preview.

The first controller update exposed an operational mistake: stopping its transient
unit unloaded it, so a separate start failed. The exact Avesra unit was recreated
with its recorded properties; the subsequent update used atomic binary replacement
and `restart`, verified successfully. The runbook records the correction. Other
existing model services were not restarted.

## Live faults found and corrected

1. Models & Drivers had hard-coded ASR/TTS status rather than inspecting configured
   services. Actual private health showed ASR/speaker/TTS loaded. The new authenticated
   status route and native UI now show their real revisions and capabilities.
   `04-real-model-status-result.png` records Nemotron/ECAPA loaded but unqualified;
   the rendered DOM also reported Qwen3-TTS loaded, streaming advertised, idle.
2. The normal user's saved output ID ended in `276c2536-f564-4e6a-aef1-83a6a8e2d928`,
   which was absent from current enumeration. Settings explicitly showed **Selected
   output unavailable** (`05-stale-output-selection.png`). The actual SPDIF default
   endpoint ended in `da79a4fd-f332-4d13-9113-68aa797b0f02`; it was selected through
   the real Settings select/change handler, which confirmed preferences saved.
   No OS default device, system volume or other application's audio was changed.
3. Changing output briefly invalidated the voice-status session. The first refresh
   reported **Wait for the Spark session**; explicit Refresh status then succeeded
   and enabled Play text. The first sampled video containing only this preparation
   remains an unused capture, not successful playback evidence.

## Native default and typed-text proof

Both instances used the normal physical saved voice store. Launch logs report
`GetPackageFullName=15700`, distinct `AvesraInspection-*` desktops and unchanged
input desktop `Default`. Debugging listened only on `127.0.0.1:9475`.
No mouse/keyboard injection, input-desktop switch, foreground activation or
microphone operation was used. Automatic voice was unqualified/inactive.

The real Audio & Voice DOM showed Digital atmosphere **On**, voice volume 80%,
voice presence 85% and Galaxy/background 85%; these sound settings were preserved.
The test acts on the actual selected/active voice, independently of a different
unselected candidate still shown in the designer draft. No voice selection or
regeneration was performed.

| Recording | Actual UI action and observed result | Native output |
| --- | --- | --- |
| Default | Clicked Use default, observed exact 120-byte default text, clicked Play text; UI progressed through Synthesizing and playing to Final preview samples submitted to the output device | 240,000 stereo frames, 24 kHz, 10.00 s; finalized/stream_disposed |
| Custom | Entered `This is a custom voice test. I am speaking the words you typed, while the recording runs quietly in the background.` through the real textarea/input event, then clicked Play text; same actual completion state | 209,280 stereo frames, 24 kHz, 8.72 s; finalized/stream_disposed |

Every physical hardware buffer was deliberately zeroed by native diagnostic mode
after recording the real post-mixer/post-format samples. The files include the
actual native effects/music path; they **do not prove acoustic speaker output**.
The existing completion wording describes ordinary operation; the diagnostic
sidecar explicitly distinguishes these pre-silence samples. No claim of an
independent listening audition is made. Direct file inspection confirmed finalized
audio formats; the custom recording's observed peak was -1.3 dBFS.

Artifacts are retained locally under `artifacts/e2e-20260925/`:

| Artifact | SHA256 |
| --- | --- |
| `native-default.wav` | `28302D69E3E17A76981ED87CE3109F55DDA9B3AE85896D4107E2B5B04F419B45` |
| `native-custom.wav` | `7EA2E1E1ADD26D533D84EAF8A0CF89E66E45818024D49ED673840B4C85E28A56` |
| `native-default-proof.mp4` | `F43F42EBBA82687438A3C9801E0F43677E66B6AB47C81D9F72A42B009A0D2326` |
| `native-custom-proof.mp4` | `5D474F0ED9BF9685A916CE919BD9E38B82E525EC26BF6EDF33CA6A80F8C13471` |

The matching `.wav.json` sidecars identify output provenance/timing. Each video
contains 166 genuine WebView frames sampled near 5 fps, encoded at 10 fps with
repeated frames and matching native audio, aligned by recorded UTC first-sample
time. Audio offsets are 16.9247517586 s and 16.9281806946 s respectively. Originals,
frame timing JSON and mux metadata are retained. These are sampled application
videos, not whole-desktop captures.

Screenshots: `06-default-text-ready.png`, `07-default-text-playing.png`,
`08-default-text-result.png`, `09-default-complete-status.png`,
`10-custom-text-ready.png`, `11-custom-text-playing.png`,
`12-custom-text-result.png`, `13-custom-complete-status.png`. All were taken from
the real production WebView. The raw DOM completion observation supplements the
screenshots when the final status lies below the textarea viewport.

After sidecar finalization, only the exact owned diagnostic PIDs (56088 and 8620)
were stopped after rechecking executable path and capture argument. Final inventory
found zero Avesra processes and no listener on port 9475. Avesra remains closed for
the owner to launch normally.

## Remaining proof boundary

This establishes default/custom text preview through the actual UI, native bridge,
paired controller, deployed selected-voice TTS and native mixer. It does not
establish automatic listening, held-out owner recognition, acoustic echo/replay
rejection, accepted reasoning, PC actions, OW1-OW3 or full Plan 001 completion.
Source work after the frozen snapshot is not covered by these binary hashes or
recordings and needs its own applicable checks. Plan 001 remains IN PROGRESS.
