use avesra_contracts::ErrorCode;

#[cfg(windows)]
fn transform(bytes: &[u8], protect: bool) -> Result<Vec<u8>, ErrorCode> {
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::Cryptography::{
                CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
            },
        },
        core::PCWSTR,
    };
    if bytes.is_empty() || bytes.len() > 65_536 {
        return Err(ErrorCode::Malformed);
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: input points to a live bounded slice. DPAPI owns the returned allocation;
    // it is copied before LocalFree. No machine-wide flag is used: protection is per user.
    unsafe {
        let result = if protect {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        result.map_err(|_| ErrorCode::Unauthenticated)?;
        if output.pbData.is_null() || output.cbData == 0 {
            return Err(ErrorCode::Malformed);
        }
        if output.cbData > 131_072 {
            let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
            return Err(ErrorCode::TooLarge);
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        // Clear the owned DPAPI allocation before freeing it, including decrypted
        // browser credentials; callers separately own and clear their returned copy.
        use zeroize::Zeroize;
        std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize).zeroize();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(result)
    }
}
#[cfg(windows)]
pub fn protect(bytes: &[u8]) -> Result<Vec<u8>, ErrorCode> {
    transform(bytes, true)
}
#[cfg(windows)]
pub fn unprotect(bytes: &[u8]) -> Result<Vec<u8>, ErrorCode> {
    transform(bytes, false)
}
#[cfg(not(windows))]
pub fn protect(_bytes: &[u8]) -> Result<Vec<u8>, ErrorCode> {
    Err(ErrorCode::Unsupported)
}
#[cfg(not(windows))]
pub fn unprotect(_bytes: &[u8]) -> Result<Vec<u8>, ErrorCode> {
    Err(ErrorCode::Unsupported)
}
