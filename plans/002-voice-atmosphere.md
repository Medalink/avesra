# Plan 002: Give Avesra a distinctive voice and atmospheric accompaniment

## Status and intent

- Status: IN PROGRESS; Digital sound explicitly approved on 2026-09-25 and saved as factory defaults. Expanded pitch control and animation verified live. Human audition/approval deferred.
- Priority: P1 product direction. Effort: L, staged. Risk: medium; native real-time output, cancellation and completion semantics change.
- Planned at: `17a10ea`, `codex/companion-onboarding`, 2026-09-25, INCLUDING the inspected dirty working tree.
- Depends on: Plan 001's existing generated-reference preview and native output ownership. Preview can establish this feature independently of normal assistant readiness; normal conversations still require Plan 001 qualification.
- User request: a separate looping white-noise/music atmosphere accompanying speech, bass/tone/effects, an emotionally expressive digital identity reminiscent of an EDM concert, and an easy off toggle. Expanding the app is permitted. The owner's follow-up specifies exactly two presets: **Digital** (default) and **Human**.
- Owner acceptance, 2026-09-25: done means playing BOTH presets to the owner and receiving approval; iterate until the owner is happy. The latest exact audition text is **"Hello I am Avesra, A Very Effective Smart Reasoning Assistant. I am designed to help you manage your thoughts and ideas."** The owner explicitly requested looping this while tuning live, then halting it. This supersedes the initial Hello Eric greeting and earlier no-playback restrictions for this work. Static/build success is not completion.
- Subsequent steering: finish Digital first; Human is deferred until Digital is right. First native Digital audition completed source/mix output, but the owner heard only voice, no noticeable background or effects. This is rejected, not approved. Increase audible atmosphere/effects and regenerate a distinctly synthetic source voice as the owner requested. Do not claim completion until explicit approval.
- Existing product instruction: specs/code, static checks and builds; no automated tests, fixtures or test harnesses. Do not run playback or disruptive UI inspection as part of implementing this document without an applicable user instruction. This planning session did neither.

The default Digital sound direction is **a warm, grounded digital voice surrounded by a dark, slowly moving space**. Keep a clear central voice, add harmonic weight and a restrained synthetic texture, and let space bloom around phrase endings. Increasing Character can make Digital theatrical; everyday responses must remain easy to understand. Human is the close, warm, minimally processed alternative. This is a proposed listening direction, not a claim that any preset has been heard or tuned.

## Architecture decision

Use two logical audio lanes inside the existing Windows renderer, sharing one CPAL output stream and device clock:

```text
Spark selected-voice PCM -> validate/queue -> existing rate converter
                                             |
                           voice EQ/compression/texture -> dry voice --------+
                                             |                              |
                                             +-> filtered space return -----+-> mix
                                                                            |    |
local prepared atmospheric loop -> loop crossfade -> speech-driven ducking -+    |
                                                                                 v
                                                        master gain -> limiter -> device
                                                                                 |
                                            actual submitted channel references -+
```

Keep the network TTS payload clean and mono at its negotiated 24 kHz. Mixing, loops and effects belong on the companion CPU. There is no second live network stream, second sound device, WebView audio player or music model running for each reply. Prepare loops once and reuse them. This preserves the single-Spark and Gaming/no-client-inference product requirements; CPU performance still needs measurement.

Use a fixed DSP graph. Evaluate a pinned FunDSP release for filters, compressor, noise/oscillators and algorithmic reverb; its documented `allocate()` step belongs before real-time use. Verify the chosen nodes, dependency license and Windows build before adopting it. Do not introduce plugin hosting, an arbitrary graph editor or a DAW dependency. If the dependency is unsuitable, report the specific issue before inventing an entire DSP framework.

## Current state and integration evidence

Paths are relative to `E:\Dev\Avesra`.

- `crates/avesra-windows/src/audio.rs:81`: `PlaybackFrame` is mono `[i16; 480]`, typed 16/24 kHz, with exact epoch/utterance/sequence/deadline/final-frame binding. Keep its transport meaning.
- `crates/avesra-windows/src/audio.rs:108`: `PlaybackReference` contains mono device-rate samples and `final_submitted`; its comment assumes identical channel replication. Stereo invalidates that assumption.
- `crates/avesra-windows/src/playback.rs:23`: `Renderer` owns bounded source queues, sinc conversion, final sample counts, and replay tombstones. Its end logic currently clears the renderer at the last source-derived output sample:

  ```rust
  let last = self.target == Some(self.emitted);
  if last {
      self.clear();
  }
  Ok(Some((value, binding, last)))
  ```

- `crates/avesra-windows/src/playback.rs:300`: current output converts one sample and replicates it:

  ```rust
  let submitted = T::from_sample(sample);
  for channel in frame {
      *channel = submitted;
  }
  ```

