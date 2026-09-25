//! Local transport only. Binary/principal checks do not authenticate a browser
//! profile or authorize commands. No listener is started by this module.
use avesra_contracts::ErrorCode;
use std::{
    os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    path::Path,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions},
    time::timeout,
};
use windows::{
    Win32::{
        Foundation::{HANDLE, HLOCAL, LocalFree, WAIT_TIMEOUT},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        System::{
            Pipes::{
                GetNamedPipeClientProcessId, GetNamedPipeClientSessionId,
                GetNamedPipeServerProcessId, GetNamedPipeServerSessionId,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
            },
        },
    },
    core::{PCWSTR, PWSTR},
};

pub const MAX_FRAME: usize = 65536;
const IO_BUDGET: Duration = Duration::from_secs(5);
const CONNECT_BUDGET: Duration = Duration::from_secs(15);

struct SecurityDescriptor(PSECURITY_DESCRIPTOR);
impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(HLOCAL(self.0.0)));
        }
    }
}

fn endpoint() -> Result<(String, String, u32), ErrorCode> {
    let sid = crate::principal::current_user()?;
    let session = crate::principal::current_session()?;
    if !crate::principal::valid_sid(&sid) {
        return Err(ErrorCode::Unauthenticated);
    }
    Ok((
        format!(r"\\.\pipe\Avesra.browser.{sid}.{session}.v1"),
        sid,
        session,
    ))
}
pub struct Listener {
    pipe: NamedPipeServer,
    session: u32,
}
impl Listener {
    /// Call from the single native owner only. An occupied name is an error;
    /// never connect to or share a preexisting server as a fallback.
    pub fn create() -> Result<Self, ErrorCode> {
        let (name, sid, session) = endpoint()?;
        let logon = crate::principal::current_logon_sid()?;
        let sddl: Vec<u16> = format!("O:{sid}D:P(A;;GA;;;{logon})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|_| ErrorCode::Unavailable)?;
        let descriptor = SecurityDescriptor(descriptor);
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0.0,
            bInheritHandle: false.into(),
        };
        let pipe = unsafe {
            ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .max_instances(1)
                .in_buffer_size(MAX_FRAME as u32)
                .out_buffer_size(MAX_FRAME as u32)
                .create_with_security_attributes_raw(
                    name,
                    (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
                )
        }
        .map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self { pipe, session })
    }
    pub async fn accept(self) -> Result<Connection<NamedPipeServer>, ErrorCode> {
        timeout(CONNECT_BUDGET, self.pipe.connect())
            .await
            .map_err(|_| ErrorCode::Expired)?
            .map_err(|_| ErrorCode::Unavailable)?;
        let peer = inspect_peer(&self.pipe, true, self.session).await?;
        Ok(Connection {
            pipe: self.pipe,
            peer,
            failed: false,
        })
    }
}
pub async fn connect() -> Result<Connection<NamedPipeClient>, ErrorCode> {
    let (name, _, session) = endpoint()?;
    // SECURITY_IDENTIFICATION prevents a malicious same-user pipe endpoint
    // from impersonating this client. There is no remote path or retry loop.
    let pipe = ClientOptions::new()
        .security_qos_flags(0x0001_0000)
        .open(name)
        .map_err(|_| ErrorCode::Unavailable)?;
    let peer = inspect_peer(&pipe, false, session).await?;
    Ok(Connection {
        pipe,
        peer,
        failed: false,
    })
}
async fn inspect_peer(
    pipe: &impl AsRawHandle,
    server: bool,
    session: u32,
) -> Result<OwnedHandle, ErrorCode> {
    let retained = unsafe { BorrowedHandle::borrow_raw(pipe.as_raw_handle()) }
        .try_clone_to_owned()
        .map_err(|_| ErrorCode::Unavailable)?;
    // The duplicated pipe/process handles stay owned by this blocking job if
    // its async waiter is cancelled. No dangling borrowed HANDLE is retained.
    tokio::task::spawn_blocking(move || {
        let pipe = HANDLE(retained.as_raw_handle());
        let mut pid = 0;
        let mut peer_session = 0;
        unsafe {
            if server {
                GetNamedPipeClientProcessId(pipe, &mut pid)?;
                GetNamedPipeClientSessionId(pipe, &mut peer_session)?;
            } else {
                GetNamedPipeServerProcessId(pipe, &mut pid)?;
                GetNamedPipeServerSessionId(pipe, &mut peer_session)?;
            }
        }
        if pid == 0 || peer_session != session {
            return Err(denied());
        }
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )?
        };
        let process = unsafe { OwnedHandle::from_raw_handle(process.0) };
        if !crate::principal::same_process_context(HANDLE(process.as_raw_handle())).unwrap_or(false)
        {
            return Err(denied());
        }
        let current = std::env::current_exe().map_err(|_| denied())?;
        let expected = current.parent().ok_or_else(denied)?.join(if server {
            "avesra-native-host.exe"
        } else {
            "avesra-desktop.exe"
        });
        let selected = crate::apps::ExplicitExecutable::inspect(&expected).map_err(|_| denied())?;
        let mut path = [0u16; 4096];
        let mut count = path.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                HANDLE(process.as_raw_handle()),
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &mut count,
            )?;
        }
        if count == 0 || count as usize >= path.len() {
            return Err(denied());
        }
        let actual = String::from_utf16(&path[..count as usize]).map_err(|_| denied())?;
        let actual =
            crate::apps::ExplicitExecutable::inspect(Path::new(&actual)).map_err(|_| denied())?;
        if !selected.same_file(&actual)
            || unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), 0) } != WAIT_TIMEOUT
        {
            return Err(denied());
        }
        Ok(process)
    })
    .await
    .map_err(|_| ErrorCode::Unavailable)?
    .map_err(|_| ErrorCode::Unauthenticated)
}
/// One owned duplex exchange; callers cannot concurrently borrow its reader or
/// writer. Any timeout/partial frame poisons this connection for further use.
pub struct Connection<T> {
    pipe: T,
    peer: OwnedHandle,
    failed: bool,
}
impl<T: AsyncRead + AsyncWrite + Unpin> Connection<T> {
    fn current(&self) -> bool {
        !self.failed && self.peer_alive()
    }
    fn peer_alive(&self) -> bool {
        (unsafe { WaitForSingleObject(HANDLE(self.peer.as_raw_handle()), 0) }) == WAIT_TIMEOUT
    }
    pub async fn receive(&mut self) -> Result<Vec<u8>, ErrorCode> {
        if !self.current() {
            return Err(ErrorCode::Stale);
        }
        self.failed = true;
        let result = timeout(IO_BUDGET, async {
            let length = self
                .pipe
                .read_u32_le()
                .await
                .map_err(|_| ErrorCode::Unavailable)? as usize;
            if length == 0 || length > MAX_FRAME {
                return Err(ErrorCode::TooLarge);
            }
            let mut bytes = vec![0; length];
            self.pipe
                .read_exact(&mut bytes)
                .await
                .map_err(|_| ErrorCode::Unavailable)?;
            if std::str::from_utf8(&bytes).is_err() {
                return Err(ErrorCode::Malformed);
            }
            Ok(bytes)
        })
        .await
        .map_err(|_| ErrorCode::Expired)
        .and_then(|v| v);
        if result.is_err() || !self.peer_alive() {
            self.failed = true;
            return Err(result.err().unwrap_or(ErrorCode::Stale));
        }
        self.failed = false;
        result
    }
    pub async fn send(&mut self, bytes: &[u8]) -> Result<(), ErrorCode> {
        if !self.current() {
            return Err(ErrorCode::Stale);
        }
        if bytes.is_empty() || bytes.len() > MAX_FRAME || std::str::from_utf8(bytes).is_err() {
            self.failed = true;
            return Err(ErrorCode::Malformed);
        }
        self.failed = true;
        let result = timeout(IO_BUDGET, async {
            self.pipe.write_u32_le(bytes.len() as u32).await?;
            self.pipe.write_all(bytes).await?;
            self.pipe.flush().await
        })
        .await
        .map_err(|_| ErrorCode::Expired)
        .and_then(|v| v.map_err(|_| ErrorCode::Unavailable));
        if result.is_err() || !self.peer_alive() {
            self.failed = true;
            return Err(result.err().unwrap_or(ErrorCode::Stale));
        }
        self.failed = false;
        result
    }
}

fn denied() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::Win32::Foundation::E_ACCESSDENIED)
}
