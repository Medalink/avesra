## Performance presentation contract

The Performance content follows the authoritative `design/mockups/SettingsMemory.dc.html`
import of `Settings.dc.html`: compact status strip, three equal summary cards,
36-pixel stage rows with Stage / Model · device / p50 / p95 / p99 / max / err,
and a concise footer. Existing shared design tokens supply typography and surfaces.
The mock's simulated data, passing gates, attribution and comparison result are
never product measurements. The Settings shell is outside this content change.

The first card displays only successful, retained PC `endpoint_response_submission`
attempts from the existing explicit accepted-trace read, only when every endpoint
record belongs to one actual process and one complete model/image/config identity.
Missing identity or multiple cohorts suppresses the card percentiles and gives a
reason and source count; no current-build/profile identity is inferred. Its label is therefore
“Endpoint → first submitted speech”, not speech-end/useful/audible reply. The
quiet tail is excluded; repeated attempts are not unique turns. Counts, failed or
missing outcomes, bounded query coverage and observer loss remain visible in the
trace details. There is no new protected read, automatic inference or Hello request.
The projection clears with its native view context. The local table, errors and
endpoint card also clear on Settings hide, lock and teardown; pending reads
cannot republish after withdrawal. The native hide listener is installed before
the initial read, and actual window focus/visibility reopening permits a fresh
read, without overlapping an earlier pending request. Verified-action and verified-
completion endpoint cards remain unavailable until those exact measurements exist.
No sample count is a release gate; all displayed tails are descriptive.

The local stage table retains actual preview/greeting/activity summaries, including
failure/abandonment durations and all original outcome/missing counts. This source
has no model deployment identity: the source column says unavailable model / PC
observer rather than borrowing current configuration. Empty, loading and error
states are distinct. App timing rows retain separate outcome and frontend page-
clock cohorts; no failed-fast attempt is merged with successful latency. Detailed
scope, retention, registration/loss, diagnostic mode and exports remain accessible
in disclosures. The real saved-comparison flow remains in accepted-trace details;
there is no simulated Compare button. No runtime or pixel-parity proof is implied.

# App timing foundation

Separate installation-scoped startup, UI and history-worker timings now use bounded durable storage and explicit local export; see [app-timing.md](app-timing.md). Coverage is partial and distinct from the voice aggregates below.

# Local observed performance

Cached PC/controller resource readings and their bounded retained export are now
part of the accepted-trace inspection surface; see [resource-observer.md](resource-observer.md).
They are host/process-scoped, never attributed to the selected accepted turn.

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

Only visible, unlocked native Settings may read this process-local snapshot.
Refreshing its in-memory aggregate performs no inference, playback, capture or
disk write. The separate accepted-trace/resource inspection and export surface
reads persisted telemetry, may compact retention during a query, and writes an
explicitly requested export. It still starts no inference, playback or capture.
Each view names its measured scope and retention and uses explicit refresh.

Instrumented entry points: preview::play_owned and play (admitted preview and
startup greeting); playback_signal::Telemetry::sample (validated off-callback
submission); voice_check_activity::Stream::run (explicit live check);
performance_snapshot/Performance.svelte (read-only projection). Instrumentation
is a source capability; only separately recorded runtime observations prove it ran.
Batch activity remains outside these older preview/activity timing aggregates.
Preacceptance voice outcomes and accepted reasoning/reply/action observations are
implemented by later observers with their own scope and retention, described below.
No new callback allocation/locking, retry, permissive fallback or device operation
is introduced. Under the owner override, verification uses source review and
coordinated static/build checks, with no tests, fixtures or harnesses.

Accepted-turn drilldown and redacted export are now a separate observer described
in [accepted tracing](accepted-tracing.md). It stores IDs and typed host-local
stage spans across native/controller/private-job/output owners; it does not turn
this older process-local preview aggregate into release performance evidence.
The accepted view displays retained per-host stage percentiles/counts/errors/max,
explicit unavailable queue/deployment fields, observer losses and collector starts.
Preacceptance capture/analysis/intent/gate timings and enum-only outcome totals are
source-integrated; actual controller ASR/speaker driver receipts are promoted only
after genuine durable acceptance, as specified in [voice-timing.md](voice-timing.md).
Host/process resource sampling and cached headroom projection are source-integrated
under [resource-observer.md](resource-observer.md). These implementations do not
establish runtime sampling, exported cross-host evidence, detector accuracy,
whole-companion overhead, complete engine counters, or baseline/candidate report
comparison. Those proof and implementation boundaries remain open as applicable;
the acceptance ledger records the exact verified artifact/run scope.

