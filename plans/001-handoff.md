# Plan 001 handoff — 2026-09-25

## Resumed companion work — 2026-09-25

**Actual saved-voice root cause resolved:** Codex's inherited MSIX AppData virtualization redirected enrollment files into `C:\Users\medal\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Roaming\com.avesra.desktop`, while the user-launch EXE saw the real normal AppData folder empty. GetFinalPathNameByHandle proved it. **Previous hidden/separate-desktop inspections were not equivalent to Explorer's filesystem view. Package identity15700 and desktop-app process attributes alone also did not prove a normal store.** Launching hidden helper PowerShell via the user's EXISTING Explorer folder window's `Document.Application.ShellExecute` reproduced the empty store. Merely instantiating Shell.Application and invoking its own ShellExecute still inherited redirection here.

Recovered only the original hash-verified DPAPI six-segment candidate and saved selection into the actual same-user normal folder with Avesra closed, create-only and no existing-destination overwrite. Originals retained; no owner/credentials/settings/ledger DBs copied. `artifacts/voice-store-recovery.json` has paths/hashes. Actual native decoding/selection read succeeded afterward, and final fresh Explorer-origin restart still showed all3 steps Complete and a successful explicit recheck. Screenshot17-normal-store-after-restart.png. Normal Seiren selection preserved; no recording, Hello or qualification performed. Native candidate diagnostics now return the canonical physical directory; inspection helper refuses redirected storage before launch. Follow this guard in future work so subsequent saved settings/voice changes are not made in Codex's private overlay again.

Latest canonical release:2026-09-25 14:13:14,23817728bytes,SHA256 `1EBFA0C2BB04E14951179E345636588EED4C30623B9B2DD2F26E01CF6C0512D6`; release/format/diff checks passed, existing cache. Inspection PID17916 stopped. Saved-voice recovery was checked from wrong-state reproduction through restored native read, manual check and full normal-store process restart. Automatic owner recognition is STILL unfinished; do not conflate this completed persistence repair with assistant readiness or request another six-segment collection. User's fresh optional voice check remains deferred.

Latest recheck follow-up: owner confirmed direct canonical EXE launch. Added visible manual Checking/completion timestamps/results and native storage path, response-shape validation and direct selected-record lookup before directory enumeration; unreadable selected records remain errors rather than absence. **Normal release was inspected on an isolated Win32 desktop, from target/release, without switching the user's desktop or input.** Actual registration read and selected six-segment record both returned successfully: all3 steps Complete. Clicked Check saved voice and observed loading followed by all6 phrases intact; navigate away/back retained completion and no first-time recording button. Screenshot14-release-saved-voice-recheck.png. No Hello, profile mutation, recording or Computer Use. Earlier false-empty root cause still unproven; manual silent-result defect fixed. Final canonical exe13:40:06,23817216bytes,SHA256 `F89B89201E5648FAEC48C483BFDA45B0AC9C67D6E260F4ED981F35DCEDCAF622`, bundled `index-4F4QPz8_.js`; Svelte/Rust check/production/release passed. Inspection app/helper stopped. See evidence for details; no need to repeat six recordings and automatic recognition still remains unfinished.

Latest saved-voice follow-up: owner screenshot showed first-time recording despite completed enrollment. Both native debug and packaged release reads still returned the original readable selected six-segment candidate; the reported empty UI was not reproduced, so root cause remains unproven. Added refresh on Settings focus and confirmed Spark registration through the existing coalesced reader, a fresh read before first-time enrollment/Hello to stop unnecessary collection, an explicit Retry for existing unreadable records and visible Check saved voice in the empty state. Only advanced replacement bypasses the existing-record guard. Packaged UI with real native data showed Complete /6 of6 (`artifacts/ui/13-release-saved-voice-restored.png`). No voice/protected mutation or microphone use; original protected file hash unchanged. Final normal canonical release built13:30:58,23813632bytes,SHA256 `82C089BF682BB0B55B1A24F5DA8E8B59D610BA51D99BA5559F7482A46EE59C4C`, bundled `index-D5I2byST.js`; Svelte/production/release checks passed. Hidden inspection stopped. Automatic recognition still unfinished; do not request recollection or the deferred live check.

