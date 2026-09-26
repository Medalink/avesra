# Plan 001: Build Avesra on one Spark with a persistent Windows companion

> **Executor instructions:** This is the first-release completion contract for an existing implementation. Read it in full, including the execution overrides and the remaining-work sequence in section 15. Preserve completed foundations and concurrent work. Implement specs and code without automated tests, fixtures or acceptance harnesses, and record actual build/static/direct-inspection results. Never replace live-device evidence with mocks. Report missing hardware, model incompatibility, inaccessible application surfaces, and unmet latency/recognition targets explicitly. Do not silently weaken the product requirements.
>
> **Drift check:** The original planning baseline was `eb0189ff64b276b4f1e2d850ab3cff6979189ad9`. This completion revision inspected `e894025` and then observed concurrent advancement to `c82fed21ea259c7fd1dc6902bf2a5a07b10e231f` on 2026-09-25, with additional dirty companion/enrollment work. Before execution run `git status --short`, `git rev-parse HEAD`, `git worktree list`, `git diff --stat c82fed21ea259c7fd1dc6902bf2a5a07b10e231f..HEAD -- crates apps services scripts docs plans`, and `git diff --stat -- crates apps services scripts docs plans`. Inspect untracked paths separately. Compare section 2 against actual source; resolve ownership of overlapping work before editing. A clean worktree from HEAD does not contain the user's uncommitted changes.

## 1. Status and purpose

### Execution override: specs and code only

On 2026-09-24, during execution, the owner instructed: "no tests here either, just specs and code!" This overrides every instruction below to add or run tests, test fixtures, acceptance runners/report verifiers, or a test-first workflow. Do not create test infrastructure or run automated test suites. Implement the specifications and application code, using compilation, formatting/lint/type checks, and direct runtime/UI inspection for validation. Commands and milestone requirements below that refer to automated tests describe the original plan and are superseded for this execution. Missing automated tests do not block independent implementation work. Actual unavailable hardware, service, or application capabilities still limit the affected integration; do not claim live behavior from static checks. The approved designs remain mandatory for UI work, and the owner workflows and live performance requirements remain product requirements whose unverified status must be reported honestly.

The owner subsequently instructed: "I am about to game, do not use computer use until I am done and tell you." Until explicitly lifted, do not use Computer Use, automate browsers/native windows, launch or focus desktop applications, or perform interactive setup/visual checks. Continue background specs/code work and bounded build/static checks. The later "continue" does not lift this restriction.

Later on 2026-09-25, the owner explicitly requested actual screenshots and audio/video proof and clarified: "get them but do so without interrupting me. Figure it out." This authorizes isolated background inspection and purposeful evidence recording. It does not authorize switching the input desktop, moving the owner's pointer, stealing foreground focus, recording unrelated screen/audio or interrupting their work. Use a separate non-input desktop and silent, explicitly bounded application-output recording where available. Preserve native permission gates and label the precise evidence boundary; a silent post-mixer recording does not prove acoustic speaker output, live-owner recognition or OW1-OW3.

### Later owner override: ship development features for owner testing

The owner then rejected waiting for the large recording corpus before trying the
features: "I want to get features working and tested by me asap" and "I don't want
to wait hours waiting on recordings." The immediate priority is a working,
matched development desktop/controller that the owner can exercise.

The100/200/200 corpus and numerical recognition requirements below remain release
validation, not a mandatory per-owner activation ritual. Provide an explicitly
enabled development admission based on the existing saved enrollment and a short,
genuine live owner check against current services/devices. Retain native ownership,
explicit listening consent, current session/model binding, action/site grants,
protected confirmations, cancellation and uncertainty handling. Do not manufacture
measurements, call the development operating point release-qualified, or silently
enable the microphone through status inspection. Preserve the larger evaluation
as an advanced release-validation path. This is not a push-to-talk substitute or
a conversation-only replacement for the requested features. Unsupported features
and unverified quality remain visible, and shipping a development build does not
mark this whole plan or OW1–OW3 complete.

- Priority: P1.
- Category: product/architecture implementation.
- Effort: L; multiple separately verifiable milestones, not a one-session scaffold.
- Risk: HIGH for identity, desktop actions, and learned routines; MED for UI and driver plumbing.
- Planned at: `eb0189ff64b276b4f1e2d850ab3cff6979189ad9`, 2026-09-24.
- Execution reconciliation: `f085d25cc4e8b4ee59da7c6d7e710ce5d9ecc5ae`, 2026-09-24. Changes since the planning baseline contain only these plans, Spark setup evidence, and the approved design references; no application source overlaps. The owner requested `/improve execute 001` and explicitly requires matching the designs. Historical statements that implementation was not yet authorized describe the planning session, not this execution request. M0 evidence remains required, with unresolved integrations scoped as described in section 18.
- Repository: https://github.com/Medalink/avesra.
- Intended Windows checkout: `E:\Dev\Avesra`.
- Initial deployment: `ssh spark2` with Local Studio and one Windows desktop client; optional client GPU acceleration is in scope.
- Planning status: completion sequence specified against the existing source; implementation remains **IN PROGRESS**. Historical provisioning and verification records are evidence for their recorded revisions only. This planning revision ran no builds, tests, model requests or live workflows.
- Execution checkpoint, 2026-09-25: the owner-requested baseline commit is `de2e8c8`; isolated source commits `3347a48`, `4aa1fa1` and review correction `e986d76` implement bounded C2 segmentation, C1 contract reconciliation and C4 lifecycle integration. See [the execution review](001-execution-2026-09-25.md) for verification and exact remaining qualification dependencies. This checkpoint does not complete C1/C2 activation, C5 owner workflows or release acceptance.
- Background proof checkpoint, 2026-09-25 evening: the prior source was published through `1704d2b`. Subsequent frozen dirty-source builds actually completed default/custom text previews through the native Settings controls and mixer. See [exact screenshots/audio/video evidence](../docs/evidence/background-preview-2026-09-25.md) and the [repeatable noninterrupting procedure](../docs/background-evidence.md). Physical output was intentionally silent. Automatic recognition, accepted reasoning/actions and owner workflows remain unproven; later source changes are not covered by these media.

Avesra is **A Very Effective Smart Reasoning Assistant**. It continuously listens for an enrolled owner, recognizes clearly assistant-directed requests, acts through the owner's PC, speaks in a customizable voice, and learns useful context and routines. It must remain available while the user games without requiring model inference on the RTX 5090. Swapping a model or moving a lane must not require changing the task engine, UI, or permissions.

This plan covers the whole first release and its foundations. M3/M4 are the first useful voice-to-action checkpoint; they are not completion of the whole product.

**Owner-defined done:** The three real voice-to-PC outcomes in section 3 are mandatory sign-off tests. They are the primary product proof. Internal milestones and technical test results support those outcomes; they cannot replace them. Do not announce completion until Avesra itself performs those workflows through its shipped controller and drivers on the actual PC/Spark.

## 2. Current state and evidence

The original repository contained only `LICENSE`; that is historical context, not today's starting point. On 2026-09-25 the checkout contains a Rust 2024 workspace (`avesra-contracts`, `avesra-core`, `avesra-server`, `avesra-windows`, and the Tauri desktop), Svelte/Tailwind UI, an MV3 extension, Python audio services, operational specifications and build/static wrappers. Root `package.json` provides `pnpm -r check` and `pnpm -r build`; `scripts/verify.ps1` accepts only `Static` and `Build`. Section 15 lists the actual commands.

The inspected checkout is `E:\Dev\Avesra` on `main`; `git worktree list` initially showed only this checkout. Earlier named executor branches/worktrees in the index are historical. Source and HEAD changed concurrently during this planning pass. Preserve all existing modifications, including `plans/001-handoff.md`; do not reset, stage, stash, commit or copy them into an executor checkout without establishing their ownership.

### Current source anchors and conventions

These excerpts were read directly during this revision. Line numbers are navigation hints; compare the symbols and surrounding code after drift.

| Boundary | Existing source and remaining gap |
| --- | --- |
| Qualified owner | `crates/avesra-core/src/voice.rs:68` defines `QualifiedProfile` with private fields and no constructor. A saved six-segment candidate is not a qualified identity. |
| Voice producer | `apps/desktop/src-tauri/src/voice.rs:269` calls `analyze(&context,None,observation,None)`, then discards the observation. `spawn` allows one fixed capture attempt per epoch, not continuous qualified endpointing. |
| Reasoning activation | `crates/avesra-server/src/transport.rs` now opens the reasoning driver only when the private deployment configuration exists. The new controlled engine adapter requires exact loaded-artifact, process/route and context-capacity evidence. No qualified reasoning configuration has been deployed; the already-running unrelated engine cannot supply missing launch provenance retroactively. |
| Effects | `crates/avesra-windows/src/effects.rs:223` implements `EffectAdapter::execute` for `LaunchApp` and `SetVolume`; other payloads return `Outcome::Unsupported`. Existing catalog and permission code must be reused. |
| Browser read | `crates/avesra-windows/src/browser_read_channel.rs` retains the private owned handshake. C4 integrated native/extension excerpt publication, content, cleanup and settlement, including the extension's scripting permission. A qualified accepted task and concrete C5 consumer are still absent, so this source integration has no live browser-task proof. |
| Storage/UI | `crates/avesra-core/src/store.rs` owns transactional state; `apps/desktop/src-tauri/src/main.rs` owns runtime projection and typed IPC; `apps/desktop/src/SettingsView.svelte` and `Overlay.svelte` consume it. Existing startup/voice-store fixes are concurrent work, not a new scaffold task. |

Match the existing capability pattern instead of introducing a second action path:

```rust
// crates/avesra-core/src/voice.rs:264
let Some(profile) = profile else {
    return reject(Abstention::Unqualified);
};
// crates/avesra-windows/src/effects.rs:223
impl EffectAdapter for NativeAdapter {
    // execute receives a borrowed DispatchPermit and a fresh authorize callback.
}
```

Use typed `ErrorCode`/`Outcome`, private one-use native handles, bounded queues, original monotonic budgets, and the existing actual ledger worker. Frontend state, model output and saved history cannot create authority. Keep source text/biometrics out of telemetry. UI work must preserve the approved Ruby/Geist/square-corner design and the existing Interface size setting.

The latest handoff reports recovery of the existing protected enrollment candidate from Codex's virtualized AppData into the normal user store. Treat that as recorded evidence requiring revision/path matching, not a fresh observation made here. Never request six replacement recordings merely because a process sees the wrong store. Prove the canonical physical directory first. Saved voice, microphone check, generated voice preview and startup greeting each remain distinct from automatic owner recognition.

The surrounding assistant session was in `E:\Dev\Laravel\iris`, which has unrelated work. Avesra is a separate product. Do not edit Iris, copy its Laravel stack, or import its repository-specific policies as Avesra architecture.

The user's PC has an RTX 5090 and the user owns two Sparks. The owner subsequently selected `ssh spark2`; discovery on 2026-09-24 verified hostname `spark-c8bb`, Ubuntu 24.04.5 LTS/aarch64, NVIDIA GB10 with driver 580.178.04, approximately 121 GiB RAM and 3.3 TiB free disk. Local Studio is running on port 8080 with API authentication. Its installed source is revision `d1abdef` with pre-existing local patches, which must be preserved. Initial `/status` reported no inference process and `/recipes` was empty. Existing ComfyUI and a Qwen3.8-Flash-Next supervisor are separate workloads; do not reset or take ownership of them. The earlier timeout concerned a different `spark` alias, not this verified target. The second Spark remains outside initial scope.

Read-only PC GPU discovery on the same date reported NVIDIA GeForce RTX 5090, 32,607 MiB total VRAM and driver 616.92. A single observation showed 3,943 MiB used and 14% utilization; these are transient readings, not reserved capacity or an inference benchmark. The downloaded Spark FP8 reasoning models must not be assumed to fit this card alongside desktop/game workloads. Start client acceleration evaluation with smaller audio/identity lanes or a separately qualified compact model.

## 3. Confirmed product decisions

