# Paired accepted-planner ingress

This contract extends planner-driver.md and reasoning-adapter.md. It does not create a voice qualification, accepted turn, model deployment proof or browser grant. The native ledger's opaque PlannerClaim is the only intended producer. Public parsing alone does not prove acceptance; the paired native companion asserts that its claim was durably accepted under the existing same-user trust boundary.

## Request and authority

POST /planner accepts the strict planner-v4 Request, at most 32 KiB encoded JSON (the shared planner MAX_REQUEST_BYTES limit, covering escaped 8192-byte text plus context). Authentication, actor lookup, queueing, deployment checks, model streaming and final publication share the remaining budget supplied by the native claim, capped at 30 seconds and anchored once at server route admission. The native original Instant remains authoritative on reply; server receipt cannot attest time spent before receipt, so the native producer must compute remaining_ms immediately before send, bound transit and never retry or renew the claim.

The authenticated paired device must equal context.device. Its active stored actor binding must match actor and registration_revision, be unrevoked, and belong to the exact live control session/current action epoch with action permission and a heartbeat younger than 30 seconds. Frozen accepted capture_epoch is provenance; current mic mute/deafen must not revoke accepted action authority. Stop, pause, lock, disconnect, action change and actor/device revocation withdraw publication. Read the actual binding before model send and after terminal completion; a configured actor label or a status cached at registration is insufficient.

At most two public preparation owners and one actual private model owner may exist. Blocking authentication/registration reads retain actual admission until completion, even if the waiter disappears. No jobs/SQLite mutex may block cancellation. Committed actor revocation must signal matching planner ownership from the actual writer, even if its HTTP waiter was cancelled. During waiting, bounded fresh authority inspection covers out-of-process device revocation; inability to establish authority withdraws rather than preserves availability.

## Replay and withdrawal

Planner wire version4 adds a positive safe-integer `ordinal` to the immutable
context. The native ledger increments its schema17 singleton counter in the
same transaction that creates the actual one-use planner claim. The counter is
not a timestamp, frontend argument or reconstructed history handle. Rollback
publishes no claim. It survives restart and is never reset by history deletion.
All request, reply, cancellation and normal-speech contexts preserve the exact
ordinal; normal-speech wire version3 carries this changed context. Older wire
versions reject. Historical planner versions1–3 remain read-only history with
ordinal0; local compatibility validation cannot produce a current claim.

Each server control session retains a monotonically increasing admitted ordinal
high-water mark until that session ends. A new operation must exceed it; admission
consumes the ordinal before model work, including subsequent failure. The bounded
roster additionally rejects collisions of request, turn/revision or utterance
among retained contexts. Compaction never lowers the high-water mark, so replay
of any original paired-native assertion stays rejected after its roster entry
is removed. As before, these are authenticated native assertions, not signed
proof of a ledger: a modified ordinal is a new forged native assertion, and the
trusted native producer must only allocate it with a genuinely new durable claim.
No model/frontend/history path can do so. A new control session cannot reuse an
old claim's frozen session context.

The roster has at most64 retained entries, independently of total conversation
length. Remove an entry only after every retained public/private preparation or
inference owner has retired, and after any reserved speech's actual public/private
owner retires. A completed response without reserved speech additionally keeps
its original ten-second output opportunity. Withdrawal, timeout, `live=false`
or terminal text alone is not retirement. Actual private job uncertainty remains
in its existing durable ledger and cannot be cleared by compaction. A genuinely
full live roster refuses new work; it never evicts an active owner or reconnects
the microphone/control session to manufacture space.

POST /planner/cancel carries the planner version and exact context, without text or budget. Its separate bounded admission uses only the withdrawal digest established by that authenticated control session, in constant time. It can withdraw an existing matching request or consume a newer ordinal as a cancellation-before-admission tombstone; it cannot register, inspect accepted text, create a task or grant authority. A cancelled ordinal remains below the high-water mark even after its tombstone is compacted, so a late operation cannot run. An old exact cancel cannot withdraw a newer context; after compaction it may return conflict without changing state. The native owner sends one correlated cancel when its claim is withdrawn; losing the network caller also withdraws local publication, but remote cancellation remains best effort.

