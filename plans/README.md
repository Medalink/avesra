# Avesra implementation plans

Avesra means **A Very Effective Smart Reasoning Assistant**.

Repository: https://github.com/Medalink/avesra

Original planning baseline: `eb0189ff64b276b4f1e2d850ab3cff6979189ad9` on `main`, inspected on 2026-09-24; at that baseline the repository contained only an MIT `LICENSE`. The owner subsequently requested publication and execution. Completion planning on 2026-09-25 inspected the primary checkout `E:\Dev\Avesra` on `main`, initially at `e894025`, then concurrently advanced to `c82fed21ea259c7fd1dc6902bf2a5a07b10e231f`, with additional uncommitted enrollment/UI work. The initial live worktree list contained only this checkout. Branch/worktree names in historical checkpoints below are not current execution locations.

Model provisioning evidence and remaining serving gaps: [Spark2 setup record](spark2-model-setup.md). Approved UI direction and interactive mockups: [design](../design/README.md) (simulated prototype, not application code). The main plan includes the owner's later requests for optional client GPU acceleration and instrumentation throughout the pipeline.

Latest development checkpoint, 2026-09-26: `3958812`/`8613e48` implement simple
Personal conversation and continuous selected-device input. Matching Windows and
ARM static/release builds passed. Real isolated native previews completed before
and after cancellation; the cancelled TTS request retired while the model stayed
loaded. See [background evidence](../docs/background-evidence.md). These observations
do not prove live owner conversation, acoustic interruption, actions, or full-plan
acceptance. Plan 001 remains in progress. The canonical development executable is
`E:\Dev\Avesra\target\release\avesra-desktop.exe`; preserve the dirty primary source.

## Owner-defined completion tests

The owner must be able to speak these requests and see Avesra complete them on the actual PC using the single Spark:

1. Open Claude for the **IRIS** project, create a **new chat**, and type exactly `123`; leave it unsubmitted unless separately told to start/run.
2. Check the **latest 10 emails** in the intended mailbox and determine whether they contain a package-delivery confirmation, identifying the supporting message or honestly reporting no confirmation/ambiguity.
3. Open **X** in Chrome/Brave so the owner can post about a new app; verify the intended site/account is ready, without publishing a post.

These are mandatory live end-to-end outcomes. A model server, overlay, passing unit tests, mocked app, or dispatched click is not proof of completion. See `owner-workflows` and acceptance cases A24–A26 in Plan 001.

Handoff and next-step boundaries: [Plan 001 handoff](001-handoff.md). The owner resumed implementation on 2026-09-25 with a focus on real UI and screenshots. Later that day the owner explicitly authorized obtaining media without interruption. Follow the [background evidence procedure](../docs/background-evidence.md), including canonical physical-store verification, a non-input desktop and silent native output. The [actual default/custom preview recordings](../docs/evidence/background-preview-2026-09-25.md) prove that scoped path. Full assistant qualification remains incomplete.

## Execution order

The owner resumed full completion on 2026-09-26. The current
[acceptance ledger](001-acceptance-2026-09-26.md) supersedes stale implementation
and activation claims in the historical tables below. All required live outcomes
remain explicit; successful preview/build evidence does not close them.

