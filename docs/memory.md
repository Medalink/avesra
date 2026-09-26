# Sourced private memory

## Settings Memory presentation

The saved-record list follows the card hierarchy in
`design/mockups/Settings.dc.html` lines 964–1015: 14-pixel horizontal/12-pixel
vertical padding, source-kind badge, 13-pixel content, 11-pixel source/date and
compact correction/deletion controls. Only real current snapshot records appear.
Facts are labeled Stated (an owner assertion); routines retain their actual
candidate/verified-invocation/disabled state. Neither label implies inferred
knowledge, playback or a fresh successful action. Exact source identifiers remain
available in expandable source details, and accepted named-fact source inspection
keeps its existing protected reader and exact ID/revision checks.

Add opens the existing verified-task form explicitly. Correct opens that same
editor with the actual current entry, retaining its named-fact limits and routine
disable option. Closing the form only hides it and preserves local input; the
visible Continue action reopens it. Cancel correction retains the existing clear
semantics. Successful save or deletion of the edited record closes the editor.
Lock/hide/context invalidation clears it with the existing private-memory state.
No new memory search, inferred records, undo, source selection, native command or
authorization path is added. SetupLock, pending deletion approval, error/status,
Teaching and source-history views remain accessible outside the collapsed form.

Only PrivateMemory's presentation and local form visibility change. Its existing
refresh/save/correct/delete/source-read entry points, Hello and grant admission,
source/revision binding, busy gates, retained command ownership and stale-result
checks are unchanged. Source review and a focused frontend check cover this slice;
no automated tests or installed private-record proof are claimed.

## Explicit verified-task records

The first product writer supports explicit owner facts and one-step routines
saved from a selected task with an original successful native observation.
Settings supplies only a task lookup ID and bounded owner text; the same native
ledger worker resolves actor, original accepted source, exact action and original
verified outcome. A fact is an explicit owner assertion associated with that
task, not an assertion proved by the task. No model inference is involved.
The existing protected management proof is required for save, correction and
deletion. Current-owner inspection is bounded to256 records. Entries retain
immutable source identity and revision provenance; duplicate saves do not emit
events. Deletion clears every revision body and dependent learning-event text in
the same transaction, retaining only content-free identity tombstones. Original
accepted task history and separately retained backups are not erased by deleting
a derived memory. This path is separate from scoped demonstration capture.

Memory revisions use retention rather than a lifetime write quota. In the same
transaction, keep the newest 512 memory-event records, every current live memory
revision, and exact revisions referenced by retained routine invocations. Expire
other revisions and then unreferenced deleted-memory tombstones in foreign-key
order. Retained invocations remain governed by the task-history owner; memory
compaction cannot erase their provenance. Active records remain limited to 256
per actor. Repeated correction/deletion therefore cannot exhaust a global 4,096
revision counter and permanently disable future useful writes. Deletion still
redacts all protected historical bodies before any retention decision.

Plan 001 C6 and sections 10–11 govern this store. Accepted conversations and task
records already have an owner; do not create a second history database or writer.
Add versioned migrations to the existing Store, with strict schema validation,
foreign keys and a transaction for each committed useful change and its event.

Every fact has an immutable ID, actor, revision, bounded value, explicit/inferred
kind, current/corrected/deleted state and one or more typed source references.
Sources name an existing accepted turn, verified task result or authorized scoped
demonstration/observation. Model text cannot invent a source, actor or visibility.
Resolve and validate source ownership on the same writer before commit. Never
learn from ambient/unknown speech or retain raw audio, screenshots, credentials,
password/MFA values or complete page/email bodies as a substitute for a fact.

Retrieval always takes the current authenticated actor and current native source
authority, with bounded paging/row sizes. A caller-supplied actor UUID is not an
authorization check. Private facts, history and routines are private by default;
shared routines must have a separate explicit scope. Another enrolled speaker
does not inherit owner email/project details. No full-store scan is supplied to a
model by default, and similarity search never bypasses actor/source filtering.

