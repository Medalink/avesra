# Operations

2026-09-25 final streaming-preview checkpoint: matching desktop/controller binaries
are installed, respectively SHA256
`20BB7DA12B8DDD7E1CA9B608135D654FF3F1331FCC2FED5F99885F41109FF8D4` and
`7b144ddf84dbd2226b68971b41c0822fc93c33922ccb642a35911dfe5cbc7ec7`.
Actual default/custom silent UI runs completed; first submitted speech was
397.0/407.6ms after removing the measured whole-utterance pre-ready buffer.
See [matching media and checks](evidence/streaming-preview-2026-09-25.md).
No automatic recognition or action readiness is claimed; the previous records
below retain their historical binary/media identity.

2026-09-25 subsequent native performance checkpoint: canonical desktop SHA256
`3B5B69FED9E14CC293BF4B6F3758BAB3970CEE3FBBB197789F7DF6A8FC17F9E4`
completed a real silent preview and rendered its actual stage timings. Both local
schema10 databases were backed up consistently before the observed schema11 startup;
the protected task view read successfully without grants or effects. See
[exact evidence and the measured pre-ready delay](evidence/performance-preview-2026-09-25.md).
The controller was unchanged. This supersedes the canonical desktop identity below,
not the earlier media's recorded provenance or unfinished acceptance boundaries.

2026-09-25 later background proof: the owner requested noninterrupting actual
screenshots/audio/video and a reusable procedure. See
[the runbook](background-evidence.md) and [revision-matched native evidence](evidence/background-preview-2026-09-25.md).
The current proven canonical desktop SHA256 is
`C9A8988A44C92A89EBF2C22F5EB8E52C807858C7EA7E465AF94BF621A2F2F839`;
controller SHA256 is `cf13e5cdbc18a31ac7e509b4559ad45207a983c7be493014c3c88519583db12b`.
Default/custom Settings previews completed in a separate non-input desktop with
native postmix recording and silent hardware buffers. The stale saved SPDIF
selection was corrected through Settings. ASR/speaker/TTS status now comes from
the paired controller; a separate loaded activity service is configured with its
exact revision. No general voice/task readiness is claimed. Later source changes
require new checks; the older records below describe their historical binaries.

