# Setup guide presentation

The Get started page adopts the numbered rail and main-panel hierarchy from
`design/mockups/Onboarding.dc.html` (rail lines 33–56 and 670–675; panel headings
and controls from line 88). It remains inside the current Settings window.
The rail is 200 CSS pixels wide with 36-pixel rows, 20-pixel markers, 13-pixel
labels and the mock's inset Ruby selection stripe. Panel headings use the mock's
22-pixel semibold type, 28-pixel horizontal inset and shared card/button tokens.

This is setup review/navigation, not an imposed wizard. All six existing
destinations remain available in any order. Selecting a rail row changes only
the local review panel; its action navigates to the existing Settings control.
There is no new native command, persisted completion flag, auto-advance, proof,
permission, device selection, model load or capture. Selection resets on remount.

Connection completion means only the current authenticated connection. Device
selection requires the selected input/output to occur in the actual current
enumeration, with no pending enumeration or error. The voice row reflects actual
Personal state; learning is not completion. Speech services always offer review:
`voice_ready` does not prove every reasoning/output lane ready. Generated voice
and actions are optional and never receive invented completion marks. Changing
the selected review page does not set, clear or override native status.

The current Personal status/reason and mute, deafen, pause and lock restrictions
remain visible. Automatic Personal startup is unchanged: no mandatory enrollment,
calibration or step completion is introduced. Diagnostic-mode unavailable reasons
remain literal native status. Browser preview is explicitly identified, not
treated as an installed or paired application.

## Remaining behavior and visual boundaries

The mock's separate setup window/title bar, installer/progress, local starter
model, spoken captions/replies, first-name conversation, provider credentials,
voice-driven tour and typed setup conversation have no matching workflow in this
page. They are not implemented or simulated by this presentation slice. The
existing Settings shell and preference footer remain; the guide rail is an inset
adaptation rather than a claim of full standalone-window pixel parity. No idle
signal animation is invented where this component receives no real signal.

Only `SetupOverview.svelte` and this contract change. Existing pairing, Models,
Audio, People, voice designer, action and browser setup navigation destinations
and their authority/lifecycle behavior remain in their owning components.

## Verification boundary

The owner prohibits automated tests, fixtures and harnesses. Use source review and
a focused frontend static check. Future authorized installed inspection should
compare rail geometry/type at matching scale, select every row with keyboard and
pointer, verify each destination/focus transfer, and inspect actual unavailable,
connected, learning/listening and selected-device states when naturally present.
Confirm optional controls remain optional and long real reasons/device names
wrap without hiding actions. No installed proof or listening outcome is claimed
by this source change.
