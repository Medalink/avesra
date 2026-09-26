# People card presentation and explicit redraw

The current People mock is `design/mockups/Settings.dc.html` (Your voice), with
`SettingsPeopleYou.dc.html` and `SettingsPeopleOther.dc.html` as simulated states.
The other-speaker demo is not an enrolled-person record. The native avatar contract
in `voice-avatar.md` governs all actual source and immutable parameter data.

## This slice

Native `settings-hidden` immediately clears the sensitive card and invalidates
pending reads/actions. Native `settings-shown`, document restoration and focus
may request a fresh native `isVisible` check. Only the current mounted visibility
generation can restore read eligibility. Event listeners are installed before
initial eligibility. No focus event is required after a real native show; no old
card, diagnostic result or pending mutation response is revived.

The neutral card uses the mock's status dot, 88px/11px score label, blue Profile
chips, and expanding phrase-list treatment. Listening is readiness/control state,
not a measured speaker match. The signal panel remains unavailable and the score
remains Not measured without a native current-source observation. Completed
VoiceCheck results remain separate from live activity.

Redraw stays inside the existing Advanced voice tools disclosure, preserving the
mock's top-level card and footer. Redraw is a discrete explicit action, using normal Windows owner-management
verification and the native current-owner source. Prepare returns a one-shot
ticket and actual proposed parameters; the UI labels this preview Pending and
requires a separate Save action. Cancel, disclosure collapse, hide, context change, unmount or expiry
clears the preview and requests native ticket cancellation; a failed cancellation
request is not a claim that native work retired. Native expiry/current checks
remain authoritative. The original remaining confirmation
budget is conservatively measured from before Prepare; it is never renewed. It does not record audio or
re-enroll a speaker. The UI passes no vectors or source selection. It clears the
previous avatar before submitting confirmation and refreshes only after a still-current
response; failure remains visible and never triggers an automatic retry. Native
publication remains authoritative, including on caller loss. Hiding, locking,
changing the source context or leaving the page prevents stale UI publication.
The native operation preserves the actor ring and rebinds the current real source.

| Entry point | Scope |
| --- | --- |
| EnrollmentView mount/show/focus/document restoration | Native visibility confirmed, fresh reads only |
| settings-hidden, lock/source change, unmount | Clear/invalidate sensitive projections |
| YourVoiceCard phrases and neutral state | Mock geometry with actual available metadata |
| prepare_voice_avatar_redraw | Owner verification, strict native ticket/preview decoding, pending proposal only |
| confirm_voice_avatar_redraw | Explicit Save of original ticket; stale/lost response triggers actual-store reconciliation, never retry |
| cancel_voice_avatar_redraw | Explicit Cancel or lifetime withdrawal; no new authority |
| Test my voice / Re-enroll | Existing optional diagnostic/enrollment path unchanged |
| Live match, portrait capture, other people | Not implemented by this presentation slice |

Persisted integer parameters already provide repeatable geometry inputs; this UI
does not claim an executed repeatability check, enrollment date, held-out pass or
live match. No avatar parameters/digests enter timing. The user prohibited tests
and harnesses; verification here is source review and focused static checking.