Cancellation and control/registration invalidation are independent of actual backend completion. Once the private driver might have sent inference, its spawned coordinator retains the model slot and durable job record while draining terminal output or reaching the original deadline. A dropped public waiter must not free that actual slot or prove that GPU work stopped. Unknown termination retains the durable uncertainty row and closes subsequent inference; no timer, changed recipe or process restart clears it automatically.

## Driver and reply

The public route requires a private configured Driver, which constructs an opaque QualifiedDeployment afresh for the exact accepted request only after the observed checks in [reasoning-adapter.md](reasoning-adapter.md). Missing reasoning configuration leaves the driver absent; invalid configuration fails startup. Existing configuration alone cannot send inference: capture, controlled new load, exact process/routing identity, reviewed terminal behavior and loaded-tokenizer capacity must all qualify. An authenticated request without that evidence returns unavailable. No frontend flag or config boolean bypasses this boundary.

Qualification binds the actual loaded artifact, exact request routing to the observed incarnation, backend terminal semantics and serving context capacity. max_tokens=512 limits output only: the exact loaded tokenizer must admit the fixed prompt, accepted text and output reserve. Before/after deployment metadata and a matching model label cannot prove routing across an ABA change; the helper retains the actual container namespace and engine pidfds.

The private driver performs native authority checks after blocking preparation and immediately before model send, then rechecks before publishing its result. Those checks use actual paired registration and session state; the original caller's live withdrawal is checked after awaited work. Once inference starts, authority withdrawal suppresses publication while the owned drain continues. Only a complete validated Answer, NeedsInput or bounded typed Proposal yields a strict correlated planner-v4 Reply. The server never executes a proposal or accepts it as a success claim; the native worker resolves it under the original accepted ownership and explicit grants as described in [planner-driver.md](planner-driver.md).

The companion must validate exact context and original claim lifetime before the same native ledger worker finalizes a stored reply. History/status cannot reconstruct the opaque handle. Answered history is immutable; later output cancellation needs its separate playback lease. Normal TTS consumes the opaque published reply under [normal-speech.md](normal-speech.md); this route does not qualify a voice profile.


## Source checkpoint

The controller wires strict /planner and /planner/cancel routes, with shared32KiB accepted-request framing and bounded decoded text. Normal startup opens the driver only for explicit version2 reasoning configuration. Authenticated requests reserve their retired identity, verify exact current registration, and then attempt fresh bounded qualification. Missing configuration performs no credential/model read; a configured driver may read credentials and read-only engine metadata/tokenization, but no inference occurs before successful qualification and durable actual-job reservation. No live qualified load or accepted response has been established by these source checks.

Preparation permits survive dropped callers through actual authentication and blocking authority reads. Authority checks validate the decoded binding and finish with a joined active-device/exact-actor SQL snapshot, followed by original deadline/current session checks. The private driver calls this native check on retained blocking ownership immediately before send and after terminal work. A 250 ms admission interval rechecks actual registration while the public request waits; a blocked inspection retains its owner, and the fixed original deadline still governs publication. SQLite inspection and in-process withdrawal are separate: cancellation never waits for an auth or jobs lock.

The actual actor writer signals matching planner entries after a committed revoked binding, independently of HTTP response delivery. Mode action changes and session destruction synchronously withdraw entries. Frozen capture changes alone do not withdraw accepted planning. Exact-context cancel installs or withdraws a bounded tombstone; the admitted ordinal high-water mark remains until session destruction, including unavailable requests, while actual retired entries compact under the rules above. The companion's opaque-claim transport, durable reply finalization and normal TTS handoff remain separate authority boundaries. Their source integration does not prove a positive voice qualification or end-to-end spoken response.
