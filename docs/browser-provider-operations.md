# Owned Gmail and X provider operations

This is the next source contract for Plan001 C5. The implemented partial-read
consumer is governed by `browser-task-consumer.md`; it grants none of the new
operations below. Provider compatibility has not been observed on the owner's
accounts. No Gmail CSS selectors, application-private APIs or private browser
profile copies may be invented to fill that evidence gap.

The first integration checkpoint adds accepted read-only provider inspection on
wire7/schema19. The exact whole request `inspect Gmail provider` or
`inspect X provider` resolves one existing owner-granted ReadPage scope at the
fixed provider origin and its explicitly selected current document. No setup
button, arbitrary text IPC or model proposal can start this inspection.
It reuses C4's actual job, original ten-second deadline, DOM guard, exact cleanup,
settlement and delivery ownership. Protected semantic binding and mutating
provider operations below remain separate subsequent work.

## Fixed operations and bindings

Only Gmail at `https://mail.google.com` and X at `https://x.com` are supported
provider identities. A protected binding contains the exact native actor, saved
browser scope/profile/pairing, provider, account identity, selected Inbox (Gmail),
binding revision and reviewed observed semantic element identities. Frontend
choices are opaque lookup IDs from one native-retained probe; they cannot supply
selectors, JavaScript, URLs, DOM paths or arbitrary operation labels.

The initial fixed vocabulary is Inspect, GmailInboxPage, GmailOpenThread,
GmailExpandThread, GmailNextPage and XReady. Inspection reads bounded semantic
metadata under a real accepted Read operation. Gmail navigation/expansion and X
opening/focus require separate exact provider-operation grants and accepted-task
payloads; a generic ReadPage grant does not authorize clicks or navigation.
There is no compose, input, submit, send, delete, archive, attachment or external
tracking-link operation in this vocabulary.

Wire7 adds an explicit InspectBrowserProvider accepted payload and typed provider
probe read mode/outcome. It uses the existing Read grant because it observes
bounded metadata, but cannot invoke controls. Native observations store only the
provider, choice count, completeness flag and content/document digests; no control
labels or attributes enter telemetry/history. Schema19 accepts legacy excerpt
observations without provider metadata, while requiring it for new inspection
payloads. Old browser wire versions fail closed rather than dropping mode fields.

Each inspected control carries its observed role, accessible label, bounded
identity attributes and structural relationships. Metadata excludes values of
password/input/contenteditable elements, hidden elements and arbitrary script
properties. An owner may map actual choices into semantic fields, but mapping
alone does not establish semantics. The provider validator independently requires
meaningful labels, correct structural association and verified postconditions.
An arbitrary button renamed Next by setup cannot become a pagination target.

The fixed inspector finds one application header while pruning main, message,
feed, list, table, form and dialog subtrees. The explicit `provider_header` scope
is not a whole-page or mailbox observation. At most2048 bounded visits and a50ms
synchronous budget cover discovery and at most64 header choices with four
whitelisted attributes each. Ancestor visibility is cached only within this
guarded synchronous observation. It does not read editable values, scripts,
frames or shadow contents. Missing or ambiguous app headers remain incomplete.
Accessible labels come only from explicit aria-label/aria-labelledby or a native
button/link/heading's bounded text; label LF/CR/tab whitespace normalizes to spaces.
Truncation makes this header probe incomplete. The
choice UUIDs and ancestor references belong to this one settled observation;
they cannot be supplied as executable selectors. Settings displays the actual
probe transiently for at most60 seconds using the existing caller-delivery marker.
Neither a complete probe nor an owner label establishes account/message semantics.

For Gmail, the provider must independently distinguish individual-message IDs
from thread IDs; verify sender/date/subject/body associations; prove each expanded
body is complete; identify selected account and Inbox; and establish newest
individual-message ordering with continuation/end evidence. Unknown attributes,
ambiguous rows or unsupported locale/provider revisions fail closed. The mailbox
accumulator accepts only the resulting validated observations. Search snippets,
thread counts, aria row indexes and absent Next buttons cannot prove completeness.

For X, readiness requires the exact configured account, authenticated session,
intended origin and actual enabled compose affordance. Sign-in/CAPTCHA are typed
NeedsInput states. No generated draft is part of the operation. Browser focus is
an explicit final owner-requested effect, never part of background inspection.

## Native and extension ownership

An accepted provider task is durable before dispatch. Its original monotonic
deadline, source, current grants and exact binding remain fixed through all steps.
The native effect worker retains the actual browser resource while each typed
operation and cleanup settles. The extension uses the same module-global actual
job exclusion as C4 and metadata discovery. It never starts a provider operation
from a webview message or arbitrary setup field.

Every step freezes the expected document, account, Inbox/view, message/thread or
page boundary and exact semantic target. Immediately before a one-shot mutation,
recheck permissions, document lifecycle, target identity, binding, current native
authority and expected prior state. The operation is never retried after a
possible invocation. Poll only read-only postconditions under the original budget.
Navigation invalidates the prior document guard; a successor document must have
explicit observed continuity to that exact one-shot operation. A moved/replaced
tab cannot be substituted.

The settlement protocol binds full original task/dispatch/step/request and
operation ordinal. Content cannot establish settlement. Lost/rejected invocation
or cleanup retains uncertain ownership and blocks replacement. Native durable
acknowledgement alone clears the metadata outbox; reconnect never replays content.
Caller loss withdraws publication, never drops actual work. New grants or binding
changes cannot revive an old job.

## Entry-point inventory

| Entry point | Requirement |
| --- | --- |
| Accepted provider inspection | Same C4 actual read owner; bounded transient probe metadata |
| Protected binding commit | One retained probe, exact source/revision, native-derived identities and independently validated provider semantics |
| Protected operation grant | Separate from generic excerpt grant; exact provider/account/scope/binding |
| Accepted Gmail/X task resolver | Whole-request grounding; no model target/selector authority |
| Extension typed step owner | Fixed operation dispatch, original actual lease, no retry after possible mutation |
| Native result consumer | Borrowed current evidence; message completeness/account readiness validated before delivery |
| Lock/disconnect/navigation/cancellation | Synchronous publication withdrawal; actual cleanup/uncertainty retained |

No source-level inspection or binding UI is a live compatibility pass. Actual
Gmail/X field identities, semantic relationships and navigation behavior must be
observed in a separately authorized browser session before an adapter revision
may be claimed usable. No active desktop/account inspection runs during source
implementation. Root owns aggregate checks and real OW2/OW3 evidence.
