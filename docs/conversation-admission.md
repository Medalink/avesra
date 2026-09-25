# Durable accepted conversation and planner admission

The current TurnGate can only abstain because no qualified runtime profile is available. Its opaque AcceptedConversation/Conversation types nevertheless define the input boundary for this implementation slice. Neither selected enrollment metadata, an ASR transcript, a model confidence value, a Settings permission record nor a browser result may construct accepted authority. This work adds product code, not a qualification report importer, evaluation runner or alternate manual acceptance path.

## Linearization and ownership

Preserve the gate's original monotonic admission time when AcceptedConversation is consumed into Conversation. Consumption must not renew its five-second pre-commit lifetime. Queueing, owner/profile/grant reads and storage waits share that remaining lifetime. Only the opaque native Conversation may enter the durable acceptance method; no Deserialize implementation or webview/network constructor is allowed.

Submit the consumed value to the existing NativeEffects worker's bounded management queue. That actual worker already owns the Store and its cross-process lease; do not open a second live ledger or run restart recovery while it is active. A dropped frontend/async response receiver does not release the worker or undo a transaction. Retain ownership through the actual commit and reply.

Immediately before commit initiation, a native callback rechecks the original actor, qualification/profile/grant revisions, exact acknowledged device/session and connection generation, capture/action epochs and microphone identity under native synchronization. Storage work stays outside Runtime.local. A changed context or elapsed five-second budget aborts before commit. The durable commit is the accepted-conversation boundary: after it, a microphone mute can stop incomplete input but cannot silently erase this accepted turn or cancel its already-authorized task. Lock, pause/Stop, disconnect, action/grant revocation and explicit cancellation retain their separate action rules.

An uncertain commit is reconciled by exact source identity before any new explicit work. Do not automatically create a replacement accepted turn, repeat planner submission, or infer rollback from a timeout. History records the accepted source and current outcome even when UI publication arrives after a mode transition.

## Durable record

Advance the native ledger schema with a transactional migration distinct from app-catalog, pairing and browser-scope schemas. Add a create-only accepted-conversation record with native turn UUID, actor, device/session/utterance identity, frozen capture/action provenance, microphone endpoint, profile/qualification/grant revisions, bounded accepted text, display creation time and lifecycle state. Enforce uniqueness of the exact source device/session/utterance tuple. All identities are non-nil, epochs positive, text nonempty and at most 8,192 UTF-8 bytes. No ambient audio, unknown-speaker transcript, embedding or unaccepted partial is stored.

The transcript is private accepted history, not diagnostic output. Display times are not freshness clocks. Indefinite accepted-history retention/deletion remains subject to M6; do not create a hidden short-lived cleanup policy that silently deletes accepted work. Unsupported schema or malformed required records remain unavailable rather than being recreated. Restart suspends unfinished planning and never reissues an effect from old source authority.

Acceptance returns an opaque native durable-turn handle bound to its actual stored actor/source/revision. A status lookup may return display metadata after authenticated owner filtering, but serialized metadata cannot recreate the handle or bypass stored lifecycle checks. Planning and tool-intent derivation must look up the authoritative record on the same owner before moving its state.

## Conversation is not tool intent

A valid accepted conversation may request an explanation or ask a question and therefore have no tool payload. Do not force it into AcceptedIntent, whose existing contract requires a nonempty frozen payload sequence. Keep conversation acceptance, planner work and tool intent as distinct transitions.

The planner receives bounded accepted text plus explicitly authorized context, under a fresh native-generated request UUID and fixed deadline. Its result is an untrusted typed proposal: answer text, a request for missing input, or bounded suggested actions. Model output cannot issue grants, choose authority IDs, write ledger state or directly submit effects. Browser/page content remains untrusted context even when returned by an accepted read.

