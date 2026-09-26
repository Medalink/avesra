# Background evidence without interrupting the owner

This is a manual operating procedure for the owner's explicitly requested
screenshots and audio/video evidence. It is not an automated acceptance suite.
The owner authorized isolated background inspection on 2026-09-25 and asked that
the procedure be retained for future work. Do not interpret this as permission
to control their active desktop, capture their microphone or record other apps.

## Evidence boundaries

| Artifact | What it establishes | What it does not establish |
| --- | --- | --- |
| Production Settings WebView screenshot | The actual bundled UI rendered the observed state through the native bridge | Foreground keyboard/mouse interaction or an owner workflow |
| Native diagnostic WAV plus JSON sidecar | The actual selected output stream rendered post-format voice/effects/music samples before diagnostic hardware silencing | Sound from the physical speakers or microphone/owner qualification |
| Timestamped WebView screenshot sequence | Visible application state over the recorded interval | A full-desktop recording; label this as sampled application video |
| Deployed TTS service WAV | Actual generated speech and terminal completion from that service | Native UI, native effects/music or WASAPI rendering |
| ASR transcript of a generated recording | Actual inference by the configured ASR deployment for that input | Live-owner recognition, microphone quality, replay resistance or acceptance percentages |

No successful build, capture or metadata response closes Plan 001. A01-A29 and
OW1-OW3 still require their specific evidence. Record failures as failures.

## Preconditions

1. Record `git rev-parse HEAD`, dirty source inventory, source snapshot identity,
   build/static results, executable SHA256, controller SHA256 and UTC times.
2. Use a build containing native `--capture-output` support and its early
   initialization, unconditional hardware silencing, startup-greeting suppression
   diagnostic single-instance rejection, and suppression of the normal automatic
   microphone worker. Explicit microphone checks are outside this procedure.
   **Never pass this option to an older
   binary**: an unrecognized option could leave its normal audible behavior active.
3. Check `Get-Process avesra-desktop -ErrorAction SilentlyContinue`. If the owner
   already has Avesra open, do not stop it or use its UI. The inspection launcher
   also refuses an existing process and an occupied local inspection port.
4. Keep the normal user's existing pairing, settings, owner and voice stores.
   Verify the physical `AppData\Roaming\com.avesra.desktop\speaker-candidates`
   directory. Codex/MSIX filesystem redirection previously made saved enrollment
   appear absent. Do not create replacement enrollment to compensate for a wrong
   process filesystem view. The launcher fails on a redirected physical path.
5. Build with one Cargo job and BelowNormal priority while the owner is gaming.
   Use an independent target directory if the canonical executable is in use;
   never close the owner's application to release a build lock.
6. Before inspecting a build that changes persistent schemas, preserve consistent
   local database backups using SQLite's backup API while the owner app is closed.
   Include the WAL through that API; copying only a live `.db` file can omit saved
   data. Read this application's version from its `schema_version` table, not
   SQLite's unrelated `PRAGMA user_version`. Keep backups private in ignored local
   artifacts. Never roll back a database that has received later owner changes.

## Start an isolated, silent production instance

The checked-in `scripts/inspect-background.ps1` creates a separate Win32 desktop
with `CreateDesktopW` and starts the real release executable on it. It never calls
`SwitchDesktop`, `SendInput` or foreground-window APIs. Settings remains a real
visible native window on that non-input desktop, so native visible-Settings
permission checks remain in force. Do not replace it with a mocked browser page
or bypass the checks by invoking private Rust functions.

Run the helper as a hidden PowerShell process, with an absolute **new** WAV path
under an owner-authorized local evidence directory. Example after verifying the
current build supports the option:

```powershell
$evidence = Join-Path 'E:\Dev\Avesra\artifacts' ("background-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $evidence | Out-Null
$arguments = @(
  '-NoProfile', '-File', 'E:\Dev\Avesra\scripts\inspect-background.ps1',
  '-Executable', 'E:\Dev\Avesra\target\release\avesra-desktop.exe',
  '-OutputWav', "$evidence\native-default.wav"
)
Start-Process pwsh -WindowStyle Hidden -ArgumentList $arguments `
  -RedirectStandardOutput "$evidence\native-default-launch.log" `
  -RedirectStandardError "$evidence\native-default-launch-error.log" -PassThru
```

