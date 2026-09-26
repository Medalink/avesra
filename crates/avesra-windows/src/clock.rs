use avesra_contracts::{ErrorCode, clock::LocalReading};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) fn observe() -> Result<avesra_core::clock::Observation, ErrorCode> {
    let observed = Instant::now();
    let value = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let observed_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ErrorCode::Unavailable)?
        .as_millis()
        .try_into()
        .map_err(|_| ErrorCode::Malformed)?;
    avesra_core::clock::Observation::new(
        LocalReading {
            observed_unix_ms,
            year: value.wYear,
            month: value.wMonth,
            day: value.wDay,
            hour: value.wHour,
            minute: value.wMinute,
            second: value.wSecond,
        },
        observed,
    )
}
