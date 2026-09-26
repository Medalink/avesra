# Bounded Local Studio reasoning adapter

## Exact-container qualification adapter

Two reviewed profiles are supported: `legacy_35b` retains the original35B image,
arguments and16,384-token capacity; `owned_27b` is the separate text-only27B image
and4,096-token capacity in the adapter setup. The profile is private fixed
configuration, never chosen by a prompt. Each pins repository/revision, immutable
image, exact entrypoint/arguments, loopback port, model label and capacity. No
arbitrary image/model/argument combination is supported. The owned profile also
requires its unchanged bounded-start script and Docker memory/swap/CPU limits on
every observation; the pre-load resource gate is independent of loaded-artifact
qualification. A text-only instance must not be presented as vision capability.

The current owned recipe reserves768MiB of KV cache. Retained actual engine
initialization output measured0.57GiB required for its4096-token context; the
earlier256MiB recipe failed after loading weights. The new immutable guard/image
changes that allocation only, retains the same memory/CPU/headroom limits, and
requires a fresh image-bound capture before a new controlled load. A successful
image build does not prove the engine initialized or served a request.

The owned profile may select the separately owned loopback controller at
`http://127.0.0.1:18080/` (or its IPv6 equivalent). This is an explicit fixed
configuration alternative, never discovery or fallback. Its copied source fixes
only the inspected NVIDIA telemetry omission of `memory_shared`; existing Local
Studio source, controller, data, credentials and engine remain unchanged. Fresh
private keys and a dedicated data/SQLite directory isolate its instance roster.
The setup producer copies and hashes source/dependencies and records that one
delta; it never imports an existing controller database or adopts another engine.

This section supersedes the historical dormant-source checkpoints below. The
installed engine emits nullable `prompt_token_ids`, `prompt_text` and choice
`token_ids`, an optional numeric EOS `stop_reason`, and `reasoning` deltas. The
strict parser accepts only null for the token/prompt trace fields, only the two
pinned EOS IDs at a stop terminal, and bounded discarded reasoning. Unexpected
fields, strings/custom stops and non-null trace data still reject. No field is
used as a readiness flag or substitute for the complete stream terminal.

Startup opens a private configured driver only when `reasoning.json` exists;
missing configuration leaves reasoning unavailable and invalid configuration
fails startup. Version2 pins the inspected engine image, immutable model revision,
absolute model directory and credential-file reference. The instance name remains
explicit configuration so a separately owned Local Studio instance can qualify
without replacing unrelated work; its exact current container ID is always
resolved from that owner's fresh record. No unconfigured instance is selected.
The file schema and controlled capture procedure are in
[the adapter setup](../services/reasoning/README.md).

Each accepted planner request performs fresh metadata and an owned exact-container
inspection/tokenization, then persists its existing actual Jobs lease before the
only possible generation POST. Before that POST the same helper rechecks the
exact engine process identity, artifact observations and prompt budget. A failed
or missing capture never sends inference. No configured driver, status result or
artifact file independently creates a native accepted turn. The existing private
job uncertainty row still blocks replacement; this adapter adds no clear/reset.

Helper failures are finite typed metadata: `capture_missing`, `capture_invalid`,
`artifact_changed`, `controlled_load_required`, `engine_identity`,
`context_capacity`, `stream_drain`, or `helper_unavailable`. The server records
only the corresponding enum label; HTTP failure remains unavailable. Exceptions,
paths, tokens, prompts, responses and credentials are excluded from diagnostics.

The fixed loopback request helper uses a distinct name from Python's
`http.client` module. Engine inspection must reach the actual authenticated
`/v1/models` response before tokenization; successful pre-HTTP file/process checks
alone do not establish adapter readiness. The corrected helper retains every
identity, capacity, deadline and terminal check.

Helper filesystem reads are explicitly bounded: each reviewed serving source is
at most1MiB, generation defaults64KiB, proc command/stat metadata16KiB, proc comm
256bytes, boot identity128bytes, private capture128KiB and configuration8KiB.
Read actual bytes through limit-plus-one, not only an earlier file-size check;
refuse symlinks and non-regular inputs. Process discovery visits at most4096 proc
entries and opens at most two matching pidfds. Artifact directory enumeration is
bounded to258 entries, including the two ignored metadata names. Operator hashing
reads each model file only through its original measured size and rejects growth.

