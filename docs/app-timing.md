# Bounded local app timings

This foundation observes actual operations on this Windows installation. It is
not accepted-turn evidence, a release benchmark, an authority source or proof of
work completing on another host. Existing accepted trace query version 3 stays
unchanged. Shared telemetry schema 6 adds local app timing tables; deploy matched
binaries when upgrading the shared writer. Older binaries reject schema 6.

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
