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

## Settings chrome follow-up

Normal Settings opens Audio & Voice when no valid six-section selection is saved.
The legacy `setup` selection also restores Audio. The six reference navigation
rows begin at the top of the 200-pixel rail with no extra heading or seventh row.
The existing setup guide remains a temporary route reached from the Profiles
connection area; it is not the complete reference onboarding implementation.

The title uses the reference circle and three bars. Navigation icons preserve
the literal circle, ellipse and path geometry in `Settings.dc.html`. Device
selectors use the reference input/select chrome and 14-pixel chevron through
`SelectFrame`. The same wrapper supplies existing action, teaching, private-memory
and saved-voice-candidate selectors; private-memory textareas regain input chrome. Existing
switch tracks remain 36 by 20 pixels with 14-pixel thumbs; the checked translation
is 18 pixels, and the unchecked translation is 3 pixels. Existing real control
values, disabled states and native commands are unchanged.

Entry points: Settings restored navigation, Profiles setup link, title/navigation
icons, device selectors, and shared selects/textareas. Audio, owner, pairing,
protected action and save commands retain their current authority. No synthetic
values, new backend functionality or fake Save/Discard footer is introduced.

The footer is a separate behavior gap: `App.update` immediately invokes
`save_settings`; native state and window/device changes happen before the
durable writer completes. It does not apply at a next-turn boundary. A future
draft must allowlist ordinary preferences, merge untouched current values,
detect conflicts, guard actual close/hide exits and preserve independently live
sound/safety/protected operations. The mock's Discard explicitly preserves sound.

The manual WebView inspector accepts a fourth CLI argument `settings` (default)
or `overlay`. It selects exactly one page at the matching production URL from
the loopback CDP page list and preserves the existing local-socket check. This
only permits an explicit DOM expression and optional new screenshot; it neither
creates application state nor supplies a visual assertion. See
`background-evidence.md` for commands and evidence boundaries.

## People wrapper follow-up

The People section places the existing Windows verification banner before the
Your voice card. The banner reads current verification status; Windows Hello
runs only from its explicit button or an existing protected management action.
The remembered-name editor belongs inside the voice card's Advanced section,
not a separate duplicate outside the card. This wrapper change must be integrated
with the EnrollmentView change that owns that editor.

Other people uses the reference header/action and card spacing, while the enroll
action remains visibly unavailable because there is no supported additional-person
flow. It must not invent a person, grant or successful enrollment. Rules use the
reference divided card, with current Personal policy from `docs/enrollment.md`:
initial natural speech may establish the owner's provisional voice association;
it is not truthful to claim all unknown voices are ignored during that phase.
Voice association never grants permissions or proves identity. Source entry points
are the SettingsView People wrapper and existing SetupLock; enrollment, protected
verification and speaker persistence commands are unchanged.

## Observed production proof for 7690fc1

On 2026-09-26 the separately operated, installed production package was inspected
on its owned non-input desktop. The launch record identifies PID 85512 and an
unchanged `Default` input desktop. The matching desktop SHA256 is
`95e0a768a317153894125fc72a60c8121d8259b24c08c07993f72efe80ed4080`;
native host is `cade5da3db83ff7b2a2431e1fca8f23a414cdebb269fe3c14e9b0ff7518a9bff`.
The controller remained at d024 with its native/service source unchanged.

Evidence is retained in `artifacts/ui-parity-7690fc1/` on the proof workstation:

| Observation | Retained evidence |
| --- | --- |
| Settings viewport 880 by 640 CSS pixels, header 44 pixels, rail 200 pixels | `history-controls-dom.json`, `history-controls.png` |
| History mode selector is 32 pixels high and uses `av-input av-select` | Same History DOM record and screenshot; protected content was not opened |
| Expanded overlay remains 440 by 370 CSS pixels at interface scales 100, 110, 125, 150 and 175% | `expanded-100.json`, `expanded-110.json`, `expanded-restored-125.json`, `expanded-150.json`, `expanded-175.json`; screenshots at 100 and 175% |
| After restoring 125%, collapse returns to 440 by 124 CSS pixels | `compact-restored-125.json` |
| Selected Razer Seiren X input, SPDIF output, Digital 85/85 and voice volume 100 remain unchanged | `audio-preserved.json` |