Entry-point review: `/planner` is covered by fresh qualification plus its existing
paired accepted-source authority; `/planner/cancel` withdraws publication while
the retained actual job drains; normal TTS still requires the resulting stored
reply and separate selected-voice/output authority. Generated-voice preview,
startup greeting and exact browser commands are unaffected. Artifact capture is
an explicit operator preparation command with no lifecycle calls. There is no
automatic retry, proxy fallback, imported-row reply producer or readiness switch.
The user's no-test requirement overrides synthetic test/fixture work: source
review, formatting and compilation are the checks; controlled-load observations
and accepted-response proof remain separate required runtime evidence.

The configured version2 adapter uses Local Studio for discovery and lifecycle but
does not send inference through its dynamically routed proxy. A fixed bundled
helper executes in the exact full Docker container ID returned by current owner
metadata. It addresses only that container's loopback vLLM API. An already-open
container namespace cannot become a replacement container's endpoint. API-server
and EngineCore process IDs/start identities are observed and retained with Linux
pidfds across tokenization and generation; exit, replacement or missing evidence
denies publication. Commands, code, URLs and credentials are never model-selected.

Loaded-artifact evidence requires a controlled observed load. The operator first
stops the selected engine through Local Studio, then runs the Avesra artifact
capture producer against its pinned Hugging Face repository/revision. The producer
verifies package bytes against immutable upstream file metadata and records exact
device/inode/size/ctime and SHA-256 observations in a private record. It refuses a
running selected container. Local Studio must then start a fresh container with
the pinned image and that exact read-only model mount. Runtime qualification
requires its creation/start after capture, the unchanged exact package records,
the actual API-server model arguments and matching live model/tokenizer metadata.
An existing process, old timestamps or an operator-written qualified boolean
cannot establish this path. No automatic model stop, restart, swap or recovery is
part of normal planner admission; the controlled load is an explicit setup step.

The initial deployment is narrowly pinned to the inspected vLLM image and serving
source hashes. Every request uses one choice, no tools, no custom stop strings,
fixed output limits and the same explicit chat-template options for `/tokenize`
and generation. The actual loaded tokenizer must report the exact prompt count
and current capacity; reserve512 output tokens before any inference POST. Runtime
checks cannot infer token capacity merely from input bytes or a recipe label.
The reviewed no-custom-stop path reports completion only after EngineCore output
finishes; a valid finish, DONE and EOF are all required by the existing parser.
Unexpected serving code, model arguments, generation defaults or terminal shape
remain unavailable. This compatibility boundary must be re-reviewed on upgrade.

One actual Rust coordinator retains the helper process, original request budget,
durable Jobs lease and bounded output through completion/cancellation. Caller
loss withdraws publication but does not abandon the drain. A helper failure,
timeout, process change, unknown EOF or invalid terminal keeps the durable job
uncertain; it never authorizes a retry. Native accepted-source authorization
remains separately mandatory before transmission and publication. Fresh
qualification is obtained per request; no startup token silently expires into a
permanent or bypassable readiness state. Source/build success is not the required
controlled-load or end-to-end spoken-answer evidence.

This source slice implements the private answer adapter behind durable native planner claims. It does not call Local Studio, load or swap a model, activate voice, or turn arbitrary received text into acceptance during implementation. The public paired route remains separate and must bind active actor registration and durable replay ownership before invoking this adapter.

## Configuration and deployment identity

Only private Avesra configuration selects the installed Local Studio controller endpoint, one exact local recipe/model, artifact revision and credential-file reference. Neither the native accepted text, planner result, extension nor frontend may choose them. Use bounded canonical HTTP loopback on the configured Spark for the existing local proxy (or separately verified TLS identity), with environment proxies and redirects disabled. Restrict recipe/model IDs to one plain bounded local identifier, excluding provider/slash routing. Credentials are loaded from the configured private file, bounded and never logged or returned. Configuration absence or unsupported deployment identity remains unavailable.

