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
The correction is source-checked; installed confirmation remains a separate step.

Entry points: Overlay shell classes, expanded control classes and existing status
notice branch only. App expansion, runtime controls and native sizing remain
unchanged. The owner prohibits tests, fixtures and harnesses; validation is source
review and focused Svelte checking. Separately authorized installed proof should
compare compact hover/focus and expanded no-hover controls at matching scale,
check actual paused/disconnected states, drag and collapse, and inspect clipping
on the real transparent window. No runtime or acoustic proof is produced here.
