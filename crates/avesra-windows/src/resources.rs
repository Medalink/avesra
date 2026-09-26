//! Own-process/host counters only. Never enumerate applications or touch audio.
use avesra_core::{
    resource_observer::{self as r, Metric as M, Reason, Value},
    trace::Host,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::FILETIME,
    System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{
            ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, GetCurrentProcess, GetCurrentThread,
            GetProcessTimes, SetThreadPriority, THREAD_PRIORITY_LOWEST,
        },
    },
};
static STARTED: AtomicBool = AtomicBool::new(false);
fn ticks(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
fn cpu() -> Option<(u64, u64)> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
        .ok()?;
    }
    Some((
        ticks(created) / 10_000,
        ticks(kernel).checked_add(ticks(user))?.checked_mul(100)?,
    ))
}
pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    if std::thread::Builder::new()
        .name("avesra-resource-observer".into())
        .spawn(|| {
            // Failing low-priority setup leaves the observer unavailable, never voice.
            if unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST) }.is_err() {
                return;
            }
            let mut clock = r::Clock::new(Host::Native);
            let mut previous = None;
            let mut identity = None;
            let mut previous_processors = None;
            loop {
                let began = Instant::now();
                let observed = cpu();
                let at = Instant::now();
                let processors = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
                let mut values = Vec::with_capacity(12);
                let window = previous.map(|(_, start)| at.saturating_duration_since(start));
                match observed {
                    Some((created, counter)) => {
                        let changed = identity.is_some_and(|value| value != created)
                            || previous_processors.is_some_and(|value| value != processors);
                        values.push(if changed {
                            Value::unavailable(M::ProcessCpu, Reason::Reset)
                        } else {
                            r::cpu(M::ProcessCpu, previous, counter, at, processors)
                        });
                        previous = Some((counter, at));
                        identity = Some(created);
                    }
                    None => {
                        values.push(Value::unavailable(M::ProcessCpu, Reason::Unavailable));
                        previous = None;
                        identity = None;
                    }
                }
                previous_processors = Some(processors);
                let mut memory = PROCESS_MEMORY_COUNTERS {
                    cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                    ..Default::default()
                };
                values.push(
                    if unsafe {
                        GetProcessMemoryInfo(
                            GetCurrentProcess(),
                            &mut memory,
                            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                        )
                    }
                    .is_ok()
                    {
                        u64::try_from(memory.WorkingSetSize).map_or_else(
                            |_| Value::unavailable(M::ProcessWorkingBytes, Reason::Invalid),
                            |v| Value::measured(M::ProcessWorkingBytes, v),
                        )
                    } else {
                        Value::unavailable(M::ProcessWorkingBytes, Reason::Unavailable)
                    },
                );
                let mut host = MEMORYSTATUSEX {
                    dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                    ..Default::default()
                };
                if unsafe { GlobalMemoryStatusEx(&mut host) }.is_ok() {
                    values.push(Value::measured(M::HostTotalBytes, host.ullTotalPhys));
                    values.push(Value::measured(M::HostAvailableBytes, host.ullAvailPhys));
                } else {
                    values.push(Value::unavailable(M::HostTotalBytes, Reason::Unavailable));
                    values.push(Value::unavailable(
                        M::HostAvailableBytes,
                        Reason::Unavailable,
                    ));
                }
                values.push(Value::measured(M::HostUptimeMs, unsafe {
                    GetTickCount64()
                }));
                for metric in [
                    M::HostSwapTotalBytes,
                    M::HostSwapFreeBytes,
                    M::CgroupCpu,
                    M::CgroupMemoryBytes,
                    M::CgroupMemoryLimitBytes,
                    M::CgroupSwapBytes,
                    M::CgroupSwapLimitBytes,
                ] {
                    values.push(Value::unavailable(metric, Reason::Unsupported));
                }
                clock.sample(began, None, identity, window, values);
                std::thread::sleep(Duration::from_secs(5).saturating_sub(began.elapsed()));
            }
        })
        .is_err()
    {
        STARTED.store(false, Ordering::SeqCst);
    }
}
