# Avesra implementation plans

Avesra means **A Very Effective Smart Reasoning Assistant**.

Repository: https://github.com/Medalink/avesra

Planning baseline: `eb0189ff64b276b4f1e2d850ab3cff6979189ad9` on `main`, inspected on 2026-09-24; at that baseline the repository contained only an MIT `LICENSE`. The owner subsequently requested publication of these planning documents. `E:\Dev\Avesra` is now the repository checkout; no application source has been implemented.

Model provisioning evidence and remaining serving gaps: [Spark2 setup record](spark2-model-setup.md). Approved UI direction and interactive mockups: [design](../design/README.md) (simulated prototype, not application code). The main plan includes the owner's later requests for optional client GPU acceleration and instrumentation throughout the pipeline.

## Owner-defined completion tests

The owner must be able to speak these requests and see Avesra complete them on the actual PC using the single Spark:

1. Open Claude for the **IRIS** project, create a **new chat**, and type exactly `123`; leave it unsubmitted unless separately told to start/run.
2. Check the **latest 10 emails** in the intended mailbox and determine whether they contain a package-delivery confirmation, identifying the supporting message or honestly reporting no confirmation/ambiguity.
3. Open **X** in Chrome/Brave so the owner can post about a new app; verify the intended site/account is ready, without publishing a post.

These are mandatory live end-to-end outcomes. A model server, overlay, passing unit tests, mocked app, or dispatched click is not proof of completion. See `owner-workflows` and acceptance cases A24–A26 in Plan 001.

## Execution order

| Plan | Outcome | Priority | Effort | Dependencies | Status |
| --- | --- | --- | --- | --- | --- |
| [001](001-single-spark-assistant.md) | Build and verify Avesra on Spark2 with Local Studio, a Windows companion, optional client GPU acceleration and per-stage metrics | P1 | L, multiple milestones | Hardware/app preflight in M0 | M0 in progress; application not implemented |

Read the entire plan before implementation. Execute its milestones in order, retain verification evidence, and update this index only when the corresponding evidence exists. Planning does not authorize deployment, account access, production changes, or publication by itself.

## Milestone checkpoints

| Milestone | Deliverable | Depends on | Status |
| --- | --- | --- | --- |
| M0 | Verify hardware, app surfaces, model feasibility, and setup facts | — | PARTIAL — six models downloaded/hash checked; two reasoning recipes passed initial probes; speech/app/concurrency proof remains |
| M1 | Establish workspace, contracts, test harness, and configuration | M0 | TODO |
| M2 | Pair PC/Spark; implement task policy, persistence, and cancellation | M1 | TODO |
| M3 | Prove local speech, speaker enrollment, intent gating, and playback | M2 | TODO |
| M4 | Ship overlay, settings, onboarding, and volume/app launch slice | M3 | TODO |
| M5 | Control Chrome/Brave and desktop/CLI prompt inputs | M4 | TODO |
| M6 | Learn app mappings and routines, maintain memory, explain chimes | M5 | TODO |
| M7 | Diagnose PC issues and operate the existing VPN client | M5, M6 | TODO |
| M8 | Verify swappable deployments, optional Jev, packaging, and recovery | M7 | TODO |
| M9 | Pass real-device workflow, voice, privacy, performance, and soak gates | M8 | TODO |

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

The plan is written; the application is not implemented or deployed. The owner authorized model setup through Local Studio on `ssh spark2`; that target is reachable, all six selected model packages are downloaded and hash checked, and reasoning recipe verification is recorded in the setup evidence. The earlier failed `spark` alias is superseded. The owner chose dedicated local speech/speaker drivers alongside Local Studio. No Avesra microphone, enrollment, desktop-automation, or end-to-end performance proof exists. Downloads and inference probes alone cannot satisfy the owner workflows.
