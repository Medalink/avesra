//! Native current-process principal, never supplied by frontend/model text.
use avesra_contracts::ErrorCode;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, IsValidSid,
            SID_AND_ATTRIBUTES, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_USER, TokenGroups, TokenSessionId,
            TokenUser,
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
    process_user(unsafe { GetCurrentProcess() })
}
fn process_user(process: HANDLE) -> Result<String, ErrorCode> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
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
fn session(process: HANDLE) -> Result<u32, ErrorCode> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
        .map_err(|_| ErrorCode::Unauthenticated)?;
    let token = Token(token);
    let mut value = 0u32;
    let mut returned = 0;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenSessionId,
            Some((&mut value as *mut u32).cast()),
            4,
            &mut returned,
        )
    }
    .map_err(|_| ErrorCode::Unauthenticated)?;
    if returned != 4 {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}
pub(crate) fn same_process_context(process: HANDLE) -> Result<bool, ErrorCode> {
    let current = unsafe { GetCurrentProcess() };
    Ok(process_user(process)? == process_user(current)? && session(process)? == session(current)?)
}
pub(crate) fn current_session() -> Result<u32, ErrorCode> {
    session(unsafe { GetCurrentProcess() })
}
pub(crate) fn current_logon_sid() -> Result<String, ErrorCode> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|_| ErrorCode::Unauthenticated)?;
    let token = Token(token);
    let mut required = 0;
    let _ = unsafe { GetTokenInformation(token.0, TokenGroups, None, 0, &mut required) };
    if required < std::mem::size_of::<TOKEN_GROUPS>() as u32 || required > 65536 {
        return Err(ErrorCode::Malformed);
    }
    let mut storage = vec![0usize; (required as usize).div_ceil(std::mem::size_of::<usize>())];
    let capacity = required;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenGroups,
            Some(storage.as_mut_ptr().cast()),
            capacity,
            &mut required,
        )
    }
    .map_err(|_| ErrorCode::Unavailable)?;
    let groups = unsafe { &*storage.as_ptr().cast::<TOKEN_GROUPS>() };
    let count = groups.GroupCount as usize;
    let offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    if required > capacity
        || count > 1024
        || offset + count * std::mem::size_of::<SID_AND_ATTRIBUTES>() > required as usize
    {
        return Err(ErrorCode::Malformed);
    }
    let values = unsafe {
        std::slice::from_raw_parts(
            storage
                .as_ptr()
                .cast::<u8>()
                .add(offset)
                .cast::<SID_AND_ATTRIBUTES>(),
            count,
        )
    };
    let mut result = None;
    for value in values
        .iter()
        .filter(|v| v.Attributes & 0xc000_0000 == 0xc000_0000)
    {
        if result.is_some() || !unsafe { IsValidSid(value.Sid) }.as_bool() {
            return Err(ErrorCode::Malformed);
        }
        let mut raw = PWSTR::null();
        unsafe { ConvertSidToStringSidW(value.Sid, &mut raw) }
            .map_err(|_| ErrorCode::Unavailable)?;
        let text = unsafe { raw.to_string() };
        unsafe {
            LocalFree(Some(HLOCAL(raw.0.cast())));
        }
        let text = text.map_err(|_| ErrorCode::Malformed)?;
        if !valid_sid(&text) || !text.starts_with("S-1-5-5-") {
            return Err(ErrorCode::Malformed);
        }
        result = Some(text);
    }
    result.ok_or(ErrorCode::Unauthenticated)
}
pub fn valid_sid(value: &str) -> bool {
    value.starts_with("S-1-")
        && value.len() <= 184
        && value[4..]
            .split('-')
            .all(|v| !v.is_empty() && v.bytes().all(|v| v.is_ascii_digit()))
}
