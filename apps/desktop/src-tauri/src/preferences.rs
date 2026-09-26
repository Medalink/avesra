//! Expected-value edits of ordinary preferences; application is immediate.
use crate::{Runtime, apply_interface_scale, enqueue, settings_editor, tasks};
use avesra_contracts::Profile;
use avesra_core::state::{LocalState, Settings};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

#[derive(Deserialize)]
#[serde(tag = "field", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Microphone {
        #[serde(deserialize_with = "required_nullable")]
        expected: Option<String>,
        #[serde(deserialize_with = "required_nullable")]
        value: Option<String>,
    },
    Speaker {
        #[serde(deserialize_with = "required_nullable")]
        expected: Option<String>,
        #[serde(deserialize_with = "required_nullable")]
        value: Option<String>,
    },
    LearningChime {
        expected: bool,
        value: bool,
    },
    ActionChime {
        expected: bool,
        value: bool,
    },
    LearningChimeVolume {
        #[serde(deserialize_with = "required_nullable")]
        expected: Option<u8>,
        #[serde(deserialize_with = "required_nullable")]
        value: Option<u8>,
    },
    ActionChimeVolume {
        #[serde(deserialize_with = "required_nullable")]
        expected: Option<u8>,
        #[serde(deserialize_with = "required_nullable")]
        value: Option<u8>,
    },
    Profile {
        expected: Profile,
        value: Profile,
    },
    InterfaceScale {
        expected: u16,
        value: u16,
    },
    AlwaysOnTop {
        expected: bool,
        value: bool,
    },
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn replace<T: PartialEq + Clone>(current: &mut T, expected: &T, value: &T) -> bool {
    if current != expected {
        return false;
    }
    *current = value.clone();
    true
}

