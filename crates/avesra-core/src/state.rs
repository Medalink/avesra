use avesra_contracts::{ErrorCode, Profile};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub microphone: Option<String>,
    pub speaker: Option<String>,
    pub profile: Profile,
    pub always_on_top: bool,
    pub learning_chime: bool,
    pub action_chime: bool,
    pub chime_volume: u8,
    pub speech_volume: u8,
    pub speech_rate: u16,
    pub explicit_mute: bool,
    pub deafened: bool,
    pub paused: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            microphone: None,
            speaker: None,
            profile: Profile::SingleSpark,
            always_on_top: true,
            learning_chime: false,
            action_chime: false,
            chime_volume: 15,
            speech_volume: 80,
            speech_rate: 100,
            explicit_mute: true,
            deafened: false,
            paused: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.chime_volume > 100
            || self.speech_volume > 100
            || !(50..=200).contains(&self.speech_rate)
        {
            return Err(ErrorCode::Malformed);
        }
        if self
            .microphone
            .as_ref()
            .is_some_and(|x| x.is_empty() || x.len() > 512)
            || self
                .speaker
                .as_ref()
                .is_some_and(|x| x.is_empty() || x.len() > 512)
        {
            return Err(ErrorCode::Malformed);
        }
        if self.profile == Profile::Accelerated {
            return Err(ErrorCode::Unavailable);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalState {
    pub revision: u64,
    pub settings: Settings,
    pub capture_epoch: u64,
    pub action_epoch: u64,
    pub connected: bool,
    pub enrolled: bool,
    pub voice_ready: bool,
    pub enrollment_capture: bool,
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
            action_epoch: 1,
            connected: false,
            enrolled: false,
            voice_ready: false,
            enrollment_capture: false,
            locked: false,
            active_task: false,
            status: String::new(),
            reason: String::new(),
        };
        value.refresh();
        value
    }
    pub fn capture_allowed(&self) -> bool {
        self.connected
            && ((self.enrolled && self.voice_ready) || self.enrollment_capture)
            && !self.locked
            && !self.settings.explicit_mute
            && !self.settings.deafened
            && !self.settings.paused
    }
    pub fn apply(&mut self, control: LocalControl) {
        self.enrollment_capture = false;
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
        self.revision = self.revision.saturating_add(1);
        let (status, reason) = if self.settings.paused {
            ("paused", "Listening and new actions are paused.")
        } else if self.settings.deafened {
            ("deafened", "Microphone and assistant sound are off.")
        } else if self.settings.explicit_mute {
            ("muted", "Microphone is off.")
        } else if !self.connected {
            ("disconnected", "Pair the Spark to connect Avesra.")
        } else if self.enrollment_capture {
            (
                "enrolling",
                "Recording an explicit enrollment phrase. Cancel or mute to stop.",
            )
        } else if !self.enrolled {
            (
                "unavailable",
                "Enroll and validate the owner before listening.",
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
    }
}
