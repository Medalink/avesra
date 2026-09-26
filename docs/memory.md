# Sourced private memory

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
