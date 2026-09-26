# Submitted playback signal

The process-local Performance view additionally consumes the same validated
off-callback reference metadata for first submitted speech timing. It copies no
samples and adds no callback locks or allocation. See `performance.md` for bounded
retention, missing-reference handling and the distinction from audible delivery.

An explicit whole-gate assistant-playback trial also owns one native rejection
observation, separate from display events. The validated off-callback postmix
consumer records at most 2,048 nonzero speech submission intervals over twelve
seconds for one exact native output UUID and epoch. The probe exists before
capture and stores no samples. Only overlap with the completed endpoint's
original capture interval supplies conservative output-overlap rejection; it
cannot assert acoustic echo, clean audio, audible delivery or external replay
resistance. Webview telemetry is never accepted as evidence. See `turn-gating.md`.

Explicit native `--capture-output` launches are the diagnostic exception: references and waveform events describe the real post-format mix before hardware silencing, while every hardware buffer is zeroed. Such recordings/screenshots must be identified as silent diagnostic callback observations, never audible-device evidence. See `playback.md` for bounded recording, provenance and unchanged authority.

## Local microphone check

Audio & Voice provides an explicit Check microphone / Stop check control. Merely opening the page never starts capture. A check uses the existing native MediaWorker, selected endpoint, Capture timing/gates and capture signal event for at most30seconds. It requires visible Settings, an unlocked Windows session, a selected input and unmuted/unpaused/undeafened local controls; Spark pairing and voice enrollment are not prerequisites. It cannot overlap an enrollment session or qualified normal capture. It never sets voice readiness, sends audio to Spark, stores PCM or queues samples for inference.

The shared runtime carries transient microphone_check state. Stop, page departure, Settings hide/close, device/profile replacement, local controls, lock, failure and native deadline retire the exact capture epoch. A late page cleanup cannot stop a newer check. Native deadline and window visibility enforcement work even if the WebView stops responding. Check cancellation preserves independent playback.

All capture signal events include measured full-frame RMS and peak from the same AudioFrame used by enrollment, rather than deriving volume from averaged visualization samples. The input meter displays a bounded -60 to0dBFS scale, a quiet floor, clipping indication and explicit idle/checking/recording states. Stale events expire within500ms and must match the current epoch. Playback signals never feed the input meter.

Entry points: begin_microphone_check/stop_microphone_check, MediaWorker publish/capture consumer/take_capture_frame, LocalState capture controls, Settings hide/close, device selection, Audio & Voice component teardown, App signal validation and enrollment/normal capture callers. Shared worker/epoch paths cover these; speaker playback remains independent. Owner's no-automated-tests override applies; verify with static checks and explicitly authorized real device observations.

Plan 002 adds a `speech` boolean to each native sample event. The fixed reference pool retains actual post-format L/R samples on stereo endpoints and two equal copies of the replicated mono sample on other layouts, with the actual endpoint channel count. Display reduction uses stereo-energy RMS only for visualization. Lead-in and tail events set `speech` false and keep their original output identity/clock without asserting speech activity or retiring the output lease; the first speech-to-tail transition bypasses the ordinary sample throttling. The lease retires after actual final mix submission and estimated drain, or immediately on cancellation. These observations still do not qualify echo cancellation or audible delivery.

Every admitted output purpose, including saved-reference tests and generated-candidate previews, reveals the overlay on its first submitted speech frame. This is triggered by actual native output rather than clicking Play, waiting for generation, lead-in or background-only tails. The speaking bars consume the same fresh samples for previews and replies. Showing the overlay does not request focus, change microphone state or grant readiness. The overlay is revealed once per output; hiding it during that output is respected until the next utterance.

The overlay consumes actual native playback references for its approved Ruby speaking bars. This is observation of post-gain, post-device-format samples submitted by CPAL, not evidence that a listener heard them, owner identity, echo qualification, successful synthesis, or accepted-task completion. Preview and accepted reply are explicit distinct purposes. A preview needs no enrolled speaker and must never imply an accepted conversation.