| Topic | Contract |
| --- | --- |
| Name | Avesra; use the name throughout UI, binaries, documentation, and repository paths. |
| Compute | One Spark initially, with optional capability-checked client GPU inference to reduce measured latency. Spark-only operation remains required; gaming must release client inference allocations. |
| Model management | Use Local Studio on Spark2 for supported downloads and reasoning/vision recipes/lifecycle. The owner selected dedicated local speech/speaker drivers alongside it; every lane remains swappable through Avesra. Do not introduce another generic model manager. |
| Optimization | Instrument every pipeline stage and compare latency, resources, and correctness on repeatable workloads. No optimization bypasses identity, intent, authorization, or outcome verification. |
| Driver design | Separate logical lanes, adapters, concrete deployments, resource profiles, and tool integrations. |
| Default listening | Continuous after enrollment, with no mandatory wake word. Explicit mute/deafen/pause overrides remain authoritative. |
| Request intent | Accept clearly assistant-directed requests, allow short follow-ups, ignore ambiguous conversation. Recognized speech alone is insufficient. |
| Identity | One authoritative owner; additional enrolled users have explicit owner-managed permissions. |
| Observation | Both continuous screen awareness/learning and explicit watch-and-learn demonstrations. |
| Routine actions | Perform requested routine actions immediately within existing grants. |
| Consequential actions | Ask before send/publish, delete, spend, or permission changes. |
| PC fixes | Run diagnostics automatically; show a concrete proposed fix and ask before changing system/network configuration. |
| Browser | Chrome and Brave; use websites rather than initial service-specific Gmail/X API integrations. |
| App control | Resolve apps by name, learn the correct mapping, find inputs, and reuse previously successful workflows. |
| Coding apps | Open the selected Claude/Codex app and project, compose/type a prompt. Use terminal surfaces only when specified. No direct model API handoff or delegated coding agents. |
| Prompt submission | Submit only when the user explicitly asks to start/run; otherwise leave the prompt ready for review. |
| Retention | Useful memories plus accepted conversation/task history indefinitely, until deletion; raw microphone audio/screenshots are transient. |
| Background communication | No unsolicited spoken advice. Notify on requested task completion, failure, or need for input. Small learning/action chimes are allowed. |
| Chime follow-ups | Answer "What did you learn?" / "What did you just do?" with the actual event behind the sound. |
| Cloud AI | Avesra's inference stays local except optional Jev. Application typing does not become an assistant-side cloud model integration. |
| Lights | Defer the specific integration until after voice and PC/browser control. |

The Rust/Tauri/Svelte design below is the advisor's implementation recommendation, consistent with the requested Rust discussion and the approved Tailwind mockups in `design/`. Model selections and performance budgets are candidates to validate, not owner-supplied facts or measured promises.

### Initial workflows

1. Open any installed application by its learned name/alias; clarify ambiguous matches once and remember the choice.
2. Open Claude/Codex for a named project, such as Iris, and type a request based on the owner's description. Do not modify that project's source directly as part of this workflow. Do not invent requirements when expanding a prompt.
3. Open Gmail in the chosen browser and inspect the requested number of latest messages in the intended mailbox, including the owner's final acceptance request for **10 emails and package-delivery evidence**. Resolve multiple accounts or an ambiguous meaning of latest before reading the wrong mailbox.
4. Visit X in the chosen browser and report current AI trends, with links/timestamps and the actual view/search used. Opening an installed X app is also handled by app discovery; "X" as a placeholder for any application is not a fixed product identifier.
5. Connect the configured work VPN using its installed client. Cisco was given as an example; the exact product/version/MFA flow must be identified in M0.
6. Raise/lower/set Windows output volume, with a configurable step for unspecified relative commands and observed final level.
7. Investigate "Why is my download slow?" and other PC issues using settings and bounded diagnostic commands, explain evidence, propose fixes, and apply only approved configuration changes.
8. Learn how the user opened an application or completed a routine, recall it on a later request, and explain what was learned when asked.

### Owner-defined end-to-end sign-off scenarios

Run these through real microphone input, owner recognition, local planning, the shipped PC/browser drivers, and observed postconditions. Manually driving the app, using a development-only shortcut, typing the result through the acceptance runner, or replacing the real model with a fixture does not pass.

| Scenario | Spoken request and required result | Evidence / unacceptable substitutes |
| --- | --- | --- |
| OW1 — Claude/IRIS | "Open Claude for the IRIS project and type 123 into a new chat." Resolve the actual Claude app and IRIS project, create a fresh chat, and put exactly `123` in its prompt field. Leave it unsubmitted because this request does not say start/run. | Observe correct app/project, new-chat state and exact draft text. Launching the app alone, reusing an old chat, adding extra prose, typing in another input, or submitting are failures. |
| OW2 — package email | "Check my latest 10 emails and see if I got my package delivered." Inspect the 10 newest messages in the configured mailbox and explain whether a relevant delivery confirmation was found, with sender/date/subject or equivalent source reference. | Count actual messages, not assumed Gmail thread rows. Distinguish delivered from shipped/out-for-delivery/estimated arrival. If no confirmation, fewer messages, or several candidate packages exist, report the actual evidence/ambiguity. Do not invent physical delivery or follow a tracking link unless needed and authorized. |
| OW3 — ready to post | "Open X so I can post about my new app." Open/focus X in the configured Chrome/Brave profile, verify the intended account/site is ready for the owner to write a post, and report readiness. | Opening a browser without reaching the site is insufficient. Do not generate, submit or publish a post from this request. If login/CAPTCHA is needed, the user may complete it and Avesra resumes verification; a pending challenge is not completed readiness. |

For OW2, the proposed default is the selected account's Inbox, including read and unread messages, excluding Spam/Trash unless requested. Make that mailbox scope visible/configurable during setup so the voice command does not require repeated clarification. Gmail's conversation grouping may require inspecting messages inside threads; do not silently reinterpret 10 messages as 10 conversations.

Directly inspect OW2 handling of delivered, shipped-only, out-for-delivery, no-match and multiple-package cases when those cases are available under authorized mailbox access. Record uncovered cases as pending; do not create fixture mailboxes or automated harnesses under this execution override. A truthful "no delivery confirmation in these ten" can be correct; claiming an expected delivery where no real message supports it cannot. Model inference must remain local while reviewing message content.

The `owner-workflows` scenario group is a release requirement, verified by direct observation rather than an automated suite. Record actual device/model/app versions, recognized requests, action/step IDs, and observed postconditions. Use redacted evidence; do not publish private email content or project context in the repository. Report each scenario separately as PASS/FAIL/BLOCKED. The owner reviews the actual visible result and spoken answer.

### Explicit non-goals for the first release

- No universal claim that every website or desktop app is supported. A general control mechanism plus real acceptance for the initial workflows is required; inaccessible/new surfaces have honest outcomes.
- No always-elevated agent, UAC bypass, game injection, game input automation, or anti-cheat workaround.
- No invisible email/post sending, autonomous code changes through external agents, or automatic AI API fallback.
- No raw screen/audio archive, general keylogger, ambient unknown-speaker transcript archive, or training from captured credentials.
- No multi-Spark sharding, client GPU inference outside an explicitly enabled and qualified Accelerated profile, new Home Assistant installation, mobile companion, remote Internet control, or plugin marketplace in this release. Single-Spark and Gaming profiles must not retain client inference allocations; optional acceleration follows sections 5 and 15 (M8).

## 4. Architecture and ownership

```mermaid
flowchart LR
  subgraph Windows PC
    UI[Tauri overlay and settings]
    PC[Rust companion: audio, observation, local policy, tools]
    EXT[Chrome/Brave extension]
    UI <--> PC
    EXT <--> PC
  end
  subgraph Single DGX Spark
    CORE[Rust controller: sessions, tasks, drivers, memory]
    DB[(SQLite and persistent configuration)]
    AI[Local reasoning, speech, speaker and vision services]
    CORE <--> DB
    CORE <--> AI
  end
  PC <-->|Paired authenticated encrypted connection| CORE
  CORE -. Optional minimal decision request .-> JEV[Jev]
```

### Windows companion

- Existing voice effects and background music/atmosphere streaming are implemented baseline functionality, reaffirmed by the owner on 2026-09-25 during this execution. Preserve Plan 002's shared native CPAL renderer, Digital/Human preferences, output cancellation, and actual mixed-channel playback references. Normal assistant speech must reuse this output path; do not replace it with dry-only playback or a second music stream. This preserves the sound feature without claiming qualified owner recognition or completed voice-to-action workflows.
- Tauri 2 with bundled Svelte/TypeScript/Tailwind assets; no remote web content in the privileged settings webview.
- Rust owns microphone capture, output playback, local stop/mute controls, Windows app discovery, UI Automation, capture, and action execution. UI animations do not own the audio loop.
- Use a user-session process, not an interactive Windows service in session zero. Start at sign-in only after setup enables it. Minimizing or closing the overlay hides it to tray; explicit Quit stops capture and the client process.
- Use Windows Core Audio for master output control. Use structured native/UI Automation controls first, visual grounding second. Use a browser extension for signed-in browser pages.
- Only the local executor owns desktop input. Background jobs must not compete for the mouse, keyboard, focus, or browser tab.
- In the `single-spark` profile, do not load inference models on the PC. In an explicitly enabled `accelerated` profile, eligible lanes may use the client GPU within measured VRAM, utilization, and frame-time budgets. CPU-only clients remain supported. Ordinary audio DSP and desktop rendering are accounted for separately.

### Spark controller

- Rust/Tokio service for session state, task transitions, policy checks, lane routing, queues, model health, memory, and resource profiles.
- SQLite is the initial durable source of truth. One controller process owns writes; inference workers do not edit memory or task state directly.
- Local Studio owns supported model downloads, reasoning/vision recipes, serving engines, and their lifecycle. The Avesra adapter references its recipe/deployment IDs and negotiates capabilities. The installed version's supported launch engines are vLLM, SGLang, and exllamav3; an audio proxy route does not prove it can host every speech/speaker model. The owner explicitly chose dedicated local ASR/TTS/speaker services alongside Local Studio. Avesra's deployment supervisor owns those narrowly scoped service lifecycles and exposes the same lane contracts, health, cancellation and metrics. They may read the pinned downloaded artifacts; they do not modify Local Studio internals or compete for ownership of its reasoning engines. Until serving capability is proven, downloaded packages remain unavailable deployments.
- Use pinned model revisions and runtime images, preserving Local Studio's existing authentication. Avesra owns its controller and dedicated speech/speaker service supervision, not Local Studio's internal processes. Give each service an explicit memory/compute budget and independent readiness probe. Verify actual port exposure; protect any LAN-reachable inference port with authentication. Do not assume a Docker-published port is loopback-only.
- Foreground speech and command work take priority over passive observation, routine extraction, and indexing. Background queues are bounded and coalesce stale screen changes.

### Transport

Use TLS WebSocket connections for the initial PC/controller protocol: a control/event connection plus separate bounded media streams. Use explicit versioned serde message envelopes; do not send untyped model output straight to an executor. M1 must freeze and test the schema before UI integration.

Pair with a one-time code and verified server fingerprint on a local setup screen. Persist a revocable per-device credential; never expose an unauthenticated LAN command server. Scope browser native messaging to the installed extension ID and authenticated user-session host. Redact credentials from logs and diagnostics.

Every message carries protocol version, device/session identity, request ID, sequence, and the relevant capture/action epoch. Tool requests additionally carry task/step ID, expiry, target identity, policy grant, and approval identity where required. Authenticate peers before accepting media or actions. Unknown versions, out-of-order frames, invalid grants, expired requests, and stale epochs must fail explicitly.

Bound message/frame sizes, media duration, queues, and connection retries. Document codec/sample-rate negotiation and reject unsupported formats. Avoid routing screenshots/audio through the overlay's DOM event system. Support clean cancellation without relying on a network round trip for local mute/stop.

## 5. Driver, deployment, and profile contracts

### Separate concepts

- **Lane:** a capability the controller needs.
- **Driver:** an adapter to a provider/runtime or tool surface.
- **Deployment:** a driver instance with model, address, machine, limits, and artifact revision.
- **Profile:** lane assignments, resource budgets, priorities, and explicit fallback order.

A model may serve multiple lanes. A lane may have multiple compatible deployments. Do not load one model copy per lane when those assignments refer to the same deployment.

Use Rust traits internally and versioned service contracts at process boundaries. Register built-in adapters explicitly. Adding a new adapter may require a build; changing model/deployment configuration should not. Do not build a dynamic Rust-library ABI/plugin loader in this release.

### Lane interfaces