Latest owner-setup clarity follow-up: People now presents three concrete steps (local owner, Spark registration, six saved phrases), each with independent loading/error/completion. Automatic recognition's missing implementation is stated prominently above them; a saved voice check is optional troubleshooting, not an activation step. Separate People unlock/countdown and default replacement/delete/selection controls were removed from the main flow. Protected actions retain the shared verification coordinator, and EnrollmentView now owns page-disposal cancellation previously supplied by SetupLock. Recording six saves via the existing native finish command automatically; save failure retains/reloads the actual session for retry. Registration now schedules a fresh read after capture/verification context changes and waits for parent operations instead of losing its initial read, with Settings visibility/focus lifetime handling. Native permissions/readiness remain unchanged. Real hidden app UI showed the stored owner and six phrases; registration correctly waits for visible Settings and was not invoked by hidden inspection. The owner's live speech check remains explicitly deferred. See companion-ui.md for screenshot/build evidence.

Latest canonical delivery for this clarity follow-up: 2026-09-25 13:15:30,23813632bytes,SHA256 `C9138A50BB1CE348451A2FD1BC85179CC06AFD0C83881ED808D00849AEDB3835`. Final Svelte/production assets/bundled release passed; `index-CpzrbF5g.js` was confirmed embedded. The hidden inspection app was stopped, and normal release launch remains the user's action.

Automatic owner recognition remains the explicit owner requirement (the owner declined a push-to-talk substitute). The six-segment candidate is already stored; do not ask for recollection. This follow-up found that qualification/segmentation/overlap/echo/directness and the reasoning deployment constructor are still absent, so no combination of current setup buttons can activate the assistant. Do not misdiagnose that as an owner setup failure or set readiness flags to bypass it.

Saved-voice-check canonical release delivery completed12:24:47 America/Chicago,23813632bytes,SHA256 `8F72ACB6F232FDDA36D2CB2A91EC479D7AD16B223525A8DDDD3A47B1672A8159`; Windows static/frontend checks and final bundled release passed. Read the newer evidence below before replacing this binary. The live voice check request is pending; do not claim ASR or automatic recognition proof from the build.

Current implementation adds a real read-only saved-voice check: shared bounded phrase capture, paired ASR+speaker preflight/inference, transient transcript/cosine/held-out/clipping results and recording/processing progress in People. `phrase_capture.rs` is now used by enrollment and this check. Protected candidate loading is shared; frontend refreshes are serialized/coalesced with independent retryable error state. The Spark's existing ASR model was running but absent from controller configuration; a private asr-deployment.json now connects it. The controller adds GET /voice-analysis health and was rebuilt/deployed in its existing path/service. See operations.md and companion-ui.md. The real Settings UI was inspected without Computer Use; no new microphone capture or protected owner operation was performed by the agent. A live spoken check has been requested from the owner and remains pending. Automatic recognition is NOT complete; no qualified profile, listening grant, accepted turn or reasoning activation was fabricated.

Concurrent `Design Avesra voice atmosphere` work is active in this same checkout and app. It stopped our hidden debug PID63956 while updating its playback build, then opened its own visible debug app and began voice preview. Do not stop/restart that process or interfere with playback. Desktop source/release integration must retain its changes and use the shared build cache. Only the canonical target/release/avesra-desktop.exe is delivered.

Latest meter/registration follow-up: Audio & Voice now has an explicit30second local microphone check through the shared MediaWorker/Capture and actual frame RMS/peak. Real Seiren meter movement and Stop were observed through the existing app bridge; no diagnostic PCM was saved or uploaded. A shared frontend verification helper guides all protected People actions, including Register/Revoke, while native one-use proofs stay unchanged. Settings restores the last allowlisted section. Candidate-list refresh no longer loses the stored six-segment candidate to startup epoch churn. Real Settings reload restored People, the candidate and Register control. Registration/Hello itself was left to the owner; no voice readiness was asserted. See companion-ui.md for screenshots and final release delivery.

