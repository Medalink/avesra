use avesra_contracts::ErrorCode;
use serde::Serialize;
#[cfg(windows)]
pub mod audio;
pub mod credentials;

#[derive(Debug, Clone, Serialize)]
pub struct AudioDevice {
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
        let default_name = default
            .and_then(|device| device.description().ok())
            .map(|d| d.name().to_string());
        for device in items.map_err(|_| ErrorCode::Unavailable)? {
            let name = device
                .description()
                .map_err(|_| ErrorCode::Unavailable)?
                .name()
                .to_string();
            devices.push(AudioDevice {
                is_default: default_name.as_ref() == Some(&name),
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
