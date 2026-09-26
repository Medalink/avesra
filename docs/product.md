# Avesra product contract, revision 1

The desktop uses Tauri's single-instance plugin as its first startup plugin, before Avesra stores, native workers or tray setup. A second launch with the same application identifier exits and asks the existing settings window to show, restore and focus. It must not open another database writer, microphone worker, shortcut owner or Spark connection. Forwarded arguments do not execute operations. A build explicitly configured with invisible settings (background developer inspection) stays invisible on duplicate launch. This does not retroactively coordinate binaries built before single-instance support; those must be closed once when upgrading.

An exclusive OS-held lock on the app-data desktop-instance.lock file is retained for the process lifetime before opening the settings store. It prevents duplicate native startup even if launches overlap before the plugin's activation window exists. Lock failure prevents initialization; process exit/crash releases the lock without deleting its file. No stale PID deletion or process-name killing is used.

Both overlay and Settings remember their last window position in local app data across close/reopen and normal app restarts. Restore positions without restoring hidden/minimized state or the overlay's temporary expanded size. First launch keeps the existing default placements. Saved positions must leave the window reachable on an available monitor; a removed monitor falls back to an on-screen placement. Startup default centering/top-right placement must not overwrite restored positions. Hiding a window and quitting explicitly save its position; background inspection remains invisible. This UI preference grants no device or task permissions.

Plan 001 is the governing product specification. The owner requires specs and code without automated tests for this execution. This does not waive identity, permission, privacy or live outcome requirements.

The first owner is created only through authenticated local setup. Until pairing, speech capabilities and owner enrollment are ready, the assistant cannot listen or admit voice tasks. Unknown speakers, overlaps, weak evidence and ambiguous directed intent abstain. Text entry through the local UI does not bypass enrollment or action policy.

Microphone mute, deafen, pause, lock and quit are locally authoritative. Deafen does not clear explicit mute on restore. Capture and action epochs invalidate stale data; reconnect never replays effects. Close hides the overlay; Quit terminates it. A failed persistence write is surfaced.

Every effect requires an accepted task, current actor grant, exact target and observed outcome. Send, publish, delete, spend, permission changes and configuration changes require a fresh exact approval. External content and models cannot grant approvals. Typing a prompt does not imply permission to submit it.

Single Spark is the baseline. Gaming prohibits client inference. Accelerated remains unavailable until measured capability and transition requirements are met. No cloud inference fallback exists. Service absence remains visible and prevents dependent work.

The current Single Spark/Gaming toggle preserves the existing Spark conversation
and tasks because it changes no inference placement. Gaming defers optional
passive app learning and retires its actual observer; explicit teaching and
Personal voice learning remain available. An observer still finishing is not
reported as retired. Device changes retain their existing invalidation behavior.
Any future transition involving different inference placement must implement the
separate validated replacement and actual job-drain contract before using it.

The approved design is normative: square corners, bundled Geist/Geist Mono, Ruby accents, neutral greys, six settings sections, compact transparent 440×124 overlay and separate 880×640 settings with 200px navigation. Prototype signals and results must never appear as runtime facts.

## Companion setup overview

Settings opens on a Get started overview, accessible again above the existing six settings sections. It connects the setup steps in Plan 001 section 7 to the actual settings pages: Spark pairing, service readiness, audio devices, owner enrollment, generated voice, observation/browser scope and application aliases. Selecting a step navigates to its existing control; it does not perform that operation or grant permissions.

The header's Spark connection status is a keyboard-accessible button. In either connection state, activating it opens Profiles & Machines and scrolls to the Spark pairing controls, using the same navigation behavior as the setup overview. It does not connect or disconnect automatically.

The header is the single live Spark connection indicator. Its disconnected, connecting, connected and error phases and failure detail come from the same revisioned native runtime snapshot used by pairing controls. Command completion does not mean connected: only the authenticated control acknowledgement does. Pairing controls never retain optimistic connection messages. Other actionable operation/storage/audio errors remain visible. Pairing, overview and diagnostics must not duplicate a competing live connection badge. The setup checklist can show completion of its connection step.

On the first verified Windows unlock during each app launch, the native connection coordinator attempts the saved pairing once, using the same credential loader, certificate pinning, bounded handshake and generation checks as Reconnect. No saved pairing is quiet; unreadable credentials or failed connections produce actionable header detail. Explicit disconnect or a newer pairing request supersedes a pending startup attempt. Opening pages never starts another connection. Later Windows unlocks do not undo a manual disconnect. Reconnect grants no voice readiness, restores no setup proof, and replays no task.

If Windows lock interrupts a connected, connecting or already scheduled native
connection, retain only its reconnect intent. On actual unlock, schedule one fresh
saved-pairing attempt under the new native generation. Repeated lock notifications
preserve that intent; a second lock before the scheduled attempt starts must also
preserve it. Duplicate unlock notifications do not create more attempts. An
automatic attempt waits for an earlier pairing operation to actually return,
abandoning its wait when its generation or unlocked context changes. It does not
retry failed connection or inference work. Explicit Disconnect, Forget or Quit
clears pending resume intent. Existing mute, deafen and pause choices remain
unchanged, and normal Personal admission revalidates its current dependencies.
Unlock resume does not repeat the startup greeting or restore any prior task,
reply, microphone lease, management proof or output authority.

Future direct verification (not claimed by source/static review): check a first
launch with saved pairing, then an actual Windows lock/unlock while connected.
Observe any repeated real Windows notifications without manufacturing them;
confirm one resume attempt and no repeated greeting or replayed task. Explicitly
Disconnect before another lock/unlock and confirm it remains disconnected.
Repeat with saved mute, deafen and pause individually, confirming each choice
survives reconnection and still controls listening/output.

The owner-requested startup greeting follows that one saved-connection attempt.
With a selected active voice and permitted speaker output, say `Hello.` or
`Hello, <remembered owner name>.` once. This fixed greeting is a narrow product
output exception to accepted-task speech; it grants no microphone, planner or
action authority. People > Your name stores or forgets the initial owner-scoped
name memory. See [startup greeting](generated-voices.md#startup-greeting) for
identity, timing, cancellation and transport rules.

Disconnected pairing opens with automatic local discovery and a Use this Spark action for an available server. Manual certificate/fingerprint/code entry remains under Advanced. Selection trusts the discovered certificate on first use and reuses protected pairing persistence; discovery alone does not connect or grant permissions. See [Spark discovery](spark-discovery.md) for the bounded scan and one-device pairing window.

The overview derives connection, selected-and-present audio devices, enrollment and voice readiness from the native runtime and current device enumeration. Unknown state is unavailable, never complete. Voice readiness is not browser/action readiness. Browser scope and app alias review have no aggregate completion claim. There is no manually checked completion flag, simulated progress percentage or fabricated task history. A disconnected runtime prominently offers pairing; active local mute/deafen/pause/lock and runtime errors remain visible. Browser rendering is explicitly labeled without substituting a native snapshot.

Audio device refresh enumerates current Windows endpoints without opening a microphone or playback stream. A refresh failure clears the previously enumerated devices and is visible with a retry control. An unavailable saved endpoint remains visible as unavailable, never silently replaced by a default. Setup navigation resets the content scroll position and moves keyboard focus to the destination heading.
