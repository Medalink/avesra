//! Numeric native PC-clock observations; validation is not source authentication.
use crate::ErrorCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Time,
    Date,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalReading {
    pub observed_unix_ms: u64,
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}
impl LocalReading {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let leap = self.year.is_multiple_of(4)
            && (!self.year.is_multiple_of(100) || self.year.is_multiple_of(400));
        let days = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return Err(ErrorCode::Malformed),
        };
        if !(1601..=9999).contains(&self.year)
            || !(1..=days).contains(&self.day)
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
            || self.observed_unix_ms == 0
            || self.observed_unix_ms > 253_402_300_799_999
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
    pub fn answer(&self, kind: Kind) -> Result<String, ErrorCode> {
        self.validate()?;
        Ok(match kind {
            Kind::Time => {
                let hour = match self.hour % 12 {
                    0 => 12,
                    value => value,
                };
                let period = if self.hour < 12 { "AM" } else { "PM" };
                format!("Your PC clock says {hour}:{:02} {period}.", self.minute)
            }
            Kind::Date => {
                let months = [
                    "January",
                    "February",
                    "March",
                    "April",
                    "May",
                    "June",
                    "July",
                    "August",
                    "September",
                    "October",
                    "November",
                    "December",
                ];
                format!(
                    "Your PC clock says {} {}, {}.",
                    months[usize::from(self.month - 1)],
                    self.day,
                    self.year
                )
            }
        })
    }
}