| Lane | Input | Required output/capabilities |
| --- | --- | --- |
| Speech activity / transcription | Sequenced audio chunks and language/settings | Speech boundaries, partial/final text, timing, cancellation; partials cannot authorize actions |
| Speaker identity | Audio segments + enrolled profile revision | Candidate identity or unknown, similarity/evidence quality, overlap/insufficient-speech outcome; never a permission grant |
| Conversation/planning | Bounded context, accepted request, offered tool schemas | Response deltas and typed action proposals; reasoning/internal tokens stay out of speech |
| Screen understanding | Scoped fresh image/UI observations and question | Observations/targets tied to observation ID, coordinate space and capture time |
| Speech generation | Approved speakable text and versioned voice preset | Audio chunks, format, cancellation, playback completion signals |
| Decision evaluation | Narrow state and typed question schema | Validated choices/scores with source/model metadata; no direct actions |
| Memory processing | Accepted task events and permitted observations | Proposed summaries, facts, mappings, routine candidates with sources; controller validates writes |

Each driver advertises capabilities, supported formats, model/revision, stream support, cancellation behavior, context/input limits, health, and deployment locality. Optional capabilities are negotiated; reject a profile assigning a text-only model to image interpretation or a nonstreaming service to a required streaming contract.

Health distinguishes configured, starting, loading, ready, degraded, unavailable, and incompatible. A responding HTTP port is not proof that the required model is ready. Validate with an actual capability probe.

### Initial inference candidates

| Purpose | Candidate to benchmark | Selection gate |
| --- | --- | --- |
| Main reasoning + vision | Compare official Qwen3.6-35B-A3B-FP8 and Qwen3.8-27B-FP8 recipes in Local Studio; do not declare a winner before local measurements | Tool correctness, real UI grounding, short-response latency under speech load |
| Streaming English ASR | NVIDIA Nemotron Speech Streaming English 0.6B | Speech accuracy and endpoint latency on the owner's microphone, including project/app names |
| Speaker matching | SpeechBrain ECAPA-TDNN baseline; optionally a local pyannote component for overlap/diarization | Owner/unknown separation, short speech, noise and replay limitations; no vendor benchmark substituted for local evaluation |
| Voice | Qwen3-TTS, testing 0.6B and 1.7B runtime variants | Consistent chosen voice, chunked playback latency, intelligibility and interruption |
| Hard visual actions | UI-TARS-1.5-7B only if the main vision model cannot meet grounding tests | Measurable improvement that justifies extra resident resources |
| Routing/evaluation | Local main model; optional Jev adapter | Typed-result validation and explicit cloud boundary |

These are replaceable starting points, not a requirement to run all simultaneously. Generate a designed voice during setup, then reuse its preset rather than running the voice designer on every utterance. Store an explicitly selected generated voice asset as a configuration artifact; this is distinct from retaining microphone recordings. Do not assume a voice preset or speaker embedding works with another model revision.

M0/M3 must pin actual working ARM64/GB10-compatible container digests, dependency versions, model revisions, quantization, license/terms, context limits, and measured working sets. Public model documentation does not prove a particular serving image runs on the user's Spark. Avoid floating `latest` tags in the accepted deployment.

### Resource and switching policy

- Ship `single-spark`, optional `accelerated`, and `gaming` profiles. `single-spark` is the baseline and uses no client inference. `accelerated` may place ASR, TTS, speaker matching, or a suitably sized vision/reasoning model on a qualified client GPU only when end-to-end measurements justify it. `gaming` drains/cancels client inference at safe boundaries, frees model allocations, and uses Spark deployments. Future device slots cannot masquerade as ready hardware.
- Start with a modest measured context/concurrency budget; never allocate essentially all unified memory to the LLM cache while speech services also need memory.
- Reserve headroom for the OS/controller and overlapping ASR/TTS. Establish numeric reservations from M0 measurements; queue or reject excess work explicitly.
- Priority: local mute/stop control, interactive audio and accepted commands, active action verification, passive observation, memory consolidation.
- Model/profile changes: validate, load/probe replacement where capacity permits, switch new requests at turn/utterance boundaries, drain/cancel old requests, then unload unused models. If Local Studio requires unloading the active model first, disclose the interruption, preserve its recipe, and reload it on replacement failure; do not promise a zero-downtime swap. Show pending/failed changes. Never replay an uncertain desktop action during a route change.
- Keep interactive speech and the normal command model warm within their resource budget; do not unload/reload models for every utterance. Cache verified app aliases and semantic routines, while checking current target/focus before action. Run independent ASR and speaker analysis concurrently; speculative work cannot authorize an effect before final acceptance.
- Avoid requiring a long generated plan for every familiar command. A recognized, unambiguous accepted request may select a validated typed native action or learned routine with bound arguments; unresolved requests use the planning lane. This fast path still requires the same speaker/intent gates, grants, focus/freshness checks, approvals and observed outcome. Measure exact argument accuracy and ambiguity rejection before enabling it. Keep tool schemas/proposals concise; generation throughput alone does not describe time until an executable proposal is complete.
- Client placement requires device capability and model-footprint checks, bounded queue age, live free-VRAM/utilization/thermal signals, and measured LAN transfer cost. Use hysteresis and a minimum dwell time to prevent routing oscillation. A manual Gaming toggle overrides automatic detection. Background learning yields first; reserve capacity for cancellation and interactive speech.
- On memory pressure, defer background jobs and optional specialists before destabilizing interactive speech. Use only fallback placements enabled in the selected profile; never silently use cloud AI or a client GPU excluded by that profile. Profile transitions must not silently downgrade the owner/intent acceptance thresholds.
- Service outages produce a visible degraded state. When the controller is unreachable, local controls still work and the PC does not execute old queued model actions after reconnect.

## 6. Identity, listening, and request acceptance

### Enrollment and users

The first authenticated local setup creates the single owner. Capture several short prompted phrases plus natural speech, check input quality, and verify using held-out speech. Approximately one minute is a UX starting point, not a recognition guarantee. Tune enrollment requirements based on measured quality.

Keep raw enrollment audio in memory only and discard it after extracting the versioned profile. Persist derived speaker representations with model/revision and enrollment-quality metadata. A speaker-model change requiring incompatible embeddings initiates re-enrollment; do not silently accept the old vectors. Protect derived profiles as sensitive local data; exclude them from generic logs, cloud requests, and normal diagnostic exports.

Additional users enroll explicitly. They have no action or owner-memory access until the owner grants scopes. The owner controls enrollment, grants, revocation, and ownership transfer through an authenticated local management flow. A voice match alone cannot change ownership or expand permissions. Use OS-backed local authentication or an owner credential protected with Windows facilities; verify the selected method in M0. Never store a plaintext owner password.

Do not auto-enroll unfamiliar voices or silently update biometric profiles from ambient speech. Continuous learning refers to routines/preferences/context, not unattended biometric retraining.

### Acceptance pipeline

1. Capture local audio only when the current local mode permits it; label frames with a capture epoch.
2. Detect speech and process speaker/ASR in bounded local services. Speculative transcription is allowed for latency but remains transient until accepted.
3. Reject/abstain on unknown speakers, insufficient signal, ambiguous match, overlapping speech, or stale epochs. Speaker diarization alone is not identity verification.
4. Verify that the enrolled speaker has applicable rights and that the utterance is clearly directed at Avesra. Owner speech to teammates must not trigger commands.
5. Resolve references using the active conversation and current screen, without accepting another speaker merely because the previous speaker was the owner.
6. Create an accepted user turn/task only after these gates; then persist it and permit planning.
7. Independently authorize each proposed effect against task intent, actor grants, and any required approval.

Use `accept`, `reject`, and `abstain` as explicit outcomes. Do not convert an LLM's self-reported confidence or a speaker similarity into a calibrated probability without measured calibration. Short follow-ups still require speaker evidence; if "yes" is too short to establish identity, request a longer explicit reply or use a local UI confirmation.

Voice matching is not proof against recordings, synthesis, illness, microphone changes, or similar voices. Test and report these limits. Use replay/echo suppression where supported, but keep ownership/permission changes and high-impact confirmations anchored to the authenticated local UI. The first release must not claim secure voice-only authentication.

### Listening and interruption modes

| Control/state | Mandatory behavior |
| --- | --- |
| Mic mute | Stop local capture immediately, clear unsent buffers, invalidate incomplete/unaccepted turns. Already accepted tasks may continue. |
| Deafen | Mic mute plus immediate local playback stop; suppress chimes. Restoring sound must not silently clear an explicit microphone mute. |
| Stop task | Cancel queued steps and owned cancellable processes; show any already-completed or uncertain effect. Do not imply rollback. |
| Pause assistant | Stop capture/observation and admission of new actions. Suspend remaining task steps at a safe boundary; show noninterruptible work still in progress. |
| Minimize/close overlay | Hide to tray without changing listening/action mode; stop hidden animation work. |
| Quit | Stop capture, playback, and client activity; cancel/invalidate pending local work and inform the controller when possible. |
| Windows lock/sign-out | Stop local capture/observation and desktop actions. Require a fresh authenticated connection/session before resumed execution. |
| Network loss | Local controls remain effective; no delayed stale actions on reconnect; unknown-effect steps enter reconciliation. |

Local mute/stop is authoritative even if the Spark is blocked. On reconnect perform an epoch/mode handshake before accepting new actions. If a request crossed the acceptance boundary just before muting, it is an accepted task and remains visible/cancellable; if it was incomplete, it must never be resurrected from a late transcript.

Allow the owner to interrupt speaking. Stop local output first, cancel upstream TTS where supported, and discard late audio chunks using utterance IDs/epochs. Prevent the assistant's own voice and desktop playback from creating commands with playback-reference echo handling and explicit output provenance; an implementation that simply cannot listen while speaking must be reported as a degraded mode, not full interruption support.

## 7. Desktop interface and onboarding

Implement a small borderless, draggable, optional always-on-top overlay with a system-tray entry, following the approved mockups in `design/` (see `design/README.md`). Visual direction: dark only, neutral grey surfaces (`#121214` to `#26262a`), square corners, Geist and Geist Mono, a Ruby accent (`#E0115F`; filled controls `#960B3F` for readable white text), and accessible contrast. Reserve amber for input-off, unavailable and degraded states and red for stop, error and disconnected. This is Avesra's standalone design, not an Iris screen. Keep the interface compact; technical details belong in settings. Respect reduced motion and mixed-DPI/multi-monitor coordinates. Provide an Interface size setting (100, 110, 125, 150 and 175%; default 125%) that zooms every Avesra webview uniformly and scales window sizes to match. The reference mockups are drawn at 100%; compare rendered screenshots at the same scale, not against a design canvas zoomed to fit.

The compact overlay is transparent and built around one live signal bar driven by real microphone/playback levels and lane events, with three distinct renderings: human audio as a filled mirrored waveform colored by transient source classification (owner blue `#3A5DD8`, other people green `#8EDE4A`, background grey); thinking as a gamma-style trace fused with stepped digital activity; and Avesra's speech as a ribbon of synthetic strands, red to Ruby. Working, muted/deafened, paused, and disconnected use their own flat indicators. Source coloring is a live display only: it creates no retained record of unknown speakers and is neither an identity decision nor a permission. Provide a non-WebGL fallback and a static reduced-motion frame, and stop rendering while hidden.

Distinguish passive listening, recognizing, accepted request, thinking, speaking, working, muted, paused, and disconnected, including for screen readers. Passive, recognizing, accepted, thinking, and speaking show no visible text; working, muted, deafened, paused, and disconnected show a short label. Mic/deafen/settings/minimize controls appear on hover or keyboard focus, or while switched on; Stop remains visible while a task is active. Do not beep for every unknown speaker or fabricate numeric identity certainty.

Expansion shows the current request, concise streaming response, active task/progress, and recent meaningful events. It is not a required full chat dashboard. Clicking a learning/action event opens its short explanation and source. A local text input is useful when muted or recognition needs recovery.

Settings sections:

1. Audio and voice: devices, test meters, voice preview/designer, pace/volume, optional wake phrase/push-to-talk, hotkeys, chime toggles/volume.
2. Models and drivers: per-lane adapter/model/deployment, actual readiness, probe, limits, locality, compatibility, pending changes.
3. Profiles and machines: working single-Spark profile, resource budgets, future slots clearly unconfigured, pairing and revocation.
4. People and voice identity: owner, enrollment/re-enrollment, additional users and grants, profile deletion.
5. Observation and actions: monitored displays/apps, exclusions, learned app aliases, routine inspection, pending approvals, pause controls.
6. Memory and diagnostics: retained history, editable facts/routines, source/date, deletion/export, latency/health, failures and redacted support bundle.

