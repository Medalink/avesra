# Bounded local app timings

This foundation observes actual operations on this Windows installation. It is
not accepted-turn evidence, a release benchmark, an authority source or proof of
work completing on another host. Existing accepted trace query version 3 stays
unchanged, as does product Store schema 30. Telemetry schema 6 introduced the
local app timing tables; schema 7 extended the preferences command roster.
Schema 8 added native avatar preparation/redraw operations. Schema 9 extends the
finite native phases and command roster for optional portrait observation, without
adding tables or record fields. Deploy compatible binaries when upgrading the
shared writer. Writers supporting at most schema 8 reject schema 9 before reading
or writing its records.

The existing bounded trace writer receives records with `try_send`; observation
must never wait for queue capacity or alter operation results, admission, retry,
cancellation or retirement. Keep at most 8192 records for seven days, each at most
1024 encoded bytes. Counts expose eviction and observer loss. Read/export returns
the bounded retained cohort, not all activity since installation. Export is an
explicit visible, unlocked Settings operation, with an eight-MiB ceiling and an
atomic new file in the private app export directory. It cannot overwrite a file. The directory is capped at 32 entries; moving old explicit exports is manual.

Records contain finite operation/stage/outcome identifiers, native process UUID,
local receipt wall time, monotonic duration, native start offset when applicable,
package version and optional actual build fingerprint (unavailable initially).
No arguments, SQL, paths, messages, free-form errors, transcript, biometric input,
profile features, render parameters, account identities or credentials are kept.
Scope is the current Windows user's installation, not a selected actor, device,
conversation or paired controller. Historical records never inherit the current
build/profile/engine identity. Package version is not a source fingerprint.

Frontend reports are explicitly frontend observations. Native code assigns the
window and per-page session UUID and checks both on every batch. They cannot
claim a native origin or worker stage. Accept at most 32 records/32 KiB per batch,
128 records per window per second, durations from zero through ten minutes, and
only the closed command/view roster. Local frontend buffering is capped at 64;
flush is event-driven with at most one in flight. Missing/failed submission is
loss, never successful operation evidence. Telemetry commands are excluded from
command timing to avoid recursive instrumentation. Reported frontend loss remains
distinguished from native observer loss.

UI invocation measures the actual promise settlement, including IPC and caller
wait. Mount commit and next animation frame are scheduling observations, not
physical presentation or GPU render duration. Frontend clocks and native clocks
are never subtracted. Native worker observations are made within existing owners:
queued work retains its enqueue Instant and timing token through dequeue; work
ends on actual command completion. Retirement follows dropping command content
and authorization, not receiving a reply. Unfinished token Drop is abandonment,
never completion. Caller withdrawal does not imply actual worker retirement.

## Foundation entry-point inventory

