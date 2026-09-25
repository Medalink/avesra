use avesra_contracts::ErrorCode;
use serde::Serialize;
#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod authentication;
pub mod credentials;
#[cfg(windows)]
pub mod effects;
#[cfg(windows)]
mod playback;
#[cfg(windows)]
mod resampling;
#[cfg(windows)]
pub mod session;
#[cfg(windows)]
pub mod shortcuts;
#[cfg(windows)]
pub mod volume;

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
            let name = device
                .description()
                .map_err(|_| ErrorCode::Unavailable)?
                .name()
                .to_string();
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
pub mod discovery;
#[cfg(windows)]
pub mod packages;
#[cfg(windows)]
pub mod principal;