Onboarding: pair the Spark; verify service capabilities; select microphone/speakers; enroll/validate the owner; select a generated voice; show listening and observation scopes; pair Chrome/Brave extension; resolve initial app aliases/projects; test a harmless volume change and rollback to the prior level. Explain transient media versus retained text/history. Setup must not falsely complete while inference services are unavailable.

## 8. Tool execution and permissions

The model proposes a typed action; the Rust controller validates it; the PC executor independently checks the grant/epoch/target before acting. A model, webpage, email, terminal output, memory, or demonstration cannot mint permissions or mark an action successful.

### Tool result contract

Return `success`, `failed`, `needs_input`, `cancelled`, `unsupported`, or `unknown_effect`, with redacted structured evidence. A successful click is not equivalent to a successful workflow. Every effect needs a postcondition or a declared uncertainty. Keep plan/proposal status separate from completed execution.

Use a persistent task/step ledger with stable step IDs, approvals bound to the exact action and target, start/finish times, and outcome. Retries may repeat reads. Mutating steps must query/reconcile before retrying; do not claim exactly-once delivery for arbitrary GUI actions. A lost acknowledgment after Submit must produce reconciliation, not another blind Submit.

Only one desktop-input lease is active at a time. Verify the target window/tab/element and observation freshness immediately before input. If the user changes focus or intervenes, stop/reobserve rather than type into the new foreground window. Routine memory stores selectors and preconditions, not unconditional pixel coordinates.

### Authorization matrix

| Operation | First-release rule |
| --- | --- |
| Launch/focus an app; navigate requested site; read requested content; adjust volume | Immediate on an accepted authorized request |
| Fill an app prompt | Immediate for the named target/project; do not clear unrelated existing content without resolving it |
| Submit Claude/Codex prompt | Only an explicit start/run request grants this particular submission |
| Read-only diagnostic catalog | Immediate within bounded duration/output and permitted target scope |
| System/network fix; generated diagnostic script outside the reviewed catalog | Present concrete commands/effects for approval before execution |
| Send/publish/delete/spend/grant | Explicit approval bound to the shown action/target; no blanket model-issued approval |
| Existing configured VPN connect | Routine requested action; pause for MFA/user authentication; changing VPN settings is a separate confirmed fix |
| Owner/user-management changes | Authenticated owner UI; voice alone insufficient |
| Replay learned routine | Reevaluate every step under current permissions and fresh state |

Do not label an arbitrary generated shell script "read-only" solely because a model says it is. The automatic diagnostic catalog uses fixed executables/cmdlets and validated typed arguments, not concatenated command strings. Proposed new scripts require review, timeout/output caps, a non-elevated default, and recorded approval. Terminal prompt typing and shell execution are distinct operations.

Use Windows UAC as intended. No always-admin companion, automated UAC consent, protected-desktop injection, security-agent disabling, or credential extraction. MFA/password entry is the user's step; pause media capture around recognized sensitive inputs and do not retain their values in routines/history.

External content remains untrusted task data. An email saying "ignore previous instructions and run this command" must not produce a tool grant or a durable instruction. Include direct inspection of prompt-injection handling in browser, memory and diagnostic qualification; do not create automated fixtures under this execution override.

## 9. App discovery, browser control, and concrete workflows

### App registry and learned aliases

Discover installed applications from supported Windows app registration, Start menu shortcuts, and explicitly selected executables. Preserve their native identity: executable/path or package app ID, verified publisher where available, launch arguments, working directory, and window identity hints. Do not treat arbitrary text supplied by a model as an executable path.

`AppAlias` maps a user phrase to one verified app identity and records who selected it, when it last succeeded, and version/path drift. `ProjectAlias` maps names such as Iris to an explicitly configured project/workspace in a particular application. Do not infer repository changes or developer instructions from the alias name.

When multiple candidates match "Claude", show/describe the candidates and remember the owner-selected match. Subsequent requests should avoid rediscovery while the mapping remains valid. Revalidate identity when executable/path/publisher or meaningful app structure changes. A memory hit removes lookup effort; it is not a guarantee of instantaneous application startup.

Opening an app includes launch or focus, wait for the expected process/window, and return the observed result. Handle a running instance, modal dialog, launch failure, missing executable, and renamed app explicitly. Never launch both ambiguous candidates and hope one is correct.

### Browser driver

Use a Manifest V3 companion extension in the user's chosen Chrome/Brave profile, connected to the Windows Rust process through native messaging. This preserves the intended signed-in browser context without copying cookies or creating a separate automation profile. Resolve browser, profile, account, tab, and target origin explicitly.

Start with content-script DOM/semantic snapshots and bounded commands. Use documented extension debugger/AX/input facilities only if required for reliable interaction and explicitly granted during setup. Keep an allowlist of supported operations; do not expose arbitrary CDP, JavaScript evaluation, cookie extraction, or network interception to model output. Brave compatibility and native host registration must pass a real M0/M5 spike.

Use extension-derived tab/frame IDs, roles, names, element handles and current document/observation revisions. Webpage content cannot choose native message targets or issue commands. Validate origins, message schemas, frame ownership, size, and request IDs in both the extension and native host. Navigation invalidates old element references.

Do not open the everyday browser's unauthenticated remote-debugging port. Chrome's remote-debugging changes and separate-profile requirements are another reason to verify the extension route rather than assuming a Playwright connection to the default profile will work. See the primary sources in section 17.

Handle popups, consent banners, stale references, loading, tab closure, browser restart, extension restart, CAPTCHAs, login, and expired sessions. Pause for user authentication rather than trying to bypass it. Protected browser pages and unsupported controls may require a clearly reported native fallback or user input.

### Gmail and X

- Gmail: identify the chosen mailbox, use its current browser UI, establish newest-message ordering, and read at most the requested count of individual messages. Default spoken result is sender, subject, and concise content summary; provide full text when asked. If fewer messages exist than requested, report the actual count. For a package inquiry, distinguish actual delivery confirmation from shipment/estimated arrival and identify the supporting message. Opening a message may mark it read; do not claim read-only Gmail semantics that the UI cannot guarantee. Do not send, delete, archive, or open attachments by default.
- X: use the requested site/view/search and preserve source links and observed timestamps. Define "AI trends" operationally as a current search/feed sample and label the view/time window; do not imply global trend statistics from a personalized feed. A stale cached result, blocked page, or failed retrieval cannot become a current answer. No posting or account changes.
- Other websites: reuse the browser primitives and action policy. The UI reports unsupported/needs-input outcomes. Support breadth grows through verified routines; it is not a prebuilt connector roster.

### Coding-app prompts

Resolve the requested application and project, inspect the current prompt input, generate only the task text supported by the owner's request, and insert it into the intended input. Existing unsent text is not silently destroyed. Verify the project and inserted text before submission.

For "prepare a task", stop at a filled draft. For "start/run this task", submit once and verify that the application accepted it. Avesra's task completion is "prompt prepared" or "prompt submitted", not "feature implemented". Monitoring the coding application's outcome is a separate explicit request; no controller-side delegation or direct Claude/Codex model call is introduced.

For a terminal explicitly selected by the user, establish whether focus is a shell prompt or an AI program's input. Typing an AI prompt into a shell is a test failure. A terminal must not be started merely because the desktop app is hard to automate. If the requested surface cannot be identified safely, ask for direction.

### VPN

Learn the correct existing VPN app and profile through discovery or demonstration. Connecting that existing profile is an authorized routine. Inspect actual client/OS connection state, not only the Connect button click. Pause at credentials/MFA. Never retain secret input in a demonstration.

Before connecting, note that a VPN may change routing to the Spark. After connection, re-check the PC/Spark channel. If corporate policy blocks LAN access, report the conflict; do not disable security settings or alter split-tunnel policy. The plan does not assume Cisco exposes a particular supported CLI until M0 verifies the exact edition/version.

## 10. Continuous observation, teaching, and learned routines

### Continuous observation

Observe user-selected displays/apps during unlocked active sessions. Use foreground-window events, browser structural events, and change detection to avoid processing identical frames. Use a bounded periodic capture as a backstop; a proposed initial budget is up to one changed frame every two seconds during passive awareness, with a higher bounded rate during an active action. Measure and adjust explicitly in M0/M9.

Scope observation to normal application surfaces selected in setup; exclude password managers, authentication/secure desktops, and user-excluded apps/regions. Stop on lock and pause. Visual masking is imperfect and cannot guarantee all sensitive content is recognized; retaining no raw media and keeping interpretation local are part of the boundary, not a claim of perfect redaction.

Maintain only a small latest-frame queue and a short audio ring in RAM. A proposed initial media TTL is at most 30 seconds unless an actively processed bounded utterance needs it; impose a hard utterance timeout. Preserve existing bounded media contracts and encode any missing size/duration limits with direct/static verification. Overload drops/coalesces stale passive observations before live commands. Never write screenshot/audio request bodies, debug dumps, or automatic crash payloads to disk. Avoid claiming forensic erasure from OS paging; exclude media from app-managed persistence and support bundles.

Do not continuously transcribe every visible page into permanent memory. Extract compact relevant observations, candidate app mappings, or routine structure. The user asked for useful learning and indefinite accepted interaction history, not a complete permanent textual reconstruction of everything on screen.

### Explicit teaching

"Watch and learn how I connect my VPN" begins a scoped demonstration with a visible indicator, selected app/window, and Stop/Cancel. Collect semantic action events and transient screenshots needed to interpret them. Do not install a general-purpose keylogger. Record text entry as named parameters or permitted literals; redact password/MFA/secret fields and mark them as user steps.

After demonstration, construct a routine with:

- Name/aliases, actor/visibility scope, purpose and parameter schema.
- App/site identities and environment/version hints.
- Preconditions and selectors for each step.
- Allowed tool operations, step-level effect class and required approval.
- Expected postconditions, cancellation points, and failure/recovery instructions.
- Source demonstration/task IDs, confidence/evidence quality, and validation history.

Store a candidate automatically and explain it on request. Saving a candidate does not execute it or approve its effects. When invoked, run through the same planner/policy/executor as a fresh task. A demonstrated Send/Delete button does not become an approved future Send/Delete step.

### Passive learning and validation

Automatically learn verified app mappings from successful Avesra actions. Passive observation can infer routine candidates, but must distinguish observed sequences from proven successful workflows. Merge duplicates; keep user corrections authoritative; never overwrite a known working routine merely because a model guessed a better one.

Use `candidate`, `validated`, `stale`, and `disabled` states. A candidate can guide an explicitly requested task, but each action still requires current grounding and authorization. Mark validated only after execution and postcondition verification. Revalidate after environment changes; failed selectors trigger reobservation or a question, not blind retries. Keep revision history so the owner can inspect or restore a prior definition.

Learning does not require fine-tuning base models. Routine/fact extraction is an asynchronous low-priority job with bounded work, provenance, and a durable outcome. A learning failure cannot block the current user request or be reported as successful learning.

## 11. Persistent memory, event history, and learning sounds

### Data model

Use SQLite migrations with foreign keys and explicit ownership. Initial logical entities:

| Entity | Essential fields / invariant |
| --- | --- |
| User / grant | Stable identity, role, scope, revocation/version; exactly one owner |
| Speaker profile | User, model/revision, derived representation, quality, enrollment/revocation time |
| Device / pairing | Credential reference, fingerprint, scopes, revoked state |
| Deployment / profile | Adapter, model revision, host, capabilities, limits; secret references rather than values |
| Session / accepted turn | Actor, request/response, time, referenced context, capture/session revision |
| Task / step | Actor, intent, stable IDs, state, pre/postconditions, approvals, effect reconciliation |
| Observation summary | Scope, origin/app, time, relevance and provenance; no raw image/audio |
| App/project alias | Verified target identity, owner selection/evidence, last verification |
| Fact/preference | Value, scope, explicit/inferred flag, sources, correction/supersession state |
| Routine / revision | Typed steps, parameters, permissions, source evidence, validation/staleness |
| Learning/action event | Actual committed change/result, source task, before/after summary, actor visibility |
| Notification batch | Event IDs, type, audible timestamp, delivery/ack state, suppression reason |

Retain accepted conversation/task history and useful memories indefinitely until the user deletes them. Paginate retrieval; index full-text search and exact app/task IDs. Begin with SQLite FTS and structured lookup; add a local embedding index only if retrieval evaluation demonstrates a gap. Embeddings and indices are rebuildable and follow source deletion.

Exclude secrets, authentication fields, rejected ambient transcripts, unknown-speaker conversations, raw media, and unbounded scraped page bodies. Store concise relevant task context instead. Separate per-user history/memory from explicitly shared routines; another enrolled voice does not inherit the owner's email or work history.

