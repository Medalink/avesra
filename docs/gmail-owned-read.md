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

## Current source checkpoint

Browser wire v9 and Store compatibility marker24 add the exact account target,
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
