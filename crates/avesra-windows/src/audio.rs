//! Native media primitives. Construction is explicit and never runs at startup.
//! Callbacks use fixed buffers and nonblocking bounded queues; no model/network IO.
use avesra_contracts::{ErrorCode, MAX_AUDIO_QUEUE};
use cpal::{
    FromSample, Sample, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::time::Instant;

pub const FRAME_SAMPLES: usize = 320;
#[derive(Clone)]
pub struct AudioFrame {
    pub epoch: u64,
    pub sequence: u64,
    pub captured: Instant,
    /// Hardware stream clock of the first sample, for later echo alignment.
    pub device_time: Option<cpal::StreamInstant>,
    pub samples: [i16; FRAME_SAMPLES],
    pub rms: f32,
    pub peak: f32,
}
/// Shared local invalidation gate; only trusted native state should publish it.
pub struct MediaGate {
    permission: Arc<MediaPermission>,
    attempt_epoch: Option<u64>,
    failed: AtomicBool,
    dropped: AtomicU64,
}
struct MediaPermission {
    enabled: AtomicBool,
    epoch: AtomicU64,
}
impl Default for MediaGate {
    fn default() -> Self {
        Self {
            permission: Arc::new(MediaPermission {
                enabled: AtomicBool::new(false),
                epoch: AtomicU64::new(1),
            }),
            attempt_epoch: None,
            failed: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
        }
    }
}
impl MediaGate {
    /// Every device attempt gets independent failure state and an immutable epoch.
    /// A still-opening old device can never adopt a replacement device's epoch.
    pub fn new_attempt(&self, epoch: u64) -> Arc<Self> {
        Arc::new(Self {
            permission: self.permission.clone(),
            attempt_epoch: Some(epoch),
            failed: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
        })
    }
    /// Call serialized with native state changes; disable first and invalidate
    /// before enabling a newly authenticated session. Old queue entries stay stale.
    pub fn publish(&self, enabled: bool, epoch: u64) {
        self.permission.enabled.store(false, Ordering::SeqCst);
        self.permission.epoch.store(epoch, Ordering::SeqCst);
        if enabled && !self.failed.load(Ordering::SeqCst) {
            self.permission.enabled.store(true, Ordering::SeqCst);
        }
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
    pub fn current(&self, epoch: u64) -> bool {
        self.permission.enabled.load(Ordering::SeqCst)
            && self.permission.epoch.load(Ordering::SeqCst) == epoch
            && self.attempt_epoch.is_none_or(|attempt| attempt == epoch)
            && !self.failed.load(Ordering::Relaxed)
    }
    fn fail(&self) {
        self.failed.store(true, Ordering::SeqCst);
    }
    pub fn close_attempt(&self) {
        self.fail();
    }
}
pub struct Capture {
    _stream: Stream,
    pub frames: Receiver<AudioFrame>,
    pub gate: Arc<MediaGate>,
}
pub struct Playback {
    _stream: Stream,
    pub frames: SyncSender<AudioFrame>,
    /// Actual submitted playback samples are the future echo-reference input.
    pub reference: Receiver<AudioFrame>,
    pub gate: Arc<MediaGate>,
}
fn selected(name: &str, input: bool) -> Result<cpal::Device, ErrorCode> {
    if name.is_empty() || name.len() > 512 {
        return Err(ErrorCode::Malformed);
    }
    let host = cpal::default_host();
    let devices = if input {
        host.input_devices()
    } else {
        host.output_devices()
    }
    .map_err(|_| ErrorCode::Unavailable)?;
    let mut found = None;
    for device in devices {
        if device
            .description()
            .map_err(|_| ErrorCode::Unavailable)?
            .name()
            == name
        {
            if found.is_some() {
                return Err(ErrorCode::Denied);
            }
            found = Some(device);
        }
    }
    found.ok_or(ErrorCode::Unavailable)
}
fn config_valid(config: &StreamConfig) -> bool {
    (1..=8).contains(&config.channels) && config.sample_rate == 16000
}
fn native_config(
    device: &cpal::Device,
    input: bool,
) -> Result<cpal::SupportedStreamConfig, ErrorCode> {
    let configs: Vec<_> = if input {
        device
            .supported_input_configs()
            .map_err(|_| ErrorCode::Unavailable)?
            .collect()
    } else {
        device
            .supported_output_configs()
            .map_err(|_| ErrorCode::Unavailable)?
            .collect()
    };
    configs
        .into_iter()
        .filter(|config| {
            config.min_sample_rate() <= 16000
                && config.max_sample_rate() >= 16000
                && (1..=8).contains(&config.channels())
                && matches!(
                    config.sample_format(),
                    cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
                )
        })
        .min_by_key(|config| config.channels())
        .map(|config| config.with_sample_rate(16000))
        .ok_or(ErrorCode::Unsupported)
}
impl Capture {
    pub fn open(selected_name: &str) -> Result<Self, ErrorCode> {
        Self::open_with_gate(selected_name, Arc::new(MediaGate::default()))
    }
    pub fn open_with_gate(selected_name: &str, gate: Arc<MediaGate>) -> Result<Self, ErrorCode> {
        let device = selected(selected_name, true)?;
        let supported = native_config(&device, true)?;
        let config = supported.config();
        if !config_valid(&config) {
            return Err(ErrorCode::Unsupported);
        }
        let (sender, frames) = sync_channel(MAX_AUDIO_QUEUE);
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                capture_stream::<f32>(&device, &config, sender, gate.clone())
            }
            cpal::SampleFormat::I16 => {
                capture_stream::<i16>(&device, &config, sender, gate.clone())
            }
            cpal::SampleFormat::U16 => {
                capture_stream::<u16>(&device, &config, sender, gate.clone())
            }
            _ => return Err(ErrorCode::Unsupported),
        }?;
        stream.play().map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self {
            _stream: stream,
            frames,
            gate,
        })
    }
}
fn capture_stream<T: Sample + SizedSample>(
    device: &cpal::Device,
    config: &StreamConfig,
    sender: SyncSender<AudioFrame>,
    gate: Arc<MediaGate>,
) -> Result<Stream, ErrorCode>
where
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let error_gate = gate.clone();
    let mut samples = [0i16; FRAME_SAMPLES];
    let mut count = 0usize;
    let mut epoch = 0;
    let mut sequence = 0u64;
    let mut device_time = None;
    device
        .build_input_stream(
            config,
            move |input: &[T], info: &cpal::InputCallbackInfo| {
                let current = gate.permission.epoch.load(Ordering::SeqCst);
                if current != epoch || !gate.current(current) {
                    samples.fill(0);
                    count = 0;
                    device_time = None;
                    epoch = current;
                    sequence = 0;
                }
                if !gate.current(epoch) {
                    return;
                }
                for (offset, frame) in input.chunks_exact(channels).enumerate() {
                    if !gate.current(epoch) {
                        samples.fill(0);
                        count = 0;
                        device_time = None;
                        return;
                    }
                    if count == 0 {
                        device_time = info
                            .timestamp()
                            .capture
                            .add(std::time::Duration::from_secs_f64(offset as f64 / 16000.0));
                    }
                    let mono = frame
                        .iter()
                        .map(|value| f32::from_sample(*value))
                        .sum::<f32>()
                        / channels as f32;
                    let mono = if mono.is_finite() {
                        mono.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    };
                    {
                        samples[count] = (mono * 32767.0).round() as i16;
                        count += 1;
                        if count == FRAME_SAMPLES {
                            let (sum, peak) =
                                samples.iter().fold((0.0f32, 0.0f32), |(sum, peak), value| {
                                    let value = f32::from(*value) / 32768.0;
                                    (sum + value * value, peak.max(value.abs()))
                                });
                            if let Some(next) = sequence.checked_add(1) {
                                sequence = next;
                            } else {
                                gate.fail();
                                return;
                            }
                            let packet = AudioFrame {
                                epoch,
                                sequence,
                                captured: Instant::now(),
                                device_time,
                                samples,
                                rms: (sum / FRAME_SAMPLES as f32).sqrt(),
                                peak,
                            };
                            if gate.current(epoch) && sender.try_send(packet).is_err() {
                                gate.dropped.fetch_add(1, Ordering::Relaxed);
                            }
                            samples.fill(0);
                            count = 0;
                        }
                    }
                }
            },
            move |_| error_gate.fail(),
            None,
        )
        .map_err(|_| ErrorCode::Unavailable)
}
impl Playback {
    pub fn open(selected_name: &str) -> Result<Self, ErrorCode> {
        Self::open_with_gate(selected_name, Arc::new(MediaGate::default()))
    }
    pub fn open_with_gate(selected_name: &str, gate: Arc<MediaGate>) -> Result<Self, ErrorCode> {
        let device = selected(selected_name, false)?;
        let supported = native_config(&device, false)?;
        let config = supported.config();
        if !config_valid(&config) {
            return Err(ErrorCode::Unsupported);
        }
        let (frames, receiver) = sync_channel(MAX_AUDIO_QUEUE);
        let (reference_sender, reference) = sync_channel(MAX_AUDIO_QUEUE);
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                playback_stream::<f32>(&device, &config, receiver, reference_sender, gate.clone())
            }
            cpal::SampleFormat::I16 => {
                playback_stream::<i16>(&device, &config, receiver, reference_sender, gate.clone())
            }
            cpal::SampleFormat::U16 => {
                playback_stream::<u16>(&device, &config, receiver, reference_sender, gate.clone())
            }
            _ => return Err(ErrorCode::Unsupported),
        }?;
        stream.play().map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self {
            _stream: stream,
            frames,
            reference,
            gate,
        })
    }
}
fn playback_stream<T: Sample + SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    receiver: Receiver<AudioFrame>,
    reference: SyncSender<AudioFrame>,
    gate: Arc<MediaGate>,
) -> Result<Stream, ErrorCode> {
    let channels = usize::from(config.channels);
    let error_gate = gate.clone();
    let mut current: Option<AudioFrame> = None;
    let mut index = 0usize;
    let mut last_sequence = 0u64;
    let mut epoch = 0u64;
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
                let now_epoch = gate.permission.epoch.load(Ordering::SeqCst);
                if now_epoch != epoch || !gate.current(now_epoch) {
                    current = None;
                    index = 0;
                    last_sequence = 0;
                    epoch = now_epoch;
                }
                if !gate.current(epoch) {
                    for _ in 0..MAX_AUDIO_QUEUE {
                        if receiver.try_recv().is_err() {
                            break;
                        }
                    }
                }
                let mut drain_budget = MAX_AUDIO_QUEUE;
                for (offset, frame) in output.chunks_exact_mut(channels).enumerate() {
                    if current.is_none() && gate.current(epoch) {
                        // A bounded stale drain prevents callback work scaling without limit.
                        while drain_budget > 0 {
                            drain_budget -= 1;
                            let Ok(mut packet) = receiver.try_recv() else {
                                break;
                            };
                            if packet.epoch == epoch
                                && packet.sequence > last_sequence
                                && packet.captured.elapsed().as_secs_f32() < 1.0
                            {
                                last_sequence = packet.sequence;
                                packet.device_time = info.timestamp().playback.add(
                                    std::time::Duration::from_secs_f64(offset as f64 / 16000.0),
                                );
                                current = Some(packet);
                                index = 0;
                                break;
                            }
                        }
                    }
                    let value = if gate.current(epoch) {
                        current
                            .as_ref()
                            .map_or(0.0, |packet| f32::from(packet.samples[index]) / 32768.0)
                    } else {
                        0.0
                    };
                    for channel in frame {
                        *channel = T::from_sample(value);
                    }
                    if current.is_some() {
                        index += 1;
                        if index == FRAME_SAMPLES {
                            if let Some(mut packet) = current.take() {
                                packet.captured = Instant::now();
                                if gate.current(epoch) && reference.try_send(packet).is_err() {
                                    gate.dropped.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            index = 0;
                        }
                    }
                }
            },
            move |_| error_gate.fail(),
            None,
        )
        .map_err(|_| ErrorCode::Unavailable)
}
