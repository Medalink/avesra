# Compact Voice settings

`design/mockups/Settings.dc.html` lines 177–233 govern the compact Voice card,
repeat control, 44-pixel reference strip, explicitly expanded designer and pace/
volume row. `generated-voices.md` governs all real voice operations and the
owner-requested editable default/custom text test, which remains visible here.

Each mounted Voice view starts with the designer collapsed. Only the explicit
Design control opens it. Discovering a selected or saved candidate, refreshing
status and restoring an old draft must not open it. Existing description and
candidate drafts remain saved/restored; the legacy saved open flag is accepted
for compatibility but does not restore expansion. Collapsing the designer does
not discard a draft, cancel a preview or change a selected voice.

The compact reference strip is always present. At rest it displays actual text
from the shown candidate, or an explicit unavailable label. During a reference
preview it displays the original requested reference snapshot. It must not claim
the default test sentence is a saved reference, or show invented waveform bars.
An in-progress request label describes the operation, not proof of acoustic output.

The editable test remains a visible two-row textarea with its byte count,
validation, Use default and Play text actions. This deliberate addition to the
mock preserves the owner's explicit request. Its text remains distinct from the
reference strip. No mount, edit, reset or expand action starts audio.

Entry points: VoiceDesigner draft restoration/status projection and markup only.
Generate/autoplay, saved-reference Preview/Repeat, custom Play text, selection,
discard, clear, refresh, native panel lifetime, cancellation, busy gates, volume,
errors and output restrictions retain their existing commands and behavior.
Pace remains visibly unavailable; no new synthesis capability is implied.

Validation uses focused Svelte/static checks and source review under the owner's
no-tests/fixtures/harness override. Separately authorized manual proof must inspect
the actual 880-by-640 Settings view at equal app/mock scale, with a genuine saved
voice and no voice, then explicit designer open/close and refresh. Confirm drafts
survive collapse and default/custom testing remains accessible. Preview/repeat,
output interruption and custom speech need separately authorized real playback
proof; source checks and screenshots do not establish acoustic success. No runtime
proof is performed by this presentation change.

## Audio detail geometry

The Devices input meter follows the 24-pixel height in
`design/mockups/Settings.dc.html` lines 80–85, including the shared Signal
renderer height. Its real measurements, unavailable state, check command,
freshness and capture ownership are unchanged.

The Chimes section follows the grouped card in that mock's lines 324–343:
an eight-pixel section gap, one divided card, and rows padded 14 pixels
horizontally and 10 pixels vertically. Toggle labels use 12.5-pixel normal text.
The two real learning/action volume controls remain independent, as required by
`notifications.md`, rather than collapsing them into the mock's single slider.
Their labels, ranges, values, disabled states and preferences draft/save behavior
remain unchanged. No chime Preview button is added without a real preview path.
The committed-events view following the controls remains in place.

VoiceDesigner request progress uses the existing `av-spin` SVG arc pattern from
the Settings mock instead of a rounded border spinner, which conflicts with the
shared square-corner rule. Its existing busy conditions and status text remain
unchanged; the arc indicates request progress, not measured audio output.

Entry points changed are only SettingsView's Chimes markup and
MicrophoneMeter's meter/Signal height, plus VoiceDesigner's progress icon.
Preference updates, event delivery,
microphone checking and all VoiceDesigner operations are unaffected. This
presentation slice uses focused Svelte checking and source review; the owner's
no-tests/harness restriction applies. No playback, capture, runtime or complete
visual-parity proof is claimed. Real preview bars remain a separate integration.

## Exact preview display receipt

The native `preview_voice` command accepts one per-invocation Tauri channel for
a display-only receipt. After the existing Settings/panel/current-session checks,
shared output reservation and successful `open_preview`, native sends at most
one `{ output, epoch }` value identifying that actual output lease. Failure to
deliver the receipt does not fail audio, renew a deadline, release ownership or
retry anything. The terminal command result and actual output retirement are
unchanged. The receipt alone never means speech has been submitted or heard.

All three VoiceDesigner entry points use this channel: Generate's saved-candidate
autoplay, saved-reference Preview/Repeat (a fresh channel for each take), and
editable Play text. Native greeting and the explicit voice-check playback trial
do not issue this receipt. The private audio operation with the same name and the
controller preview protocol are unaffected; no new generic command is added.

App passes its existing validated, calibrated output frame separately through
SettingsView. VoiceDesigner displays the mock's 93-by-20-pixel, 16-bar Signal only
when that frame matches both native receipt IDs, preview purpose, assistant speech,
the current playback epoch and this still-pending local invocation/panel. Signal
keeps the original frame expiry; no busy-state waveform, input frame, greeting or
reply can substitute. Missing or malformed receipts and missing frames show no
bars. Bars observe submitted output, not acoustic delivery.
During Play text, the strip shows the exact locally submitted text snapshot and
labels it as test text; it never pairs those bars with the saved reference copy.

Receipt callbacks are bound to the original invocation and visibility generation.
Native Settings-hidden events, lock, disconnect, output restriction, unmount and
command settlement discard the display attempt. Reopening does not revive it.
Native shown/hidden listeners are registered before an initial actual visibility
read; late registration/read results cannot publish after hide or unmount.
Document-visible restoration also requests a fresh native visibility read,
limited to one in flight and one coalesced pending check; it never revives a
previous invocation. Native shown events can restore display eligibility without
DOM focus or visibility changes.
Listener/visibility failures disable bars without disabling ordinary preview.
No timer polls window visibility and no observer changes audio ownership.

Validation is scoped Svelte/Rust static checking and independent source review,
with no automated tests, fixtures or runtime work. Exact native display receipt
is a desktop IPC change; controller, core contracts and telemetry schemas remain
unchanged. Matching native and frontend desktop artifacts must ship together.
