# Overlay shell and expanded controls

The source reference is `design/mockups/Overlay.dc.html`: shell classes at
629–632, expanded control visibility at 729–738 and disconnected/paused notices
at 171–188. This slice ports those visual rules into the existing Overlay.

The shell uses the mock's translucent expanded surface, ring, backdrop blur and
shadow classes. Compact mode remains transparent until hover/focus. The existing
native window bounds and App-owned expansion/scale behavior are unchanged:
440 by 124 compact, 440 by 370 expanded, at 100%. The CSS shadow can be clipped by
the native surface; this change does not claim an exterior composited shadow or
alter the window/hit-test bounds to accommodate it.
The expanded body has a bounded scrolling region below the unchanged signal and
controls. Long actual reasons/errors remain accessible inside the fixed native
height rather than overflowing the hidden outer shell.

Expanded controls remain visible without hover. Compact controls keep their
existing hover/focus visibility, while active mute/deafen and Stop preserve their
current visibility and actions. Drag ownership, button handlers, input controls,
the 84-pixel Signal, submitted-sample precedence and working indicators are
unchanged. No new native operation or status authority is introduced.

Existing disconnected and paused states receive the corresponding reference
notice treatments. Copy uses the actual runtime reason or a conservative missing
connection message; it never invents a disconnect time, completed task, safe retry
or unchanged external state. Known explicit microphone-check/enrollment notices
remain; the generic notice uses the actual runtime reason and does not infer
capture absence from legacy enrollment/readiness flags. Expanded
request/reply text, task steps, recent events and typed requests are still absent;
the UI keeps that limitation explicit instead of copying prototype content.

Installed `7aa1b8e` expanded-overlay evidence showed the same runtime reason in
both the introductory paragraph and the status notice. Generic, paused and
disconnected states now show that reason only in their notice. The introductory
paragraph remains for actual speaking/preview state and explicit microphone-check
or enrollment activity, whose separate stop instructions remain visible. This
only changes duplicate rendering; it adds no status flags or native behavior.
Installed `1e0aaa9` confirmation is retained in
`E:/Dev/Avesra/artifacts/ui-parity-1e0aaa9/overlay-expanded.png` and
`overlay-expanded-dom.txt`: 440 by 370 CSS pixels at DPR 1.25, the actual reason
shown once, and all listed expanded controls at opacity 1. The operator recorded
owned app PID 65452 / WebView PID 84124 on a non-input desktop with `Default`
unchanged. This establishes that baseline correction only, not the later
controls/footer source below; no app was launched for the later source change.

Entry points: Overlay shell classes, expanded control classes and existing status
notice branch only. App expansion, runtime controls and native sizing remain
unchanged. The owner prohibits tests, fixtures and harnesses; validation is source
review and focused Svelte checking. Separately authorized installed proof should
compare compact hover/focus and expanded no-hover controls at matching scale,
check actual paused/disconnected states, drag and collapse, and inspect clipping
on the real transparent window. No runtime or acoustic proof is produced here.

## Status geometry and expanded footer

The next source slice uses the reference six-square 8 by 12 grip, state dot and
10.5-pixel uppercase monospace label, paused 13-pixel filled glyph with 3-pixel
hatched rails, offline 14-pixel X, and Stop ring/hover treatment. Drag still calls
the existing native drag handler. Status comes only from current Runtime and
submitted signal precedence; no mock task fraction or speaker inference appears.
The compact status subline may show the actual runtime reason. Expanded mode
keeps that reason in its existing notice, avoiding duplicated prose.

The expanded footer follows the reference border/padding and uses Collapse and
Pause assistant / Resume. Collapse changes the existing App-owned expanded value;
Pause/Resume calls the existing local control and does not restart a task. These
remain local controls while Spark is disconnected; only missing runtime, Windows
lock or the current pending control disables this footer action. This preserves
native offline control instead of the mock offline gate.
No new IPC, reconnect, typed request or task-resume path is introduced. The header
remains fixed; only the body scrolls above the footer inside the unchanged
440 by 370 expanded window. Active mute/deafen/Stop remain visible, and all
expanded controls stay visible without hover.

Entry points changed: Overlay presentation and its existing `control`, `drag`
and bound `expanded` callbacks. Runtime admission, App/native sizing, settings
show/hide ownership, signal expiry and audio authority are unaffected. Validation
is focused Svelte/source checking only; new installed geometry, drag and real
Pause/Resume behavior remain manual proof, not covered by the baseline image.

## Remaining backend projection contract

Runtime currently exposes task activity only as a boolean. Real accepted text and
planner response/provenance exist in native conversation/PublishedReply owners;
TaskView has typed state/outcome/target but no general ordered steps or fractional
progress. Committed notification events provide real learning/action descriptions.
Their present Settings readers are not Overlay feeds. In particular, broad
ActionSnapshot also includes permissions and private memories; do not widen its
window allowlist or forward it wholesale to reproduce the mock.

A separate reviewed native contract must define a bounded, read-only Overlay
projection of the current accepted turn, actual reply, task summary and recent
events. Preserve the applicable owner/privacy admission and bind publication to
native session and content generation; withdraw on lock, hide, context changes
and deletion. Neither a stored response nor submitted samples prove acoustic
playback. Display-only records cannot recreate output or action capabilities.

Five-step progress, task resume, typed-request acceptance and owner/other hearing
classification require their real missing capabilities. Do not manufacture
those states, use elapsed time as progress, replay history as current speech, or
add privileged debug/IPC shortcuts. Request/reply, Recent and task panels remain
explicitly unavailable until the projection contract is implemented.
