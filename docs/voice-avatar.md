# Native voice avatar, versions 1 and 2

An avatar is optional sensitive presentation, never identity, permission, speaker
matching or a prerequisite to Personal conversation. Missing portrait capture,
missing avatar storage or an unavailable avatar cannot disable talking. The
native source is the current owner's actual protected Personal voice. Its
actor/revision is checked against the actual Windows owner. When that record
references a candidate, the exact candidate must still be the selected (or
unambiguous sole) startup source, and its seed, endpoint and model must match.
A naked candidate is not an actor binding: if no actual protected Personal record
exists, the avatar is unavailable until normal startup establishes that binding
or a future explicit verified avatar-only association is implemented. Independently
learned Personal observations need no candidate. No webview
vector, actor, score or fabricated six-phrase candidate is accepted.

## Immutable source and public parameters

The first available real representation is frozen into one actor-bound avatar
record. Candidate ID/revision and a SHA-256 source digest, or Personal voice ID
and its actual observation-snapshot digest, identify that source privately.
Personal learning after this point does not redraw the avatar. A changed selected
source is reported explicitly; it is not silently substituted. The explicit
prepare/confirm redraw operation below can replace that same actor's immutable
record, keeping its ring. Owner revision changes fail closed on ordinary reads;
redraw requires the actual current owner-bound source and a fresh management proof.

The webview receives versioned presentation parameters: 24 six-bit shape values, 20 optional petal
slots, a 16-digit lowercase hexadecimal ring, integer rotation in 1/256 turns,
optional quantized tempo, and a display digest. Version 1 has no measured
word features or speaking tempo: every petal and tempo is null. It must not
render fake measured petals, claim a completed portrait or invent syllable rate.
Parameters and their digest never enter telemetry, logs, exports or diagnostics.

Persisted integer parameters are the restart-stable rendering input; numerical
recomputation is not required on read. Renderer version and geometry remain a
separate contract. Identical parameters do not promise identical raster pixels
across graphics stacks. Different installation keys deliberately produce different
parameters. Lossy projection is not a proved biometric-inversion defence or a
guarantee against linking publicly shared pictures.

## Key, deterministic projection and ring

One private vault holds a CSPRNG-generated 32-byte install key, actor ring roster
and avatar records. HKDF-SHA256 uses salt `avesra/avatar/v1` and distinct UTF-8
info labels `avatar/v1/shape` and `avatar/v1/ring`, each yielding 32 bytes.
HMAC-SHA256(shape-key, big-endian row-u32 || dimension-u32) supplies each initial
matrix coefficient: its first bit chooses -1 or +1. Ordered modified Gram-Schmidt
produces 24 normalized rows of length 192. Degenerate/nonfinite vectors fail.
The normalized actual speaker vector is projected onto each row and multiplied
by sqrt(192). The fixed reference CDF has knots -3,-2.5,...,3, with Q16 values
88,407,1491,4378,10398,20218,32768,45317,55137,61157,64044,65128,65447.
Linear interpolation, clamping to [0,65535] and floor(value*64/65536) yield 0..63.
These fixed reference values are visual normalization, not a measured speaker
population or a probability. Exact integers are persisted after construction.

