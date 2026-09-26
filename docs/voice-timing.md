# Preacceptance voice timing

Voice-analysis wire version2 requires a nullable, strict typed timing receipt.
The speaker-only enrollment route remains version1. Both normal voice and explicit
VoiceCheck use the version2 analysis client. A receipt contains exactly one ASR
and one speaker measurement: original public utterance/request, actual distinct
private worker UUIDs, controller host/process identity, pinned lane revision,
controller wall start and process-local monotonic start/duration. Durations cover
the actual controller-to-driver round trip, including IPC and validation; they are
not GPU kernel time. The collector being unavailable yields null, not invented
identity or a failed otherwise-valid analysis. Invalid receipts reject correlation.
Successful replies alone carry receipts; failed server stages are not inferred
from native failure totals.

Native capture span, last activity exchange producing an endpoint, endpoint-to-
consumer queue age, analysis round trip, intent and gate decision timing remain
in a bounded nonserialized utterance-owned bundle. They retain original capture
and endpoint Instants. Only the actual returned DurableTurn promotes that bundle
to persisted trace records. Ambient/rejected/expired input leaves no retained
utterance/request ID, audio, transcript or embedding in telemetry. Fixed enum-only
process aggregate counters account for queue overflow/expiry, unknown reference,
insufficient span, empty transcript, gate abstention, analysis failure and caller
abandonment. They are rolling process observations, not labeled accuracy rates.
The fixed enum totals are included in the accepted-trace export with an explicit
process-wide scope, independent of its selected-turn filter; unavailable is null.

The promoted native Analysis record embeds the validated controller receipt.
Settings displays its separate controller timings, and the redacted export
projects them as controller OTLP resource spans under the genuine accepted turn.
Parent-operation attributes link the actual worker to the original utterance;
no unknown parent span or cross-host clock offset is fabricated. Native rollups
remain native durations. The nested successful receipt does not contribute a fake
native inference duration or hide native failed/abandoned measurements.

Original endpoints are preserved for later A06 end-to-playback instrumentation.
This slice does not claim first meaningful audible response, partial ASR timing,
private model compute time, engine counters, hardware headroom or live release
proof. Existing bounded trace retention/export applies after promotion. The
observer cannot authorize/retry work or block voice when local tracing is absent.
The separate telemetry database advances its compatibility marker from1 to2 for
new stage variants/nested receipts; old rows deserialize without receipts and are
not rewritten. Older collectors refuse schema2 nonfatally. This does not migrate
conversation, identity, memory or action storage. Trace-query envelope version1
is unchanged; records without receipts omit the new optional field.
No tests, fixtures, live capture or runtime operations are part of implementation.
