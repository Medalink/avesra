# Plan 001 release evidence

Status: **IN PROGRESS — release acceptance has not passed.**

This is a manual evidence ledger, not a test runner or a completion certificate.
The governing cases are Plan 001 section 16. A successful build, model health
response, isolated preview, or manual agent action does not pass an owner workflow.
No required case may be omitted to obtain a green result.

## Revision boundary

The resumed implementation starts at `1704d2badf40f5521de372d162d2f420e6bdcb6c`
plus the preserved, uncommitted Plan 001 changes. This moving source tree is not
a frozen release. The latest installed preview checkpoint has its exact source,
Windows/Spark binary identities, direct UI observations, screenshots, audio and
video in [streaming-preview-2026-09-25.md](streaming-preview-2026-09-25.md).
Later implementation requires its own applicable static/build checks and direct
evidence; earlier media must retain its original artifact identity.

The owner requires background work without disturbing the active desktop.
Follow [background-evidence.md](../background-evidence.md) for separate non-input
desktop inspection and silent postmix recording. That procedure does not supply
real owner speech, acoustic playback, browser-account permission, or Windows
verification. None may be synthesized or bypassed for acceptance.

## Acceptance ledger at resumed implementation

`FAIL` below means the required product behavior/evidence is incomplete, not that
a fabricated scenario was executed. `BLOCKED` names a prerequisite of direct
observation. Replace an entry with PASS only after adding a matching dated record
with environment/revisions, sample count, actual measurements and retained
failure outcomes. Source progress continues independently of live blockers.

| Case | Status | Required remaining evidence |
| --- | --- | --- |
| A01 | BLOCKED | Final artifact direct malformed/revoked/stale transport cases; verify no desktop effect. |
| A02 | BLOCKED | Real held-out owner qualification; six saved enrollment phrases alone are insufficient. |
| A03 | BLOCKED | At least 100 real directed owner requests with condition breakdown and >=95% correct acceptance. |
| A04 | BLOCKED | At least 200 adverse-speaker/replay/overlap attempts and 200 owner non-addressing utterances, zero unauthorized effects. |
| A05 | BLOCKED | Real speaker switch, ambiguous follow-up and immediate revocation cases. |
| A06 | BLOCKED | Genuine accepted useful spoken responses; warm end-of-utterance median <=1.5s/p95 <=3s. Preview timings measure a different path. |
| A07 | BLOCKED | Local mute/deafen/stop p95 <=150ms, including stalled/disconnected Spark and late buffers. |
| A08 | BLOCKED | Actual tray/quit/lock/mixed-DPI and changed-target behavior without disturbing current desktop use. |
| A09 | FAIL | Complete planner-to-effect path, then five actual launch/volume cycles and restart/ambiguity evidence. |
| A10 | FAIL | Complete mailbox-specific individual-message enumeration; actual N=3/10 and all required account/content/drift cases. |
| A11 | FAIL | Grounded current X summary in Chrome and Brave, including login/blocked states. |
| A12 | FAIL | Actual app/project/input adapter and draft/readback/submission-once evidence. |
| A13 | FAIL | Explicit supported terminal AI-input path; no desktop-to-shell substitution. |
| A14 | FAIL | Sourced automatic app resolution, demonstration and passive-learning candidates with unchanged permissions. |
| A15 | FAIL | Validated routine after restart/window relocation; structure drift must require reobservation. |
| A16 | FAIL | Accepted history and completed memory deletion/isolation behavior, inspected actual storage. |
| A17 | FAIL | Committed learning/verified-effect notification batches and exact follow-up after newer silent events. |
| A18 | FAIL | Bounded real diagnostic evidence and exact fresh approval/revocation behavior for any change. |
| A19 | FAIL | Configured VPN adapter, actual connected-state/MFA/routing-loss observations. |
| A20 | FAIL | Qualified replacement and real drain/recovery of the existing inference job owner. |
| A21 | BLOCKED | Core operation with assistant cloud destinations blocked. Jev remains disabled and its optional case is not applicable. |
| A22 | BLOCKED | Revision-matched crash/disk-full/lost-ack recovery, preserving unknown effects without replay. |
| A23 | BLOCKED | Eight-hour final-artifact soak after functional prerequisites; no claim from a short hidden preview. |
| A24 | FAIL | OW1 through real recognized owner speech: actual Claude, IRIS, new chat, exact unsubmitted `123`. |
| A25 | FAIL | OW2 through real recognized owner speech: latest ten individual messages and grounded delivery answer. |
| A26 | FAIL | OW3 through real recognized owner speech: intended X browser/profile/account ready, no post. |
| A27 | FAIL | Implemented and qualified Accelerated/Gaming transition with actual benefit, game-load/resource and recovery observations. |
| A28 | FAIL | Full request correlation through inference/effect/playback; current preview/activity observations cover only part of the path. |
| A29 | FAIL | Same-scenario cold/warm/contention baseline/candidate report with accuracy and failure-inclusive gating. |

## Non-disruptive setup observations

Read-only Windows package/registry inspection on 2026-09-25 at 22:09 CDT found
Claude `2.9939.2.0`, package family `Claude_pzs8sxrjxfjjc`, application ID
`Claude`, executable `app\\Claude.exe`; Chrome `154.0.8037.57`; Brave
`154.1.96.59`; Cisco Secure Client AnyConnect VPN `5.1.20.333`; and Tailscale
`1.102.3`. Cisco's installed `vpncli.exe` reports file version `5, 1, 20, 333`.
Windows built-in `Get-VpnConnection` returned no configured entries.

These observations identify installed products only. No application was opened,
no foreground/window content was read, no VPN command was invoked, and no account,
project, connection readiness or supported semantic control surface was inferred.