Deployment readiness must be derived from actual current owner metadata and the configured artifact/model. A recipe being listed, its active flag, `/status` running label, an HTTP 200 or a static configuration revision is insufficient. The inspected Local Studio `models/routes.ts` sets `created` from the current wall clock on every request; this is not an engine incarnation. Its active recipe is a process/recipe match and can remain active if the inference model query fails. The inspected status layer can synthesize a process with PID zero. These values must not clear uncertainty or permit a second request after backend ownership becomes unknown. The installed compute-instance metadata supplies bounded live-incarnation identity as described below; loaded-artifact and backend-terminal semantics qualification remain unresolved.

## Actual request ownership

One actual reasoning job owns the configured adapter. Before transmission, durably record a private server-generated request UUID, deployment identity and outstanding state under the existing controller's exclusive actual writer. Do not store raw accepted text in this uncertainty record. The original fixed planner deadline includes preparation and transmission; it is never renewed by model readiness checks or SSE activity. A cancelled client immediately loses publication, while the actual adapter may continue draining/discarding the same upstream response solely to establish terminal completion. It cannot admit a replacement request merely because the HTTP waiter, task, timeout or process ended.

An incomplete SSE EOF, socket loss, invalid terminal or interrupted request leaves the backend execution uncertain. Persist that state and withdraw further model admission across Avesra restarts. Known normal completion may retire the outstanding execution record even when the result itself is invalid/unpublishable. Recovery needs independent evidence that the exact backend job has ended, or a verified actual engine incarnation replacement; an elapsed timer, rewritten config UUID or Avesra restart is not recovery. No model restart/reset, eviction or serving-engine change is an automatic recovery mechanism.

## Bounded request and SSE parsing

Build the request from current accepted text, the bounded same-context native-selected completed dialogue specified in [planner-driver.md](planner-driver.md), an immutable configured system instruction for typed JSON, and the exact configured local model. No tool definitions, arbitrary request options, external context or authority IDs are sent to the model. Actual tokenization includes every dialogue message and the current utterance. Use explicit output-token/context limits. Treat its JSON as untrusted content, require the strict answer shape and bounded UTF-8 result, and preserve its provenance as assistant output rather than observed fact.

SSE parsing is incremental and bounded before allocation: cap each line/event, total transport bytes, result text, idle time and total original deadline. Accept UTF-8 boundaries across transport chunks; only parse a complete bounded event. Keepalive comments do not count as answer data or extend the fixed deadline. Require one stable response ID/model, exactly one choice index zero, allowed assistant role/content deltas, a single finish reason and the final `[DONE]` marker. Reject tool/function calls, extra choices, model/ID drift, malformed data, JSON error objects (including after HTTP 200), nonterminal/truncated/length finish and missing terminal. Usage-only events may carry bounded metrics but cannot establish completion. Unknown stream data must not become a successful answer by omission.

Distinguish transport completion from response validity: a structurally complete upstream terminal may prove the stream ended while its answer is rejected; neither implies a tool effect. An error body, abrupt EOF or locally generated timeout is not proof that engine/GPU work ended. The actual installed Local Studio abort propagation proves upstream HTTP cancellation only. The adapter must preserve the durable uncertainty boundary until its deployment-specific completion semantics are qualified.

## Publication and source boundary

Only a validated complete result bound to the exact original accepted planner request/current action authority may become the strict public controller reply. The native ledger must still finalize it against its opaque original-time claim. A response arriving after Stop, pause, lock, disconnect, actor/device revocation or action epoch change is discarded. Capture-only mute does not revoke accepted action authority. No automatic output occurs: normal TTS requires the separate opaque stored reply and current playback lease.

Source implementation and static checks are not live readiness. Current actor setup, durable planner claim helpers and private streaming TTS are compiled components; the qualified native producer, actual configured reasoning route, engine lifecycle evidence and normal TTS bridge remain incomplete. No fixture/corpus/evaluation harness or automated test is introduced under the user's specs-and-code instruction.


## Installed deployment-owner metadata boundary

Read-only source inspection of installed Local Studio on 2026-09-25 found GET `/compute/instances` returning `{instances:[{record,state}]}`. `controller/src/modules/compute/contracts.ts:154–203` defines local process references (PID/process group/session/start token), Docker references (container/daemon/executable identity), instance name/node/engine/recipe/runtime/port/devices/nonce/timestamps and lifecycle states. `compute/lifecycle.ts:85–106` checks launcher liveness before engine health for ready; lines287–294 produce the roster. The initial adapter accepts only exact configured local Docker identities with nonempty actual container incarnation values, and rejects process, null/reserving, remote, pinned or pending handles. Bounded strict metadata parsing and one exact matching instance are required.

