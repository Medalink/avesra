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
    ) -> Result<Option<(f32, Binding, bool)>, ErrorCode> {
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
            // Fixed storage, longer than the maximum 30s packet deadline. Keep
            // tombstones through filter/utterance reset; capacity fails closed.
            *entry = Some((first.utterance, now + Duration::from_secs(31)));
            let binding = Binding {
                epoch,
                utterance: first.utterance,
                deadline: first.deadline,
            };
            self.binding = Some(binding);
            self.admit(first, binding)?;
        }
        let binding = self.binding.ok_or(ErrorCode::Malformed)?;
        if binding.epoch != epoch || Instant::now() >= binding.deadline {
            return Err(ErrorCode::Expired);
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
        Ok(Some((value, binding, last)))
    }
}

pub(crate) fn output_stream<T: Sample + SizedSample + FromSample<f32>>(
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
    let channels = usize::from(config.channels);
    let output_rate = config.sample_rate;
    let reference_size = output_rate as usize / 50;
    let mut renderer = Renderer::new(rate, output_rate)?;
    let mut pending: Option<PlaybackReference> = None;
    let error_gate = gate.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info: &cpal::OutputCallbackInfo| {
                let epoch = gate.epoch();
                let mut invalidated = !gate.current(epoch);
                if invalidated {
                    renderer.clear();
                    pending = None;
                    for _ in 0..MAX_AUDIO_QUEUE {
                        if receiver.try_recv().is_err() {
                            break;
                        }
                    }
                }
                for (offset, frame) in output.chunks_exact_mut(channels).enumerate() {
                    if !gate.current(epoch) && !invalidated {
                        renderer.clear();
                        pending = None;
                        invalidated = true;
                        for _ in 0..MAX_AUDIO_QUEUE {
                            if receiver.try_recv().is_err() {
                                break;
                            }
                        }
                    }
                    let value = if gate.current(epoch) {
                        renderer.next(&receiver, epoch)
                    } else {
                        Ok(None)
                    };
                    let (sample, binding) = match value {
                        Ok(Some((value, binding, last))) if gate.current(epoch) => {
                            (value, Some((binding, last)))
                        }
                        Ok(_) => (0.0, None),
                        Err(_) => {
                            gate.close_attempt();
                            renderer.clear();
                            pending = None;
                            (0.0, None)
                        }
                    };
                    let submitted = T::from_sample(sample);
                    for channel in frame {
                        *channel = submitted;
                    }
                    if let Some((binding, last)) = binding {
                        let record = pending.get_or_insert_with(|| PlaybackReference {
                            epoch,
                            utterance: binding.utterance,
                            submitted: Instant::now(),
                            device_time: info.timestamp().playback.add(Duration::from_secs_f64(
                                offset as f64 / f64::from(output_rate),
                            )),
                            sample_rate: output_rate,
                            samples: [0.0; 3840],
                            valid_samples: 0,
                        });
                        if record.utterance != binding.utterance || record.epoch != binding.epoch {
                            gate.close_attempt();
                            pending = None;
                            continue;
                        }
                        record.samples[record.valid_samples] = f32::from_sample(submitted);
                        record.valid_samples += 1;
                        if (record.valid_samples == reference_size || last)
                            && let Some(record) = pending.take()
                            && reference.try_send(record).is_err()
                        {
                            gate.close_attempt();
                        }
                    } else {
                        pending = None;
                    }
                }
            },
            move |_| error_gate.close_attempt(),
            None,
        )
        .map_err(|_| ErrorCode::Unavailable)
}
