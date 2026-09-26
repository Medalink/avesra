# Accepted-turn traces

Preacceptance voice measurements are promoted only by genuine durable acceptance;
see [voice-timing.md](voice-timing.md) for receipts, aggregate abstentions and limits.

Plan001 section16 governs this observer. Only genuine accepted turn IDs start
retained traces. Existing turn, request, private job, task and output IDs are
correlation, never admission. Native and controller independently retain host-
local monotonic spans, explicit operation-parent links, terminal error codes,
queue durations, cancellation/abandonment and retry counts. Missing deployment
metadata is unavailable; only observed model/image/config digests may be stored.
No text, arbitrary errors, account labels, prompts, media, embeddings, endpoints,
credentials or external URLs belong in this schema.

A bounded nonblocking observer queue feeds a separate telemetry SQLite owner.
Instrumentation failure does not affect work. Cumulative drop/storage-loss counters persist when the writer can next commit;
collector-start counts identify restart boundaries and possible unobserved crash
gaps. A store that cannot open is explicitly unavailable, never zero loss. Default trace retention is seven days, configurable
between one and seven days, with a32768-record hard cap. Fixed-stage daily rollups
retain thirty days under a bounded row cap. UTC dates serve retention only; each
span duration comes from one process monotonic clock. Wall-clock rollback never
removes the hard storage bound. Process UUIDs distinguish clock origins.

Settings reads current-owner/current-device records and can select one accepted
turn for drilldown. The paired controller endpoint checks the current native
registration and control session before and after its bounded telemetry read.
A disconnected controller is explicitly unavailable, never an empty-success
trace. Export writes only schema-approved records to a new native-selected file
under the private application directory, with the same owner checks. No frontend
path or arbitrary export text is accepted. Saved exports persist until the user
deletes them; trace retention is separate from immutable conversation history.

The export mapping is explicit: accepted turn UUID bytes are the128-bit W3C
trace ID; each observation has an independent random128-bit record ID whose
first64 bits are its span ID. The complete record ID remains an attribute for
lossless recovery. Existing operation UUID and parent-operation UUID are separate
link attributes, not invented OTel parentSpanIds. Cross-host causality uses explicit correlation attributes in the same trace;
no OTel links array or parentSpanId is invented without an actual span ID; it does not imply clock
synchronization. Native and controller UNIX observation timestamps are supplied
with explicit clock-offset-unavailable attributes. Measured monotonic duration
is preserved separately; no host timestamps are subtracted. The JSON export
contains the lossless native schema and an OTLP JSON resourceSpans projection;
missing true parent span identity stays absent rather than being guessed.

Terminal actual postmix submission is measured off-callback using its original
submission Instant. It proves native submission, not acoustic delivery. Driver
spans retain the actual private request UUID; model text cannot create links.
A cancelled waiter does not prove worker retirement. Observer spans distinguish
caller withdrawal, actual completion and missing terminal evidence.

Source/static checks do not prove A28. Runtime evidence must follow an actually
accepted conversation and action across both hosts and an exported drilldown,
including failure/cancellation and a content-canary inspection.
