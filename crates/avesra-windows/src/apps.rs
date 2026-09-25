//! Explicit native identities and bounded launch observation. No public IPC.
use avesra_contracts::{ErrorCode, Outcome};
use avesra_core::{
    apps::{AppRecord, AppSource, ExecutableIdentity, LaunchIdentity, local_path},
    execution::{EffectObservation, EffectResult},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;
use windows::{
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM},
        Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_NAME_NORMALIZED, GetFileInformationByHandle,
            GetFinalPathNameByHandleW,
        },
        System::Threading::{PROCESS_NAME_WIN32, QueryFullProcessImageNameW},
        UI::{
            Input::KeyboardAndMouse::IsWindowEnabled,
            WindowsAndMessaging::{
                EnumWindows, GW_OWNER, GetClassNameW, GetWindow, GetWindowThreadProcessId,
                IsWindow, IsWindowVisible,
            },
        },
    },
    core::{BOOL, PWSTR},
};

fn path_of(file: &File) -> Result<String, ErrorCode> {
    let mut buf = [0u16; 4096];
    let len = unsafe {
        GetFinalPathNameByHandleW(HANDLE(file.as_raw_handle()), &mut buf, FILE_NAME_NORMALIZED)
    } as usize;
    if len == 0 || len >= buf.len() {
        return Err(ErrorCode::Unavailable);
    }
    let path = String::from_utf16(&buf[..len]).map_err(|_| ErrorCode::Malformed)?;
    if !local_path(&path) {
        return Err(ErrorCode::Denied);
    }
    Ok(path)
}
fn identity(
    file: &mut File,
    arguments: String,
    working_directory: String,
) -> Result<ExecutableIdentity, ErrorCode> {
    let metadata = file.metadata().map_err(|_| ErrorCode::Unavailable)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 1_073_741_824 {
        return Err(ErrorCode::TooLarge);
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(|_| ErrorCode::Unavailable)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| ErrorCode::Unavailable)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer).map_err(|_| ErrorCode::Unavailable)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > metadata.len() {
            return Err(ErrorCode::Stale);
        }
        hash.update(&buffer[..count]);
    }
    if total != metadata.len() {
        return Err(ErrorCode::Stale);
    }
    Ok(ExecutableIdentity {
        path: path_of(file)?,
        arguments,
        working_directory,
        sha256: format!("{:x}", hash.finalize()),
        bytes: total,
        volume: info.dwVolumeSerialNumber,
        file_index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}
fn open_executable(path: &str) -> Result<File, ErrorCode> {
    if !local_path(path) || !path.to_ascii_lowercase().ends_with(".exe") {
        return Err(ErrorCode::Denied);
    }
    // Read sharing permits Windows' loader, while refusing write/delete races.
    OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|_| ErrorCode::Unavailable)
}
fn open_directory(path: &str) -> Result<File, ErrorCode> {
    if !local_path(path) {
        return Err(ErrorCode::Denied);
    }
    let file = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(0x02000000)
        .open(path)
        .map_err(|_| ErrorCode::Unavailable)?;
    if !file
        .metadata()
        .map_err(|_| ErrorCode::Unavailable)?
        .is_dir()
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(file)
}
/// Call only after the native owner explicitly selected this executable and
/// arguments. Discovery and selection never execute the candidate.
pub fn select_executable(
    owner: Uuid,
    name: String,
    path: &Path,
    arguments: String,
    working_directory: &Path,
) -> Result<AppRecord, ErrorCode> {
    let path = path.to_str().ok_or(ErrorCode::Malformed)?;
    let working_directory = working_directory.to_str().ok_or(ErrorCode::Malformed)?;
    let mut executable = open_executable(path)?;
    let directory = open_directory(working_directory)?;
    let record = AppRecord {
        id: Uuid::new_v4(),
        revision: Uuid::new_v4(),
        selected_by: owner,
        name,
        source: AppSource::ExplicitExecutable,
        source_identity: Some(path.to_string()),
        publisher: None,
        window_class: None,
        launch: LaunchIdentity::Executable(identity(
            &mut executable,
            arguments,
            path_of(&directory)?,
        )?),
    };
    record.validate()?;
    Ok(record)
}
struct Windows {
    pid: u32,
    class: Option<String>,
    seen: usize,
    matched: Vec<u64>,
    overflow: bool,
}
fn matches_window(window: HWND, pid: u32, class: &Option<String>) -> bool {
    let Some(expected) = class else {
        return false;
    };
    let mut actual_pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut actual_pid));
    }
    if actual_pid != pid
        || !unsafe { IsWindow(Some(window)) }.as_bool()
        || !unsafe { IsWindowVisible(window) }.as_bool()
        || !unsafe { IsWindowEnabled(window) }.as_bool()
        || unsafe { GetWindow(window, GW_OWNER) }.is_ok()
    {
        return false;
    }
    let mut cloaked = 0u32;
    if unsafe { DwmGetWindowAttribute(window, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4) }
        .is_err()
        || cloaked != 0
    {
        return false;
    }
    let mut class = [0u16; 257];
    let count = unsafe { GetClassNameW(window, &mut class) };
    count > 0
        && count < 256
        && String::from_utf16(&class[..count as usize]).ok().as_ref() == Some(expected)
}
unsafe extern "system" fn visit(window: HWND, value: LPARAM) -> BOOL {
    // EnumWindows calls synchronously while the stack-owned context is alive.
    let state = unsafe { &mut *(value.0 as *mut Windows) };
    state.seen += 1;
    if state.seen > 1024 {
        state.overflow = true;
        return BOOL(0);
    }
    if !matches_window(window, state.pid, &state.class) {
        return BOOL(1);
    }
    if state.matched.len() == 2 {
        state.overflow = true;
        return BOOL(0);
    }
    state.matched.push(window.0 as usize as u64);
    BOOL(1)
}
pub fn launch(
    record: &AppRecord,
    authorize_commit: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<EffectResult, ErrorCode> {
    record.validate()?;
    let LaunchIdentity::Executable(expected) = &record.launch else {
        return Ok(EffectResult {
            outcome: Outcome::Unsupported,
            observation: None,
        });
    };
    let mut executable = open_executable(&expected.path)?;
    let directory = open_directory(&expected.working_directory)?;
    if path_of(&directory)? != expected.working_directory
        || identity(
            &mut executable,
            expected.arguments.clone(),
            expected.working_directory.clone(),
        )? != *expected
    {
        return Err(ErrorCode::Stale);
    }
    let mut command = Command::new(&expected.path);
    command
        .raw_arg(&expected.arguments)
        .current_dir(&expected.working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    authorize_commit()?;
    let Ok(mut child) = command.spawn() else {
        return Ok(EffectResult {
            outcome: Outcome::UnknownEffect,
            observation: Some(EffectObservation::Application {
                app_id: record.id,
                catalog_revision: record.revision,
                process_id: None,
                window: None,
                image_matched: false,
            }),
        });
    };
    let pid = child.id();
    let started = Instant::now();
    let mut observed = None;
    let mut image_matched = false;
    let mut image = [0u16; 4096];
    let mut length = image.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(
            HANDLE(child.as_raw_handle()),
            PROCESS_NAME_WIN32,
            PWSTR(image.as_mut_ptr()),
            &mut length,
        )
    }
    .is_ok()
        && length > 0
        && (length as usize) < image.len()
        && let Ok(path) = String::from_utf16(&image[..length as usize])
    {
        image_matched = path
            .trim_start_matches(r"\\?\")
            .eq_ignore_ascii_case(expected.path.trim_start_matches(r"\\?\"));
    }
    while image_matched
        && record.window_class.is_some()
        && started.elapsed() < Duration::from_secs(3)
    {
        if !matches!(child.try_wait(), Ok(None)) {
            break;
        }
        let mut windows = Windows {
            pid,
            class: record.window_class.clone(),
            seen: 0,
            matched: Vec::with_capacity(2),
            overflow: false,
        };
        let result =
            unsafe { EnumWindows(Some(visit), LPARAM((&mut windows as *mut Windows) as isize)) };
        if result.is_err() || windows.overflow || windows.matched.len() > 1 {
            break;
        }
        if let Some(window) = windows.matched.first() {
            if matches!(child.try_wait(), Ok(None))
                && matches_window(
                    HWND((*window as usize) as *mut _),
                    pid,
                    &record.window_class,
                )
            {
                observed = Some(*window);
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(EffectResult {
        outcome: if observed.is_some() {
            Outcome::Success
        } else {
            Outcome::UnknownEffect
        },
        observation: Some(EffectObservation::Application {
            app_id: record.id,
            catalog_revision: record.revision,
            process_id: Some(pid),
            window: observed,
            image_matched,
        }),
    })
}
