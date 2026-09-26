//! Native-launch-only, silent, single-stream diagnostic recording.
use std::{
    fs::{File, OpenOptions},
    io::{self, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const LIMIT: Duration = Duration::from_secs(120);
const FRAMES: usize = 1024;
const POOL: usize = 16;
static MODE: OnceLock<Mode> = OnceLock::new();

struct Mode {
    files: Mutex<Option<(File, File)>>,
    origin: Instant,
    utc: SystemTime,
    notifications: bool,
}

/// Must run before constructing the application or any media worker.
pub fn configure_from_args() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mut requested = None;
    let mut notifications = false;
    while let Some(arg) = args.next() {
        if arg == "--capture-output" {
            if requested.is_some() {
                return Err("Duplicate diagnostic output argument".into());
            }
            requested = Some(PathBuf::from(args.next().ok_or("Missing output WAV path")?));
        } else if arg == "--capture-notifications" {
            if notifications {
                return Err("Duplicate diagnostic notification argument".into());
            }
            notifications = true;
        } else if arg.to_string_lossy().starts_with("--capture-notifications") {
            return Err("Use --capture-notifications with --capture-output".into());
        } else if arg.to_string_lossy().starts_with("--capture-output") {
            return Err("Use --capture-output followed by an absolute new WAV path".into());
        }
    }
    let Some(path) = requested else {
        return if notifications {
            Err("Diagnostic notifications require --capture-output".into())
        } else {
            Ok(())
        };
    };
    if !path.is_absolute() || path.extension().is_none_or(|value| value != "wav") {
        return Err("Diagnostic output requires an absolute new .wav path".into());
    }
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(".json");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Cannot create diagnostic WAV: {error}"))?;
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(sidecar)
        .map_err(|error| format!("Cannot create diagnostic metadata: {error}"))?;
    let origin = Instant::now();
    let utc = SystemTime::now();
    serde_json::to_writer(
        &mut metadata,
        &serde_json::json!({
            "state": "armed_or_incomplete", "hardware_silent": true,
            "provenance": "native_postmix_postformat_before_hardware_silencing",
            "launch_utc_unix_ns": unix_ns(utc), "maximum_recording_seconds": 120,
            "notification_recording": notifications,
        }),
    )
    .map_err(|error| error.to_string())?;
    metadata.flush().map_err(|error| error.to_string())?;
    MODE.set(Mode {
        files: Mutex::new(Some((file, metadata))),
        origin,
        utc,
        notifications,
    })
    .map_err(|_| "Diagnostic output already configured".to_string())
}

/// Read-only native provenance; there is no IPC toggle or way to disable it.
pub fn enabled() -> bool {
    MODE.get().is_some()
}
pub fn notifications_enabled() -> bool {
    MODE.get().is_some_and(|mode| mode.notifications)
}

struct Block {
    samples: [[f32; 2]; FRAMES],
    count: usize,
    first: Option<(Instant, SystemTime)>,
}

pub(crate) struct Tap {
    sender: SyncSender<Box<Block>>,
    recycled: Receiver<Box<Block>>,
    pending: Option<Box<Block>>,
    state: Arc<AtomicU8>,
    opened: Instant,
    started: Option<Instant>,
    frames: u64,
    rate: u32,
}

impl Tap {
    /// File/thread setup and allocation occur outside the real-time callback.
    pub(crate) fn open(rate: u32, channels: u16) -> io::Result<Option<Self>> {
        let Some(mode) = MODE.get() else {
            return Ok(None);
        };
        let files = mode
            .files
            .lock()
            .map_err(|_| io::Error::other("Recorder lock failed"))?
            .take();
        let Some((file, metadata)) = files else {
            return Ok(None);
        };
        let (sender, receiver) = mpsc::sync_channel(POOL);
        let (recycle, recycled) = mpsc::sync_channel(POOL);
        for _ in 0..POOL {
            recycle
                .try_send(Box::new(Block {
                    samples: [[0.0; 2]; FRAMES],
                    count: 0,
                    first: None,
                }))
                .map_err(|_| io::Error::other("Recorder pool failed"))?;
        }
        let state = Arc::new(AtomicU8::new(0));
        let worker_state = state.clone();
        let opened = Instant::now();
        std::thread::Builder::new()
            .name("output-recording".into())
            .spawn(move || {
                write_recording(Writer {
                    file,
                    metadata,
                    receiver,
                    recycle,
                    state: worker_state,
                    opened,
                    rate,
                    channels,
                });
            })?;
        Ok(Some(Self {
            sender,
            recycled,
            pending: None,
            state,
            opened,
            started: None,
            frames: 0,
            rate,
        }))
    }

    pub(crate) fn capture<T: cpal::Sample>(&mut self, output: &[T], channels: usize)
    where
        f32: cpal::FromSample<T>,
    {
        let now = Instant::now();
        if self.state.load(Ordering::Relaxed) != 0 {
            return;
        }
        if now.duration_since(self.started.unwrap_or(self.opened)) >= LIMIT {
            self.finish(if self.started.is_some() { 1 } else { 5 });
            return;
        }
        for (offset, frame) in output.chunks_exact(channels).enumerate() {
            let samples = [
                frame[0].to_sample::<f32>(),
                frame[usize::from(channels == 2)].to_sample::<f32>(),
            ];
            if self.started.is_none() && samples == [0.0; 2] {
                continue;
            }
            if self.frames >= u64::from(self.rate) * LIMIT.as_secs()
                || self.started.is_some_and(|started| {
                    now.duration_since(started).as_secs_f64() + offset as f64 / f64::from(self.rate)
                        >= LIMIT.as_secs_f64()
                })
            {
                self.finish(1);
                break;
            }
            if self.pending.is_none() {
                self.pending = self.recycled.try_recv().ok();
            }
            let Some(block) = self.pending.as_mut() else {
                self.finish(2);
                break;
            };
            if self.started.is_none() {
                self.started = Some(now);
                block.first = Some((now, SystemTime::now()));
            }
            block.samples[block.count] = samples;
            block.count += 1;
            self.frames += 1;
            if block.count == FRAMES && !self.flush() {
                break;
            }
        }
    }

    fn flush(&mut self) -> bool {
        let Some(block) = self.pending.take() else {
            return true;
        };
        match self.sender.try_send(block) {
            Ok(()) => true,
            Err(TrySendError::Full(block) | TrySendError::Disconnected(block)) => {
                // Keep the allocation owned until stream destruction, outside callback.
                self.pending = Some(block);
                self.state.store(2, Ordering::Relaxed);
                false
            }
        }
    }
    fn finish(&mut self, reason: u8) {
        if self.flush() {
            let _ = self
                .state
                .compare_exchange(0, reason, Ordering::Relaxed, Ordering::Relaxed);
        }
    }
}

impl Drop for Tap {
    fn drop(&mut self) {
        self.finish(3);
    }
}

fn unix_ns(value: SystemTime) -> Option<String> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_nanos().to_string())
}

