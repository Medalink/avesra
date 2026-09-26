# Desktop parity with the approved design

The authoritative source is `design/` in the active worktree, including design
revision `ca27240`. `design/README.md` defines the 100% scale comparison: one CSS
pixel per screen pixel at 100% Windows scaling. App and reference must be compared
at the same interface scale and genuine state. Prototype measurements, tasks,
people and outcomes are not implementation evidence.

## First bounded overlay slice

`App.svelte` owns the sole reactive expanded state and the overlay's scaled native
window size. `Overlay.svelte` changes that bound presentation state but does not
call the window sizing API. Native startup still establishes the initial compact
size, and native Settings owns WebView zoom and the Settings window size. Reference
overlay dimensions remain 440 by 124 compact and 440 by 370 expanded, multiplied
by the selected interface scale. Scaling must preserve the actual expanded state.
Settings retains its 880 by 640 reference size and 200-pixel navigation rail.

The overlay passes an explicit 84-pixel height to its 44-bar Signal, matching
`Overlay.dc.html`; the defaults for other signal consumers are unchanged. Samples,
source labels, retirement and freshness remain owned by the existing native lanes.

Actual native `working` status displays five equal 4-pixel tracks separated by
4-pixel gaps, matching the mock's segment geometry. The present Runtime projection
contains no step position or progress percentage. These tracks therefore use an
indeterminate sheen, with no completed fills, numbered step, progress fraction,
or invented task data. Reduced motion retains still tracks. Fresh submitted speech
keeps its existing visual precedence. The working label uses the reference Ruby
color; it must not look like a muted microphone. This is partial presentation
parity, not the mock's demonstrated five-step task execution.

The protected History search-mode selector uses the approved `av-input av-select`
chrome, relative chevron and 1.5-unit label spacing. Its adjacent query field uses
the same label spacing. Search commands, bounds, proof and reader behavior remain
unchanged; this fixes only the control presentation observed in the real app.

Entry points: App expansion binding/interface-scale effect; Overlay expansion
button and indicator branch; existing Signal renderer and native startup/zoom.
No capture, playback, planner or accepted-task authority is changed.

## Manual proof still required

With the genuine app, inspect compact and expanded modes at each supported scale
(100, 110, 125, 150 and 175%). Change scale while expanded, collapse again, and
repeat expansion quickly. Native dimensions and rendered content must agree with
the same bound state; status snapshots must not collapse an expanded window.
Inspect actual native working, speaking, mute, pause and disconnected transitions.
The working indicator must not claim measured progress, and reduced motion must
remove its shimmer. Compare the actual 84-pixel signal with the reference at equal
scale using real permitted capture/output, without fabricating samples or states.

No runtime/UI/device proof or automated tests are performed by this slice. Focused
frontend static validation is separate from visual and functional verification.
The history search selector also needs a same-scale visual recheck after packaging.
The missing design-canvas `support.js` prevents authentic local rendering of the
`.dc.html` boards; reconstructing its behavior would not establish exact parity.

## Remaining source-visible gaps

| Surface | Remaining work / boundary |
| --- | --- |
| Settings shell | Additional Get started navigation, different title icon, and missing 52-pixel draft/save/discard footer; current preferences save immediately. Reconcile behavior before adding controls. |
| Onboarding | SetupOverview's six cards differ from the reference step rail, captions, signal band and twelve install-to-rest boards. Genuine onboarding transitions must replace simulated ones. |
| Compact overlay | Resume/reconnect controls, reference grip and finer state chrome remain separate; no real step-level progress projection exists yet. |
| Expanded overlay | Request/reply, task-step and recent-activity projections, typed input, state banners and shell chrome are not yet the complete reference implementation. |
| Audio | Device select/chevron details and output test differ; reference listening-mode choices conflict with current natural-conversation policy and need a product decision. |
| Models | Probe-only status cards do not implement the reference driver/model/machine editing and deployments; several lanes explicitly remain unintegrated. |
| Profiles | Accelerated is unavailable; automatic Gaming, assignment details, memory budget and full machine roster are not the complete reference flow. |
| Awareness | Fixed Off/empty scope does not provide display/app selection and the complete observation/approval UI. Existing task and alias tools cover narrower real functions. |
| Memory / History | Real protected records/search/deletion exist, but toolbar/filter/compact-row/source-chip geometry and reference History export remain separate work. |
| Performance | Genuine measurements and limitations must be retained when replacing stacked cards with the reference summary cards/stage table. No prototype latency or release-pass values may be copied. |
| Shared controls | Modifier-only selects/textareas miss `av-input` chrome; switch thumb offsets and per-icon stroke geometry differ. |
| People / Plan 003 | Separate active owner: resolve the 192-feature contour versus 20-petal/signature-ring design before claiming exact visual parity. This slice does not edit People or avatars. |

These gaps prevent a 100% design-parity or beta-readiness claim. Completion requires
real matching-state visual evidence and actual supported control outcomes.
