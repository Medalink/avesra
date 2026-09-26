# Owned installation and recovery

This specification governs Plan 001 C8. It does not change inference,
qualification, device selection, owner identity, or action permissions. Packaging
must preserve those existing stores and their exact validation boundaries.

## Observed gap

On 2026-09-25 the running controller was the transient user unit
`avesra-controller-enrollment-preflight.service`, with `Restart=no`, a 1GiB
memory limit and a two-CPU quota. The owned ASR, speaker, TTS, voice-design,
batch-activity and candidate streaming-activity containers also reported
`RestartPolicy.Name=no`. Successful current health does not establish login,
reboot, model reload or failure recovery.

The Windows release is a canonical portable executable. The dedicated native
host is compiled, but selected-extension registration and installation packaging
are not supplied by `cargo build`. Tauri bundling is disabled at this checkpoint.

## Windows package contract

An immutable version directory contains the matched desktop and native-host
executables, bundled browser extension, a file-hash manifest and runbook. The
manifest records build/source identity, relative paths, byte lengths and SHA256
values. Reject absolute/traversing paths, reparse-point inputs, duplicate paths,
unexpected files and mismatched hashes before installation. Do not execute
arbitrary manifest commands or load code from user profile content.

Registration uses the current Windows user only. The caller selects Chrome or
Brave and the actual installed extension ID (exactly 32 lowercase `a`-`p`
characters). Write the native host manifest for `com.avesra.companion` and its
single exact `chrome-extension://<id>/` allowed origin, and the sibling
`avesra-extension-origin.txt` expected by the actual native-host executable.
The registry record is only transport discovery: it grants no browser/profile,
origin, page-reading or task authority. Never claim that registration pairs the
extension. Browser extension loading, permission gestures and comparison-code
pairing remain their existing explicit product flows.

Before replacing a registration, retain its exact prior value and the previous
version path. Refuse to overwrite an unrelated registration. Do not terminate
the owner's browser or active companion to complete an update. Installation may
stage a new version while the old one is in use; activation must wait for the
owned process to close. Both native executables must remain siblings because the
pipe peer check validates the actual sibling native-host binary.

Rollback switches only to a hash-verified retained package and compatible store
schema. It never restores an old database over new accepted history or protected
identity. Unknown effects and inference jobs remain unknown across update and
rollback. Uninstall removes only matching owned registration/package artifacts;
owner credentials, enrollment, settings and memory are preserved unless a
separate explicit deletion was requested.

## Spark package contract

### Selected audio model startup

The controller snapshots only the configured speaker, ASR, activity and TTS
deployments. Its single background coordinator starts after configuration succeeds;
TLS binding does not wait for models. Voice-design is excluded. No inference,
qualification, deployment replacement or supervisor mutation occurs here.

Configured socket paths must be absolute, normalized and bounded. An absent
socket or its absent private parent is a temporary startup condition, not a fatal
controller configuration error. Present unsafe objects are rejected. Every actual
connection rechecks the exact socket and private parent, same-user ownership,
permissions, non-symlink identity and peer UID. The standalone unconfigured audio
CLI constructor still requires the socket to exist. Nothing creates runtime paths.

One original 60-second availability window bounds read-only startup health waits.
Each later lane still gets one health observation if earlier loading consumed that
window. A loaded lane is skipped even when busy, without consuming a load attempt.
Loading, busy or termination-pending work is never replaced. If it cannot retire
within the availability window, the coordinator stops rather than overlapping a
possibly active load. Missing lanes do not prevent observing other configured lanes.

Model admissions are sequential. Each client may submit at most one load in this
controller incarnation, with one original 120-second deadline established before
worker enqueue and retained through preparation. The spawned worker owns admission
even if its caller disappears. Success must be the exact private loaded terminal followed, within that same
deadline, by exact lane/revision loaded health from the same socket. Selected
activity/TTS must advertise streaming. Once that terminal settles the load, later
health failure reports unavailable without cancelling or unloading it.
On a lost, invalid or expired terminal it retains admission while requesting exact
request cancellation for at most three additional seconds. Those seconds only
settle the original request; they cannot extend its model-loading lifetime.
Confirmed cancellation permits retirement. Otherwise the local client closes its
admission permanently for this incarnation and reports local quarantine separately
in controller diagnostics; its health becomes unavailable without inventing service
health. The coordinator stops on quarantine, and never retries a load or infers
retirement from an unknown request. A new controller observes current service health
again; it does not repair credentials or resurrect a previous operation.

