//! Local rendering preferences. They never select a voice or grant audio authority.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundPreset {
    #[default]
    Digital,
    Human,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTexture {
    #[default]
    Atmosphere,
    Pink,
    White,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundAmounts {
    pub effects: bool,
    pub background_enabled: bool,
    pub character: u8,
    pub background: u8,
    pub warmth: u8,
    pub space: u8,
    pub texture: u8,
    /// Downward pitch interval in half-semitones (-24..=24; negative is upward).
    #[serde(default = "default_harmonizer_depth")]
    pub harmonizer_depth: i8,
    #[serde(default = "default_presence")]
    pub presence: u8,
    #[serde(default = "default_echo")]
    pub echo: u8,
    #[serde(default)]
    pub echo_delay_left_ms: u16,
    #[serde(default)]
    pub echo_delay_right_ms: u16,
    pub background_texture: BackgroundTexture,
}
impl SoundAmounts {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if [
            self.character,
            self.background,
            self.warmth,
            self.texture,
            self.presence,
            self.echo,
        ]
        .iter()
        .any(|value| *value > 100)
            || self.space > 200
            || !(-24..=24).contains(&self.harmonizer_depth)
            || self.echo_delay_left_ms > 400
            || self.echo_delay_right_ms > 400
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}

fn default_presence() -> u8 {
    50
}
fn default_echo() -> u8 {
    35
}
fn default_harmonizer_depth() -> i8 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundSettings {
    pub revision: u64,
    pub enabled: bool,
    pub preset: SoundPreset,
    pub digital: SoundAmounts,
    pub human: SoundAmounts,
}
impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            revision: 0,
            enabled: true,
            preset: SoundPreset::Digital,
            digital: SoundAmounts {
                effects: true,
                background_enabled: true,
                character: 40,
                background: 85,
                warmth: 100,
                space: 100,
                texture: 75,
                harmonizer_depth: 1,
                presence: 85,
                echo: 10,
                echo_delay_left_ms: 105,
                echo_delay_right_ms: 65,
                background_texture: BackgroundTexture::Atmosphere,
            },
            human: SoundAmounts {
                effects: true,
                background_enabled: false,
                character: 25,
                background: 15,
                warmth: 30,
                space: 10,
                texture: 0,
                harmonizer_depth: 4,
                presence: 0,
                echo: 0,
                echo_delay_left_ms: 0,
                echo_delay_right_ms: 0,
                background_texture: BackgroundTexture::Atmosphere,
            },
        }
    }
}
impl SoundSettings {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.revision > 9_007_199_254_740_990 {
            return Err(ErrorCode::Malformed);
        }
        self.digital.validate()?;
        self.human.validate()
    }
    pub fn active(&self) -> SoundAmounts {
        match self.preset {
            SoundPreset::Digital => self.digital,
            SoundPreset::Human => self.human,
        }
    }
}

/// Each edit addresses one field, so unrelated slider/overlay updates cannot roll
/// back each other. Amount edits always name the preset the user was editing.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SoundEdit {
    Enabled {
        value: bool,
    },
    Preset {
        value: SoundPreset,
    },
    Volume {
        value: u8,
    },
    Amount {
        preset: SoundPreset,
        field: SoundField,
        value: u8,
    },
    Layer {
        preset: SoundPreset,
        field: SoundLayer,
        value: bool,
    },
    EchoDelay {
        preset: SoundPreset,
        channel: EchoChannel,
        value: u16,
    },
    HarmonizerDepth {
        preset: SoundPreset,
        value: i8,
    },
    BackgroundTexture {
        preset: SoundPreset,
        value: BackgroundTexture,
    },
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundField {
    Character,
    Background,
    Warmth,
    Space,
    Texture,
    Presence,
    Echo,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EchoChannel {
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundLayer {
    Effects,
    Background,
}
