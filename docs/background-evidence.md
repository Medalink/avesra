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

## Personal conversation and interruption evidence

The continuous conversation requirement added on 2026-09-26 needs a separate,
owner-operated observation in the ordinary application. The silent diagnostic
instance below deliberately disables automatic microphone capture; its preview
WAV, an operator reasoning greeting, or a successful build cannot establish
continuous listening, native acceptance, dialogue context, or interruption.
Do not remove those diagnostic guards to obtain a result. This procedure adds no
microphone recording automation, input-desktop control, injected transcript,
synthetic accepted turn, or automated acceptance harness.

Record the matched desktop/controller source and binary identities, current
reasoning incarnation, actual loaded audio lanes, selected stable input/output
endpoints, and whether Personal admission is learning or has a native voice
anchor. Keep identifiers in private evidence where appropriate; do not dump
credentials, embeddings, raw microphone audio or unrelated application content.
Personal operation is not release qualification. Its initial voice learning
must not learn the assistant's own playback as the owner.

The owner then performs these brief natural interactions on **both headphones
and room speakers**, noting which endpoint and sound preset were actually used:

1. Start ordinary Avesra and say a natural greeting or question. No exact wake
   phrase or calibration script is required for Personal conversation. Observe
   an actual native accepted turn, a correlated complete planner reply, and the
   matching accepted-reply output stream. A generated answer stored in history
   is not proof that output reached the callback or was heard.
2. Ask a follow-up that depends on the previous answer. Record whether the answer
   uses the recent dialogue correctly. The actual current-session native claim
   must select that context; replaying a preview or an old stored reply is not
   a second accepted turn. Context presently contains at most three completed
   pairs. The controller bounds its retained planner roster at 64 entries,
   compacting only retired actual owners; this is no longer a lifetime attempt
   limit. Durable native claim ordinals survive restart, and the control session
   keeps its admitted high-water mark so compaction cannot permit replay. Live
   or uncertain owners still consume bounded capacity. Do not report a long
   conversation or indefinite soak from this brief observation.
3. While Avesra is still speaking, interrupt naturally with a new question, then
   separately with one owner-chosen action that already has its explicit native
   grant. Do not grant or invoke an unrelated action merely to manufacture proof.
   Observe capture continuing during actual output, a distinct bounded utterance,
   and the new request's current native provenance. Capture continuity alone
   does not establish correct separation of the owner's speech from playback.
4. For interruption, distinguish immediate old-output gate closure from actual
   stream/device retirement and upstream job settlement. Record whether old
   speech stops, whether the new request is accepted only once, and whether the
   new answer/action starts under its own current ownership. A cancelled waiter
   or dropped task handle does not prove inference or an OS effect was cancelled.
   A committed action can remain an uncertain effect; interruption must not
   silently replay it. A bounded pending request must retain its original age
   and context rather than receive a fresh lifetime on dequeue.
5. Let Avesra finish speaking without interrupting. Note any self-trigger, false
   owner learning, repeated answer or unintended action. Repeat the interaction
   with the user's normal effects/background music enabled, since the reference
   must cover the actual full mix, including tails, rather than dry speech alone.
   Check mute, deafen, pause and lock separately through the owner's normal
   controls; neither listening nor output may survive its revoked permission.

For source/runtime review, submitted output references must retain exact
utterance/epoch, sample rate, continuity and original timing before buffers are
recycled. Microphone hardware capture time and output callback submission time
are different observations; account for actual scheduled playback time or an
explicit bounded latency search. Missing or discontinuous references are unknown,
not evidence of clean audio. Correlation or adaptive residual handling is a
provisional Personal signal-processing observation, not a measured acoustic echo
cancellation guarantee, biometric proof, or justification to label simultaneous
playback as `NoOutput`. Headphone success does not prove speaker-room behavior.

Retain the owner's observed success/failure and any existing bounded native
metadata needed to correlate the steps. Do not add plaintext/biometric diagnostic
logging for this procedure. If a boundary has no instrumentation, record it as
unobserved instead of inferring it from a UI animation. Existing silent background
screenshots/WAV/video procedures remain useful for their narrower output/UI
claims and remain unchanged below.

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