Microphone follow-up: the owner explicitly authorized physical mic diagnosis and selected Razer Seiren X. The shared native/resampled capture now tolerates bounded QPC jitter and discards invalid startup clock pre-roll for at most250ms before admitting audio. The actual Seiren completed8.008seconds/400frames/128000samples with zero sequence gaps or capture errors and a measurable input signal (max frame RMS0.0734,peak0.1685). The live app confirms Seiren is saved. No diagnostic audio was retained/uploaded or identity enrolled by the agent. The owner then completed all six enrollment segments; fresh native speaker_candidates confirms a readable persisted six-segment candidate, still quality-unqualified. The reported app closure coincided with Design Avesra voice atmosphere explicitly stopping/relaunching the shared debug process; no Windows crash report was found and the saved candidate survived. Do not ask the owner to repeat collection. Normal listening/quality qualification remain unproven. Canonical release integration must preserve concurrent atmosphere work; final delivery status is recorded in companion-ui evidence.

Latest checkpoint: merged origin/main at17a10ea (Interface size) with all companion changes preserved. Native connection phase/error now drive the shared header/pairing projection; real saved-pairing startup reconnect was observed through an invisible native build. Settings/overlay positions survived a hidden process restart. Enrollment immediately fails after Record for the owner; misleading expiry reporting now preserves actual media failure categories, but the root device/clock failure still needs the next owner-triggered attempt. No recording or Hello prompt was performed by the agent. See companion-ui.md for direct observations and scope boundaries.

Current follow-up: Spark status belongs only in the top bar; revision-bound connection errors replace stale shared page banners. The owner clarified that Create owner failed/did nothing, so its explicit action now requests Windows verification when missing before invoking native creation; enrollment has the same guided verification and visible prerequisites. Single-instance startup uses the pinned official Tauri plugin plus an OS-held app-data lock before stores/workers. No owner authentication or recording was performed by the agent. The canonical release remains the sole delivered executable.

Voice UX follow-up: the owner confirmed manual Preview worked, then requested draft persistence and automatic preview after Generate. The frontend now persists local description/reviewed-candidate metadata, restores candidates from fresh Spark status, and automatically invokes the existing native preview after generation with visible progress. Direct hidden-app navigation/reload/full-process-restart proved the description survives; new autoplay/live candidate restoration still need owner observation. The canonical release is rebuilt for delivery; never distribute feature-specific executables.

Latest follow-up: owner confirmed pairing/connection, then Generate failed because neither TTS nor VoiceDesign was wired into the controller. Both dedicated audio services are now deployed/loaded, share a private generated-voice store, and are configured in the controller. A real request with the owner's description generated a4.32second candidate in5.07seconds; TTS status lists it as available and unselected. No audio was played. Desktop status gating/error handling was corrected and the canonical release rebuilt. See operations.md and companion-ui.md for deployment identity, validation and pending playback/general-readiness boundaries. Automatic service loading/reboot recovery remains future installer work.

The owner subsequently requested resuming implementation with real UI and screenshot evidence. Work is now on `codex/companion-onboarding` in the primary checkout. The historical stop below is superseded by that request.

**Latest owner instruction: do not use Computer Use.** Control/inspect Avesra through its Tauri bridge or another fast, non-disruptive background mechanism. Do not focus windows, inject desktop mouse/keyboard input or use native screen capture. A hidden development build can expose its own WebView2 CDP on loopback for DOM inspection, real Tauri command invocation and rendered-page screenshots. This is developer inspection, not a product action path or proof of assistant task execution. No automated tests/harnesses, microphone capture, playback, account interaction or model-service changes are authorized by this UI checkpoint.

The companion now has a Get started overview linking to existing native setup controls, audio-device refresh, and corrected CSS precedence for the approved typography. The owner reported indistinguishable microphone entries; Windows enumeration returned both endpoints but the UI displayed only their generic description. The correction uses the full Windows friendly name while retaining exact endpoint IDs and existing selections. Verification details belong in `docs/evidence/companion-ui.md`.

## Stop state and publication

