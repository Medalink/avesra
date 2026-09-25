# Plan 001 handoff — 2026-09-25

## Stop state and publication

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
