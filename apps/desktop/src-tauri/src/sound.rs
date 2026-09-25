//! Native-authoritative partial edits; output changes precede persistence.
use crate::{Runtime, enqueue};
use avesra_core::{
    sound::{EchoChannel, SoundEdit, SoundField, SoundLayer, SoundPreset},
    state::LocalState,
};
use tauri::{Emitter, Manager};

fn amounts(
    settings: &mut avesra_core::state::Settings,
    preset: SoundPreset,
) -> &mut avesra_core::sound::SoundAmounts {
    match preset {
        SoundPreset::Digital => &mut settings.sound.digital,
        SoundPreset::Human => &mut settings.sound.human,
    }
}

#[tauri::command]
pub fn sound_output_channels(state: tauri::State<'_, Runtime>) -> Option<u16> {
    state.media.output_channels()
}

#[tauri::command]
pub fn sound_diagnostics(
    state: tauri::State<'_, Runtime>,
) -> Result<crate::media::SoundDiagnostics, String> {
    state.media.sound_diagnostics()
}

#[tauri::command]
pub async fn update_sound(
    edit: SoundEdit,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<LocalState, String> {
    if !matches!(window.label(), "settings" | "overlay") {
        return Err("Use Avesra's local sound controls".into());
    }
    let state = app.state::<Runtime>();
    let (snapshot, pending) = {
        let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut settings = local.settings.clone();
        match edit {
            SoundEdit::Enabled { value } => settings.sound.enabled = value,
            SoundEdit::Preset { value } => settings.sound.preset = value,
            SoundEdit::Volume { value } => settings.speech_volume = value,
            SoundEdit::Amount {
                preset,
                field,
                value,
            } => {
                let a = amounts(&mut settings, preset);
                match field {
                    SoundField::Character => a.character = value,
                    SoundField::Background => a.background = value,
                    SoundField::Warmth => a.warmth = value,
                    SoundField::Space => a.space = value,
                    SoundField::Texture => a.texture = value,
                    SoundField::Presence => a.presence = value,
                    SoundField::Echo => a.echo = value,
                }
            }
            SoundEdit::Layer {
                preset,
                field,
                value,
            } => {
                let a = amounts(&mut settings, preset);
                match field {
                    SoundLayer::Effects => a.effects = value,
                    SoundLayer::Background => a.background_enabled = value,
                }
            }
            SoundEdit::BackgroundTexture { preset, value } => {
                amounts(&mut settings, preset).background_texture = value
            }
            SoundEdit::HarmonizerDepth { preset, value } => {
                amounts(&mut settings, preset).harmonizer_depth = value
            }
            SoundEdit::EchoDelay {
                preset,
                channel,
                value,
            } => {
                let a = amounts(&mut settings, preset);
                match channel {
                    EchoChannel::Left => a.echo_delay_left_ms = value,
                    EchoChannel::Right => a.echo_delay_right_ms = value,
                }
            }
        }
        settings.sound.revision = settings
            .sound
            .revision
            .checked_add(1)
            .ok_or("Sound revision exhausted")?;
        settings.validate().map_err(|e| e.to_string())?;
        local.settings = settings;
        local.refresh();
        // Sound controls are local rendering preferences, not control-session
        // mode transitions. Do not churn remote acknowledgements during preview.
        state.media.publish(&local);
        let pending = enqueue(&state, local.settings.clone());
        (local.clone(), pending)
    };
    app.emit("runtime-state", &snapshot)
        .map_err(|_| "Sound changed; window notification failed")?;
    pending
        .map_err(|e| format!("Sound changed for this session, but could not be saved: {e}"))?
        .await
        .map_err(|_| "Sound changed for this session, but the settings writer stopped")??;
    crate::runtime_snapshot(state)
}