Latest resumed checkpoint: the header Spark status navigates to pairing. Default pairing now discovers local servers and offers Use this Spark, with manual certificate/code entry under Advanced; see `docs/spark-discovery.md`. Windows and ARM Clippy/builds passed. The actual hidden native companion discovered `spark-c8bb` at `192.168.50.11`, with its pairing window open; no credential was issued during inspection. The owner requires one canonical desktop executable: rebuild `target/release/avesra-desktop.exe` with bundled assets for each delivery. Do not distribute separate per-feature executable copies. Spark2's existing Avesra enrollment controller unit now runs `/home/medalink/.local/share/avesra-build/ui-discovery-20260925/avesra-server`, retaining its existing private configuration/certificate. This supersedes the historical control-v1 deployment note below. Other model services were untouched. See the native UI evidence and operations record for exact proof boundaries.

The owner stopped implementation and requested this handoff plus publication of all existing work. **Do not resume implementation without a new request.** The executor was interrupted. No source changes are pending as of this handoff.

All implementation through `24c8ff3` and its verification record `b8be270` has been fast-forwarded and pushed to `origin/main` at https://github.com/Medalink/avesra. This handoff is a subsequent documentation commit. Plan 001 remains **IN PROGRESS / incomplete**, not approved as a working assistant. None of the three mandatory owner workflows has been demonstrated.

- Primary checkout: `E:\Dev\Avesra`, branch `main`.
- Retained implementation worktree: `C:\Users\medal\.codex\worktrees\avesra-plan-001\Avesra`, branch `codex/avesra-plan-001`. It was clean at `b8be270`; it will not automatically include this later main documentation commit.
- No PR, deployment, new service activation or model qualification accompanied publication.
- Last Windows release and ARM verification processes completed. No reviewer compilation remains pending. The shared ARM build cache was released.

## Owner constraints

1. **Specs and code only; no tests, fixtures, acceptance/evaluation harnesses or test commands.** Formatting checks, Clippy, TypeScript/Svelte checks, builds and source review were used instead.
2. **No Computer Use until the owner explicitly says gaming is finished.** This restriction has not been lifted. Do not launch/focus apps, automate browsers, capture the screen, use UIA, or open microphone/speaker streams. “Continue,” “status report,” and requests to commit do not lift it.
3. Reference and match the approved designs for UI work: `design/README.md`, `design/tailwind/input.css`, and `design/mockups/*.dc.html`. No UI changed in the latest browser-finalization checkpoint. Rendered parity remains unverified.
4. Preserve unrelated work, especially existing Local Studio/model services on Spark2. Runtime setup, credential/owner/catalog/ledger invocation, page reads, inference and device interaction were not performed during the latest checkpoints.
5. Do not turn passing compilation into a live-readiness claim. Keep incomplete milestones and missing owner evidence explicit.

The original `/improve execute 001` used a separate executor and independent review. The reviewer maintained `plans/README.md` and `plans/001-execution-review.md`; the executor implemented source. Later explicit owner instructions authorized committing and pushing the accumulated work to main.

## Read first

1. `plans/001-single-spark-assistant.md` — full intended product and completion contract.
2. `plans/README.md` — milestone index and owner overrides.
3. `plans/001-execution-review.md` — detailed reviewed checkpoints, corrections and actual verification evidence.
4. `docs/browser-read-execution.md` — immediate unfinished integration contract.
5. `docs/browser-read-channel.md` and `docs/browser-reading.md` — concrete ownership and current dormant boundaries.
6. `docs/protocol.md`, `docs/native-planner.md`, `docs/reasoning-adapter.md`, `docs/normal-speech.md` — other important dormant dependencies.

## Product status

The repository now contains a Rust workspace, Windows/Tauri/Svelte companion, Chrome-compatible extension/native host, paired Spark controller, audio-service sources, policy/ledger ownership and detailed specs. It is **not yet usable end to end**.

- M0/M1: setup/build foundations exist; qualification and complete contracts/telemetry remain partial.
- M2: pairing, policy, immutable task/action history, cancellation and native-worker ownership exist. Actual PC pairing, qualified acceptance and live recovery proof remain missing.
- M3: substantial ASR/speaker/TTS transport and native audio code exists. Qualified speaker/voice producer, segmentation/overlap/echo/directness proof, long-response TTS and live audio remain incomplete. Normal short-response speech is bounded to 512 UTF-8 bytes; longer responses remain unsupported rather than clipped.
- M4: design-referenced UI and native app/volume/setup primitives exist. Onboarding, usable action ingress and actual-device proof remain incomplete.
- M5: browser pairing, scope management, document metadata, recovery and dormant accepted-read foundations exist. Active semantic reads and desktop/CLI prompt drivers remain incomplete.
- M6–M9: memory/routines, diagnostics/VPN, deployment/packaging and final live qualification remain unfinished.

