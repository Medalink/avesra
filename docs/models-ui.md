# Models and Drivers presentation

The reference is `design/mockups/Settings.dc.html` lines 347–390 and its health
styles, with shared chrome from `design/tailwind/input.css`. This bounded slice
matches the explanatory header card, health legend, 14-pixel card padding,
title/description hierarchy and machine badge layout using existing observations.

`docs/drivers.md` remains the authority for metadata and readiness. Loaded models
remain **Loaded · unqualified**, with an amber treatment, never the prototype's
green Ready state. Loading/stopping use neutral progress styles; incompatible is
red; unavailable is gray; unconfigured uses the dashed configured-state geometry
but retains **Not configured**. Not probed and Not integrated stay explicit and
neutral. The legend describes these actual product states, not simulated states.
The local-machine chip appears only when existing metadata identifies Paired Spark;
no hardware, OS, deployment-sharing or capacity values are invented.

Entry points: SettingsView's Models branch and lane presentation mapping change.
The shared audio/reasoning probe owner, native commands, fifteen-second polling,
visibility/epoch invalidation, Memory > Health, disabled states and per-lane
failure isolation remain unchanged. Every Probe button still invokes the existing
combined metadata refresh, as does Probe all; neither invokes inference. Driver
editing and a deployment roster remain outside this presentation slice because
their prototype controls lack matching supported product operations/data.

The owner prohibits automated tests, fixtures and harnesses. Use focused Svelte
static validation and source review. Separately authorized manual proof must
compare the real Models page at the mock's 880-by-640 reference size and equal
interface scale: initial Not probed, actual loaded/unqualified, disconnected or
unavailable, incompatible if genuinely observed, and in-flight refresh. Verify
legend wrapping, narrow columns, readable status labels and unchanged probe
disabled conditions. Do not fabricate service observations to fill the mock.
No source or static check establishes installed visual parity or model readiness.