The helper logs the exact child PID and package-identity check. Record that PID;
only that explicitly owned inspection instance may be closed afterward. Do not
kill every process named Avesra. No microphone operation is part of this procedure.

The native recorder writes a new float32 stereo WAV at the selected device rate
and `<path>.json`. It captures only the first opened output stream, starting with
its first nonzero mixed sample. It has a 120-second no-signal wait and a maximum
120-second recording, and finalizes on stream disposal. **The process remains
hardware-silent after recording ends or fails.** Automatic startup greeting is
suppressed in this explicit diagnostic mode so it does not consume the recording.
The current source also suppresses the normal automatic voice worker before it
can open a capture window. The initial recordings below predate that additional
guard and were taken while automatic voice was unqualified/inactive; future runs
must use a build that includes the guard, even if the owner later qualifies voice.
Use a new diagnostic process/file for a second preview.

## Inspect the real WebView manually

The launcher enables WebView2 debugging only on local port 9475. Verify the
listener belongs to the launched process tree and is loopback-only. Do not expose
the port to the LAN. `scripts/inspect-webview.mjs` accepts exactly one production
Settings target at `http://tauri.localhost/index.html?window=settings`.

Read the current DOM before choosing controls:

```powershell
node scripts/inspect-webview.mjs 'document.body.innerText'
node scripts/inspect-webview.mjs `
  'Array.from(document.querySelectorAll("button")).map(b => ({text:b.textContent,disabled:b.disabled}))'
```

Perform one action at a time against a control observed in that current DOM,
then inspect again. Use DOM events on the isolated WebView; never move the system
pointer or call keyboard injection/foreground APIs. For a typed-text check, use
the actual textarea and its normal input event, then click its actual Play text
button. Do not invoke `preview_voice` directly and claim the UI was exercised.
Do not replace application state, inject successful replies, or weaken authority.

Capture a genuine WebView image alongside observed status:

```powershell
node scripts/inspect-webview.mjs 'document.body.innerText' `
  "$evidence\native-default-complete.png"
```

For video, retain original timestamped WebView frames and recording metadata.
Align video to `first_sample_utc_unix_ns` or the recorded process-monotonic time.
Label a slideshow/sampled sequence accurately; do not present a static screenshot
with unrelated service audio as a synchronized recording of a native interaction.

The checked-in capture and assembly commands are:

```powershell
node scripts/record-webview.mjs "$evidence\default-frames" 35
# While that recording is running, perform the individually inspected UI actions.
# After both frame capture and the matching native WAV sidecar finalize:
python scripts/assemble-evidence.py `
  "$evidence\default-frames" `
  "$evidence\native-default.wav" `
  "$evidence\default-proof.mp4"
