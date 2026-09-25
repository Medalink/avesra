# Submitted playback signal

The overlay consumes actual native playback references for its approved Ruby/red speaking ribbon. This is observation of post-gain, post-device-format samples submitted by CPAL, not evidence that a listener heard them, owner identity, echo qualification, successful synthesis, or accepted-task completion. Preview and accepted reply are explicit distinct purposes. A preview needs no enrolled speaker and must never imply an accepted conversation.

The output event lane is independent of microphone telemetry. Every sample and clear carries the exact playback epoch, immutable native output UUID and one process-monotonic, positive, JavaScript-safe sequence. Admission starts once per output epoch; clearing retires that identity. A clear received before its runtime snapshot also retains a monotonic retired-epoch tombstone. A delayed sample cannot reopen a retired identity, replace another UUID within the epoch, or clear a replacement. Mic mute/unmute, capture-only device changes and capture clears preserve output. Deafen, pause, Stop, lock, disconnect, output change, source/caller withdrawal and device failure withdraw it. Native publication and retirement serialize under media configuration ownership; no callback takes a Runtime/SQLite lock or emits IPC.

The media worker reduces only the reference's valid submitted samples to32 bounded values, at most20 sample events per second. It retains no waveform history. References older than250ms are discarded. A reference's original monotonic submission time gives a fixed250ms display expiry; receiving an event never renews it. Clear events are consumed even without clock calibration. Native sequence exhaustion disables this observation lane rather than wrapping or affecting effect authority.

`playback_signal_clock` is a read-only native clock sample using the exact reference telemetry origin. The frontend brackets it with `performance.now()` t0/t1 and uses t0 minus native time as a conservative offset. Samples expire at native submission plus250ms plus that offset, so IPC delay and calibration roundtrip consume the budget. Reject negative/over250ms roundtrips, missing/malformed calibration and calibration older than30seconds. Refresh periodically every10seconds and on explicit visibility recovery, one actual request at a time; no queued samples are replayed after recovery. Hiding the page clears display/calibration; visibility recovery requires a fresh sample. A50ms display sweep clears expired frames. Timers are advisory: every event and visibility transition rechecks monotonic expiry. A suspended event loop cannot provide real-time rendering guarantees; it must not revive expired output when execution resumes.

The approved references are design/README.md, design/tailwind/input.css, Overlay.dc.html (compact geometry and indicator precedence), and OverlayStates.dc.html (speaking state). Fresh submitted output has ribbon precedence over mic mute, while the microphone-off control and muted label remain active. Disconnect, lock, pause, deafen and missing selected speaker suppress output. Reply-purpose display also requires current enrolled/voice_ready; preview remains independent. Compact speaking has no extra visible label; expanded/accessibility copy uses plain preview/reply activity wording; it never asserts audible delivery or task completion. Settings' input meter receives capture only. Reduced motion freezes within the exact lane/epoch/output identity and resets on retirement; existing WebGL/CSS fallback stays in place. No simulated signal or fake task/history copy is introduced.

## Entry points and verification boundary

| Entry point | Decision |
| --- | --- |
| Native PlaybackReference consumer | Covered: valid post-gain samples, original timestamp, current output UUID/epoch/source/caller |
| MediaWorker open/publish/teardown/Drop | Covered: serialized identity admission and lane-scoped retirement |
| playback_signal_clock | Covered: read-only monotonic calibration; no device or permission mutation |
| App output event/runtime/visibility/expiry | Covered: exact identity, ordered retirement, conservative age, independent capture lane |
| Overlay/Signal/Settings meter | Covered: approved speaking geometry, purpose, reduced motion identity; capture-only meter |
| Preview and normal speech ingress | Unchanged: telemetry cannot admit audio or widen their separate authority |
| Echo/identity/planner/activity/history | Out of scope: samples do not qualify these lanes or create task state |

Implementation proceeds under the user's specs-and-code-only override: no tests or fixture harness. Static compilation/type checks and source/design comparison are the available checks. No device, model, UI or visual inspection is invoked while the gaming restriction remains. Normal accepted speech stays dormant until its separate qualification/producer boundary exists.
