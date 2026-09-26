# Accepted exact-site partial page reads

This callable C4 consumer is infrastructure for Plan001 C5. It does not establish
mailbox enumeration, authenticated account readiness, navigation or posting.

Wire7 also supports the exact accepted requests `inspect Gmail provider` and
`inspect X provider`. They resolve the same explicit ReadPage grant and selected
document at the respective fixed origin, then return bounded transient semantic
metadata under the identical owned read/settlement/delivery path. See
`browser-provider-operations.md`. Inspection is not mailbox completeness or X
readiness, and introduces no clicks, navigation or new implicit site grant.

Protected owner setup may grant ReadPage for one exact saved browser ScopeRef,
origin, actor, pairing, selected browser/profile and app revision. The ordinary
ledger grant remains separate from Chrome's host permission. Native setup reads
the actual saved scope and rechecks it on the same effect worker immediately
before grant commit; UI references are lookup hints only. Schema16 marks this
new persisted target variant, following notification schema15.

The closed whole accepted request `read page at https://HOST` (optionally an
Avesra prefix) independently resolves exactly one current saved site grant.
Paths, queries, fragments and model-provided target IDs cannot select a target.
The original opaque accepted turn is linked to a durable task before execution.
It requests at most16 partial excerpt blocks, not16 messages.

Setup's original five-second observation deadline still governs completing
document selection. The selected pointer is retained as metadata only. An
accepted task must resolve exactly one explicitly selected current document for
the exact saved scope; no first-tab fallback is allowed. Transport, pairing,
selection, app revision, navigation observation revision and resource generation
must still match. Moved/replaced tabs and ambiguous documents require fresh owner
selection. Actual Chrome permission/document checks, original10-second ownership,
DOM guard, exact cleanup, content-before-settlement handling and durable settlement
acknowledgement remain governed by the existing C4 contracts.

The actual worker gives a concrete consumer the one-use borrowed read only after
settlement and native current authority checks. The consumer retains no transfer
of dispatch authority. It publishes bounded untrusted text synchronously from
that actual consumer once, before the read/preparation owners drop; no raw reply
is sent to a later async continuation. Matching source/task/current session permits
pending transient storage for at most60 seconds. A separate native delivery marker
allows Settings display only when the original accepted caller actually receives
the matching successful browser receipt. Caller loss before that delivery abandons
and removes pending content. This marker is independent of normal worker cleanup's
withdrawal signal; dropping worker resources never manufactures delivery. Content is
neither conversation history, learned memory nor telemetry. Settings renders
plain text and marks coverage partial. Caller loss/current-context change clears
publication, while actual browser work continues to owned cleanup.

Empty reads are reported as an empty partial observation, never proof that the
site/mailbox has no messages. There is no mailbox summary or normal spoken answer
from this checkpoint, and no provider selector binding is fabricated.

Entry points: protected `grant_browser_read_action`, original `tasks::accepted`,
the existing ledger/`execute_read`, C4 `ReadJob`, and actor-filtered `action_status`
projection. Settings cannot submit arbitrary reads or text as an accepted task.
Existing read permission removal, setup hide, lock, disconnect, navigation and
registration withdrawal remain effective. No tests or live browser operations
are performed under the owner's source-only execution override.
