//! Dedicated registration/message owner; constructing this type invokes Windows.
use avesra_contracts::ErrorCode;
use avesra_core::shortcuts::{ACTIONS, Chord, ShortcutAction, Shortcuts};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::Duration;
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
        },
        WindowsAndMessaging::{
            MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_NOREMOVE, PM_REMOVE,
            PeekMessageW, PostThreadMessageW, QS_ALLINPUT, WM_APP, WM_HOTKEY, WM_QUIT,
        },
    },
};

const WAKE: u32 = WM_APP + 17;
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationState {
    Registered,
    Disabled,
    Conflict,
    Unavailable,
}
#[derive(Clone, Serialize)]
pub struct ShortcutStatus {
    pub action: ShortcutAction,
    pub binding: Option<Chord>,
    pub state: RegistrationState,
}
struct Entry {
    status: ShortcutStatus,
    id: Option<i32>,
}
struct Set {
    action: ShortcutAction,
    chord: Option<Chord>,
    reply: SyncSender<Result<ShortcutStatus, ErrorCode>>,
}
struct OwnedIds(Vec<i32>);
impl OwnedIds {
    fn remove(&mut self, id: i32) -> Result<(), ErrorCode> {
        unsafe { UnregisterHotKey(None, id) }.map_err(|_| ErrorCode::Unavailable)?;
        self.0.retain(|v| *v != id);
        Ok(())
    }
}
impl Drop for OwnedIds {
    fn drop(&mut self) {
        for id in &self.0 {
            let _ = unsafe { UnregisterHotKey(None, *id) };
        }
    }
}
pub struct Hotkeys {
    send: SyncSender<Set>,
    thread: u32,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<Vec<ShortcutStatus>>>,
}
fn rebind(
    entries: &mut [Entry],
    owned: &mut OwnedIds,
    next: &mut i32,
    action: ShortcutAction,
    chord: Option<Chord>,
) -> Result<ShortcutStatus, ErrorCode> {
    if let Some(chord) = chord {
        chord.validate()?;
    }
    if entries.iter().any(|v| {
        v.status.action != action && v.id.is_some() && v.status.binding == chord && chord.is_some()
    }) {
        return Err(ErrorCode::Denied);
    }
    let entry = entries
        .iter_mut()
        .find(|v| v.status.action == action)
        .ok_or(ErrorCode::Malformed)?;
    if entry.status.binding == chord
        && matches!(
            entry.status.state,
            RegistrationState::Registered | RegistrationState::Disabled
        )
    {
        return Ok(entry.status.clone());
    }
    let replacement = if let Some(chord) = chord {
        if *next >= 0xbfff {
            return Err(ErrorCode::Unavailable);
        }
        *next += 1;
        unsafe {
            RegisterHotKey(
                None,
                *next,
                HOT_KEY_MODIFIERS(chord.modifiers()) | MOD_NOREPEAT,
                u32::from(chord.key),
            )
        }
        .map_err(|error| {
            if error.code() == windows::core::HRESULT::from_win32(1409) {
                ErrorCode::Denied
            } else {
                ErrorCode::Unavailable
            }
        })?;
        owned.0.push(*next);
        Some(*next)
    } else {
        None
    };
    if let Some(old) = entry.id
        && let Err(error) = owned.remove(old)
    {
        if let Some(new) = replacement {
            let _ = owned.remove(new);
        }
        return Err(error);
    }
    entry.id = replacement;
    entry.status.binding = chord;
    entry.status.state = if replacement.is_some() {
        RegistrationState::Registered
    } else {
        RegistrationState::Disabled
    };
    Ok(entry.status.clone())
}
impl Hotkeys {
    pub fn spawn(
        bindings: Shortcuts,
        callback: impl Fn(ShortcutAction) + Send + 'static,
    ) -> Result<Self, ErrorCode> {
        bindings.validate()?;
        let (send, receive) = mpsc::sync_channel::<Set>(8);
        let (ready, identity) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let status = Arc::new(Mutex::new(
            ACTIONS
                .iter()
                .map(|action| ShortcutStatus {
                    action: *action,
                    binding: bindings.get(*action),
                    state: RegistrationState::Unavailable,
                })
                .collect::<Vec<_>>(),
        ));
        let worker_status = status.clone();
        std::thread::Builder::new()
            .name("avesra-hotkeys".into())
            .spawn(move || {
                let mut message = MSG::default();
                let _ = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
                if ready.send(unsafe { GetCurrentThreadId() }).is_err() {
                    return;
                }
                let mut owned = OwnedIds(Vec::with_capacity(4));
                let mut next = 0;
                let mut entries = ACTIONS
                    .iter()
                    .map(|action| Entry {
                        status: ShortcutStatus {
                            action: *action,
                            binding: None,
                            state: RegistrationState::Unavailable,
                        },
                        id: None,
                    })
                    .collect::<Vec<_>>();
                for action in ACTIONS {
                    if worker_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(error) = rebind(
                        &mut entries,
                        &mut owned,
                        &mut next,
                        action,
                        bindings.get(action),
                    ) && let Some(entry) = entries.iter_mut().find(|v| v.status.action == action)
                    {
                        entry.status.binding = bindings.get(action);
                        entry.status.state = if error == ErrorCode::Denied {
                            RegistrationState::Conflict
                        } else {
                            RegistrationState::Unavailable
                        };
                    }
                }
                if let Ok(mut value) = worker_status.lock() {
                    *value = entries.iter().map(|v| v.status.clone()).collect();
                }
                while !worker_stop.load(Ordering::SeqCst) {
                    // A bounded wait also drains accepted commands if posting the wake fails.
                    unsafe {
                        MsgWaitForMultipleObjectsEx(None, 100, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
                    };
                    let mut fatal = false;
                    while let Ok(command) = receive.try_recv() {
                        if worker_stop.load(Ordering::SeqCst) {
                            let _ = command.reply.try_send(Err(ErrorCode::Stale));
                            continue;
                        }
                        let result = rebind(
                            &mut entries,
                            &mut owned,
                            &mut next,
                            command.action,
                            command.chord,
                        );
                        // Uncertain cleanup closes this owner rather than accepting
                        // more bindings while a stray registration may remain.
                        if matches!(result, Err(ErrorCode::Unavailable)) {
                            fatal = true;
                            drop(std::mem::replace(&mut owned, OwnedIds(Vec::new())));
                            for entry in &mut entries {
                                entry.id = None;
                                entry.status.state = RegistrationState::Unavailable;
                            }
                        }
                        if let Ok(mut value) = worker_status.lock() {
                            *value = entries.iter().map(|v| v.status.clone()).collect();
                        }
                        let _ = command.reply.try_send(result);
                        if fatal {
                            break;
                        }
                    }
                    if fatal {
                        break;
                    }
                    if !unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                        continue;
                    }
                    if message.message == WM_QUIT {
                        break;
                    }
                    if message.message == WM_HOTKEY && !worker_stop.load(Ordering::SeqCst) {
                        let id = message.wParam.0 as i32;
                        let packed = message.lParam.0 as u32;
                        if let Some(entry) = entries.iter().find(|v| v.id == Some(id))
                            && let Some(chord) = entry.status.binding
                            && packed & 0xffff == chord.modifiers()
                            && packed >> 16 == u32::from(chord.key)
                        {
                            callback(entry.status.action);
                        }
                    }
                }
                drop(owned);
                if let Ok(mut value) = worker_status.lock() {
                    for status in value.iter_mut() {
                        status.state = RegistrationState::Unavailable;
                    }
                }
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        let thread = identity.recv_timeout(Duration::from_secs(2)).map_err(|_| {
            stop.store(true, Ordering::SeqCst);
            ErrorCode::Unavailable
        })?;
        Ok(Self {
            send,
            thread,
            stop,
            status,
        })
    }
    pub fn snapshot(&self) -> Result<Vec<ShortcutStatus>, ErrorCode> {
        self.status
            .lock()
            .map(|v| v.clone())
            .map_err(|_| ErrorCode::Unavailable)
    }
    pub fn set(
        &self,
        action: ShortcutAction,
        chord: Option<Chord>,
    ) -> Result<Receiver<Result<ShortcutStatus, ErrorCode>>, ErrorCode> {
        if let Some(chord) = chord {
            chord.validate()?;
        }
        let (reply, result) = mpsc::sync_channel(1);
        self.send
            .try_send(Set {
                action,
                chord,
                reply,
            })
            .map_err(|_| ErrorCode::Unavailable)?;
        // Once queued, return its owned outcome even if the optional wake fails.
        let _ = unsafe { PostThreadMessageW(self.thread, WAKE, WPARAM(0), LPARAM(0)) };
        Ok(result)
    }
}
impl Drop for Hotkeys {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = unsafe { PostThreadMessageW(self.thread, WAKE, WPARAM(0), LPARAM(0)) };
    }
}