Mandatory unproven owner workflows:

1. Open Claude for the IRIS project, create a new chat, type exactly `123`, leave unsubmitted.
2. Check the intended mailbox’s latest 10 emails for delivery confirmation with actual supporting evidence. Generic partial page excerpts do not establish latest-N/account completeness.
3. Open X in Chrome/Brave with the intended account ready, without posting.

## Latest implemented browser checkpoint

Source commit: **`24c8ff3` — Add dormant authenticated browser read finalization**.

Key files:

- `crates/avesra-core/src/browser_execution.rs`: original borrowed Store/ReadExecution, full authorized Request retention, one attempted finalization, publication phases and resource retirement.
- `crates/avesra-core/src/browser_reading.rs`, `execution.rs`, `ledger.rs`: exact correlation and bounded BrowserRead observations, specialized finalization transaction.
- `crates/avesra-core/src/observation_schema.rs`, `store.rs`: schema10 and exact required observation/finalization layouts.
- `crates/avesra-windows/src/browser_receive.rs`: actual authenticated ReceiveOwner, opaque Content and borrowed one-use ReadReply.
- `crates/avesra-windows/src/browser_read_channel.rs`, `effects.rs`: sole-worker preparation channel, exact resource reservation, separate recovery mailbox and cancellation.
- `apps/desktop/src-tauri/src/browser/reading.rs`, `documents.rs`, `browser.rs`: dormant runtime preparation/current checks, immutable native target registry and pipe lifecycle.
- `apps/browser-extension/src/page-excerpt.ts`, `browser-job.ts`: dormant fixed extractor/guard and shared actual Chrome-job owner.

Implemented boundaries:

- Browser protocol/authentication is v6. Status `read` remains null; ReadPage dispatch remains unsupported. No preparation offers are emitted. The extractor is not imported/registered for active injection, and scripting permission has not been activated.
- Native document targets come only from actual authenticated exact revalidation, are immutable, bounded to 16 entries and never rebound. Metadata is not action authority.
- Preparation retains the actual blocking job through caller loss, checks the original deadline/current owner/registration/session/scope/pairing and reserves the exact resource generation. Missing saved state is not initialized by these readers.
- Actual authenticated settlement can retire the exact durable marker and produce a read acknowledgment. Historical duplicates use bounded indexed receipt lookup. A settlement receipt proves resource retirement, not content success.
- Core success finalization requires settled resources plus valid current Excerpt/Empty for the full originally authorized Request. Other possibly-published outcomes remain UnknownEffect. No retry/timeout/reset may invent settlement.
- The specialized transaction obtains IMMEDIATE write ownership, validates exact dispatch/source/grant, repeats policy with fresh time, and checks original cancellation/deadline/clock adjacent to commit. A cancellation racing a committed transaction withholds transient delivery without rewriting immutable history.
- Schema10 stores at most 4096 encoded bytes of provenance, document identity, partial coverage, counts and SHA-256 digests. Raw page blocks/title/URL are not stored in the observation. Generic observation finalization rejects BrowserRead, and application last-success remains application-only.
- ReceiveOwner alone mints Content from the actual authenticated current connection. Same-pairing old-session content is discarded without closing the current transport; wrong actor/app/pairing is an authentication error.
- Content finalization yields a borrowed ReadReply retaining the live execution and native checker. Failed/drop paths withdraw promptly. Consumption yields bounded untrusted data, not transferable effect authority. No escaping successor/publication handle exists.

## Immediate next work, only after owner resumes

Review the existing integration spec before implementation. The next concrete native endpoint spec was discussed but **was not written or committed** before the stop; do not assume it exists.

Proposed direction: extend the existing Offer/Prepared handshake with private paired publication/content endpoints tied to the original Shared/Withdrawal. The actual worker retains its endpoint; the runtime retains the matching native endpoint. No public Context-built sender or raw Reply submission API.

The coherent integration still needs:

