# Useful sourced learning

Explicit named facts are governed by [memory.md](memory.md). A committed explicit
fact is an owner assertion with an actual accepted-turn source; its save event
claims only that this value was saved. The reply/chime must follow the successful
transaction. Duplicate values, failed writes and unsupported requests do not
create useful-learning events.

Explicit facts do not themselves implement demonstrations or passive screen learning.
Those require an explicitly selected native app/window or granted origin, a
visible stoppable observation owner, semantic change coalescing, credential-field
exclusion, bounded background work and typed source records. Existing verified
one-step routines remain separate and never inherit extra permissions from a
fact, demonstration or past execution.

## App-opening demonstrations (schema 25)

The first teaching family observes one explicitly selected, current native app
identity. Protected Settings stores an owner-specific observation roster with
allowed/excluded state and immutable revision. A saved app alias alone is not
observation permission. A visible Start/Stop/Cancel session selects an allowed
entry and a routine name. Already-open, ambiguous, inaccessible or excluded apps
produce no candidate; the assistant never closes them to prepare teaching. This
family requires a previously grounded main-window class in the native app catalog.
An existing scope can be excluded even when its saved app is now unavailable.
Uncertain hook retirement refuses saving or a replacement observer; a process
restart may be needed to recover that native observation lane.

A separate retained native observer verifies the executable/package once, keeps
its native identity, and collects title-free absent-to-matching-window and
foreground transitions. WinEvent hints coalesce into one dirty flag with a
2-second backstop; the session lasts at most five minutes and 64 changed semantic
states. It stores no titles, text, screenshots, key events, URLs or credentials.
Accepted foreground work defers scans and invalidates continuity rather than
attributing a transition across an observation gap. Lock, pause, owner/session/
action change, exclusion, Stop, Cancel and caller loss retain the actual worker
until it exits. Cancel discards; Stop saves only an observed qualifying transition.

An explicitly saved candidate is sourced as ObservedTransition, never an
Avesra successful task or proof of which user click caused the window to appear.
Its typed operation is LaunchApp with exact app/alias revisions, a matching live
window postcondition and ordinary current launch permission. It grants nothing.
A new accepted 'run routine NAME' links a fresh task; only an actual verified
invocation validates that exact candidate revision. Identity or structure drift
requires reobservation. Correcting its name creates a candidate revision; protected
deletion clears candidate content and derived notification text, preserving only
source-free audit identifiers. Duplicate observations do not overwrite corrections
or emit duplicate useful-learning events.

This family is not general multi-step/VPN teaching, passive screen interpretation,
or arbitrary click replay. Genuine demonstration, restart, invocation, relocation,
structure drift, correction and deletion remain required live evidence.

## Optional passive app-opening session (schema 27)

The owner may explicitly enable passive learning for one existing allowed app
scope. This is optional and never a prerequisite for conversation. The saved
setting binds the exact scope revision and is separate from execution grants.
The native coordinator may begin one bounded observation attempt in a fresh
acknowledged session, or after an explicit setting revision. It requires the
current native owner, unlocked/unpaused connected state, and the unchanged scope,
app and alias. Exclusion, setting replacement, owner/session/action changes and
Stop retire the actual observer. Settings need not remain open. Stop ends the
current attempt; the saved optional setting may apply to a later fresh session.

The same title-free absent-to-foreground observation produces a distinct
PassiveObservedTransition candidate, never a successful workflow or inferred user
click. No microphone, screenshots, titles, field values or general UI events enter
this learner. An unchanged or already-open app supplies no new opening evidence.
One dirty hint and a two-second backstop remain bounded by five minutes and 64
semantic changes. Interactive speech/tasks defer observation and invalidate
continuity. The low-priority Store mailbox holds at most one pending operation;
foreground operations are selected first. Pending evidence has an additional
30-second age ceiling, never renewed when queued.

The first genuine transition may automatically save the deterministic routine
name `open <saved alias>` and its useful-learning event in one transaction. A
separate owner/scope dedup key is committed with the candidate and survives
correction or deletion. Passive learning never overwrites a routine or recreates
a deleted candidate. New explicit candidates and subsequent explicit corrections or
deletions also retain a scope barrier. A pre-migration erased row without any
recoverable scope conservatively blocks automatic learning for that owner; explicit
teaching remains available without reconstructing deleted content. Existing name collisions produce no learning event. A changed
scope needs explicit revalidation; an existing source key still prevents silent
replacement. The owner can use explicit teaching for a new definition.

No observer is restarted just because a lease expires or an observation fails.
After one candidate, cancellation, timeout or failure the attempt ends for that
session/setting revision. This avoids repeated executable hashing and indefinite
file leases. A restart first rechecks current identity, enabled intent and dedup
state; historical evidence alone never opens an observer or an action. A fresh
invocation retains current ordinary LaunchApp permission and postcondition checks.
This bounded optional family does not complete general continuous visual learning,
multi-step routine extraction or the required live privacy/recovery evidence.

Idle policy reads are cached for 30 seconds under the native session and setting
generation. The observer checks native owner-management exclusion, current session,
lock, pause and busy state without decrypting owner storage on every 100-ms tick.
The create-once owner record is revalidated at admission and immediately around
the candidate publication transaction; the cache never authorizes a stored result.
Changing the supported create-once owner contract requires native owner-generation
invalidation before this cache can remain valid; silent replacement is unsupported.

Both explicit and passive observation use the original native admission Instant
for the entire five-minute lease, including worker scheduling, identity hashing,
baseline sampling and hook setup. The executable reader checks cancellation,
pause/current authority and deadline before and after each 64-KiB read. No hooks
are installed after expired preparation. Synchronous OS I/O retains its actual
file/thread owner until the call returns; a deadline does not pretend to retire a
stalled kernel read or release admission for a replacement observer.