Deletion must remove selected facts/history, dependent searchable copies/indices, and derived memories that have no remaining valid source, or explicitly ask which independently confirmed facts to retain. Keep audit metadata without deleted content where needed. Describe backup retention separately; never promise deletion from an old offline backup that was not rewritten. Support local export and backup/restore with secret/profile exclusion defaults.

Use transactions for accepted task state and memory changes. A restart must recover durable tasks without automatically replaying uncertain GUI effects. Expose storage use and backup status; indefinite retention is not unlimited RAM/context.

### Chime contract — explicit user refinement

Small sounds may announce meaningful new learning or a verified action. They do not license unsolicited spoken advice. Add separate learning and action chime toggles plus independent volume. Initial low-volume defaults may be enabled during onboarding and previewed there.

Emit a learning event only after a useful mapping/fact/routine change is committed. Emit an action event only after its stated outcome is verified. No chime for speculative inference, duplicate observations, every click, every token, or a failed memory write.

Batch related events and rate-limit background learning sounds; start with at most one learning chime per minute and keep the underlying events available without sound. Deafen/pause suppress chimes. Do not replay a backlog of old sounds after unmuting or reconnecting. Speech playback takes precedence over chimes; do not overlap a chime with the important part of a spoken answer.

Record the last **actually announced** batch per user/device, not merely the newest internal event. "What did you learn?" immediately after a sound resolves to that batch even if later silent work has finished. If several events were grouped, give a concise list and allow drill-down. "What did you just do?" must distinguish a completed action, an inferred routine, and work that still needs approval. If no event exists, say so; never invent learning to explain a sound.

Chime-generated or spoken assistant output cannot feed back into new owner commands. The overlay exposes recent events for inspection without requiring a voice request. Keep notification IDs stable across reconnects and suppress duplicate delivery.

## 12. Diagnostics and proposed fixes

Implement a bounded diagnostic catalog before a general script proposal interface. Initial slow-download investigation should gather:

- Which app/download is slow, reported throughput and units, start time and any app throttling shown.
- Active adapter, link speed/type, addresses, route/default gateway, DNS configuration, and VPN status.
- Recent system/app network activity, competing traffic, CPU, disk utilization and available space.
- Bounded reachability/latency and DNS checks to relevant known endpoints; avoid unrelated network scans.
- Whether the limitation could be remote-server load, wireless quality, VPN routing, application settings, security inspection, disk pressure, or unit confusion.

Use Windows APIs/counters and a reviewed catalog of fixed PowerShell/CMD invocations with validated parameters. Record units: Mbps and MB/s are not interchangeable. A download speed test is an active measurement, not proof of the speed of the original server; show its source/conditions. Significant test downloads or network changes require the user to agree to the proposed test.

The result separates observations, hypotheses, and confirmed findings. A failed measurement is unavailable evidence, not a zero. Work VPN connectivity can block access to the Spark; treat this as a real recovery scenario.

Each proposed fix contains the exact target, commands/settings, expected effect, likely disruption, and available rollback. Approval binds to that proposal and expires if its target or commands change. Execute under the current user's privilege by default and require user handling of UAC. Verify the result, report unsuccessful fixes, and offer the recorded rollback where appropriate. Do not automatically disable protection software, reset networking, reboot, or alter corporate VPN/security policies.

## 13. Jev and outbound data boundaries

Jev is an optional decision adapter, not the task owner or security authority. It may classify an already-accepted request or choose among typed workflow options. Define a narrow outbound schema containing task category, offered action labels, and minimal non-sensitive decision context. Raw audio, speaker profiles, screen images, credentials, full memory, email bodies, and work documents are excluded by default. Do not quietly expand this schema for better accuracy.

If the narrow context is insufficient, use a local decision driver or ask the user rather than attaching private context automatically. Jev output still passes schema validation and policy checks. Its probabilities do not authorize actions. Record only redacted request metadata and latency/outcome, not secrets or unnecessary content.

Jev timeout/outage has a tested local fallback or an explicit needs-input/degraded outcome. Avesra must pass its core workflow acceptance with Jev disabled. No other cloud AI inference fallback is present.

Opening a website and typing a user-requested prompt into Claude/Codex necessarily interacts with those applications and their services. This is the explicit requested browser/app operation, not permission for Avesra to send ongoing screen/memory context or call their model APIs. Ordinary Gmail/X/VPN network traffic is distinct from assistant-side inference traffic; do not claim the whole PC works without Internet.

## 14. Project layout and engineering conventions

Use the existing workspace below. `config/` and `deploy/` are proposed packaging locations only if C8 needs them; do not relocate existing working configuration just to match this tree. This planning task creates no source files.

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml
package.json / pnpm-workspace.yaml / pnpm-lock.yaml
crates/
  avesra-contracts/       # Wire messages, typed actions/events, driver capabilities
  avesra-core/            # Tasks, policy, model adapters, memory, learning, routing
  avesra-server/          # Spark service, authenticated transport, health/config API
  avesra-windows/         # Audio, app registry, capture, UIA, tools, local checks
apps/
  desktop/               # Svelte/Tailwind assets + Tauri Rust shell
  browser-extension/     # Chrome/Brave MV3 content/service-worker UI
services/
  audio/                 # Isolated ASR/speaker/TTS processes and pinned Python deps
config/
  profiles/              # Example single-Spark assignments, no secrets
  models.lock.json       # Validated model/runtime artifact revisions and limits
deploy/
  spark/                 # Compose/service manifests, health and resource settings
  windows/               # Installer/native host registration and user startup
scripts/
  verify.ps1             # Existing Windows Static/Build gates
  verify.sh              # Existing Spark Rust static/build gates
docs/
  product.md / architecture.md / protocol.md / drivers.md
  security.md / verification.md / operations.md / learning.md
plans/
  README.md / 001-single-spark-assistant.md
