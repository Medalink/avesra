//! Bounded selected-endpoint observations on the retained native effect worker.
use avesra_contracts::ErrorCode;
use avesra_core::{
    diagnostics::{DiskSpace, Reading, Unavailable},
    download::{Context, DnsState, Endpoint, Report},
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows::Win32::Foundation::DNS_REQUEST_PENDING;
use windows::{
    Win32::{
        NetworkManagement::{
            Dns::*,
            IpHelper::{GetBestRoute2, MIB_IPFORWARD_ROW2},
        },
        Networking::WinSock::*,
        Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW},
    },
    core::PCWSTR,
};

fn unavailable<T>() -> Reading<T> {
    Reading::Unavailable {
        reason: Unavailable::CounterUnavailable,
    }
}
fn current(
    end: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<(), ErrorCode> {
    if Instant::now() >= end {
        return Err(ErrorCode::Expired);
    }
    authorize()
}
unsafe extern "system" fn completed(context: *const core::ffi::c_void, _: *mut DNS_QUERY_RESULT) {
    // The retained worker keeps this stable allocation until this release.
    unsafe { &*context.cast::<AtomicBool>() }.store(true, Ordering::Release);
}
fn query(
    name: &[u16],
    kind: u16,
    end: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<(DnsState, Vec<IpAddr>), ErrorCode> {
    current(end, authorize)?;
    let done = Box::new(AtomicBool::new(false));
    let mut result = DNS_QUERY_RESULT {
        Version: 1,
        ..Default::default()
    };
    let mut cancel = DNS_QUERY_CANCEL::default();
    let request = DNS_QUERY_REQUEST {
        Version: 1,
        QueryName: PCWSTR(name.as_ptr()),
        QueryType: kind,
        QueryOptions: u64::from(DNS_QUERY_TREAT_AS_FQDN.0),
        pQueryCompletionCallback: Some(completed),
        pQueryContext: (&*done as *const AtomicBool).cast_mut().cast(),
        ..Default::default()
    };
    let status = unsafe { DnsQueryEx(&request, &mut result, Some(&mut cancel)) };
    let mut failure = None;
    if status == DNS_REQUEST_PENDING {
        // DNS_REQUEST_PENDING: only callback retires this query.
        while !done.load(Ordering::Acquire) {
            if failure.is_none()
                && let Err(error) = current(end, authorize)
            {
                failure = Some(error);
                unsafe {
                    DnsCancelQuery(&cancel);
                }
            }
            // Cancellation is a request, not completion. Never free OS buffers
            // or release the effect worker while the callback still owns them.
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    struct Records(*mut DNS_RECORDA);
    impl Drop for Records {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { DnsFree(Some(self.0.cast()), DnsFreeRecordList) };
            }
        }
    }
    let records = Records(result.pQueryRecords);
    if let Some(error) = failure {
        return Err(error);
    }
    current(end, authorize)?;
    let code = if status == DNS_REQUEST_PENDING {
        result.QueryStatus
    } else {
        status
    };
    let state = match code {
        0 => DnsState::Resolved,
        9003 => DnsState::NameNotFound,
        9501 => DnsState::NoRecords,
        9002 => DnsState::ServerFailure,
        9005 => DnsState::Refused,
        _ => DnsState::Unavailable,
    };
    let mut addresses = Vec::new();
    let mut seen = Vec::new();
    let mut pointer = records.0;
    while !pointer.is_null() {
        if seen.len() >= 64 || seen.contains(&pointer) {
            return Err(ErrorCode::TooLarge);
        }
        seen.push(pointer);
        let record = unsafe { &*pointer };
        let ip = match record.wType {
            1 if record.wDataLength >= 4 => Some(IpAddr::V4(Ipv4Addr::from(
                unsafe { record.Data.A.IpAddress }.to_ne_bytes(),
            ))),
            28 if record.wDataLength >= 16 => Some(IpAddr::V6(Ipv6Addr::from(unsafe {
                record.Data.AAAA.Ip6Address.IP6Byte
            }))),
            _ => None,
        };
        if let Some(ip) = ip
            && !ip.is_loopback()
            && !ip.is_unspecified()
            && !ip.is_multicast()
            && !addresses.contains(&ip)
            && addresses.len() < 4
        {
            addresses.push(ip);
        }
        pointer = record.pNext;
    }
    Ok((
        if state == DnsState::Resolved && addresses.is_empty() {
            DnsState::Unavailable
        } else {
            state
        },
        addresses,
    ))
}
fn route(ip: IpAddr) -> Reading<u32> {
    let destination = match ip {
        IpAddr::V4(ip) => SOCKADDR_INET {
            Ipv4: SOCKADDR_IN {
                sin_family: AF_INET,
                sin_addr: IN_ADDR {
                    S_un: IN_ADDR_0 {
                        S_addr: u32::from_ne_bytes(ip.octets()),
                    },
                },
                ..Default::default()
            },
        },
        IpAddr::V6(ip) => SOCKADDR_INET {
            Ipv6: SOCKADDR_IN6 {
                sin6_family: AF_INET6,
                sin6_addr: IN6_ADDR {
                    u: IN6_ADDR_0 { Byte: ip.octets() },
                },
                ..Default::default()
            },
        },
    };
    let mut best = MIB_IPFORWARD_ROW2::default();
    let mut source = SOCKADDR_INET::default();
    if unsafe { GetBestRoute2(None, 0, None, &destination, 0, &mut best, &mut source) }.is_err()
        || best.InterfaceIndex == 0
    {
        unavailable()
    } else {
        Reading::Available {
            value: best.InterfaceIndex,
        }
    }
}
pub fn run(
    context: &Context,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Report, ErrorCode> {
    context.validate()?;
    let end = Instant::now() + Duration::from_secs(10);
    current(end, authorize)?;
    let resources = crate::diagnostics::host_resources(
        context.input.destination_drive,
        end.min(Instant::now() + Duration::from_secs(5)),
        authorize,
    )?;
    let name: Vec<u16> = context
        .input
        .endpoint
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let dns_started = Instant::now();
    let (dns_a, dns_aaaa, addresses) = if let Ok(ip) = context.input.endpoint.parse::<IpAddr>() {
        (DnsState::Numeric, DnsState::Numeric, vec![ip])
    } else {
        let dns_end = end.min(Instant::now() + Duration::from_secs(5));
        let mut observed_query = |kind| match query(&name, kind, dns_end, authorize) {
            Err(ErrorCode::Expired) => {
                authorize()?;
                Ok((DnsState::TimedOut, Vec::new()))
            }
            value => value,
        };
        let (a, mut addresses) = observed_query(1)?;
        let (aaaa, other) = observed_query(28)?;
        for ip in other {
            if addresses.len() < 4 && !addresses.contains(&ip) {
                addresses.push(ip);
            }
        }
        (a, aaaa, addresses)
    };
    let dns_micros =
        u64::try_from(dns_started.elapsed().as_micros()).map_err(|_| ErrorCode::Expired)?;
    let mut endpoints = Vec::new();
    for address in addresses {
        authorize()?;
        if Instant::now() >= end {
            endpoints.push(Endpoint {
                address,
                route_interface: Reading::Unavailable {
                    reason: Unavailable::Expired,
                },
                tcp_connect_micros: Reading::Unavailable {
                    reason: Unavailable::Expired,
                },
            });
            continue;
        }
        let route_interface = route(address);
        let start = Instant::now();
        let timeout = end
            .saturating_duration_since(start)
            .min(Duration::from_millis(500));
        if timeout.is_zero() {
            authorize()?;
            endpoints.push(Endpoint {
                address,
                route_interface,
                tcp_connect_micros: Reading::Unavailable {
                    reason: Unavailable::Expired,
                },
            });
            continue;
        }
        let tcp_connect_micros = match TcpStream::connect_timeout(
            &SocketAddr::new(address, context.input.port),
            timeout,
        ) {
            Ok(socket) => {
                drop(socket);
                Reading::Available {
                    value: u64::try_from(start.elapsed().as_micros())
                        .map_err(|_| ErrorCode::Expired)?,
                }
            }
            Err(error) => Reading::Unavailable {
                reason: if error.kind() == std::io::ErrorKind::TimedOut {
                    Unavailable::Expired
                } else {
                    Unavailable::CommandFailed
                },
            },
        };
        authorize()?;
        endpoints.push(Endpoint {
            address,
            route_interface,
            tcp_connect_micros,
        });
    }
    let destination_space = if let Some(drive) = context.input.destination_drive {
        let root = [drive as u16, b':' as u16, b'\\' as u16, 0];
        let (mut available, mut total) = (0, 0);
        // DRIVE_FIXED (3): never let an optional mapped network drive trigger
        // an unrelated remote filesystem operation.
        if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != 3 {
            Reading::Unavailable {
                reason: Unavailable::Unsupported,
            }
        } else if Instant::now() >= end {
            Reading::Unavailable {
                reason: Unavailable::Expired,
            }
        } else if unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(root.as_ptr()),
                Some(&mut available),
                Some(&mut total),
                None,
            )
        }
        .is_ok()
            && available <= total
        {
            Reading::Available {
                value: DiskSpace {
                    drive,
                    caller_available_bytes: available,
                    caller_total_bytes: total,
                },
            }
        } else {
            unavailable()
        }
    } else {
        Reading::Unavailable {
            reason: Unavailable::Missing,
        }
    };
    authorize()?;
    let report = Report {
        context: context.id,
        revision: context.revision,
        dns_a,
        dns_aaaa,
        dns_micros,
        endpoints,
        destination_space,
        resources: Box::new(resources),
    };
    report.validate()?;
    Ok(report)
}

/// Fixed reviewed executable/argument. Exit completion is not a repaired DNS claim.
pub fn flush(
    authority: &mut avesra_core::execution::EffectAuthority<'_>,
) -> Result<bool, ErrorCode> {
    use sha2::{Digest, Sha256};
    use std::{
        fs::OpenOptions,
        io::Read,
        os::windows::{fs::OpenOptionsExt, process::CommandExt},
        process::{Command, Stdio},
    };
    const PATH: &str = r"C:\Windows\System32\ipconfig.exe";
    let end = Instant::now() + Duration::from_secs(5);
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(PATH)
        .map_err(|_| ErrorCode::Unavailable)?;
    if file.metadata().map_err(|_| ErrorCode::Unavailable)?.len() != 61440 {
        return Err(ErrorCode::Unsupported);
    }
    let mut bytes = Vec::with_capacity(61440);
    file.by_ref()
        .take(61441)
        .read_to_end(&mut bytes)
        .map_err(|_| ErrorCode::Unavailable)?;
    if bytes.len() != 61440
        || format!("{:x}", Sha256::digest(&bytes))
            != "8a013c65ff778cf8341cd0b3404f6e323c5ff91b5059ac47f3bfe2cd6208f6a1"
    {
        return Err(ErrorCode::Unsupported);
    }
    current(end, &mut || authority.current())?;
    authority.commit()?;
    let child = Command::new(PATH)
        .arg("/flushdns")
        .current_dir(r"C:\Windows\System32")
        .creation_flags(0x0800_0000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ErrorCode::Unavailable)?;
    struct Owned(std::process::Child, bool);
    impl Drop for Owned {
        fn drop(&mut self) {
            while !self.1 {
                let _ = self.0.kill();
                if matches!(self.0.try_wait(), Ok(Some(_))) {
                    self.1 = true;
                } else {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
    let mut child = Owned(child, false);
    let mut failed = false;
    loop {
        failed |= current(end, &mut || authority.current()).is_err();
        if failed {
            let _ = child.0.kill();
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                child.1 = true;
                return Ok(!failed && status.success());
            }
            Err(_) => failed = true,
            Ok(None) => {}
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
