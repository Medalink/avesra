# Optional portrait observation UI

This supplements `voice-avatar.md` and `people-card.md`. It is an optional
Advanced voice tools flow, not a conversation activation step or speaker test.
Normal Personal capture keeps its existing owner and continues unless the user's
listening controls withdraw it. No UI command opens or pauses a hardware stream.

Begin consumes normal Windows owner-management verification and returns the
native session, actual fixed words, slot states and original remaining budget.
Two separate explicit Record actions each observe an eight-second batch from the
existing Personal producer. The UI starts no recording on mount, Begin, progress,
reopen, failed response or save. Cancel is always available during observation.

Only exact session/request progress for the mounted visible owner/source context
is displayed. Original expiry is conservatively anchored before Begin and only
shortened by later native replies. Hide, disclosure collapse, lock, context change,
expiry or unmount clears sensitive state and requests cancellation; no timer or
IPC acknowledgement proves worker retirement. A failed/lost Record or Save is
not retried. Cancellation can race an already-committed Save: the UI reports a
request, not rollback, and rereads saved state after actual settlement only for
the same still-visible owner/source context. Save is allowed only after both batches completed and at least one
slot was captured. Missing slots are missing features, not unrecognized words.

Native v1 parameters remain readable. V2 requires exactly20 nullable feature
slots with finite six-bit integer length/curve/width/density, edge8 and energy4;
tempo stays null. Version/state/parameter mismatch is unavailable. The current
192-point avatar renderer still uses the24 shape values; it does not add petals,
a visible ring, invented tempo or a live identity score. A captured portrait is
optional metadata and is not a passed repeatability or speaker-verification test.

| Entry point | Ownership |
| --- | --- |
| begin_voice_portrait | Explicit verified Begin, exact native session/budget |
| record_voice_portrait | Explicit batch request UUID, bounded progress only |
| save_voice_portrait | Explicit current completed-session Save; reread actual saved state |
| cancel_voice_portrait | Explicit or lifecycle cancellation, no replacement capture |
| speaker_candidates | Strict v1/v2 saved view decoder; original current read ownership |
| Settings show/hide and Advanced disclosure | Mount/unmount and context invalidation; never resume capture |

No transcript, PCM, parameters or digest enters UI timing. Static/source review
only for this slice under the user's prohibition on automated tests/harnesses;
real owner observations and repeatability remain separate evidence.
