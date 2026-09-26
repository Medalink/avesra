# Voice Atmosphere presentation

The Audio & Voice atmosphere panel follows `design/mockups/Settings.dc.html`
lines 235–320 and the shared `design/tailwind/input.css` primitives: section
kicker, card heading, master and layer switches, Digital/Human radio cards,
Original/Atmosphere segmented comparison, six advanced amount controls, and the
three-column texture/left-spacing/right-spacing row.

Preset cards use a single selected Tab stop. Arrow keys move focus and invoke
the adjacent card's existing click action; Space/Enter retain native button
activation. No separate sound mutation path is introduced.

This is presentation only. `VoiceAtmosphere.svelte` keeps the existing
`update_sound` partial-edit handlers, per-preset saved values, original bounds,
reverb and pitch unit conversion, one in-flight coalesced slider writer, immediate
bypass path, media-health subscription and current output-layout read. All
controls remain available with their existing disabled conditions. No preview,
capture, device selection, sound defaults, DSP, authority or persistence changes.
Actual output layout remains unknown until observed; the mock's sample values
are never substituted. The output contract remains `docs/playback.md`.

Verification is source review and focused Svelte checking under the owner's
no-tests instruction. A source match is not an installed visual or audio proof.