- `apps/desktop/src-tauri/src/media.rs`: `Media` owns output leases, worker/device lifetime, submitted references and completion. `sync` deliberately keeps independent playback alive through microphone-only changes.
- `apps/desktop/src-tauri/src/preview.rs:295` and `speech.rs:406`: both apply `speech_volume` by multiplying PCM before enqueue. Move this gain to the common renderer, exactly once, so master zero also silences the atmospheric layer and controls apply during speech.
- `apps/desktop/src-tauri/src/preview.rs`: after End it currently waits up to 1500 ms for source submission and reports source sample counts to the server, then retains the device through estimated drain. Do not make that acknowledgement wait for an arbitrary music tail.
- `apps/desktop/src-tauri/src/playback_signal.rs`: `Telemetry::sample` derives a displayed waveform from submitted samples; this is not an echo canceller.
- `crates/avesra-core/src/state.rs:14`: serde settings are strictly validated, with additive defaults for existing preferences. `speech_volume` currently defaults to 80; preserve its stored value.
- `apps/desktop/src-tauri/src/main.rs:258`: `save_settings` applies native settings, advances output epochs for device/profile changes and uses the existing bounded persistence writer. Sound-character changes must not advance playback/action/capture epochs or reopen devices.
- `apps/desktop/src/VoiceDesigner.svelte`: reference preview and generation are already connected; pace is unavailable. A preview is the generated reference, not synthesized arbitrary text.
- `apps/desktop/src/Overlay.svelte:117`: the existing deafen button supplies the local-control styling/accessibility pattern. Keep deafen separate from atmosphere bypass.
- `services/audio/avesra_audio/streaming_tts.py:113`: normal synthesis passes a selected clone prompt to the pinned Base model; it does not expose a per-utterance emotional instruction.

Match existing typed Rust boundaries, bounded queues, native-authoritative state and Svelte 5 components. Preserve Ruby accents, square controls, bundled Geist fonts, the existing Audio & Voice settings section and interface scaling. Extend the page with a sound card; a seventh settings section is unnecessary.

## Product behavior

### Easy off and controls

Name the feature **Voice atmosphere** in product UI. Explain it once as "Voice character and background sound."

1. One master On/Off toggle in Audio & Voice and a matching one-click overlay control. The overlay button remains discoverable while enabled, has a tooltip and `aria-pressed`, and is keyboard accessible. Both show the same native state.
2. Off crossfades within 30 ms to the dry voice at the current master volume, removes the background and effect returns, then clears their state. It never cancels or restarts the sentence. Off while idle never opens an endpoint.
3. Deafen/Stop/Pause/lock/disconnect remain immediate output invalidation: silence every lane, discard tails and never wait for the cosmetic fade. Mic mute alone continues to preserve valid output authority.
4. Exactly two presets: **Digital** (default; resonant synthetic character, spacious effects, quiet atmosphere) and **Human** (warm, close, light transparent processing; background off by default). Higher Character/Space values provide the concert-scale sound within Digital, without a third preset. A preset selects DSP/background parameters; it never changes the selected generated voice. Human cannot turn an inherently synthetic source into a different natural speaker; it preserves the source with minimal coloration.
5. Main controls: preset, **Character** amount, **Background** amount, **Voice volume** (existing master). Advanced controls: voice effects enabled, background enabled, warmth, space and texture. Either layer can remain enabled independently under the master toggle.
6. A visible **Compare original / atmosphere** action uses the same admitted preview while playing; it changes local rendering only. Starting a preview retains existing explicit intent and panel ownership. Switching a preset while idle never autoplays.
7. Persist preferences per PC through native settings. Initial/migrated master state is Off, with Digital ready as the default selected preset. Re-enabling restores saved amounts. Keep each preset's adjusted amounts separately; first selecting Human uses its background-off defaults. A failed save is reported; a requested runtime bypass remains applied even if storage is unavailable.
8. Use a dedicated typed partial sound-settings command for these controls so the overlay cannot overwrite unrelated settings from an old whole-settings snapshot. Guard the existing whole-settings save path against stale sound values using a sound-settings revision; update revision even for runtime changes whose persistence fails. Synchronize both windows from native events.

### Proposed sound recipe

These values are starting ranges for later equal-loudness listening, not measured final presets.

