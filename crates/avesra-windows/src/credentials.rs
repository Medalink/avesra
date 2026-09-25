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
    struct Allocation(CRYPT_INTEGER_BLOB);
    impl Drop for Allocation {
        fn drop(&mut self) {
            if !self.0.pbData.is_null() {
                // SAFETY: only a successful DPAPI result constructs this owner;
                // cbData describes its entire allocation, including rejected output.
                unsafe {
                    use zeroize::Zeroize;
                    std::slice::from_raw_parts_mut(self.0.pbData, self.0.cbData as usize).zeroize();
                    let _ = LocalFree(Some(HLOCAL(self.0.pbData.cast())));
                }
            }
        }
    }
    // SAFETY: input points to a live bounded slice. The owner clears/frees every
    // successful DPAPI output. No machine-wide flag is used: protection is per user.
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
        let allocation = Allocation(output);
        let output = &allocation.0;
        if output.pbData.is_null() || output.cbData == 0 {
            return Err(ErrorCode::Malformed);
        }
        if output.cbData > 131_072 {
            return Err(ErrorCode::TooLarge);
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
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