Corrections create an authoritative revision and retire prior retrieval copies.
Do not overwrite a corrected fact with a later inferred guess. Identical evidence
is deduplicated and creates no learning event. Deletion atomically removes the
selected content and dependent retrieval/index copies, and retires derived facts
without another valid source. Metadata tombstones contain IDs/times/categories,
not deleted text. Existing offline backups are separately retained and must not
be described as erased. Deletion of one actor's record cannot delete another
actor's independent evidence.

A historical native event answer remains an immutable conversation record, but
it is not an unconditional retrieval copy. Before adding it to a later planner
request, the same claim transaction revalidates its exact submitted batch,
ordered event IDs, actor/device/kind and current event bodies, and requires the
reconstructed answer to equal the stored text. Deleted/corrected bodies, expired
batches or missing/mismatched dependencies exclude that pair from model context.
An empty, content-free no-announcement answer needs no event dependency. Legacy
answers without typed provenance are excluded only when their original accepted
question matches the exact native event-question grammar. No historical reply is
erased or heuristically searched for deleted text. This does not erase old model
answers that separately repeated a fact, original tasks or retained backups, and
is not proof of the complete A16 deletion workflow.

Native Settings inspection and deletion require the existing owner-management
boundary. Spoken remember/correct/delete requests require an actually accepted
turn and the ordinary exact proposal/approval rules. No arbitrary transcript or
UI pass flag can be converted into accepted memory through a debug command.

Observe actual source commit, restart retrieval, correction, deletion, dependent
copy removal and cross-actor isolation before marking A14–A16 passed. Compilation
or a migrated empty table is not learning evidence. No test fixtures or harnesses
are introduced under the owner's specs/code/direct-observation workflow.

## Accepted named facts (schema 23)

An actual accepted owner turn can directly say `remember that KEY is VALUE`,
`update KEY to VALUE`, or `what do you remember about KEY`. KEY is an exact,
normalized private label (1–80 UTF-8 bytes); VALUE is explicit owner content
(1–512 UTF-8 bytes), not an independently verified fact. Command prefixes are
case-insensitive. Values retain their original content, including punctuation;
only label lookup and fixed query suffix punctuation are normalized. An existing
label with a different value is not overwritten by remember: use explicit update.
Unknown labels and unsupported forms receive one concise native clarification.
Identical saves/updates are no-ops and emit no learning event.

These commands consume the original opaque planner claim and current native
owner/registration. The same writer commits the memory revision, useful event
and deterministic NativeMemory reply together under the original deadline and
withdrawal. No successful task is fabricated. The existing memory table gains a
nullable task source and an accepted-turn source with exactly one present;
legacy task-sourced records remain readable. Accepted facts retain their original
turn/revision and the current correction's accepted-turn or protected-owner
management provenance. A fact never becomes a routine or grants tool authority.

`forget KEY` resolves one exact current private ID/revision, presents that record
for explicit current-owner approval, and retains its original pending claim.
Only the protected native approval may complete that deletion before expiry.
Missing/ambiguous/changed records do not delete anything. There is no delete-all
voice command. Closing the approval surface, pause/lock, caller loss, owner
change or original deadline withdraws the pending proposal. The existing Memory
page also permits protected exact-record correction/deletion.

Direct retrieval returns only the current actor's exact named fact, after source
and revision checks. No full store is injected into the model. NativeMemory
turns/replies and reserved memory-command turns are excluded from later model
dialogue retrieval, preventing correction/deletion from reinserting their old
values through this derived context. Deletion clears all stored revision/event
bodies and dependent notification bodies atomically. Original accepted command
history and offline backups remain independently retained; this operation does
not claim to erase them or unrelated model answers.

Known credential/authentication labels (password/passcode, PIN, MFA/OTP, API or
private keys, access tokens and recovery/seed phrases) are rejected before the
new command is persisted as an accepted turn. This is a narrow label guard, not
exhaustive semantic secret detection. Unknown/ambient speech has no entry point;
no audio, screenshot, page body or enrollment sample is added by this feature.

