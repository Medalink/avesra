//! Built-in clock replies retain their actual native observation lifetime.
use avesra_contracts::{
    ErrorCode,
    clock::{Kind, LocalReading},
};
use std::time::{Duration, Instant};

pub fn question_kind(text: &str) -> Option<Kind> {
    if text.len() > 256 {
        return None;
    }
    let normalized = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let text = normalized.trim_end_matches(['.', '?', '!']);
    let text = text
        .strip_prefix("avesra, ")
        .or_else(|| text.strip_prefix("avesra "))
        .unwrap_or(text);
    let text = text
        .strip_prefix("please ")
        .or_else(|| text.strip_suffix(" please"))
        .unwrap_or(text);
    match text {
        "what time is it" | "what is the time" | "what's the time" | "tell me the time" => {
            Some(Kind::Time)
        }
        "what is today's date"
        | "what's today's date"
        | "what is the date"
        | "what's the date"
        | "what day is it"
        | "tell me the date" => Some(Kind::Date),
        _ => None,
    }
}

/// Constructed by the native OS adapter, never deserialized from IPC/history.
pub struct Observation {
    reading: LocalReading,
    observed: Instant,
}
impl Observation {
    pub fn new(reading: LocalReading, observed: Instant) -> Result<Self, ErrorCode> {
        reading.validate()?;
        let value = Self { reading, observed };
        value.current()?;
        Ok(value)
    }
    pub(crate) fn current(&self) -> Result<(), ErrorCode> {
        if Instant::now()
            .checked_duration_since(self.observed)
            .is_none_or(|age| age > Duration::from_secs(2))
        {
            return Err(ErrorCode::Expired);
        }
        Ok(())
    }
    pub(crate) fn reading(&self) -> Result<LocalReading, ErrorCode> {
        self.current()?;
        Ok(self.reading.clone())
    }
}
