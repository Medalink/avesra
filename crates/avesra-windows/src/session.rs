//! Own-window WTS notification lifetime; no cross-process window inspection.
use avesra_contracts::ErrorCode;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::{
        RemoteDesktop::{
            NOTIFY_FOR_THIS_SESSION, WTS_CURRENT_SESSION, WTS_SESSIONSTATE_UNLOCK, WTSActive,
            WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW, WTSRegisterSessionNotification,
            WTSSessionInfoEx, WTSUnRegisterSessionNotification,
        },
        Threading::GetCurrentThreadId,
    },
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{GetWindowThreadProcessId, WM_NCDESTROY, WM_WTSSESSION_CHANGE},
    },
};

const SUBCLASS_ID: usize = 0x41565352;
struct Callback {
    changed: Box<dyn Fn(bool) + Send + Sync>,
    registered: bool,
}

fn session_closed() -> bool {
    let mut buffer = windows::core::PWSTR::null();
    let mut bytes = 0;
    // SAFETY: WTS allocates this buffer; validate its size and tagged level before
    // reading the matching union member, and free exactly once on every outcome.
    unsafe {
        let queried = WTSQuerySessionInformationW(
            None,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        );
        let closed =
            if queried.is_ok() && !buffer.is_null() && bytes as usize >= size_of::<WTSINFOEXW>() {
                let info = &*buffer.0.cast::<WTSINFOEXW>();
                info.Level != 1
                    || info.Data.WTSInfoExLevel1.SessionState != WTSActive
                    || info.Data.WTSInfoExLevel1.SessionFlags != WTS_SESSIONSTATE_UNLOCK as i32
            } else {
                true
            };
        if !buffer.is_null() {
            WTSFreeMemory(buffer.0.cast());
        }
        closed
    }
}

/// Install once on the native-owned window's event thread. The callback receives
/// true for lock/logoff/disconnect and false for unlock; unlock grants no authority.
pub fn install(
    handle: isize,
    changed: impl Fn(bool) + Send + Sync + 'static,
) -> Result<(), ErrorCode> {
    let hwnd = HWND(handle as *mut core::ffi::c_void);
    // SAFETY: These getters do not dereference caller memory. Subclassing is only
    // attempted for a valid window owned by this thread, as required by comctl32.
    if hwnd.is_invalid() || unsafe { GetWindowThreadProcessId(hwnd, None) != GetCurrentThreadId() }
    {
        return Err(ErrorCode::Denied);
    }
    let callback = Box::into_raw(Box::new(Callback {
        changed: Box::new(changed),
        registered: false,
    }));
    // SAFETY: The heap allocation survives until WM_NCDESTROY on this thread.
    // Registration is performed once during application setup.
    unsafe {
        if !SetWindowSubclass(hwnd, Some(procedure), SUBCLASS_ID, callback as usize).as_bool() {
            drop(Box::from_raw(callback));
            return Err(ErrorCode::Unavailable);
        }
        if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_err() {
            if RemoveWindowSubclass(hwnd, Some(procedure), SUBCLASS_ID).as_bool() {
                drop(Box::from_raw(callback));
            }
            // If removal failed, the installed procedure still owns the allocation.
            return Err(ErrorCode::Unavailable);
        }
        (*callback).registered = true;
        ((*callback).changed)(session_closed());
    }
    Ok(())
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_WTSSESSION_CHANGE {
        let locked = match wparam.0 {
            2 | 4 | 6 | 7 | 11 => Some(true),
            1 | 3 | 8 => Some(session_closed()),
            _ => None,
        };
        if let Some(locked) = locked {
            // SAFETY: comctl32 returns the allocation registered by install, and
            // this thread alone releases it after removing the subclass.
            let callback = unsafe { &*(data as *const Callback) };
            // Never unwind through the native callback ABI.
            if callback.registered
                && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (callback.changed)(locked)
                }))
                .is_err()
            {
                std::process::abort();
            }
        }
    } else if message == WM_NCDESTROY {
        // SAFETY: Window teardown occurs on the owning event thread. Remove the
        // callback before freeing its data, then continue the default window chain.
        unsafe {
            let _ = WTSUnRegisterSessionNotification(hwnd);
            if RemoveWindowSubclass(hwnd, Some(procedure), id).as_bool() {
                drop(Box::from_raw(data as *mut Callback));
            }
        }
    }
    // SAFETY: Preserve the original message and subclass chain for this window.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