If Codex's filesystem view is redirected and no existing Explorer window is
available to launch the helper, a verified **local-only** fallback is CIM process
creation. Use an already reviewed wrapper that invokes `inspect-background.ps1`
with the exact executable, new WAV path and evidence logs:

```powershell
$startup = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ ShowWindow = [uint16]0 }
$created = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
  CommandLine = 'pwsh.exe -WindowStyle Hidden -NoProfile -File "E:\absolute\existing-wrapper.ps1"'
  ProcessStartupInformation = $startup
}
$created
```

Do not supply a remote computer/session or bypass the wrapper's guards. A success
return and helper PID alone are insufficient: its log must still verify the
physical normal voice store, native child package result `15700` (no MSIX
identity), a new non-input desktop and unchanged `Default` input desktop. This
fallback was observed on 2026-09-26 with helper PID `83428` and native inspection
PID `80956` in session 1; no UI opened on the input desktop. That launch used
source `dda8c65` and Windows executable SHA256
`37fa18ba3ff70342c2e385b20c6cd5ac0a405b47928d146f2ebc9ec9ded6e812`.
The launch checks establish isolation; the completed preview is recorded below.
This is only a launch
fallback for the already authorized diagnostic procedure, not general permission
to create background processes or modify the guards.

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

## Controller installation and update

The current persistent method is `scripts/install-spark.py`: stage the controller
with its binary and source hashes, then explicitly activate the owned
`avesra-controller.service` against the existing private configuration directory.
The installer preserves configuration and state, verifies pinned-TLS health, and
retains the previous verified executable for explicit rollback. See
[deployment lifecycle](deployment-lifecycle.md) for commands and ownership limits.
An earlier transient listener must be retired separately by its owner; the
installer does not find or stop unrelated processes.

Source `3958812` contains the warm activity/TTS cancellation changes; `8613e48`
adds the activity capture-age correction with identical audio Python source.
On 2026-09-26, the actual Windows `8613e48` diagnostic application demonstrated
preview cancellation followed by a successful fresh preview without reloading
TTS. Evidence is retained under
`E:/Dev/Avesra/artifacts/simple-conversation-20260926`:

- `cancel.wav` is the **completed default preview**, 9.52 seconds, despite its
  earlier chosen filename. It is not cancellation evidence.
- In a fresh diagnostic process, the 366-byte preview started at
  `06:47:22.825Z`; navigating to Models at `06:47:30.701Z` unmounted VoiceDesigner.
  `interrupted.wav` finalized at 7.58 seconds with `stream_disposed` and hardware
  silence. `tts-after-interrupt.json` reports loaded, idle TTS with the successful
  inference count unchanged at two; cancellation was not counted as completion.
- A fresh process started the resumed preview at `06:48:43.909Z`.
  `resumed.wav` finalized at 9.60 seconds with hardware silence; the actual UI
  returned idle without alerts. `tts-after-resumed.json` reports loaded, idle TTS,
  three successful inferences and a last inference duration of 6221.746 ms.
  No model reload occurred between the interrupted and resumed previews.

The WAV sidecars identify actual native postmix/postformat output before hardware
silencing. This establishes the observed TTS preview cancel/resume path, not
audible-device playback, activity mute/resume, live Personal conversation,
microphone acceptance or acoustic interruption. These cancel/resume artifacts
describe `8613e48`.

The merged frontend received a separate completed default preview in native
inspection PID `80956`, started at `06:55:38.479Z` on 2026-09-26. `merged.wav` and
its sidecar in the same artifact directory record 9.03 seconds, 216720 frames,
24 kHz stereo float32, finalized with `stream_disposed` and hardware silence.
`merged-complete.png` records the actual UI idle without alerts; the saved Razer
Seiren X input and SPDIF output remained selected. The build source was
`dda8c65` (`7ec4b0c` changes documentation only), with Windows executable SHA256
`37fa18ba3ff70342c2e385b20c6cd5ac0a405b47928d146f2ebc9ec9ded6e812`.
This completes the fresh merged-build preview observation with the same
diagnostic limits above; it does not establish live conversation.

### Historical transient-unit caveat (2026-09-25)

The earlier `avesra-controller-enrollment-preflight.service` was a transient user
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
