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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackRate {
    Pcm16000,
    Pcm24000,
}
impl PlaybackRate {
    pub fn hz(self) -> u32 {
        match self {
            Self::Pcm16000 => 16000,
            Self::Pcm24000 => 24000,
        }
    }
    pub fn frame_samples(self) -> usize {
        self.hz() as usize / 50
    }
}
/// Native output only. Its explicit rate is independent of microphone framing.
#[derive(Clone)]
pub struct PlaybackFrame {
    pub epoch: u64,
    pub utterance: uuid::Uuid,
    pub sequence: u64,
    pub captured: Instant,
    pub deadline: Instant,
    pub device_time: Option<cpal::StreamInstant>,
    pub rate: PlaybackRate,
    pub samples: [i16; 480],
    pub valid_samples: usize,
    pub final_frame: bool,
}
impl PlaybackFrame {
    pub fn valid(&self) -> bool {
        self.epoch != 0
            && !self.utterance.is_nil()
            && self.sequence != 0
            && self.valid_samples > 0
            && self.valid_samples <= self.rate.frame_samples()
            && (self.final_frame || self.valid_samples == self.rate.frame_samples())
            && self.captured.elapsed() < std::time::Duration::from_millis(500)
            && self.deadline.saturating_duration_since(self.captured)
                <= std::time::Duration::from_secs(30)
            && Instant::now() < self.deadline
    }
}
/// Submitted device-rate mono samples, before identical channel replication.
pub struct PlaybackReference {
    pub epoch: u64,
    pub utterance: uuid::Uuid,
    pub submitted: Instant,
    pub device_time: Option<cpal::StreamInstant>,
    pub sample_rate: u32,
    pub samples: [f32; 3840],
    pub valid_samples: usize,
}
/// Shared local invalidation gate; only trusted native state should publish it.
pub struct MediaGate {
    permission: Arc<MediaPermission>,
    attempt_epoch: Option<u64>,
    deadline: Option<Instant>,
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
            deadline: None,
            failed: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
        }
    }
}
impl MediaGate {
    pub(crate) fn epoch(&self) -> u64 {
        self.permission.epoch.load(Ordering::SeqCst)
    }
    /// Every device attempt gets independent failure state and an immutable epoch.
    /// A still-opening old device can never adopt a replacement device's epoch.
    pub fn new_attempt(&self, epoch: u64) -> Arc<Self> {
        self.new_attempt_with_deadline(epoch, None)
    }
    pub fn new_attempt_with_deadline(&self, epoch: u64, deadline: Option<Instant>) -> Arc<Self> {
        Arc::new(Self {
            permission: self.permission.clone(),
            attempt_epoch: Some(epoch),
            deadline,
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
            && self
                .deadline
                .is_none_or(|deadline| Instant::now() < deadline)
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
    pub frames: SyncSender<PlaybackFrame>,
    /// Actual submitted playback samples are the future echo-reference input.
    pub reference: Receiver<PlaybackReference>,
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
fn native_config(
    device: &cpal::Device,
    input: bool,
    rate: u32,
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
            config.min_sample_rate() <= rate
                && config.max_sample_rate() >= rate
                && (1..=8).contains(&config.channels())
                && matches!(
                    config.sample_format(),
                    cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
                )
        })
        .min_by_key(|config| config.channels())
        .map(|config| config.with_sample_rate(rate))
        .ok_or(ErrorCode::Unsupported)
}
impl Capture {
    pub fn open(selected_name: &str) -> Result<Self, ErrorCode> {
        Self::open_with_gate(selected_name, Arc::new(MediaGate::default()))
    }
    pub fn open_with_gate(selected_name: &str, gate: Arc<MediaGate>) -> Result<Self, ErrorCode> {
        let device = selected(selected_name, true)?;
        let supported = native_config(&device, true, 16000).or_else(|_| {
            device
                .default_input_config()
                .map_err(|_| ErrorCode::Unavailable)
        })?;
        let config = supported.config();
        if !(1..=8).contains(&config.channels) || !(8000..=192000).contains(&config.sample_rate) {
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
    if config.sample_rate != 16000 {
        return capture_converted_stream::<T>(device, config, sender, gate);
    }
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
fn capture_converted_stream<T: Sample + SizedSample>(
    device: &cpal::Device,
    config: &StreamConfig,
    sender: SyncSender<AudioFrame>,
    gate: Arc<MediaGate>,
) -> Result<Stream, ErrorCode>
where
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    let mut converter = crate::resampling::CaptureResampler::new(config.sample_rate)?;
    let error_gate = gate.clone();
    let mut samples = [0i16; FRAME_SAMPLES];
    let mut count = 0usize;
    let mut epoch = 0;
    let mut sequence = 0u64;
    let mut origin: Option<cpal::StreamInstant> = None;
    let mut emitted = 0u64;
    device
        .build_input_stream(
            config,
            move |input: &[T], info: &cpal::InputCallbackInfo| {
                let current = gate.permission.epoch.load(Ordering::SeqCst);
                if current != epoch || !gate.current(current) {
                    converter.reset();
                    samples.fill(0);
                    count = 0;
                    sequence = 0;
                    origin = None;
                    emitted = 0;
                    epoch = current;
                }
                if !gate.current(epoch) {
                    return;
                }
                for (offset, frame) in input.chunks_exact(channels).enumerate() {
                    if !gate.current(epoch) {
                        converter.reset();
                        samples.fill(0);
                        count = 0;
                        origin = None;
                        return;
                    }
                    if origin.is_none() {
                        origin = info
                            .timestamp()
                            .capture
                            .add(std::time::Duration::from_secs_f64(offset as f64 / rate));
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
                    let output = match converter.push(mono) {
                        Ok(Some(output)) => output,
                        Ok(None) => continue,
                        Err(_) => {
                            gate.fail();
                            return;
                        }
                    };
                    for value in output {
                        if !gate.current(epoch) {
                            samples.fill(0);
                            count = 0;
                            return;
                        }
                        samples[count] = (value.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                        count += 1;
                        if count == FRAME_SAMPLES {
                            let (sum, peak) =
                                samples
                                    .iter()
                                    .fold((0.0f32, 0.0f32), |(sum, peak), sample| {
                                        let value = f32::from(*sample) / 32768.0;
                                        (sum + value * value, peak.max(value.abs()))
                                    });
                            let Some(next) = sequence.checked_add(1) else {
                                gate.fail();
                                return;
                            };
                            sequence = next;
                            // Startup filter delay is trimmed; this maps the emitted
                            // 16k timeline back to the original hardware capture clock.
                            let device_time = origin.and_then(|origin| {
                                origin.add(std::time::Duration::from_secs_f64(
                                    emitted as f64 / 16000.0,
                                ))
                            });
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
                            emitted += FRAME_SAMPLES as u64;
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
        Self::open_with_rate(selected_name, gate, PlaybackRate::Pcm24000)
    }
    pub fn open_with_rate(
        selected_name: &str,
        gate: Arc<MediaGate>,
        rate: PlaybackRate,
    ) -> Result<Self, ErrorCode> {
        let device = selected(selected_name, false)?;
        let supported = native_config(&device, false, rate.hz()).or_else(|_| {
            device
                .default_output_config()
                .map_err(|_| ErrorCode::Unavailable)
        })?;
        let config = supported.config();
        if !(1..=8).contains(&config.channels) || !(8000..=192000).contains(&config.sample_rate) {
            return Err(ErrorCode::Unsupported);
        }
        let (frames, receiver) = sync_channel(MAX_AUDIO_QUEUE);
        let (reference_sender, reference) = sync_channel(MAX_AUDIO_QUEUE);
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => playback_stream::<f32>(
                &device,
                &config,
                rate,
                receiver,
                reference_sender,
                gate.clone(),
            ),
            cpal::SampleFormat::I16 => playback_stream::<i16>(
                &device,
                &config,
                rate,
                receiver,
                reference_sender,
                gate.clone(),
            ),
            cpal::SampleFormat::U16 => playback_stream::<u16>(
                &device,
                &config,
                rate,
                receiver,
                reference_sender,
                gate.clone(),
            ),
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
    rate: PlaybackRate,
    receiver: Receiver<PlaybackFrame>,
    reference: SyncSender<PlaybackReference>,
    gate: Arc<MediaGate>,
) -> Result<Stream, ErrorCode>
where
    f32: FromSample<T>,
{
    crate::playback::output_stream(device, config, rate, receiver, reference, gate)
}
