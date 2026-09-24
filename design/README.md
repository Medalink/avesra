# Avesra design mockups

High-fidelity, interactive UI mockups for the Windows companion described in [Plan 001](../plans/001-single-spark-assistant.md), sections 6–7: the compact overlay, its expanded panel, and the six-section settings window.

This is a **design prototype, not application code**. Every state, measurement and task is simulated. Nothing uses the microphone, captures the screen, runs commands, or calls a model. The real overlay and settings are built later in `apps/desktop/` (Plan 001, M4).

## Layout

| Path | Contents |
| --- | --- |
| `mockups/Main.dc.html` | Desktop showcase: a simulated desktop with the draggable overlay, tray, settings window, and a separate panel of prototype-only controls. Runs owner workflow OW2 ("Check my latest 10 emails…"). |
| `mockups/Overlay.dc.html` | The overlay component, including the WebGL voice renderer, the task simulation and the expanded panel. Every other board embeds it. |
| `mockups/OverlayStates.dc.html` | State sheet: all nine overlay states, plus expanded Working, Speaking and Disconnected. |
| `mockups/Settings.dc.html` | Settings window with all six sections. The `Settings*.dc.html` files open it at one section each. |
| `mockups/canvas.json` | Board positions for the design canvas these were authored on. |
| `mockups/avesra.css` | Compiled Tailwind stylesheet shared by all boards. Generated; don't edit by hand. |
| `tailwind/` | Tailwind v4 source (`input.css`: theme tokens, shared component classes, motion) and a pinned build. |

The `.dc.html` files are Design Component pages. They need the design canvas runtime (`support.js`, supplied by the canvas and not in this repo) to render. The markup, Tailwind classes and component logic are ordinary HTML and JavaScript you can read and port.

## Rebuilding the stylesheet

```sh
cd design/tailwind
npm install
npm run build   # writes ../mockups/avesra.css
```

The build scans `../mockups` for class names and is deterministic.

## Design decisions

- **Look:** dark only, neutral grey surfaces (`#121214` to `#26262a`), square corners everywhere, Geist and Geist Mono.
- **Accent:** Ruby `#E0115F`, a scale defined as `--color-av-*` in `tailwind/input.css`. Filled buttons use `#960B3F` so white text stays readable.
- **Source colours** (triadic from Ruby, used on the voice bar):
  - Blue `#3A5DD8`: the owner's voice.
  - Green `#8EDE4A`: other people.
  - Grey: background noise.
  - Ruby/red: Avesra itself.
- **Status colours:** amber means input is off (muted, deafened) or a lane is unavailable or degraded. Red means stop, error or disconnected. Blue marks Cloud — Jev.
- **Overlay:** transparent, built around one live signal drawn in WebGL, with three distinct styles:
  - **Human audio:** a smooth, filled mirrored waveform, coloured by source.
  - **Thinking:** a gamma-wave trace fused with a stepped digital copy of the signal and compute bits.
  - **Avesra speaking:** a twisting ribbon of synthetic strands, red to Ruby.

  Other states draw their own flat indicators. Passive, recognizing, accepted, thinking and speaking show no text. Controls appear on hover, while switched on, or (Stop) while a task runs. There's a CSS fallback and a still frame for reduced motion.
- **Signal hooks:** the simulated signal lives in `Overlay.dc.html`:
  - `signalLevel()`: overall loudness.
  - `updateBars()`: one value per frequency band.
  - `ambientSource()`: who is speaking.
  - `eegSample()`: planner activity.

  In the app these come from the Rust audio pipeline, the speaker-identity lane and planner telemetry.

Plan 001 section 7 adopts this direction and points here as the reference for the overlay and settings.
