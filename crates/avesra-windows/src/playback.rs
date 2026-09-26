//! Device-rate conversion with preallocated filtering and explicit finite tails.
use crate::{
    audio::{MediaGate, PlaybackFrame, PlaybackRate, PlaybackReference},
    resampling::CaptureResampler,
};
use avesra_contracts::{ErrorCode, MAX_AUDIO_QUEUE};
use cpal::{FromSample, Sample, SizedSample, Stream, StreamConfig, traits::DeviceTrait};
use std::{
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, Copy)]
struct Binding {
    epoch: u64,
    utterance: Uuid,
    deadline: Instant,
}
struct Renderer {
    filter: Option<CaptureResampler>,
    source_rate: PlaybackRate,
    output_rate: u32,
    binding: Option<Binding>,
    waiting: Option<PlaybackFrame>,
    source: Option<PlaybackFrame>,
    prefetched: Option<PlaybackFrame>,
    source_index: usize,
    sequence: u64,
    received: u64,
    target: Option<u64>,
    source_done: bool,
    padding: usize,
    emitted: u64,
    lead_remaining: usize,
    output: [f32; 3840],
    output_len: usize,
    output_index: usize,
    recent: [Option<(Uuid, Instant)>; 64],
}
impl Renderer {
    fn new(source_rate: PlaybackRate, output_rate: u32) -> Result<Self, ErrorCode> {
        Ok(Self {
            filter: if source_rate.hz() == output_rate {
                None
            } else {
                Some(CaptureResampler::between(source_rate.hz(), output_rate)?)
            },
            source_rate,
            output_rate,
            binding: None,
            waiting: None,
            source: None,
            prefetched: None,
            source_index: 0,
            sequence: 0,
            received: 0,
            target: None,
            source_done: false,
            padding: 0,
            emitted: 0,
            lead_remaining: 0,
            output: [0.0; 3840],
            output_len: 0,
            output_index: 0,
            recent: [None; 64],
        })
    }
    fn clear(&mut self) {
        if let Some(filter) = self.filter.as_mut() {
            filter.reset();
        }
        self.binding = None;
        self.waiting = None;
        self.source = None;
        self.prefetched = None;
        self.source_index = 0;
        self.sequence = 0;
        self.received = 0;
        self.target = None;
        self.source_done = false;
        self.padding = 0;
        self.emitted = 0;
        self.lead_remaining = 0;
        self.output.fill(0.0);
        self.output_len = 0;
        self.output_index = 0;
    }
    fn admit(&mut self, packet: PlaybackFrame, binding: Binding) -> Result<(), ErrorCode> {
        if !packet.valid()
            || packet.rate != self.source_rate
            || packet.epoch != binding.epoch
            || packet.utterance != binding.utterance
            || packet.deadline != binding.deadline
            || self.sequence.checked_add(1) != Some(packet.sequence)
            || self.target.is_some()
        {
            return Err(ErrorCode::Stale);
        }
        self.sequence = packet.sequence;
        self.received += packet.valid_samples as u64;
        if self.received > u64::from(self.source_rate.hz()) * 30 {
            return Err(ErrorCode::TooLarge);
        }
        if packet.final_frame {
            self.target = Some(
                (self.received * u64::from(self.output_rate))
                    .div_ceil(u64::from(self.source_rate.hz())),
            );
        }
        self.source_index = 0;
        self.source = Some(packet);
        Ok(())
    }
    fn next(
        &mut self,
        receiver: &Receiver<PlaybackFrame>,
        epoch: u64,
    ) -> Result<Option<(f32, Binding, bool, bool)>, ErrorCode> {
        if self.binding.is_none() {
            if self.waiting.is_none() {
                self.waiting = receiver.try_recv().ok();
            }
            let Some(first) = self.waiting.as_ref() else {
                return Ok(None);
            };
            if !first.valid()
                || first.epoch != epoch
                || first.rate != self.source_rate
                || first.sequence != 1
            {
                return Err(ErrorCode::Stale);
            }
            // Prebuffer two frames (40ms), or a complete short utterance, before
            // committing output. Waiting has a bounded 250ms budget, no silence gap.
            if !first.final_frame && self.prefetched.is_none() {
                self.prefetched = receiver.try_recv().ok();
                if self.prefetched.is_none() {
                    if first.captured.elapsed() >= Duration::from_millis(250) {
                        return Err(ErrorCode::Expired);
                    }
                    return Ok(None);
                }
            }
            let first = self.waiting.take().ok_or(ErrorCode::Malformed)?;
            let now = Instant::now();
            for entry in &mut self.recent {
                if entry.is_some_and(|(_, expiry)| now >= expiry) {
                    *entry = None;
                }
            }
            if self
                .recent
                .iter()
                .flatten()
                .any(|(id, _)| *id == first.utterance)
            {
                return Err(ErrorCode::Stale);
            }
            let entry = self
                .recent
                .iter_mut()
                .find(|entry| entry.is_none())
                .ok_or(ErrorCode::TooLarge)?;
            // Fixed storage, longer than the maximum 32s packet deadline. Keep
            // tombstones through filter/utterance reset; capacity fails closed.
            *entry = Some((first.utterance, now + Duration::from_secs(33)));
            let binding = Binding {
                epoch,
                utterance: first.utterance,
                deadline: first.deadline,
            };
            self.binding = Some(binding);
            self.admit(first, binding)?;
            self.lead_remaining = self.output_rate as usize * 120 / 1000;
        }
        let binding = self.binding.ok_or(ErrorCode::Malformed)?;
        if binding.epoch != epoch || Instant::now() >= binding.deadline {
            return Err(ErrorCode::Expired);
        }
        if self.lead_remaining > 0 {
            self.lead_remaining -= 1;
            return Ok(Some((0.0, binding, false, false)));
        }
        while self.output_index == self.output_len {
            self.output_len = 0;
            self.output_index = 0;
            // Input and zero-tail work is bounded even if a malformed filter never
            // produces output. No process_partial helper (which allocates) is used.
            for _ in 0..8192 {
                if self.source.is_none() && !self.source_done {
                    let packet = self
                        .prefetched
                        .take()
                        .or_else(|| receiver.try_recv().ok())
                        .ok_or(ErrorCode::Unavailable)?;
                    self.admit(packet, binding)?;
                }
                let input = if let Some(packet) = self.source.as_ref() {
                    let value = f32::from(packet.samples[self.source_index]) / 32768.0;
                    self.source_index += 1;
                    if self.source_index == packet.valid_samples {
                        self.source_done = packet.final_frame;
                        self.source = None;
                    }
                    value
                } else {
                    self.padding += 1;
                    if !self.source_done || self.padding > 8192 {
                        return Err(ErrorCode::Malformed);
                    }
                    0.0
                };
                let Some(filter) = self.filter.as_mut() else {
                    self.output[0] = input;
                    self.output_len = 1;
                    break;
                };
                if let Some(values) = filter.push(input)? {
                    let remaining = self.target.map_or(values.len(), |target| {
                        target.saturating_sub(self.emitted) as usize
                    });
                    self.output_len = values.len().min(remaining);
                    self.output[..self.output_len].copy_from_slice(&values[..self.output_len]);
                    if self.output_len > 0 {
                        break;
                    }
                }
            }
            if self.output_len == 0 {
                return Err(ErrorCode::Malformed);
            }
        }
        let value = self.output[self.output_index].clamp(-1.0, 1.0);
        self.output_index += 1;
        self.emitted += 1;
        let last = self.target == Some(self.emitted);
        if last {
            self.clear();
        }
        Ok(Some((value, binding, last, true)))
    }
}

