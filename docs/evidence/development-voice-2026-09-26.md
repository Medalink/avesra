# Development voice delivery — 2026-09-26

The owner explicitly prioritized a usable development build over collecting
hundreds of release-validation recordings before trying features. Development
activation now uses the existing six saved enrollment segments, one genuine
eight-second live owner/activity/ASR check, a marked speech interval, and explicit
development consent. It retains normal action permissions. It neither fabricates
release measurements nor marks Plan 001 complete.

## Frozen source and Windows installation

The coherent source archive is
`artifacts/integrated-workflows-proof-20260925/source-development.tar`, SHA256
`708ef029560dfd39f8be2d55e4f5f8b7ce00c6bebe314409daf62f4e24762924`.
Windows Static passed with zero Svelte errors/warnings; the production build
passed in 4m10s. No automated tests or acceptance harness was run.

The installed canonical desktop is
`E:\Dev\Avesra\target\release\avesra-desktop.exe`, SHA256
`56c25a94839fb0252f334d319ffef75f85c64c10fe78c78ebc993200f51734ba`.
Its matching native host SHA256 is
`0cfa6d065985022ef9e50100e2d4299c7dc696d3f4fb7d30ddf12f9b987a9f13`.
Both prior binaries were retained. Five actual physical-profile databases were
backed up with SQLite's backup API before installation; no owner data was reset.

The matching Linux ARM64 static and production builds also passed (production
compilation 54.72s). The retained server SHA256 is
`c9a1df8e7a6e6f14753ba60afb036ae62dcab654657f63bc03827e1b2587e82e`.
It was staged but had not been activated when the owner restarted Spark.

## Runtime faults found during delivery

The reasoning helper's local function named `http` shadowed Python's imported
`http.client` module. Renaming that function fixed actual deployment inspection.
The first subsequent inference encountered cold GPU kernel compilation and did
not complete within the unchanged production deadline. Its failed record was
preserved; the exact owned engine and captured processes were retired before a
fresh controlled load. A separate bounded setup warm-up completed in44.952s;
the unchanged production helper then generated a valid answer in3.327s with a
correlated terminal and actual drain. See [the actual reasoning evidence](reasoning-recovery-2026-09-26.md).

Docker also returned the same container mounts in different orders on fresh
inspection. The audio installer incorrectly treated this as a changed immutable
configuration. Its correction sorts only the full mount records by their unique
destination, rejects duplicates, and preserves every field and other array order.
Fresh inspections of the actual three stopped containers produced stable hashes.
Old owner records are retained and explicitly replaced only after confirmed stop
and absence of outstanding operation journals. All four actual audio services
(ASR, speaker identity, TTS and streaming activity) then loaded successfully,
idle with zero inferences and zero supervisor restarts. This health evidence
predates the owner's subsequent reboot and is not post-reboot readiness.

The installed production Settings UI was opened on a separate non-input desktop
with silent native output and automatic microphone capture disabled. Its actual
DOM showed the existing owner, all six saved phrases, and the eight-second
development flow. No microphone or consent control was invoked.

## Publishing and requested restart

The owner requested merging current `main` into the feature branch and opening
a PR. `codex/devvoice-integration` is based on `origin/main` at73bd805; an explicit
fetch/merge reported already up to date. The integration preserves the newer
dependency/security policy, voice-atmosphere defaults and mockups. It also makes
the existing protected revoke command available when saved permission exists
but current revalidation cannot activate it. Integration validation is recorded
separately from the installed708ef source archive above.

The merged integration passed `scripts/verify.ps1 -Suite Static`: Rust formatting,
locked workspace Clippy with warnings denied, Svelte checking with zero errors or
warnings, and extension TypeScript checking. `pnpm -r build`, the final desktop
frontend rebuild and Python syntax inspection also passed. No automated tests,
native release build of this merged snapshot, or live owner acceptance were run
as part of those integration checks.

The owner then intentionally rebooted Spark. SSH loss and a new boot identity
`1872cbac-2469-48ff-bc21-53adfd5e9cb0` were observed. Reconnecting and restoring
only Avesra's services/models is authorized; fresh controlled reasoning capture
is required before using the new boot. Earlier successful health is withdrawn
until post-reboot recovery is verified.

## Evidence boundary

Matching Spark activation and fresh background UI/media proof are still pending
at this checkpoint. The owner has not performed the new live check. No microphone,
owner consent, Windows Hello, action grant or accepted conversation was fabricated.
Use [the background procedure](../background-evidence.md) for real production
WebView screenshots and silent native mixed audio without touching the input
desktop. Build success is not end-to-end conversation proof.
