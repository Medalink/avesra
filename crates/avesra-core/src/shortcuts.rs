//! Typed local controls; no executable, arbitrary key hook or remote authority.
use avesra_contracts::ErrorCode;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    Mute,
    Deafen,
    Overlay,
}
pub const ACTIONS: [ShortcutAction; 3] = [
    ShortcutAction::Mute,
    ShortcutAction::Deafen,
    ShortcutAction::Overlay,
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chord {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: u16,
}
impl Chord {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if !(self.control || self.alt) || !matches!(self.key,0x30..=0x39|0x41..=0x5a|0x70..=0x7a) {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn modifiers(&self) -> u32 {
        u32::from(self.alt) | (u32::from(self.control) * 2) | (u32::from(self.shift) * 4)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shortcuts {
    pub mute: Option<Chord>,
    pub deafen: Option<Chord>,
    pub overlay: Option<Chord>,
}
impl Default for Shortcuts {
    fn default() -> Self {
        let chord = |key| {
            Some(Chord {
                control: true,
                alt: false,
                shift: true,
                key,
            })
        };
        Self {
            mute: chord(0x4d),
            deafen: chord(0x44),
            overlay: chord(0x41),
        }
    }
}
impl Shortcuts {
    pub fn get(&self, action: ShortcutAction) -> Option<Chord> {
        match action {
            ShortcutAction::Mute => self.mute,
            ShortcutAction::Deafen => self.deafen,
            ShortcutAction::Overlay => self.overlay,
        }
    }
    pub fn set(&mut self, action: ShortcutAction, chord: Option<Chord>) {
        match action {
            ShortcutAction::Mute => self.mute = chord,
            ShortcutAction::Deafen => self.deafen = chord,
            ShortcutAction::Overlay => self.overlay = chord,
        }
    }
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let all = [self.mute, self.deafen, self.overlay];
        for (i, chord) in all.iter().enumerate() {
            if let Some(chord) = chord {
                chord.validate()?;
                if all[..i].contains(&Some(*chord)) {
                    return Err(ErrorCode::Malformed);
                }
            }
        }
        Ok(())
    }
}
