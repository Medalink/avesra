# Settings drafts: installed manual observations, 2026-09-26

This record covers the installed production Windows UI and matching native/core
source at `695fc3e45323e3aeec62cf7e8ef1aa3e32c6167a`. It documents actual retained
observations, not an automated acceptance suite. The later `1eab` UI installation
is outside this evidence; these screenshots do not cover its newer cards.

## Identity and operating boundary

Artifacts are retained under
`E:/Dev/Avesra/artifacts/ui-parity-695fc3e/`.

| Item | Recorded identity |
| --- | --- |
| Frozen source archive | `source.tar`, SHA-256 `a16ba80fc22caaf0812cb1b7b99ade1a977eca7aa63ce462002e4cd32b085243` |
| Desktop binary | SHA-256 `934dd1f7461aab3eb19db2c3c4e0219bd886c77e87726eed5f0a751053fe89b5` |
| Native host binary | SHA-256 `cdfab6f3124b8af03558c87ab919a480b33e29dd6831cfdcf95bce3525e337c7` |
| Controller binary | SHA-256 `cf67565a0b8137b2ed2d4f20d4aaf5367d6c70c6ef434e0e32820e5999d6be18` |
| Product / telemetry schemas | Store 30; telemetry 7 |

`build-receipt.json` records package/build identities and no tests. Its
`deployment_performed: false` describes the packaging step, before the separate
installed inspection. `installed-native-launch.log` records owned PID 21832 on a
separate diagnostic desktop, no package identity, and input desktop `Default`
unchanged. `installed-store.json` and `spark-telemetry-installed.json` retain the
observed schema markers and successful SQLite quick checks.

The operator used the real production WebView and its actual controls on that
non-input desktop. This did not involve global input, owner microphone capture,
Windows Hello access, a fake UI, a private preference IPC call, or a test harness.
The diagnostic launch disables microphone capture and silences hardware output;
these observations do not establish acoustic playback. Follow
[the background evidence procedure](../background-evidence.md) for future runs.

## Observed Settings steps

The viewport stayed 880 by 640 CSS pixels. The initial interface size was 125%,
and the footer rectangle was x=0, y=588, width=880, height=52 CSS pixels in
`profiles-before.json`; `footer-initial.png` retains the initial clean footer.

| Actual control sequence | Observed result | Retained artifact |
| --- | --- | --- |
| Profiles & Machines → Interface size → 110%, without Save | 110% selected; Unsaved changes; DPR remained 1.25 | `footer-unsaved-110-dom.txt`, `footer-unsaved-110.png` |
| Discard | Selection returned to 125%; clean footer; DPR 1.25 | `discard-restored.json` |
| Select 110%, then Settings title close | Keep editing and Discard and close appeared, both enabled | `unsaved-close-dom.txt`, `unsaved-close.png` |
| Keep editing | 110% draft remained selected; Unsaved changes; DPR still 1.25 | `keep-editing.json` |
| Close again, choose Discard and close, reopen Settings | Clean footer; 125% selected; DPR 1.25 | `discard-close-reopened.json` |
| Select 110%, choose Save changes | Footer acknowledged Saved on this PC and immediate application; 110% selected; DPR 1.100000023841858 | `saved-110-dom.txt`, `saved-110.png` |
| Select 125%, choose Save changes | Saved acknowledgment; 125% selected; DPR restored to 1.25 | `restored-125.json` |

This establishes the observed title-close path and immediate scale application
for this run. It does not prove every native/OS close entry point or deferred
application at a next-turn boundary.

## Discard preserves independent live sound changes

The initial input-only attempt changed DOM range values to learning volume 14
and voice volume 99, but did not trigger the chime control's `change` handler.
`live-volume-with-draft.json` therefore still says No unsaved changes. That
artifact is not a passing draft/Discard observation.

The operator then completed the actual control change so the rendered labels
and footer reflected committed control handlers:

1. In Audio & Voice, change Learning volume from 15% to 14% as a preference
   draft. Change the independent live Voice volume from 100% to 99% and complete
   that control's change. `live-volume-committed-with-draft.json` shows Unsaved
   changes, Learning volume 14%, and Voice volume 99%.
2. Choose Discard. `discard-preserves-live-volume.json` shows the clean footer,
   Learning volume restored to 15%, and Voice volume still 99%.
3. Restore live Voice volume to 100%. `live-volume-restored.json` shows Voice
   volume 100%, Learning volume 15%, and no unsaved changes.

`preserved-audio.json` additionally retains the selected Razer Seiren X microphone,
SPDIF Interface output, speech speed 1, voice volume 100, atmosphere presence and
background amounts 85/85, and both chime levels 15. The scope is control state and
draft ownership; it is not a microphone or audible-device test.

## Native hide/reopen and retained timings

In Memory & Diagnostics → Performance, six actual retained voice-preview stage
rows were visible before closing Settings (`performance-before-close.json`).
After native hide, the rows were withdrawn and the view said local measurements
were unavailable (`performance-hidden.json`), even though DOM visibility still
reported visible. Reopening Settings automatically restored those six rows
without Refresh (`performance-auto-reopened-dom.txt` and
`performance-auto-reopened.png`). This is evidence for the native shown/hidden
notification path; DOM focus/visibility alone did not represent native hiding.

The six rows describe diagnostic preview admission, preparation, response,
submission and estimated drain. The artifact labels silent diagnostic mode and
missing model attribution. These are not accepted-turn latency, acoustic output,
release-qualified percentiles, or a measured bottleneck.

The operator opened App timings & export, used Read timings, and exported the
actual retained records. `actual-app-timings-export.json` contains 285 records;
`preference-timings-dom.json` retains the rendered timing table. The JSON export,
not that DOM artifact's empty auxiliary `rows` array, establishes the count.

| Preference command name | Exported records | Recorded outcome |
| --- | ---: | --- |
| `apply_preferences` | 2 | complete |
| `begin_preferences_editor` | 3 | complete |
| `retire_preferences_editor` | 2 | complete |
| `answer_preferences_close` | 2 | complete |
| `show_settings` | 0 | Not observed in this export |

The export reports zero observed native loss, frontend-reported loss and eviction;
that does not count unobserved abrupt-loss cases. These are frontend invoke-round-
trip observations. They do not independently measure internal coordinator stages,
writer retirement, other hosts, or every close route. Four new command names
were observed; the finite five-command registry is not five-command live proof.

## Repeatable manual procedure and remaining limits

For another authorized run, verify source/binary identity and the actual owned
production WebView first. Use the background procedure's manual inspector to
operate the visible controls by their observed labels and retain new DOM results
and screenshots after each step above. Do not call private IPC, alter component
state directly, substitute fixture data, or treat an assigned range value as an
accepted application edit. Complete each real control's change and inspect its
rendered label and footer before continuing. Restore the original scale and live
volume, then retain their final observed values.

No tests or harness were run for this evidence record. Still unproven here:

- Next-turn application; the implemented and observed semantics are immediate.
- Conflict review, persistence/window-effect failure, queue failure, lost replies,
  and close-during-save recovery under actual failure conditions.
- Alt+F4/OS CloseRequested, tray Quit, Windows lock/unlock/logoff, forced teardown,
  and every registration race on the installed application.
- Device switching/check/preview with an unsaved device, every profile/control,
  independent remembered-name changes with a draft, and full UI parity.
- The later `1eab` UI, current Profiles radio-card changes, full Plan 001/003
  completion, beta readiness, accepted-owner conversation and acoustic quality.