impl Change {
    fn field(&self) -> &'static str {
        match self {
            Self::Microphone { .. } => "microphone",
            Self::Speaker { .. } => "speaker",
            Self::LearningChime { .. } => "learning_chime",
            Self::ActionChime { .. } => "action_chime",
            Self::LearningChimeVolume { .. } => "learning_chime_volume",
            Self::ActionChimeVolume { .. } => "action_chime_volume",
            Self::Profile { .. } => "profile",
            Self::InterfaceScale { .. } => "interface_scale",
            Self::AlwaysOnTop { .. } => "always_on_top",
        }
    }
    fn merge(&self, settings: &mut Settings) -> bool {
        match self {
            Self::Microphone { expected, value } => {
                replace(&mut settings.microphone, expected, value)
            }
            Self::Speaker { expected, value } => replace(&mut settings.speaker, expected, value),
            Self::LearningChime { expected, value } => {
                replace(&mut settings.learning_chime, expected, value)
            }
            Self::ActionChime { expected, value } => {
                replace(&mut settings.action_chime, expected, value)
            }
            Self::LearningChimeVolume { expected, value } => {
                replace(&mut settings.learning_chime_volume, expected, value)
            }
            Self::ActionChimeVolume { expected, value } => {
                replace(&mut settings.action_chime_volume, expected, value)
            }
            Self::Profile { expected, value } => replace(&mut settings.profile, expected, value),
            Self::InterfaceScale { expected, value } => {
                replace(&mut settings.interface_scale, expected, value)
            }
            Self::AlwaysOnTop { expected, value } => {
                replace(&mut settings.always_on_top, expected, value)
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Problem {
    Notification,
    Topmost,
    Scale,
    Persistence,
    WriterStopped,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplyResult {
    Conflict {
        fields: Vec<&'static str>,
        snapshot: LocalState,
    },
    Applied {
        snapshot: LocalState,
        durable: bool,
        problems: Vec<Problem>,
    },
}

enum Commit {
    Conflict(ApplyResult),
    Applied {
        snapshot: LocalState,
        pending: tokio::sync::oneshot::Receiver<Result<(), String>>,
        problems: Vec<Problem>,
    },
}

fn commit(
    app: &tauri::AppHandle,
    editor: uuid::Uuid,
    changes: Vec<Change>,
) -> Result<Commit, String> {
    let window = app
        .get_webview_window("settings")
        .ok_or("Settings unavailable")?;
    if !window.is_visible().unwrap_or(false) {
        return Err("Open Settings to save preferences".into());
    }
    let state = app.state::<Runtime>();
    let (snapshot, pending, scale_changed, topmost_changed) = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        settings_editor::require_editor(app, editor)?;
        if local.locked {
            return Err("Settings context changed; review your preferences".into());
        }
        let mut settings = local.settings.clone();
        let conflicts = changes
            .iter()
            .filter_map(|change| (!change.merge(&mut settings)).then_some(change.field()))
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            return Ok(Commit::Conflict(ApplyResult::Conflict {
                fields: conflicts,
                snapshot: local.clone(),
            }));
        }
        settings.validate().map_err(|error| error.to_string())?;
        // A full queue rejects before any live state changes. The writer owns
        // the queued snapshot even if this command's caller later disappears.
        let pending = enqueue(&state, settings.clone())?;
        let scale_changed = changes
            .iter()
            .any(|change| matches!(change, Change::InterfaceScale { .. }));
        let topmost_changed = changes
            .iter()
            .any(|change| matches!(change, Change::AlwaysOnTop { .. }));
        let same_spark_route = matches!(
            (local.settings.profile, settings.profile),
            (Profile::SingleSpark, Profile::Gaming) | (Profile::Gaming, Profile::SingleSpark)
        );
        let route_changed = settings.profile != local.settings.profile && !same_spark_route;
        if settings.profile != local.settings.profile && settings.profile == Profile::Gaming {
            tasks::teaching::defer_passive(&state);
        }
        if settings.speaker != local.settings.speaker || route_changed {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
            local.action_epoch = local.action_epoch.saturating_add(1);
        }
        if settings.microphone != local.settings.microphone
            || settings.speaker != local.settings.speaker
            || route_changed
        {
            local.microphone_check = false;
            local.enrollment_capture = false;
            local.capture_epoch = local.capture_epoch.saturating_add(1);
        }
        local.settings = settings;
        local.refresh();
        state.publish(&local);
        (local.clone(), pending, scale_changed, topmost_changed)
    };
    // Nothing below this point can report an unapplied rejection.
    let mut problems = Vec::new();
    if app.emit("runtime-state", &snapshot).is_err() {
        problems.push(Problem::Notification);
    }
    if topmost_changed
        && app.get_webview_window("overlay").is_none_or(|overlay| {
            overlay
                .set_always_on_top(snapshot.settings.always_on_top)
                .is_err()
        })
    {
        problems.push(Problem::Topmost);
    }
    if scale_changed && apply_interface_scale(app, snapshot.settings.interface_scale).is_err() {
        problems.push(Problem::Scale);
    }
    Ok(Commit::Applied {
        snapshot,
        pending,
        problems,
    })
}

#[tauri::command]
pub async fn apply_preferences(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    editor: uuid::Uuid,
    changes: Vec<Change>,
) -> Result<ApplyResult, String> {
    if window.label() != "settings" {
        return Err("Use the Settings editor".into());
    }
    if changes.is_empty() || changes.len() > 9 {
        return Err("A preference patch must contain one through nine fields".into());
    }
    let mut seen = std::collections::HashSet::new();
    if changes.iter().any(|change| !seen.insert(change.field())) {
        return Err("A preference patch cannot repeat a field".into());
    }
    let result =
        settings_editor::on_main(app.clone(), move |app| commit(&app, editor, changes)).await?;
    let (snapshot, pending, mut problems) = match result {
        Commit::Conflict(result) => return Ok(result),
        Commit::Applied {
            snapshot,
            pending,
            problems,
        } => (snapshot, pending, problems),
    };
    let durable = match pending.await {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            problems.push(Problem::Persistence);
            false
        }
        Err(_) => {
            problems.push(Problem::WriterStopped);
            false
        }
    };
    let state = app.state::<Runtime>();
    let snapshot = state
        .local
        .lock()
        .map(|local| local.clone())
        .unwrap_or(snapshot);
    Ok(ApplyResult::Applied {
        snapshot,
        durable,
        problems,
    })
}