Entry points: `transport::router` owns the startup coordinator; `AudioClient::load`
owns the retained load; generic audio exchanges and the direct TTS stream use the
same checked connector. Deployment installers, pinned supervisors and ordinary
inference deadlines are unchanged. The manual cold-lane observation below verifies
one recovery path; actual full-reboot and conversational evidence remain separate.

### Manual cold-lane recovery proof

This is an operator procedure for an authorized maintenance window, not an
automated check or permission to interrupt normal use. Keep the companion closed
and verify that no inference or stream is active before proceeding.

1. Record the controller source revision, deployed executable hash, UTC or
   explicitly zoned timestamps, boot ID, exact unit names and PID/start times.
   Resolve the selected lane from its existing controller deployment and owned
   installer manifest. Verify the exact full container/image IDs, pinned
   supervisor/config hashes, model revision, socket and enabled persistent unit.
   Do not substitute a candidate or rewrite ownership metadata. Snapshot the
   other selected audio and reasoning process identities for comparison.
2. Stop only that idle, owned lane through its existing unit. Before restarting
   anything, prove actual retirement: unit inactive with MainPID zero, exact
   container not running/restarting with a normal exit, and the supervisor journal
   and durable operation marker consistent with a completed stop. Confirm the
   selected endpoint is absent. A missing PID or killed Docker client alone is
   insufficient. Do not manually erase a pending marker or socket to force success.
3. Restart only the owned controller while that endpoint remains absent. Observe
   pinned-certificate TLS health and retain its time before starting the lane.
   This proves controller availability during the missing-endpoint condition.
4. Start the exact previously verified lane unit within the controller's original
   60-second availability window. Let the controller's startup coordinator perform
   its one load; an administrative `load` call is not a substitute. Retain the
   controller journal's load result and a subsequent private health observation:
   exact lane/revision, required streaming metadata, `loaded_unqualified`, idle,
   and zero successful inferences for this fresh service. Verify that the other
   recorded process identities did not change.
5. Stop the procedure on identity drift, outstanding work, timeout, quarantine,
   unexpected restart or failed health. Preserve the exact logs and ownership
   marker for reconciliation; do not repeat a load or restart to hide uncertainty.
   An acknowledged loaded terminal remains settled even if its later health check
   fails: that failure does not authorize cancelling or unloading it. Reconcile
   actual state before any separately authorized recovery operation.

Observed on 2026-09-26 with controller source `a8b0eb1`: the warm controller
PID 1856368 at 05:48:14 CDT reported all four selected lanes already loaded. After a
verified normal stop of the selected speaker unit, controller PID 1861301 started
at 05:49:05 CDT and served pinned-certificate health while its socket was absent.
The same owned speaker unit started at 05:49:06 CDT with PID 1861695; at 05:49:09 CDT
the controller reported speaker loaded and the other three lanes already loaded.
At05:49:17 CDT speaker health matched its selected revision, was loaded and idle,
and reported zero successful inferences. ASR PID 2227, activity PID 482389, TTS
PID 482390 and reasoning PID 32995 retained their prior start times. This proves
one cold speaker-lane recovery plus loaded-lane skips, not a whole-host reboot,
automatic listening, model qualification or a completed conversation.

Pin controller executable, audio images, model revisions and serving manifests.
Do not use floating image tags as installation identity. A persistent user unit
owns only Avesra's controller. Its configuration path remains the existing
private directory; never initialize new credentials or overwrite deployment
configuration as a side effect of an upgrade. Keep its existing resource limits,
nonprivileged user, `NoNewPrivileges` and bounded restart delay.