| Element | Starting direction | Purpose |
| --- | --- | --- |
| Voice core | Preserve the selected identity and clear center | Recognizable Avesra |
| Warmth | Gentle low shelf around 100-180 Hz, roughly 1-3 dB | Weight without muddying speech |
| Dynamics | Gentle compression around 2:1 with bounded gain compensation | Consistent presence |
| Texture | Low-level parallel, band-limited saturation | Audible synthetic character; avoid harshness |
| Space | Filtered reverb return, roughly 25-40 ms pre-delay | Keep consonants distinct from the space |
| Digital preset (default) | Reverb roughly 8-12% wet; warm body, subtle synthetic texture and dark pad | Distinctive everyday digital identity |
| Digital at higher Character/Space | Reverb up to roughly 12-18% wet, longer decay; stronger texture | Theatrical concert-scale voice within the same preset |
| Human preset | Transparent gentle EQ/dynamics, no synthetic texture, reverb 0-3% wet; background off | Close and natural presentation of the selected voice |
| Background | Dark sustained pad, restrained low pulse, filtered pink/brown noise | A sonic environment |
| Background level | Start roughly 24-30 dB below active voice RMS | Remain behind speech |
| Ducking | Additional 6-10 dB reduction during voiced passages, smooth recovery | Protect intelligibility |

Do not pitch-shift the entire voice down as the default bass control. A faint octave/formant layer is a later audition option with an explicit latency/quality budget. Bass presence also depends on the selected speakers/headphones; added processing cannot make a small speaker reproduce auditorium sub-bass.

Raw white noise is a selectable texture, not the default. Prefer a filtered noise bed and a sparse, harmonically stable pad without vocals, drum fills or attention-grabbing melodies. A small set of well-produced loops is better for identity than generating unrelated music each time.

### Loop and start/stop contract

- Current implementation: prepare the owner's supplied 20-second Galaxy recording off callback, resample to the output rate and crossfade a one-second overlap into a 19-second loop. Bundle the source PCM so Downloads is not a runtime dependency. This replaces the original procedural pad after the owner's audition feedback. No model service or network music stream is needed.
- The loop is its own reusable local audio asset/buffer. A future explicitly generated music asset can replace it after review; do not ship placeholder claims of AI-generated music.
- Bound decoded/prepared assets to 64 MiB total and 30 seconds per loop, stereo maximum. Prepare/resample to the negotiated device rate before installing the renderer. Reject oversized or malformed data; do not decode or load files in callbacks.
- Start its envelope on the first actually submitted voice sample. Fade in over approximately 100 ms, without delaying the voice or playing a pre-roll during slow TTS generation.
- Hold through valid pauses within the admitted utterance. Derive ducking from the local speech envelope, not text timing. Preserve exact speech underrun/error behavior; ambience must never cover a broken transport as if speech succeeded.
- Preserve all source samples, with a 120 ms local lead-in before voice starts. At normal source completion retain up to 200 ms of background/effects, then fade smoothly over up to 1600 ms (1800 ms total), shortened to the admitted deadline budget. Longer reverb settings do not create an unbounded output lifetime.
- Retain a loop phase across nearby valid output jobs only as non-audio position metadata; no background runs between jobs. Clear it on output epoch invalidation. Do not add a session-long ambient mode in this plan.
- Final device submission/drain must fit the admitted fixed deadline. Never extend deadlines per chunk or silently enlarge a remote lease. If a tail cannot fit, taper it within the remaining budget or omit it; speech correctness wins.

## Native implementation requirements

### Real-time boundaries and mixing

Construct all DSP nodes, delay lines, loop buffers, limiter state and channel-reference storage on the media worker before opening playback. Call the library's preallocation method after sample-rate setup. The callback may only perform bounded processing and nonblocking bounded handoff; no file/network/model work, mutex acquisition, dynamic graph edits, buffer allocation or final asset deallocation.

Start with a mono mix milestone that preserves existing channel/reference semantics. Then introduce stereo wet returns/background for one/two-channel devices, keeping dry speech centered. On devices with more channels, retain the existing mono replication behavior and report spatial processing unavailable; do not guess a surround channel layout or send sub-bass to an assumed LFE channel.

Stereo cannot ship with a mono reference presented as the full submitted signal. Represent actual post-format-conversion L/R samples, sample rate, frames and device clock in bounded references; explicitly represent replicated mono for other layouts. Allocate the larger reference storage off callback, and return buffers to a pool off callback. Preserve bounded capacity and fail-closed overflow. The overlay may downmix for display; future echo processing receives the actual channel signals.

Apply bounded input headroom before EQ/parallel summation, keep all DSP values finite, and place master gain and a linked stereo peak limiter at the output. Target sample peaks no higher than -1 dBFS in processed mode; this is not a true-peak or acoustic-loudness guarantee. Use a small bounded limiter delay (target <=5 ms); account for it in both bypass and completion. Match the dry bypass delay during crossfade, then avoid time jumps when processing is disabled. Master zero must emit silence from all lanes.

Dynamic parameter updates are small validated snapshots with a monotonic revision through a bounded real-time-safe control handoff. Coalesce stale slider updates outside callback. Build no new graphs on a slider move. Smooth parameters over 20-50 ms; master Off must take effect even if ordinary update capacity is saturated. No effect settings change audio-device or task authority.

