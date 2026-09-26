# Browser workflow evidence

This contract refines Plan001 C5 and `owner-workflows.md`. A generic C4 excerpt
never proves a mailbox count, account identity, message ordering, delivery, or
X readiness. Browser text is untrusted evidence, never instructions or authority.

## Message evidence consumer

The native consumer freezes the configured account, Inbox scope, original
request count (1 through 100), provider binding revision and original monotonic
deadline. It accepts only observations from the same actual accepted browser job,
after its owned C4 content/settlement checks. Parsing the structures below does
not establish that ownership. Raw message bodies remain transient and are not
serializable task receipts or telemetry.

Each batch identifies its ordinal and continuation boundary, the configured
account and Inbox, binding revision, exact observed document and DOM revision.
The provider must establish the newest individual-message order, not conversation
order. Each message has an immutable provider message ID, thread ID, unambiguous
UTC timestamp, sender, subject, complete visible body and source reference.
Thread expansion must be complete. A visible snippet, inaccessible or truncated
body, or unexpanded conversation cannot supply a complete message.

Across pages the consumer verifies consecutive batch ordinals, exact continuation
matching and non-increasing message timestamps. Identical overlapping messages
are deduplicated by immutable ID; a reused ID with changed content, date, source
or thread invalidates the observation. Continuations may not cycle. The original
deadline is checked before and after each bounded consumption. At most 100 unique
messages, 32 batches, 16 KiB per body and 256 KiB total transient body bytes are
accepted. A batch may contain at most the original requested count. More data is
not silently truncated into a successful count.

Completion requires exactly N unique messages with complete individual-message
ordering evidence, or an explicit verified end of Inbox with fewer than N. If a
provider cannot prove either boundary, the result stays incomplete. An empty
first page is complete only with explicit end-of-Inbox evidence. End-of-Inbox
and a next cursor cannot both be asserted. No generic page mutation, title,
row count or absence of a Next label proves end of Inbox.

Delivery findings are source-local literal evidence candidates, not assertions
that a physical package arrived. Negated, estimated, conditional and conflicting
phrases remain ambiguous. Shipped and out-for-delivery are never delivered.
Several candidate messages are not assumed to be the same package. Results must
identify sender/date/subject/message reference, quote only the bounded supporting
phrase and disclose that these are email claims. No tracking link, attachment,
send, archive, delete or posting operation is part of this consumer.

## Provider binding and operation boundary

Gmail currently has no inspected provider binding in this source checkpoint.
The wire7 accepted provider-header inspector is now a concrete C4 consumer;
`browser-provider-operations.md` specifies its bounded scope and transient
delivery. A complete header probe does not require scanning message/feed content
and does not prove mailbox ordering, bodies, account identity or readiness.
There are no guessed CSS selectors, private Gmail API calls or synthetic DOM
fixtures. A provider must bind actually observed account, Inbox, individual
message identity/date/body, thread expansion and pagination semantics through
native protected setup before producing these observations. Owner labels alone
do not prove those semantics. The C4 single-document excerpt remains unchanged
until that binding is available; it cannot manufacture a semantic batch.

Opening Gmail messages may mark them read. Opening/expanding/paginating are
distinct authorized browser operations, not hidden mutations inside a read-only
extractor. Each operation needs the original accepted authority, same actual
browser owner and revalidated expected state. Do not retry a mutation after an
unknown result. Navigation changes document identity and requires explicit
continuity, never reusing the old document's DOM guard.

X readiness separately requires current configured browser/profile/account,
exact `https://x.com` origin, authenticated account identity and an actual enabled
compose surface. Login, CAPTCHA and unknown provider state remain NeedsInput or
Unsupported. No text entry or submission follows from a readiness request.

## Entry points and evidence boundary

| Entry point | Treatment |
| --- | --- |
| Core mailbox accumulator | Pure bounded evidence consumer; cannot create browser authority |
| Extension mailbox batch validation | Strict untrusted observation validation; no DOM operation |
| C4 `beginPageExcerpt` / `ReadJob` | Unchanged partial excerpt only; cannot enter mailbox consumer |
| Accepted provider-header inspection | Uses the actual C4 job and delivered transient probe; no provider mutation or completeness inference |
| Protected provider binding / Gmail or X workflow | Requires independently checked account semantics and separate operation grants before activation |
| Task receipt / model summary | Only bounded source references and conclusions; no raw mailbox body telemetry |

No tests or harnesses are added under the owner's execution override. Root owns
static/build verification and authorized real Gmail/X workflow evidence. This
consumer source does not establish OW2/OW3 compatibility or successful operation.
