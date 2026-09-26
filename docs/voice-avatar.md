# Native voice avatar, version 1

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
source is reported explicitly; it is not silently substituted. A later explicit
portrait/redraw operation will replace the immutable record, keeping its actor
ring. Owner revision changes fail closed and never transfer an old record.

The webview receives only version 1, 24 six-bit shape values, 20 optional petal
slots, a 16-digit lowercase hexadecimal ring, integer rotation in 1/256 turns,
optional quantized tempo, and a display digest. The foundation has no measured
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

Portrait capture/DSP, redraw UI, other-person management, live matching animation
and runtime repeatability evidence follow this foundation; empty slots are honest
absence, not completion. Verification follows the owner's no-tests override:
source review, static checks/builds when released, then separately authorized real
app observations. No test fixtures, imported evidence or generated voice identity.

Native timing integration inventory: `speaker_candidates` has the actual retained
blocking operation; `voice_avatar::current` owns source resolution/vault read and
parameter preparation; `shape::build` owns the keyed projection; `publish` owns
protected encoding/staging/sync/publication. These phases must use the shared
Portrait-domain timing owner when integrated, with failed/withdrawn/abandoned
outcomes. Until then, a measured IPC round-trip is not separate native phase
evidence. No source identity, ring, digest, parameter, vector, word or feature may
enter those observations. Capture/extraction have no implementation in this slice.
