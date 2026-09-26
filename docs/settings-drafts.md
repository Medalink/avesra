# Settings preference drafts and voluntary close

The reference is the six-section Settings window and 52-pixel footer in
`design/mockups/Settings.dc.html`. This slice implements real local preference
drafts, not the prototype's simulated save or next-turn application promise.
The existing native application semantics remain immediate. Next-turn application
requires a separate ownership contract and is explicitly unfinished.

## Exact editable boundary

Only the nine fields currently edited through `SettingsView.update` participate:
`microphone`, `speaker`, `learning_chime`, `action_chime`,
`learning_chime_volume`, `action_chime_volume`, `profile`, `interface_scale`, and
`always_on_top`. No arbitrary JSON property or whole Settings object is accepted.
Each typed field change carries its original expected value and proposed value;
nullable device/volume values permit explicit null but both keys are required; omission is rejected. A batch
contains one through nine unique fields and rejects duplicates or unknown fields.

Under the existing native local-state mutex, compare every touched field against
its expected value, clone the latest Settings, merge only those fields, validate
the complete candidate, then enqueue before publishing. Any conflict rejects the
whole batch without mutation and names the conflicting fields. The UI preserves
the draft and requires explicit review against refreshed values before retrying;
it cannot silently rebase touched fields. Untouched fields always come from the
current native snapshot, including sound, safety, owner and shortcuts.

The existing immediate capture/playback/action epoch rules, profile restrictions,
scale and topmost window effects remain in force. Once applied, the reply must
distinguish durable success from an applied preference whose storage or window
effect failed. A lost reply is uncertain and requires reconciliation, not an
automatic replay or a false "not applied" claim. The storage owner keeps its
queued write through actual completion even if the caller goes away.

Native edit registration, commit, window effects and voluntary close transitions
serialize on the main event thread. Registration and commit require the actual
Settings window to be visible and Windows unlocked. No window getter runs under
the local-state mutex. A retry reapplies requested scale/topmost effects even if
the stored value already matches. Persistence completion is awaited off-thread.

The frontend uses native shown/hidden notifications, not DOM visibility or focus
as evidence of a native window being shown. Stale registration responses are
retired by exact identity and cannot revive or clear a newer editor. A stable
per-context registration nonce permits an explicit retry after a lost reply to
recover the same editor and pending close request. An early close notification
is retained until the matching registration settles; it never acknowledges a
different editor. Native close revisions order the buffered notification against the returned snapshot. An explicit Refresh close decision action reconciles a lost close-answer reply without dropping or rebasing preference edits. Show success still notifies the view if focus fails; delivery
failure cancels only that pending close request and reports the error.

## Footer and failure semantics

The footer uses the reference 52-pixel geometry and actual clean, dirty, saving,
conflict, saved and failure states. Save publishes the bounded typed draft;
Discard clears only edits that have not been applied. Save success means the
existing writer acknowledged persistence, not an acoustic/device proof and not
a deferred next-turn operation. Never display the prototype's fixed save time.

Applied-but-not-durable values are already live: Discard cannot undo them. Keep a
visible applied/unsaved failure, permit an explicit persistence retry against
reconciled current values, and label any decision to close without another save
accurately. Window-effect failure similarly does not undo a stored preference.
Do not drop failure status merely because the current snapshot equals the draft.
Windows lock/logoff, actual native hide and forced teardown invalidate draft publication. Remembered-name changes are cosmetic and preserve ordinary preference drafts; they are not an owner identity signal. These ordinary preferences belong to the current native process and Windows session, not to a protected owner-management grant. Any future genuine owner-replacement route must explicitly invalidate the editor.

Active profile summaries continue to show the actual runtime profile. Unsaved
device choices are marked beside their selectors with the actual in-use device;
microphone checks and voice previews continue to use that actual runtime device.
Save is required to switch. Safety Stop/cancel controls remain usable.

Live voice volume/atmosphere, mute/deafen/pause/Stop, shortcuts, owner name,
pairing, voice candidates, enrollment, protected permissions, history and memory
retain their own immediate commands. Discard never reverses these operations.

## Voluntary close coordinator

Do not infer native close safety from an asynchronously sent dirty flag: a user
can close immediately after editing. Every voluntary Settings hide/native close
uses one native request coordinator while the editor is registered. It retains
one current opaque request and intent, emits that request to the real Settings
view, and keeps the window available until the current UI answers. A clean view
can acknowledge; a dirty view shows Keep editing and Discard and close. A stale
request or a changed edit generation cannot close a newer draft. Listener failure
stays visible and does not silently discard. There is no timeout-based discard.

Only an accepted current response performs existing Settings invalidation,
position saving and hide. Tray Quit uses the same confirmation with a quit intent,
then performs existing quit teardown. Overlay hide remains independent. OS lock,
logoff, safety Stop, destruction and forced process termination are never blocked
by a preference dialog. Lock/logoff and teardown invalidate editor/close tokens; safety Stop keeps its immediate behavior and does not discard ordinary drafts. No UI draft grants
or retains native protected-operation authority.

## Entry points and scope

| Entry point | Handling |
| --- | --- |
| SettingsView's nine ordinary update keys | Local draft and expected-value native patch |
| App.update / old whole-object save_settings | Replaced for ordinary UI edits; no stale full snapshot save path |
| App.hide / Settings title close / hide_window(settings) | Shared close request and current response |
| Native CloseRequested (including Alt+F4) | Prevent destruction; route through same close request |
| Tray Quit | Quit intent through same coordinator before existing teardown |
| Overlay hide; tray/overlay show Settings | Unchanged, no preference discard |
| Focus loss / shortcut recording cancellation | Unchanged; does not discard drafts |
| Windows lock/logoff and native destruction | Immediate existing authority withdrawal; invalidate draft/close context |
| Safety controls | Immediate existing behavior; no preference confirmation or automatic draft discard |
| Sound, owner, shortcuts and protected operations | Unchanged independent owners, excluded from draft |
| Existing storage worker | Bounded queue, actual write acknowledgment and visible failure |

Validation follows the explicit owner instruction: no automated tests, fixtures
or harnesses. Use source review, focused Svelte/static checks, coordinated native
checks, and separately authorized real-app manual proof. No build, static pass or
footer screenshot establishes complete functional parity or beta readiness.

Integration follow-up: register the new bounded preference commands in the app-timing registry when that separately developed branch is merged. Timing coverage for these new commands is not yet claimed.
