# Controller admission and TTS buffer observations

These counters observe existing events only. Controller reasoning/audio admission
uses `try_acquire_owned`, so it has no waiting admission queue. Count the actual
attempt, busy refusal and closed-permit refusal at that operation; do not infer model/GPU utilization,
model waiting time, or backend retirement from a semaphore. Enumerated operations
identify the instrumented sites; uninstrumented health/voice-management calls are
not silently included. No extra inference, health request or deployment scan runs.

Normal speech has an actual 64-piece producer/consumer channel. A successful
capacity reservation is followed by one original enqueue Instant and a retained
ticket published with that exact piece. The ticket remains queued until actual
receive or drop. Dequeue records dwell; drop records discarded dwell separately.
Capacity-reservation wait has separate complete/failed/abandoned counts and times.
It is controller output backpressure, never model compute or admission waiting.
One bounded 65-entry timestamp roster exposes outstanding-ticket count and oldest
outstanding age: 64 real channel slots plus at most one consumer receive handoff.
A ticket starts immediately before publication and ends at the synchronous receive
handoff, so that roster is not exact instantaneous channel occupancy. The present
producer and consumer are polled together by try_join; the extra bound also covers
a future separate producer poll without losing a legitimate handoff observation.
No audio/text is copied into telemetry. The actual unchanged channel and original
source deadline still control buffering and cancellation.

The last actual queue/ticket owner records one terminal summary, linked only to
the existing genuine accepted turn/output. Caller withdrawal is not proof of
private TTS retirement. Pending wait/ticket ownership survives until actual drop;
missing/poisoned observer state counts as loss and never fails speech. At most
eight queue observers can be live. Queue snapshots show observations as of query,
not inferred continuously fresh engine state.

The existing five-second telemetry writer persists cumulative lane admission
counters by actual observation UTC day/process (not a repeated lifetime total), plus final queue summaries and fixed daily rollups.
Detailed queue summaries share the configured one-to-seven-day age ceiling and
an 8192-record cap; daily rows retain thirty days under an 8192-row cap. Queue
inspection/export is current-owner/device filtered, while lane counters are
explicitly controller-process scoped. Query/snapshot version 3 requires the typed
engine projection, and telemetry database schema 5 adds its separate tables. Voice,
speech, action and private service protocols do not change. Old peers reject the
new query; unavailable observation is not an empty-success queue.

Kernel/GPU timing, model token throughput, engine-internal queue depth, cold-load
time and profile-transition drain/freeing remain separate, unimplemented metrics.
Source/static checks alone do not establish runtime correctness or overhead.

Reasoning admission includes its existing metadata/tokenizer preparation and any
subsequent generation under the same permit; it is not a count of GPU jobs.
Audio load, infer and ASR/TTS/activity streaming entry points are instrumented.
A controller process can lose admission increments since its last five-second
checkpoint when it exits. UI marks that gap; collector starts and observer losses
remain visible. Fixed daily maps have at most 1,260 keys; dropping an unpersisted
key counts observer loss. The observer cannot retain or release a model permit.
