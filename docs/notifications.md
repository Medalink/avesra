# Committed event announcements

Plan 001 section 11 requires a learning sound only after a useful memory change
commits, and an action sound only after its actual outcome is verified. Store a
bounded typed event description and source identity in that same writer
transaction. No event from a speculative model result, duplicate observation,
failed write, generic acknowledgement or merely dispatched effect is announceable.

Build immutable batches with stable IDs and exact member event IDs for one actor
and device. Learning and action toggles and volumes remain independent. Deafen,
pause and active speech take precedence. Coalesce background learning, initially
at most one sound per minute, and preserve silent events for inspection. Do not
replay old batches after reconnect or unmute.

Delivery has an actual native output owner. A scheduled/submitted batch is not
yet the last announced batch. Record the exact batch only at the defined observed
playback milestone; retain delivery uncertainty and never infer an acoustic sound
solely from a server response. Failed/suppressed/cancelled output must not advance
the last-announced pointer. Ordinary output ownership also prevents a chime from
being accepted as an owner command.

The source implementation uses a schema-15 native journal. Event content is typed
and bounded; batches contain at most 32 exact event IDs and never change members.
The publication milestone is the existing renderer's final sample submission
and bounded drain observation under the original output lease. This is explicitly
submission evidence, not proof of audible delivery. Silent diagnostic output
cannot advance the announced pointer. A pending batch found after process/session
replacement becomes uncertain and is never replayed. Output control loss suppresses
new pending events from that interval. Raw microphone data, browser content,
credentials and speculative planner text are never notification payloads.

Retention is transactional: old settled batches expire together with unreferenced
settled events, preserving pending deliveries, the latest submitted batch for
each actor/device/kind and its immutable members, plus recent inspection entries.
Events older than one minute become silent. If protected records exhaust the
hard metadata bound, suppress new announcements and increment one content-free
capacity counter in that transaction. Notification capacity never rolls back a
verified action outcome or useful memory commit; their original ledgers remain
the exact sources. Expired pending delivery becomes uncertain, never rescheduled.
The limits are 32,768 event rows and 4,096 batches. Compaction keeps the newest
512 event rows and 256 settled batches in addition to protected batch members.
An answer that exceeds the speech text budget describes the first complete
members that fit and gives the exact remaining member count for Settings inspection.

The launch-only `--capture-notifications` flag requires `--capture-output` and
explicitly allows genuine new committed events to reach the silent recorder.
It does not create an event or replay history, and recorded delivery stays
uncertain rather than advancing the announcement pointer. The recording sidecar
identifies this mode. Ordinary output capture keeps this worker disabled.

Personal continuous listening does not need a microphone gap for a notification.
Only an actually current native Personal profile permits notification output
while capture is owned. The notification keeps its original actor/session,
foreground-task priority, toggles, volume, bounded output owner and final-submission
checks. Its real postmix tone enters the same Personal echo-reference lane; no
notification manufactures quiet input or voice authority. Qualification locks are
released before runtime publication. Other admission modes retain the measured
quiet-gap behavior below.

Non-Personal always-on listening hands off only at a measured idle boundary. A genuine claimed
batch requests a native token; the normal utterance owner first observes quiet,
sends its real final tail, verifies that tail stayed quiet, and awaits stream
retirement before granting the token. New windows defer while the token is held.
Active or unknown speech does not grant it. The tone occupies a bounded 240-ms
gap and releases ownership only after output cleanup. Listening then requires a
fresh no-output proof and measured initial quiet. This degraded mode does not
provide acoustic barge-in during the gap; playback echo cancellation and listening
while speaking remain separate qualification requirements.

An accepted “What did you learn?” or “What did you just do?” reads the actor's
last actually announced batch even if newer silent events exist. Distinguish
verified actions, learned facts, unvalidated routine candidates and pending
approval. No recorded batch means no claimed event. Deletion removes dependent
event content without exposing a deleted fact through an old batch; retain only
the minimum content-free tombstone needed for no-repeat behavior.

The native ledger derives this answer inside the original accepted claim's final
transaction. Its stored reply carries explicit `NativeEvents` provenance: learning
or action, the exact last submitted batch ID and its ordered event IDs, or no batch
and no IDs when nothing was announced. IDs are nonnil, unique and bounded to32.
Deleted event bodies remain described as deleted. No caller supplies the answer
text or event list, and history cannot reconstruct the opaque published handle.

Speech wire4 carries this provenance through the existing normal-speech handshake
and echoed stream context. Native-derived output is not a model completion. The
controller authenticates the paired current actor/session/action/output context,
consumes the genuine claim's ordinal above its session high-water mark, records
the exact typed response/provenance/reply digest and immediately reserves its one
output attempt under the same lock. It cannot relabel an existing model request,
reuse an old ordinal or recover a cancelled opportunity. The actual public owner
is retained before private work; existing private TTS retirement/uncertainty also
holds the source. Model provenance still requires the actual model completion.

This is a paired-native assertion of the PC ledger's derivation, not independent
server verification of PC events. Only the native worker consuming a genuine
accepted claim can mint this path; there is no frontend plaintext command, new
debug endpoint, imported response or synthetic accepted turn. The native original
claim lifetime is checked immediately before sending the handshake. Schema22
marks stored reply provenance compatibility; older history defaults to Model and
cannot gain a native-derived output handle.

The overlay/settings can inspect recent scoped events without replaying sound.
Required live evidence includes commit failure, coalescing, mute/pause, restart,
newer silent events, exact follow-up and cancellation. Silent diagnostic media
proves submitted output only; it must remain labeled as such.

Native event answers are retrieved into later dialogue only while their exact
submitted batch and every ordered event dependency still belong to the same
actor/device/kind and retain unchanged content. The native claim transaction
reconstructs the bounded answer and compares it with immutable history. Redacted,
corrected, missing or expired dependencies exclude that pair; historical records
remain under the separate history policy in [memory](memory.md).