2026-09-25 editable voice test: the owner requested a default test phrase that can
be edited and played, then explicitly approved closing Avesra and updating its
existing Spark controller. Audio & Voice now has Test selected voice, Play text
and Use default; candidate reference playback remains separate. See
[generated-voices.md](generated-voices.md#editable-voice-test) for the contract.

Windows workspace formatting/locked Clippy and frontend type checks passed;
Svelte reported zero errors and warnings. The Windows release initially compiled
the new desktop binary but could not replace the canonical executable because the
owner had opened Avesra during the build. The exact canonical process was closed
after approval; the full release build retry passed, exit 0, in 3m20s. Canonical
`E:\Dev\Avesra\target\release\avesra-desktop.exe` is 24,912,384 bytes, SHA256
`BB34EEB3F5BAED9D96933B88C22418A14AD45C3B40F478102EDAB88F147E7DE4`.
The bundled frontend is `index-CDDpRTjH.js`. No automated tests or agent playback ran.

The exact changed-source snapshot is under
`artifacts/editable-voice-test-20260925/source.tar`, SHA256
`83747C7D0841EF00B2E2CCABD5DFA9313E5D13B1424887D5C0135167163324B4`.
On actual ARM64 Spark2, the pinned Rust compiler used for Plan 001 ran
`scripts/verify.sh static` and `scripts/verify.sh build`, both exit 0 (4.43 seconds
checking, 32.93 seconds release compilation). The source and verification log are
under `avesra-build/editable-voice-test-20260925` on Spark2. Independent source
review found no actionable validation, correlation or ownership issue.

After approval, the matching controller replaced only
`ui-discovery-20260925/avesra-server`; the previous binary is retained as
`editable-voice-test-20260925/previous-avesra-server`. Restarted only
`avesra-controller-enrollment-preflight.service`. The active process binary SHA256
is `04e27af004cfeaf94939a4f327cded8495c910e63940ec54cfbc6125fcd9240a`.
Pinned-certificate HTTPS health succeeded and an unauthenticated HTTP/1.1
WebSocket handshake to `/voice-preview` returned 401. Health reports protocol 2; normal voice remains
unavailable and actions disabled, independently of explicit Settings preview.
The certificate hash remained unchanged. No pairing/owner/settings/voice store
was replaced and no model service was restarted. Avesra is left closed for the
owner to launch; audible custom-text playback remains unverified.

2026-09-25 startup greeting: built the canonical Windows release executable with
the launch-only greeting and People > Your name memory field. SHA256:
`d9a1d21508de92e8f89f2898396e0011cbe1688bfda8701fdb580c644567e4ae`.
Windows and Linux locked Clippy, frontend type/build checks, and changed Rust
formatting checks passed. Automated tests remain excluded by Plan 001.

Built the matching ARM controller with pinned Rust image
`c52e4328faff5e497232822421cff808c46f539161783d93c0c6ea5aa6f60fe2`,
locked Clippy and release compilation. Source, build log, previous binary and
deployment script are under `avesra-build/startup-greeting-20260925` on `spark2`.
Atomically replaced the existing `ui-discovery-20260925/avesra-server` binary and
restarted `avesra-controller-enrollment-preflight.service`. The running binary
SHA256 is `5c2004f3cf325af7268ba5757c383ef65ad01cbcc783d947c3e825bd2406210f`.
The controller is active, pinned-certificate HTTPS health succeeds, and an
unauthenticated HTTP/1.1 WebSocket handshake to `/startup-greeting` returns 401.
The TLS certificate is unchanged; pairing and actor stores were not replaced.

The existing generated-voice TTS configuration lacked `tts_streaming`. Verified
the installed qwen-tts package against the adapter's pinned source hashes, saved
the prior config beside the rollout, enabled that flag, restarted only
`avesra-tts-voices`, and loaded its existing Base model and saved voice store.
Private health reports loaded_unqualified, streaming true, busy false and zero
successful inferences. Normal voice/action qualification stays closed. No app
launch, microphone capture, synthesis, or audible greeting was performed during
this rollout; live end-to-end greeting quality remains unverified.

2026-09-25 saved-voice check: installed private mode0600 `asr-deployment.json` into the existing `/home/medalink/.local/share/avesra-controller-ip` directory, selecting `/home/medalink/.local/share/avesra-audio/run/asr.sock` at Nemotron revision `ebe59e5a817142986528bbbee5dba8db7b38ed50`. The already-running ASR container reported loaded_unqualified, streaming false and zero successful inferences before this work; no model was loaded/replaced or unrelated container changed. The controller now exposes authenticated GET /voice-analysis for same-service ASR/speaker preflight; POST remains transient inference without acceptance authority. ARM locked Clippy and release build passed using the existing target cache and pinned compiler image. Source/log/binary are under `avesra-build/voice-check-20260925`; the running binary was atomically replaced at the existing `avesra-build/ui-discovery-20260925/avesra-server` path, SHA256 `d9f4db0ec807aba58b402553b78dd9b931f7c180670aa36a69ffa02e33ec9e68`. Only the existing enrollment controller unit was restarted; it reports active. Pairing, actor records and TLS credentials were preserved. This proves configuration/build/service availability, not successful transcription, speaker qualification or automatic listening.

2026-09-25 generated-voice setup: after the owner paired the companion and attempted Generate, inspection found only `speaker-deployment.json`; TTS was stopped and VoiceDesign was absent. Deployed current audio source over the existing pinned TTS runtime as local immutable image `sha256:6369ea830a12630308535fa059b1fd6c07626fdd995e13ae6e0b5a5fac9ee2ba`. Containers `avesra-tts-voices` and `avesra-voice-design-voices` use UID1000:1000, network none, 2 CPUs / 12 GiB each, GPU access, dropped capabilities and no-new-privileges. Models are existing read-only downloads; both mount the same mode0700 `/home/medalink/.local/share/avesra-audio/voices` store. Config/build evidence is under `avesra-audio/voice-setup-20260925/`; canonical config equivalents are `services/audio/tts-voices.config.json` and `services/audio/voice-design.config.json`.

Both shipped admin loads returned loaded_unqualified. A direct real VoiceDesign request using the owner's submitted description generated candidate `c75576b6-d03b-4ecb-9d5f-2672515f54a1` in5.07seconds:103680samples at24kHz (4.32seconds). TTS voice_status read that same candidate as available, with no voice selected. No playback, selection or microphone recording occurred. Added private `tts-deployment.json` and `voice-design-deployment.json` pointing to `run/tts-voices.sock` and `run/voice-design-voices.sock` with exact model revisions, then restarted only the Avesra controller using normal serve without reopening pairing. TLS health still truthfully reports general voice unavailable/actions disabled. Existing Local Studio, speaker and ASR containers remained running. These services are manually deployed/loaded; automatic cold-start recovery and general synthesis/voice readiness remain unqualified. Rollback can restore the controller's prior configuration by removing these two newly added deployment files, then restarting its existing unit; the previous stopped TTS container/configuration remains preserved.

2026-09-25 local discovery deployment: the existing user unit `avesra-controller-enrollment-preflight.service` now runs `/home/medalink/.local/share/avesra-build/ui-discovery-20260925/avesra-server serve /home/medalink/.local/share/avesra-controller-ip`, with MemoryMax=1G, CPUQuota=200% and NoNewPrivileges=yes. ARM locked Clippy and release build passed from the read-only uploaded source with the pinned Rust image. Before replacement, the database contained zero paired devices; a private mode0600 SQLite backup is `authentication-before-discovery.db` in the configuration directory. The previous50dd7e2 binary remains available. TLS health after replacement reports protocol2, voice unavailable and actions disabled. The actual hidden Windows companion discovered `spark-c8bb` and its open pairing window. No device credential was issued by the inspection, and no model service was changed. The first-install window opened at09:57:52 America/Chicago and lasts five minutes; an existing server can explicitly reopen a five-minute window with `serve <directory> --pairing`. See [discovery contract](spark-discovery.md) and [native UI evidence](evidence/companion-ui.md). This is discovery/deployment proof, not connected or voice/task proof.

After independent inspection, only `avesra-tts-final-preflight` was stopped to release unused model residency; its container/configuration/image remain. The final socket was removed. The old supervisor surfaced its expected SIGTERM cancellation as an uncaught `CancelledError` and exited1 despite cleanup. Source now consumes only the explicit SIGTERM cancellation and ignores duplicate termination signals during cleanup; Python3.12 syntax compilation on Spark passed. The existing immutable image does not contain this source correction, and corrected shutdown has not been exercised.

2026-09-24 final TTS load qualification: `avesra-tts-final-preflight` runs immutable image `sha256:5c3d411b1825c673d8bd48089a4040c5a3ab7401029316d7ea475ae589cfcbfe` as UID1000:1000, network none, 2 CPUs/8 GiB, capabilities dropped and no-new-privileges. Models and `source/tts.config.json` are read-only; the private socket is `/home/medalink/.local/share/avesra-audio/run/tts-final.sock`. The shipped admin load returned `loaded_unqualified`; both admin and compiled50dd7e2 Rust `audio-health` reported exact Base revision5d83992436eae1d760afd27aff78a71d676296fc, streaming false, permission authority false, zero inferences and null last-inference duration. No text/audio/selected voice was supplied and no generation occurred. Existing three services and enrollment controller remained active. This qualifies the final supervisor's model-load path only; usable synthesis and streaming remain unproven.

2026-09-24 enrollment prerequisite deployment: `avesra-controller-enrollment-preflight.service` runs `/home/medalink/.local/share/avesra-build/50dd7e2/avesra-server` with the existing private `avesra-controller-ip` configuration/certificate. Binary SHA256 is `197a11a95d6c740f7babb23551e4cf095a922b9b0d5880060a6bb24d262f819f`. The earlier IP preflight unit was stopped; its binary and configuration remain. A0600 `speaker-deployment.json` selects `/home/medalink/.local/share/avesra-audio/run/speaker-final.sock` and the pinned ECAPA revision. `avesra-speaker-final-preflight` runs final source image65296b932886 with2CPUs/6GiB, UID1000, networknone, cap-dropALL and no-new-privileges. Shipped load and Rust metadata inspection returned loaded_unqualified with zero inferences. Windows TLS health verified the unchanged certificate and still reports voice unavailable/actions disabled. Paired-device count was zero before replacement; no pairing, enrollment or media request was performed. Existing Local Studio, Comfy and Qwen services remained active. This is deployment/load evidence, not voice acceptance.

Avesra is under implementation. See [current M0 evidence](evidence/m0-preflight.md). No installed assistant, owner enrollment or live workflow is claimed.

The only initial server is owner-confirmed SSH alias `spark2`. Preserve Local Studio's authentication and unrelated ComfyUI/Qwen supervisor workloads. Avesra must own only its own controller and narrowly scoped speech services. Never reset shared model metrics.

Secrets belong in owner-protected OS configuration or credential storage, referenced by name. Do not commit them, copy browser cookies, print environment dumps, or include media/biometrics in diagnostics.

Before enabling observation or voice, select actual devices/scopes and complete capability probes and owner enrollment. Missing services remain unavailable. Browser accounts and Claude projects require explicit selection. Authentication/MFA stays with the owner.

The baseline `single-spark` and `gaming` profiles prohibit client model allocation. `accelerated` is opt-in and requires measured device eligibility. A disconnected controller never grants permission to execute delayed actions.

Saved-pairing recovery: Profiles > Manage saved pairing shows the public device UUID, plus a warning that local removal is not server revocation. Revoke through the existing authenticated Spark CLI (`avesra-server revoke <private-directory> <device-uuid>`), then explicitly remove the PC copy. Local removal disconnects and invalidates pending connection work, serializes with pairing/reconnect, and binds removal to the reviewed encrypted file revision. If DPAPI cannot decrypt the record, the UI reports the missing device identity; do not guess a server device UUID. A pairing save failure reports the newly issued public UUID for revocation before retry. The server and PC never automatically replace existing credentials or certificates.
