# Plan 003 acceptance matrix

Audited against integration `2d8eb3aa5ae66897a8f15620674d1de14c8a2563`.
This matrix is a completion gate, not a replacement for the original requirements.
Plan 003 remains **IN PROGRESS**. Static checks do not establish live acceptance.

## Reviewed implementation checkpoint: `49cf90c`

PRs 28–31 add native protected prepare/confirm/cancel redraw, its optional advanced
UI, Settings reopen recovery, and bounded native avatar/redraw timings. They were
reviewed and merged into PR16's integration branch, not main. The source below
was the audit baseline; redraw and native timing are now implemented at this
checkpoint, while their successful protected-operation proof remains unrun.
The frozen Windows static checks passed (22.362 seconds, Svelte 0 errors and 0
warnings); the matching ARM static/release build passed. Windows release passed
in 297.638 seconds. Both matched binaries were installed and inspected on the
separate diagnostic desktop. Optional continuous portrait observation is excluded
from this installed `49cf90c` evidence.

The real People entry initially failed with owner-management busy. Explicit Retry
recovered Eric's actual owner status, but no current owner-bound Personal voice
source existed for a populated avatar. This is a recorded failure, not acceptance.
PR33 serializes those reads and repairs native-show recovery. PRs34–35 add native
continuous-input portrait observation and its optional advanced controls. Root
and independent source reviews passed; they are integrated at `878ca13`, but are
not yet covered by installed evidence. New features require telemetry schema9
and a matching packaged desktop/controller.

Installed `49cf90c` actually exported prepare=4.438ms and source_load=4.084ms,
one observation each, from the unsuccessful avatar-source lookup. The export
contains 127 Settings and 4 native current-process rows, zero Overlay rows.
These are descriptive observations, not a benchmark or populated-avatar proof.
Private artifacts: `artifacts/ui-parity-49cf90c/people-installed.png`,
`people-after-retry.png`, `portrait-timings-installed.png`,
`app-timings-export.json`, and `final-audio-preferences-dom.json`.
Selected Razer Seiren X input, Realtek SPDIF output, voice100, Digital85/85,
chimes15/15 and Single Spark remained unchanged.

| Criterion | Source at audit | Required remaining evidence |
| --- | --- | --- |
| Genuine owner-bound avatar | Protected Personal source required; candidate selection alone is insufficient | Actual populated card from the current owner's selected microphone and saved source |
| Repeatable saved geometry | Version 1 stores 24 quantized values and interpolates 192 display points | Restart the same installed build and compare the protected record's parameters without exporting biometric material |
| Explicit redraw | Protected prepare/confirm/cancel and pending UI implemented in installed49 | Authorized redraw succeeds, preserves the actor ring, and rejects stale owner/device/window/proof contexts |
| Optional twenty-word capture | Reviewed native observer and UI integrated at878ca13; not yet installed | Real two-batch capture, missing/ambiguous slots, cancellation, selected-device ownership and worker retirement |
| Measured portrait features | Version2 native acoustic feature extraction/storage implemented; tempo remains absent | Actual native extraction and versioned storage; prompt pacing must not be reported as measured speech rhythm |
| Speaking and matched state | Empty signal area and unavailable match | Actual correlated speaker observation; no generic input waveform presented as an owner match |
| Reduced motion | Static avatar only | Actual matched activity respects reduced motion without changing saved geometry |
| Other people | Enrollment disabled | Genuine separately enrolled consenting person, green presentation, deletion, and no automatic permission grants |
| Voice setup reveal | Missing | Actual completed voice setup produces the saved portrait, without gating ordinary Personal conversation |
| Deletion and source lifecycle | Candidate deletion removes source-bearing record; actor ring retained | Live deletion/recovery and interrupted-operation proof; owner/Personal reset needs a defined product operation |
| Native timing | Actual prepare/source-load exported from installed49; redraw and observer phase hooks implemented | Actual populated derivation/publication/capture/extraction, including failure and withdrawal; Overlay collector repair |
| Exact mock appearance | Tokens and unavailable-state geometry inspected | Authentic rendered reference and actual app in matching state, viewport and scale; examine image differences |
| Re-enrollment similarity / second-person distinction | Not measured | Real voices and retained before/after comparisons; stored ring uniqueness is not visible uniqueness |

## Design authority and honest limits

The current mock's 148px contour, guide circles and spokes override the original
proposal to display twenty petals and a signature ring. Do not add those visible
layers merely to satisfy older plan prose. Stored ring uniqueness therefore does
not prove visibly distinct avatars. Cross-machine raster identity, perceptual
uniqueness and biometric unlinkability are not established guarantees.

The mock's dates, scores, waveforms and people are simulated design content.
Only genuine native observations may populate the real app. Missing data is not
a reason to invent a success state. The checked-in Design Component pages refer
to an absent `support.js`; a reconstructed renderer would not be an authentic
reference for a 100% visual-match claim.

## Noninterrupting proof procedure

Use `docs/background-evidence.md`: run only an owned production instance on a
separate non-input diagnostic desktop, with microphone disabled and hardware
output silenced. Record exact source/package hashes and inspect actual WebView
controls. Never switch desktops, inject global input, capture the owner's desktop
or microphone, stop their app, synthesize protected records, or unlock their
stores. This proves available UI and native output operations, not real-owner
capture, a second person or acoustic playback. Those remaining evidence gates
require actual authorized use and must remain explicitly unproved until observed.

Follow the owner's no-automated-tests instruction. Use source review, static
checks, production builds and actual permitted app operations. Keep each PR's
implementation status separate from its live evidence and whole-plan status.