Ring bytes are the first eight bytes of HMAC-SHA256(ring-key,
actor UUID's canonical 16 bytes || collision-counter-u32 big-endian). Remaining
byte eight determines rotation. A serialized registry checks all retained actor
rings before publication, trying counters 0..1023. At most 128 actors are stored.
Exhaustion is an explicit error. Distinct retained 64-bit ring values on this
install are guaranteed by the registry; global uniqueness, perceptual distinction
or pixel uniqueness is not claimed. The ring registry survives profile deletion
so re-enrollment cannot reassign another actor's signature.
At most 32 source-bearing avatar records coexist within the 128-entry ring roster;
capacity exhaustion does not evict somebody else's sensitive record silently.

## Protected publication and deletion

The vault is user-scoped DPAPI under `voice-avatars`, with cleartext bounded to
65536 bytes and protected reads to 131072 bytes. A native exclusive file lock
serializes every read/modify/publication. A create-only initialization marker is
synced before the first vault publication. Once either exists, missing, corrupt,
foreign-user or unsupported vault/key state is an error, never automatic key
replacement. An interrupted first initialization needs explicit recovery rather
than silently redrawing identities. Full intentional reset is not implemented by
this foundation. Neither key nor biometric source is sent to another machine.

Publication stages and syncs a private protected file, then rechecks original
Settings lifetime/deadline, current owner/revision and source before atomic rename.
The actual blocking owner retains the management admission through retirement
even if its caller disappears; caller disappearance withdraws publication.
Profile deletion first removes the source-bound avatar record under this same
vault lock, preserving only actor/ring/counter metadata, then deletes the profile.
Failure is explicit and may leave the avatar removed while the profile remains;
there is no claim of cross-file atomicity. Existing source deletion cannot leave
a displayable avatar whose source is missing. No source audio is retained.
There is currently no Personal-record clearing or owner-reset product entry point.
Any future such operation must remove its associated avatar records before source
removal; an owner revision mismatch already blocks reading prior parameters.

## Entry points and foundation boundary

| Entry point | Behaviour |
| --- | --- |
| `speaker_candidates` | existing visible Settings reader; native current-owner avatar derivation/read alongside summaries; no frontend source selection |
| candidate selection/clear | existing protected management, no avatar authority; changed source is unavailable until explicit redraw |
| candidate deletion | protected source-bound avatar removal before existing candidate deletion |
| Personal learning/startup | unchanged; no new key, portrait, capture or enrollment prerequisite |
| avatar storage and derivation | bounded native-only keyed inputs, integer public parameters, retained owner and original deadline |

Portrait capture/DSP, other-person management, live matching animation
and runtime repeatability evidence remain incomplete; empty slots are honest
absence, not completion. Verification follows the owner's no-tests override:
source review, static checks/builds when released, then separately authorized real
app observations. No test fixtures, imported evidence or generated voice identity.

## Native portrait timing contract

The existing bounded app-timing writer observes actual native work, independently
of the `speaker_candidates` frontend invoke round trip. The finite portrait
operation names use the existing `work` stage:

| Operation | Actual measured boundary |
| --- | --- |
| `redraw_prepare` | Native `prepare_redraw` helper through proposal construction; excludes command admission, ticket installation and caller receipt |
| `redraw_confirm` | Native `confirm_redraw` helper through replacement/cleanup; excludes later command context checks and caller receipt |
| `prepare` | Entire retained `voice_avatar::current` call, including source/vault locking and preparation; not the Settings IPC or worker retirement |
| `source_load` | Native current-owner/Personal/candidate source resolution |
| `vault_load` | Protected vault/key read, decode and validation, including a legitimate absent vault |
| `derive` | Actual keyed projection and integer parameter construction in `shape::build`; absent on stored-record reads |
| `publish` | Protected encoding, staging, sync, final authorization and atomic publication, including pending-file cleanup |
| `remove` | Source-bound avatar cleanup during candidate deletion; not subsequent candidate/selection deletion |

Each preparation, removal or redraw helper call creates a fresh random timing
operation UUID, unrelated to its actor, profile, candidate, source, display digest or ring. Nested
phases share only that timing UUID. Existing authorization-check rejection and a
publication-time owner mismatch produce `withdrawn`; redraw confirmation also
marks missing/changed expected source, registry actor or previous record as
withdrawn. The locked finalization callback distinguishes context withdrawal from
publication failure with an internal typed result, preserving the original error
and authority behavior. Other returned errors produce `failed`. A returned successful phase produces `complete`; a dropped unfinished
span produces `abandoned`. An ordinary read with no source/vault or an
already-absent cleanup is a successful lookup/no-op, not proof of a populated
avatar or deleted profile. Redraw preparation requires a source/vault, so absence
fails; confirmation treats a missing expected source/registry actor as withdrawn.
No new authorization checks, retries or result changes are introduced by timing.

A phase that completed before later withdrawal keeps its actual outcome. In
particular, completed publication followed by withdrawal is not relabelled as a
rollback. The enclosing preparation/removal span records its own result. Nested
elapsed durations overlap and must not be added. The spans remain owned by the
actual synchronous blocking call if its IPC caller disappears; they neither
prove caller receipt nor worker retirement. Process abort may lose observations.

No error text, source identity, parameters, digest, ring, vector, word, feature,
PCM, path or other biometric detail enters observations. Existing bounded
nonblocking observation, loss counters and retention apply. Telemetry schema 8
is the forward-only compatibility marker for the new finite portrait operations;
product Store schema 30 and accepted trace query version 3 remain unchanged.
Older telemetry writers must not reopen the newer store. Installed migration and
retained timing/export proof remain unrun.

Capture/extraction remain unimplemented. Redraw helper phases are covered below.
Frontend SVG preparation, GPU presentation, owner qualification and end-to-end latency are not
measured by these native phases. Timing does not enable capture, create authority,
make the portrait required for conversation or establish Plan 003 completion.

## People card presentation and source disclosure

The authoritative Settings mock supplies the 148px card avatar, 240-unit viewbox,
44/108-unit guide circles, 192 contour points and 96 radial lines. The renderer
periodically interpolates the native 24 quantized shape values; those display
points are not 192 measured features. No synthetic shape appears when unavailable.
The stored ring and optional portrait fields are not rendered as new geometry.

The nested avatar response has explicit version 1 and an optional candidate
reference (ID/revision) taken only from the validated immutable native source.
Missing/unsupported response versions are unavailable, never inferred from a
selected or first candidate. The six phrase markers and source metadata require
that reference to match an actual six-segment summary. Independent Personal
learning has no phrase markers. Neither candidate references nor parameters enter
metrics. This optional presentation response does not change the vault format.

The card shows current owner display name only for a matching actual owner actor.
It never fabricates enrollment dates, similarity, held-out success, live voice
matching, tempo or waveform activity. Signal space rests empty and match is
unavailable until a genuine correlated observation exists. Test my voice opens
existing explicit protected diagnostics; Re-enroll opens optional advanced tools.
Neither button starts capture. Existing owner/registration, recording, selection,
delete and audio controls retain their original authorization. Lock, hidden page,
owner/action/device context changes clear avatar data and reject old read results.
The speaker_candidates response, strict frontend decoder, renderer and Enrollment
host are the affected entry points; normal startup/capture and Settings shell are
unchanged. Static review/checks follow the owner's no-tests instruction. Visual
runtime comparison remains separate evidence, not a claim of this source change.

The native settings-hidden event clears cached source/owner data immediately;
reads resume only after ordinary visibility/focus. A capture-only epoch transition
clears avatar presentation but retains the actual diagnostic component/source
through its own recording. Owner/action/device/lock/hide transitions still retire
that diagnostic. Owner-name editing remains inside advanced tools.

## Explicit same-actor redraw

Redraw is an optional visual-only operation. It never selects a speaker candidate,
changes Personal admission, opens audio, or advances audio/action epochs. It uses
the current actual owner-bound Personal source, including a changed selected
candidate only when that Personal record really binds it. An existing validated
same-actor ring/key registry is required; missing keys are never regenerated.
The version-one 24-value shape and approved 192-point display stay unchanged as
formats. No petals, tempo, live match, or lexical evidence is introduced.

`prepare_voice_avatar_redraw()` consumes the existing Windows Hello management
proof in visible Settings. It returns `{version:1,ticket,remaining_ms,preview}`;
preview is an ordinary strictly decoded VoiceAvatar but is pending, not saved.
One native ticket retains the original proof, owner/control/session context,
exact source digest, previous actor-record digest and actual proposed parameters.
Confirmation expires at the earlier of the original 60-second proof and 30 seconds
from preparation start. Each file operation also has its own original 12-second
budget. Neither a response, confirmation nor retry renews these clocks.

`confirm_voice_avatar_redraw({ticket})` consumes that ticket once. The actual
blocking owner retains owner-management admission through source reread, protected
staging and publication, even after caller withdrawal. It requires the identical
current owner/source and previous record, holds candidate/avatar file locks, then
rechecks current native proof/context under Runtime.local across the atomic rename.
The actor ring/counter/rotation are preserved. Learning or source changes between
preparation and confirmation refuse the stale proposal; prepare again explicitly.
The protected replacement uses the existing bounded vault and temporary-file
sync rules. A lost reply after rename is an uncertain presentation outcome: read
actual saved state before proposing again, never blindly repeat confirmation.

`cancel_voice_avatar_redraw({ticket})` withdraws only that Settings ticket. Hide,
lock and management/context invalidation also withdraw it; expired or abandoned
tickets cannot block a later fresh preparation. No timer is a grant, and no
frontend actor/vector/source/parameter input is accepted. Preparation publishes
only while the original caller is still current. Confirmation never recreates a
removed actor registry or changes another actor's record.

Affected entry points are these three commands, Setup invalidation, and the native
avatar prepare/replace helpers. Existing avatar reading/initial creation, source
selection/deletion and Personal startup remain unchanged. Optional acoustic
capture and additional-person management remain outside this slice. The shared
content-free observer records distinct `redraw_prepare` and `redraw_confirm`
roots. Preparation measures source/vault loading and actual derivation;
confirmation measures exact expected source/vault validation and publication,
without pretending to derive the retained proposal again. The two explicit calls
have independent fresh timing UUIDs; ticket/source identity never links them.
Success is local helper completion, not proof of ticket installation, later
command acceptance or UI receipt. Early command admission/cancellation and work
outside these helpers remain outside native phase coverage (frontend round trips
remain separate). Parameters, digests, tickets, source IDs and error details never
enter observations. These finite roots join unreleased telemetry schema 8; no
additional schema bump or accepted-query change is introduced.

Manual proof remains unrun: with a genuine saved source and explicit owner Hello,
inspect pending preview, cancel without publication, save, and reopen/restart to
compare saved integer parameters. Also withdraw via hide/lock and change source
between prepare/confirm; stale publication must fail. This source work does not
access private stores or run capture, automated tests, fixtures or harnesses.

## Optional acoustic prompt observation

This workflow observes the already running native Personal producer. It never
opens another device, drains the media queue, pauses normal input, suppresses
actions/replies, or changes controls, admission or audio epochs. If normal
Personal input is unavailable, the optional operation is unavailable too.

`begin_voice_portrait()` consumes an existing visible-Settings management proof
and snapshots the actual owner-bound immutable avatar source. Its original Hello
expiry (at most 60 seconds) covers both batches and save; no response or batch
renews it. `record_voice_portrait({session,request})` explicitly observes one of
two eight-second batches. A caller request UUID correlates progress only, never
authority. One batch must actually retire before the next can start. Save accepts
two completed batches with at least one measured slot; all missing is not a
completed portrait. Cancel, hide, lock, source/control/session change, caller loss
and expiry withdraw only this observer.

The sole Personal producer copies at most one actual 320-sample frame into a
bounded eight-frame channel after its existing sequence/clock/reference checks.
Its tap uses only try-lock/try-send, never awaits, performs DSP/file work or emits
UI events. Contention/overflow ends the observer without changing the producer's
result. A batch retains its original first capture lease, ordered sequence and
hardware-derived timestamps through exactly 400 frames; it never joins a new
producer lease or substitutes delayed, padded or replayed samples. No owner
management lock is held during observation/DSP, so normal learning can continue.

Each 40-frame/800-ms slot is acoustic prompt evidence, not verified word or
speaker recognition. Progress follows actual received samples. Any frame with
assistant-output overlap/tail or unknown reference coverage makes that slot
missing; no residual is asserted to be clean voice. Only eligible original
microphone samples enter local DSP. Ambiguous energy islands, clipping and absent
pitch also yield missing slots. Normal conversations/actions remain active during
the prompts; resulting assistant speech can make slots unavailable.

DSP uses the documented bounded local energy/pitch/spectrum measurements. Duration
means energy-island duration, not total voiced duration. Harmonicity is a
pitch-period correlation estimate; pitch slope and relative mel-band contrast
form bounded visual values. There is no lexical verification, measured speaking
tempo, release qualification or person-relative pitch-normalization claim.
PCM queues, slot buffers and floating-point working vectors are zeroized on drop.
Raw samples never enter storage, frontend, logs or telemetry. The actual consumer
owns channel/DSP cleanup through retirement even if an IPC caller disappears.

The protected vault accepts v1 and v2; v2 records may contain twenty optional
native feature records and matching bounded typed parameter slots. v1 records
remain readable without recomputation. `save_voice_portrait({session})` reacquires
owner management only for bounded protected publication, rechecks current owner,
source binding, original avatar/key and original proof, then atomically replaces
the record under the current-context lock. Ordinary Personal observation growth
does not change that immutable avatar identity; candidate/source replacement does.
Source redraw deliberately clears prior acoustic features. Existing deletion
removes them with their avatar; ring metadata survives.

The first explicit acoustic Save upgrades this protected vault to v2. A v1-only
binary rejects it; do not downgrade, regenerate its key or silently erase features.
Keep a consistent protected vault/initialization-marker backup and a compatible
binary for rollback. This is separate from telemetry schema9's strict timing
reader compatibility. Observer queue contention/overflow is a processing failure;
original-proof/source/deadline withdrawal is classified as withdrawn, not success.

Public v2 uses `ready_with_portrait`, exact bound candidate reference and v2
parameters: unchanged shape24/ring/rotation, twenty null or typed
`{length,curve,width,density,edge[8],energy[4]}` six-bit integer slots, tempo null.
The strict v1 reader remains supported. The approved 192-point contour still
renders only the core: capturing these supplementary features does not imply a
visibly changed avatar or authorize adding petals/ring to the mock. Advanced UI
reports captured/missing prompt slots without a similarity or lexical score.

Affected entry points: four portrait commands, Setup invalidation, the optional
Personal producer tap, bounded DSP and protected avatar reader/writer. Normal
producer/analysis/action semantics and media ownership are otherwise unchanged.
Native timing records only finite phases/outcomes and opaque operation IDs; no
session/source/feature/parameter values. No automated tests or real microphone,
Hello, protected-store mutation or capture operation is run by the implementation
agent. Real owner observation/continuity/cancellation/save/restart proof remains
unrun and Plan003 remains partial until those and its other criteria are proven.