Latest owner priority: ship working development features promptly for the owner's
own testing. The large voice corpus remains release validation and must no longer
block initial use for hours. Follow the explicit development-admission override
in [Plan001](001-single-spark-assistant.md#later-owner-override-ship-development-features-for-owner-testing).
The later simple Personal conversation override removes the mandatory short-check
ritual. Preserve saved local controls, native owner identity and existing action
grants; do not claim completed release evidence.

| Plan | Outcome | Priority | Effort | Dependencies | Status |
| --- | --- | --- | --- | --- | --- |
| [001](001-single-spark-assistant.md) | Build and verify Avesra on Spark2 with Local Studio, a Windows companion, optional client GPU acceleration and per-stage metrics | P1 | L, multiple milestones | Current-state reconciliation C0; scoped hardware/app qualification | IN PROGRESS / PARTIAL — isolated execution through `e986d76` implements bounded reply segmentation, directed-request contract reconciliation and the owned browser-read lifecycle. C1 qualification and C2 reasoning activation remain blocked on concrete qualification dependencies; C5 consumers and live owner workflows remain incomplete. See the [current execution review](001-execution-2026-09-25.md). |
| [002](002-voice-atmosphere.md) | Add native voice character, looping atmosphere, stereo mixing and an easy live bypass | P1 | L, staged | Existing Plan 001 preview/output ownership; normal speech retains its qualification gates | IN PROGRESS — Digital sound approved and saved as defaults; adjustable pitch and animated overlay implemented. Human audition/approval remains deferred. |
| [003](003-voice-avatar.md) | Draw each enrolled person's voice as a unique, reproducible avatar from their stored profile and a 20-word voice portrait | P2 | M, staged | Plan 001 protected owner and enrollment flow | PROPOSED |

Plan 002 records the owner's 2026-09-25 sound-design request: Digital as the default preset, Human as the second preset, an easy off toggle and permission to expand the app. It can start with explicit generated-reference preview without waiting for every Plan 001 milestone. It does not mark normal voice readiness complete or change Plan 001's existing execution status.

Read the entire plan before implementation. Execute the remaining slices in [section 15](001-single-spark-assistant.md#15-completion-sequence-and-verification-commands), retain verification evidence, and update this index only when the corresponding evidence exists. Planning does not authorize deployment, account access, production changes, or publication by itself.

## Current completion order — 2026-09-25

The planning refresh was followed by isolated execution from the owner's requested baseline commit `de2e8c8`. That checkpoint was subsequently fast-forwarded and pushed to `origin/main` at `1704d2badf40f5521de372d162d2f420e6bdcb6c`. Current follow-up work is uncommitted in the primary `E:\Dev\Avesra` checkout; the sibling worktree is retained for its build cache. The [execution review](001-execution-2026-09-25.md) records the historical source checks. The newer [background preview evidence](../docs/evidence/background-preview-2026-09-25.md) identifies its exact frozen dirty-source archive, deployed binaries, actual native UI/audio/video results and preservation of effects/background music. Source written after that archive is not covered by those recordings.

The preview checkpoint corrected hard-coded model status and a stale saved output endpoint. Both default and typed text completed through the actual Settings controls and native mixer on a separate desktop, with physical output deliberately silenced. Native qualification tooling, activity streaming and action integration remain work in progress; none closes C1, C2, C3, OW1-OW3 or overall acceptance.

The later [streaming preview correction and fresh media](../docs/evidence/streaming-preview-2026-09-25.md) also removed an observed full-utterance buffer: actual default/custom first submitted speech was 397.0/407.6ms versus 6,816.3ms in the preceding default run. These individual native observations do not establish release percentiles or full assistant readiness. The procedure is retained for future direct proof; never substitute a build or private service response for that proof.

| Slice | Required outcome | Dependencies | Current boundary |
| --- | --- | --- | --- |
| C0 | Reconcile live source/worktree ownership, normal-user store and verification baseline | — | Source/worktree ownership and immutable Windows/ARM baseline checks complete; later background runs directly verified the physical normal-user store |
| C1 | Qualified owner recognition, continuous endpointing and directed intent without a mandatory wake word | C0 | Native calibration measurement tooling and optional batch/activity-streaming adapters implemented; generated-input observations do not qualify the owner, endpointing or directness. Activation still incomplete; native producer abstains |
| C2 | Qualified local reasoning and one real accepted spoken response | C0; C1 for activation | Bounded reply segmentation and controlled engine adapter implemented. Startup is configuration-gated; no qualified deployment configuration or accepted spoken-response proof exists |
| C3 | Accepted app/volume actions with observed effects | C1, C2 | Exact app/volume coordinator, protected permissions and durable task projection integrated. Schema migration and empty read view observed; qualified ingress, typed model proposals/clarification and real effects remain unproven |
| C4 | Owned browser publication, content, cleanup and settlement | C0; C1-C3 for live admission | Source lifecycle integrated through `e986d76`; qualified task producer and concrete C5 consumer absent, so no live admission/read proof |
| C5 | Claude/IRIS exact draft, latest ten emails and X ready state | C3, C4 | Required OW1-OW3 proof remains absent |
| C6 | Sourced learning/history, routines and event-grounded chimes | C3, C4; C5 for supported workflow reuse | Remaining integration and direct proof |
| C7 | Bounded diagnostics and actual configured VPN | C3, C6 | Exact VPN surface and live proof pending |
| C8 | Profiles, packaging/recovery and complete per-stage metrics | Instrument C1-C7 as built; finalize after C7 | Bounded process-local preview/activity instrumentation implemented; actual preview measurements exposed the pre-ready buffering delay. Full tracing/profiles/recovery/release gates remain incomplete |
| C9 | Revision-matched A01-A29 evidence and eight-hour soak | C1-C8 | Release incomplete until all required direct gates pass |

C2/C4 source can proceed while C1 live qualification is pending, but activation cannot bypass it. C6 follows C5 in the recommended execution order. The owner's no-automated-tests override is reflected in the current command table and done criteria: use build/static checks and direct observations; do not implement an acceptance runner or report verifier. Old checkpoints below preserve history and must not override this current sequence.

Historical execution instructions, 2026-09-24: the owner requested `/improve execute 001`, emphasized matching the approved designs, and then instructed "no tests here either, just specs and code!" The plan records this override: no automated tests or test harnesses; use build/static checks and direct inspection, and report unverified live behavior explicitly. That execution used `C:\Users\medal\.codex\worktrees\avesra-plan-001\Avesra`; rediscover current worktrees rather than assuming it still exists.

The owner prohibited Computer Use while gaming, then explicitly authorized noninterrupting background media proof. Active-desktop input, foreground automation and ambient screen/audio capture remain prohibited. Only isolated application inspection following the runbook is permitted; a general "continue" does not expand that scope.

## Historical milestone checkpoints

| Milestone | Deliverable | Depends on | Status |
| --- | --- | --- | --- |
| M0 | Verify hardware, app surfaces, model feasibility, and setup facts | — | PARTIAL — six models downloaded/hash checked; two reasoning recipes passed initial probes; speech/app/concurrency proof remains |
| M1 | Establish workspace, contracts, build/static gates, and configuration (no tests per owner override) | M0 | PARTIAL — latest Windows Static independently passed at source `24c8ff3`; latest full bundled-asset release Build passed at `24c8ff3`; shared core/server passed independent Spark ARM64 Static at `24c8ff3`, with executor ARM release confirmed in its log; remaining contracts/telemetry incomplete |
| M2 | Pair PC/Spark; implement task policy, persistence, and cancellation | M1 | PARTIAL — TLS control, ordered ledger, exact approvals/cancellation/reconciliation, pairing recovery and bounded paired WSS ASR/speaker ingress through `2051e82`; native producer remains dormant; original-budget opaque conversation storage, schema-5 recovery and same-worker acceptance/status/cancel helpers through `f3e66a8` reviewed; narrow exact-app intent producer, schema-6 sealed task/source linkage and exact cancellation through `09d5792` passed independent Windows/ARM Static and bundled Windows release; actual PC pairing, qualified runtime acceptance, activated planner/reply producer and live effect/recovery proof remain incomplete |
| M3 | Prove local speech, speaker enrollment, intent gating, and playback | M2 | IN PROGRESS — batch speaker/ASR supervisors loaded but unqualified; final TTS load observed then stopped; protected candidate selection, recording/paired inference, stable endpoint binding and typed playback remain uninvoked; incremental ASR supervisor/client and paired streaming ingress through `2051e82` are compiled but undeployed; early-codec TTS supervisor and typed private receiver through `53a6bb5` are source/static checked; bounded native producer through `415a000` is static checked but dormant; typed conversation admission through `3654f46` remains unqualified; generated-reference WSS/native preview through `8bc7b23` passes Windows Static/release but remains uninvoked; short-response normal TTS bridge/native consumer through `87f2cfd` reviewed and Windows-static/release checked; playback-reference telemetry/speaking ribbon through `a1a7286` reviewed and Windows-static/release checked; speech segmentation, identity activation and live voice proof incomplete |
| M4 | Ship overlay, settings, onboarding, and volume/app launch slice | M3 | PARTIAL — approved design port, native settings/tray/local controls inspected; source includes native volume adapter through `95a84b1`, immutable app catalog/executable launch through `3fb18ae`, bounded shortcut/App Paths discovery through `37f3113`, protected owner creation/design-matched People card through `073564b`, and authenticated app/alias setup with the approved Learned names rows through `83e9639`; generated-voice management through `11580d0` and shortcuts/editor through `6b21ac9` pass Static/release Build; owner and enrollment management through `d53a911` pass Static; native effects, discovery, shortcuts, setup and new UI remain uninvoked; native alias resolver and immutable observed-history readback through `baee6be` pass Static; native window hints and bounded existing-instance focus through `c69cee9` pass Static/release but remain uninvoked, with the final authorization-order correction in `2b5b7d7` reviewed and statically checked; current-user package discovery, protected selection, window hints and activation through `32c241e` pass Static/release but remain uninvoked, with the activation-clock correction and explicit native executable chooser through `26583d6` reviewed and statically checked; onboarding, planner integration and usable action ingress remain incomplete |
| M5 | Control Chrome/Brave and desktop/CLI prompt inputs | M4 | IN PROGRESS — native/extension pairing, protected immutable browser selection and selected transport lifetime reviewed; exact-origin scope setup/recovery through `d6cb0e4` passed Windows and ARM checks; bounded setup document metadata, browser v5, lifecycle invalidation and design-referenced selection controls through `dc32f98` reviewed and passed independent full Windows Static, bundled release and Spark ARM64 Static, with ARM release confirmed in its log; paths remain uninvoked and rendered parity unverified; semantic page operations, accepted-action integration and native prompt drivers remain incomplete |
| M6 | Learn app mappings and routines, maintain memory, explain chimes | M5 | TODO |
| M7 | Diagnose PC issues and operate the existing VPN client | M5, M6 | TODO |
| M8 | Verify swappable deployments, optional Jev, packaging, and recovery | M7 | TODO |
| M9 | Pass real-device workflow, voice, privacy, performance, and soak gates | M8 | TODO |

Additional M2/M4 checkpoint: protected native owner registration, immutable certificate/device-bound registration intent, exact remote status/revocation and design-referenced Owner-card controls through `1bfd6bb` have been source-reviewed and passed independent Windows/ARM Static and full bundled Windows release. Registration/cancellation paths remain uninvoked; qualified acceptance and general planner/reply/normal TTS integration remain incomplete. See the execution review for the corrected cancellation lock and paired-context races.

Additional M2/M3 checkpoint: strict planner wire contracts and schema-7 opaque request/immutable reply helpers through `1f550be` are reviewed and pass independent Windows/ARM Static. They use the existing native ledger worker, original deadline and exact-turn cancellation. Configured reasoning execution, paired planner route, qualified native producer and normal speech output remain incomplete.

Reasoning adapter checkpoint: bounded SSE parsing, strict configured Docker-incarnation metadata and durable single-job uncertainty bookkeeping through `f849559`, plus owned HTTP/drain transport through `2063be6`, are source-reviewed and pass independent Windows/ARM Static and bundled Windows release. A dropped caller or restart cannot clear outstanding backend work. Actual artifact, request-routing, terminal and context-capacity qualification remains unavailable; the transport has no qualification constructor and startup cannot activate the planner driver. No request or runtime storage operation has been invoked.

Paired planner checkpoint: strict request/cancellation routes through `09bc208` are reviewed and pass independent Windows/ARM Static, with actual ARM release confirmed. They bind current paired device, registered actor, session/action and bounded retained replay identities, and propagate committed revocation independently of reply delivery. Startup deliberately leaves the reasoning capability absent; no model request is possible through it. Native opaque-claim orchestration through `4d68443` is reviewed and passes independent Windows/ARM Static and bundled Windows release: original-deadline TLS, exact owner/pairing/intent rechecks, same-worker failure retirement and cancellation-aware durable reply handoff. A distinct published-reply handle and continuous bounded source-withdrawal registry through `7d6a34b` are reviewed and pass Windows/ARM Static. Controller completed-response reservation and incremental normal-speech transport through `67d4afb` are reviewed and pass independent Windows/ARM Static, with actual ARM release confirmed. Final private-send authorization hardening through `28790e6` is reviewed and independently ARM-static checked, with actual ARM release confirmed. Native PublishedReply consumption and actual output ownership through `87f2cfd` are reviewed and pass full Windows Static and bundled release Build, plus independent ARM Static with actual ARM release confirmed. Playback-reference telemetry and design-referenced speaking presentation through `a1a7286` are reviewed and pass full Windows Static and bundled release Build. Qualified acceptance, long-response segmentation and live speech remain incomplete; rendered parity remains unverified.

Accepted browser reading checkpoint: strict partial-only excerpt/action/source/document contracts and existing DispatchPermit correlation helpers through `59f79b7` are reviewed and pass independent Windows/ARM Static, with actual ARM release confirmed. These are data validation only. The bounded, self-contained isolated excerpt/retained DOM guard and shared actual Chrome-job owner through `479ea04` are source-reviewed and independently extension-type/build checked, including the correction excluding non-rendered media fallback text; the extractor is not invoked or registered. Schema-8 external ownership validation/recovery and all-effect claim exclusion through `d2083c0`, plus borrowed same-owner publication/never-published retirement helpers through `311ee97`, are reviewed and pass independent Windows/ARM Static, with actual ARM release confirmed. The live publication helpers have no native caller; the actual read work channel and opaque result publication remain unimplemented. Authenticated recovery retirement is recorded below. Settings metadata does not grant content access. Generic excerpts cannot establish mailbox latest-N/account completeness.

Native browser receive checkpoint: actual pipe receiving, one-use authentication token, original-deadline verification seal and contiguous sequence/observation ownership through `1fe3367` are source-reviewed and pass independent full Windows Static and bundled release Build. Existing v5 setup/status wire behavior is preserved in source; actual transport remains uninvoked. This provides a concrete future settlement capability boundary without enabling a page read.

Browser receipt prerequisite: schema-9 immutable retirement metadata validation/indexed lookup, unused settlement data and verified native pairing actor/app provenance through `a05f6e2` are reviewed and pass independent Windows/ARM Static, with actual ARM release confirmed. No receipt writer, authenticated retirement or active semantic read is enabled; the native work channel remains required. The subsequent coherent v6 wire/authentication switch through `7bb10d7` is reviewed and passes independent full Windows Static and bundled release Build, plus independent ARM Static with actual ARM release confirmed. Actual pipe receipt can mint an opaque settlement proof, but desktop status still publishes null read/ack fields and no retirement occurs. See the execution review for the corrected borrowed recovery lease specification.

Browser recovery checkpoint: exact borrowed live/recovery retirement, transactional immutable receipt writing, actual authenticated settlement mailbox and asynchronous read_ack delivery through `c9962b1` are source-reviewed and pass independent Windows/ARM Static and bundled Windows release, with actual ARM release confirmed. Native resource state blocks metadata through unresolved ownership and worker failure; monotonic generations prevent old setup requests from returning after recovery. A reviewed failure-path correction retains a blocked state after uncertain receipt readback. Live ReadPage publication, immutable target resolution, extractor activation and qualified acceptance remain incomplete; no runtime path was invoked.
Native document target checkpoint: private immutable metadata minted only from authenticated exact revalidation through `0e4a57c` is reviewed and passes independent full Windows Static. A connection retains at most 16 identities without eviction/rebinding; navigation and withdrawal stale them permanently. Review corrected actor withdrawal races in both pending replies and pre-await preparation. The resolver currently validates Selected metadata only; no accepted read publication or browser invocation is enabled.
Dormant read-channel checkpoint: original-budget one-use preparation, same-borrow historical receipt lookup, actual-worker sender/take-once receiver and exact resource reservation through `8b2a565` are reviewed and pass independent Windows/ARM Static, with actual ARM release confirmed. Review corrected terminal worker exit, failed-attempt retry and exact-generation exhaustion behavior. No offers are emitted, no desktop consumer runs, and no dispatch/read publication is activated. Full native preparation, withdrawal, content/settlement integration and the extension outbox remain required.
Native preparation consumer checkpoint: the event-driven runtime receiver, retained actual blocking owner, current registered owner/scope/pairing checks, immutable target reservation and exact lifecycle withdrawal through `e433b34` are reviewed and pass independent full Windows Static, bundled release Build and ARM Static, with actual ARM release confirmed. Existing state readers do not initialize runtime storage. No offers are emitted and ReadPage dispatch/publication remain dormant; live content finalization, extension settlement outbox and qualified acceptance remain incomplete. No tests or runtime browser/device/UI operations occurred.

Dormant read-finalization checkpoint: exact authorized-request retention, schema10 digest/count provenance, current settled-content finalization and opaque authenticated native content through `24c8ff3` are reviewed and pass independent full Windows Static, bundled release Build and ARM Static, with actual ARM release confirmed. Borrowed result consumption retains the original execution/checker; no escaping authority or active browser read is enabled. Review corrections cover adjacent transactional validation, stale-session discard and dropped-result withdrawal. Successor registration, active read publication/loop, extension outbox and qualified producer remain unfinished. No tests or runtime browser/device/UI operations occurred.

## Deferred

Specific home-light integration, multiple-Spark distribution, additional desktop operating systems, and remote/mobile clients. Optional client GPU inference is now in scope, alongside a Spark-only baseline and a Gaming profile that releases client inference resources.

## Approaches considered and rejected

- One monolithic model performing every role: prevents independent tuning and makes resource contention harder to manage.
- Requiring the RTX 5090: conflicts with the single-Spark and gaming requirements.
- Cloud reasoning fallback beyond optional Jev: outside the agreed assistant design.
- Direct Claude/Codex model APIs or agent delegation: user requested opening applications and typing into their prompt inputs.
- Dedicated Gmail/X API integrations as the first path: user requested website interaction through Chrome/Brave.
- Blind replay of screen coordinates: does not reliably survive window changes or app updates.
- Retraining model weights as the memory system: unnecessary for remembering app mappings, preferences, and workflows.
- Raw microphone/screenshot recording history: user selected transient media with indefinite accepted text/task history.
- Unsolicited spoken suggestions: user selected quiet background operation, with small learning/action sounds as a later refinement.

## Completion boundary

Implementation is in progress, with intermediate source/build/native UI evidence recorded in [the execution review](001-execution-review.md). A functioning voice assistant has not been demonstrated or deployed. The owner authorized model setup through Local Studio on `ssh spark2`; that target is reachable, all six selected model packages are downloaded and hash checked, and earlier reasoning recipe verification is recorded in the setup evidence. The earlier failed `spark` alias is superseded. The owner chose dedicated local speech/speaker drivers alongside Local Studio. No Avesra microphone, enrollment, desktop-automation, or end-to-end performance proof exists. Downloads, build checks and static UI inspection cannot satisfy the owner workflows.
# Current execution priority

The2026-09-26 owner request supersedes manual voice calibration: normal launch
should start personal conversation and learn the voice during natural dialogue.
Existing owner/enrollment is reused. Implemented work is on
`codex/simple-conversation`; the unpublished interval-feedback change is deferred.
See Plan001's latest owner override. Full-plan and release validation remain open.