Direct two-turn save/read, restart read, explicit update, exact approved delete,
post-delete retrieval and cross-actor checks remain required live evidence.
Source/static/build results alone do not satisfy A14–A16.

Value-bearing native recalls carry their exact memory ID/revision in the native
publication owner. Protected Settings corrections/deletions withdraw the actor
's queued/published replies immediately. Accepted updates and approved forgets
withdraw earlier pending replies and published value-bearing recalls before the
writer changes storage. Publication and every normal-speech freshness check use
that same withdrawal signal. Save/delete acknowledgments contain no saved value;
already rendered audio cannot be erased retroactively.
## Protected accepted-history inspection

Settings History offers read-only accepted conversation history; Settings Memory
also links exact named-fact source drilldown. It uses the existing ledger worker and current protected Windows
owner, owner revision, device and paired server; a webview cannot select an actor.
The first read consumes the existing management proof into one native reader handle.
That handle expires with the original proof (at most sixty seconds), panel, challenge
or connection context; later pages cannot renew it. Every operation has its own
original twelve-second deadline and retains actual reader/owner admission through
worker retirement. Lock, hide, caller loss or changed context prevents publication.

Pages return at most twenty actor/device-matching accepted turns. The first page
freezes the actual highest rowid; subsequent native-only keyset cursors walk backward
without adding newly accepted turns. At most two hundred rowid-indexed headers are
scanned per call, with at most twenty bounded bodies. Sparse windows may return fewer
or no matching rows with a continuation; only actual keyset exhaustion means no older
history remains. Ordinary paging introduces no full-history load or model-context expansion;
the separately bounded schema30 search path is described below. Responses are bounded to 512 KiB; malformed or oversize stored rows
fail explicitly, never masquerade as an empty page.

Rows distinguish original accepted text/time/state, stored planner response and typed
provenance, and an optional linked task ID/state. A stored reply is not proof of speech
playback or a successful action; persisted playback outcome is unavailable in this
view. Historical owner-registration metadata remains historical; a paired-server
identity absent from the record remains unavailable and is never backfilled.

A named-fact source request supplies only the current memory ID/revision. The same
worker validates that exact current private entry and resolves its original accepted
turn/revision and any accepted correction source. It displays the current fact value
separately from the historical request/reply; a protected Settings correction without
an accepted-turn correction source is labeled accordingly. Deleted, changed, foreign-
actor/device or malformed references are refused. This read path constructs no live
turn, output capability or action, and cannot replay a stored reply. Text is escaped.

The UI clears loaded history/source content on lock, hide, owner/context change and
component disposal, and discards stale command completions. Selected-history deletion and indexed search use the separate contracts below;
independently retained backups remain separate work; inspecting a deleted
fact's original accepted command does not undo that fact's removal from memory retrieval.

### Manual history verification (not yet performed)

Use the ordinary owner's existing Windows Hello management flow, then inspect
History after genuine accepted conversation turns. Open the first page and move
to older pages; accept a new turn between pages and verify the existing reader's
frozen history window does not gain it. Reopen with a fresh protected reader to
see the new turn. A sparse page with continuation must not claim history ended.

Speak a real named-fact save, inspect its accepted source from Memory, then make
an explicit correction and inspect again. Confirm the current value is separate
from the unchanged original accepted request and any accepted correction source.
Check absent replies/playback evidence remain labeled unavailable. Lock Windows,
hide Settings, change owner/context, and allow the original proof to expire;
loaded content must clear and stale pending reads must not republish it.

These are manual product checks, not an automated runner or synthetic-turn ingress.
If genuine accepted records or the owner's live management proof are unavailable,
record the corresponding case as **unrun**. A silent diagnostic desktop can prove
only the visible locked/unverified controls; it cannot establish successful Hello,
accepted history, source correctness or the complete A16 deletion workflow.


## Selected ordinary conversation deletion (schema29)