pub(crate) struct OutputContext {
    pub reference: SyncSender<Box<PlaybackReference>>,
    pub recycled: Receiver<Box<PlaybackReference>>,
    pub control: Arc<crate::sound::SoundControl>,
    pub gate: Arc<MediaGate>,
}
struct Tail {
    binding: Binding,
    remaining: usize,
    total: usize,
}

pub(crate) fn output_stream<T: Sample + SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    rate: PlaybackRate,
    receiver: Receiver<PlaybackFrame>,
    context: OutputContext,
) -> Result<Stream, ErrorCode>
where
    f32: FromSample<T>,
{
    let OutputContext {
        reference,
        recycled,
        control,
        gate,
    } = context;
    let channels = usize::from(config.channels);
    let output_rate = config.sample_rate;
    let reference_size = output_rate as usize / 50;
    let mut renderer = Renderer::new(rate, output_rate)?;
    let mut mixer = crate::sound::Mixer::new(output_rate, &control)?;
    let mut pending: Option<Box<PlaybackReference>> = None;
    let mut reference_sequence = 0u64;
    let mut tail: Option<Tail> = None;
    let mut invalidated = false;
    let error_gate = gate.clone();
    let diagnostic_silent = crate::output_recording::enabled();
    let mut recording = crate::output_recording::Tap::open(output_rate, config.channels)
        .map_err(|_| ErrorCode::Unavailable)?;
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
                let entered = Instant::now();
                let scheduled = info
                    .timestamp()
                    .playback
                    .duration_since(&info.timestamp().callback)
                    .filter(|delay| *delay <= Duration::from_millis(500))
                    .and_then(|delay| entered.checked_add(delay));
                // Bound the complete callback's predicted DAC horizon, including
                // its last sample. A partial reference may be lost on cancellation.
                let horizon = Duration::from_secs_f64(
                    (output.len() / channels) as f64 / f64::from(output_rate),
                );
                if scheduled
                    .and_then(|start| start.checked_add(horizon))
                    .is_none_or(|end| {
                        end.saturating_duration_since(entered) > Duration::from_millis(500)
                    })
                {
                    gate.close_attempt();
                }
                let epoch = gate.epoch();
                for (offset, frame) in output.chunks_exact_mut(channels).enumerate() {
                    // Recheck authority per sample. A cosmetic fade never delays stop.
                    if !gate.current(epoch) {
                        if !invalidated {
                            renderer.clear();
                            mixer.reset();
                            tail = None;
                            if let Some(record) = pending.as_mut() {
                                record.valid_samples = 0;
                            }
                            for _ in 0..MAX_AUDIO_QUEUE {
                                if receiver.try_recv().is_err() {
                                    break;
                                }
                            }
                            invalidated = true;
                        }
                        frame.fill(T::from_sample(0.0));
                        continue;
                    }
                    if offset % 32 == 0 {
                        mixer.sync(&control);
                    }
                    let value = if let Some(active) = tail.as_mut() {
                        if Instant::now() >= active.binding.deadline {
                            Err(ErrorCode::Expired)
                        } else {
                            let elapsed = active.total - active.remaining;
                            let hold = (output_rate as usize / 5).min(active.total / 4);
                            let progress = (elapsed.saturating_sub(hold) as f32
                                / (active.total - hold).max(1) as f32)
                                .clamp(0.0, 1.0);
                            let taper = 1.0 - progress * progress * (3.0 - 2.0 * progress);
                            active.remaining -= 1;
                            let last = active.remaining == 0;
                            Ok(Some((0.0, active.binding, false, last, taper, false)))
                        }
                    } else {
                        renderer.next(&receiver, epoch).map(|sample| {
                            sample.map(|(value, binding, last, speech)| {
                                if last {
                                    // Reserve the existing conservative device-drain estimate
                                    // inside the original source deadline; never extend it.
                                    let available = binding
                                        .deadline
                                        .saturating_duration_since(Instant::now())
                                        .saturating_sub(Duration::from_millis(1100));
                                    let count = if mixer.has_tail() {
                                        (available.min(Duration::from_millis(1800)).as_secs_f64()
                                            * f64::from(output_rate))
                                            as usize
                                    } else {
                                        0
                                    };
                                    if count > 0 {
                                        tail = Some(Tail {
                                            binding,
                                            remaining: count,
                                            total: count,
                                        });
                                    }
                                }
                                (value, binding, last, last && tail.is_none(), 1.0, speech)
                            })
                        })
                    };
                    let Some((sample, binding, speech_last, mix_last, taper, speech)) = (match value
                    {
                        Ok(value) => value,
                        Err(_) => {
                            gate.close_attempt();
                            None
                        }
                    }) else {
                        frame.fill(T::from_sample(0.0));
                        continue;
                    };
                    let mut mixed = mixer.tick(sample, taper);
                    if channels != 2 {
                        mixed = [(mixed[0] + mixed[1]) * 0.5; 2];
                    }
                    if mixed
                        .iter()
                        .any(|value| !value.is_finite() || value.abs() > 1.000_001)
                    {
                        gate.close_attempt();
                        frame.fill(T::from_sample(0.0));
                        continue;
                    }
                    // Acquire fixed storage before committing output. On backpressure
                    // keep ownership locally; no box/buffer is destroyed in processing.
                    if pending.is_none() {
                        pending = recycled.try_recv().ok();
                    }
                    let Some(record) = pending.as_mut() else {
                        gate.close_attempt();
                        frame.fill(T::from_sample(0.0));
                        continue;
                    };
                    if !gate.current(epoch) {
                        frame.fill(T::from_sample(0.0));
                        continue;
                    }
                    let submitted = [
                        T::from_sample(mixed[0].clamp(-1.0, 1.0)),
                        T::from_sample(mixed[1].clamp(-1.0, 1.0)),
                    ];
                    for (channel, destination) in frame.iter_mut().enumerate() {
                        *destination = submitted[usize::from(channels == 2 && channel == 1)];
                    }
                    if record.valid_samples == 0 {
                        reference_sequence = reference_sequence.saturating_add(1);
                        record.sequence = reference_sequence;
                        record.played_at = scheduled.and_then(|start| {
                            start.checked_add(Duration::from_secs_f64(
                                offset as f64 / f64::from(output_rate),
                            ))
                        });
                        record.epoch = epoch;
                        record.utterance = binding.utterance;
                        record.submitted = Instant::now();
                        record.speech = speech;
                        record.final_submitted = false;
                        record.mix_final_submitted = false;
                        record.drain_until = None;
                        record.component_peaks = [0.0; 3];
                        record.device_time = info.timestamp().playback.add(
                            Duration::from_secs_f64(offset as f64 / f64::from(output_rate)),
                        );
                    }
                    if record.utterance != binding.utterance
                        || record.epoch != binding.epoch
                        || record.speech != speech
                    {
                        gate.close_attempt();
                        continue;
                    }
                    record.samples[record.valid_samples] = [
                        f32::from_sample(submitted[0]),
                        f32::from_sample(submitted[1]),
                    ];
                    record.valid_samples += 1;
                    for (peak, level) in record.component_peaks.iter_mut().zip(mixer.levels()) {
                        *peak = peak.max(level);
                    }
                    record.final_submitted = speech_last;
                    record.mix_final_submitted = mix_last;
                    if mix_last {
                        let timestamp = info.timestamp();
                        let delay = timestamp
                            .playback
                            .duration_since(&timestamp.callback)
                            .and_then(|delay| {
                                delay.checked_add(Duration::from_secs_f64(
                                    (offset + 1) as f64 / f64::from(output_rate),
                                ))
                            })
                            .filter(|delay| *delay <= Duration::from_secs(1))
                            .unwrap_or(Duration::from_secs(1));
                        let drained = Instant::now() + delay + Duration::from_millis(50);
                        if drained > binding.deadline {
                            gate.close_attempt();
                        }
                        record.drain_until = Some(drained);
                        tail = None;
                        mixer.reset();
                    }
                    if (record.valid_samples == reference_size || speech_last || mix_last)
                        && let Some(record) = pending.take()
                        && let Err(error) = reference.try_send(record)
                    {
                        pending = Some(match error {
                            std::sync::mpsc::TrySendError::Full(record)
                            | std::sync::mpsc::TrySendError::Disconnected(record) => record,
                        });
                        gate.close_attempt();
                    }
                }
                if diagnostic_silent {
                    if let Some(recording) = recording.as_mut() {
                        recording.capture(output, channels);
                    }
                    // This stays enabled for the entire process, even after recording
                    // has ended or failed. Never unexpectedly resume audible output.
                    output.fill(T::from_sample(0.0));
                }
            },
            move |_| error_gate.close_attempt(),
            None,
        )
        .map_err(|_| ErrorCode::Unavailable)
}
