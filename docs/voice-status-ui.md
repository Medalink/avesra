# Personal voice status in Settings

The default Settings overview and People view show the native `personal_voice`
state and reason: Listening, Learning your voice, or Unavailable. The heading
invites the owner to talk only when native `voice_ready` is true. A saved voice
is reused by native code; no manual recording, calibration or consent step is
required by these views. New voice learning is reported only while native code
actually reports learning. This personal mode does not claim release validation.

Mute, deafen, pause and audio device controls remain available. Owner management,
explicit enrollment and voice diagnostics remain collapsed advanced tools.
Opening or leaving the passive People view never calls `cancel_setup`; cleanup
may cancel only enrollment that this component explicitly started. Existing
protected management commands retain their native checks.

Entry points: SetupOverview, EnrollmentView and SettingsView are presentation
consumers; owner-setup.ts maps native status to labels; runtime.ts declares the
native projection. VoiceCheck is unchanged and optional inside advanced tools.
No frontend state creates voice admission or action authority. Native learning,
recognition and persistence are governed by enrollment.md and turn-gating.md.
Verification for this change is Svelte check/build and source review; automated
tests and runtime microphone operations are excluded by the owner's instruction.
