//! Synchronous fixed catalog. The actual effect worker retains this work.
use avesra_contracts::ErrorCode;
use avesra_core::diagnostics::{Catalog, DiskSpace, Network, Reading, Report, Unavailable};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::FILETIME,
        NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2},
        Storage::FileSystem::GetDiskFreeSpaceExW,
        System::{SystemInformation::GetWindowsDirectoryW, Threading::GetSystemTimes},
    },
    core::PCWSTR,
};

fn unavailable<T>(reason: Unavailable) -> Reading<T> {
    Reading::Unavailable { reason }
}
fn check(
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<(), ErrorCode> {
    if Instant::now() >= deadline {
        return Err(ErrorCode::Expired);
    }
    authorize()
}
#[derive(Clone, Copy)]
struct Cpu {
    idle: u64,
    kernel: u64,
    user: u64,
}
fn ticks(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
fn cpu() -> Option<Cpu> {
    let (mut idle, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).ok()?;
    }
    Some(Cpu {
        idle: ticks(idle),
        kernel: ticks(kernel),
        user: ticks(user),
    })
}
struct Counters {
    luid: u64,
    name: String,
    kind: u32,
    status: i32,
    rx_link: u64,
    tx_link: u64,
    rx: u64,
    tx: u64,
}
fn network() -> Result<Vec<Counters>, Unavailable> {
    let mut raw = std::ptr::null_mut();
    unsafe {
        GetIfTable2(&mut raw)
            .ok()
            .map_err(|_| Unavailable::CounterUnavailable)?;
    }
    if raw.is_null() {
        return Err(Unavailable::CounterUnavailable);
    }
    struct Table(*mut MIB_IF_TABLE2);
    impl Drop for Table {
        fn drop(&mut self) {
            unsafe {
                FreeMibTable(self.0.cast());
            }
        }
    }
    let table = Table(raw);
    let count = unsafe { (*table.0).NumEntries as usize };
    if count > 64 {
        return Err(Unavailable::OutputLimit);
    }
    // Use the binding's actual array address/stride; the header can have padding.
    let rows = unsafe { std::slice::from_raw_parts((*table.0).Table.as_ptr(), count) };
    let mut result = Vec::with_capacity(count);
    for row in rows {
        let luid = unsafe { row.InterfaceLuid.Value };
        if luid == 0 || result.iter().any(|v: &Counters| v.luid == luid) {
            return Err(Unavailable::CounterUnavailable);
        }
        let length = row
            .Alias
            .iter()
            .position(|c| *c == 0)
            .ok_or(Unavailable::CounterUnavailable)?;
        let full = String::from_utf16(&row.Alias[..length])
            .map_err(|_| Unavailable::CounterUnavailable)?;
        let mut name = String::new();
        for character in full.chars() {
            if character.is_control() {
                return Err(Unavailable::CounterUnavailable);
            }
            if name.len() + character.len_utf8() > 96 {
                break;
            }
            name.push(character);
        }
        result.push(Counters {
            luid,
            name,
            kind: row.Type,
            status: row.OperStatus.0,
            rx_link: row.ReceiveLinkSpeed,
            tx_link: row.TransmitLinkSpeed,
            rx: row.InOctets,
            tx: row.OutOctets,
        });
    }
    Ok(result)
}
fn disk() -> Reading<DiskSpace> {
    let mut directory = [0u16; 32768];
    let length = unsafe { GetWindowsDirectoryW(Some(&mut directory)) } as usize;
    if length < 3
        || length >= directory.len()
        || directory[1] != b':' as u16
        || directory[2] != b'\\' as u16
    {
        return unavailable(Unavailable::Unsupported);
    }
    let Some(drive) = char::from_u32(u32::from(directory[0])).filter(char::is_ascii_alphabetic)
    else {
        return unavailable(Unavailable::Unsupported);
    };
    let root = [
        drive.to_ascii_uppercase() as u16,
        b':' as u16,
        b'\\' as u16,
        0,
    ];
    let (mut available, mut total) = (0, 0);
    if unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            Some(&mut available),
            Some(&mut total),
            None,
        )
    }
    .is_err()
        || available > total
    {
        return unavailable(Unavailable::CounterUnavailable);
    }
    Reading::Available {
        value: DiskSpace {
            drive: drive.to_ascii_uppercase(),
            caller_available_bytes: available,
            caller_total_bytes: total,
        },
    }
}
fn rate(before: u64, after: u64, interval: Duration) -> Reading<u64> {
    let Some(delta) = after.checked_sub(before) else {
        return unavailable(Unavailable::CounterReset);
    };
    let nanos = interval.as_nanos();
    let Some(value) = (u128::from(delta) * 1_000_000_000)
        .checked_div(nanos)
        .and_then(|v| u64::try_from(v).ok())
    else {
        return unavailable(Unavailable::CounterUnavailable);
    };
    Reading::Available { value }
}
fn default_routes() -> Reading<Vec<avesra_core::diagnostics::DefaultRoute>> {
    use windows::Win32::{
        NetworkManagement::IpHelper::{GetIpForwardTable2, MIB_IPFORWARD_TABLE2},
        Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC},
    };
    let mut pointer = std::ptr::null_mut();
    if unsafe { GetIpForwardTable2(AF_UNSPEC, &mut pointer) }.is_err() || pointer.is_null() {
        return unavailable(Unavailable::CounterUnavailable);
    }
    struct Table(*mut MIB_IPFORWARD_TABLE2);
    impl Drop for Table {
        fn drop(&mut self) {
            unsafe { FreeMibTable(self.0.cast()) };
        }
    }
    let table = Table(pointer);
    let count = unsafe { (*table.0).NumEntries as usize };
    if count > 4096 {
        return unavailable(Unavailable::OutputLimit);
    }
    let rows = unsafe { std::slice::from_raw_parts((*table.0).Table.as_ptr(), count) };
    let mut values = Vec::new();
    for row in rows
        .iter()
        .filter(|r| r.DestinationPrefix.PrefixLength == 0 && !r.Loopback)
    {
        if values.len() >= 64 {
            return unavailable(Unavailable::OutputLimit);
        }
        let family = unsafe { row.DestinationPrefix.Prefix.si_family };
        let ipv6 = if family == AF_INET6 {
            true
        } else if family == AF_INET {
            false
        } else {
            return unavailable(Unavailable::UnrecognizedOutput);
        };
        if row.InterfaceIndex == 0 {
            return unavailable(Unavailable::UnrecognizedOutput);
        }
        values.push(avesra_core::diagnostics::DefaultRoute {
            interface_index: row.InterfaceIndex,
            ipv6,
            route_metric: row.Metric,
        });
    }
    Reading::Available { value: values }
}
fn ipv4_dns_servers() -> Reading<u16> {
    use windows::Win32::NetworkManagement::IpHelper::{
        FIXED_INFO_W2KSP1, GetNetworkParams, IP_ADDR_STRING,
    };
    let mut bytes = 0;
    let _ = unsafe { GetNetworkParams(None, &mut bytes) };
    if bytes < std::mem::size_of::<FIXED_INFO_W2KSP1>() as u32 || bytes > 65536 {
        return unavailable(Unavailable::OutputLimit);
    }
    // Aligned bounded API buffer. Host/domain/scope names are never projected.
    let mut buffer = zeroize::Zeroizing::new(vec![0u64; (bytes as usize).div_ceil(8)]);
    let base = buffer.as_mut_ptr().cast::<FIXED_INFO_W2KSP1>();
    if unsafe { GetNetworkParams(Some(base), &mut bytes) }.is_err() {
        return unavailable(Unavailable::CounterUnavailable);
    }
    let low = base as usize;
    let high = low + buffer.len() * 8;
    let mut pointer = unsafe { &raw mut (*base).DnsServerList };
    let mut seen = Vec::new();
    let mut count = 0;
    while !pointer.is_null() {
        let address = pointer as usize;
        if seen.len() >= 64
            || seen.contains(&address)
            || address < low
            || address
                .checked_add(std::mem::size_of::<IP_ADDR_STRING>())
                .is_none_or(|end| end > high)
            || !address.is_multiple_of(std::mem::align_of::<IP_ADDR_STRING>())
        {
            return unavailable(Unavailable::UnrecognizedOutput);
        }
        seen.push(address);
        let row = unsafe { &*pointer };
        let value = row.IpAddress.String;
        let Some(length) = value.iter().position(|v| *v == 0) else {
            return unavailable(Unavailable::UnrecognizedOutput);
        };
        let text: Vec<u8> = value[..length].iter().map(|v| *v as u8).collect();
        if !text.is_empty() {
            let ip = std::str::from_utf8(&text)
                .ok()
                .and_then(|s| s.parse::<std::net::Ipv4Addr>().ok());
            match ip {
                Some(ip) if !ip.is_unspecified() => count += 1,
                Some(_) => {}
                None => return unavailable(Unavailable::UnrecognizedOutput),
            }
        }
        pointer = row.Next;
    }
    Reading::Available { value: count }
}
pub fn run(
    catalog: Catalog,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Report, ErrorCode> {
    let deadline = Instant::now() + Duration::from_secs(5);
    check(deadline, authorize)?;
    if catalog == Catalog::CiscoVpnStatus {
        return crate::vpn::observe(deadline, authorize);
    }
    let first_cpu = cpu();
    let first_network = network();
    let origin = Instant::now();
    while origin.elapsed() < Duration::from_secs(1) {
        check(deadline, authorize)?;
        std::thread::sleep(Duration::from_millis(20));
    }
    check(deadline, authorize)?;
    let last_cpu = cpu();
    let last_network = network();
    let interval = origin.elapsed();
    let cpu_busy_basis_points = match first_cpu.zip(last_cpu).and_then(|(a, b)| {
        let total = b
            .kernel
            .checked_sub(a.kernel)?
            .checked_add(b.user.checked_sub(a.user)?)?;
        let busy = total.checked_sub(b.idle.checked_sub(a.idle)?)?;
        if total == 0 {
            return None;
        }
        u16::try_from(u128::from(busy) * 10_000 / u128::from(total)).ok()
    }) {
        Some(value) => Reading::Available { value },
        None => unavailable(Unavailable::CounterUnavailable),
    };
    let network = match (first_network, last_network) {
        (Ok(before), Ok(after)) => {
            let before: HashMap<_, _> = before.into_iter().map(|v| (v.luid, v)).collect();
            Reading::Available {
                value: after
                    .into_iter()
                    .map(|value| {
                        let prior = before
                            .get(&value.luid)
                            .filter(|old| old.kind == value.kind && old.status == value.status);
                        Network {
                            interface_luid: value.luid,
                            name: value.name,
                            interface_type: value.kind,
                            operational_status: value.status,
                            receive_link_bits_per_second: value.rx_link,
                            transmit_link_bits_per_second: value.tx_link,
                            receive_bytes_per_second: prior.map_or_else(
                                || unavailable(Unavailable::Changed),
                                |old| rate(old.rx, value.rx, interval),
                            ),
                            transmit_bytes_per_second: prior.map_or_else(
                                || unavailable(Unavailable::Changed),
                                |old| rate(old.tx, value.tx, interval),
                            ),
                        }
                    })
                    .collect(),
            }
        }
        (Err(reason), _) | (_, Err(reason)) => unavailable(reason),
    };
    let report = Report::HostResources {
        interval_ms: interval.as_millis() as u64,
        cpu_busy_basis_points,
        network,
        system_drive_space: disk(),
        disk_pressure: unavailable(Unavailable::Unsupported),
        default_routes: default_routes(),
        ipv4_dns_servers: ipv4_dns_servers(),
    };
    check(deadline, authorize)?;
    report.validate()?;
    Ok(report)
}
