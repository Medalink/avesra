# Owned Gmail Inbox reads

This refines Plan001 C5, `mailbox-workflows.md`, and the browser read channel.
The owner authorized a ReadInbox-specific original 120-second lifetime. Every
other action retains its existing lifetime; generic excerpts retain ten seconds.
The deadline starts at exact accepted-task linking, includes preparation and all
pages, and cannot be renewed by acknowledgments, account input, or navigation.

ReadInbox binds the existing protected Gmail browser/profile scope, exact account,
Inbox, and requested count (1–100). Only the original accepted task can publish
the request. Settings may configure the account but cannot manufacture a read,
provider observation, or account identity. Ambiguity requires account selection.
No send, archive, delete, attachment access, or posting is authorized. Opening a
message may mark it read; navigation and expansion require the existing explicit
Read plus Navigate scope and remain one-shot owned operations.

Mailbox bytes use bounded, consecutive, acknowledged chunks on the original
authenticated connection. One chunk may be outstanding; each carries the full
original read context, ordinal and preceding digest. Acknowledgment means only
that the actual native worker retained those bytes under current original
authority. It does not mean message semantics were accepted, cleanup completed,
or a result was published. Disconnection/cancellation destroys transient content;
no body replay or resumption occurs on another connection. The existing durable
metadata-only settlement outbox retains actual browser exclusion until native
acknowledgment. Unknown actual Chrome completion never releases that exclusion.

Each UTF-8 chunk is at most 8 KiB and its encoded frame remains within the existing
64 KiB wire limit. Complete message bodies are at most 64 KiB; combined body bytes
are at most 1 MiB. Semantic metadata and framing have separate bounded overhead.
Bounds are refusal/incomplete boundaries, never silent truncation into success.
The final terminal correlates the exact byte count, chunk count and digest before
parsing. Only final actual cleanup settlement plus current native authority can
release the completed semantic result to the original borrowed consumer.

Semantic completion requires message-specific Inbox membership, immutable message
and thread identities, unambiguous dates, complete loaded bodies, and evidence of
newest-individual-message order through the unseen-page frontier or explicit end
of Inbox. An Inbox conversation row is not proof that all its messages are in
Inbox. A locale-formatted minute date is not an unambiguous UTC timestamp. A
collapsed, clipped, incompletely loaded, or ambiguous body is not complete.
No private globals, network interception, private Gmail API, or fabricated DOM
fixture supplies missing observations. Actual provider/account DOM inspection is
still required; the source must produce an explicit incomplete reason until its
fixed semantic observer can prove those facts. No generic excerpt becomes mail.

| Entry point | Boundary |
| --- | --- |
| Protected site/account permission | Existing native owner proof, exact saved scope; no read execution |
| Exact accepted Inbox resolver | One durable task before any browser publication |
| Browser ReceiveOwner / worker stream | Actual authenticated connection and original current dispatch |
| Extension Gmail job | Shared actual browser owner; bounded fixed-provider observations only |
| Chunk acknowledgment | Transient transport receipt only, not semantic acceptance |
| Final native mailbox consumer | Complete checked evidence or explicit incomplete outcome |
| Generic excerpt / X | Existing semantics and shorter deadlines unchanged |
| History / reconnect / model / Settings | Cannot reconstruct content or an accepted job |

The explicit owner override forbids automated tests, fixtures and harnesses.
Root performs source review, aggregate builds and separately authorized live proof.
This source contract makes no Gmail compatibility or OW2 runtime-pass claim.

## Previous transport checkpoint (before successor support)

The preceding browser wire v9 and Store compatibility marker24 added the exact account target,
original120s read admission, one-outstanding-chunk transport, semantic accumulator,
and transient result display. Marker24 adds no tables and preserves the schema23
memory migration. Protected account setup does not assert that Gmail is readable.
The current extension observer checks only the actual visible account header and
always returns a typed incomplete result. No message-opening, body enumeration,
pagination or complete-mailbox producer is implemented at this checkpoint.