These source files and compute routes were reported clean against installed d1abdef. The separately inspected `active-model.ts` is an existing user-patched file (SHA-256 `67e499bf068bf622a78c1adbbd6ff4ab03cd22ab741e67259a5df3e2012a4d08`); synthetic PID-zero and broad status-liveness observations refer to that installed source, not an unmodified upstream release. Existing patches were preserved. No metadata HTTP request was made.

An InstanceRecord supplies no frozen artifact digest or loaded model path. Its ready state and real incarnation therefore cannot promote configured artifact revision into observed load evidence. The eventual runtime adapter needs separate deployment-owner artifact evidence bound to that exact incarnation; otherwise artifact qualification remains unavailable. Changed nonce/ref/config or a missing old roster record alone cannot clear an uncertain old job. No whole-instance stop/cancel/restart API is part of the Avesra reasoning adapter.

## Parser source checkpoint

The private parser now bounds 64KiB input chunks/lines/events, one MiB total stream, 4096 events, 64KiB accumulated encoded answer and discarded reasoning, with final plain response limited to8192UTF-8bytes. It accumulates UTF-8 across transport chunks and parses only complete SSE lines/events. Parser failure is terminal/poisoned; a later DONE cannot repair it. Stable response/model identity, one choice0, explicit finish and DONE are required; keepalive comments, optional bounded usage and optional bounded service metadata cannot substitute for them. Tool calls, model/ID drift, unexpected fields/events, malformed data, truncated streams and oversized data reject.

Only EOF after a valid terminal produces a private CompletedStream transport observation. Length/content-filter completion or invalid Answer/NeedsInput JSON yields an invalid response while retaining the distinction from missing stream terminal. This is not a public reply constructor or GPU readiness proof. The parser is source-compiled only: no synthetic stream, fixture runner, endpoint, model or accepted text was executed.


The first metadata parser deliberately supports only configured local (`nodeId=self`) vLLM Docker instances. The installed compute contract declares runtime `docker`; process references are not enabled merely because the broader reference union contains them. Unsupported selected references fail closed, while other bounded unrelated roster entries do not become candidates. It caps the response/roster, requires one exact name/recipe/port, ready state, full container/daemon/executable identity, nonce and bounded distinct devices. Its five-second freshness starts before metadata I/O and includes parsing. A canonical fingerprint identifies only this owner metadata record; it is not artifact evidence and cannot clear an outstanding-job uncertainty record. `artifact_qualified` remains false until the separate actual artifact evidence adapter exists. No metadata request was issued.


The outstanding-job store uses one exact version-1 table with at most one row and a process-lifetime exclusive OS file lock acquired before SQLite initialization/recovery. Its immutable request UUID/model/incarnation-fingerprint metadata and outstanding/uncertain state contain no prompt, answer or credential. Empty new storage initializes transactionally; occupied unversioned, unexpected schema objects/constraints or oversized/malformed records reject. Before any model send, begin creates the row and returns an opaque same-owner lease. Dropping that lease leaves the row blocking new work. Restart converts outstanding to uncertain; there is no configuration-reset, replacement-nonce or timer-based clearing method.

Only the original same-process lease plus its privately correlated parser completion can retire that exact record. Parser construction binds the actual server-generated job UUID/model before reading the owned HTTP response; the transport-complete observation cannot be reused across jobs. Explicit uncertain retirement preserves the row and consumes the lease. This store is resource/lifecycle bookkeeping, not artifact qualification or permission to make a request. Actual HTTP ownership and verified artifact admission must wrap it before the first send.


The metadata and outstanding-job bookkeeping source now compiles without making HTTP requests. Job storage validates exact schema/object count and bounded records, uses a two-second SQLite busy timeout, performs exact-record compare-and-swap for uncertainty, and holds the exclusive OS lock until the connection owner is dropped. The configured existing private Avesra directory is required; no Local Studio file or model is modified. Successful begin persistence can outlive a cancelled caller and intentionally blocks replacement until its exact result is known. Completion requires the same owner lease, private request UUID/model and the parser's correlated complete-stream observation. There is no generic clear/recover command. Actual backend-job ownership, qualified artifact proof and the paired reasoning route are still pending; these source checks do not create or migrate a jobs database.


