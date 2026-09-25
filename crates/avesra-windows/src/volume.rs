//! Native endpoint adapter. The caller owns durable dispatch and fresh policy.
//! Nothing in this module is exposed as an unrestricted webview command.
use avesra_contracts::ErrorCode;
use serde::Serialize;
use uuid::Uuid;
use windows::{
    Win32::{
        Media::Audio::{
            Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, IMMEndpoint, MMDeviceEnumerator,
            eRender,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
    },
    core::{GUID, Interface, PCWSTR},
};

pub struct VolumeTarget {
    pub id: Uuid,
    endpoint: String,
}
impl VolumeTarget {
    /// Resolve this from a native target registry, never a model-provided path.
    pub fn new(id: Uuid, endpoint: String) -> Result<Self, ErrorCode> {
        let parsed: cpal::DeviceId = endpoint.parse().map_err(|_| ErrorCode::Malformed)?;
        if id.is_nil()
            || parsed.0 != cpal::HostId::Wasapi
            || parsed.1.is_empty()
            || parsed.to_string() != endpoint
            || endpoint.len() > 1024
            || endpoint.chars().any(char::is_control)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Self { id, endpoint })
    }
}
#[derive(Clone, Serialize)]
pub struct VolumeSnapshot {
    pub scalar: f32,
    pub muted: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeOutcome {
    Verified,
    Uncertain,
}
#[derive(Serialize)]
pub struct VolumeChange {
    pub target_id: Uuid,
    pub before: VolumeSnapshot,
    pub after: Option<VolumeSnapshot>,
    pub outcome: VolumeOutcome,
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: Created only after a successful CoInitializeEx on this thread.
        unsafe {
            CoUninitialize();
        }
    }
}
fn with_endpoint<T>(
    target: &VolumeTarget,
    operation: impl FnOnce(&IAudioEndpointVolume) -> Result<T, ErrorCode>,
) -> Result<T, ErrorCode> {
    // SAFETY: This synchronous worker scope balances even S_FALSE initialization;
    // all COM interfaces are released before its apartment guard is dropped.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|_| ErrorCode::Unavailable)?;
    let _apartment = Apartment;
    let mut name: Vec<u16> = target
        .endpoint
        .strip_prefix("wasapi:")
        .ok_or(ErrorCode::Malformed)?
        .encode_utf16()
        .collect();
    name.push(0);
    // SAFETY: MMDeviceEnumerator is the documented local COM class; the owned
    // UTF16 string remains valid through GetDevice and interface activation.
    let endpoint = unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|_| ErrorCode::Unavailable)?;
        let device = enumerator
            .GetDevice(PCWSTR(name.as_ptr()))
            .map_err(|_| ErrorCode::Unavailable)?;
        let identity: IMMEndpoint = device.cast().map_err(|_| ErrorCode::Unavailable)?;
        if identity.GetDataFlow().map_err(|_| ErrorCode::Unavailable)? != eRender {
            return Err(ErrorCode::Denied);
        }
        device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .map_err(|_| ErrorCode::Unavailable)?
    };
    operation(&endpoint)
}
fn snapshot(endpoint: &IAudioEndpointVolume) -> Result<VolumeSnapshot, ErrorCode> {
    // SAFETY: The interface was activated in the current COM apartment.
    let (scalar, muted) = unsafe {
        (
            endpoint
                .GetMasterVolumeLevelScalar()
                .map_err(|_| ErrorCode::Unavailable)?,
            endpoint
                .GetMute()
                .map_err(|_| ErrorCode::Unavailable)?
                .as_bool(),
        )
    };
    if !scalar.is_finite() || !(0.0..=1.0).contains(&scalar) {
        return Err(ErrorCode::Malformed);
    }
    Ok(VolumeSnapshot { scalar, muted })
}
pub fn read(target: &VolumeTarget) -> Result<VolumeSnapshot, ErrorCode> {
    with_endpoint(target, snapshot)
}
/// Recheck current exact-action authority immediately before the single OS write.
/// An error after that boundary is uncertainty, never permission to retry.
pub fn set_percent(
    target: &VolumeTarget,
    percent: u8,
    dispatch: Uuid,
    authorize_commit: impl FnOnce() -> Result<(), ErrorCode>,
) -> Result<VolumeChange, ErrorCode> {
    if percent > 100 || dispatch.is_nil() {
        return Err(ErrorCode::Malformed);
    }
    with_endpoint(target, |endpoint| {
        let before = snapshot(endpoint)?;
        authorize_commit()?;
        let expected = f32::from(percent) / 100.0;
        let context = GUID::from_u128(dispatch.as_u128());
        // SAFETY: Valid endpoint, bounded normalized scalar and a live GUID. This
        // never calls SetMute or selects another/default endpoint.
        let applied = unsafe { endpoint.SetMasterVolumeLevelScalar(expected, &context) }.is_ok();
        let after = snapshot(endpoint).ok();
        let verified = applied
            && after.as_ref().is_some_and(|value| {
                (value.scalar - expected).abs() <= 0.0001 && value.muted == before.muted
            });
        Ok(VolumeChange {
            target_id: target.id,
            before,
            after,
            outcome: if verified {
                VolumeOutcome::Verified
            } else {
                VolumeOutcome::Uncertain
            },
        })
    })
}
