//! Title-free native observation under one retained, explicitly selected scope.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use windows::Win32::UI::{
    Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
    WindowsAndMessaging::{
        DispatchMessageW, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, MSG, PM_REMOVE, PeekMessageW,
        TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    },
};
fn now_ms() -> Result<u64, ErrorCode> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ErrorCode::Expired)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| ErrorCode::Expired)
}
static OWNED: AtomicBool = AtomicBool::new(false);
static DIRTY: AtomicBool = AtomicBool::new(false);
unsafe extern "system" fn changed(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    DIRTY.store(true, Ordering::Relaxed);
}
struct Hooks(Vec<HWINEVENTHOOK>, bool);
impl Hooks {
    fn retire(&mut self) -> Result<(), ErrorCode> {
        if self.1 {
            return Ok(());
        }
        self.0
            .retain(|hook| !unsafe { UnhookWinEvent(*hook) }.as_bool());
        if self.0.is_empty() {
            self.1 = true;
            OWNED.store(false, Ordering::SeqCst);
            Ok(())
        } else {
            // Uncertain native hook retirement forbids another observer.
            Err(ErrorCode::Unavailable)
        }
    }
}
impl Drop for Hooks {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}
pub struct Evidence {
    passive: Option<avesra_core::demonstration::passive::Setting>,
    observed_at: Option<Instant>,
    setup: avesra_core::demonstration::Setup,
    device: Uuid,
    name: String,
    demonstration: Uuid,
    baseline_ms: u64,
    process_created: u64,
    started: Instant,
    observed_ms: u64,
    elapsed_ms: u64,
    transitions: u16,
}
impl Evidence {
    pub(crate) fn passive_deadline(&self) -> Result<Instant, ErrorCode> {
        self.observed_at
            .map(|at| (at + Duration::from_secs(30)).min(self.started + Duration::from_secs(300)))
            .ok_or(ErrorCode::Stale)
    }
    pub(crate) fn passive(&self) -> Option<avesra_core::demonstration::passive::Setting> {
        self.passive.clone()
    }
    pub(crate) fn candidate(self) -> Result<avesra_core::demonstration::Candidate, ErrorCode> {
        if self.started.elapsed() >= Duration::from_secs(300)
            || (self.passive.is_some()
                && self
                    .observed_at
                    .is_none_or(|at| at.elapsed() > Duration::from_secs(30)))
        {
            return Err(ErrorCode::Expired);
        }
        let window_class = self
            .setup
            .record
            .window_class
            .clone()
            .ok_or(ErrorCode::Stale)?;
        Ok(avesra_core::demonstration::Candidate {
            demonstration: self.demonstration,
            baseline_ms: self.baseline_ms,
            process_created: self.process_created,
            window_class,
            source: if self.passive.is_some() {
                avesra_core::demonstration::SourceKind::PassiveObservedTransition
            } else {
                avesra_core::demonstration::SourceKind::ObservedTransition
            },
            passive_revision: self.passive.as_ref().map(|s| s.revision),
            id: Uuid::new_v4(),
            revision: Uuid::new_v4(),
            actor: self.setup.scope.actor,
            device: self.device,
            scope: self.setup.scope,
            name: self.name,
            observed_ms: self.observed_ms,
            elapsed_ms: self.elapsed_ms,
            transitions: self.transitions,
            disabled: false,
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Sample {
    window: u64,
    pid: u32,
    created: u64,
    foreground: bool,
}
struct Scan<'a> {
    record: &'a AppRecord,
    found: Option<Sample>,
    complete: bool,
    seen: usize,
    started: Instant,
}
unsafe extern "system" fn inspect(window: HWND, arg: LPARAM) -> BOOL {
    let scan = unsafe { &mut *(arg.0 as *mut Scan<'_>) };
    scan.seen += 1;
    if scan.seen > 1024 || scan.started.elapsed() > Duration::from_millis(250) {
        scan.complete = false;
        return BOOL(0);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    match identity_process(pid, &scan.record.launch) {
        Ok(Some((_process, created))) => {
            let class = window_class(window);
            let main = main_class(window);
            // Ancillary hidden/owned windows are not app-opening evidence.
            // A hidden expected window or a different visible main window is
            // uncertainty, not an absent baseline.
            if main.is_none() && class.as_ref() != scan.record.window_class.as_ref() {
                return BOOL(1);
            }
            if main.as_ref() != scan.record.window_class.as_ref() || scan.found.is_some() {
                scan.complete = false;
                return BOOL(1);
            }
            scan.found = Some(Sample {
                window: window.0 as usize as u64,
                pid,
                created,
                foreground: unsafe { GetForegroundWindow() } == window,
            });
        }
        Ok(None) => {}
        Err(_) => {
            if scan.record.window_class.as_ref() == window_class(window).as_ref() {
                scan.complete = false;
            }
        }
    }
    BOOL(1)
}
fn sample(record: &AppRecord) -> Result<Option<Sample>, ErrorCode> {
    let mut scan = Scan {
        record,
        found: None,
        complete: true,
        seen: 0,
        started: Instant::now(),
    };
    unsafe { EnumWindows(Some(inspect), LPARAM((&mut scan as *mut Scan<'_>) as isize)) }
        .map_err(|_| ErrorCode::Unavailable)?;
    if !scan.complete {
        return Err(ErrorCode::Unavailable);
    }
    Ok(scan.found)
}
pub struct Admission<'a> {
    pub started: Instant,
    pub control: &'a AtomicU8,
}
/// Control 0=continue,1=explicit Stop/save,2=Cancel. No caller Drop releases this
/// actual worker's hook/file ownership. `current` true means foreground work has
/// priority: abandon continuity and skip sampling until it becomes idle.
pub fn observe(
    setup: avesra_core::demonstration::Setup,
    device: Uuid,
    name: String,
    admission: Admission<'_>,
    current: &mut dyn FnMut() -> Result<bool, ErrorCode>,
    progress: &mut dyn FnMut(bool, u16),
) -> Result<Option<Evidence>, ErrorCode> {
    observe_inner(setup, device, name, admission, current, progress, None)
}
pub fn observe_passive(
    prepared: avesra_core::demonstration::passive::Prepared,
    device: Uuid,
    admission: Admission<'_>,
    current: &mut dyn FnMut() -> Result<bool, ErrorCode>,
    progress: &mut dyn FnMut(bool, u16),
) -> Result<Option<Evidence>, ErrorCode> {
    // Called only on the native coordinator's dedicated passive thread.
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_LOWEST,
    };
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST) }
        .map_err(|_| ErrorCode::Unavailable)?;
    let name = format!("open {}", prepared.setup.scope.name);
    observe_inner(
        prepared.setup,
        device,
        name,
        admission,
        current,
        progress,
        Some(prepared.setting),
    )
}
fn observe_inner(
    setup: avesra_core::demonstration::Setup,
    device: Uuid,
    name: String,
    admission: Admission<'_>,
    current: &mut dyn FnMut() -> Result<bool, ErrorCode>,
    progress: &mut dyn FnMut(bool, u16),
    passive: Option<avesra_core::demonstration::passive::Setting>,
) -> Result<Option<Evidence>, ErrorCode> {
    let Admission { started, control } = admission;
    let deadline = started
        .checked_add(Duration::from_secs(300))
        .ok_or(ErrorCode::Expired)?;
    let mut preparation = || {
        if Instant::now() >= deadline {
            return Err(ErrorCode::Expired);
        }
        if control.load(Ordering::SeqCst) != 0 {
            return Err(ErrorCode::Stale);
        }
        if current()? {
            return Err(ErrorCode::Unavailable);
        }
        Ok(())
    };
    preparation()?;
    setup.scope.validate()?;
    setup.record.validate()?;
    let name = avesra_core::apps::alias_phrase(&name)?;
    if device.is_nil()
        || setup.scope.excluded
        || setup.scope.app != setup.record.id
        || setup.scope.app_revision != setup.record.revision
    {
        return Err(ErrorCode::Denied);
    }
    if OWNED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ErrorCode::Unavailable);
    }
    let mut hooks = Hooks(Vec::new(), false);
    let _file = match &setup.record.launch {
        LaunchIdentity::Executable(expected) => {
            let mut file = open_executable(&expected.path)?;
            if identity_checked(
                &mut file,
                expected.arguments.clone(),
                expected.working_directory.clone(),
                &mut preparation,
            )? != *expected
            {
                return Err(ErrorCode::Stale);
            }
            Some(file)
        }
        LaunchIdentity::Packaged { .. } => {
            crate::packages::revalidate_identity(&setup.record.launch)?;
            None
        }
    };
    preparation()?;
    if sample(&setup.record)?.is_some() {
        return Err(ErrorCode::Denied);
    }
    for event in [EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_SHOW] {
        preparation()?;
        let hook = unsafe {
            SetWinEventHook(
                event,
                event,
                None,
                Some(changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.0.is_null() {
            return Err(ErrorCode::Unavailable);
        }
        hooks.0.push(hook);
    }
    preparation()?;
    let demonstration = Uuid::new_v4();
    let baseline_ms = now_ms()?;
    let mut process_created = 0;
    let mut last = Instant::now();
    let mut previous = None;
    let mut absent = true;
    let mut qualified = false;
    let mut observed_ms = 0;
    let mut observed_at = None;
    let mut transitions = 0u16;
    loop {
        if started.elapsed() >= Duration::from_secs(300) {
            return Ok(None);
        }
        let busy = current()?;
        match control.load(Ordering::SeqCst) {
            2 => return Ok(None),
            1 => {
                hooks.retire()?;
                return Ok(if qualified && !busy {
                    Some(Evidence {
                        passive,
                        observed_at,
                        setup,
                        device,
                        name,
                        demonstration,
                        baseline_ms,
                        process_created,
                        started,
                        observed_ms,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                        transitions,
                    })
                } else {
                    None
                });
            }
            _ => {}
        }
        let mut message = MSG::default();
        let mut pumped = 0;
        while pumped < 64 && unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool()
        {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            };
            pumped += 1;
        }
        if busy {
            absent = false;
            previous = None;
            if qualified {
                progress(false, transitions);
            }
            qualified = false;
            DIRTY.store(false, Ordering::Relaxed);
            last = Instant::now();
        } else if last.elapsed() >= Duration::from_secs(2)
            || (last.elapsed() >= Duration::from_millis(200)
                && DIRTY.swap(false, Ordering::Relaxed))
        {
            let value = sample(&setup.record)?;
            last = Instant::now();
            if value != previous {
                transitions += 1;
                if transitions > 64 {
                    return Ok(None);
                }
                if let Some(now) = value {
                    if absent && now.foreground {
                        qualified = true;
                        process_created = now.created;
                        observed_ms = now_ms()?;
                        observed_at = Some(Instant::now());
                        if passive.is_some() {
                            let _ =
                                control.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
                        }
                    }
                } else {
                    absent = true;
                }
                previous = value;
                progress(qualified, transitions);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