fn header(file: &mut File, rate: u32, frames: u64) -> io::Result<()> {
    let bytes = u32::try_from(frames * 8).map_err(|_| io::Error::other("WAV too large"))?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&3u16.to_le_bytes())?; // IEEE float
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&rate.to_le_bytes())?;
    file.write_all(&(rate * 8).to_le_bytes())?;
    file.write_all(&8u16.to_le_bytes())?;
    file.write_all(&32u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())
}

struct Writer {
    file: File,
    metadata: File,
    receiver: Receiver<Box<Block>>,
    recycle: SyncSender<Box<Block>>,
    state: Arc<AtomicU8>,
    opened: Instant,
    rate: u32,
    channels: u16,
}

fn write_recording(writer: Writer) {
    let Writer {
        mut file,
        mut metadata,
        receiver,
        recycle,
        state,
        opened,
        rate,
        channels,
    } = writer;
    let mut frames = 0u64;
    let mut first = None;
    let mut bytes = [0u8; FRAMES * 8];
    let result = (|| -> io::Result<()> {
        header(&mut file, rate, 0)?;
        loop {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(mut block) => {
                    if first.is_none() {
                        first = block.first;
                    }
                    for (destination, sample) in bytes
                        .chunks_exact_mut(4)
                        .zip(block.samples[..block.count].iter().flatten())
                    {
                        destination.copy_from_slice(&sample.to_le_bytes());
                    }
                    file.write_all(&bytes[..block.count * 8])?;
                    frames += block.count as u64;
                    block.count = 0;
                    block.first = None;
                    let _ = recycle.try_send(block);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if state.load(Ordering::Relaxed) != 0 {
                        break;
                    }
                    if first.map_or(opened, |(time, _)| time).elapsed() >= LIMIT {
                        let _ = state.compare_exchange(
                            0,
                            if first.is_some() { 1 } else { 5 },
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        );
                        break;
                    }
                }
            }
        }
        header(&mut file, rate, frames)?;
        file.flush()
    })();
    if result.is_err() {
        state.store(4, Ordering::Relaxed);
    }
    let mode = MODE.get().expect("Recorder configured before writer");
    let reason = match state.load(Ordering::Relaxed) {
        1 => "duration_limit",
        2 => "queue_overflow_or_writer_disconnected",
        4 => "write_failed",
        5 => "no_signal_timeout",
        _ => "stream_disposed",
    };
    let value = serde_json::json!({
        "state": if result.is_ok() { "finalized" } else { "incomplete" },
        "provenance": "native_postmix_postformat_before_hardware_silencing", "hardware_silent": true,
        "launch_utc_unix_ns": unix_ns(mode.utc), "first_sample_utc_unix_ns": first.and_then(|(_, time)| unix_ns(time)),
        "first_sample_process_monotonic_ns": first.map(|(time, _)| time.duration_since(mode.origin).as_nanos().to_string()),
        "sample_rate": rate, "device_channels": channels, "wav_channels": 2, "frames": frames,
        "duration_seconds": frames as f64 / f64::from(rate), "maximum_recording_seconds": 120,
        "termination": reason, "audible_device_proof": false,
        "notification_recording": mode.notifications,
    });
    let _ = (|| -> io::Result<()> {
        metadata.seek(SeekFrom::Start(0))?;
        metadata.set_len(0)?;
        serde_json::to_writer(&mut metadata, &value)?;
        metadata.flush()
    })();
}
