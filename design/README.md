# Avesra design mockups

High-fidelity, interactive UI mockups for the Windows companion described in [Plan 001](../plans/001-single-spark-assistant.md), sections 6–7: the compact overlay, its expanded panel, and the six-section settings window.

This is a **design prototype, not application code**. Every state, measurement and task is simulated. Nothing uses the microphone, captures the screen, runs commands, or calls a model. The real overlay and settings are built later in `apps/desktop/` (Plan 001, M4).

## Layout

| Path | Contents |
| --- | --- |
| `mockups/Main.dc.html` | Desktop showcase: a simulated desktop with the draggable overlay, tray, settings window, and a separate panel of prototype-only controls. Runs owner workflow OW2 ("Check my latest 10 emails…"). |
| `mockups/Overlay.dc.html` | The overlay component, including the task simulation and the expanded panel. Every other board embeds it. |
| `mockups/VoiceBars.dc.html` | The voice bars: one WebGL renderer, with a CSS fallback, used wherever voice activity shows. |
| `mockups/OverlayStates.dc.html` | State sheet: all nine overlay states, live transition sequences, and expanded Working, Speaking and Disconnected. |
| `mockups/Settings.dc.html` | Settings window with all six sections. The `Settings*.dc.html` files open it at one section each; `SettingsPeopleYou` and `SettingsPeopleOther` freeze the Your voice card mid-check. |
| `mockups/Onboarding*.dc.html` | First-run setup: the setup window (`Onboarding`), an interactive run on the desktop with the real overlay (`OnboardingDemo`), the flow map, and one frozen board per step. |
| `mockups/canvas.json` | Board positions for the design canvas these were authored on. |
| `mockups/avesra.css` | Compiled Tailwind stylesheet shared by all boards. Generated; don't edit by hand. |
| `tailwind/` | Tailwind v4 source (`input.css`: theme tokens, shared component classes, motion) and a pinned build. |

The `.dc.html` files are Design Component pages. They need the design canvas runtime (`support.js`, supplied by the canvas and not in this repo) to render. The markup, Tailwind classes and component logic are ordinary HTML and JavaScript you can read and port.

## Size and scale

The boards are drawn at **100%**: one CSS pixel is one screen pixel at 100% Windows display scaling. The design canvas zooms boards to fit your browser, often 1.5–2× on a 3440-pixel-wide display, so they look larger there than the app does at the same settings.

- To judge real size, set the canvas zoom to 100%, or to the interface size you use.
- The app's **Interface size** setting (Profiles & Machines → Display) zooms every Avesra window uniformly with WebView2 zoom and scales the window sizes to match: 100, 110, 125 (default), 150 or 175%. The reference geometry stays exact at every size.
- Compare the app with the mockups using screenshots at the same scale, never against a canvas that's zoomed to fit.

## Rebuilding the stylesheet

```sh
pnpm install    # from the repository root; design/tailwind is a workspace package
cd design/tailwind
pnpm build      # writes ../mockups/avesra.css
```

The build scans `../mockups` for class names and is deterministic. It shares the workspace lockfile and its 7-day package age rule (see `docs/security.md`).

## Design decisions

- **Look:** dark only, neutral grey surfaces (`#121214` to `#26262a`), square corners everywhere, Geist and Geist Mono.
- **Accent:** Ruby `#E0115F`, a scale defined as `--color-av-*` in `tailwind/input.css`. Filled buttons use `#960B3F` so white text stays readable.
- **Source colours** (triadic from Ruby, used on the voice bar):
  - Blue `#3A5DD8`: the owner's voice.
  - Green `#8EDE4A`: other people.
  - Grey: background noise.
  - Ruby/red: Avesra itself.
- **Status colours:** amber means input is off (muted, deafened) or a lane is unavailable or degraded. Red means stop, error or disconnected. Blue marks Cloud — Jev.
- **Voice bars:** every place Avesra shows voice activity uses one bar style: 3 px bars on a 6 px pitch, centred, tallest in the middle, pulsing with CSS keyframes. Colour says who:
  - Blue: you. Green: other people. Grey: background.
  - Ruby: Avesra speaking. Light Ruby (`av-300`): Avesra thinking, low and slow so it never reads as talking.

  They're drawn by one WebGL shader: a bright core fading toward the tips, a highlight at each tip, a soft glow in the bar's colour, and a slow sheen through Avesra's speaking bars. State changes reshape and recolour the same bars in a centre-out ripple (420 ms, 9 ms per bar from the centre). Every mockup mounts `mockups/VoiceBars.dc.html`; the app uses `apps/desktop/src/voice-bars.ts` through `Signal.svelte`. Without WebGL, plain CSS bars with the same geometry and colours take over. Reduced motion draws a still frame.
- **Overlay:** transparent, built around the voice bars. Other states draw their own flat indicators. Passive, recognizing, accepted, thinking and speaking show no text. Controls appear on hover, while switched on, or (Stop) while a task runs.
- **One shell:** the Settings window (880 × 640 at 100%) is the reference chrome. The setup window uses the same size, title bar and navigation styling, and showcase boards embed the real overlay component rather than copies.
- **Voice atmosphere** (Plan 002): Audio & Voice → Voice atmosphere mirrors the app.
  - A master switch, and **Digital** (default) or **Human** presets. Each preset keeps its own values.
  - Main sliders: Voice presence and Galaxy / background.
  - Original / Atmosphere comparison during a repeating preview.
  - More sound controls: the effect and background layers; Bass, Reverb, Echo, Harmonizer and Compression; Harmony pitch; background texture; left/right echo spacing.
  - Digital shows the owner-approved factory tune. Human disables the controls it ignores.
  - Atmosphere is on by default and is controlled only from Settings; the compact overlay has no atmosphere button. `av-iconbtn-accent` (Ruby) remains the style for any icon toggle that switches a feature on, since amber means input is off.
- **Signal hooks:** the simulated signal lives in `Overlay.dc.html`:
  - `signalLevel()`: overall loudness.
  - `ambientSource()`: who is speaking while passive (the `ambient` prop can pin it).

  In the app these come from the Rust audio pipeline, the speaker-identity lane and planner telemetry.

Plan 001 section 7 adopts this direction and points here as the reference for the overlay and settings.