History offers an explicit preparation and confirmation for one exact accepted
turn/revision. This first deletion scope supports only completed ordinary Model
answers/clarifications in the current live paired control session. It refuses
linked tasks, memory commands/sources, event-derived answers, other provenance,
incomplete records, unsupported deletion records and unknown dependencies. These
refusals leave content unchanged; this is partial A16, not general task or memory
history deletion.

Preparation reads the indexed same-owner/device/session suffix from the selected
turn through the current high-water. At most 128 records and 2 MiB of encoded bodies
may enter the closure; observing a 129th record or exceeding the byte bound refuses.
Because old planner dialogue has no source IDs, confirmation explicitly includes
removal of all later stored planner requests/dialogue/model replies in that suffix.
Later independently accepted user text is preserved. The selected accepted text
is removed too. The ticket retains exact metadata and a digest, never raw text.
A native-only ticket is bound to the original protected History reader, panel,
owner revision/pairing, live session/generation and a 30-second preparation lifetime;
confirmation and network waits never renew it or the original Hello proof.

Confirmation holds the actual accepted-work coordinator, revalidates the complete
closure, refuses live native reply/claim/retirement owners, and obtains exact
current-session controller retirement observations. Only retired or same-session
closed-and-compacted contexts with separately validated stored Model replies
(or validated prior deletion tombstones recording that eligibility) are eligible. Busy, uncertain, missing/restarted sessions or lost original authority
refuse. The final ledger transaction rebuilds and compares the closure and checks
current authorization immediately before commit. No action is cancelled merely to
make its history deletable.

Schema29 preserves accepted/plan/reply IDs, native ordinals and replay constraints.
Content-free tombstones distinguish removed accepted text from removed dependent
planner content. Replaced bodies contain only an empty JSON object. Source,
planner and reconstruction readers reject tombstones; History skips deleted
accepted records and explicitly labels retained user text whose model response
was deleted. Neither tombstones nor stored metadata recreate a live capability.

This is logical product deletion and exclusion from retrieval. The writer verifies
SQLite secure_delete is enabled before mutation; a post-commit WAL truncation is
best effort and its result is reported separately from the committed deletion.
It is not forensic erasure of old pages, filesystem snapshots, process memory or
offline backups. No existing backup is rewritten. Settings projections clear on
confirmation, lock/hide or expired authority. Development must never invoke this
operation against user data or manufacture accepted records for proof.

Validated schema29 tombstones are content-free members of subsequent same-session
suffix closures. They retain exact original request/turn/reply identities and the
original native transaction's ordinary-Model eligibility; they do not reconstruct a
reply or confer authority. Every member still needs a fresh current-session remote
retirement result and dead native content owners. The digest includes the existing
tombstone and any surviving accepted body. Selecting a dependent-only tombstone
removes only that turn's surviving accepted text; previously selected rows stay
removed. Later independent originals remain available in History with an explicit
response-deleted state and are never reused as planner dialogue.

The native History reader carries a content-generation value. Any deletion commit
invalidates that generation and emits `conversation-history-changed`, so queued
old reads and WebView callbacks cannot republish removed content. Preparation and
confirmation retain the actual native reader and owner guards; confirmation also
retains the accepted-work coordinator. The original Settings challenge, Hello
proof, panel, connected session generation and action epoch remain current through
the ledger authorization. A dropped caller withdraws publication but does not
release actual blocking work. Confirmation consumes its ticket and reads the same
pinned controller retirement API; unknown or missing sessions fail closed. It
never cancels work to manufacture retirement.

Manual proof remains unrun: after genuine ordinary accepted replies have retired,
open verified History and prepare one current-session ordinary turn. Check the
exact selected text and later response cascade before confirming. Reopen History
to inspect absence of the selected original, retained later independent originals,
and explicit deleted-response labels; later select one such retained original to
check repeated deletion. Lock/hide or let the original confirmation expire before
confirming to observe refusal. Task/fact-linked, uncertain, old-session and oversized
closures must refuse without deletion. Do not use this procedure on content the
owner wishes to retain. Source/static checks are not proof that deletion ran.

