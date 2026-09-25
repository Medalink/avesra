//! Bounded metadata discovery. No Resolve, ShellExecute, process or UI calls.
use avesra_contracts::ErrorCode;
use avesra_core::apps::{AppRecord, AppSource, local_path};
use serde::Serialize;
use std::{
    fs::OpenOptions,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;
use windows::{
    Win32::{
        Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize, IPersistFile, STGM_READ, STGM_SHARE_DENY_WRITE,
            },
            Registry::{
                HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY,
                KEY_WOW64_64KEY, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW,
                RegOpenKeyExW,
            },
        },
        UI::Shell::{
            FOLDERID_CommonPrograms, FOLDERID_Programs, IShellLinkW, KF_FLAG_DONT_VERIFY,
            SHGetKnownFolderPath, SLGP_RAWPATH, ShellLink,
        },
    },
    core::{Interface, PCWSTR, PWSTR},
};

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub name: String,
    pub source: AppSource,
    pub executable: String,
    pub arguments: String,
    pub working_directory: Option<String>,
    pub registered_search_path: Option<String>,
    pub source_identity: String,
    pub selectable: bool,
}
impl Candidate {
    /// Native selection supplies an explicit cwd where registration omitted it.
    /// Owner confirmation applies to this exact discovered argument snapshot.
    pub fn select(&self, owner: Uuid, working_directory: &Path) -> Result<AppRecord, ErrorCode> {
        if !self.selectable {
            return Err(ErrorCode::Unsupported);
        }
        let fresh = match self.source {
            AppSource::StartMenu => {
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                    .ok()
                    .map_err(|_| ErrorCode::Unavailable)?;
                let _apartment = Apartment;
                link(Path::new(&self.source_identity))?
            }
            AppSource::WindowsRegistration => registered_apps()?
                .candidates
                .into_iter()
                .find(|v| v.source_identity == self.source_identity)
                .ok_or(ErrorCode::Stale)?,
            _ => return Err(ErrorCode::Unsupported),
        };
        if fresh != *self {
            return Err(ErrorCode::Stale);
        }
        if self
            .working_directory
            .as_ref()
            .is_some_and(|v| Path::new(v) != working_directory)
        {
            return Err(ErrorCode::Stale);
        }
        let mut record = crate::apps::select_executable(
            owner,
            self.name.clone(),
            Path::new(&self.executable),
            self.arguments.clone(),
            working_directory,
        )?;
        record.source = self.source.clone();
        record.source_identity = Some(self.source_identity.clone());
        Ok(record)
    }
}
#[derive(Default, Serialize)]
pub struct Discovery {
    pub candidates: Vec<Candidate>,
    pub skipped: u32,
    pub truncated: bool,
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}
fn decode(buf: &[u16]) -> Result<String, ErrorCode> {
    let end = buf
        .iter()
        .position(|v| *v == 0)
        .ok_or(ErrorCode::TooLarge)?;
    if end + 1 >= buf.len() {
        return Err(ErrorCode::TooLarge);
    }
    if buf[end + 1..].iter().any(|v| *v != 0) {
        return Err(ErrorCode::Malformed);
    }
    let value = String::from_utf16(&buf[..end]).map_err(|_| ErrorCode::Malformed)?;
    if value.chars().any(char::is_control) {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}
fn link(path: &Path) -> Result<Candidate, ErrorCode> {
    let name = path
        .file_stem()
        .and_then(|v| v.to_str())
        .filter(|v| !v.is_empty() && v.len() <= 256 && !v.chars().any(char::is_control))
        .ok_or(ErrorCode::Malformed)?
        .to_string();
    let source = path
        .to_str()
        .filter(|v| local_path(v))
        .ok_or(ErrorCode::Malformed)?;
    let held = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000) // FILE_FLAG_OPEN_REPARSE_POINT: inspect the link itself.
        .open(path)
        .map_err(|_| ErrorCode::Unavailable)?;
    let metadata = held.metadata().map_err(|_| ErrorCode::Unavailable)?;
    if !metadata.is_file() || metadata.len() > 2_097_152 || metadata.file_attributes() & 0x400 != 0
    {
        return Err(ErrorCode::TooLarge);
    }
    let shell: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let persist: IPersistFile = shell.cast().map_err(|_| ErrorCode::Unavailable)?;
    let wide = source.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    unsafe { persist.Load(PCWSTR(wide.as_ptr()), STGM_READ | STGM_SHARE_DENY_WRITE) }
        .map_err(|_| ErrorCode::Unsupported)?;
    let mut executable = [0u16; 4097];
    let mut arguments = [0u16; 8193];
    let mut cwd = [0u16; 4097];
    unsafe { shell.GetPath(&mut executable, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32) }
        .map_err(|_| ErrorCode::Unsupported)?;
    unsafe { shell.GetArguments(&mut arguments) }.map_err(|_| ErrorCode::Unsupported)?;
    unsafe { shell.GetWorkingDirectory(&mut cwd) }.map_err(|_| ErrorCode::Unsupported)?;
    let executable = decode(&executable)?;
    // GetPath's documented MAX_PATH ceiling cannot prove a full long target.
    if executable.encode_utf16().count() >= 259 {
        return Err(ErrorCode::Unsupported);
    }
    let arguments = decode(&arguments)?;
    let cwd = decode(&cwd)?;
    if !local_path(&executable)
        || !executable.to_ascii_lowercase().ends_with(".exe")
        || (!cwd.is_empty() && !local_path(&cwd))
    {
        return Err(ErrorCode::Unsupported);
    }
    Ok(Candidate {
        name,
        source: AppSource::StartMenu,
        executable,
        arguments,
        working_directory: if cwd.is_empty() { None } else { Some(cwd) },
        registered_search_path: None,
        source_identity: source.into(),
        selectable: true,
    })
}
/// Dedicated background thread only; bounds are count/depth and a monotonic
/// admission budget, not a claim that individual filesystem/COM calls cancel.
pub fn start_menu() -> Result<Discovery, ErrorCode> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(|_| ErrorCode::Unavailable)?;
    let _apartment = Apartment;
    let mut result = Discovery::default();
    let started = Instant::now();
    let mut queue: Vec<(PathBuf, usize)> = Vec::new();
    let mut seen = 0;
    for folder in [FOLDERID_Programs, FOLDERID_CommonPrograms] {
        if started.elapsed() >= Duration::from_secs(5) {
            result.truncated = true;
            return Ok(result);
        }
        let raw = unsafe { SHGetKnownFolderPath(&folder, KF_FLAG_DONT_VERIFY, None) }
            .map_err(|_| ErrorCode::Unavailable)?;
        let value = unsafe { raw.to_string() };
        unsafe {
            CoTaskMemFree(Some(raw.0.cast()));
        }
        let value = value.map_err(|_| ErrorCode::Malformed)?;
        if local_path(&value) {
            queue.push((PathBuf::from(value), 0));
        } else {
            result.skipped += 1;
        }
    }
    while let Some((directory, depth)) = queue.pop() {
        if started.elapsed() >= Duration::from_secs(5) {
            result.truncated = true;
            return Ok(result);
        }
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(v) => v,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        if metadata.file_attributes() & 0x400 != 0 {
            result.skipped += 1;
            continue;
        }
        let entries = match std::fs::read_dir(directory) {
            Ok(v) => v,
            Err(_) => {
                result.skipped += 1;
                continue;
            }
        };
        for entry in entries {
            seen += 1;
            if seen > 2048
                || result.candidates.len() >= 256
                || started.elapsed() >= Duration::from_secs(5)
            {
                result.truncated = true;
                return Ok(result);
            }
            let Ok(entry) = entry else {
                result.skipped += 1;
                continue;
            };
            let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
                result.skipped += 1;
                continue;
            };
            if metadata.file_attributes() & 0x400 != 0 {
                result.skipped += 1;
                continue;
            }
            if metadata.is_dir() {
                if depth < 4 {
                    queue.push((entry.path(), depth + 1));
                } else {
                    result.truncated = true;
                }
                continue;
            }
            if entry
                .path()
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("lnk"))
            {
                match link(&entry.path()) {
                    Ok(candidate) => result.candidates.push(candidate),
                    Err(_) => result.skipped += 1,
                }
            }
        }
    }
    Ok(result)
}
fn registry_text(key: HKEY, name: PCWSTR) -> Result<Option<String>, ErrorCode> {
    let mut buf = [0u16; 4097];
    let mut bytes = (buf.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            name,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if result == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if result != ERROR_SUCCESS {
        return Err(ErrorCode::Unsupported);
    }
    if bytes == 0 || !bytes.is_multiple_of(2) || bytes as usize > buf.len() * 2 {
        return Err(ErrorCode::Malformed);
    }
    decode(&buf).map(Some)
}
pub fn registered_apps() -> Result<Discovery, ErrorCode> {
    let mut result = Discovery::default();
    let started = Instant::now();
    let path = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\App Paths");
    for (root, label) in [(HKEY_CURRENT_USER, "HKCU"), (HKEY_LOCAL_MACHINE, "HKLM")] {
        for (view, bits) in [(KEY_WOW64_64KEY, "64"), (KEY_WOW64_32KEY, "32")] {
            if started.elapsed() >= Duration::from_secs(5) {
                result.truncated = true;
                return Ok(result);
            }
            let mut key = HKEY::default();
            if unsafe { RegOpenKeyExW(root, path, None, KEY_READ | view, &mut key) }
                != ERROR_SUCCESS
            {
                result.skipped += 1;
                continue;
            }
            let key = Key(key);
            for index in 0..512 {
                if result.candidates.len() >= 256 || started.elapsed() >= Duration::from_secs(5) {
                    result.truncated = true;
                    return Ok(result);
                }
                let mut name = [0u16; 257];
                let mut size = 256;
                let status = unsafe {
                    RegEnumKeyExW(
                        key.0,
                        index,
                        Some(PWSTR(name.as_mut_ptr())),
                        &mut size,
                        None,
                        None,
                        None,
                        None,
                    )
                };
                if status == ERROR_NO_MORE_ITEMS {
                    break;
                }
                if index == 511 {
                    result.truncated = true;
                }
                if status != ERROR_SUCCESS {
                    result.skipped += 1;
                    continue;
                }
                let name_text = match decode(&name) {
                    Ok(v) => v,
                    Err(_) => {
                        result.skipped += 1;
                        continue;
                    }
                };
                let mut child = HKEY::default();
                if unsafe {
                    RegOpenKeyExW(
                        key.0,
                        PCWSTR(name.as_ptr()),
                        None,
                        KEY_READ | view,
                        &mut child,
                    )
                } != ERROR_SUCCESS
                {
                    result.skipped += 1;
                    continue;
                }
                let child = Key(child);
                let executable = match registry_text(child.0, PCWSTR::null()) {
                    Ok(Some(v)) => v,
                    _ => {
                        result.skipped += 1;
                        continue;
                    }
                };
                let search_path = match registry_text(child.0, windows::core::w!("Path")) {
                    Ok(v) => v,
                    Err(_) => {
                        result.skipped += 1;
                        continue;
                    }
                };
                if !local_path(&executable) || !executable.to_ascii_lowercase().ends_with(".exe") {
                    result.skipped += 1;
                    continue;
                }
                let selectable = search_path.as_ref().is_none_or(String::is_empty);
                result.candidates.push(Candidate {
                    name: name_text.clone(),
                    source: AppSource::WindowsRegistration,
                    executable,
                    arguments: String::new(),
                    working_directory: None,
                    registered_search_path: search_path,
                    source_identity: format!("{label}/{bits}/App Paths/{name_text}"),
                    selectable,
                });
                if index == 511 {
                    result.truncated = true;
                }
            }
        }
    }
    Ok(result)
}
