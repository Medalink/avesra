# Local observed performance

This first instrumentation slice measures actual admitted preview/greeting output
sessions and explicit streaming activity checks. It grants no permission, starts
no device/model work, and cannot qualify owner recognition or an accepted request.
The process-local Performance view is not the complete Plan001 tracing, durable
retention, deployment comparison or release-latency capability.

Keep at most 512 terminal observations and eight pending output correlations in
memory. Use fixed operation/stage/outcome enums, opaque request/output UUIDs and
host-local monotonic durations only. Do not retain text, free-form errors, endpoint
IDs, waveform values, transcripts, embeddings, scores or credentials. Eviction
and observer-capacity loss are visible; process restart clears this history.
Instrumentation failure never changes admission, cancellation or output behavior.

An admitted output session begins after the existing native output reservation.
Preparation covers device/session readiness and paired configuration loading;
remote-ready covers socket connection, request send and validated Ready receipt,
so it is not pure server inference time. Final submission and estimated drain
remain separately measured. First submitted speech comes only from the existing
fresh, exact-output PlaybackReference on the media worker, outside the callback.
It uses that reference's original submission timestamp. Missing references remain
missing, never zero. Silent diagnostic recordings are explicitly identified and
neither ordinary nor diagnostic submission proves acoustic delivery.

Activity measurements cover the explicit capture/response session, individual
correlated packet exchanges and original oldest-sample capture age at send. They
do not measure detector accuracy, endpointing or pure model inference latency.
They retain failed sessions and stale age observations. No generated sample is
relabeled as owner qualification.

Owned spans record terminal success/failure and known caller withdrawal; a dropped
unfinished span records abandonment. Observed duration on failure is time to that
failure, not successful latency. A bounded native snapshot computes nearest-rank
p50/p95/p99/max over all retained observations with a measured duration, including
failed/abandoned observations, and reports each outcome and missing-duration count.
Fewer than 30 timed observations are explicitly provisional. Values from different
hosts, operations or stages are never merged or subtracted.

Only visible, unlocked native Settings may read this snapshot. Opening or refreshing
Performance performs no inference, playback, capture or disk write. The view names
the measured scope and process-only retention and uses explicit refresh.

Instrumented entry points: preview::play_owned and play (admitted preview and
startup greeting); playback_signal::Telemetry::sample (validated off-callback
submission); voice_check_activity::Stream::run (explicit live check);
performance_snapshot/Performance.svelte (read-only projection). Instrumentation
is a source capability; only separately recorded runtime observations prove it ran.
Rejected
pre-admission attempts, batch activity, accepted reasoning/reply/action stages,
server internals, persistence/export and cross-host tracing remain out of scope.
No new callback allocation/locking, retry, permissive fallback or device operation
is introduced. Under the owner override, verification uses source review and
coordinated static/build checks, with no tests, fixtures or harnesses.
