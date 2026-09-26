use avesra_contracts::{ErrorCode, Profile};
use serde::{Deserialize, Serialize};

/// Interface sizes, in percent. Every Avesra window is zoomed uniformly, so the
/// approved reference geometry (drawn at 100%) keeps its proportions.
pub const INTERFACE_SCALES: [u16; 5] = [100, 110, 125, 150, 175];
pub const DEFAULT_INTERFACE_SCALE: u16 = 125;
fn default_interface_scale() -> u16 {
    DEFAULT_INTERFACE_SCALE
}

/// Explicitly remembered personal detail, bound to the authenticated local owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RememberedName {
    pub actor: uuid::Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub owner_name: Option<RememberedName>,
    #[serde(default)]
    pub sound: crate::sound::SoundSettings,
    #[serde(default)]
    pub shortcuts: crate::shortcuts::Shortcuts,
    #[serde(default)]
    pub audio_device_schema: u16,
    pub microphone: Option<String>,
    pub speaker: Option<String>,
    pub profile: Profile,
    pub always_on_top: bool,
    #[serde(default = "default_interface_scale")]
    pub interface_scale: u16,
    pub learning_chime: bool,
    pub action_chime: bool,
    pub chime_volume: u8,
    #[serde(default)]
    pub learning_chime_volume: Option<u8>,
    #[serde(default)]
    pub action_chime_volume: Option<u8>,
    pub speech_volume: u8,
    pub speech_rate: u16,
    pub explicit_mute: bool,
    pub deafened: bool,
    pub paused: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            owner_name: None,
            sound: crate::sound::SoundSettings::default(),
            shortcuts: crate::shortcuts::Shortcuts::default(),
            audio_device_schema: 1,
            microphone: None,
            speaker: None,
            profile: Profile::SingleSpark,
            always_on_top: true,
            interface_scale: DEFAULT_INTERFACE_SCALE,
            learning_chime: false,
            action_chime: false,
            chime_volume: 15,
            learning_chime_volume: None,
            action_chime_volume: None,
            speech_volume: 80,
            speech_rate: 100,
            explicit_mute: false,
            deafened: false,
            paused: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.owner_name.as_ref().is_some_and(|value| {
            value.actor.is_nil() || !avesra_contracts::preview::Greeting::valid_name(&value.name)
        }) {
            return Err(ErrorCode::Malformed);
        }
        self.sound.validate()?;
        self.shortcuts.validate()?;
        if self.audio_device_schema != 1
            || self.chime_volume > 100
            || self.learning_chime_volume.is_some_and(|v| v > 100)
            || self.action_chime_volume.is_some_and(|v| v > 100)
            || self.speech_volume > 100
            || !(50..=200).contains(&self.speech_rate)
            || !INTERFACE_SCALES.contains(&self.interface_scale)
        {
            return Err(ErrorCode::Malformed);
        }
        if self
            .microphone
            .as_ref()
            .is_some_and(|x| !valid_audio_device_id(x))
            || self
                .speaker
                .as_ref()
                .is_some_and(|x| !valid_audio_device_id(x))
        {
            return Err(ErrorCode::Malformed);
        }
        if self.profile == Profile::Accelerated {
            return Err(ErrorCode::Unavailable);
        }
        Ok(())
    }
}
pub fn valid_audio_device_id(value: &str) -> bool {
    value.starts_with("wasapi:")
        && value.len() > 7
        && value.len() <= 1024
        && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionPhase {
    Disconnected,
    Connecting,
    Connected,
    Error,
}
#[derive(Clone, Debug, Serialize)]
pub struct PersonalVoiceStatus {
    pub state: PersonalVoicePhase,
    pub reason: String,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalVoicePhase {
    Learning,
    Listening,
    Unavailable,
}
impl Default for PersonalVoiceStatus {
    fn default() -> Self {
        Self {
            state: PersonalVoicePhase::Unavailable,
            reason: "Preparing personal voice services.".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct LocalState {
    pub revision: u64,
    pub settings: Settings,
    pub capture_epoch: u64,
    pub playback_epoch: u64,
    pub action_epoch: u64,
    pub connected: bool,
    pub connection_phase: ConnectionPhase,
    pub connection_error: Option<String>,
    pub enrolled: bool,
    pub voice_ready: bool,
    pub personal_voice: PersonalVoiceStatus,
    pub enrollment_capture: bool,
    pub microphone_check: bool,
    #[serde(skip)]
    pub microphone_check_epoch: Option<u64>,
    pub capture_error: Option<String>,
    pub locked: bool,
    pub active_task: bool,
    pub status: String,
    pub reason: String,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalControl {
    Mute,
    Unmute,
    Deafen,
    Undeafen,
    Pause,
    Resume,
    Stop,
    Lock,
    Disconnect,
    Quit,
}
impl LocalState {
    pub fn new(settings: Settings) -> Self {
        let mut value = Self {
            revision: 1,
            settings,
            capture_epoch: 1,
            playback_epoch: 1,
            action_epoch: 1,
            connected: false,
            connection_phase: ConnectionPhase::Disconnected,
            connection_error: None,
            enrolled: false,
            voice_ready: false,
            personal_voice: PersonalVoiceStatus::default(),
            enrollment_capture: false,
            microphone_check: false,
            microphone_check_epoch: None,
            capture_error: None,
            locked: false,
            active_task: false,
            status: String::new(),
            reason: String::new(),
        };
        value.refresh();
        value
    }
    pub fn capture_allowed(&self) -> bool {
        !self.microphone_check
            && self.connected
            && ((self.enrolled && self.voice_ready) || self.enrollment_capture)
            && !self.locked
            && !self.settings.explicit_mute
            && !self.settings.deafened
            && !self.settings.paused
    }
    pub fn microphone_check_allowed(&self) -> bool {
        self.microphone_check
            && !self.locked
            && !self.settings.explicit_mute
            && !self.settings.deafened
            && !self.settings.paused
            && self.settings.microphone.is_some()
    }
    pub fn apply(&mut self, control: LocalControl) {
        if matches!(
            control,
            LocalControl::Lock | LocalControl::Disconnect | LocalControl::Quit
        ) {
            self.connection_phase = ConnectionPhase::Disconnected;
            self.connection_error = None;
        }
        self.enrollment_capture = false;
        self.microphone_check = false;
        match control {
            LocalControl::Mute => self.settings.explicit_mute = true,
            LocalControl::Unmute => self.settings.explicit_mute = false,
            LocalControl::Deafen => self.settings.deafened = true,
            LocalControl::Undeafen => self.settings.deafened = false,
            LocalControl::Pause => self.settings.paused = true,
            LocalControl::Resume => self.settings.paused = false,
            LocalControl::Stop => self.active_task = false,
            LocalControl::Lock => {
                self.locked = true;
                self.connected = false;
                self.active_task = false;
            }
            LocalControl::Disconnect => {
                self.connected = false;
                self.active_task = false;
            }
            LocalControl::Quit => {
                self.settings.explicit_mute = true;
                self.connected = false;
                self.active_task = false;
            }
        }
        self.capture_epoch = self.capture_epoch.saturating_add(1);
        if !matches!(control, LocalControl::Mute | LocalControl::Unmute) {
            self.playback_epoch = self.playback_epoch.saturating_add(1);
        }
        if matches!(
            control,
            LocalControl::Pause
                | LocalControl::Stop
                | LocalControl::Lock
                | LocalControl::Disconnect
                | LocalControl::Quit
        ) {
            self.action_epoch = self.action_epoch.saturating_add(1);
        }
        self.refresh();
    }
    pub fn refresh(&mut self) {
        if self.microphone_check_epoch != Some(self.capture_epoch) {
            self.microphone_check = false;
        }
        if !self.microphone_check {
            self.microphone_check_epoch = None;
        }
        if self.connected {
            self.connection_phase = ConnectionPhase::Connected;
            self.connection_error = None;
        }
        self.revision = self.revision.saturating_add(1);
        let (status, reason) = if self.locked {
            (
                "unavailable",
                "Windows session is locked, disconnected, or unavailable.",
            )
        } else if self.settings.paused {
            ("paused", "Listening and new actions are paused.")
        } else if self.settings.deafened {
            ("deafened", "Microphone and assistant sound are off.")
        } else if self.settings.explicit_mute {
            ("muted", "Microphone is off.")
        } else if self.microphone_check {
            (
                "checking",
                "Checking microphone locally. No audio is saved or sent.",
            )
        } else if !self.connected {
            ("disconnected", "Pair the Spark to connect Avesra.")
        } else if self.enrollment_capture {
            (
                "enrolling",
                "Recording an explicit voice setup phrase. Cancel or mute to stop.",
            )
        } else if !self.enrolled {
            (
                "unavailable",
                "Preparing personal voice for this Windows owner.",
            )
        } else if !self.voice_ready {
            ("unavailable", "Speech and identity services are not ready.")
        } else {
            (
                "passive",
                "Listening for an assistant-directed owner request.",
            )
        };
        self.status = status.into();
        self.reason = reason.into();
        if self.locked
            || !self.connected
            || self.settings.paused
            || self.settings.deafened
            || self.settings.explicit_mute
        {
            self.personal_voice = PersonalVoiceStatus {
                state: PersonalVoicePhase::Unavailable,
                reason: self.reason.clone(),
            };
        }
    }
}