### Completion and telemetry

Keep three distinct observations:

1. **Speech submitted**: final speech sample after DSP latency reaches the device buffer; preserve existing protocol acknowledgement sample totals and timeouts.
2. **Mix submitted**: final atmosphere/effect/limiter tail sample reaches the device buffer.
3. **Estimated device drain**: the current bounded CPAL playback-clock estimate after mix submission. None proves audible delivery.

Introduce separate typed native completion events/status instead of overloading `final_submitted` for incompatible meanings. Retain the existing output lease/worker admission through mix drain or immediate cancellation. A validated transport End stops new source input, not all DSP state. DSP tails never synthesize extra source PCM frames or alter source sequence/totals.

Submitted references include the complete mix throughout the tail. Speech activity and its completion are additional telemetry so the overlay does not claim Avesra is still speaking merely because a pad is fading. Any displayed atmosphere/tail state must come from native output, not a frontend timer. Do not enable echo readiness or barge-in: neither is qualified today. Louder/wider output makes the future full-mix echo reference more important.

## Emotion is a separate workstream

Effects provide character, weight and space. Delivery needs prosody: emphasis, pauses, rhythm and emotional inflection from TTS.

Avesra currently uses a VoiceDesign reference plus Base cloning. Upstream documents instruction control for the 1.7B VoiceDesign/CustomVoice variants; Base cloning is a different interface. Do not add an `emotion` parameter to Avesra's current streaming adapter and claim it works. Do not send stage directions as spoken text.

For this plan, preserve the selected dry identity and use clearly labelled DSP presets. Later investigate a small identity-consistent delivery palette (calm, warm, energized, serious), either qualified reference variants or an instruction-capable synthesis path. A spike must demonstrate speaker consistency, supported incremental streaming, first-audio latency and selection/provenance binding before integration. Effects cannot guarantee emotional delivery and multiple independently designed references may change the speaker.

Suggested reference-design direction for an explicit future audition: "A warm, grounded digital assistant with resonant low-mid presence, precise diction, relaxed confidence and expressive phrasing; a subtle synthetic edge, with clear dry recording and no music, distortion or room reverb." Apply the space and music after synthesis so the off switch remains real.

## Scope and execution sequence

Before changes, inspect `git status --short`, `git worktree list` and `git diff --stat 17a10ea..HEAD -- <paths below>`. This plan was written against uncommitted feature work: a HEAD-only worktree does not reproduce it. Re-read the excerpts and reconcile the actual baseline; never reset, stage or copy unrelated dirty work wholesale. Use `codex/voice-atmosphere` if an isolated branch is appropriate. No commit, push, merge or publication is implied by this design.

Allowed source/doc scope for a later executor:

- `crates/avesra-core/src/state.rs`, `lib.rs`, new `sound.rs` for validated preference types/defaults.
- `crates/avesra-windows/src/audio.rs`, `playback.rs`, `lib.rs`, new `sound.rs`, `Cargo.toml` and only the resulting scoped `Cargo.lock` delta.
- `apps/desktop/src-tauri/src/main.rs`, `media.rs`, `preview.rs`, `speech.rs`, `playback_signal.rs`, new `sound.rs` for native controls.
- `apps/desktop/src/runtime.ts`, `App.svelte`, `Overlay.svelte`, `Icon.svelte`, `VoiceDesigner.svelte`, `SettingsView.svelte`, `playback-signal.ts`, new `VoiceAtmosphere.svelte`.
- `docs/playback.md`, `docs/playback-telemetry.md`, `docs/generated-voices.md`, `docs/normal-speech.md`, `docs/architecture.md`, this plan and its index row.

Out of scope: inference service/model changes, server/protocol payload changes, voice-selection identity changes, capture/ASR/speaker policy, owner readiness, planner/actions, cloud services, music downloads, automatic listening, VST hosting and persistent microphone/speech recording. If changing output acknowledgement timing requires a server/protocol change, stop and design it explicitly rather than bypassing the bound.

