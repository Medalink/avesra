//! Three fixed local counters, owned synchronously by the actual effect worker.
use super::check;
use avesra_contracts::ErrorCode;
use avesra_core::diagnostics::{DiskActivity, DiskScope, Reading, Unavailable};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use windows::{
    Win32::{
        Storage::FileSystem::{GetDriveTypeW, GetVolumeNameForVolumeMountPointW},
        System::{Performance::*, SystemInformation::GetWindowsDirectoryW},
    },
    core::PCWSTR,
};

// The SDK Pdh.h flags are absent from windows-rs 0.62.2 metadata.
const FORMAT: PDH_FMT = PDH_FMT(PDH_FMT_DOUBLE.0 | 0x0000_1000 | 0x0000_8000);
const COUNTERS: [&str; 3] = ["% Idle Time", "Disk Read Bytes/sec", "Disk Write Bytes/sec"];
// One query may be live or quarantined. A failed close permanently prevents
// another allocation in this process; no retry assumes that its handle is valid.
static QUERY_OWNED: AtomicBool = AtomicBool::new(false);
struct Query {
    handle: PDH_HQUERY,
    attempted_close: bool,
}
impl Query {
    fn close(&mut self) -> bool {
        if self.attempted_close {
            return false;
        }
        self.attempted_close = true;
        let retired = self.handle.is_invalid() || unsafe { PdhCloseQuery(self.handle) } == 0;
        if retired {
            QUERY_OWNED.store(false, Ordering::Release);
        }
        retired
    }
}
impl Drop for Query {
    fn drop(&mut self) {
        if !self.attempted_close {
            self.close();
        }
    }
}
#[derive(Clone, Copy)]
struct Raw {
    kind: u32,
    value: PDH_RAW_COUNTER,
}
struct Sample {
    finished: Instant,
    timestamp: Option<u64>,
    values: [Result<Raw, Unavailable>; 3],
}
struct Active {
    query: Query,
    counters: [Option<PDH_HCOUNTER>; 3],
    drive: char,
    scope: DiskScope,
    volume: uuid::Uuid,
    first: Sample,
}
pub(super) struct Pending {
    active: Option<Active>,
    reason: Unavailable,
}
impl Pending {
    fn unavailable(reason: Unavailable) -> Self {
        Self {
            active: None,
            reason,
        }
    }
    pub(super) fn finish(
        self,
        deadline: Instant,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Reading<DiskActivity>, ErrorCode> {
        check(deadline, authorize)?;
        let Some(mut active) = self.active else {
            return Ok(reading(Err(self.reason)));
        };
        let second = sample(&active.query, &active.counters, deadline, authorize)?;
        let observed = identity(active.drive, deadline, authorize)?;
        check(deadline, authorize)?;
        match observed {
            Ok(volume) if volume == active.volume => {}
            Ok(_) => return Ok(reading(Err(Unavailable::Changed))),
            Err(reason) => return Ok(reading(Err(reason))),
        }
        let interval_ms = u64::try_from(
            second
                .finished
                .duration_since(active.first.finished)
                .as_millis(),
        )
        .map_err(|_| ErrorCode::Expired)?;
        if !(1000..=5000).contains(&interval_ms) {
            return Ok(reading(Err(Unavailable::Expired)));
        }
        let query_time = active
            .first
            .timestamp
            .zip(second.timestamp)
            .ok_or(Unavailable::CounterUnavailable)
            .and_then(|(first, last)| advancing(first, last));
        if let Err(reason) = query_time {
            return Ok(reading(Err(reason)));
        }
        let mut values = [Err(Unavailable::CounterUnavailable); 3];
        for (index, value) in values.iter_mut().enumerate() {
            check(deadline, authorize)?;
            *value = formatted(&active, &second, index);
            check(deadline, authorize)?;
        }
        let non_idle = values[0].and_then(|idle| {
            if !(0.0..=100.0).contains(&idle) {
                return Err(Unavailable::UnrecognizedOutput);
            }
            Ok(((100.0 - idle) * 100.0).round() as u16)
        });
        let bytes = |value: Result<f64, Unavailable>| {
            value.and_then(|value| {
                if !(0.0..=avesra_contracts::browser::MAX_SAFE_COUNTER as f64).contains(&value) {
                    return Err(Unavailable::UnrecognizedOutput);
                }
                Ok(value.floor() as u64)
            })
        };
        // Query is closed synchronously here, before this report leaves the worker.
        let report = DiskActivity {
            drive: active.drive,
            scope: active.scope,
            interval_ms,
            non_idle_basis_points: reading(non_idle),
            read_bytes_per_second: reading(bytes(values[1])),
            write_bytes_per_second: reading(bytes(values[2])),
        };
        let retired = active.query.close();
        check(deadline, authorize)?;
        if !retired {
            return Ok(reading(Err(Unavailable::CounterUnavailable)));
        }
        drop(active);
        check(deadline, authorize)?;
        Ok(Reading::Available { value: report })
    }
}
pub(super) fn begin(
    destination: Option<char>,
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Pending, ErrorCode> {
    check(deadline, authorize)?;
    let scope = if destination.is_some() {
        DiskScope::SelectedDestination
    } else {
        DiskScope::SystemVolume
    };
    let drive = match destination.map_or_else(system_drive, Ok) {
        Ok(drive) => drive,
        Err(reason) => return Ok(Pending::unavailable(reason)),
    };
    check(deadline, authorize)?;
    let volume = identity(drive, deadline, authorize)?;
    check(deadline, authorize)?;
    let volume = match volume {
        Ok(value) => value,
        Err(reason) => return Ok(Pending::unavailable(reason)),
    };
    if QUERY_OWNED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(Pending::unavailable(Unavailable::CounterUnavailable));
    }
    let mut raw = PDH_HQUERY::default();
    let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut raw) };
    let query = Query {
        handle: raw,
        attempted_close: false,
    };
    check(deadline, authorize)?;
    if status != 0 || query.handle.is_invalid() {
        return Ok(Pending::unavailable(Unavailable::CounterUnavailable));
    }
    let mut counters = [None; 3];
    for (index, name) in COUNTERS.iter().enumerate() {
        check(deadline, authorize)?;
        // Drive is one ASCII uppercase letter; the rest is a fixed local path.
        let path = format!("\\LogicalDisk({drive}:)\\{name}")
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut counter = PDH_HCOUNTER::default();
        let status =
            unsafe { PdhAddEnglishCounterW(query.handle, PCWSTR(path.as_ptr()), 0, &mut counter) };
        check(deadline, authorize)?;
        if status == 0 && !counter.is_invalid() {
            counters[index] = Some(counter);
        }
    }
    if counters.iter().all(Option::is_none) {
        return Ok(Pending::unavailable(Unavailable::CounterUnavailable));
    }
    let first = sample(&query, &counters, deadline, authorize)?;
    Ok(Pending {
        active: Some(Active {
            query,
            counters,
            drive,
            scope,
            volume,
            first,
        }),
        reason: Unavailable::Missing,
    })
}
fn reading<T>(value: Result<T, Unavailable>) -> Reading<T> {
    match value {
        Ok(value) => Reading::Available { value },
        Err(reason) => Reading::Unavailable { reason },
    }
}
fn system_drive() -> Result<char, Unavailable> {
    let mut path = [0u16; 32768];
    let length = unsafe { GetWindowsDirectoryW(Some(&mut path)) } as usize;
    if length < 3
        || length >= path.len()
        || path[1] != u16::from(b':')
        || path[2] != u16::from(b'\\')
    {
        return Err(Unavailable::Unsupported);
    }
    char::from_u32(u32::from(path[0]))
        .filter(char::is_ascii_alphabetic)
        .map(|v| v.to_ascii_uppercase())
        .ok_or(Unavailable::Unsupported)
}
fn identity(
    drive: char,
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Result<uuid::Uuid, Unavailable>, ErrorCode> {
    check(deadline, authorize)?;
    if !drive.is_ascii_uppercase() {
        return Ok(Err(Unavailable::Unsupported));
    }
    let root = [drive as u16, u16::from(b':'), u16::from(b'\\'), 0];
    let kind = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    check(deadline, authorize)?;
    if kind != 3 {
        return Ok(Err(Unavailable::Unsupported));
    }
    let mut name = [0u16; 50];
    let result = unsafe { GetVolumeNameForVolumeMountPointW(PCWSTR(root.as_ptr()), &mut name) };
    check(deadline, authorize)?;
    Ok((|| {
        result.map_err(|_| Unavailable::CounterUnavailable)?;
        let end = name
            .iter()
            .position(|c| *c == 0)
            .ok_or(Unavailable::UnrecognizedOutput)?;
        let name = String::from_utf16(&name[..end]).map_err(|_| Unavailable::UnrecognizedOutput)?;
        let id = name
            .strip_prefix(r"\\?\Volume{")
            .and_then(|v| v.strip_suffix("}\\"))
            .ok_or(Unavailable::UnrecognizedOutput)?;
        uuid::Uuid::parse_str(id)
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or(Unavailable::UnrecognizedOutput)
    })())
}
fn ticks(value: &PDH_RAW_COUNTER) -> u64 {
    (u64::from(value.TimeStamp.dwHighDateTime) << 32) | u64::from(value.TimeStamp.dwLowDateTime)
}
fn valid(status: u32) -> bool {
    matches!(status, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA)
}
fn sample(
    query: &Query,
    counters: &[Option<PDH_HCOUNTER>; 3],
    deadline: Instant,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Sample, ErrorCode> {
    check(deadline, authorize)?;
    let mut timestamp = 0i64;
    let collected = unsafe { PdhCollectQueryDataWithTime(query.handle, &mut timestamp) };
    let finished = Instant::now();
    check(deadline, authorize)?;
    let mut values = [Err(Unavailable::CounterUnavailable); 3];
    if collected == 0 {
        for (index, counter) in counters.iter().enumerate() {
            let Some(counter) = counter else {
                continue;
            };
            check(deadline, authorize)?;
            let mut raw = PDH_RAW_COUNTER::default();
            let mut kind = 0;
            let status = unsafe { PdhGetRawCounterValue(*counter, Some(&mut kind), &mut raw) };
            check(deadline, authorize)?;
            values[index] = if status != 0 || !valid(raw.CStatus) {
                Err(Unavailable::CounterUnavailable)
            } else {
                Ok(Raw { kind, value: raw })
            };
        }
    }
    Ok(Sample {
        finished,
        timestamp: (collected == 0)
            .then_some(timestamp)
            .and_then(|v| u64::try_from(v).ok())
            .filter(|v| *v > 0),
        values,
    })
}
fn formatted(active: &Active, second: &Sample, index: usize) -> Result<f64, Unavailable> {
    let first = active.first.values[index]?;
    let last = second.values[index]?;
    if first.kind != last.kind || first.value.MultiCount != last.value.MultiCount {
        return Err(Unavailable::Changed);
    }
    if first.value.FirstValue < 0
        || first.value.SecondValue < 0
        || last.value.FirstValue < first.value.FirstValue
        || last.value.SecondValue < first.value.SecondValue
    {
        return Err(Unavailable::CounterReset);
    }
    advancing(ticks(&first.value), ticks(&last.value))?;
    // WinPerf.h: PERF_PRECISION_100NS_TIMER and PERF_COUNTER_BULK_COUNT.
    let expected = if index == 0 { 0x2057_0500 } else { 0x1041_0500 };
    if first.kind != expected {
        return Err(Unavailable::Unsupported);
    }
    let mut value = PDH_FMT_COUNTERVALUE::default();
    let mut kind = 0;
    let status = unsafe {
        PdhGetFormattedCounterValue(
            active.counters[index].ok_or(Unavailable::Missing)?,
            FORMAT,
            Some(&mut kind),
            &mut value,
        )
    };
    if status != 0 || !valid(value.CStatus) {
        return Err(Unavailable::CounterUnavailable);
    }
    if kind != expected {
        return Err(Unavailable::Changed);
    }
    // FORMAT fixes the union discriminator to a double.
    let number = unsafe { value.Anonymous.doubleValue };
    if !number.is_finite() || number < 0.0 {
        return Err(Unavailable::UnrecognizedOutput);
    }
    Ok(number)
}

fn advancing(earlier: u64, later: u64) -> Result<(), Unavailable> {
    // FILETIME describes representation, not provider clock precision. Compare
    // only ordering within each source; PDH owns the provider rate denominator.
    // The reported interval is separately measured native collection time.
    if later <= earlier {
        return Err(Unavailable::CounterReset);
    }
    Ok(())
}
