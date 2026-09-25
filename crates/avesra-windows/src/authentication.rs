//! OS-backed local management verification. Never called automatically.
use avesra_contracts::ErrorCode;
use windows::{
    Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
    },
    Win32::{Foundation::HWND, System::WinRT::IUserConsentVerifierInterop},
    core::{HSTRING, factory},
};
use windows_future::{AsyncStatus, IAsyncOperation};

#[derive(Clone)]
pub struct Verification(IAsyncOperation<UserConsentVerificationResult>);
impl Verification {
    pub fn cancel(&self) {
        let _ = self.0.Cancel();
    }
    pub fn running(&self) -> bool {
        self.0
            .Status()
            .map_or(true, |status| status == AsyncStatus::Started)
    }
    pub async fn verified(&self) -> Result<(), ErrorCode> {
        match self.0.clone().await.map_err(|_| ErrorCode::Unavailable)? {
            UserConsentVerificationResult::Verified => Ok(()),
            _ => Err(ErrorCode::Denied),
        }
    }
}

pub async fn available() -> Result<bool, ErrorCode> {
    Ok(UserConsentVerifier::CheckAvailabilityAsync()
        .map_err(|_| ErrorCode::Unavailable)?
        .await
        .map_err(|_| ErrorCode::Unavailable)?
        == UserConsentVerifierAvailability::Available)
}

/// The caller supplies its own live Settings window, never a web argument.
pub fn request_for_settings(handle: isize) -> Result<Verification, ErrorCode> {
    let hwnd = HWND(handle as *mut core::ffi::c_void);
    if hwnd.is_invalid() {
        return Err(ErrorCode::Denied);
    }
    let verifier: IUserConsentVerifierInterop =
        factory::<UserConsentVerifier, IUserConsentVerifierInterop>()
            .map_err(|_| ErrorCode::Unavailable)?;
    // SAFETY: HWND is supplied by the native Tauri Settings window; WinRT owns
    // the returned operation and the HSTRING is valid throughout this call.
    unsafe {
        verifier.RequestVerificationForWindowAsync(
            hwnd,
            &HSTRING::from(
                "Verify your Windows user to manage Avesra ownership, people and permissions.",
            ),
        )
    }
    .map(Verification)
    .map_err(|_| ErrorCode::Unavailable)
}
