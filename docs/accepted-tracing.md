# Accepted-turn traces

Trace query/snapshot version 3 also carries separate typed controller admission/TTS queue ([contract](engine-observer.md)) and cached host resource
observations; see [resource-observer.md](resource-observer.md). They are not
accepted spans and remain outside the OTLP span projection. Telemetry schema 6 adds separate local installation [app timings](app-timing.md), excluded from accepted query version 3 and remote responses. Schema 5 introduced engine observations (schema 4 introduced resource
samples/rollups); earlier stage/schema
notes below describe their introduction rather than the current schema number.

Preacceptance voice measurements are promoted only by genuine durable acceptance;
see [voice-timing.md](voice-timing.md) for receipts, aggregate abstentions and limits.

## Original endpoint to response submission

After genuine durable voice acceptance, a native-only timing owner retains the
original `Completed` endpoint Instant. It stays with that accepted caller through
planning and transfers to the actual normal speech coordinator; neither a saved
reply nor frontend input can reconstruct it. Cancellation drops that owner, and
no retry or output admission renews its start time. It has no authority role.
`Completed.completed` is the end of the accepted endpoint span after the required
quiet tail and minimum real capture span, not the last detected speech frame or
human utterance end. This measurement therefore excludes endpointing's required
quiet tail from user-speech-end latency. A separate original last-speech-end
observation is still needed to measure that full latency; A06 targets remain
unmeasured by this stage.

The `endpoint_response_submission` stage ends at the original submission timestamp
of the first postmix reference block with a finite nonzero speech-component peak
and nonzero mixed samples, correlated to that accepted response's exact output
UUID and live output epoch. Each block spans at most twenty milliseconds; its
timestamp marks the block's first sample, so a later nonzero speech sample within
the block can be reported up to twenty milliseconds early. This is block-level
submission timing, not an exact first-nonzero-sample timestamp. Observation
runs before display throttling. Background music, effects, chimes, previews and
startup greetings cannot satisfy this measurement. The output correlation roster
is bounded to eight entries and retained by the actual speech coordinator.

Each observed accepted turn ends with a completed, failed, missing, or abandoned
timing record. Missing means the normal path ended without a qualifying speech
submission; it is not a zero-latency success. Only completed records contribute to
this stage's latency percentiles. Failures before output and caller withdrawal
remain in the counts. Collector loss/crash gaps are separately unavailable.

This is endpoint-to-first-nonzero-response-block **submission**, not acoustic delivery or
human-rated usefulness. A generated generic acknowledgment can still fail A06;
the trace alone never proves its useful-response criterion or its latency gates.
Actual captured endpoint and callback clocks remain in the same native process.
Telemetry schema3 adds this stage and missing outcome, reading existing schema1/2
records without rewriting them. Older collectors refuse schema3 nonfatally; the
action database and speech wire are unchanged.

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

Manual saved cohorts use existing current-owner queries without changing trace wire version. See [manual comparison contracts](performance.md#manual-saved-cohorts-report-format-1). Reports preserve actual source records and distinguish owner annotations from measured provenance. No imported traces can become saved native evidence.
