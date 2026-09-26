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
