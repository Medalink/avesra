# Plan 003: Draw each person's voice as a unique, reproducible avatar

## Status and intent

- Status: IN PROGRESS (implementation explicitly authorized by the owner, 2026-09-26). Draft PR14 now includes the protected native foundation and mock-aligned Your voice renderer (5b43c37). Source/static review passed; combined production build and actual rendering proof are pending. Portrait capture and full acceptance remain outstanding. This is not a completion claim.
- Priority: P2 product identity. Effort: M, staged. Risk: low to medium; it touches protected speaker storage and enrollment capture, not action authority.
- Depends on: Plan 001's protected owner and enrollment flow (`docs/owner-identity.md`, `docs/enrollment.md`) and the People & Voice ID screen in the design mockups.
- Owner request: generate a unique-to-the-user "avatar" graph and animation of their voice, built from their real voice. The same user must get the same avatar every time. Suggested starting point: have them say about 20 words.

The avatar is a portrait, not a password. It makes the owner's voice identity visible and personal. It never authenticates anyone, grants anything or replaces the speaker check.

## Owner delivery requirements (2026-09-26)

The owner requires complete functional implementation, exact matching to the authoritative design mockups in the real app, actual evidence, and PRs as work proceeds. Compare matching states at the same scale. Simulated mock data must be replaced with actual native state, never copied as success evidence. Preserve the simple Personal conversation startup: optional portrait creation must not become a new prerequisite to talking. Instrument portrait capture/extraction/render preparation with bounded content-free timings, including failures/cancellation; biometric inputs and render parameters are excluded from metrics. Live capture and repeatability evidence remain unrun.


### Current foundation and precision of guarantees