The closed accepted grammar supports read/check latest1–100 emails, including
spoken English numbers and “Check my latest ten emails and see if I got my package
delivered.” The account comes from the exact saved scope, not the request or model.
A selected current Gmail document is still required. Owned Inbox navigation,
individual membership/date/order/body proof, and original120s source-grounded
spoken delivery remain follow-up work. No generic planner30s deadline was enlarged
and no renewed speech lifetime is implied. Actual matched extension/host install,
profile pairing, intended account selection and authorized real DOM observation
remain live prerequisites. This checkpoint is incomplete OW2 infrastructure.

## Owned transitions and terminal speech

The next source slice separates the fixed provider observer from its lifecycle.
A same-document Gmail operation may consume one opaque control offer created by
the fixed observer from a currently connected element in the observed main region.
The offer retains that element, its exact current route, operation kind and expected
postcondition scope; it accepts neither caller CSS nor a reconstructed DOM ID.
Only open-message, expand-message, next-page and return-to-Inbox operations exist. Each operation
is consumed before the first click, never retried after partial/unknown completion.
Only mutations within that retained observed region during that one transition
may be pending. Account/header changes, user input, scope replacement, unexpected
route changes and lifecycle loss withdraw the job. Completion requires fresh
account and operation-specific semantic postconditions, not a timer or successful
click return. Every Chrome promise stays owned through actual completion; an
unknown completion retains exclusion. A new provider generation is not a new
120-second deadline. The current account-only observer issues no control offers.

A complete read may mint opaque MailboxEvidence only inside the actual core read
worker after final semantic validation and authenticated cleanup settlement. It
binds the original dispatch/action/context/account/count/digest and original
absolute deadline. The native accepted coordinator retains it once alongside its
original live ObservationClaim; history, Settings and serialized mailbox data
cannot recreate it. The final native writer checks the exact persisted browser
observation/finalization and current grant before consuming it into one bounded
NativeMailbox reply. Raw bodies remain transient. The saved spoken summary labels
statements as email claims and retains source references; it never claims physical
delivery, a matched package, or complete latest-message coverage from ambiguous
or incomplete evidence. No match means no matching confirmation in the checked
messages, not proof that a package was not delivered.

This continuation uses the original mailbox deadline through reply publication
and output. Generic planner claims retain30 seconds. Speech preparation and output
are capped by the original mailbox deadline, never renewed from completion time.
Failure, cancellation, caller loss or expiry drops transient evidence; no retry or
history replay constructs new speech. NativeMailbox sources are excluded from
model dialogue history. Real provider proof and complete enumeration remain
required before this complete-result path can be exercised.

## Continuation checkpoint and remaining browser lifecycle

Browser wire10 now accepts only same-document fragment-route changes during an
original Gmail job; original tab/window/document/origin/path/query identities
remain exact. The isolated Gmail guard retains actual control elements and one
observed main region, consumes offers before invocation, and requires a fresh
fixed-observer postcondition to settle. Main-root replacement, unrelated mutations
and user input withdraw. A click with unproven completion makes cleanup unknown;
its browser exclusion is retained. Native chrome revalidation surrounds the owned
operation. These lifecycle entry points do not invent account/message semantics:
the current header-only observer issues no offers and still reports incomplete.

Speech wire6 adds NativeMailbox provenance. The core complete-evidence source and
native registered browser successor transfer only after the exact settled read's
borrowed consumer and receipt succeed. Normal successful teardown then releases
Chrome ownership while the original source cancellation/deadline remains registered
through output retirement. Failed/uncertain teardown still withdraws. Successor
checks retain browser session/process-selection/target generation/scope context;
scope/pairing replacement, browser disconnect, actor/action revocation and caller
loss withdraw the pending reply or output. Raw bodies are not written to history.
Generic planner lifetimes stay unchanged. Real mailbox production is not proven.

The former 300-second manual-reconnect gap has a source implementation in the
saved-selection supervisor described in browser-control.md. Sessions still expire
at their original 300-second limit; new sessions use fresh authentication, with
independent persistent Disconnect preferences in native Settings and the extension.
No job, content, document handle or mailbox successor is replayed or renewed.
Read admission refuses a remaining job budget that cannot fit the session horizon.
This slice performs no early rotation and preserves unresolved cleanup exclusion.
Live browser installation/pairing and reconnect behavior remain unverified.
