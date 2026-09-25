# Native companion UI checkpoint — 2026-09-25

## Saved enrollment and the missing activation path

The owner explicitly retained automatic owner recognition rather than a push-to-talk substitute. Source tracing found no constructor for QualifiedProfile, no completed signal/overlap/echo/directness qualification, and no configured qualified reasoning deployment. Thus the Registered owner plus six recordings cannot currently activate automatic listening; this is unfinished implementation, not another enrollment step for the owner to repeat.

Fixed a real saved-candidate display race: mount/runtime changes could issue overlapping reads against the protected store's exclusive lock; a busy read could replace the successful read and owner refresh could hide its error. `latest-read.ts` serializes/coalesces reads. Candidate loading/error/empty states are independent, preserve readable results and offer retry. Native candidate decoding is shared by listing, selection and the new check.

Added a read-only saved-voice check through the same phrase-capture helper used by enrollment. An explicit eight-second phrase performs authenticated paired ASR/speaker preflight, native bounded capture, then transient analysis. The Settings page shows checking/recording/processing progress and measured transcript/cosine/held-out/clipping results, without exposing vectors or saving PCM/transcripts. Request/session/capture/action/connection bindings, cancellation and visibility checks remain native. Preview playback prevents a check; cancellation owns capture only and cannot stop an independently started preview. No thresholds or clean overlap/echo evidence are invented. No profile, permission, enrolled or voice-ready value is changed.

Real hidden native Settings inspection through WebView2 CDP confirmed the existing six-segment candidate and Seiren endpoint, reopened People successfully, and rendered the new check: `artifacts/ui/10-saved-voice-check.png`. This is real app/storage/UI evidence, not successful live voice-analysis evidence. The actual owner recording has been requested and is pending. No microphone was opened by this inspection and no Windows authentication or protected profile operation was invoked.

Spark's existing ASR was loaded but not configured in the controller. Installed its private deployment reference, compiled/deployed the new authenticated health route and restarted only the existing Avesra controller. ARM locked Clippy passed and release build took32.81s; the unit is active. Fresh lane metadata still shows ASR0 and speaker6 successful inferences, so no new inference success is claimed. Windows locked desktop/server Clippy, formatting, Svelte (0 errors/0 warnings), production assets and bundled release passed. Automated tests/evaluation harnesses remain excluded by owner instruction.

Canonical release updated2026-09-25 12:24:47 America/Chicago: `E:\Dev\Avesra\target\release\avesra-desktop.exe`,23813632bytes,SHA256 `8F72ACB6F232FDDA36D2CB2A91EC479D7AD16B223525A8DDDD3A47B1672A8159`. Final bundled asset `index-DoozHvdJ.js`; final release build2m43s with existing Cargo caches and normal window configuration. No per-feature executable was delivered. Concurrent atmosphere development continues after this build; its later source changes are not evidence about this exact binary. The owner still needs to perform the explicit voice check; automatic recognition remains incomplete.

Work in the primary checkout on `codex/companion-onboarding`, following source baseline `bd220d1`. This is an incomplete assistant, with actual native UI evidence. No automated tests, fixtures or acceptance harnesses were added or run.

## Implemented

- Get started overview with six setup links, current native connection/mode/readiness and selected-and-present device state. It never marks the full assistant ready.
- Navigation to pairing, services, audio, owner identity, voice design and observation/actions; separate browser setup link. Navigation resets scroll and focuses the destination heading.
- Explicit audio-device refresh through the existing native command. Failure clears stale enumeration and offers retry without substituting devices.
- Base CSS now allows the approved Tailwind typography and spacing to take effect.
- Full Windows audio endpoint friendly names. CPAL 0.17.3 WASAPI's primary description was only `Microphone`; its extended description retains the hardware name. Display-name fallback uses the interface name when available. Stable endpoint IDs, default matching and saved selection are unchanged.

## Direct observations

The native app was initially visually inspected at 880×640. After the owner prohibited Computer Use, inspection switched to a hidden development build and the app's own WebView2 debugging connection on `127.0.0.1:9475`. Both native windows reported `isVisible() == false`. Navigation and native command calls ran through the WebView without desktop input or focus changes.

Real `audio_devices` and rendered microphone option labels after the correction:

- `Microphone (Razer Kiyo Pro) · default`
- `Microphone (Razer Seiren X)`
- Output: `SPDIF Interface (Realtek USB2.0 Audio) · default`

