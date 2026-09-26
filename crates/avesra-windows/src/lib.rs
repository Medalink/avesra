use avesra_contracts::ErrorCode;
use serde::Serialize;
#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod authentication;
#[cfg(windows)]
mod clock;
pub mod credentials;
#[cfg(windows)]
pub mod diagnostics;
#[cfg(windows)]
pub mod download;
#[cfg(windows)]
pub mod effects;
#[cfg(windows)]
pub mod output_recording;
#[cfg(windows)]
mod playback;
#[cfg(windows)]
pub mod prompt;
#[cfg(windows)]
mod resampling;
#[cfg(windows)]
pub mod session;
#[cfg(windows)]
pub mod shortcuts;
#[cfg(windows)]
pub mod sound;
#[cfg(windows)]
pub mod volume;
#[cfg(windows)]
pub mod vpn;

#[derive(Debug, Clone, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub direction: &'static str,
    pub is_default: bool,
}

#[cfg(windows)]
pub fn audio_devices() -> Result<Vec<AudioDevice>, ErrorCode> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let mut devices = Vec::new();
    for (direction, items, default) in [
        ("input", host.input_devices(), host.default_input_device()),
        (
            "output",
            host.output_devices(),
            host.default_output_device(),
        ),
    ] {
        let default_id = default.and_then(|device| device.id().ok());
        for device in items.map_err(|_| ErrorCode::Unavailable)? {
            let description = device.description().map_err(|_| ErrorCode::Unavailable)?;
            // CPAL's WASAPI name is often just "Microphone". Its extended
            // description retains Windows' full endpoint friendly name.
            // Display metadata must never replace the stable selection ID.
            let name = description
                .extended()
                .first()
                .map(|name| name.trim())
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    match description
                        .driver()
                        .map(str::trim)
                        .filter(|driver| !driver.is_empty() && *driver != description.name())
                    {
                        Some(driver) => format!("{} ({driver})", description.name()),
                        None => description.name().to_owned(),
                    }
                });
            let id = device.id().map_err(|_| ErrorCode::Unavailable)?;
            devices.push(AudioDevice {
                is_default: default_id.as_ref() == Some(&id),
                id: id.to_string(),
                name,
                direction,
            });
        }
    }
    Ok(devices)
}
#[cfg(not(windows))]
pub fn audio_devices() -> Result<Vec<AudioDevice>, ErrorCode> {
    Err(ErrorCode::Unsupported)
}
#[cfg(windows)]
pub mod apps;
#[cfg(windows)]
pub mod browser_pairing;
#[cfg(windows)]
pub mod browser_pipe;
#[cfg(windows)]
pub mod browser_read_channel;
#[cfg(windows)]
pub mod browser_receive;
#[cfg(windows)]
pub mod discovery;
#[cfg(windows)]
pub mod packages;
#[cfg(windows)]
pub mod principal;