| Entry point | Coverage in this slice |
| --- | --- |
| `runtime.ts::command` | Closed registered-command roster, frontend invoke duration/outcome; no arguments/errors |
| `main.ts::mount`, `App.svelte::onMount` | Mount/next-frame and initial subscription/runtime/device readiness |
| `SettingsView::navigate` | Selected finite view commit/next-frame; superseded work abandoned |
| Native `main::setup`, `Store::open` + settings read | Native initialization and actual synchronous store work |
| `NativeEffects::conversation_history`, `Command::History` | Original queue wait, actual work and content-owner retirement |
| `voice_avatar::current`, redraw prepare/confirm, source/vault load, derivation/publication and candidate avatar cleanup | Native finite portrait phases; see [the phase boundaries](voice-avatar.md#native-portrait-timing-contract) |
| App timing ingress/read/export | Observer-only; excluded from command instrumentation |
| Preview/normal voice/planner/actions | Existing performance/accepted trace observers unchanged |
| Other native commands' internals, model load/health, protected writes, browser/provider phases | Frontend round trip only; internal stage instrumentation remains pending |
| Audio hardware callback, GPU kernel, compositor presentation | No new instrumentation or claims |

Percentiles must keep origin/process/operation/stage separate and include outcome
counts. Failed duration is time to failure, not successful latency. No observer
creates accepted-turn identifiers. A later benchmark comparison must verify its
actual build, profile, scenario and cohort coverage separately.

Verification follows the explicit owner override: source review and coordinated
static/build checks only, no automated tests, fixtures or harnesses. Runtime and
export proof remain outstanding until separately observed. This inventory is not
an app-wide 100% coverage claim.

Abrupt process/page termination can lose unsent frontend batches or buffered writer messages. Loss counters cover observed refusal/eviction, not an invented count of unobserved crash losses. Native startup before collector initialization has no native-origin offset and cannot persist a failure that prevents opening telemetry.

Native history queue/work/retirement rows carry one shared random operation UUID, unrelated to accepted-turn or source identity. UI summaries separate outcomes; p95/p99 remain descriptive even above 30 samples. Native page replacement does not reset its per-window rate limiter. Failed frontend observer registration is shown in App timings and is not retried automatically; reopening the app window is the explicit recovery.

### Timing registration startup order

The native timing ingress state is registered on the Tauri builder, before any
configured webview is created. Both Overlay and Settings import the same one-shot
frontend registration module. Tauri creates configured windows before invoking
the application setup callback, so registering this state inside that callback
leaves the first webview able to invoke before its command state exists.

`begin_app_timing` and batch ingress retain their bounded, nonblocking `try_lock`
admission. A transient refusal still makes one-shot registration unavailable;
there is no automatic retry or change to application work. The App timings status
describes its own Settings page only. It does not report Overlay registration, and
zero exported frontend loss does not prove zero losses on a page that never
registered and therefore could not submit its counter. The observed schema-8
export with no Overlay rows does not establish which registration failure occurred.

Affected entry points are builder state installation and the unchanged
`begin_app_timing` state argument. Batch submission, snapshot/export, finite
registries and telemetry schema 9 are unchanged. Static source verification
establishes ordering only; a separately authorized real-app export must establish
whether the installed Overlay now contributes observations.

## Preferences command roster (telemetry schema 7)

The frontend and native closed registries admit exactly five additional command
names: `apply_preferences`, `begin_preferences_editor`,
`retire_preferences_editor`, `answer_preferences_close`, and `show_settings`.
All five use `runtime.ts::command`; timing records only its existing frontend
invoke-round-trip duration and complete/failed outcome. No preference value,
argument, editor identifier, close-request identifier, result or error text enters
telemetry. Existing observer admission/buffering/loss limits remain unchanged.
These observations cannot authorize Save, Discard, Close, show or any retry.

Native tray and OS-close routes call internal coordinator functions; they are not
IPC samples and gain no fabricated round trip. Editor-close response settlement
does not prove that the whole native close coordinator, file writer or downstream
operation has retired. Those internal timing stages remain pending. The old
`save_settings` name stays in both finite registries so its retained historical
records remain valid, although the new coordinator removes that command surface.

Schema-6 readers validate the old finite command roster and cannot read these new
names. The schema-7 marker therefore prevents an old binary from opening the newer
telemetry store, rather than leaving it to fail halfway through a query. There is
no record rewrite, accepted-query change or app-store migration. Preserve the
pre-upgrade telemetry backup for rollback; never erase conversation/owner stores
to work around this telemetry compatibility boundary. Source/static verification
cannot prove an installed upgrade or observed timings for these new commands.

## Portrait operation compatibility (telemetry schema 8)

`Operation::Portrait` carries only `prepare`, `source_load`, `vault_load`,
`derive`, `publish`, `remove`, `redraw_prepare` or `redraw_confirm`, using the
existing native `work` stage. The Settings frontend ingress rejects these native-only operations. App timings UI
and export retain operation kind/name and outcome; no source or biometric fields
are added. Nested native phases share a fresh per-call timing UUID and overlap;
their durations must not be summed or subtracted from frontend clocks.

The schema-8 `trace::initialize` writer is shared by desktop and controller.
That release accepts telemetry versions 1 through 8 and writes the schema-8 marker only
after existing table initialization succeeds. Older schema-7 binaries reject 8
before table writes. Without that marker their strict `app_timing::read` record
deserializer would reject the newly serialized operation variant. New readers
continue accepting historical operations, including `save_settings`, without
rewriting records. Snapshot/export version 1 and accepted trace query version 3
remain unchanged; the latter does not carry local app timings. Package Store
schema 30 is unrelated and must not change for this addition. The installer
currently gates the product Store schema, not telemetry; deployment must preserve
the telemetry backup and use compatible writers. No installed upgrade/export or
rollback proof is claimed by source/static verification.

## Optional portrait observer roster (telemetry schema 9)

This compatibility slice defines the finite native `Portrait` variants `observe`,
`capture`, `extract` and `save`, and adds exactly `begin_voice_portrait`,
`record_voice_portrait`, `save_voice_portrait` and `cancel_voice_portrait` to both
frontend/native command registries. UI calls use `runtime.ts::command`, which
measures only the frontend invoke round trip. No arguments, ticket/session/source
identifiers, render parameters, biometric features or error text enter records.

The native variants are definitions for the separately implemented observer's
actual call sites, not evidence that those operations ran or hooks are complete.
The native implementation owns their final measured boundaries and cancellation/
retirement inventory in `voice-avatar.md`. Frontend ingress still rejects native
portrait operations. Adding a roster name does not register an IPC handler, grant
capture authority, change native admission or manufacture an observation.

Schema-8 readers cannot deserialize these new operation variants or validate the
new command names. `trace::initialize`, shared by desktop and controller, accepts
versions 1 through 9 and marks 9 after existing table initialization; older
writers reject the new store before table writes. New readers retain all previous
operations and command names, including historical `save_settings`, without
rewriting records. Snapshot/export version 1, accepted query version 3 and product
Store schema 30 stay unchanged. The installer gates product Store compatibility,
not telemetry: preserve the previous telemetry backup and deploy compatible
writers. This source change does not modify the frozen schema-8 build or
prove an installed upgrade, export or rollback.