1. **Specify output lifecycle and settings.** Update playback specs with separate source/mix/drain semantics, tail deadline and live bypass. Add bounded settings with serde defaults and revisioned native update contract. Match existing native persistence and visible error reporting. Verify: `pwsh -NoProfile -File scripts/verify.ps1 -Suite Static` exits 0.
2. **Implement mono processing and immediate bypass.** Reuse the existing resampler and output gate; move master gain from both ingress paths to one renderer; add bounded EQ/dynamics/texture/space and lifecycle events. Verify: the same Static command exits 0; inspect both gain paths for double application and every cancellation path for DSP reset.
3. **Add prepared looping atmosphere and ducking.** Generate the original bounded loop off callback; add sample-clock start/fade/tail, master-zero behavior and loop seam handling. Keep errors visible and speech transport semantics intact. Verify: Static exits 0; source review confirms no callback allocation/I/O and finite source versus mix completion.
4. **Add stereo and accurate references.** Make channel-specific mixing and native references agree; adapt telemetry consumers and fixed storage accounting together. Verify: Static exits 0; inspect every `PlaybackReference` consumer using `rg -n 'PlaybackReference|final_submitted' crates apps/desktop/src-tauri/src`. No consumer may keep an undocumented mono/full-mix assumption.
5. **Add controls and comparison.** Implement Audio & Voice card, overlay toggle and native-synchronized updates. Existing preview remains explicit and labelled Reference preview. Sound controls can be used during preview; unrelated voice generation/selection operations retain their existing busy restrictions. Verify: Static and `pwsh -NoProfile -File scripts/verify.ps1 -Suite Build` both exit 0. Canonical release remains `target/release/avesra-desktop.exe`.
6. **Record qualification honestly.** Record actual gates and remaining listening/latency work in this plan, without marking DONE based only on compilation. No automated tests or harnesses under the existing owner instruction. Direct listening is a separate user-authorized activity.

The commands above are verified from `scripts/verify.ps1`; they were not run during this planning session. Static runs Rust formatting/Clippy and recursive frontend checks; Build builds bundled frontend assets and the release workspace with the desktop custom-protocol feature.

## Acceptance and evidence

### 2026-09-25 implementation and audition checkpoint