The output event lane is independent of microphone telemetry. Every sample and clear carries the exact playback epoch, immutable native output UUID and one process-monotonic, positive, JavaScript-safe sequence. Admission starts once per output epoch; clearing retires that identity. A clear received before its runtime snapshot also retains a monotonic retired-epoch tombstone. A delayed sample cannot reopen a retired identity, replace another UUID within the epoch, or clear a replacement. Mic mute/unmute, capture-only device changes and capture clears preserve output. Deafen, pause, Stop, lock, disconnect, output change, source/caller withdrawal and device failure withdraw it. Native publication and retirement serialize under media configuration ownership; no callback takes a Runtime/SQLite lock or emits IPC.

The media worker reduces only the reference's valid submitted samples to32 bounded stereo-energy RMS values, at most20 sample events per second. Averaging squared channel samples avoids phase cancellation and signed-wave averaging hiding audible output; original playback references remain unchanged. It retains no waveform history. References older than250ms are discarded. A reference's original monotonic submission time gives a fixed250ms display expiry; receiving an event never renews it. Clear events are consumed even without clock calibration. Native sequence exhaustion disables this observation lane rather than wrapping or affecting effect authority.

Signal rendering follows every fresh frame: each bar's height comes from the current measured envelope, and the WebGL voice-bar renderer (`voice-bars.ts`) adds the approved pulse and transitions on animation frames between them. Neither invents samples nor extends the frame expiry. Hidden, cleared, expired or retired output returns the bars to the resting line. Reduced motion retains the existing frozen-per-output presentation and disables the pulse and transitions.

The speaking bars map measured energy through a bounded display curve, `1 - exp(-4 * energy)`, capped at 94% of their available height. This gives quieter phrases more visible movement without changing playback gain or turning silence into an invented signal. Capture displays use `min(1, (4 * |sample|)^0.6)`; thinking uses its own low, bounded scale.

`playback_signal_clock` is a read-only native clock sample using the exact reference telemetry origin. The frontend brackets it with `performance.now()` t0/t1 and uses t0 minus native time as a conservative offset. Samples expire at native submission plus250ms plus that offset, so IPC delay and calibration roundtrip consume the budget. Reject negative/over250ms roundtrips, missing/malformed calibration and calibration older than30seconds. Refresh periodically every10seconds and on explicit visibility recovery, one actual request at a time; no queued samples are replayed after recovery. Hiding the page clears display/calibration; visibility recovery requires a fresh sample. A50ms display sweep clears expired frames. Timers are advisory: every event and visibility transition rechecks monotonic expiry. A suspended event loop cannot provide real-time rendering guarantees; it must not revive expired output when execution resumes.

The approved references are design/README.md, design/tailwind/input.css, Overlay.dc.html (compact geometry and indicator precedence), and OverlayStates.dc.html (speaking state). Fresh submitted output has speaking-bar precedence over mic mute, while the microphone-off control and muted label remain active. Disconnect, lock, pause, deafen and missing selected speaker suppress output. Reply-purpose display also requires current enrolled/voice_ready; preview remains independent. Compact speaking has no extra visible label; expanded/accessibility copy uses plain preview/reply activity wording; it never asserts audible delivery or task completion. Settings' input meter receives capture only. Reduced motion freezes within the exact lane/epoch/output identity and resets on retirement. Without WebGL, plain CSS bars with the same geometry and colours take over. No simulated signal or fake task/history copy is introduced.

## Entry points and verification boundary

| Entry point | Decision |
| --- | --- |
| Native PlaybackReference consumer | Covered: valid post-gain samples, original timestamp, current output UUID/epoch/source/caller |
| MediaWorker open/publish/teardown/Drop | Covered: serialized identity admission and lane-scoped retirement |
| playback_signal_clock | Covered: read-only monotonic calibration; no device or permission mutation |
| App output event/runtime/visibility/expiry | Covered: exact identity, ordered retirement, conservative age, independent capture lane |
| Overlay/Signal/Settings meter | Covered: approved speaking geometry, purpose, reduced motion identity; capture-only meter |
| Preview and normal speech ingress | Unchanged: telemetry cannot admit audio or widen their separate authority |
| Native playback rejection observation | Exact owned submission intervals may reject an overlapping explicit held-out endpoint; no clean-audio assertion |
| Acoustic echo/identity/planner/activity/history | Samples do not qualify these lanes or create task state |

Implementation proceeds under the user's specs-and-code-only override: no tests or fixture harness. Static compilation/type checks and source/design comparison are the available checks. No device, model, UI or visual inspection is invoked while the gaming restriction remains. Normal accepted speech stays dormant until its separate qualification/producer boundary exists.