The jobs/lock final-path guards use direct symlink_metadata, explicitly distinguishing NotFound from existing regular files and rejecting symlinks (including dangling links), directories or other non-files before open. This is within the existing private-directory/same-user local-code boundary; it does not claim protection against arbitrary same-user concurrent path replacement.


## Configured HTTP/drain continuation

The next transport source reads only fixed `reasoning.json` under the existing private Avesra directory. Version1 configuration selects a canonical loopback controller URL (HTTP127.0.0.1 orIPv6loopback on8080), exact local instance/recipe/model/port, expected artifact revision and an absolute private LocalStudio environment-file reference. Only the bounded LOCAL_STUDIO_API_KEY assignment is extracted, without expansion or diagnostics containing values. Unsupported/missing configuration remains unavailable. This source does not read that credential file during implementation.

Transport admission requires a native-only qualified-deployment value bound to exact config digest, actual incarnation/model and loaded artifact, plus qualified terminal semantics. It has no Deserialize, public constructor, config boolean or activation route while the actual qualification evidence is missing. The driver still implements the complete bounded I/O and ownership path so later qualification cannot bypass its lifecycle rules.

One try-acquired actual job permit is retained by a spawned coordinator independent of the invoking future. Metadata reads, durable job begin, model send, bounded SSE drain and exact job retirement all retain it. Caller loss withdraws publication but does not abandon the running reader; incomplete/deadline/transport exits retain durable uncertainty before releasing local resources. The original admission Instant/remaining budget governs all work, with a20-second idle cap inside that total. No keepalive, retry, readiness query or result persistence renews it. Successful complete-stream retirement may still produce an invalid/unpublished answer. The public paired route and registration/replay/current-action authorization remain separate required source work.


Qualified deployment admission must additionally bind the exact request routing to the qualified engine. The installed LocalStudio proxy dynamically resolves current recipe/served-name; matching metadata before/after a stream and matching its model label do not exclude routing drift or an ABA replacement. Therefore the qualified-deployment constructor stays unavailable pending actual artifact, request-routing and terminal-semantics evidence. The transport explicitly disables reqwest protocol-level retries as well as redirects/proxies; private accepted requests are not automatically replayed. Arbitrary reqwest chunk grouping is sliced into bounded parser pushes with the same total/deadline checks.


## Configured transport source checkpoint

The private driver now implements strict fixed configuration, bounded private credential extraction, one actual spawned request owner, fresh exact metadata checks, durable begin before send, one non-retried model POST, bounded SSE drain and exact completion/uncertainty persistence. Only pinned already-locked reqwest0.12.28 and zeroize1.9.0 dependencies were added to the server manifest; the lockfile changes only that package's dependency edges. Client loss suppresses publication without releasing an in-flight send/drain owner, and blocking credential/SQLite work retains the actual permit. Every preparation/send/publication boundary checks original deadline/current qualification; the durable begin callback also checks caller presence after blocking I/O. Invalid/late result never becomes a public planner reply.

The current early exit after durable begin but before model send conservatively retains an uncertainty record; it does not guess recovery or issue another request. No artifact/routing/terminal/context qualifier constructor exists, and normal startup does not open Driver; the paired route can call answer only with the absent qualified capability. A real qualifier must establish configured serving context capacity for the fixed system instruction, worst-case accepted input and512-token output reserve (or exact tokenizer-budget admission); the8192-byte input bound and max_tokens alone do not prove context suitability. This remains an explicit implementation/readiness boundary, with no model, HTTP, credential or storage operation invoked by these source checks.

The actor-registration client also explicitly disables the same pinned reqwest protocol-level retry default, preserving its existing no-automatic-retry contract. This does not alter exact registration intent/reconciliation semantics or send a registration request.


The paired /planner integration is specified in [planner ingress](planner-ingress.md). Its native registration/current-session authorization callback is distinct from deployment qualification and runs on actual retained blocking ownership before model send and final publication. Normal startup deliberately provides no qualified driver. The route and private drain are source-integrated but unavailable; no request has been sent or accepted reply produced.