1. One-use publication handoff and separate held/possibly-published/terminal state, stopping status.read before settlement acknowledgment without prematurely cancelling valid already-sent content.
2. The live loop on the original borrowed worker/Store, including exact AppCatalog validation, original deadlines and bounded servicing of content and settlement. Never enqueue required work behind the worker waiting for it, and never cancel ReceiveOwner.receive mid-frame to poll a channel.
3. Continuous exact task/actor/registration/grant/scope/action withdrawal through result consumption. Releasing WorkerPreparation advances the resource generation and makes the existing native checker stale. Either consume while the reservation remains held, or implement a real registered successor checker. Disarming Drop or adding a completed boolean is insufficient. Current manage() cancellation of State.active must also cover any future successor.
4. Extension actual-job ownership through every real Chrome promise and exact guard cleanup, plus one metadata-only settlement outbox retained until exact native acknowledgment. No Promise.race-based release, reinjection, reconnect text replay or fresh-worker reset of uncertainty.
5. Fix extractor cleanup evidence before activation: current `started:false` may mean a pre-existing same-request guard/tombstone. It does not prove no owned guard exists. Missing/rejected/lost cleanup remains uncertain.
6. Connect ReadPage dispatch and scripting/extractor activation only after the complete native loop and extension outbox are coherent. A qualified accepted producer is a separate still-missing dependency; setup/UI must not fabricate an Action or read Request.

Avoid spending another large run re-proving unchanged dormant foundations. Use the recorded checkpoints, identify the smallest coherent remaining integration slice, and keep a clear boundary between source completion and live proof.

## Latest verification evidence

All results below are for source `24c8ff3`; only documentation was committed afterward.

| Check | Result |
| --- | --- |
| Independent Windows full Static | PASS; locked Clippy 7.10s, formatting, extension TypeScript, Svelte 0 errors / 0 warnings |
| Independent Windows full bundled release Build | PASS; 2m59s, desktop custom-protocol enabled |
| Executor immutable ARM Static | PASS; 4.65s |
| Executor immutable ARM server release | PASS; 31.71s |
| Independent ARM Static rerun | PASS; 1.23s, exit 0 |

Windows commands, run in the implementation worktree with reduced priority:

```powershell
$env:CARGO_BUILD_JOBS='1'
$env:CARGO_HOME='E:\Dev\cargo-home'
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass='BelowNormal'
pwsh -NoProfile -File scripts/verify.ps1 -Suite Static
pwsh -NoProfile -File scripts/verify.ps1 -Suite Build
```

ARM immutable source/log: `/home/medalink/.local/share/avesra-build/24c8ff3/`, `build.log`.
Shared target cache: `/home/medalink/.local/share/avesra-build/d0d2a60/target`.
Pinned compiler image: `rust@sha256:8fa55b2f3ddf97471ab6a767bfa3f37e6bad0986ba823e75fea57e2a2a5c3073`.
Use source read-only, target writable, two CPUs / 8 GiB, no GPU, and coordinate cache ownership. The ARM script covers core/server only, not desktop/audio/live behavior.

No tests or runtime schema/ledger/browser/device/UI/inference probes were run for these checkpoints. Do not rerun builds just for this documentation handoff.

## Spark environment caution

The last recorded Spark2 environment is historical session evidence, not refreshed by this handoff: `medalink@192.168.50.11`, Ubuntu ARM64/GB10; model cache `/home/medalink/.cache/huggingface/avesra`.

The recorded running controller was the older `50dd7e2` control-v1 enrollment preflight service, with voice unavailable/actions disabled. Current source uses control-v2 and was not deployed. Speaker/ASR candidates remained unqualified; TTS had been stopped. Existing Local Studio/Comfy/Qwen work and a dirty Local Studio `active-model.ts` must be preserved. Recheck actual service/git state before any future authorized deployment; do not infer that main publication updated Spark.

See `plans/spark2-model-setup.md`, `docs/evidence/m0-preflight.md`, and the execution review for prior setup evidence. Do not reproduce credentials or replace unrelated services.

## Publication alert

GitHub reported 13 dependency alerts (8 high, 5 moderate) during the handoff push. These were not triaged or fixed in this stopped session. Review the repository Dependabot alerts before claiming release readiness; this notice is not an independent vulnerability assessment.