Each owned audio service retains its exact image, UID, model mounts, runtime
socket, generated-voice store and resource limits. Startup must distinguish
process/socket availability from model load and model qualification. A bounded
health observation reports unavailable/starting/loading without granting voice
readiness. Readiness timeout is visible; no restart loop recreates enrollment or
retries an uncertain generation. The voice designer may remain demand-loaded;
it must not consume the command model's reserved resources simply because the
machine restarted.

The batch activity service and separately measured streaming candidate are not
interchangeable. An install must preserve the selected exact deployment unless
an explicit, compatible, observed transition activates its replacement. Never
rewrite the controller socket to the candidate merely because it exists.

Do not stop or reconfigure Local Studio, unrelated Qwen jobs, ComfyUI or other
host services during installation. A controlled reasoning launch must use a
separate compatible instance and measured capacity, or remain unavailable. An
already running engine cannot be retroactively labeled as a controlled load.

Before activation, save the prior binary hash, unit definition and config-path
identity. Stage the replacement, verify hashes and permissions, then replace
atomically. Restart only the owned controller/service. Observe pinned-certificate
health within a fixed timeout and retain failure diagnostics without credentials.
On failure, restore the previous verified artifact; never clear durable uncertain
jobs. A healthy process still does not prove qualified inference.

## Packaging entry points and bounds

`scripts/package-windows.ps1` builds the portable release directory from already
built matched executables and the extension distribution; it does not compile or
launch them. `scripts/install-windows.py` stages, activates, rolls back or removes
only owned per-user registration. Python3.12 or newer is an explicit installer
prerequisite; its standard-library SQLite reader checks actual store schema
without executing the companion or migrating data. A package manifest and its
trusted externally supplied SHA256 are required. A hash in the same untrusted
directory is not an authenticity anchor. Installation records are private local
ownership metadata, never credentials or model qualification.

Activation refuses a live companion/native host and refuses unknown registration
ownership. Store compatibility checks read both actual `avesra.db` and
`native-actions.db`; no missing schema may silently become an empty database.
Rollback uses the same verifier and compatibility checks as activation. Uninstall
removes only registrations still matching the owned installation; version
directories and all application data remain retained for explicit later cleanup.
An interrupted activation retains a recovery journal and refuses further mutation
until the exact previous registration is restored by the recovery command.

The package carries its Python installer and common verifier. Stage the unpacked
extension at the stable `%LOCALAPPDATA%\Avesra\packages\browser-extension` path
with `prepare-extension`, then explicitly load/reload that path in the selected
browser and read its actual ID. Version-directory extension paths must not be
used for unpacked loading: without a manifest key their IDs can change. Activation
checks the stable extension against the selected package and creates an owned
Start-menu Avesra shortcut pointing to its exact desktop/native-host siblings.
An old portable/canonical shortcut is not silently redirected. Use the new owned
shortcut after activation. No app is launched by installation.

Chrome and Brave registrations can share a switched package only when their
recorded extension ID is identical; differing IDs are refused. To switch an
integration or update extension files, close the companion normally, unregister
the owned integration, prepare the verified replacement at the same stable path,
explicitly reload it, then stage/activate with the actual selected ID. Re-pair or
renew browser permissions through the existing visible product flow as required.
An interrupted stable-directory swap keeps its old contents and a journal;
`recover-extension` restores that previous directory without recursive deletion.

Example operator commands (hashes are from the trusted build record, never
guessed; these commands are not executed by writing this specification):

```powershell
./scripts/package-windows.ps1 -SourceIdentity <source-archive-sha256> -StoreSchema <13-through-30-from-frozen-build> -Output <new-package-directory>
python ./scripts/install-windows.py prepare-extension --package <package-directory> --sha256 <manifest-sha256>
# Explicitly load/reload the printed stable extension path and inspect its ID.
python ./scripts/install-windows.py stage --package <package-directory> --sha256 <manifest-sha256> --extension-id <actual-id>
python ./scripts/install-windows.py activate --version <printed-version> --browser chrome --data "$env:APPDATA/com.avesra.desktop"
```

