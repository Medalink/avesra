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