Draft [PR14](https://github.com/Medalink/avesra/pull/14) implements the native
foundation and the mock-aligned People card. Its governing implementation contract is `docs/voice-avatar.md`
on `codex/voice-avatar`. Actual owner-bound PersonalVoice is required to derive a
portrait; an unbound candidate alone is insufficient. Candidate2 and Personal1
remain unchanged; a separate protected avatar vault supports either real source.
All v1 petals and tempo are absent, not estimated or completed.

The original design language below overstates several mathematical and rendering
guarantees. Stable stored integer parameters on this installation are the precise
reproducibility contract; a versioned renderer must produce stable geometry, but
cross-machine raster pixels cannot be guaranteed across graphics stacks. Registry
checks guarantee distinct retained ring values, not global or perceptual uniqueness.
Lossy keyed projection is not a proved biometric inversion or unlinkability defence.
These limits must remain explicit in product and evidence claims; they cannot be
converted into passing acceptance results by relabeling the foundation as complete.
Personal learning must not redraw an existing portrait. Source changes require an
explicit future redraw; changed owner revision fails closed.

### Current mock authority and remaining portrait work

The owner's latest instruction makes the checked-in mock appearance authoritative.
The current renderer uses 192 display points interpolated from 24 lossy native
shape values, matching the mock's guide circles and spokes. These are not 192
independently retained biometric features. It does not add a visible twenty-petal
layer or signature ring absent from the mock. The native ring remains stored;
the original visual-layer proposal below is not authority to alter the mock.
Personal sources and exact candidate-bound phrase summaries remain distinct.
Unavailable live scores, check results and dates must not be fabricated.

Optional two-batch capture work is parked, unfinished and excluded from the
current integration build. Its intended next native slice uses actual received-sample prompt
progress, original management lifetime, selected-device ownership and retained
worker retirement. Current ASR supplies no word timestamps. Energy islands in
800ms prompt slots may provide bounded acoustic features, not lexical verification;
ambiguous slots remain missing. Version2 typed petals will carry actual edge and
energy geometry while retaining the v1 reader and stable ring. Prompt pace is not
measured syllable rate. No portrait or verification requirement may be added to
ordinary Personal conversation.
## The core problem: unique and reproducible pull in opposite directions

- **Live audio is never the same twice.** Microphone, room, distance, mood and illness all change the waveform. Hashing a new recording gives a different avatar every time. Drawing straight from live features gives an avatar that drifts from day to day.
- **Voice embeddings are stable but fuzzy.** Two sessions of the same person land close together in the ECAPA space, not on the same point. That's good for matching and bad for an exact picture.

**Decision: draw from the stored profile, never from a fresh recording.** The avatar is a pure, versioned function of data Avesra already keeps in the protected speaker record (plus a few new derived features, below). The same record, algorithm version and install key always give the same avatar, down to the pixel. Uniqueness comes from three layers with different jobs.

| Layer | Built from | Job | Changes when |
| --- | --- | --- | --- |
| **Shape** | The 192-value ECAPA profile (normalized mean of the profile phrases) | Looks like *this voice*. Similar voices look related; re-recording gives a close cousin, not a stranger | The person re-enrolls (new revision) |
| **Petals** | 20 spoken words from a voice-portrait capture | The personal, legible detail: one petal per word, shaped by how they said it | The person redraws their portrait |
| **Signature ring** | HMAC of the owner or person's stable actor UUID | Guarantees no two people on this install ever share an avatar | Never (stable across re-enrollment) |

- **Reproducible:** every input is stored and every step is deterministic, so the avatar is identical across restarts, machines and app updates within one algorithm version.
- **Unique on this install, guaranteed:** the signature ring is derived from the actor UUID. Avesra checks it against every other enrolled person's ring when saving, and bumps a counter on the astronomically unlikely collision. Two people can't share one.
- **Unique elsewhere, overwhelmingly likely but not guaranteed:** 64 ring bits, plus a keyed 24-dimension shape and 20 petals, make a match between two installs vanishingly unlikely. A guarantee across unrelated installs would need a central registry, which this plan rejects (privacy).
- **Recognizably the same person across re-enrollment:** the ring stays the same and the shape moves only slightly, so a re-enrolled owner still looks like themselves.

## The 20-word voice portrait

The six enrollment phrases already make the identity profile. The portrait adds a short, deliberate ritual that gives the avatar its detail. It's the moment the user watches their voice draw itself.

**Capture**
- Twenty short words, shown one at a time at a steady pace (about 0.8 s each), in two recordings of ten. Each recording fits the existing 8-second capture and 12-second native deadline, so no capture rules change.
- The words are chosen for phonetic coverage: every English vowel family, the fricatives (s, z, sh, zh, f, v, th, h), the nasals (m, n, ng), the plosives (p, b, t, d, k, g), the affricates (ch, j), the liquids and the glides. Draft list, to be checked for coverage before shipping: *sweet, river, jazz, father, thought, ocean, loop, measure, church, morning, pepper, garden, yellow, whistle, bright, shadow, violin, cushion, zebra, kind*.
- Word boundaries come from the ASR lane's word timestamps when the recognizer returns them. Otherwise an energy-based splitter with the known word count is used. A word that can't be isolated cleanly is marked missing, and its petal is drawn as an empty slot rather than guessed.
- As each word is heard, its petal grows onto the avatar live. By the twentieth word the user has watched their portrait assemble.

**Features per word** (computed natively on the PC from the transient PCM, which is then discarded, as enrollment audio is today)
- Pitch: median F0 and range (YIN or equivalent), in semitones relative to the person's own median.
- Spectral envelope: 16-band log-mel mean, normalized per person.
- Brightness: spectral centroid. Breathiness: harmonic-to-noise ratio.
- Timing: voiced duration and an energy contour summarized by 4 coefficients.

Around 25 numbers per word, 20 words, a few kilobytes. These are derived features like the embeddings, stored in the same DPAPI-protected candidate record (a schema bump). No audio is kept.

## Building the avatar (native)

A Rust `voice_avatar` module computes render parameters from the protected record. The webview never sees vectors or features, only the finished parameters.

1. **Install key:** 32 random bytes generated once per install, stored with user-scoped DPAPI beside the owner store. All keyed steps derive sub-keys from it with HKDF (`avatar/v1/shape`, `avatar/v1/ring`).
2. **Shape:** project the 192-value profile through a keyed random rotation (seeded from the shape sub-key) down to 24 values. Rank-normalize them against a fixed reference distribution and quantize to 6 bits each. Keying means the same voice draws differently on different installs, so avatars can't be used to link a person across machines. The projection also loses information by design, so it can't be inverted back into a usable embedding.
3. **Petals:** for each of the 20 words:
   - length from voiced duration;
   - curvature from the pitch contour slope;
   - edge detail from the 16-band envelope (downsampled to 8 control points);
   - width from brightness;
   - fill density from harmonic-to-noise ratio.

   All values are quantized.
4. **Signature ring:** the first 64 bits of `HMAC-SHA256(ring sub-key, actor UUID || counter)`, drawn as 64 ticks, long or short, around the outer ring, plus a rotation offset for the whole avatar. The counter starts at 0 and only increases to resolve a collision with another enrolled person.
5. **Rhythm:** the person's median syllable rate becomes the avatar's idle breathing tempo, so it breathes at the pace they talk.
6. **Output:** a bounded, versioned parameter object (`{version: 1, shape: [24], petals: [20 × 6], ring: 64 bits, rotation, tempo}`), with a digest for display ("avatar 7F3A-91C2"). It's returned in the People & Voice ID Settings response alongside the existing candidate summaries.

`version` pins the whole algorithm: projection seed derivation, reference distribution, quantization steps, renderer geometry. A future v2 can change the look, but a v1 record always renders the same way.

## Drawing it (frontend)

- **`VoiceAvatar.svelte`:** a pure renderer from parameters to SVG. No randomness, fixed numeric precision and integer geometry, so the same parameters always give the same markup.
  - The centre core is drawn from the 24 shape values as a smooth closed contour (the voiceprint shape in the current mockups).
  - The 20 petals go around it in word order, starting at the ring's rotation offset.
  - The signature ticks form the outer ring.
- **Colour follows the existing rule:** owner avatars in blue (#3A5DD8, with lighter and darker blues for depth), other enrolled people in green (#8EDE4A). Unknown voices never get an avatar, because they're never enrolled. Ruby stays reserved for Avesra.
- **Motion:**
  - Idle, the avatar breathes at the person's tempo.
  - While that person is speaking and matched, the live signal's band energies ripple through the petals, and the core pulses with loudness, reusing the overlay's 420 ms centre-out transitions.
  - If someone else talks, the avatar stays still.
  - Reduced motion shows the still portrait.
- **Where it appears:**
  - People & Voice ID → Your voice, replacing the generic voiceprint glyph.
  - Each enrolled person's card, in green.
  - The end of voice setup in onboarding, as a reveal ("This is your voice").
  - Optionally, beside "You" in the expanded overlay's conversation.

  The compact overlay keeps the voice bars, where colour alone carries who is speaking.

## Privacy and security

- The avatar is derived from biometric data and is treated as sensitive. It's never uploaded, never included in diagnostics or exports, and is removed with the voice profile.
- Vectors and portrait features stay native, as they do today. Only the lossy, keyed parameters cross into the webview.
- The avatar has no authority. A voice match still comes only from the speaker check against the stored profile. Nothing compares avatars to decide who is speaking.
- Keying with the install secret prevents cross-install linking. Deleting the install key (a full reset) redraws every avatar, which is the intended behaviour.
- The portrait capture follows every enrollment rule: explicit start, local verification where required, cancel on lock, close or device change, and memory-only PCM.

## Scope and sequence

1. **Spec:** add the portrait capture, feature list, parameter schema and version rules to `docs/enrollment.md` and `docs/owner-identity.md`, and add the Settings response shape.
2. **Native features and storage:** add portrait feature extraction to the enrollment media worker, persist the features in the candidate record (schema bump, with migration that leaves existing candidates portrait-less), and generate the install key.
3. **Avatar builder:** add the `voice_avatar` module (projection, quantization, petals, ring, collision check) and return the parameters in the Settings response.
4. **Renderer and UI:**
   - `VoiceAvatar.svelte`.
   - The People & Voice ID card.
   - The portrait capture screen, where petals grow as each word is heard.
   - A "Redraw my portrait" action (new revision; the old one is removed after confirmation).
5. **Onboarding reveal and other people:** show the avatar at the end of voice setup, and draw green avatars for enrolled people.

**Verification** follows the owner's rules: static checks and build, plus live inspection on the owner's PC, with no test harness.
- The same profile should show the same digest across restarts.
- Re-enrolling should keep the ring and move the shape only slightly.
- A second enrolled person should always look different.

**Until the portrait exists:** an existing profile without portrait features still gets an avatar, from the shape core and signature ring, with empty petal slots and an invitation to "Draw your voice".

## Alternatives considered

- **Hash the raw audio:** unique, but never reproducible. Rejected.
- **Draw live features only:** personal, but it drifts every session. Rejected as the base; kept as the live animation layer.
- **Unkeyed projection of the embedding:** reproducible everywhere, but the same person would look identical on every install, which enables linking. Rejected in favour of a per-install key.
- **Random seed per user:** unique and reproducible, but not *their voice*. Kept only for the signature ring, where a guarantee is the point.
- **A central registry for global uniqueness:** would guarantee uniqueness everywhere, but requires uploading biometric-derived data. Rejected.

## Open questions for the owner

1. Is an optional second portrait language, or free speech instead of fixed words, worth it later? Fixed words keep petals comparable and reproducible.
2. Should the owner be able to export the avatar as an image (for a profile picture)? It would still be keyed per install, but it would leave the app.
3. Show the avatar's short digest ("avatar 7F3A-91C2") in the UI, or keep it internal?
