//! Read only this controller's counters and fixed host GPU metadata.
use avesra_core::{
    resource_observer::{self as r, Metric as M, Reading, Reason, Value},
    trace::Host,
};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use uuid::Uuid;
static STARTED: AtomicBool = AtomicBool::new(false);
fn read(path: impl AsRef<Path>, cap: usize) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(u64::try_from(cap).ok()?.checked_add(1)?)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > cap {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn number(path: impl AsRef<Path>) -> Option<u64> {
    read(path, 128)?.trim().parse().ok()
}
fn scaled(input: &str, scale: u64) -> Option<u64> {
    let input = input.trim();
    let (whole, fraction) = input.split_once('.').unwrap_or((input, ""));
    if whole.is_empty()
        || !whole.bytes().all(|v| v.is_ascii_digit())
        || fraction.len() > 3
        || !fraction.bytes().all(|v| v.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.parse::<u64>().ok()?.checked_mul(scale)?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>().ok()?.checked_mul(scale)?
            / 10u64.checked_pow(u32::try_from(fraction.len()).ok()?)?
    };
    whole.checked_add(fraction)
}
fn process(hz: u64, page: u64) -> Option<(u64, u64, u64)> {
    let body = read("/proc/self/stat", 8192)?;
    let (_, tail) = body.rsplit_once(')')?;
    let fields = tail.split_whitespace().collect::<Vec<_>>();
    let n = |index: usize| fields.get(index)?.parse::<u64>().ok();
    let cpu = n(11)?
        .checked_add(n(12)?)?
        .checked_mul(1_000_000_000)?
        .checked_div(hz)?;
    Some((
        n(19)?.checked_mul(1000)?.checked_div(hz)?,
        cpu,
        n(21)?.checked_mul(page)?,
    ))
}
fn own_cgroup() -> Option<PathBuf> {
    let body = read("/proc/self/cgroup", 4096)?;
    let mut paths = body.lines().filter_map(|v| v.strip_prefix("0::"));
    let path = paths.next()?;
    if paths.next().is_some()
        || !path.starts_with('/')
        || Path::new(path)
            .components()
            .any(|v| !matches!(v, Component::RootDir | Component::Normal(_)))
    {
        return None;
    }
    let root = Path::new("/sys/fs/cgroup").canonicalize().ok()?;
    let value = root
        .join(path.trim_start_matches('/'))
        .canonicalize()
        .ok()?;
    value.starts_with(&root).then_some(value)
}
fn value(metric: M, n: Option<u64>) -> Value {
    n.map_or_else(
        || Value::unavailable(metric, Reason::Unavailable),
        |v| Value::measured(metric, v),
    )
}
fn limit(metric: M, path: PathBuf) -> Value {
    match read(path, 128) {
        Some(v) if v.trim() == "max" => Value::unavailable(metric, Reason::Unlimited),
        Some(v) => value(metric, v.trim().parse().ok()),
        None => Value::unavailable(metric, Reason::Unavailable),
    }
}
fn gpu_unavailable(reason: Reason) -> Vec<Value> {
    [
        M::GpuBusy,
        M::GpuTemperatureMilliC,
        M::GpuPowerMilliW,
        M::GpuMemoryBytes,
        M::GpuMemoryTotalBytes,
    ]
    .into_iter()
    .map(|metric| Value {
        metric,
        gpu: None,
        reading: Reading::Unavailable { reason },
    })
    .collect()
}
fn gpu() -> Vec<Value> {
    let start = Instant::now();
    let Ok(mut child) = Command::new("/usr/bin/nvidia-smi")
        .env_clear()
        .env("LC_ALL", "C")
        .args([
            "--query-gpu=index,utilization.gpu,temperature.gpu,power.draw,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return gpu_unavailable(Reason::Unavailable);
    };
    let Some(stdout) = child.stdout.take() else {
        retire(&mut child);
        return gpu_unavailable(Reason::Unavailable);
    };
    let reader = std::thread::Builder::new()
        .name("avesra-resource-gpu-read".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(4097).read_to_end(&mut bytes).map(|_| bytes)
        });
    let Ok(reader) = reader else {
        retire(&mut child);
        return gpu_unavailable(Reason::Unavailable);
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < Duration::from_secs(1) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                retire(&mut child);
                break None;
            }
        }
    };
    // Retain the one owner until both actual child and actual pipe reader finish.
    let bytes = reader.join();
    if status.is_none() {
        return gpu_unavailable(Reason::Timeout);
    }
    if !status.is_some_and(|v| v.success()) {
        return gpu_unavailable(Reason::Unavailable);
    }
    let Ok(Ok(bytes)) = bytes else {
        return gpu_unavailable(Reason::Unavailable);
    };
    if bytes.len() > 4096 {
        return gpu_unavailable(Reason::Invalid);
    }
    let Ok(body) = String::from_utf8(bytes) else {
        return gpu_unavailable(Reason::Invalid);
    };
    let rows = body.lines().collect::<Vec<_>>();
    if rows.is_empty() || rows.len() > 4 {
        return gpu_unavailable(Reason::Invalid);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for row in rows {
        let fields = row.split(',').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 6 {
            return gpu_unavailable(Reason::Invalid);
        }
        let Ok(index) = fields[0].parse::<u8>() else {
            return gpu_unavailable(Reason::Invalid);
        };
        if index >= 4 || !seen.insert(index) {
            return gpu_unavailable(Reason::Invalid);
        }
        for ((metric, scale), field) in [
            (M::GpuBusy, 100),
            (M::GpuTemperatureMilliC, 1000),
            (M::GpuPowerMilliW, 1000),
            (M::GpuMemoryBytes, 1_048_576),
            (M::GpuMemoryTotalBytes, 1_048_576),
        ]
        .into_iter()
        .zip(&fields[1..])
        {
            let mut v = if matches!(*field, "N/A" | "[N/A]" | "[Not Supported]") {
                Value::unavailable(metric, Reason::Unsupported)
            } else {
                scaled(field, scale).map_or_else(
                    || Value::unavailable(metric, Reason::Invalid),
                    |v| Value::measured(metric, v),
                )
            };
            v.gpu = Some(index);
            result.push(v);
        }
    }
    result
}
fn retire(child: &mut std::process::Child) {
    loop {
        let _ = child.kill();
        if child.wait().is_ok() {
            return;
        }
        // Uncertain OS retirement retains this owner and suppresses all future
        // probes; a timeout alone never permits a replacement child.
        std::thread::sleep(Duration::from_secs(5));
    }
}
pub fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    if std::thread::Builder::new()
        .name("avesra-resource-observer".into())
        .spawn(|| {
            // Linux nice applies to this sampler thread; children inherit it.
            if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) } != 0 {
                return;
            }
            let hz = u64::try_from(unsafe { libc::sysconf(libc::_SC_CLK_TCK) })
                .ok()
                .filter(|v| *v > 0);
            let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) })
                .ok()
                .filter(|v| *v > 0);
            let boot = read("/proc/sys/kernel/random/boot_id", 64)
                .and_then(|v| Uuid::parse_str(v.trim()).ok());
            let mut clock = r::Clock::new(Host::Controller);
            let mut prior = None;
            let mut identity = None;
            let mut cgroup_prior = None;
            let mut cgroup_path = None;
            let mut previous_processors = None;
            loop {
                let began = Instant::now();
                let p = hz.zip(page).and_then(|(hz, page)| process(hz, page));
                let at = Instant::now();
                let processors =
                    u32::try_from(unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) })
                        .unwrap_or(0);
                let cpu_changed = previous_processors.is_some_and(|value| value != processors);
                previous_processors = Some(processors);
                let mut values = Vec::with_capacity(32);
                let window = prior.map(|(_, start)| at.saturating_duration_since(start));
                if let Some((created, cpu, rss)) = p {
                    values.push(
                        if identity.is_some_and(|old| old != created) || cpu_changed {
                            Value::unavailable(M::ProcessCpu, Reason::Reset)
                        } else {
                            r::cpu(M::ProcessCpu, prior, cpu, at, processors)
                        },
                    );
                    values.push(Value::measured(M::ProcessWorkingBytes, rss));
                    prior = Some((cpu, at));
                    identity = Some(created);
                } else {
                    values.push(Value::unavailable(M::ProcessCpu, Reason::Unavailable));
                    values.push(Value::unavailable(
                        M::ProcessWorkingBytes,
                        Reason::Unavailable,
                    ));
                    prior = None;
                    identity = None;
                }
                let memory = read("/proc/meminfo", 16384)
                    .map(|body| {
                        body.lines()
                            .filter_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                let mut parts = value.split_whitespace();
                                let value = parts.next()?.parse::<u64>().ok()?.checked_mul(1024)?;
                                if parts.next() != Some("kB") || parts.next().is_some() {
                                    return None;
                                }
                                Some((key.to_owned(), value))
                            })
                            .collect::<BTreeMap<_, _>>()
                    })
                    .unwrap_or_default();
                for (metric, key) in [
                    (M::HostTotalBytes, "MemTotal"),
                    (M::HostAvailableBytes, "MemAvailable"),
                    (M::HostSwapTotalBytes, "SwapTotal"),
                    (M::HostSwapFreeBytes, "SwapFree"),
                ] {
                    values.push(value(metric, memory.get(key).copied()));
                }
                values.push(value(
                    M::HostUptimeMs,
                    read("/proc/uptime", 128)
                        .and_then(|v| scaled(v.split_whitespace().next()?, 1000)),
                ));
                let path = own_cgroup();
                if let Some(path) = path.as_ref() {
                    let counter = read(path.join("cpu.stat"), 4096)
                        .and_then(|v| {
                            v.lines().find_map(|line| {
                                line.strip_prefix("usage_usec ")?.trim().parse::<u64>().ok()
                            })
                        })
                        .and_then(|v| v.checked_mul(1000));
                    let current = Instant::now();
                    values.push(match counter {
                        Some(_) if cpu_changed => Value::unavailable(M::CgroupCpu, Reason::Reset),
                        Some(n) if cgroup_path.as_ref() == Some(path) => {
                            r::cpu(M::CgroupCpu, cgroup_prior, n, current, processors)
                        }
                        Some(_) if cgroup_path.is_some() => {
                            Value::unavailable(M::CgroupCpu, Reason::Reset)
                        }
                        Some(_) => Value::unavailable(M::CgroupCpu, Reason::Warmup),
                        None => Value::unavailable(M::CgroupCpu, Reason::Unavailable),
                    });
                    cgroup_prior = counter.map(|v| (v, current));
                    values.push(value(
                        M::CgroupMemoryBytes,
                        number(path.join("memory.current")),
                    ));
                    values.push(limit(M::CgroupMemoryLimitBytes, path.join("memory.max")));
                    values.push(value(
                        M::CgroupSwapBytes,
                        number(path.join("memory.swap.current")),
                    ));
                    values.push(limit(M::CgroupSwapLimitBytes, path.join("memory.swap.max")));
                } else {
                    cgroup_prior = None;
                    for metric in [
                        M::CgroupCpu,
                        M::CgroupMemoryBytes,
                        M::CgroupMemoryLimitBytes,
                        M::CgroupSwapBytes,
                        M::CgroupSwapLimitBytes,
                    ] {
                        values.push(Value::unavailable(metric, Reason::Unavailable));
                    }
                }
                cgroup_path = path;
                values.extend(gpu());
                clock.sample(began, boot, identity, window, values);
                std::thread::sleep(Duration::from_secs(5).saturating_sub(began.elapsed()));
            }
        })
        .is_err()
    {
        STARTED.store(false, Ordering::SeqCst);
    }
}