```

Capture samples the actual WebView approximately every 200 ms. Assembly preserves
the measured frame intervals and delays the matching native audio by its measured
UTC offset; it refuses incomplete recordings or video that does not cover the full
audio interval. The encoded 10 fps video repeats sampled frames; it is not a 10 fps
desktop capture. These tools do not click, type, fabricate state or declare a test
passed. Preserve their source files alongside evidence, because media alone is not
enough to reproduce the procedure.

## Inspect the artifacts before claiming success

- Require the UI's actual final state, not just an invoked command or busy state.
- Require finalized WAV metadata and a nonzero sample count. Queue overflow,
  writer failure, incomplete sidecar, missing terminal or truncated output are
  incomplete evidence. Preserve the failure record when retrying with a new name.
- Inspect the screenshots and media; record the exact default/custom text and
  selected sound mode. Effects/music evidence needs native mixed output, not a
  clean service WAV. Do not claim an unlistened recording has been auditioned.
- Record device format, duration, termination, SHA256 and provenance. Silence at
  the hardware buffer is intentional and must remain explicit in the report.
- Close only the recorded inspection PID after output/sidecar finalization.
  Confirm its debugging listener is gone. Leave the user's ordinary apps alone.

## Controller update caveat

The existing `avesra-controller-enrollment-preflight.service` is a transient user
unit. A separate `systemctl stop` can unload it, making a later `start` fail with
"Unit not found." Preserve its full unit properties before an update. Install a
verified binary by an atomic same-directory rename and use `systemctl --user
restart` to preserve the transient unit. Keep the previous binary for rollback.
If the unit was already unloaded, its observed setup uses the existing private
configuration directory with `MemoryMax=1G`, `CPUQuota=200%` and
`NoNewPrivileges=yes`; recreate only that Avesra unit, then verify pinned-TLS health
and the actual process binary hash. Do not restart LocalStudio or audio models as
part of a controller-only update.

## Recorded evidence as of 2026-09-25

### Building from frozen snapshots

Archive the coherent source before verification and retain its SHA256, output
hashes and logs. When several extracted snapshots share a Cargo target directory,
refresh the frozen snapshot's build-input modification times after extraction
and before its first static/build command. Archive restoration and `Copy-Item`
can preserve times older than the cached crate outputs. A directly observed
failure on2026-09-25 reused an older core crate and reported a missing `Report`
type that existed in the new source. After refreshing input times, Cargo rebuilt
the actual core and exposed a real invalid error-enum variant, which was fixed.
Do not diagnose this cache result as proof that the new source was compiled.
Changing modification times does not change the archived source content.

### Retaining failed model startup evidence

When an owned deployment controller removes a failed container, attach a bounded
log reader to its exact verified container ID before releasing its startup guard.
Retain the initial engine log as well as the tail: a controller response that
keeps only60lines/4KiB can omit the real engine exception and leave only the parent
process traceback. This occurred during the owned reasoning deployment.

Record the image/container identity, resource limits and actual cgroup memory
events alongside the logs. Redact credentials across complete records before
writing; bound output, duration and the logging process lifetime. Capture failure
must remain visible. Never raise resource limits or restart unrelated models just
to obtain a green health response. A captured initialization exception is failure
evidence, not a completed inference or accepted conversation.

Use one Cargo job and BelowNormal process priority on Windows. Retain warm
dependency outputs, exclude dependency/build junctions from source archives,
and verify that changed workspace crates really recompile. For Spark, retain
the pinned compiler image and explicit CPU/memory limits; do not overlap a build
with model loading when available headroom is uncertain. These are build checks,
not acceptance tests or substitutes for the actual application recordings.

Latest matching deployment and actual media:
[streaming preview correction](evidence/streaming-preview-2026-09-25.md).
The real Performance view exposed a whole-utterance buffer that delayed first
speech to 6.82 seconds despite early private TTS chunks. After correction, fresh
default/custom UI runs observed 397.0/407.6ms and complete mixed output. Always
measure the actual native route; service-only first-chunk timing does not prove
the user-facing start time. Earlier recordings below remain valid for their own
recorded revisions.

The initial live service recordings are under `artifacts/e2e-20260925/`:
`default-service.wav` (7.68 seconds, 96 chunks) and `custom-service.wav`
(6.24 seconds, 78 chunks), with `service-audio-evidence.json`. Both received exact
complete terminals. ASR transcribed the generated custom recording in 0.701
seconds; `asr-generated-speech-evidence.json` preserves its actual reply and
scope. These are service proofs only.

The subsequent isolated production runs actually completed the default and custom
text previews through their rendered Settings controls. Native WAVs are 10.00 and
8.72 seconds, respectively, stereo 24 kHz float32; both sidecars finalized with
`stream_disposed` and `hardware_silent: true`. Synchronized sampled videos are
`native-default-proof.mp4` and `native-custom-proof.mp4`. Both launch logs prove a
distinct `AvesraInspection-*` desktop and an unchanged `Default` input desktop.
The exact owned processes were stopped after finalization; the local debugging
listener was then absent. See [the detailed evidence record](evidence/background-preview-2026-09-25.md)
for hashes, screenshots, actual faults discovered and the remaining boundaries.