- Added pinned FunDSP 0.23.0 with std only (MIT OR Apache-2.0). Fixed filters, stereo reverb and short echo allocate before stream creation. Prepared original pad is generated at the negotiated device rate, max 24.5 seconds transient / 24 seconds retained; no live music model is used.
- Native renderer now mixes the lanes, uses a linked zero-lookahead sample-peak limiter, retains bounded 600 ms tails and sends source submission independently from mix drain. A 65-buffer return pool retains actual stereo/replicated-mono submitted references without callback allocation/deallocation. Additional local drain wait is bounded to 1900 ms; remote source acknowledgement totals/deadlines are unchanged.
- Local atomic settings updates support Digital/Human, independent saved amounts, both layer switches, filtered/white noise alternatives, live bypass, volume and an overlay shortcut. Sound-only changes do not notify remote mode watchers or change audio/action epochs. Whole-settings writes reject stale sound snapshots.
- Windows Static passed for revision 2: Rust format/locked workspace Clippy and Svelte/TypeScript checks (zero frontend errors/warnings). Revision 2 full bundled release Build passed. No tests or automated audio harnesses were run.
- Live audition used the real paired Spark generated-voice operation and native reference preview on the owner's saved SPDIF endpoint, through the app's WebView bridge without desktop input/focus changes. The sentence was exactly "Hello Eric, how can I help you today?" Original voice selection was retained; new candidates were saved for explicit review.
- Digital revision 1 was rejected: owner heard only voice, no background/effects. Digital revision 2 adds more audible midrange to the pad, raises background/space levels, adds 90/125 ms stereo echo, and uses a newly generated synthetic female voice. Current audition amounts: Character 70, Background 60, Warmth 75, Space 70, Texture 60; master volume 80.
- Revision 2 native observation: two output channels; component peaks (voice, effect difference, background) approximately 0.49685, 0.14272, 0.03630 after master/limiter and before sample-format conversion. Source and mix both reported final submission. These peaks prove nonzero layer output, not equal loudness, perceptual quality, true-peak bounds, heard delivery or approval.
- Native screenshot inspected at `artifacts/ui/voice-atmosphere-digital.png` (ignored): Audio & Voice controls render with Digital selected, independent amounts and comparison buttons.
- Owner heard revision 2 and reported it was much better, but requested more bass, a fuller synth pad, more digitization, reverb and echo.
- Revision 3 retains the exact revision 2 reference candidate (`9b2d49ba-304d-48be-ae85-c0d8a81e20f4:4ceee36b-ecab-4a79-84f1-1ae1102db25e`). Native processing adds parallel 65 Hz ring modulation and amplitude quantization, stronger bass shelving, 2.6-second internal reverb decay (still bounded by the existing 600 ms output tail), and three attenuating stereo echo taps up to 540 ms. The prepared pad adds sub-bass and musical harmonics, with less noise and more gain. Digital initial and audition amounts are now 85/80/90/85/85; master volume remained 80.
- Revision 3 live native preview completed with the exact requested sentence. Component peaks (voice, effect difference, background) were approximately 0.49685, 0.39221, 0.11069 on two output channels; source and mix both reported final submission. These are scalar output observations, not listening approval.
- Revision 3 changed-file formatting, locked workspace Clippy and frontend checks passed. The canonical Static suite stopped at unrelated formatting changes in microphone-check/setup/state work; those files were left untouched. Canonical bundled release Build passed (3m 04s); the live audition used the corresponding optimized-DSP development build.
- Owner then supplied the 20-second WAV/MP3/M4A Galaxy ambience and requested using it beneath the processed voice. Revision 4 replaces the procedural pad with the WAV's unchanged lossless stereo PCM, bundled in `crates/avesra-windows/assets/galaxy-ambience.s16le`. FFmpeg's decoded-source SHA-256 matches the bundled PCM hash recorded in the asset README. Preparation uses the existing band-limited resampler where needed and a one-second overlap to retain a 19-second loop, entered at two seconds for short replies. The UI names the recording Galaxy ambience. Revision 3 voice effects and saved levels remain unchanged.
- Revision 4 Static and canonical bundled release Build passed (2m 36s release). The first development audition timed out before output admission; rebuilding with the existing Rubato dependency optimized alongside the DSP resolved the development preparation delay. Production release already optimizes that dependency. No deadline was extended. The subsequent native preview completed with voice/effect/background peaks approximately 0.49685/0.39221/0.05466, two channels, source and mix finally submitted. No automated tests or audio harnesses were run.
- Owner reported Galaxy needed more level, the processed voice had lost clarity/presence, and iteration was too slow. The owner then requested a powerful vocoder and explicitly looping the new introduction while tuning live.
- Revision 5 retains the clean centered core, adds a ten-band speech-envelope synth vocoder (harmonic saw carrier), separates Echo from Reverb, and adds Voice presence. Galaxy gain range increases by 2.5x; master volume remains independent. Missing new settings fields migrate safely. Live sliders coalesce field edits and update while dragging; Repeat preview reuses the saved take with a 900 ms gap, and Stop repeating finishes the current take. Existing native admission and cancellation apply to every repeat.
- New saved reference candidate `6b2b6693-3cbd-454d-bf9d-08edefb83a0d:07386808-95bd-47da-b60c-f270e7174e93` uses the exact new introduction and the existing synthetic-female description. Generation and initial native preview completed. Explicit repeat was started at the owner's request, and subsequent playback was observed. Live slider delivery incremented the sound revision without changing the active playback epoch. The owner is actively adjusting sliders; these evolving levels are not an approved factory preset. The selected voice identity was not changed by this work.
- Revision 5 Static and canonical bundled release Build passed (3m 06s). Live UI screenshot inspected at `artifacts/ui/voice-live-tuning.png`; loop and sliders are visible/working in the native settings surface. No automated tests or audio harnesses were run.
- The owner liked the live mix with Presence 55, Background 85, Bass 53, Reverb 69, Echo 28, Vocoder 0 and Compression 35, then explicitly requested these as defaults. Those values are now the Digital factory defaults, with Galaxy selected and both layers enabled. This establishes the preferred balance, not final approval of the entire feature or Human.
- Owner rejected the vocoder's excessive gain/intelligibility and requested a pass or replacement. Revision 6 reduces internal vocoder gain from 10 to 3, gives its slider a squared response, and softly bounds the additive return to at most 30% of the recent clear-core peak at maximum amount. It remains off in the approved default mix. Owner listening remains the quality gate.
- Owner requested unclipped starts/ends and background fading 1–2 seconds after speech. Revision 6 adds a 120 ms local lead-in without consuming speech samples, then up to 200 ms hold plus 1600 ms smooth tail fade. Preview packet lifetime reserves four seconds after source duration, capped at the unchanged 32-second packet maximum. Tail still fits the admitted deadline and device-drain reserve; local drain wait is now 3100 ms. Remote source totals/acknowledgements, transport deadlines and cancellation remain unchanged. Lead-in and tail are marked non-speech in references.
- Revision 6 changed-file formatting, workspace Clippy and frontend checks passed. Full Static hit unrelated formatting changes in concurrent voice-analysis work; those changes were left alone. An intermediate compile saw that work's missing `voice_check` initializer; it was resolved in that work before the successful Clippy/build. Optimized-DSP development build and canonical release Build passed (3m 08s release). Saved values matched the owner's defaults, and the same introduction loop was restarted with Vocoder 0. Subsequent loop playback and final-preview submission status were observed; owner feedback on the fade and revised vocoder remains pending.
- Revision 7 replaces the rejected vocoder with Harmonizer (the owner's requested name): an actual speech copy four semitones lower, complementary delay-head windows, mild stereo width and prepared filtering. The centered voice is retained at full gain; the parallel return is mixed at up to 35% and stays out of the reverb/echo inputs. No synth carrier remains. The new screenshot tune becomes the Digital factory default: Presence 85, Background 85, Bass 80, Reverb 80, Echo 25, Harmonizer 0 and Compression 20.
- Owner clarified delay controls mean spacing between echo repeats. Independent left/right controls add 0–400 ms per repeat to the existing 120/180 ms intervals; both default/migrate to zero. A fixed two-second native echo buffer replaces the fixed FunDSP tap graph. The three attenuating repeats remain finite (maximum delay 1740 ms), with no dry-voice delay or feedback. Timing uses a separate atomic mailbox and the existing coalesced live UI path.
- Owner requested preview speech animate the overlay. Native telemetry now reveals the overlay once at the first actual submitted speech frame for every admitted output, including reference tests; lead-in and tail do not assert speaking. Existing preview/reply sample-driven animation and permission gates remain. Live verification observed the overlay visible with `Voice preview` activity and a rendered speaking canvas, while `is_focused` remained false. Screenshot inspected at `artifacts/ui/voice-overlay-preview.png`.
- Revision 7 Static and canonical release Build passed (3m 12s release). The native introduction loop was restarted; defaults/migration were read back, including both zero echo delays. The owner then adjusted Harmonizer and echo spacing live, which was observed in native settings and left untouched. Sound quality remains subject to owner feedback.
- Owner found Harmonizer too subtle even at maximum and supplied new defaults: Presence/Background 85, Bass/Reverb 80, Harmonizer 100, Compression 40, Echo 15, left/right spacing +115 ms. Revision 8 saves these factory values, raises the shifted-voice gain from 0.35 to 1.2 (about 3.4x), and lowers its high-pass cutoff from 90 to 55 Hz. The clear voice remains intact before the shared limiter. Subsequent live spacing edits are preserved independently of the screenshot's factory defaults.
- Owner requested additional reverb beyond the existing 80–100% range. Revision 8 extends only Reverb to 200%, preserving the exact 0–100 mapping and the default of 80. Validation and the atomic gain field support the full range; other percentage controls remain capped at 100. Static and release Build passed. The owner then requested retaining a 0–100% UI; it now displays half the stored reverb amount, so old default 80 becomes displayed 40 without changing sound.
- Digital sound now approved (revision 10 below); pending Human audition and approval later. Overall plan remains incomplete. CPU/latency, longer-loop seam listening and full normal-assistant speech qualification remain unmeasured.

Machine/static completion requires both existing verification suites to exit 0, a reviewed diff confined to scope, every reference consumer updated, both source gain multipliers removed, and documented bounds for all buffers/tails/control messages. Compilation alone cannot establish the following live criteria:

- The same preview can switch original/processed mid-sentence without restarting, losing words, clicking or becoming significantly louder in a comparison.
- Master bypass removes both layers; independent background/effects controls and master-zero work; reopening settings/restarting restores saved state accurately.
- Deafen, stop, pause, lock, disconnect and output-device changes silence all layers immediately through the existing gate, including during reverb drain. Mic-only changes preserve output.
- A loop runs through multiple seam crossings without a click, gap or obvious jump; short replies do not have an excessive intro/outro.
- On stereo headphones, voice remains centered and ambience is wide; mono stays intelligible; unsupported layouts honestly expose their mono fallback.
- Queue errors and underruns remain failures, with no orphaned music. A new utterance cannot inherit old reverb content or stale controls from a replaced output job.
- Measure incremental DSP first-audio delay (design target <=10 ms, separate from the explicit 120 ms voice lead-in), callback execution against the actual device callback period with headroom, peak levels and audible intelligibility. These are targets awaiting measurement, not current performance claims.
- Verify the preset selector contains exactly Digital and Human, defaults to Digital, restores their independent adjustments, and initializes Human with background off. Evaluate routine short answers and sustained speech at matched loudness in both presets, including higher-intensity Digital. The user's preference is the final sound-design decision.

If direct listening is not authorized/available, report implementation as PARTIAL with static/build evidence and the exact unrun items. Keep normal assistant-readiness claims separate.

## Stop conditions and maintenance

Stop the affected step if the dirty baseline cannot be reproduced safely, pinned DSP nodes allocate in processing, a tail requires extending a remote deadline, stereo cannot preserve submitted references, or live bypass requires canceling speech. Resolve the contract before continuing. Do not silently drop those requirements to deliver a toggle.

Future AEC, speech segmentation, a different output backend, reference-preview changes and new TTS delivery modes all interact with this work. Review source completion versus mix completion, master gain placement, callback work, deadline ownership and bypass latency whenever any changes. Continuous ambient listening-room music, generated music tooling, pitch/formant doubling and per-utterance emotional TTS are deferred follow-ups.

## Primary technical references

- [Qwen3-TTS model capabilities and Voice Design then Clone workflow](https://github.com/QwenLM/Qwen3-TTS): reference design plus clone prompt is supported; instruction control differs by model variant. Avesra's pinned adapter remains the runtime authority.
- [FunDSP](https://github.com/SamiPerttu/fundsp): DSP candidate with explicit preallocation, filters, dynamics and reverberation. A dependency choice still requires a pinned build and callback review.
- [CPAL 0.17.3](https://docs.rs/cpal/0.17.3/cpal/): existing native output callback/device interface.

## Alternatives considered

- Second network music stream: unnecessary transport/clock/failure coupling for a reusable local loop.
- WebView music alongside native speech: splits output selection, cancellation and submitted references between owners.
- Generate music for every answer: adds variable startup delay and compute contention; cannot provide a stable immediately interruptible sound identity.
- Bake effects/music into the generated reference: weakens independent mixing/bypass and may contaminate cloned delivery.
- Turn up bass and reverb globally: does not solve expression, clarity or small-speaker limitations.

### Revision 9: adjustable harmony depth and moving overlay

- Owner requested a shallower Harmonizer and reported that the overlay looked static. Added a separate 0–12-semitone downward depth slider, in half-semitone steps, default/migration 2 semitones lower. Existing gain/ambience/echo settings remain intact. Amount controls strength; depth controls interval.
- Depth is validated as 0–24 half-semitone units in persisted settings and `update_sound`, published in unused timing-mailbox bits, and smoothly applied by the shared native mixer for preview and reply. Human ignores the shifted layer. No callback allocation or graph rebuild was introduced.
- Signal's WebGL callback is now reactive, with explicit frame dependency even before initialization. Fresh speech redraws on browser animation frames; each frame retains its original calibrated expiry. Reduced motion, hidden pages, clear/retirement and WebGL fallback are preserved. Canvas backing storage resizes only when dimensions change.
- Native display reduction now measures stereo RMS energy rather than signed averaging that cancelled the visible signal. Actual submitted stereo references and audio output are unchanged.
- Governing contracts: docs/playback.md and docs/playback-telemetry.md. Entry points reviewed: persisted SoundAmounts migration/validation, save_settings, update_sound, UI coalesced sliders, SoundControl/Mixer, native Telemetry::sample, PlaybackSignal expiry and Signal render. Shared preview/reply admission and cancellation are unchanged. Per the owner's no-tests override, verification uses static/build checks, source review and authorized native audition instead of automated tests.
- Static passed; subsequent frontend expiry/canvas refinements passed pnpm check (zero errors/warnings). Debug audition read back migrated depth 4 half-semitones and resumed the exact saved long introduction at Harmonizer 100%. Successive live overlay screenshots (artifacts/ui/voice-overlay-motion-a.png and -b.png) show distinct waveforms; shader phase advanced 118.48 to 130.74 in speaking mode. Native overlay visible=true, focused=false. This verifies changing rendered output, not perceived sound approval.
- At this checkpoint Digital remained under audition and Human was deferred; later owner approval is recorded below. Revision-9 release Build passed (3m 13s).
### Revision 10: Digital sound approved; wider pitch and motion

- Owner confirmed the animation works but requested more movement and a wider Harmonizer range. Pitch now spans -12..+12 semitones, retaining signed downward half-semitones in the existing persisted field. Negative phase increments wrap correctly for upward shifting. Positive saved depths preserve their exact sound. The speaking shader uses a bounded energy display curve for more expressive movement; audio output is unaffected.
- Owner explicitly approved the screenshot tune: "I love the way this sounds!" Digital factory defaults are now Presence 85, Background 85, Bass 100, displayed Reverb 50 (native 100), Echo 10, Harmonizer 75, Compression 40, Harmony pitch -1 semitone, Galaxy ambience, echo spacing +105/+65 ms. Both layers enabled. Missing pitch migrates to -1 semitone; current saved tuning already matches and is preserved.
- Approved saved reference: 6b2b6693-3cbd-454d-bf9d-08edefb83a0d:07386808-95bd-47da-b60c-f270e7174e93, exact long introduction. Voice selection was not changed. Digital sound approval does not imply Human approval or full assistant qualification.
- Final Static passed (zero Svelte errors/warnings). Native UI roundtrip verified upward pitch +1 semitone stores depth -2 and the slider exposes -12..+12 in 0.5 steps. Approved pitch was restored through the UI, after which the owner resumed the loop and continued changing pitch; those later live values were left untouched. Larger actual speaking ribbon inspected at artifacts/ui/voice-overlay-expanded.png. Factory defaults remain the approved screenshot tune. Revision-10 release Build passed (3m 01s); final half-semitone factory-default Static and release Build also passed (3m 15s).
- Final owner screenshot supersedes the earlier -1-semitone tune: Harmony pitch is now **0.5 semitones lower** (native depth 1). All other approved defaults stay unchanged. New/missing depth and factory Digital settings adopt that exact value. Digital is approved; Human remains deferred.