The existing selected input remained the Kiyo Pro endpoint, and explicit microphone mute remained enabled. No microphone or playback stream was opened for this inspection. Refresh completed in the native UI. No simulated runtime, model result or device fixture was used.

All six overview links were directly exercised through the hidden WebView DOM. Destination headings matched the intended settings sections; pairing and voice-design links scrolled to their specific controls, and other destinations began at the top. The hidden instance was stopped after inspection and its debug endpoint became unreachable.

Local screenshots (ignored, not committed): `artifacts/ui/01-get-started.png`, `02-native-audio-devices.png` (before label correction), and `03-correct-device-names.png` (hidden WebView CDP capture after correction).

## Verification

- `pnpm -r check`: PASS, extension TypeScript and Svelte with zero errors/warnings.
- `pnpm -r build`: PASS, extension and desktop assets.
- `cargo fmt --all -- --check`: PASS after formatting the changed Rust source.
- `cargo clippy -p avesra-desktop --locked -- -D warnings`: PASS after the device-name fix. Compiler emitted incremental-cache access-denied notes; the check completed successfully.
- `cargo build -p avesra-desktop --locked`: PASS; corrected native device names confirmed live.
- `cargo build -p avesra-desktop --release --locked --features custom-protocol`: PASS after the device-name fix, 2m27s. `target/release/avesra-desktop.exe` contains bundled frontend assets and the corrected names. It was not launched visibly after the owner prohibited disruptive UI interaction.

## Background development inspection

Use no Computer Use. For development only, copy the configured window entries into a `TAURI_CONFIG` JSON merge override with `visible: false`, `focus: false`, and `skipTaskbar: true`, then compile the debug desktop. Set `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9475 --remote-debugging-address=127.0.0.1` only in the app launch process, and start it hidden. Preserve other running instances/work and avoid duplicate native workers.

Discover the settings target through `http://127.0.0.1:9475/json/list`. Its URL is `http://127.0.0.1:1420/index.html?window=settings`. Use its returned WebSocket endpoint for CDP `Runtime.evaluate` and `Page.captureScreenshot`. In the Vite development WebView, `await import('/src/runtime.ts')` exposes the existing `command` wrapper, e.g. `command('audio_devices')`; use real native results. No production IPC API, app permissions, Tauri configuration file or release debugging listener was changed. The one-shot local console helper is `artifacts/devtools.mjs` (ignored). Stop the inspection instance when finished so its debugging endpoint closes.

## Remaining

### Connection, owner setup and single-instance follow-up

The stale page-wide Spark connection banner came from the generic runtime-error state, which survived a subsequent connected snapshot. Transport now emits a separate revision-bound spark-connection-error; the header owns its detail and reconnect clears it. Removed duplicate connection badges/rows from pairing, overview and health pages. Other operation errors remain visible.

The owner reported Create owner failing or doing nothing. It previously invoked the protected operation without obtaining a missing Windows proof. Create owner and Start voice enrollment now check proof status, request Windows verification when needed, and continue only in the same still-mounted connected/unlocked context. The native one-use proof and owner guards are unchanged. Enrollment explains owner/microphone/mute/deafen/pause prerequisites; owner status refreshes after reconnect/lock changes. The actual Spark speaker service was loaded_unqualified, idle, with zero inferences when inspected. No Windows authentication prompt, owner creation or microphone recording was invoked by the agent; successful owner creation/enrollment remains for the owner to confirm.