For rollback, record the intended retained version before unregistering, prepare
that old trusted package's extension, reload the browser, then
`rollback --version <that-retained-version> --browser chrome --data <actual-app-data>`.
The explicit target is never inferred from a previous pointer modified by
unregistration. Rollback refuses a store schema newer than the package's declared
support. Package creation requires schema13 through30 to match the actual frozen
source's schema marker; the trusted build record must also bind those executables
to that source archive. The installer never guesses schema from a filename and
never restores an old database. `unregister` retains package directories, source
manifests, extensions and protected data. Incomplete staging directories are
retained and reported for explicit inspection, never silently overwritten.

Current source writes schema30 (the preceding source marker was29). Schema30 adds the private ordinary-content FTS5 search table/shadow tables, stable accepted-rowid document metadata and owner/device backfill checkpoint/index revision. It enables SQLite and FTS5 secure-delete and does not rebuild accepted source tables. Backfill is explicit, bounded and resumable; source deletion updates the index in the same transaction. Packages below30 cannot reopen a migrated store. Schema29 adds the content-free conversation tombstone table and exact actor/device/session history index in both stores opened by the shared Store implementation. The migration is additive: accepted-conversation rowids and replay constraints are preserved without rebuilding that table. Once either store has migrated, rollback to a package declaring support below29 is refused; the installer does not downgrade or restore a database. Schema27 adds the passive teaching journal; schema28 adds typed disk-activity diagnostic observations. Schema25 adds the bounded owner demonstration journal; schema26 adds NativeMailbox reply provenance and adds no tables. Schema23 rebuilds private memory source columns for mutually exclusive task/accepted-turn provenance while preserving foreign keys; schema24 adds typed Gmail targets, payloads and bounded mailbox observations without new tables. Schema20 preserves explicit already-satisfied action outcomes; schema21 adds exact X-ready targets; schema22 adds typed model/native-event reply provenance. These compatibility markers prevent old readers from reopening newly written serialized records; they add no tables. Schema17 adds the monotonically increasing native
planner claim counter;18 and19 mark VPN and browser-provider record compatibility.
No database downgrade or counter reset is performed. Native/controller deployments
must match planner-v4 and normal-speech-v6; older wire versions reject.

`scripts/install-spark.py` stages a caller-hash-pinned controller, and installs or
rolls back the owned persistent user unit against an existing private directory.
It never runs `init`, changes deployment JSON, changes a model/container or deletes
state. The service uses bounded on-failure restarts; durable uncertain jobs remain
in the original private directory. Service activation/recovery is explicit and
checks the existing certificate pin through the actual loopback TLS health route.
An existing unrelated/transient listener must be retired separately by its owner;
the installer never searches for or kills it. User services start at login; boot
without login additionally requires an administrator's existing user-linger
configuration, which this installer observes and reports but never modifies.

```sh
python3 scripts/install-spark.py stage --binary <built-avesra-server> --sha256 <binary-sha256> --source <source-archive-sha256>
python3 scripts/install-spark.py activate --sha256 <binary-sha256> --config <existing-private-controller-directory>
# An explicit later rollback uses only the previous retained verified executable.
python3 scripts/install-spark.py rollback
```