design/                  # Existing: approved UI mockups + Tailwind source (reference only, not shipped)
```

Prefer concrete modules within these crates over a crate per conceptual lane. The contract crate has no Tauri, Windows, or model-runtime dependencies. The server/core must build on Linux ARM64 without desktop libraries; Windows modules are target-gated. The desktop shell calls typed Rust commands, not arbitrary frontend shell execution.

Use Rust `Result`/typed errors, explicit cancellation and deadlines, bounded queues, and small responsibilities. No blocking network/model work on the UI/audio callback threads. Use a single owner for each mutable device/desktop/task resource. Use SQLite schema migrations and transactions rather than writing state to scattered ad hoc JSON files.

Preserve pinned toolchains/dependencies and update lockfiles only for a required scoped dependency change. Use formatting, clippy, TypeScript/Svelte checks and builds as specified in section 15, without adding automated tests. Reuse the existing Tokio, serde, HTTP/WebSocket, SQLite, CPAL/Windows and Tauri integration; validate changed versions and licenses before replacement. Do not introduce a second generic agent framework merely to provide memory or orchestration.

Git branch prefix: `codex/`. Preserve the existing LICENSE and unrelated work; stage only scoped files when publication is requested. Earlier publication/execution events are historical, not a request to commit or deploy this planning refresh. Do not create GitHub workflows as a substitute for running local/live gates.

## 15. Completion sequence and verification commands

### Execution scope and current gates

This section replaces the original greenfield M0-M9 implementation instructions. M0-M9 remain product milestone identifiers in the index; C0-C9 below are the ordered remaining implementation slices. Completing one slice does not mark the corresponding whole milestone DONE. The planning revision changes only this plan and `plans/README.md`; source paths below are scope for a later executor.

Use an isolated `codex/` worktree for execution when concurrent work overlaps. Select its starting commit deliberately after reading the current dirty diff; do not drop the enrollment/UI changes merely because they are not in HEAD. Do not merge, publish, change Spark services or perform account actions solely because this planning document exists. Existing Computer Use restrictions remain in force. Hidden native inspection must follow the existing handoff's permitted method and physical-store checks; do not interpret it as permission to operate the user's desktop or microphone.

The commands below exist in the inspected source. They were read, not run, during this planning revision. Run them in the executor checkout after changes. Install dependencies only if missing in that checkout, using the committed lockfiles. No automated tests, fixtures, acceptance runner or report-verifier implementation is in scope.

| Gate | Exact command | Required result and limits |
| --- | --- | --- |
| W-static | `pwsh -NoProfile -File scripts/verify.ps1 -Suite Static` | Exit 0: `cargo fmt --all -- --check`, locked workspace clippy with warnings denied, recursive frontend checks. Does not prove runtime behavior. |
| W-build | `pwsh -NoProfile -File scripts/verify.ps1 -Suite Build` | Exit 0: recursive frontend build, then locked workspace release with `avesra-desktop/custom-protocol`. Not installation/signing. |
| S-static | `bash scripts/verify.sh static` | Exit 0 on the actual Spark ARM64 build environment; formats workspace and checks contracts/core/server. No Windows or Python/audio qualification. |
| S-build | `bash scripts/verify.sh build` | Exit 0 in that same environment; locked release of contracts/core/server. No model image or inference qualification. |
| Scope | `git diff --check` and `git status --short` | No whitespace errors; every changed/untracked path has an owning slice. Preserve the recorded pre-existing dirty set. |
| Source drift | Commands in the opening drift check | Record HEAD/worktrees/dirty paths and resolve excerpt mismatches before edits. |

Run W-static for every changed slice, W-build when frontend assets/native packaging change, and S-static/S-build whenever shared contracts/core/server changes. For a frontend-only slice use W-static and W-build; do not rebuild unchanged Spark code. Record exact command, source commit plus dirty scope, environment and exit status. A failed gate requires a focused fix; a second failed attempt becomes an explicit blocker, not an expanded rewrite.

For Python audio changes inspect `services/audio/pyproject.toml` and its existing serving entry points, then record import/startup validation under the actual pinned serving environment when available. Rust checks cannot establish Python/audio compatibility. Do not invent a nonexistent audio verification wrapper or mark an unavailable environment PASS.

### C0 — Reconcile the actual baseline and protect working data (M0/M1)

**Scope:** read-only source/worktree inventory; updates to `docs/verification.md`, `docs/operations.md`, `docs/evidence/m0-preflight.md`, `plans/001-handoff.md` and this index during execution. Do not alter concurrent enrollment code as part of reconnaissance.

1. Record the opening drift commands, current dirty/untracked paths, and current actor owning overlapping changes. Read the latest handoff first; its former `codex/companion-onboarding`/executor-worktree labels are not current branch facts.
2. Confirm the real normal-user data directory using the existing canonical-path diagnostics. Preserve selected endpoint IDs, the recovered protected enrollment candidate and registration. A missing candidate in a redirected process is not a reason to overwrite or recollect it.
3. Refresh only facts needed for the next slice: exact Spark2 service/model revisions and lifecycle owners, installed app/browser surfaces and selected device identity. Do not stop unrelated Local Studio, ComfyUI or other model jobs. Record inaccessible runtime facts as pending.
4. Capture a current static/build baseline using the gates above, with unrelated failures attributed separately. Carry completed code forward; do not restart M1.

**Verify:** Scope plus applicable Windows/Spark gates; a dated baseline record distinguishes committed source, dirty work, historical evidence and newly observed facts. Missing live access blocks live qualification, not independent source/spec work.

### C1 — Qualify and activate owner-aware continuous voice (M2/M3/M4)

**Scope:** `crates/avesra-core/src/{voice,enrollment,state}.rs`, voice/media/enrollment contracts under `crates/avesra-contracts/src/`, `crates/avesra-server/src/{voice_stream,voice_setup,audio_stream,transport}.rs`, `services/audio/avesra_audio/{service,drivers,streaming_asr}.py` and their pinned audio manifests, `apps/desktop/src-tauri/src/{voice,voice_check,microphone_check,phrase_capture,profiles,media,setup,connection}.rs`, minimal runtime/People UI wiring, and `docs/{turn-gating,enrollment,owner-identity,streaming-asr}.md`. Preserve concurrent edits in these files; resolve ownership before implementation.

1. Specify a native qualification record bound to candidate, registered actor/grant, selected endpoint, model/adapter revisions and calibrated signal/speaker operating point. Actual explicit calibration/review evidence creates the private `QualifiedProfile`; persisted candidate selection, a UI boolean or a model-reported confidence cannot construct it. Invalidation on endpoint/profile/grant/model change must be immediate.
2. Replace the fixed one-attempt-per-epoch transport with bounded continuous VAD/endpointing and explicit utterance lifecycle. Supply measured signal, overlap, echo and directed-intent evidence. Unknown evidence continues to abstain. Preserve transient handling for ambient/unknown speech and no raw-media archive.
3. Reconcile the current lexical name-prefix guard (`voice.rs` around `addressed`) with section 3's **no mandatory wake word** contract. A measured directed-intent path must accept clearly addressed requests without a prefix while rejecting teammate conversation. Do not simply delete the guard and treat every recognized owner sentence as a command. Bind short follow-ups to the existing one-use invitation and expiry.
4. Replace `analyze(..., None, ..., None)` only after current native qualification exists. Derive actor/grant/session/epochs from authoritative native state; keep setup recording, speaker checks, greeting and generated preview separate. Opening Settings or reading saved enrollment must not start recording or activate listening.
5. Persist only accepted conversations through existing `conversations.rs` / `conversation_tasks.rs` boundaries (extend those files only for required admission integration). Unaccepted transcripts must not reach UI, history, planner or learning. Expose native readiness/abstention reasons without optimistic completion.

**Verify:** W-static/W-build and S-static/S-build. During authorized direct qualification record A02-A07 outcomes, including owner requests without a wake word, owner speech to teammates, non-owner/recorded speech, overlap, assistant playback, device removal, revocation and local mute with Spark unavailable. Under the later development-shipping override, short measured owner setup may enable explicitly consented development listening; keep full C1 release qualification pending until its required positive/negative evidence exists. Do not create an automated evaluation runner or request another six-segment enrollment solely to repair UI state.

### C2 — Activate reasoning and connect one accepted spoken response (M0/M2/M3)

**Scope:** `crates/avesra-server/src/reasoning{.rs,/http.rs,/deployment.rs,/jobs.rs,/stream.rs}`, `planner_ingress.rs`, `transport.rs`, `normal_speech.rs`; `crates/avesra-core/src/{conversation_planner,conversation_tasks,conversations}.rs`; `apps/desktop/src-tauri/src/{planner,voice,speech,output}.rs`; existing planner/TTS contracts and `docs/{reasoning-adapter,planner-driver,planner-ingress,native-planner,normal-speech,streaming-tts}.md`.

1. Define the concrete qualification procedure for loaded model artifact, immutable request routing to the actual incarnation, context capacity and terminal/drain semantics. Inspect the installed Local Studio interface before choosing the binding. Its dynamic proxy model label and before/after metadata are insufficient to rule out replacement during a request. If the interface cannot supply the required binding, scope an explicit adapter change; do not create a `qualified: true` configuration shortcut.
2. Implement qualification/revocation using actual bounded runtime observations. Only then provide `QualifiedDeployment` to server startup in place of `reasoning: None`. Missing or stale qualification retains unavailable status. Preserve the single actual job owner, durable uncertainty and non-retried original-budget request.
3. Connect C1's opaque accepted conversation to the same-worker durable `PlannerClaim`, existing `planner::answer`, `StoredReply`/publication and normal-speech output ownership. Commit accepted state before presenting acceptance. Never add a frontend arbitrary-transcript admission command.
4. Finish long-response segmentation, bounded TTS scheduling and cancellation under the same original reply/output authority. Playback drain, transport completion and observed speech remain different states; no task success from a generic acknowledgment.
5. Instrument acceptance, queue, reasoning, TTS and actual playback using host-local monotonic spans before extending tool execution. Preserve failed/cancelled samples and content redaction.

**Verify:** applicable four gates. Authorized direct operation must produce one genuine spoken answer through the shipped path, then demonstrate Stop, caller loss, disconnect and model-incarnation change without stale speech, duplicate inference or clearing uncertain jobs. This is a conversation checkpoint, not OW1-OW3 completion. C2 source work can proceed while C1 qualification is pending; activation requires both.

### C3 — Connect the existing first useful actions (M2/M4)

**Scope:** `crates/avesra-core/src/{execution,ledger,policy,conversation_tasks,apps}.rs`, action/planner contracts, `crates/avesra-windows/src/{effects,apps,volume}.rs`, native planner/catalog/runtime projection and minimal task UI, `docs/{native-actions,conversation-admission,app-catalog}.md`.

1. Feed exact typed app-launch and volume proposals from accepted C2 planning into the existing task/step ledger, grants and `DispatchPermit`. Models select supported actions; they do not select arbitrary executables, command lines, approval values or success outcomes.
2. Resolve aliases to immutable actual catalog records and clarify ambiguity. Revalidate current action/session/actor/target immediately before effect. Reuse installed/package app handling and observed volume result instead of a new shell/IPC bypass.
3. Show durable task status and exact cancellation in the existing UI. Map verified effect, unsupported target, needs-input and unknown-effect separately. Mic mute after acceptance must not silently delete an accepted task; Stop must withdraw action/output ownership.

**Verify:** W-static/W-build and shared-code Spark gates. Directly ask Avesra to open a configured app and adjust volume; observe actual app identity/final level and correlated history. Repeat cancellation/reconnect at effect boundaries and verify no uncertain replay. A launch-only result still fails OW1.

### C4 — Complete the owned browser-read lifecycle (M2/M5)

**Scope:** `crates/avesra-core/src/{browser_execution,browser_jobs,browser_reading,execution,ledger}.rs`, browser wire contracts; `crates/avesra-windows/src/{effects,browser_read_channel,browser_receive,browser_pipe}.rs`; `apps/desktop/src-tauri/src/browser{.rs,/reading.rs,/documents.rs}`; `apps/browser-extension/src/{background,browser-job,reading,read-job,page-excerpt,protocol,documents,authority}.ts`, `manifest.json`; `docs/browser-read-{channel,execution}.md` and browser-reading/observation specs. Execution refinement: `read-job.ts` isolates the specified actual Chrome job/outbox from `reading.ts` wire validation; this adds no new permission or entry point.

This is one coherent integration slice; do not activate individual pieces early. Existing preparation and settlement machinery is retained.

1. Extend private `Offer`/`Prepared` with paired publication/content endpoints bound to the original Shared/Withdrawal. The actual ledger worker retains its endpoint and borrowed Store/ReadExecution; the native coordinator retains its matching endpoint. No public sender accepting arbitrary Context/Reply.
2. Define held, possibly-published and terminal states. Publish at most once under the original deadline. Remove `status.read` before acknowledging settlement without prematurely cancelling valid already-sent content. A failed or lost publication after possible send leaves durable uncertainty.
3. Implement the active loop on the original worker. Service content and settlement there without enqueuing work behind that same blocked worker. Keep one actual pipe receive owner through partial frames; do not cancel `ReceiveOwner.receive` merely to poll another channel.
4. Consume the borrowed opaque `ReadReply` while the actual resource reservation and `State.active` withdrawal registration remain held. This is the default lifetime design. If the consumer must outlive that borrow, specify a concrete registered successor/checker before changing lifetimes; disarming Drop or adding a completed flag is insufficient. Scope, task, actor, registration, permission and action changes continue to withdraw through consumption.
5. Build the extension actual-job owner using shared `acquireBrowserJob`. Hold it through all Chrome promises and exact-document cleanup even after timeout/disposal. Retain one metadata-only settlement outbox until matching native acknowledgment. Seen IDs are bounded with no eviction/reinjection. A new service worker or navigation cannot claim settlement for abandoned work.
6. Correct `beginPageExcerpt`'s `started:false` meaning: an existing same-request guard/tombstone does not prove no owned guard. Distinguish proven never-started absence from unknown cleanup; a rejected/lost injection or finish remains uncertain. Recheck exact document/origin/permission/observation after every await; inject only bundled isolated-world code into the selected exact document.
7. Only after steps 1-6 integrate `ReadPage` dispatch, extension imports and the minimum scripting permission. Setup metadata still grants no reading authority. Accept content only after actual settlement and matching current Excerpt/Empty; durable evidence stores bounded provenance/digests, not page bodies. Generic excerpts cannot prove mailbox latest-N completeness.

**Verify:** W-static/W-build and S-static/S-build. Review the complete lifetime/diff before activation. Authorized direct reads must cover actual Chrome and Brave, navigation/permission loss, caller drop, duplicate request/receipt, lost acknowledgment and browser restart. Success requires exact observed content plus settled resources; uncertain jobs block replacement. C4 source can be completed before C1, but no setup or debug path may manufacture its accepted producer.

### C5 — Deliver the three concrete owner workflows (M5)

**Scope:** existing app/browser action contracts, policy/ledger/planner integration, catalog and browser modules from C3/C4; new narrowly scoped `crates/avesra-windows/src/prompt.rs`, `crates/avesra-core/src/workflows.rs`, and `apps/browser-extension/src/mailbox.ts` if no equivalent exists after drift check; associated module registration, required UIA dependency entries, setup/task UI and `docs/owner-workflows.md` (new). Do not build a general script interpreter.

1. Specify typed operation/precondition/postcondition contracts before wiring new effect types. Reuse exact actor, approval, target, source and action revisions. Identify actual supported Claude/Codex windows and browser profile/account surfaces through permitted preflight; do not guess selectors or silently substitute a CLI.
2. Implement OW1: select actual Claude and the configured IRIS project, create a fresh chat, verify the input surface and draft policy, type exactly `123`, read back its exact text and leave unsubmitted. Preserve existing unsent drafts or ask for a decision. Focus loss, wrong project, modal, inaccessible input and app version drift yield needs-input/unsupported; never type into a shell by accident. Add submission only behind a separately explicit start/run request and its exact approval rules.
3. Implement mailbox-specific enumeration/read evidence for OW2 over C4's owned browser operations. Bind selected account and visible configured Inbox scope, enumerate message identities newest-first across threads/pagination, deduplicate and prove ten messages or the true smaller count. A partial DOM excerpt or ten thread rows is insufficient. Distinguish delivered/shipped/out-for-delivery, cite message source/date and retain no raw content in telemetry. Ambiguous packages/account or inaccessible bodies produce an honest incomplete result.
4. Implement OW3 navigation/focus to the intended X site/profile/account and verify ready state. Handle login/CAPTCHA as user input. No draft generation, send or publish from this request. Preserve exact-origin grants during redirects/navigation.
5. Complete initial browser current-topic/source summaries and explicitly selected Codex/terminal prompt surfaces under section 9. Keep unsupported surfaces visible; core owner workflows are mandatory even if an additional surface is unavailable.

**Verify:** applicable four gates. Record each OW1-OW3 result independently through real spoken input, local inference, Avesra drivers and observed postconditions. Redact email/project details. Both supported browsers require direct workflow coverage. Manual agent clicking/typing or a fixture result is not an Avesra pass. If account/app access is unavailable, finish independent source and report the specific pending live gate.

### C6 — Finish learning, history and explainable notifications (M6)

**Scope:** core storage/apps/conversation modules; new `crates/avesra-core/src/{memory,routines,notifications}.rs` and module registration if absent; scoped native/browser observation integration, existing Memory/Apps settings views and `docs/{memory,learning,notifications}.md` (new).

1. Add versioned migrations for sourced facts, routine candidates, validation state, corrections and committed event batches using the existing single writer. Do not recreate existing task/conversation storage. Define deletion of dependent retrieval/index copies and private per-actor access before adding retrieval.
2. Connect explicit demonstrations and permitted change-aware observation. Unknown speech, excluded apps, locked screens and credential fields provide no retained learning content. Bound background work and prioritize accepted conversation/actions.
3. Execute validated routines only by proposing ordinary policy-checked steps. Changed targets, source drift or grants suspend/clarify instead of granting more authority. Keep candidates visibly distinct from validated routines.
4. Tie learning/completion chimes to committed useful events/verified effects; persist the exact announced batch for follow-up answers. Coalesce/rate-limit and suppress duplicate backlog chimes after reconnect. Keep deafen/pause authoritative.

**Verify:** applicable four gates. Direct demonstration -> recall after restart -> invocation -> correction -> deletion, with per-user isolation and privacy inspection. Ask about an announced batch after newer silent events; answer must cite that batch. These checks require genuine application behavior, no fixture harness.

### C7 — Implement bounded diagnostics and the configured VPN (M7)

**Scope:** new `crates/avesra-windows/src/diagnostics.rs` and `vpn.rs` if needed, typed action/result contracts, core policy/routine integration, exact-proposal approval/task UI and `docs/diagnostics-vpn.md` (new).

1. Implement a fixed reviewed read-only diagnostic catalog with bounded output/duration and owned process cancellation. Capture unavailable counters explicitly; model labels cannot convert generated scripts to read-only catalog operations.
2. Ground slow-download explanations in actual throughput/settings/network/disk evidence. Present concrete configuration-changing proposals with exact target/parameters and fresh approval. Arbitrary scripts remain a separate review boundary.
3. Identify the installed VPN client/version and approved control surface, then implement a normal verified routine. Leave MFA to the owner; observe connected state and Spark reachability. Do not modify corporate policy, bypass security or persist credentials.

**Verify:** applicable four gates plus authorized direct read-only diagnosis and VPN observation. Demonstrate revoked/changed/expired approvals prevent writes and routing loss does not replay actions. Perform a reversible fix only with its exact approval. Unknown VPN product/surface blocks its adapter, not unrelated diagnostics.

### C8 — Complete deployment profiles, packaging and instrumentation (M1/M8)

**Scope:** current driver/server configuration, audio service manifests, profile/runtime settings, new focused telemetry/routing modules as required within existing crates, `deploy/{spark,windows}/` and `config/` when needed for packaging, native-host installation scripts, Tauri bundle configuration and `docs/{drivers,operations,verification}.md`; no unrelated host service changes.

1. Finish per-stage tracing/rollups and the local Performance view specified in section 16. Thread correlation through every slice; retain failures, queue delays and clock uncertainty. Provide bounded redacted export and same-scenario baseline/candidate comparison without a test runner.
2. Qualify compatible model replacement before switching; retain/drain actual jobs, preserve tasks/history and reject incompatible replacement. Single-Spark/Gaming must not retain client model allocations. Opt-in Accelerated requires measured benefit, supported capacity and safe transition under actual game load; an unavailable UI option does not satisfy A27.
3. Keep optional Jev disabled without explicit configuration. If enabled, implement only section 13's schema, explicit egress consent/redaction and local fallback. Missing optional credentials do not block local completion; required local behavior never depends on Jev.
4. Package the actual Windows app/native host/selected extension setup and owned Spark services, with pinned artifacts, readiness timeouts, startup order and logs. Preserve normal-user storage and existing protected identity during updates. Document install, upgrade, rollback, revocation, backup/restore and uninstall; memory deletion remains a separate decision.
5. Verify restart/sleep/resume, unplugged devices, browser/service restart, disk-full and lost-effect acknowledgment through existing reconciliation. No generic clear-uncertainty switch or automatic GUI replay.

**Verify:** W-static/W-build and S-static/S-build, actual artifact identity, and authorized direct install/update/recovery observations. Record cold/warm model loads, replacement, Spark-only operation, Accelerated/Gaming transitions, egress and Performance-view results. Build success is not deployment or runtime qualification.

### C9 — Complete live release evidence and close the plan (M9)

**Scope:** redacted `docs/evidence/release.md`, current operations/verification docs, plan status/handoff; implementation fixes only through the owning slice above.

1. At the release source/artifact/config revision, perform every required A01-A29 case in section 16 by direct observation. Keep the scenario IDs and quantitative targets; no automated runner/verifier is required or authorized. Record PASS/FAIL/BLOCKED per case, actual environment/model/app versions, timestamps, sample counts, observations, measured values and missing evidence.
2. Include the real OW1-OW3 spoken workflows, positive and negative identity/directness corpus, latency under contention, local-control response, gaming impact, privacy/storage inspection and eight-hour soak. Purposeful live recordings/screenshots require the applicable explicit capture permission and local retention controls; no implicit ambient corpus collection.
3. Re-run only scenarios affected by later source/config/device changes, and clearly link any retained earlier evidence to unchanged prerequisites. A failed required gate remains failed; do not discard failures from latency/accuracy totals or substitute a manually completed action.
4. Manually review evidence completeness against every section 19 checkbox. Missing OW1-OW3 or positive-owner acceptance prevents DONE, regardless of build success. Optional disabled Jev is explicitly not applicable; deferred lights are out of scope. Other requirements require evidence or an explicit owner scope revision.
5. Update the index and handoff with implemented slices, remaining blockers and exact evidence references. Use IN PROGRESS/PARTIAL until all required gates pass; use BLOCKED only for a specified dependency that prevents further relevant work. Planning completion and source completion are separate from release completion.

**Verify:** final applicable build/static gates for the actual release revision; every required acceptance row has fresh matching direct evidence; eight-hour duration and numerical targets are recorded; the owner sees the actual workflow results. No report-verifier command is invented.

### Dependencies and stopping boundaries

Execute C0 first. The main useful-product path is C1 -> C2 -> C3 -> C5, with C4 required for C5's browser operations. C2/C4 source can progress while C1 live qualification is unavailable; keep their activation gated. C6 follows accepted conversations and useful action/browser evidence; C7 uses C3/C6 policy and routines. Instrument each slice as it lands; C8 completes deployment/telemetry and C9 closes live release evidence. UI polish and Plan 002 sound design cannot replace these dependencies.

Stop the affected slice if actual source conflicts with an excerpt/contract, concurrent ownership is unresolved, the plan would require a new authority bypass, or the proposed change needs paths outside its scope. Revise the specification before broadening architecture. Do not stop independent source work solely because a human audition, account challenge or long live qualification session is pending. Do not mark a dormant implementation live-ready to avoid a blocker.

## 16. Acceptance matrix and quantitative targets

These are required first-release acceptance targets, not measured capabilities. The operator may explicitly revise a target after seeing M0 evidence; the executor must not lower it silently. Under the specs/code override, the suite labels below identify groups of directly observed scenarios, not automated commands. Small finite evaluation sets establish a release gate, not a universal false-accept guarantee.

| ID / owning suite | Scenario and mandatory outcome |
| --- | --- |
| A01 / transport-policy | Unpaired/revoked peer, malformed/oversized frame, unknown version and stale action epoch are rejected with no desktop effect. |
| A02 / voice-identity | Owner enrollment succeeds with held-out validation; poor/overlapping input requests retry rather than pretending enrollment is valid. |
| A03 / voice-identity | At least 100 representative clearly directed owner requests; >=95% accepted correctly, with per-condition breakdown. False rejection cannot be hidden by reducing coverage. |
| A04 / voice-identity | At least 200 non-owner/recorded/playback/overlap command attempts and 200 owner-but-not-addressing-Avesra utterances; zero unauthorized effects in this corpus. Document replay/synthesis limits separately. |
| A05 / voice-identity | Speaker switches within a conversation and short ambiguous approvals do not borrow the previous speaker's authority. Owner revocation/settings take effect immediately. |
| A06 / voice-identity | End of a simple owner utterance to first useful spoken response: initial gate median <=1.5 s and p95 <=3 s after services are warm, with an optimization goal of median <=0.75 s and p95 <=1.5 s. The faster goal is unverified, not a product guarantee. Report identity/ASR/planning/TTS components separately; a canned acknowledgment alone does not satisfy it. |
| A07 / transport-policy | Mute/deafen/stop gives local visual feedback and stops relevant local capture/playback within 150 ms p95, including a stalled/disconnected Spark; late buffers cannot start a new task. |
| A08 / desktop-controls | Minimize/close-to-tray preserves selected mode; Quit/lock stops capture; mixed-DPI input targets remain correct; no task typing into a changed foreground window. |
| A09 / desktop-controls | Five launch/volume cycles each verify the actual result; remembered alias survives restart; ambiguous alias never launches an arbitrary candidate. |
| A10 / browser-workflows | Latest requested N individual Gmail messages in intended mailbox, or actual smaller count, are inspected with no send/delete/archive. Test N=3 and N=10, thread grouping, delivered/shipped/out-for-delivery/no-match/multiple-package cases, multiple accounts, sign-in challenge, empty inbox and UI drift. |
| A11 / browser-workflows | X AI-topic summary includes current observed sources/view/time; login/blocked page is reported; no fabricated trend or post. Both Chrome and Brave pass. |
| A12 / app-prompts | Correct app/project/input and text. Draft remains unsubmitted without explicit start/run. Explicit submission occurs once; ack loss never blindly resubmits. |
| A13 / app-prompts | Explicit terminal case distinguishes AI input from shell input. No terminal fallback when the user requested a desktop app. |
| A14 / learning-memory | Successful app resolution automatically remembered; explicit demonstration and passive observation produce sourced candidates; a candidate has no extra permission. |
| A15 / learning-memory | Learned routine succeeds after restart and harmless window relocation; changed app structure causes reobservation/needs-input, not blind coordinate replay. |
| A16 / learning-memory | Accepted history retained; excluded/unknown speech and raw media absent from app-managed storage. Deleted fact disappears from retrieval/derived copies; other-user private memory cannot be retrieved. |
| A17 / learning-memory | Learning sound follows a committed useful event; completion sound follows verified outcome. "What did you learn/do?" returns the announced event batch even with later silent events. No duplicate backlog sounds after reconnect. |
| A18 / diagnostics-vpn | Diagnosis uses real evidence, correct throughput units and unavailable states. No configuration change before exact approval; cancelled/changed approvals cannot execute. |
| A19 / diagnostics-vpn | Existing VPN connection verified, MFA pauses, routing loss handled. No security-policy modification or credentials captured. |
| A20 / routing-recovery | Compatible Local Studio model swap preserves history/tasks; incompatible swap rejected; failed replacement retains or explicitly reloads the last working deployment; no fallback outside the selected profile. |
| A21 / egress | Core works with all assistant cloud AI destinations blocked. If Jev enabled, only approved schema reaches its endpoint; media/identity/memory/secret canaries never do. Browser/app traffic is separately accounted for. |
| A22 / routing-recovery | Controller/client crash before/after mutation produces correct reconciliation; duplicate action IDs do not repeat known completed effects. Disk full fails visibly without inventing saved memory. |
| A23 / release | Eight-hour soak: no runaway queues/unbounded resident-memory growth, no spurious executed command, no raw-media accumulation, and successful recovery of tested failures. |
| A24 / owner-workflows | OW1 passes live: recognized owner request opens actual Claude, selects IRIS, creates a new chat, types exactly `123`, verifies the correct field/project, and does not submit. |
| A25 / owner-workflows | OW2 passes live: inspect actual latest 10 mailbox messages (or report fewer), answer the delivery question using their evidence, identify relevant source, and avoid conflating shipment with delivery. No invented confirmation. |
| A26 / owner-workflows | OW3 passes live: X is ready in the correct browser/profile/account for the owner to post, with no generated/published post or unauthorized write. |
| A27 / routing-recovery | On a qualified client, Accelerated placement shows measured benefit; Gaming releases client model allocations and continues on Spark without duplicate effects, stale speech, or lost task state. Low-memory, unavailable-client, and rapid-load-change cases recover without routing oscillation. |
| A28 / telemetry | One request can be followed across client, controller, inference, tool dispatch, observed effect, and playback. Queue time, retries, errors, and abandoned requests remain visible; timestamps from unsynchronized machines are never subtracted as if synchronized. Content and biometric canaries are absent from telemetry. |
| A29 / optimization | A model/profile change produces a baseline-versus-candidate report for identical versioned scenarios, including cold/warm and contention runs. A faster candidate with worse required accuracy, unauthorized effects, or missing observations is rejected. |

### Instrumentation and continuous optimization

Metrics are a first-release capability, established in M1/M2 and extended with each lane, not a later dashboard project. Use structured Rust tracing and OpenTelemetry-compatible spans at service boundaries; consume Local Studio/engine metrics where available. A local collector/exporter and bounded local storage are sufficient initially. Do not require a cloud observability service.

Every accepted turn/task has an opaque correlation ID propagated through client, controller, driver request, tool step, observed result, and spoken reply. Each span records operation, lane, model revision/quantization, engine image/version, device, resource profile/config hash, queue duration, active duration, result, retry count, cancellation, and error category. Correlation IDs belong in traces, not high-cardinality metric labels. Do not put transcripts, email contents, screenshots, audio, prompts, secrets, or speaker embeddings in metrics/traces. Unknown/ambient speech may increment aggregate gate counters but must not create retained conversation records.

| Stage | Required measurements |
| --- | --- |
| Capture and transport | Capture-to-send age, audio gaps/overruns, encode/decode time, bytes transferred, RTT, jitter, loss/reconnects, screen-capture cost and observation age |
| Speech activity and ASR | First partial, final transcript, end-of-speech-to-final delay, endpointing delay, real-time factor, transcript revisions and cancellation latency |
| Identity and intent | Decision time, accept/reject/abstain counts; false acceptance/rejection and wrong-speaker command rates on labeled evaluation data; overlap, noise and insufficient-speech outcomes |
| Reasoning and vision | Queue/prefill time, time to first useful output, decode rate, completion time, context/image size, parser/schema failures, grounding failures and correction loops |
| Tools | Dispatch-to-start, execution and verification time, focus/target mismatches, stale-state rejections, retries, permission prompts, uncertain effects and verified success/failure |
| Speech output | First generated audio, time until client playback actually begins, first meaningful spoken response, total playback time, underruns and interruption-to-silence |
| Learning and memory | Retrieval/index/write latency, retrieved evidence age, committed versus rejected memories, routine reuse success and chime-to-event correlation |
| Hardware and lifecycle | CPU/RAM/VRAM, Spark unified-memory headroom, GPU load/temperature/power where supported, queue depth/age, cold-load time, swap time, game frame-time impact and profile transitions |

Use monotonic clocks for durations on each host. Measure user-visible end-to-end latency on the client: speech end to first meaningful audible response, speech end to first verified action, and speech end to verified task completion. Do not count a generic acknowledgement as task completion or meaningful answer. Record human approval/MFA wait separately from active processing while preserving total elapsed time. Cross-host spans require clock-offset/uncertainty metadata or host-local durations; missing counters are unavailable, not zero.

External engine/controller counters may include other applications and earlier sessions. Attribute measurements by deployment/process identity and capture counter deltas around the evaluation window; detect resets and model swaps. Preserve the owner's existing Local Studio history and statistics. Never label lifetime provider totals as Avesra usage or reset shared counters to simplify a benchmark.

The settings page needs a compact Performance view: current lane/model/device, live queue and headroom, p50/p95/p99 plus maximum, sample count, errors/abstentions, and the largest measured contributor to delay. Provide a redacted local export and baseline comparison. Preserve failures/timeouts in totals; never calculate success latency only and hide failures. Keep detailed traces bounded (initial default seven days, configurable), rollups for thirty days, and explicitly saved benchmark reports until deleted. This telemetry retention is separate from indefinite accepted conversation/task history.

Establish a versioned evaluation corpus around OW1-OW3, volume, diagnostic requests, ordinary owner conversation, unknown voices, interruptions, ambiguity and UI changes. Real voice samples require explicit test capture as already specified; synthetic fixtures are labeled and cannot satisfy live-owner gates. Evaluate task success, unauthorized effects, exact prompt contents, evidence-grounded email conclusions, false accepts/rejects, ASR word/entity error rates, and recovery behavior alongside latency. Zero observed mistakes in a finite run is a release result, not a promise of zero future mistakes.

Compare baseline and candidate on the same device, model/config revisions, scenario set, and concurrency. Separate cold/warm runs; include idle, observation load, queued background work, LAN impairment, and actual gaming. Use at least thirty repetitions per deterministic interactive scenario for initial comparison; tail quantiles with inadequate sample counts are marked provisional. Record warmups and measurement overhead. Performance wins cannot waive any required correctness gate. Before promotion, run the affected live workflows and allow explicit rollback to the previous configuration.

Automatic optimization may adjust bounded scheduling, batching, observation rate, or placement inside approved profiles. Model changes, identity thresholds, tool grants, and consequential-action rules do not silently self-modify. Failed experiments leave the last working configuration recoverable.

### Performance/resource evidence

- Measure PC process CPU/memory, capture cost, and frame-time impact with overlay visible/hidden while gaming. Initial engineering budget: idle companion average <=2% of total PC CPU and <=500 MiB working set over a 10-minute idle interval; adjust only with recorded evidence and owner review. Report hardware/method because CPU percentages are machine-dependent.
- The idle companion budget excludes separately measured opt-in inference workers. Report their CPU/RAM/VRAM and power independently and in the total; do not hide inference overhead by assigning it to another process. For Gaming, compare game frame-time distributions with Avesra off/on under matched conditions, including run-to-run variation.
- Verify no Avesra model process or model-weight allocation appears on the 5090 in the single-Spark profile. UI rendering activity is allowed and measured separately.
- Report Spark total working memory/headroom, model load time, interactive latency under observation load, audio underruns, queue age and dropped/coalesced passive frames. Averages alone are insufficient; include median/p95 and worst observed failures.
- Short app/volume requests must not wait behind background learning. Demonstrate priority with an artificially saturated background queue.
- Throttle/pause background work before dropping live conversation. If the chosen model mix cannot meet speech targets, change a candidate or report the gate unmet; do not pretend a large model fitting in RAM proves conversational usability.

### Release-report rules

Reports identify the actual live inputs and include observation counts, environment/config revisions, source commit and dirty scope, scenario IDs, measured values, and evidence timestamps. Review completeness directly; do not implement an automated verifier. Missing, skipped, stale, synthetic-only or mismatched-configuration evidence cannot satisfy required live scenarios. An all-reject speaker/intent implementation fails the positive owner-request target even when no unauthorized effects occur.

The release review must specifically reject absent/failed/blocked OW1–OW3 evidence even when build/static checks and other scenarios pass. Product instrumentation observes Avesra's steps; it must not secretly perform the requested workflow itself.

Direct qualification evidence may include temporary recordings/screenshots only when the operator explicitly enables that collection; keep it separate from product capture, clearly labeled, locally protected, excluded from commits/support bundles, and deleted when the qualification session ends unless the operator deliberately retains it. Do not create an automated fixture collection. Product operation remains transient-media-only.

## 17. Primary references and verified limits

These sources informed the plan. Recheck applicable APIs/model recipes during implementation; source documentation does not substitute for local proof.

- [Tauri architecture and webview approach](https://v2.tauri.app/start/), [tray integration](https://v2.tauri.app/learn/system-tray/), [window configuration](https://v2.tauri.app/reference/config/), and [capabilities](https://v2.tauri.app/security/capabilities/) support the proposed thin desktop shell. Limit privileged frontend commands to typed application operations.
- [Microsoft UI Automation security](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview) explains integrity-level restrictions. Do not treat an ordinary app as able to automate UAC/protected desktops.
- [Windows EndpointVolume API](https://learn.microsoft.com/en-us/windows/win32/coreaudio/endpointvolume-api) is the native basis for master output-volume control.
- [Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging) documents registered native hosts and origin constraints; [extension debugger API](https://developer.chrome.com/docs/extensions/reference/api/debugger) is an optional controlled capability, not blanket model access. [Remote-debugging changes](https://developer.chrome.com/blog/remote-debugging-port) require attention when interacting with everyday browser profiles.
- [NVIDIA Spark vLLM instructions](https://build.nvidia.com/spark/vllm/instructions) provide a container-serving starting point. They do not prove all model/quantization/runtime combinations work on this user's machine.
- [Qwen3.8-27B](https://huggingface.co/Qwen/Qwen3.8-27B) documents multimodal and configurable reasoning capabilities. [Qwen3.6-35B-A3B](https://huggingface.co/Qwen/Qwen3.6-35B-A3B) is a candidate comparison, not an established speed winner here.
- [Nemotron streaming ASR](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b) documents a local streaming recognition candidate; [Qwen3-TTS](https://github.com/QwenLM/Qwen3-TTS) documents voice design/reuse. Real full-pipeline latency remains unverified.
- [SpeechBrain ECAPA](https://huggingface.co/speechbrain/spkrec-ecapa-voxceleb) provides speaker embeddings/verification; [pyannote](https://github.com/pyannote/pyannote-audio) provides diarization components. Neither makes voice identity infallible or grants authorization.
- [TypeSafe/Jev](https://docs.typesafe.ai/introduction/quickstart) documents a hosted typed-decision API. The optional narrow outbound adapter is an Avesra design choice, not a claim that Jev runs locally.
- [Google Home APIs](https://developers.home.google.com/apis) describe mobile app integrations; they are not evidence of an arbitrary Windows desktop API for the user's unknown light hardware. The specific lights integration is explicitly deferred.

## 18. Setup dependencies, STOP conditions, and maintenance

### Facts still requiring setup verification, not another product brainstorm

- Spark2 reachability and hardware are verified; remaining preflight includes actual model runtime compatibility, model readiness, resource contention and reboot/lifecycle ownership.
- Exact Windows app identities, project paths, selected browser profiles/accounts and automation availability.
- VPN edition/version, MFA and corporate routing behavior.
- Microphone/speaker quality, owner enrollment quality and desired generated voice sample.
- Model/runtime compatibility, measured concurrency budget and model licenses.
- Optional Jev credential and whether the user enables that adapter.

These do not justify guessing success. M0 resolves them or blocks only the affected integration. Lack of a Jev key or deferred lights does not block local voice/PC milestones. Lack of a reachable Spark does block live single-Spark acceptance.

### STOP and report if

- Current source conflicts with the completion plan or another actor owns overlapping files; reconcile the affected slice before editing, preserving independent progress.
- Only an unsupported/private app interface or UAC/security bypass would make an action work.
- The implementation needs to retain raw media, send additional private data to Jev, call another cloud AI API, or use the PC GPU outside an enabled accelerated profile.
- A learned routine would expand permissions, persist credentials, or replay an uncertain consequential effect.
- Owner identity/intent thresholds fail the defined positive/negative corpus, or live interaction targets fail after reasonable candidate/tuning attempts.
- A model change makes voice/speaker presets incompatible and no valid re-enrollment path exists.
- Corporate VPN policy prevents the required Spark connectivity and the proposed remedy would change that policy.
- Required verification is unavailable or fails after two focused remediation attempts; report the exact failure and evidence before improvising a new architecture or lowering the gate.

### Maintenance obligations

Version wire contracts, adapters, model artifacts, speaker profiles, routine schemas and SQLite migrations deliberately. Compatibility changes must specify migration/re-enrollment/revalidation behavior and receive applicable static/build and direct validation. Keep operational logs bounded and redacted; keep useful user memory durable. Repeat affected direct app scenarios after browser, VPN, or coding-app UI changes. Repeat affected voice/latency qualification after changing speech/identity models or microphones.

Keep the task ledger and permission checks outside the models. Keep learned knowledge inspectable and reversible. Keep notification explanations tied to real events. New machines and drivers must reuse existing contracts rather than adding a second action path that bypasses them.

## 19. Done criteria and handoff

- [ ] **OW1:** The owner can ask Avesra to open Claude for IRIS, start a new chat, and type exactly `123`; the correct draft is visible and unsubmitted.
- [ ] **OW2:** The owner can ask about the latest 10 emails and package delivery; Avesra inspects the actual messages and gives a source-grounded answer.
- [ ] **OW3:** The owner can ask to open X to post about a new app; Avesra gets the intended browser/account ready without publishing.
- [ ] M0 environment and feasibility evidence recorded; unresolved items explicitly scoped.
- [ ] Existing Windows and Spark static/build commands pass for the release revision; no automated tests are required or claimed under the owner override.
- [ ] Single-Spark inference works without 5090 model use or undeclared cloud AI.
- [ ] Optional client acceleration is capability-checked and measured; Gaming releases client inference and preserves safe task continuity.
- [ ] Owner enrollment, continuous listening, directed intent, additional-user grants and local controls pass live gates.
- [ ] Requested app/volume/browser/prompt/diagnostic/VPN workflows have real proof, not only simulated dispatch.
- [ ] Teaching and passive learning persist sourced routines/mappings without gaining authority.
- [ ] Indefinite accepted history, local privacy boundaries, deletion and user isolation pass direct inspection scenarios.
- [ ] Learning/action chimes and follow-up explanations pass direct event-correlation scenarios.
- [ ] Driver/profile replacement, restart, network loss, input-focus loss and unknown-effect recovery pass.
- [ ] Latency/resource targets and eight-hour soak pass with measured evidence.
- [ ] Per-stage metrics, redacted traces, local Performance view, and baseline/candidate accuracy-plus-latency comparison work across supported deployments.
- [ ] Direct release review confirms complete revision-matched A01-A29 and OW1-OW3 evidence, with optional disabled Jev explicitly not applicable; no automated report verifier is required.
- [ ] Operations/setup/recovery instructions reflect the tested configuration.
- [ ] `plans/README.md` status updated with evidence; no unrelated files or services changed.

At handoff report implemented milestones, exact commands/results, actual model/runtime/app versions, live scenarios completed, measured recognition/latency/resource results, and remaining gaps. Distinguish a usable early checkpoint from the completed first release. Do not claim publication, installation, or production readiness from this planning document alone.