Single-instance support uses official tauri-plugin-single-instance2.4.5, pinned and locked, registered before application setup. Repeat launch activates the existing settings window and ignores forwarded operation arguments. An OS-held app-data file lock prevents duplicate worker/store initialization during overlapping startup. Hidden inspection configurations remain invisible on duplicate launch. See [official plugin documentation](https://v2.tauri.app/plugin/single-instance/). Svelte check, frontend build and locked desktop Clippy passed after these source changes.

### Voice draft and automatic preview follow-up

The owner reported losing the description/reviewed candidate on section changes and explicitly requested Generate to play the result with visible progress. VoiceDesigner now stores bounded local draft metadata (description, whether it was deliberately edited, designer visibility and reviewed candidate UUID/revision). Fresh Spark status resolves the candidate; a PC without a draft recovers an available selected/latest saved candidate and its description. No voice is selected or played by restoration. Generate retains the candidate, refreshes status and invokes the existing native preview for that exact generated identity while the originating panel/context remains current. Generating/Playing preview use a visible spinner and accessible status. Blocked output reports device/deafen/pause reasons, and a candidate-local Play preview button allows replay.

Svelte check returned zero errors/warnings and the frontend build passed. Direct hidden native inspection entered the owner's description, navigated away/back, reloaded the WebView, and fully stopped/relaunched the hidden process: the exact description and open designer were preserved every time. Both windows reported invisible; inspection used the separate com.avesra.inspection identifier and its loopback CDP endpoint. Screenshot: artifacts/ui/05-persistent-voice-draft.png. That inspection was disconnected, so it establishes draft persistence only, not live candidate restoration or automatic audio playback. The owner had separately confirmed manual Preview playback. No fixtures, automated tests or desktop input were used. The inspection process was stopped afterward.

### Default Spark discovery follow-up

The disconnected pairing panel now automatically scans and displays actual discovered devices with Use this Spark. The certificate/fingerprint/code form remains under Advanced. Server discovery uses bounded local UDP metadata; selection reuses normal certificate validation and protected credential persistence. See `docs/spark-discovery.md`.

Verification for this source: desktop Svelte check (zero errors/warnings), frontend build, Windows locked Clippy for desktop/server and the bundled native desktop build passed. ARM locked server Clippy and release build passed using the pinned Rust image, two CPUs / 8 GiB, source read-only and shared target writable. Immutable source, binary and build log are under `/home/medalink/.local/share/avesra-build/ui-discovery-20260925/`. A bundled development executable was temporarily supplied under `artifacts/discovery/`; the owner subsequently requested updating only the canonical `target/release/avesra-desktop.exe`. Per-feature executable copies are superseded.

Only the existing Avesra controller unit on Spark2 was updated, preserving its private configuration/certificate and previous binary, with a private SQLite backup before replacement. Health returned protocol2, voice unavailable and actions disabled. The real native `discover_sparks` path found `spark-c8bb`, address `https://192.168.50.11:9474`, pairing open. Header-button navigation and the rendered device card were inspected through hidden WebView2 CDP; both windows reported invisible. Screenshot: `artifacts/ui/04-spark-discovery.png`. Inspection used a separate app identifier, `com.avesra.inspection`, without changing the production configuration. No pairing credential was issued, no microphone/playback stream opened, and no model inference or action performed. Pairing exchange and authenticated control connection remain unproven for this checkpoint.

### Spark status navigation follow-up

The owner expected the header's Spark status to be clickable. It is now a native HTML button with a hover/focus affordance, accessible connection-settings label and small arrow. It uses the existing navigation path to Profiles & Machines / `spark-pairing` in either connection state. Svelte check (zero errors/warnings) and frontend build passed. No new connection command or automatic pairing is invoked. A bundled development executable was temporarily provided under `artifacts/companion/`; this delivery approach is superseded by the owner instruction to update the canonical release only.

Spark pairing/service qualification, owner enrollment, real voice/action ingress and OW1–OW3 remain incomplete. UI/device enumeration is not microphone quality, model inference, voice readiness or end-to-end task proof.

### Canonical release update

Voice-generation follow-up: fixed native HTTP error categorization, cleared stale voice status after refresh failure, and disabled Generate until voice-status retrieval succeeds. Removed duplicate appended error wording while keeping reconciliation guidance and the entered description. Svelte check (zero errors/warnings), frontend build and locked desktop Clippy passed. The canonical bundled release was rebuilt successfully in2m38s. The running image was renamed only inside the build directory to permit replacing the canonical executable without closing the owner's current UI; a hidden cleanup waits for that exact process to exit before removing the replaced image. No additional feature executable is distributed. Backend real-generation evidence and the shared voice-store deployment are in operations.md; desktop rendering/playback of that generated asset still requires the owner's explicit interaction.

At the owner's request, rebuilt the canonical target/release/avesra-desktop.exe with all current UI, microphone-name and discovery changes. Frontend production build and cargo build -p avesra-desktop --release --locked --features custom-protocol passed (optimized native build: 2m48s). No hidden-window or inspection-identifier override was applied. Removed the two temporary executable copies in artifacts/companion and artifacts/discovery. No visible launch was performed. Future deliveries must update the canonical release executable, not create feature-specific app copies.

Voice draft/autoplay delivery: canonical target/release/avesra-desktop.exe rebuilt with normal production window configuration and bundled assets; optimized build passed in2m37s. The owner's running instance was left open and needs a restart to load this update.

Direct single-instance observation: hidden debug instance PID13004 was started, then PID27564 launched from the same executable and identifier. The second exited with code0 within the bounded wait; exactly one matching process remained. Both native windows reported invisible afterward. Read-only setup_status returned Windows authentication available; no prompt was opened. Real People & Voice ID rendering had no stale Spark error banner and showed explicit Create owner / Start voice enrollment guidance. Screenshot: artifacts/ui/06-owner-setup.png. The inspection instance was stopped and CDP closed afterward. This proves duplicate process rejection in the hidden configuration; normal visible-window activation was deliberately not exercised.

Connection/owner/single-instance delivery: canonical target/release/avesra-desktop.exe rebuilt with bundled frontend and normal production configuration; locked optimized build passed in4m37s. The release includes the pinned single-instance plugin and startup file-lock backstop. No inspection instance remains, and no owner authentication or recording was performed by the agent.


### Shared connection state, restart behavior and main UI merge — 2026-09-25

Fetched origin/main and fast-forwarded the companion branch from bd220d1 to 17a10ea (PR #4, Interface size). Preserved and reapplied all local tracked/untracked work without conflicts; the named pre-merge stash remains a recovery copy. Interface size still applies to both native WebViews and overlay geometry.

The native revisioned runtime now carries the connection phase and error. App no longer maintains a competing connection-error event store. Header and pairing controls use the shared sparkConnection projection. The old command-local “Connecting securely” strings are removed. Startup and Reconnect use connect_saved/start_connection, with certificate validation, credential loading, generation cancellation and existing bounded handshake unchanged. Initial verified OS unlock triggers one saved-pairing attempt; later unlocks do not reconnect. All terminal local controls invalidate connection generation. Pairing completion still waits for authenticated acknowledgement before connected state.

Position persistence uses pinned tauri-plugin-window-state2.4.1 with POSITION only; startup defaults run before explicit restore, followed by work-area clamping. Visibility, maximization and the overlay's expanded size are not restored. Native hide, close and tray quit save positions. Production window capabilities were not broadened.

Direct native observations (not tests or fixtures):
- A separate invisible com.avesra.inspection build moved Settings to physical (730,240), overlay to (900,150), then invoked the actual hide_window command. The native state file stored both positions. After process restart, both positions matched exactly and both windows remained invisible. Only the process-local inspection config permitted set-position IPC.
- With no production Avesra process running, a hidden build using the real application identifier loaded the existing saved pairing. No connect command was issued: runtime reported connected=true, connection_phase=connected, error=null, locked=false. Both windows remained invisible, enrollment_capture=false and voice_ready=false.
- Manual disconnect/connect through the real commands produced disconnected then connecting snapshots and subsequently connected. A subsequent actual Disconnect/Reconnect button flow also ended connected with an empty bottom-status list. The header agreed with the runtime and the pairing page contained no stale Connecting message. Screenshot: artifacts/ui/07-shared-spark-connection.png, including the merged Interface size control at125percent.
- All inspection processes were stopped afterward. No credential was copied, new pairing issued, Hello prompt opened, microphone captured or audio played. This establishes real startup control connectivity, not voice readiness.

Enrollment remains unresolved: the owner reports “Enrollment session expired” immediately after Record. Source tracing shows this message was also emitted when native capture failure invalidated the session. Both direct-rate and resampled capture now preserve bounded failure categories, and enrollment distinguishes microphone failure, cancellation and actual five-minute expiry. Existing timing/gating checks remain unchanged. The next owner-triggered recording is needed to identify the actual device/clock failure; no successful collection is claimed.

Scope verification inventory (owner prohibits automated tests):

| Entry point | Verification |
| --- | --- |
| Initial WTS unlock, connect_spark | Same connect_saved coordinator; real saved-pairing startup and manual reconnect observed |
| pair_spark, pair_discovered_spark | Existing start_connection coordinator; source review, no new credential issued |
| disconnect_spark, local_control terminal controls, Windows lock | Generation cancellation reviewed; manual disconnect observed |
| App runtime listener, Settings header, Pairing | Same revisioned runtime and projection; real native screenshot |
| Startup placement, hide_window, native close, tray quit | Shared window_positions functions; hide/restart observed; close/quit source reviewed |
| record_enrollment, native/resampled capture, media failure publisher | Shared bounded failure reasons; source review only, owner recording still needed |

Svelte check (zero errors/warnings), frontend build, cargo fmt and locked desktop Clippy passed. Cargo continues to reuse its target cache; existing debug incremental finalization Access denied notices remain a cache limitation, not a failed check. No tests/harnesses were added or run.

Final delivery: canonical target/release/avesra-desktop.exe rebuilt with bundled final frontend and normal production configuration (no TAURI_CONFIG or CDP override). Final locked optimized rebuild passed in2m18s. SHA-256: CC988B3A4AF6BFDA08E91A1FB3162C1FA6D03BD7D11B1C3EE402744BD707644D. Native inspection processes are closed; no feature executable copies were created. Enrollment remains pending the next owner recording and its specific failure message.


### Capture clock tolerance correction — 2026-09-25

The owner's next explicit Record attempt returned the newly preserved native category: microphone audio timing was discontinuous. The shared capture check was comparing successive WASAPI QPC correlations with only one source sample plus1us of tolerance (about21.8us at48kHz). This treated clock correlation as sample-exact continuity. CPAL0.17.3's WASAPI adapter returns the QPC capture time, ignores native buffer flags and requests no device frame position; consequently that comparison cannot prove exact hardware frame loss. Microsoft documents the separate QPC and device-position values in IAudioCaptureClient::GetBuffer.

The enrollment spec now defines bounded clock tolerance. Both native16k and resampled capture use CaptureTiming: advancing nonempty timestamps;2ms adjacent prediction budget plus sample/rounding allowance;4ms total anchored sample-count drift budget plus the same allowance. The original anchor is retained throughout the attempt so repeated small gaps cannot accumulate without limit. No audio is inserted/dropped. Queue sequence loss, callback failures,500ms packet-age limits, capture deadlines and epoch cancellation remain unchanged. Clock tolerance is not proof of audio quality or sample-exact continuity.

Entry-point review: Capture::open and Capture::open_with_gate share the same native/configuration path; capture_stream and capture_converted_stream now share CaptureTiming and reset it on epoch changes. The native media worker used by enrollment and future normal speech is unchanged. Playback does not call this timing policy and is unaffected. No tests or harnesses were added/run under the owner's override; no microphone was opened by the agent. The next owner recording is the live acceptance boundary.

Verification: standalone rustfmt of the shared capture source and locked desktop Clippy passed against an isolated source snapshot at artifacts/mic-timing-build-20260925. Another task was concurrently introducing FunDSP/sound modules into the primary checkout; its incomplete module/dependency references were excluded only from that snapshot. The primary checkout and its sound work were left intact. The snapshot retains the prior shipped UI/native fixes and changes the microphone timing policy. It builds directly into the canonical target/release path using the existing dependency cache; it does not distribute another application executable. No new UI is part of this correction.

### Authorized physical microphone diagnosis — 2026-09-25

The owner subsequently explicitly authorized microphone access and volunteered to speak. This supersedes the earlier no-capture boundary for this diagnosis only. Short native diagnostics opened the actual selected endpoint, using CPAL and the shared Capture source. Audio existed only in bounded memory; no PCM was retained or uploaded, no identity was enrolled and no Windows Hello prompt was triggered. Only callback metadata and aggregate numeric levels were emitted.

The Kiyo Pro's initial callback had an invalid capture/callback clock correlation (negative callback-minus-capture time of roughly seven billion microseconds). At16kHz its initial packet contained129 frames, followed by160-frame packets. Subsequent adjacent timestamp prediction jitter reached about0.8ms. Relaxing the old sample-exact check alone therefore exposed a second startup failure: partial output frames retained the invalid initial timestamp. Both capture implementations now share a250ms bounded startup admission policy, discarding only invalid pre-roll before buffering/resampling. Invalid clocks after admission still fail immediately; no gaps inside admitted recordings are filled or silently skipped.

Direct observations after both corrections:

| Native observation | Result |
| --- | --- |
| Kiyo Pro, quiet pass | 8.038s,400 frames,128000 samples,zero sequence gaps,no failure; max frame RMS0.0004,peak0.0010 |
| Kiyo Pro, after owner's ready response | 8.038s,400 frames,128000 samples,zero sequence gaps,no failure; max frame RMS0.0012,peak0.0035 |
| Windows endpoint metadata, read-only | Kiyo Pro default at59.7percent,unmuted; Seiren X at86.0percent,unmuted |
| Seiren X, after owner chose it | 8.008s,400 frames,128000 samples,zero sequence gaps,no failure; max frame RMS0.0734,peak0.1685;66 frames above0.01 RMS |
| Live native settings command | Fresh runtime already selected Seiren X; saving that same selection completed and left enrollment_capture=false |

The diagnostic's original Kiyo selection came from an earlier persisted-settings read; the live app's subsequent fresh runtime had Seiren selected. Device IDs, not display names, were used throughout. The Seiren capture proves a continuous native input signal on the selected hardware, not successful speaker inference, completed six-part enrollment, or calibrated recognition quality. Capture paths are shared with enrollment; resampled-path source was reviewed but only the devices' selected native16kHz configuration was exercised live.

Final build integration uses the current primary checkout to retain the separate task's atmosphere changes. A release attempt caught that task midway through adding SoundDiagnostics/component_peaks and failed compilation; no incomplete executable was delivered by that attempt. Primary-checkout locked Clippy passed before those concurrent edits; final integration result is recorded below when complete. No automated tests or fixtures were added/run.

Owner follow-up: all six enrollment segments were accepted, then the app disappeared. Fresh native speaker_candidates returned one readable candidate with segments=6, state=candidate_quality_unqualified and the configured model revision. Its protected candidate file was written at11:31:41 local time. Windows Application events1000/1001/1002 contained no crash/hang report in the inspected interval. The active Design Avesra voice atmosphere task's command history explicitly stopped PID63388 (the shared debug app) and rebuilt/relaunched it as PID57664 at11:32:33. This was an external development restart, not evidence of an enrollment crash. The persisted six-segment candidate survived it; no repeat collection or profile mutation was performed in diagnosis. Selection/qualification and normal listening remain separate pending work.

### Microphone meter and registration UX — 2026-09-25

The owner reported an idle input meter and Register returning to Get started. The meter previously had no standalone capture admission and calculated loudness from averaged visualization samples. Added an explicit30second local Check microphone control using the existing MediaWorker/Capture, native deadline, exact epoch and shared signal event. Full-frame RMS/peak now drive a -60to0dBFS display. Check samples never enter the outbound inference queue; take_capture_frame rejects check mode. Native checks stop on hide/close, local controls, replacement epoch, device/profile change, failure or deadline. The epoch binding is transient and never persisted. Normal voice capture admission stays separate; no readiness or identity bypass was added.

Real native observation via the already-running app's Tauri/CDP bridge: clicked Check microphone with Seiren X selected, observed microphone_check=true, capture epoch9, no capture error and a rendered meter at-51dBFS with four illuminated bars. The UI screenshot is artifacts/ui/08-microphone-check.png. Clicked Stop check and read back microphone_check=false with no current input measurement. No synthetic meter data, desktop input, screen capture, retained PCM or new enrollment was used. This live observation used the then-current debug build; final production build status is recorded separately.

Register had no navigation command, but it also lacked the guided verification used for owner creation. Settings always initialized to Get started on remount/reload; the running development UI was receiving edits/restarts from concurrent tasks. Added allowlisted last-section persistence and a shared ensureManagementVerification helper for create/enroll/register/revoke/select/clear/delete. Native one-use proof enforcement remains unchanged. Registration tolerates verification's own capture-epoch transition while invalidating actual owner/action/connection/lock/page changes, displays inline progress/results, and refreshes remote registration on entry. No Hello prompt or protected registration/profile write was initiated by the agent.

The candidate list also used enrollment-operation generation for initial reads, so a startup epoch update could discard the only list response. Candidate reads now have their own ordered generation and refresh on context changes. The actual People page displayed the existing six-segment candidate and the available Register owner with Spark control without errors (artifacts/ui/09-owner-candidate-restored.png). This is persisted collection evidence, not qualified voice recognition or completed registration.

Static verification after these changes: locked desktop Clippy passed; Svelte check reported zero errors/warnings; frontend production bundling passed. No automated tests/fixtures were added or run under the owner's instruction.

Reload observation: after navigating to People & Voice ID and confirming no pending protected operation, reloaded the actual Settings WebView through CDP. It restored People & Voice ID, the six-segment candidate and Register owner with Spark. No protected action was replayed.

Final integrated delivery: normal production configuration with bundled final frontend; locked release build passed in2m55s, reusing the existing target cache. Canonical target/release/avesra-desktop.exe updated in place. SHA-256: 65EC9C0ABB32E11D50BA3DFAB8830517C9ECBBE9109C68A557D409CC3CE0C3C5. This includes the microphone timing fix, explicit local level check, registration guidance and section/candidate restoration while retaining concurrent atmosphere work. The running development app was not terminated by this task; reopening the canonical release is required to load its final native binary.