`scripts/install-audio.py` adds persistent user-unit ownership of an explicitly
selected existing audio container without recreating it. Preparation requires
its exact64-character container ID, exact immutable image ID, service config,
host socket and model revision; it records a hash of the actual command,
environment, UID, mounts, device/resource/network settings and image. It checks
every mount field but canonicalizes the semantically unordered Docker `Mounts`
array by its unique destination before hashing. Duplicate or malformed mount
destinations are refused; no fields are omitted and no other arrays are reordered.
The nullable Docker `HostConfig.OomKillDisable` field alone canonicalizes null
to false: both mean the OOM killer is not disabled. True remains distinct, and
non-boolean/non-null values are rejected. Actual first-start inspection on
2026-09-26 changed only this field from false to null; Docker's daemon defaults
null to false, then clears it on kernels without that optional capability
([daemon source](https://github.com/moby/moby/blob/v28.3.3/daemon/daemon_unix.go#L351)).
This correction applies to newly prepared owners only. Never rewrite existing
manifest fingerprints or replace their pinned supervisor in place; retain the
original operation evidence and explicitly prepare a reconciled replacement.
Two fresh inspections of the stopped speaker and TTS containers on 2026-09-26
differed only in mount order, confirming the original order-sensitive fingerprint
could incorrectly refuse an unchanged container. Existing prepared owners remain
bound to their original supervisor/manifest hashes and must be explicitly
reconciled and prepared again; the supervisor does not silently migrate them.
It checks
the selected controller deployment file rather than substituting a candidate.
The batch and streaming activity configurations remain distinct. Optional voice
design and unselected activity candidates are demand-started and never enabled
at login by the installer. Required selected lanes may explicitly enable startup.

Each unit runs a retained small supervisor. Before every start it rechecks all
recorded container/config identities and refuses to adopt an already running
container. Its operator must first retire an idle existing instance explicitly.
After startup, a bounded same-UID private-socket health observation verifies exact
lane/revision/streaming shape and reports the actual unqualified model state;
it never warms a model or grants readiness. Shutdown targets only that exact
container. A durable operation marker precedes start and remains until the
original start returned and exact stopped state was observed. Start is bounded
to20seconds and stop/observation to35seconds; killing a Docker client is never
container retirement. Unsettled operations leave the marker and fail closed on
restart, while the user unit has a70second shutdown ceiling so logout cannot
hang indefinitely. A pending start cannot be cleared from a momentary stopped
snapshot: the daemon might still complete that original operation. It requires
explicit operator reconciliation after the daemon's outstanding operation has
been retired. A settled-start marker can be removed only after exact stopped
state is observed. An owned/stop marker with a still-running container refuses
automatic adoption or replay; the operator must retire that exact container and
observe it stopped before restarting the unit. Journal I/O failure cannot skip
the shutdown attempt; the earlier marker remains blocking. No generation is retried.
After a host crash, a stale Unix socket is removed only when the exact configured
container is observed stopped, the exact same-user socket refuses connection,
and its inode/ctime still match immediately before unlink. A live listener,
regular file, changed identity or uncertain probe is never removed.

The existing transient controller must first be retired by its explicit owner;
the installer refuses a first activation while another listener owns9474. It
preserves audio deployment files and container settings. Audio persistence uses
the separate exact-container installer; reboot/model-load proof is still required.
The isolated reasoning-owner setup
is separately scoped in `services/reasoning/setup_owned_controller.py`.

The packaging entry points cover install/update/rollback/unregister and interrupted
registration recovery. Cargo builds, Tauri's disabled MSI bundler, browser extension
loading/pairing, protected owner setup, audio/reasoning lifecycle and calibration
remain separate paths with their existing authority. No package operation creates
an accepted turn, grant, readiness flag or clean inference state. Static parsing
and source review are the permitted checks here; the owner's no-tests instruction
excludes automated fixtures/harnesses. Direct packaged activation and recovery
evidence remains required below.

## Required direct evidence

Record exact package/source hashes and before/after configuration identities.
Observe update, ordinary process recovery and startup using the actual packaged
services. Browser registration requires actual Chrome and Brave compatibility,
not just syntactically valid registry values. The full release must additionally
demonstrate sleep/resume, device loss, browser restart, service failure, disk full
and lost effect acknowledgment through the same task/job owners. No test runner,
fixture, automatic acceptance claim or generic clear-uncertainty operation is
introduced by this packaging work.

## App timing telemetry compatibility

The optional portrait observer command/phase roster advances the shared telemetry.db writer to schema 9 (schema 8 added native avatar preparation/redraw operations; schema 7 extended preferences commands; schema 6 introduced app timing tables); accepted trace query version 3 and product Store schema 30 are separate boundaries. Install matched Windows/controller binaries built from this source. Writers supporting at most schema 8 reject schema 9, so rolling back only an executable will make telemetry unavailable; preserve the old telemetry backup or keep the newer writer. Do not reset, rename or delete the owner/conversation store as a telemetry workaround. The new installation-scoped app records are local-only and are not added to the paired accepted-trace query. Static/build checks do not prove the upgrade or retained export on an installed instance.