Exact familiar commands may use a narrow deterministic resolver, as permitted by Plan001 section 4: for example a complete accepted request to open one exact learned application name, or set an explicitly identified endpoint to a bounded volume. It must match the whole accepted request, resolve owner-bound native identity without ambiguity and preserve exact arguments. Negated/compound/unclear text does not fall through to guessed actions. This resolver is source-only until its actual runtime recognition/intent prerequisites are qualified; no invented accuracy score enables it.

Only a native intent resolver or a concrete owner clarification may freeze the exact permitted payload sequence. It must retain accepted source/actor/turn linkage and immutable target revisions, independently of the planner's suggested payload. Before creating a task, check existing policy grants; a learned alias, owner label, browser Read scope or conversation acceptance does not itself manufacture a ledger grant. Unsupported or ambiguous resolution remains NeedsInput. Consequential actions retain their exact local approval boundary, and no submission is inferred from draft insertion.

Task creation, accepted-intent payload freezing and source-turn linkage must be transactional. Subsequent Action revisions must match that frozen sequence and current target/grant/session under the existing controller. Planning that discovers more work cannot append authority after a task was sealed. Cancellation and reconciliation preserve the existing no-replay/unknown-effect rules.

## Controller and spoken reply boundaries

The paired native companion is the source of accepted-conversation provenance. The Spark must separately bind any accepted-turn message to the authenticated device/session, registered native actor/grant context, current action authority and exact native turn/request IDs; a JSON `accepted: true` flag is insufficient. Native owner/actor registration and the accepted-turn controller route remain explicit implementation dependencies, not optional fields defaulted from a model response.

Do not send unaccepted transcripts into planning, UI history or speech. A normal spoken reply must reference an accepted durable turn and the exact current selected generated-voice identity. Conversation answers and observed action outcomes have different sources; a planner saying an action succeeded cannot replace the native observed result.

The missing public TTS bridge must bind reply UUID/revision, accepted turn/actor, paired session, independent playback epoch, selected voice audio+metadata digests and bounded text to the private streaming driver. Keep synthesis and playback deadlines separate, retain final-chunk completion/truncation distinction, pace chunks into bounded native queues and preserve post-gain submitted references/drain behavior. Preview's explicit setup lease must not be reused to speak arbitrary normal replies or bypass accepted-turn authority. Mute alone may preserve accepted work/output; deafen/Stop/pause/lock/disconnect/output-context changes revoke normal output immediately.

## Entry points and initial completion boundary

| Entry point | Required source authority |
| --- | --- |
| TurnGate consume | Current qualified profile and exact native context; original five-second budget |
| Native acceptance queue | Opaque Conversation, one actual ledger owner, original commit callback |
| Durable create/read/cancel | Exact source uniqueness, actor-filtered history, no restart replay |
| Planner request/result | Stored accepted turn, correlated deadline, model proposal only |
| Intent resolver/task transaction | Exact accepted source and arguments, native targets, existing grants |
| Native effect dispatch | Existing durable claim, current action lease and observed finalization |
| Public normal TTS | Accepted reply provenance, paired actor/session and output ownership |

The first checkpoint implements the original-budget preservation and durable native acceptance owner. It does not fabricate a qualified profile, a working planner deployment, a registered Spark actor, a policy grant or a spoken reply. Subsequent checkpoints wire actual planner/intent/TTS consumers while retaining these closed boundaries wherever live prerequisites remain unavailable. No model, microphone, speaker, app effect or UI interaction is invoked during the current restriction.

## Source checkpoint: schema 5 and actual owner queue

`Conversation` retains the original gate Instant after consumption. `Store::accept_conversation` accepts only this opaque value, validates bounded provenance/text, inserts once under the exact unique source tuple, reads back the exact inserted fields, then checks original freshness before and after the native callback at commit initiation. An uncertain storage result requires explicit exact-source status reconciliation; it never automatically reissues acceptance. Status returns actor-filtered metadata without transcript or a reconstructed authority handle. Exact ID/revision/source cancellation is idempotent for a cancelled turn, rejects answered turns, and does not claim to undo any effect.

