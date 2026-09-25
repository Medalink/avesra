//! Native media primitives. Construction is explicit and never runs at startup.
//! Callbacks use fixed buffers and nonblocking bounded queues; no model/network IO.
use avesra_contracts::{ErrorCode, MAX_AUDIO_QUEUE};
use cpal::{
    FromSample, Sample, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::time::Instant;

pub const FRAME_SAMPLES: usize = 320;

/// QPC correlation has jitter; it is not the endpoint's integer frame counter.
/// Both paths use these bounds without filling or dropping admitted samples.
#[derive(Default)]
struct CaptureTiming {
    first_callback: Option<Instant>,
    clock_ready: bool,
    origin: Option<cpal::StreamInstant>,
    previous: Option<cpal::StreamInstant>,
    expected: Option<cpal::StreamInstant>,
    frames: u64,
}
impl CaptureTiming {
    fn begin_callback(
        &mut self,
        info: &cpal::InputCallbackInfo,
        frames: usize,
        rate: u32,
        entered: Instant,
    ) -> Result<bool, u8> {
        if frames == 0 {
            return Ok(false);
        }
        let first = *self.first_callback.get_or_insert(entered);
        if !self.clock_ready
            && entered.duration_since(first) >= std::time::Duration::from_millis(250)
        {
            return Err(3);
        }
        // WASAPI can report an uninitialized callback clock in its first packet.
        // Never anchor or resample that pre-roll, or skip data after admission.
        if capture_instant(info, Some(info.timestamp().capture), entered).is_none() {
            return if self.clock_ready { Err(3) } else { Ok(false) };
        }
        if !self.accept(info.timestamp().capture, frames, rate) {
            return Err(2);
        }
        self.clock_ready = true;
        Ok(true)
    }
    fn accept(&mut self, start: cpal::StreamInstant, frames: usize, rate: u32) -> bool {
        if !(8000..=192000).contains(&rate) {
            return false;
        }
        if frames == 0 {
            return true;
        }
        if self.previous.is_some_and(|previous| {
            start
                .duration_since(&previous)
                .is_none_or(|elapsed| elapsed.is_zero())
        }) {
            return false;
        }
        let rounding = std::time::Duration::from_nanos(1_000_000_000 / u64::from(rate) + 1000);
        let close_to = |expected: cpal::StreamInstant, budget: std::time::Duration| {
            start
                .duration_since(&expected)
                .or_else(|| expected.duration_since(&start))
                .is_some_and(|difference| difference <= budget + rounding)
        };
        if self
            .expected
            .is_some_and(|expected| !close_to(expected, std::time::Duration::from_millis(2)))
        {
            return false;
        }
        let origin = self.origin.unwrap_or(start);
        let Some(projected) = origin.add(std::time::Duration::from_secs_f64(
            self.frames as f64 / f64::from(rate),
        )) else {
            return false;
        };
        if !close_to(projected, std::time::Duration::from_millis(4)) {
            return false;
        }
        let Some(total) = self.frames.checked_add(frames as u64) else {
            return false;
        };
        let Some(expected) = start.add(std::time::Duration::from_secs_f64(
            frames as f64 / f64::from(rate),
        )) else {
            return false;
        };
        self.origin = Some(origin);
        self.previous = Some(start);
        self.expected = Some(expected);
        self.frames = total;
        true
    }
}
fn capture_instant(
    info: &cpal::InputCallbackInfo,
    sample: Option<cpal::StreamInstant>,
    entered: Instant,
) -> Option<Instant> {
    let age = info.timestamp().callback.duration_since(&sample?)?;
    (age <= std::time::Duration::from_millis(500))
        .then(|| entered.checked_sub(age))
        .flatten()
}
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
                <= std::time::Duration::from_secs(32)
            && Instant::now() < self.deadline
    }
}
/// Actual post-format device samples. Two channels are L/R; other layouts
/// replicate the first value to every channel and retain an equal second value.
pub struct PlaybackReference {
    /// Final content sample submitted, not proof of audible delivery.
    pub final_submitted: bool,
    pub mix_final_submitted: bool,
    pub speech: bool,
    pub channels: u16,
    pub component_peaks: [f32; 3],
    pub drain_until: Option<Instant>,
    pub epoch: u64,
    pub utterance: uuid::Uuid,
    pub submitted: Instant,
    pub device_time: Option<cpal::StreamInstant>,
    pub sample_rate: u32,
    pub samples: [[f32; 2]; 3840],
    pub valid_samples: usize,
}
/// Shared local invalidation gate; only trusted native state should publish it.
pub struct MediaGate {
    permission: Arc<MediaPermission>,
    attempt_epoch: Option<u64>,
    deadline: Option<Instant>,
    source: Option<avesra_core::conversations::PlannerCancellation>,
    caller: Option<Arc<AtomicBool>>,
    failed: AtomicBool,
    failure: AtomicU8,
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
            source: None,
            caller: None,
            failed: AtomicBool::new(false),
            failure: AtomicU8::new(0),
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
        self.output_attempt(epoch, deadline, None, None)
    }
    /// Cancellation is an atomic read in the callback, including after a blocked
    /// device constructor returns. No native runtime or database lock is touched.
    pub fn output_attempt(
        &self,
        epoch: u64,
        deadline: Option<Instant>,
        source: Option<avesra_core::conversations::PlannerCancellation>,
        caller: Option<Arc<AtomicBool>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            permission: self.permission.clone(),
            attempt_epoch: Some(epoch),
            deadline,
            source,
            caller,
            failed: AtomicBool::new(false),
            failure: AtomicU8::new(0),
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
            && self
                .source
                .as_ref()
                .is_none_or(|source| !source.cancelled())
            && self
                .caller
                .as_ref()
                .is_none_or(|caller| !caller.load(Ordering::SeqCst))
            && !self.failed.load(Ordering::Relaxed)
    }
    fn fail(&self) {
        self.fail_with(1);
    }
    fn fail_with(&self, reason: u8) {
        let _ = self
            .failure
            .compare_exchange(0, reason, Ordering::SeqCst, Ordering::SeqCst);
        self.failed.store(true, Ordering::SeqCst);
    }
    pub fn failure_reason(&self) -> &'static str {
        match self.failure.load(Ordering::SeqCst) {
            2 => {
                "Microphone audio timing was discontinuous. Retry the recording or choose another microphone."
            }
            3 => {
                "Microphone timestamps were invalid. Retry the recording or choose another microphone."
            }
            4 => "Microphone sample conversion failed. Choose another microphone and retry.",
            5 => "Microphone audio queue overflowed. Retry the recording.",
            _ => {
                "The microphone stream stopped. Check the selected device and Windows microphone access, then retry."
            }
        }
    }
    pub fn close_attempt(&self) {
        self.fail_with(5);
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
    pub reference: Receiver<Box<PlaybackReference>>,
    pub recycle: SyncSender<Box<PlaybackReference>>,
    pub channels: u16,
    pub gate: Arc<MediaGate>,
}
fn selected(name: &str, input: bool) -> Result<cpal::Device, ErrorCode> {
    if name.is_empty() || name.len() > 1024 || name.chars().any(char::is_control) {
        return Err(ErrorCode::Malformed);
    }
    let host = cpal::default_host();
    let id: cpal::DeviceId = name.parse().map_err(|_| ErrorCode::Malformed)?;
    if id.0 != host.id() || id.1.is_empty() || id.to_string() != name {
        return Err(ErrorCode::Denied);
    }
    let device = host.device_by_id(&id).ok_or(ErrorCode::Unavailable)?;
    if (input && !device.supports_input()) || (!input && !device.supports_output()) {
        return Err(ErrorCode::Unsupported);
    }
    Ok(device)
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
    let mut captured = None;
    let mut timing = CaptureTiming::default();
    device
        .build_input_stream(
            config,
            move |input: &[T], info: &cpal::InputCallbackInfo| {
                let entered = Instant::now();
                let current = gate.permission.epoch.load(Ordering::SeqCst);
                if current != epoch || !gate.current(current) {
                    samples.fill(0);
                    count = 0;
                    device_time = None;
                    captured = None;
                    timing = CaptureTiming::default();
                    epoch = current;
                    sequence = 0;
                }
                if !gate.current(epoch) {
                    return;
                }
                if !input.len().is_multiple_of(channels) {
                    samples.fill(0);
                    count = 0;
                    gate.fail_with(2);
                    return;
                }
                match timing.begin_callback(info, input.len() / channels, 16000, entered) {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(reason) => {
                        samples.fill(0);
                        count = 0;
                        gate.fail_with(reason);
                        return;
                    }
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
                        captured = capture_instant(info, device_time, entered);
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
                            let Some(captured) = captured else {
                                gate.fail_with(3);
                                return;
                            };
                            let packet = AudioFrame {
                                epoch,
                                sequence,
                                captured,
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
    let mut timing = CaptureTiming::default();
    let source_rate = config.sample_rate;
    device
        .build_input_stream(
            config,
            move |input: &[T], info: &cpal::InputCallbackInfo| {
                let entered = Instant::now();
                let current = gate.permission.epoch.load(Ordering::SeqCst);
                if current != epoch || !gate.current(current) {
                    converter.reset();
                    samples.fill(0);
                    count = 0;
                    sequence = 0;
                    origin = None;
                    emitted = 0;
                    timing = CaptureTiming::default();
                    epoch = current;
                }
                if !gate.current(epoch) {
                    return;
                }
                if !input.len().is_multiple_of(channels) {
                    converter.reset();
                    samples.fill(0);
                    count = 0;
                    gate.fail_with(2);
                    return;
                }
                match timing.begin_callback(info, input.len() / channels, source_rate, entered) {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(reason) => {
                        converter.reset();
                        samples.fill(0);
                        count = 0;
                        gate.fail_with(reason);
                        return;
                    }
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
                            gate.fail_with(4);
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
                            let Some(captured) = capture_instant(info, device_time, entered) else {
                                gate.fail_with(3);
                                return;
                            };
                            let packet = AudioFrame {
                                epoch,
                                sequence,
                                captured,
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
        let control = Arc::new(crate::sound::SoundControl::default());
        control.publish(&avesra_core::sound::SoundSettings::default(), 100);
        Self::open_with_sound(selected_name, gate, rate, control)
    }
    pub fn open_with_sound(
        selected_name: &str,
        gate: Arc<MediaGate>,
        rate: PlaybackRate,
        control: Arc<crate::sound::SoundControl>,
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
        let (recycle, recycled) = sync_channel(MAX_AUDIO_QUEUE + 1);
        for _ in 0..=MAX_AUDIO_QUEUE {
            recycle
                .try_send(Box::new(PlaybackReference {
                    final_submitted: false,
                    mix_final_submitted: false,
                    speech: false,
                    channels: config.channels,
                    component_peaks: [0.0; 3],
                    drain_until: None,
                    epoch: 0,
                    utterance: uuid::Uuid::nil(),
                    submitted: Instant::now(),
                    device_time: None,
                    sample_rate: config.sample_rate,
                    samples: [[0.0; 2]; 3840],
                    valid_samples: 0,
                }))
                .map_err(|_| ErrorCode::Unavailable)?;
        }
        let context = crate::playback::OutputContext {
            reference: reference_sender,
            recycled,
            control,
            gate: gate.clone(),
        };
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                crate::playback::output_stream::<f32>(&device, &config, rate, receiver, context)
            }
            cpal::SampleFormat::I16 => {
                crate::playback::output_stream::<i16>(&device, &config, rate, receiver, context)
            }
            cpal::SampleFormat::U16 => {
                crate::playback::output_stream::<u16>(&device, &config, rate, receiver, context)
            }
            _ => return Err(ErrorCode::Unsupported),
        }?;
        stream.play().map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self {
            _stream: stream,
            frames,
            reference,
            recycle,
            channels: config.channels,
            gate,
        })
    }
}