The operator used the actual installed WebViews and closed only the verified
owned process. No microphone input, voice preview or protected Windows Hello
read was performed. This establishes the listed geometry and state preservation
for 7690fc1 only. The later Settings chrome follow-up above has static validation
and still needs its own installed visual proof.

For manual geometry observation after opening the intended real view, the
inspector can retain an explicit DOM result and a new screenshot:

```powershell
node scripts/inspect-webview.mjs '({width: innerWidth, height: innerHeight, dpr: devicePixelRatio})' 'E:/Dev/Avesra/artifacts/settings-next.png'
node scripts/inspect-webview.mjs '({width: innerWidth, height: innerHeight, dpr: devicePixelRatio})' 'E:/Dev/Avesra/artifacts/overlay-next.png' overlay
```

Select scales and expand/collapse through actual controls; inspect each resulting
view rather than assigning dimensions or fabricating state in the DOM.

## Manual proof still required

With the genuine app, inspect compact and expanded modes at each supported scale
(100, 110, 125, 150 and 175%). Change scale while expanded, collapse again, and
repeat expansion quickly. Native dimensions and rendered content must agree with
the same bound state; status snapshots must not collapse an expanded window.
Inspect actual native working, speaking, mute, pause and disconnected transitions.
The working indicator must not claim measured progress, and reduced motion must
remove its shimmer. Compare the actual 84-pixel signal with the reference at equal
scale using real permitted capture/output, without fabricating samples or states.

The listed 7690fc1 observations do not establish real working/speaking transitions,
signal rendering under live activity, reduced motion, all controls or complete
visual parity. No automated tests were performed. Static validation is separate
from visual and functional verification.
The missing design-canvas `support.js` prevents authentic local rendering of the
`.dc.html` boards; reconstructing its behavior would not establish exact parity.

## Remaining source-visible gaps

| Surface | Remaining work / boundary |
| --- | --- |
| Settings shell | Six-section rail, default Audio, title mark and device select chrome are aligned in source. The 52-pixel draft/save/discard footer remains pending real save semantics and close guards. |
| Onboarding | SetupOverview's six cards differ from the reference step rail, captions, signal band and twelve install-to-rest boards. Genuine onboarding transitions must replace simulated ones. |
| Compact overlay | Resume/reconnect controls, reference grip and finer state chrome remain separate; no real step-level progress projection exists yet. |
| Expanded overlay | Request/reply, task-step and recent-activity projections, typed input, state banners and shell chrome are not yet the complete reference implementation. |
| Audio | Device selectors are aligned in source; output test and other screen geometry remain pending. Reference listening-mode choices conflict with current natural-conversation policy and need a product decision. |
| Models | Probe-only status cards do not implement the reference driver/model/machine editing and deployments; several lanes explicitly remain unintegrated. |
| Profiles | Accelerated is unavailable; automatic Gaming, assignment details, memory budget and full machine roster are not the complete reference flow. |
| Awareness | Fixed Off/empty scope does not provide display/app selection and the complete observation/approval UI. Existing task and alias tools cover narrower real functions. |
| Memory / History | Real protected records/search/deletion exist, but toolbar/filter/compact-row/source-chip geometry and reference History export remain separate work. |
| Performance | Genuine measurements and limitations must be retained when replacing stacked cards with the reference summary cards/stage table. No prototype latency or release-pass values may be copied. |
| Shared controls | Current device/action/teaching/private-memory/voice-candidate selectors and private-memory textarea now use reference chrome. VoiceCheck belongs to the separate People slice; atmosphere switch/card geometry and other screen-specific controls still need parity work. |
| People / Plan 003 | Separate active owner: resolve the 192-feature contour versus 20-petal/signature-ring design before claiming exact visual parity. This slice does not edit People or avatars. |

These gaps prevent a 100% design-parity or beta-readiness claim. Completion requires
real matching-state visual evidence and actual supported control outcomes.