Schema 5 is created in the existing transactional migration. Before PRAGMA, schema or recovery mutation, the new table must either be absent on an older schema or match the exact supported SQL, primary/revision/source unique indexes and column mappings on schema 5; unexpected attached triggers or additional indexes reject. A pre-existing lookalike is never adopted with CREATE IF NOT EXISTS. The touched unversioned-ledger check uses the literal SQLite internal-name prefix. Restart suspends accepted/planning/waiting-input turns; serialized recovery reads cap actual UTF-8 body bytes through a bounded BLOB slice and validate all stored identities.

`NativeEffects` supplies bounded acceptance/status/cancel commands on its existing 16-command actual owner queue. They retain the opaque input/callback through actual work even if the reply receiver disappears, and acceptance does not cancel another already accepted effect. No second Store is opened. The callback type documents a required concrete native qualification/profile/grant/session adapter; accepting an arbitrary success closure would not establish that evidence. This checkpoint has no such runtime adapter or qualified profile, no frontend/network acceptance entry point, no planner transition and no live durable acceptance. Current voice analysis still abstains. The later planner/task transaction and normal TTS consumer remain required source work.

## Next slice: exact learned-app intent to one sealed task

The first concrete accepted-intent producer is deliberately limited to a whole accepted command `open <learned phrase>` or `launch <learned phrase>`. It reads the stored text through the opaque DurableTurn on the existing ledger owner. It does not accept a supplied payload, target, executable, argument string or model interpretation. The stored transcript retains its addressing text: allow exactly one optional leading `Avesra ` or `Avesra, ` (case-insensitive), then the whole verb and phrase. Normalize casing and ASCII spaces only; reject other punctuation except alias hyphens/apostrophes, control characters, negation tokens, conjunctions and extra instruction clauses. Sentence punctuation and repeated addressing remain unsupported rather than being silently removed. Resolve the entire remaining canonical phrase against the authenticated actor's exact existing learned alias. An unresolved/unsupported utterance returns NeedsInput, not a guessed action. This deterministic path does not claim empirical speech or directedness qualification.

The same actual worker retains the AppCatalog mapping through the ledger transaction. Record alias UUID/revision and immutable app UUID/revision with source-turn linkage. Revalidate the mapping before final authorization; owner metadata never creates a grant. A native-supplied existing grant ID must be read from the ledger and must bind the actor, exact immutable app target and LaunchApp operation, with revocation false. The source turn must still be accepted, not cancelled, suspended, already linked or answered. Its original accepted device/session and action epoch must match the current native action session; frozen capture provenance is retained but current mic mute does not invalidate this accepted work.

A fresh native one-shot processing request has a fixed five-second monotonic budget from queue admission. Queueing, catalog/ledger reads and final authorization share that budget. No caller-supplied timestamp extends it. Under one transaction create source-turn/task linkage, the task, one exact AcceptedIntent payload, one Action revision/head/step, an accepted event and a sealed single-step plan. The returned task metadata is not a dispatch permit. Existing claim/execute still rechecks action authority, grants, immutable app identity and unknown-effect lifecycle. Do not invoke the native effect from this producer.

Advance the ledger schema transactionally for the new unique turn-to-task linkage. Reject a pre-existing lookalike table, attached triggers or invalid required indexes. Preserve older conversation rows and histories. Linked source state is distinct from answered: an app being queued is not a successful launch. Source cancellation must atomically mark the linked task/steps according to existing cancellation semantics, retaining running/unknown effects for reconciliation; it may not leave a cancelled conversation with a runnable linked task. Restart preserves the exact linkage and suspends queued tasks without automatically reconstructing processing authority. Status exposes actor-filtered IDs/outcome metadata only; uncertain commit is resolved through that original source, never a replacement task.

This is native product code and a dormant first producer, not a model planner or a new manual setup bypass. Qualified voice admission, real grant-management ingress, history UI, general planner answers/clarification, browser semantic actions and the normal spoken-reply bridge remain subsequent work. No live model, app or media action is invoked.