Each preparation/confirmation operation also keeps its own original twelve-second
management deadline, bounded by the same original thirty-second ticket and Hello
expiry. This does not extend the ticket. A successful ledger commit invokes its
owned content-invalidation callback before sending the result, even if the caller
has gone away; receiving the result is not the invalidation owner.

## Protected indexed history search (schema30)

The existing protected History reader also offers real SQLite FTS5 search and
structured exact turn/task/app UUID lookup. Search indexes only the authoritative
accepted original and its stored response text, not copied planner dialogue or
ambient input. Private FTS content and token tables are plaintext in the same
protected local database, like the existing source ledger; they are not DPAPI
ciphertext. Search produces historical projections only, never model context,
live claims, grants, or replayable output. Exact app lookup means the validated
Application.app or Prompt.binding.app on a linked accepted task; it does not search
installed apps, infer aliases, or cover every app mention in conversation.

The native query accepts at most256 UTF-8 bytes and eight alphanumeric terms,
quotes every term into the fixed FTS grammar, and ANDs the current native owner/
device scope. No raw MATCH syntax, wildcards, ranking across owners, LIKE fallback,
snippets or dynamic SQL identifiers are exposed. Results use authoritative source
readers and validate actor/device again. At most20 results are returned per bounded
page with original query/highwater/keyset and content generation retained in an
opaque native cursor. Exact UUID lookup does not accept a WebView actor.

Schema30 adds a stable accepted-rowid document map, ordinary-content FTS5 index,
and current-owner/device backfill checkpoints without rebuilding source tables.
The accepted, reply, native-observation and linked-task transactions update their
exact index row; schema29 deletion removes the selected document and replaces
collateral documents with accepted-original-only projections in that same
transaction. Later selection removes the surviving original's index too. Both
SQLite secure_delete and FTS5 secure-delete are enabled; this still makes no
forensic, memory-erasure or offline-backup guarantee.

Existing history is indexed only through explicit bounded current-owner/device
maintenance: up to 200 headers and 2 MiB of encoded source bodies per operation, resumable
through source-sized transactions that atomically update each document and its
checkpoint. Completed prefixes survive a later interrupted unit; a malformed unit
never advances its checkpoint. The operation reports budget exhaustion separately. New source writes index immediately. Coverage is explicitly incomplete
until the original selected backfill highwater is exhausted; an empty partial
search is not evidence that older history lacks matches. No startup full-corpus
scan or hidden background index rebuild occurs. Malformed sources fail without
advancing the checkpoint.

Every search and backfill retains the original History proof/panel/current binding,
reader owner and twelve-second operation budget. A scoped SQLite progress handler
adds a fixed VM-step/time budget across corpus operations; LIMIT alone is not the
work bound. The handler observes its immutable original deadline, while actual
native authority is checked before/after database work. It is cleared before
transaction cleanup. Unsupported FTS or exhausted work budgets are explicit errors,
not empty successful searches. Indexed historical copies are never authority and
lock/hide/expiry/content deletion invalidates the same reader and UI projections.

Search cursors also freeze the current per-owner/device index revision and coverage
snapshot. A source completion, link change, deletion or backfill changes that
revision, so continuation refuses and asks for a new search rather than silently
changing its cohort or claiming current coverage belongs to an older cursor.
Search SQL has a 200,000 VM-instruction cap; backfill has a 2,000,000 aggregate cap
and stops before beginning another unit once fewer than200,000 instructions remain.
The actual original operation deadline still applies. This bounds executed SQLite
VM work; it is not a claim that filesystem calls are forcibly preempted. Every
actual operation retains its ledger owner through cleanup.

The search migration also adds an exact `(actor,device)` accepted-history index;
its implicit rowid tail supports owner-scoped maximum/range seeks across sessions.
The existing `(actor,device,session)` index alone is insufficient for this scan.
SQL progress deadlines are the earlier of the original operation deadline and
the original Hello proof expiry, never a new twelve-second authority grant.
