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
        Foundation::{
            APPMODEL_ERROR_NO_APPLICATION, APPMODEL_ERROR_NO_PACKAGE, CloseHandle,
            ERROR_NO_MORE_FILES, ERROR_SUCCESS, FILETIME, HANDLE, HWND, LPARAM, WAIT_TIMEOUT,
        },
        Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        Storage::{
            FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_NAME_NORMALIZED, GetFileInformationByHandle,
                GetFinalPathNameByHandleW,
            },
            Packaging::Appx::{GetApplicationUserModelId, GetPackageFullName},
        },
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW,
                WaitForSingleObject,
            },
        },
        UI::{
            Input::KeyboardAndMouse::IsWindowEnabled,
            Shell::{AO_NOERRORUI, ApplicationActivationManager, IApplicationActivationManager},
            WindowsAndMessaging::{
                EnumWindows, GW_OWNER, GetClassNameW, GetForegroundWindow, GetWindow,
                GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
                SetForegroundWindow,
            },
        },
    },
    core::{BOOL, HRESULT, PCWSTR, PWSTR},
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
#[derive(Clone, PartialEq, Eq)]
struct FileFingerprint {
    path: String,
    sha256: String,
    bytes: u64,
    volume: u32,
    file_index: u64,
}
fn fingerprint(file: &mut File) -> Result<FileFingerprint, ErrorCode> {
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
    Ok(FileFingerprint {
        path: path_of(file)?,
        sha256: format!("{:x}", hash.finalize()),
        bytes: total,
        volume: info.dwVolumeSerialNumber,
        file_index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}
fn identity(
    file: &mut File,
    arguments: String,
    working_directory: String,
) -> Result<ExecutableIdentity, ErrorCode> {
    let value = fingerprint(file)?;
    Ok(ExecutableIdentity {
        path: value.path,
        arguments,
        working_directory,
        sha256: value.sha256,
        bytes: value.bytes,
        volume: value.volume,
        file_index: value.file_index,
    })
}
/// Native picker evidence only; no cwd, action authority or frontend constructor.
#[derive(Clone)]
pub struct ExplicitExecutable {
    name: String,
    fingerprint: FileFingerprint,
}
impl ExplicitExecutable {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn path(&self) -> &str {
        &self.fingerprint.path
    }
    pub(crate) fn inspect(path: &Path) -> Result<Self, ErrorCode> {
        let path = path
            .to_str()
            .filter(|v| local_path(v) && v.to_ascii_lowercase().ends_with(".exe"))
            .ok_or(ErrorCode::Malformed)?;
        // The final component is opened without following a reparse point.
        // Ancestor junctions are not claimed absent; canonical handle identity
        // is what is frozen and later compared.
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(0x00200000)
            .open(path)
            .map_err(|_| ErrorCode::Unavailable)?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
            .map_err(|_| ErrorCode::Unavailable)?;
        if info.dwFileAttributes & 0x400 != 0 {
            return Err(ErrorCode::Unsupported);
        }
        let fingerprint = fingerprint(&mut file)?;
        if !fingerprint.path.to_ascii_lowercase().ends_with(".exe") {
            return Err(ErrorCode::Malformed);
        }
        let name = Path::new(&fingerprint.path)
            .file_stem()
            .and_then(|v| v.to_str())
            .filter(|v| !v.is_empty() && v.len() <= 256 && !v.chars().any(char::is_control))
            .ok_or(ErrorCode::Malformed)?
            .into();
        Ok(Self { name, fingerprint })
    }
    pub fn revalidate(&self) -> Result<(), ErrorCode> {
        if Self::inspect(Path::new(self.path()))?.fingerprint != self.fingerprint {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    pub fn select(&self, owner: Uuid, cwd: &Path) -> Result<AppRecord, ErrorCode> {
        self.revalidate()?;
        let record = select_executable(
            owner,
            self.name.clone(),
            Path::new(self.path()),
            String::new(),
            cwd,
        )?;
        let LaunchIdentity::Executable(ref value) = record.launch else {
            return Err(ErrorCode::Malformed);
        };
        if value.path != self.fingerprint.path
            || value.sha256 != self.fingerprint.sha256
            || value.bytes != self.fingerprint.bytes
            || value.volume != self.fingerprint.volume
            || value.file_index != self.fingerprint.file_index
        {
            return Err(ErrorCode::Stale);
        }
        Ok(record)
    }
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
fn main_class(window: HWND) -> Option<String> {
    if !unsafe { IsWindow(Some(window)) }.as_bool()
        || !unsafe { IsWindowVisible(window) }.as_bool()
        || !unsafe { IsWindowEnabled(window) }.as_bool()
        || unsafe { GetWindow(window, GW_OWNER) }.is_ok()
    {
        return None;
    }
    let mut cloaked = 0u32;
    if unsafe { DwmGetWindowAttribute(window, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4) }
        .is_err()
        || cloaked != 0
    {
        return None;
    }
    window_class(window)
}
fn window_class(window: HWND) -> Option<String> {
    let mut class = [0u16; 257];
    let count = unsafe { GetClassNameW(window, &mut class) };
    (count > 0 && count < 256)
        .then(|| String::from_utf16(&class[..count as usize]).ok())
        .flatten()
}
fn matches_window(window: HWND, pid: u32, class: &Option<String>) -> bool {
    let Some(expected) = class else {
        return false;
    };
    let mut actual_pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut actual_pid));
    }
    actual_pid == pid && main_class(window).as_ref() == Some(expected)
}
struct Process(HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}
fn live_process(
    pid: u32,
    expected: &ExecutableIdentity,
) -> Result<Option<(Process, u64)>, ErrorCode> {
    let process = Process(
        unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }
        .map_err(|_| ErrorCode::Unavailable)?,
    );
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
        return Err(ErrorCode::Stale);
    }
    let mut image = [0u16; 4096];
    let mut length = image.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(image.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|_| ErrorCode::Unavailable)?;
    if length == 0 || length as usize >= image.len() {
        return Err(ErrorCode::TooLarge);
    }
    let path = String::from_utf16(&image[..length as usize]).map_err(|_| ErrorCode::Malformed)?;
    if !path
        .trim_start_matches(r"\\?\")
        .eq_ignore_ascii_case(expected.path.trim_start_matches(r"\\?\"))
    {
        return Ok(None);
    }
    if !crate::principal::same_process_context(process.0)? {
        return Err(ErrorCode::Unauthenticated);
    }
    let image = open_executable(&path)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(image.as_raw_handle()), &mut info) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    let length = (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow);
    if info.dwVolumeSerialNumber != expected.volume
        || index != expected.file_index
        || length != expected.bytes
        || path_of(&image)? != expected.path
    {
        return Err(ErrorCode::Stale);
    }
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    if created == 0 {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some((process, created)))
}
fn identity_process(
    pid: u32,
    expected: &LaunchIdentity,
) -> Result<Option<(Process, u64)>, ErrorCode> {
    if let LaunchIdentity::Executable(value) = expected {
        return live_process(pid, value);
    }
    let process = Process(
        unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }
        .map_err(|_| ErrorCode::Unavailable)?,
    );
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
        return Err(ErrorCode::Stale);
    }
    if !package_process_matches(process.0, expected)? {
        return Ok(None);
    }
    if !crate::principal::same_process_context(process.0)? {
        return Err(ErrorCode::Unauthenticated);
    }
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    if created == 0 {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some((process, created)))
}
fn package_process_matches(process: HANDLE, expected: &LaunchIdentity) -> Result<bool, ErrorCode> {
    let LaunchIdentity::Packaged {
        app_id,
        package_full_name,
        ..
    } = expected
    else {
        return Err(ErrorCode::Malformed);
    };
    let mut full = [0u16; 513];
    let mut count = full.len() as u32;
    let status = unsafe { GetPackageFullName(process, &mut count, Some(PWSTR(full.as_mut_ptr()))) };
    if status == APPMODEL_ERROR_NO_PACKAGE {
        return Ok(false);
    }
    if status != ERROR_SUCCESS {
        return Err(ErrorCode::Unavailable);
    }
    let decode = |value: &[u16], count: u32| -> Result<String, ErrorCode> {
        let count = count as usize;
        if count < 2
            || count > value.len()
            || value[count - 1] != 0
            || value[..count - 1].contains(&0)
        {
            return Err(ErrorCode::Malformed);
        }
        String::from_utf16(&value[..count - 1]).map_err(|_| ErrorCode::Malformed)
    };
    if !decode(&full, count)?.eq_ignore_ascii_case(package_full_name) {
        return Ok(false);
    }
    let mut app = [0u16; 513];
    let mut count = app.len() as u32;
    let status =
        unsafe { GetApplicationUserModelId(process, &mut count, Some(PWSTR(app.as_mut_ptr()))) };
    if status == APPMODEL_ERROR_NO_APPLICATION {
        return Ok(false);
    }
    if status != ERROR_SUCCESS {
        return Err(ErrorCode::Unavailable);
    }
    let actual = decode(&app, count)?;
    // Family is a case-insensitive package identity. Relative app ID is kept
    // exact rather than broadening authority without an API identity contract.
    Ok(match (actual.split_once('!'), app_id.split_once('!')) {
        (Some((family, app)), Some((expected_family, expected_app))) => {
            family.eq_ignore_ascii_case(expected_family) && app == expected_app
        }
        _ => false,
    })
}
#[derive(Clone)]
pub struct WindowHint {
    window: u64,
    pid: u32,
    created: u64,
    class: String,
    title: String,
    image: LaunchIdentity,
    observed: Instant,
}
impl WindowHint {
    pub fn class(&self) -> &str {
        &self.class
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
}
pub struct WindowHints {
    pub windows: Vec<WindowHint>,
    pub complete: bool,
}
struct HintScan<'a> {
    expected: &'a LaunchIdentity,
    class: &'a Option<String>,
    windows: Vec<WindowHint>,
    seen: usize,
    complete: bool,
    started: Instant,
}
unsafe extern "system" fn hint_visit(window: HWND, value: LPARAM) -> BOOL {
    let state = unsafe { &mut *(value.0 as *mut HintScan<'_>) };
    state.seen += 1;
    if state.seen > 1024
        || state.windows.len() >= 32
        || state.started.elapsed() >= Duration::from_secs(2)
    {
        state.complete = false;
        return BOOL(0);
    }
    let Some(class) = window_class(window) else {
        return BOOL(1);
    };
    let class_matches = state.class.as_ref().is_none_or(|v| v == &class);
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    match identity_process(pid, state.expected) {
        Ok(Some((_process, created))) => {
            // A matching application with a blocked/hidden/modal/wrong-class
            // window is not evidence of no running instance. Never spawn over it.
            if !class_matches || main_class(window).as_ref() != Some(&class) {
                state.complete = false;
                return BOOL(1);
            }
            let mut title = [0u16; 257];
            let size = unsafe { GetWindowTextW(window, &mut title) };
            let title = if size > 0 && size < 256 {
                String::from_utf16(&title[..size as usize]).unwrap_or_default()
            } else {
                String::new()
            };
            state.windows.push(WindowHint {
                window: window.0 as usize as u64,
                pid,
                created,
                class,
                title,
                image: state.expected.clone(),
                observed: Instant::now(),
            });
        }
        Ok(None) => {}
        Err(_) if class_matches => state.complete = false,
        Err(_) => {}
    }
    BOOL(1)
}
pub fn window_hints(record: &AppRecord) -> Result<WindowHints, ErrorCode> {
    record.validate()?;
    let _file = match &record.launch {
        LaunchIdentity::Executable(expected) => {
            let mut file = open_executable(&expected.path)?;
            if identity(
                &mut file,
                expected.arguments.clone(),
                expected.working_directory.clone(),
            )? != *expected
            {
                return Err(ErrorCode::Stale);
            }
            Some(file)
        }
        LaunchIdentity::Packaged { .. } => {
            crate::packages::revalidate_identity(&record.launch)?;
            None
        }
    };
    Ok(scan_hints(&record.launch, &record.window_class))
}
fn scan_hints(expected: &LaunchIdentity, class: &Option<String>) -> WindowHints {
    let mut scan = HintScan {
        expected,
        class,
        windows: vec![],
        seen: 0,
        complete: true,
        started: Instant::now(),
    };
    if unsafe {
        EnumWindows(
            Some(hint_visit),
            LPARAM((&mut scan as *mut HintScan<'_>) as isize),
        )
    }
    .is_err()
    {
        scan.complete = false;
    }
    WindowHints {
        windows: scan.windows,
        complete: scan.complete,
    }
}
/// Only a native-created, freshly observed hint can be attached before the new
/// immutable record is published. This does not assert application readiness.
pub fn bind_window_hint(record: &mut AppRecord, hint: &WindowHint) -> Result<(), ErrorCode> {
    if hint.observed.elapsed() >= Duration::from_secs(30) || hint.image != record.launch {
        return Err(ErrorCode::Stale);
    }
    let Some((_process, created)) = identity_process(hint.pid, &record.launch)? else {
        return Err(ErrorCode::Stale);
    };
    if created != hint.created
        || !matches_window(
            HWND(hint.window as usize as *mut _),
            hint.pid,
            &Some(hint.class.clone()),
        )
        || hint.observed.elapsed() >= Duration::from_secs(30)
    {
        return Err(ErrorCode::Stale);
    }
    record.window_class = Some(hint.class.clone());
    Ok(())
}
fn running_instance(expected: &ExecutableIdentity) -> Result<bool, ErrorCode> {
    let expected_name = Path::new(&expected.path)
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or(ErrorCode::Malformed)?;
    let snapshot = Process(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|_| ErrorCode::Unavailable)?,
    );
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    unsafe { Process32FirstW(snapshot.0, &mut entry) }.map_err(|_| ErrorCode::Unavailable)?;
    let started = Instant::now();
    let mut seen = 0;
    loop {
        seen += 1;
        if seen > 2048 || started.elapsed() >= Duration::from_secs(2) {
            return Err(ErrorCode::Unavailable);
        }
        let length = entry
            .szExeFile
            .iter()
            .position(|v| *v == 0)
            .ok_or(ErrorCode::Malformed)?;
        let name =
            String::from_utf16(&entry.szExeFile[..length]).map_err(|_| ErrorCode::Malformed)?;
        if name.eq_ignore_ascii_case(expected_name)
            && live_process(entry.th32ProcessID, expected)?.is_some()
        {
            return Ok(true);
        }
        match unsafe { Process32NextW(snapshot.0, &mut entry) } {
            Ok(()) => {}
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => {
                return Ok(false);
            }
            Err(_) => return Err(ErrorCode::Unavailable),
        }
    }
}
unsafe extern "system" fn visit(window: HWND, value: LPARAM) -> BOOL {
    // EnumWindows calls synchronously while the stack-owned context is alive.
    let state = unsafe { &mut *(value.0 as *mut Windows) };
    state.seen += 1;
    if state.seen > 1024 {
        state.overflow = true;
        return BOOL(0);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid != state.pid {
        return BOOL(1);
    }
    if !matches_window(window, state.pid, &state.class) {
        // Known same-process auxiliary/modal/ineligible windows do not prove
        // application readiness. Preserve this conservative qualification gap.
        state.overflow = true;
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
        return activate_package(record, authorize_commit);
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
    if record.window_class.is_none() {
        return Ok(EffectResult {
            outcome: Outcome::NeedsInput,
            observation: None,
        });
    }
    let existing = scan_hints(&record.launch, &record.window_class);
    if !existing.complete
        || existing.windows.len() > 1
        || (!existing.windows.is_empty() && !expected.arguments.is_empty())
    {
        return Ok(EffectResult {
            outcome: Outcome::NeedsInput,
            observation: None,
        });
    }
    if let Some(window) = existing.windows.first() {
        return focus(record, window, authorize_commit);
    }
    // A headless, starting or inaccessible same-name instance cannot establish
    // safe absence. Leave the task waiting for an explicit native resolution.
    if !matches!(running_instance(expected), Ok(false)) {
        return Ok(EffectResult {
            outcome: Outcome::NeedsInput,
            observation: None,
        });
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
                focus_verified: None,
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
            focus_verified: None,
        }),
    })
}
fn focus(
    record: &AppRecord,
    hint: &WindowHint,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<EffectResult, ErrorCode> {
    if matches!(&record.launch, LaunchIdentity::Executable(value) if !value.arguments.is_empty())
        || hint.image != record.launch
        || hint.observed.elapsed() >= Duration::from_secs(30)
    {
        return Err(ErrorCode::Stale);
    }
    let window = HWND(hint.window as usize as *mut _);
    let Some((process, created)) = identity_process(hint.pid, &record.launch)? else {
        return Err(ErrorCode::Stale);
    };
    if created != hint.created || !matches_window(window, hint.pid, &record.window_class) {
        return Err(ErrorCode::Stale);
    }
    if unsafe { IsIconic(window) }.as_bool() {
        return Ok(EffectResult {
            outcome: Outcome::NeedsInput,
            observation: None,
        });
    }
    if !crate::principal::same_process_context(process.0)? {
        return Err(ErrorCode::Stale);
    }
    if matches!(&record.launch, LaunchIdentity::Packaged { .. }) {
        crate::packages::revalidate_identity(&record.launch)?;
        if !package_process_matches(process.0, &record.launch)? {
            return Err(ErrorCode::Stale);
        }
    }
    authorize()?;
    let mut focused = false;
    if unsafe { WaitForSingleObject(process.0, 0) } == WAIT_TIMEOUT
        && matches_window(window, hint.pid, &record.window_class)
    {
        let _ = unsafe { SetForegroundWindow(window) };
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(500) {
            if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT
                || !matches_window(window, hint.pid, &record.window_class)
                || unsafe { IsIconic(window) }.as_bool()
            {
                break;
            }
            if unsafe { GetForegroundWindow() } == window {
                focused = !matches!(&record.launch, LaunchIdentity::Packaged { .. })
                    || (package_process_matches(process.0, &record.launch).unwrap_or(false)
                        && crate::principal::same_process_context(process.0).unwrap_or(false)
                        && unsafe { GetForegroundWindow() } == window
                        && matches_window(window, hint.pid, &record.window_class));
                focused &= started.elapsed() < Duration::from_millis(500);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    Ok(EffectResult {
        outcome: if focused {
            Outcome::Success
        } else {
            Outcome::UnknownEffect
        },
        observation: Some(app_observation(
            record,
            Some(hint.pid),
            Some(created),
            focused.then_some(hint.window),
            true,
            Some(focused),
        )),
    })
}

fn activate_package(
    record: &AppRecord,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<EffectResult, ErrorCode> {
    let LaunchIdentity::Packaged { app_id, .. } = &record.launch else {
        return Err(ErrorCode::Malformed);
    };
    if record.window_class.is_none() {
        return Ok(EffectResult {
            outcome: Outcome::NeedsInput,
            observation: None,
        });
    }
    let _apartment = crate::packages::Apartment::enter()?;
    crate::packages::revalidate_identity(&record.launch)?;
    let existing = scan_hints(&record.launch, &record.window_class);
    if !existing.complete || existing.windows.len() > 1 {
        return Ok(EffectResult {
            outcome: Outcome::NeedsInput,
            observation: None,
        });
    }
    if let Some(hint) = existing.windows.first() {
        return focus(record, hint, authorize);
    }
    let manager: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| ErrorCode::Unavailable)?;
    let app: Vec<u16> = app_id.encode_utf16().chain(Some(0)).collect();
    // No error UI, debugging options, arbitrary arguments, or retry. Recheck
    // registration after all preflight inspection, then final durable authority.
    crate::packages::revalidate_identity(&record.launch)?;
    authorize()?;
    let pid =
        unsafe { manager.ActivateApplication(PCWSTR(app.as_ptr()), PCWSTR::null(), AO_NOERRORUI) }
            .ok()
            .filter(|v| *v != 0);
    let started = Instant::now();
    let mut matched = false;
    let mut observed = None;
    let mut process_created = None;
    if let Some(pid) = pid
        && let Ok(Some((process, created))) = identity_process(pid, &record.launch)
    {
        matched = true;
        process_created = Some(created);
        while started.elapsed() < Duration::from_secs(3) {
            if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
                break;
            }
            let mut windows = Windows {
                pid,
                class: record.window_class.clone(),
                seen: 0,
                matched: Vec::with_capacity(2),
                overflow: false,
            };
            if unsafe { EnumWindows(Some(visit), LPARAM((&mut windows as *mut Windows) as isize)) }
                .is_err()
                || windows.overflow
                || windows.matched.len() > 1
            {
                break;
            }
            if let Some(window) = windows.matched.first() {
                if crate::packages::revalidate_identity(&record.launch).is_ok()
                    && matches!(identity_process(pid, &record.launch), Ok(Some((_, current))) if current == created)
                    && unsafe { WaitForSingleObject(process.0, 0) } == WAIT_TIMEOUT
                    && matches_window(HWND(*window as usize as *mut _), pid, &record.window_class)
                    && started.elapsed() < Duration::from_secs(3)
                {
                    observed = Some(*window);
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(EffectResult {
        outcome: if observed.is_some() {
            Outcome::Success
        } else {
            Outcome::UnknownEffect
        },
        observation: Some(app_observation(
            record,
            pid,
            process_created,
            observed,
            matched,
            None,
        )),
    })
}
fn app_observation(
    record: &AppRecord,
    process_id: Option<u32>,
    process_created: Option<u64>,
    window: Option<u64>,
    matched: bool,
    focus_verified: Option<bool>,
) -> EffectObservation {
    match &record.launch {
        LaunchIdentity::Executable(_) => EffectObservation::Application {
            app_id: record.id,
            catalog_revision: record.revision,
            process_id,
            window,
            image_matched: matched,
            focus_verified,
        },
        LaunchIdentity::Packaged {
            app_id,
            package_full_name,
            publisher_id,
        } => EffectObservation::PackagedApplication {
            app_id: record.id,
            catalog_revision: record.revision,
            aumid: app_id.clone(),
            package_full_name: package_full_name.clone(),
            publisher_id: publisher_id.clone(),
            process_id,
            process_created,
            window,
            package_matched: matched,
            focus_verified,
        },
    }
}
