//! Native current-process principal, never supplied by frontend/model text.
use avesra_contracts::ErrorCode;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, IsValidSid, TOKEN_QUERY,
            TOKEN_USER, TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
    core::PWSTR,
};
struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}
pub fn current_user() -> Result<String, ErrorCode> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|_| ErrorCode::Unauthenticated)?;
    let token = Token(token);
    let mut required = 0;
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut required) };
    if required < std::mem::size_of::<TOKEN_USER>() as u32 || required > 4096 {
        return Err(ErrorCode::Malformed);
    }
    let mut storage = vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            required,
            &mut required,
        )
    }
    .map_err(|_| ErrorCode::Unauthenticated)?;
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    if !unsafe { IsValidSid(user.User.Sid) }.as_bool() {
        return Err(ErrorCode::Malformed);
    }
    let mut raw = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut raw) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let value = unsafe { raw.to_string() };
    let _ = unsafe { LocalFree(Some(HLOCAL(raw.0.cast()))) };
    let value = value.map_err(|_| ErrorCode::Malformed)?;
    if !valid_sid(&value) {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}
pub fn valid_sid(value: &str) -> bool {
    value.starts_with("S-1-")
        && value.len() <= 184
        && value[4..]
            .split('-')
            .all(|v| !v.is_empty() && v.bytes().all(|v| v.is_ascii_digit()))
}