Actual controller admission attempts/refusals and the 64-piece TTS output channel
now have a separate [event-driven observer](engine-observer.md), visible in the
accepted-trace view/export. Received versus discarded dwell and complete, failed
or abandoned capacity-reservation waits remain distinct. No model waiting queue,
GPU compute time or token-throughput measurement is inferred from these events.
The source implementation still requires actual runtime/export evidence.

## Manual saved cohorts (report format 1)

Visible unlocked Settings may save 1–64 distinct accepted turns from a fresh,
current-owner/device trace query. Empty or missing native turns are rejected.
Scenario ID/version, cold/warm state and contention condition are explicit owner
annotations, not observed facts. Saving runs no scenario, inference or device work.
Reports contain the selected actual records and query coverage counters, never
transcripts, prompts, audio, credentials or imported measurement values. Query
process identity is separate from every record's original process. Historical
binary, profile and scenario provenance is unavailable; it is never filled from
the currently running app. A selected turn is not proof of an independent scenario
repetition, and unaccepted/ambient attempts are outside this retained cohort.

The native trace owner retains the original authority and 12-second deadline
through reading, filtering and publication. All report file work runs on a retained
blocking worker. A process writer lock serializes publication/deletion. Reports
are DPAPI protected and checked against the current native owner/device before
listing or use. At most 16 reports of 4 MiB protected bytes each are retained until
explicit deletion; capacity refuses new saves without silently evicting evidence.
Export writes a bounded redacted JSON file only on request. Deleting a saved
report does not delete source traces, accepted history or previously exported files.

A comparable claim requires matching declared scenario/version/temperature/condition and
same owner/device. Rows are grouped by host and stage; clocks are never subtracted
across hosts. Original process and deployment values remain visible. Missing
build/profile provenance, unknown annotations, unavailable controller, observer
loss/eviction/truncation, missing stage coverage, multiple process/deployment tuples
or missing deployment fields prevent a comparable claim. Cumulative query losses
are conservatively reported as possible coverage gaps, not attributed to the
selected turns. An apparently complete query is not proof of a complete corpus.

Each row separates selected unique turns, observed unique turns, span attempts and
outcome counts. Absent stage records are missing coverage, never zero duration.
All-outcome elapsed percentiles preserve failure/withdrawal/uncertainty/abandonment.
The endpoint-response stage separately summarizes successful post-quiet-endpoint
submission latency and non-success elapsed time; it retains the existing up-to-20ms
reference-block and non-acoustic limitations. Fewer than 30 timed unique accepted
turns is provisional, not 30 independent measured scenario runs. A comparison
shows descriptive measurements only: no speed-only winner, accuracy pass, promotion,
qualification or automatic rollback. Genuine matched manual repetitions and their
correctness evidence remain A29 release work.

Saved report envelopes also bind the actual protected owner revision and paired server fingerprint. The sixteen-slot bound covers the local report store, including reports hidden by a changed owner or pairing; those records are never exposed to a different binding. Percentiles weight actual span attempts, not unique turns; repeated stage attempts are disclosed. The native endpoint-submission row remains visible when neither cohort has that stage, with unavailable duration and all selected turns missing. This format is descriptive only because historical build/profile provenance is unavailable; it does not complete A29 qualification.

A separately confirmed **Delete inaccessible saved reports** operation exposes only the count and deletes old-binding report files under the current protected Windows owner. It preserves current-binding reports and rechecks the original owner/deadline before each deletion; partial cleanup errors require refreshing. Exports preserve redacted owner-revision/server-binding facts, write a single bounded temporary file, recheck authority before atomic publication, and remove a newly created export if the final check fails. No inaccessible record IDs or annotations are returned.

The thirty-unique-turn provisional label is only a minimum coverage warning. Removing that warning does not qualify tail percentiles: p95/p99 remain descriptive attempt-level observations without independent tail-accuracy evidence, even when at least thirty selected turns exist.
