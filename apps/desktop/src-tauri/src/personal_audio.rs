//! Provisional Personal-only postmix echo handling, never NoOutput evidence.
use avesra_windows::audio::{AudioFrame, PlaybackReference};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

const RATE: usize = 16_000;
const SAMPLES: usize = 320;
const LAG: usize = 3_200;
const WINDOW: usize = LAG + SAMPLES;
const TAIL: Duration = Duration::from_millis(200);
#[derive(Clone)]
struct Block {
    epoch: u64,
    start: Instant,
    end: Instant,
    samples: Vec<[f32; 2]>,
}
#[derive(Clone)]
struct Output {
    epoch: u64,
    id: Uuid,
    opened: Instant,
    closed: Option<Instant>,
    first: Option<Instant>,
    last: Option<Instant>,
    sequence: u64,
    rate: u32,
    complete: bool,
    failed: bool,
}
#[derive(Clone, Default)]
struct History {
    blocks: VecDeque<Block>,
    outputs: VecDeque<Output>,
    lost_until: Option<Instant>,
}
#[derive(Default)]
pub struct Reference(Mutex<History>);
impl Reference {
    pub fn open(&self, epoch: u64, id: Uuid) {
        let Ok(mut history) = self.0.lock() else {
            return;
        };
        let now = Instant::now();
        history.outputs.retain(|v| {
            v.closed
                .is_none_or(|t| now.saturating_duration_since(t) < Duration::from_secs(2))
        });
        if history.outputs.len() >= 64 {
            history.outputs.pop_front();
            history.lost_until = Some(now + Duration::from_secs(1));
        }
        history.outputs.push_back(Output {
            epoch,
            id,
            opened: now,
            closed: None,
            first: None,
            last: None,
            sequence: 0,
            rate: 0,
            complete: false,
            failed: false,
        });
    }
    pub fn retire(&self, epoch: u64) {
        if let Ok(mut history) = self.0.lock()
            && let Some(output) = history.outputs.iter_mut().find(|v| v.epoch == epoch)
        {
            // A cancelled callback can hold an unreported partial reference whose
            // samples are already scheduled. Keep that device horizon unknown.
            let now = Instant::now();
            output.closed = Some(if output.failed || !output.complete {
                now + Duration::from_millis(500)
            } else {
                now
            });
        }
    }
    pub fn push(&self, reference: &PlaybackReference) {
        let Ok(mut history) = self.0.lock() else {
            return;
        };
        let Some(output) = history
            .outputs
            .iter_mut()
            .find(|v| v.epoch == reference.epoch && v.id == reference.utterance)
        else {
            return;
        };
        if output.failed {
            return;
        }
        let Some(mut start) = reference.played_at else {
            output.failed = true;
            return;
        };
        if reference.valid_samples == 0
            || reference.valid_samples > reference.samples.len()
            || !(8000..=192000).contains(&reference.sample_rate)
            || reference.sequence != output.sequence.saturating_add(1)
            || reference.sequence == u64::MAX
            || (output.rate != 0 && output.rate != reference.sample_rate)
            || reference.submitted.elapsed() >= Duration::from_millis(250)
            || reference.samples[..reference.valid_samples]
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1.0)
        {
            output.failed = true;
            return;
        }
        if let Some(expected) = output.last {
            let distance = start
                .checked_duration_since(expected)
                .or_else(|| expected.checked_duration_since(start))
                .unwrap_or_default();
            if distance > Duration::from_millis(2) {
                output.failed = true;
                return;
            }
            // Preserve every actual sample; absorb only bounded callback clock jitter.
            start = expected;
        }
        let duration = Duration::from_secs_f64(
            reference.valid_samples as f64 / f64::from(reference.sample_rate),
        );
        let Some(end) = start.checked_add(duration) else {
            output.failed = true;
            return;
        };
        output.first.get_or_insert(start);
        output.last = Some(end);
        output.sequence = reference.sequence;
        output.rate = reference.sample_rate;
        output.complete |= reference.mix_final_submitted;
        // Area average the true stereo samples onto the native capture rate.
        // This bounded box filter is provisional, not a qualified resampler/AEC.
        let ratio = f64::from(reference.sample_rate) / RATE as f64;
        let count = (reference.valid_samples as f64 / ratio).ceil() as usize;
        let mut samples = Vec::with_capacity(count);
        for i in 0..count {
            let left = i as f64 * ratio;
            let right = ((i + 1) as f64 * ratio).min(reference.valid_samples as f64);
            let mut value = [0.0; 2];
            for j in left.floor() as usize..right.ceil() as usize {
                let weight = ((j + 1) as f64).min(right) - (j as f64).max(left);
                for (channel, v) in value.iter_mut().enumerate() {
                    *v += reference.samples[j][channel] * (weight / (right - left)) as f32;
                }
            }
            samples.push(value);
        }
        history.blocks.push_back(Block {
            epoch: reference.epoch,
            start,
            end,
            samples,
        });
        let now = Instant::now();
        while history
            .blocks
            .front()
            .is_some_and(|v| now.saturating_duration_since(v.end) > Duration::from_secs(1))
            || history.blocks.len() > 128
            || history
                .blocks
                .iter()
                .map(|v| v.samples.len())
                .sum::<usize>()
                > RATE
        {
            history.blocks.pop_front();
        }
    }
}
/// Constructed only through current native media configuration.
pub struct Input {
    reference: Arc<Reference>,
    epoch: u64,
    sequence: Option<u64>,
    previous: Option<Instant>,
    gain: Option<[f32; 2]>,
    delay: usize,
    path: Option<u64>,
}
/// Actual samples and provisional native metadata, not a transferable grant.
pub struct Frame {
    pub samples: [i16; SAMPLES],
    pub output_overlap: bool,
    pub echo_dominated: bool,
    pub reference_known: bool,
    pub residual_rms: f32,
}
impl Input {
    pub(super) fn new(reference: Arc<Reference>, epoch: u64) -> Self {
        Self {
            reference,
            epoch,
            sequence: None,
            previous: None,
            gain: None,
            delay: 0,
            path: None,
        }
    }
    pub fn filter(&mut self, frame: &AudioFrame) -> Result<Frame, String> {
        if frame.epoch != self.epoch
            || frame.captured > Instant::now()
            || frame.captured.elapsed() > Duration::from_millis(500)
            || self
                .sequence
                .is_some_and(|v| frame.sequence != v.saturating_add(1))
            || self.previous.is_some_and(|v| {
                frame.captured <= v || frame.captured.duration_since(v) > Duration::from_millis(25)
            })
        {
            return Err("Personal capture sequence or clock changed".into());
        }
        self.sequence = Some(frame.sequence);
        self.previous = Some(frame.captured);
        let mut result = Frame {
            samples: frame.samples,
            output_overlap: false,
            echo_dominated: false,
            reference_known: false,
            residual_rms: frame.rms,
        };
        let history = {
            let mut history = self
                .reference
                .0
                .lock()
                .map_err(|_| "Personal reference unavailable")?;
            let now = Instant::now();
            history
                .blocks
                .retain(|v| now.saturating_duration_since(v.end) <= Duration::from_secs(1));
            history.clone()
        };
        let end = frame.captured + Duration::from_millis(20);
        if history
            .lost_until
            .is_some_and(|until| frame.captured <= until)
        {
            result.output_overlap = true;
            self.gain = None;
            return Ok(result);
        }
        result.output_overlap = history.outputs.iter().any(|v| {
            v.opened < end
                && v.closed
                    .is_none_or(|t| t.max(v.last.unwrap_or(t)) + TAIL > frame.captured)
        });
        if !result.output_overlap {
            result.reference_known = true;
            self.gain = None;
            self.path = None;
            return Ok(result);
        }
        let path = history.outputs.back().map(|v| v.epoch);
        if self.path != path {
            self.path = path;
            self.gain = None;
            self.delay = 0;
        }
        let Some(origin) = frame.captured.checked_sub(TAIL) else {
            return Ok(result);
        };
        let mut reference = [[0.0f32; 2]; WINDOW];
        let mut known = [false; WINDOW];
        for (i, value) in reference.iter_mut().enumerate() {
            let at = origin + Duration::from_secs_f64(i as f64 / RATE as f64);
            // Outside actual output ownership there is no Avesra reference.
            let mut outputs = history.outputs.iter().filter(|v| {
                v.opened <= at && v.closed.is_none_or(|t| t.max(v.last.unwrap_or(t)) > at)
            });
            let Some(output) = outputs.next() else {
                known[i] = true;
                continue;
            };
            if output.failed || outputs.next().is_some() {
                // Cancellation may leave old DAC-buffered samples beside a new owner.
                // A single fitted path cannot claim this overlap is fully described.
                continue;
            }
            if let Some(block) = history
                .blocks
                .iter()
                .find(|v| v.epoch == output.epoch && v.start <= at && v.end > at)
            {
                let index = (at.duration_since(block.start).as_secs_f64() * RATE as f64) as usize;
                if let Some(sample) = block.samples.get(index) {
                    *value = *sample;
                    known[i] = true;
                }
            } else if output.first.is_some_and(|first| at < first)
                || (output.complete && output.last.is_some_and(|last| at >= last))
            {
                known[i] = true;
            }
        }
        // Every possible acoustic delay must be covered; gaps are never zeros.
        if known.iter().any(|v| !v) {
            self.gain = None;
            return Ok(result);
        }
        result.reference_known = true;
        let mic: [f32; SAMPLES] = std::array::from_fn(|i| f32::from(frame.samples[i]) / 32768.0);
        let energy = mic.iter().map(|v| v * v).sum::<f32>();
        let mut best = (0.0f32, self.delay, [0.0f32; 2]);
        let fit = |delay: usize| {
            let values = &reference[LAG - delay..LAG - delay + SAMPLES];
            let mut xx = [0.0f32; 3];
            let mut xy = [0.0f32; 2];
            for (x, y) in values.iter().zip(mic) {
                xx[0] += x[0] * x[0];
                xx[1] += x[0] * x[1];
                xx[2] += x[1] * x[1];
                xy[0] += x[0] * y;
                xy[1] += x[1] * y;
            }
            let ridge = (xx[0] + xx[2]) * 0.001 + 1e-6;
            let a = xx[0] + ridge;
            let d = xx[2] + ridge;
            let det = a * d - xx[1] * xx[1];
            let gain = [
                ((d * xy[0] - xx[1] * xy[1]) / det).clamp(-4.0, 4.0),
                ((a * xy[1] - xx[1] * xy[0]) / det).clamp(-4.0, 4.0),
            ];
            let explained = (gain[0] * xy[0] + gain[1] * xy[1]).max(0.0) / energy.max(1e-6);
            (explained, delay, gain)
        };
        for delay in (0..=LAG).step_by(16) {
            let candidate = fit(delay);
            if candidate.0 > best.0 {
                best = candidate;
            }
        }
        // Refine at native-sample resolution rather than quantizing acoustic phase.
        for delay in best.1.saturating_sub(16)..=(best.1 + 16).min(LAG) {
            let candidate = fit(delay);
            if candidate.0 > best.0 {
                best = candidate;
            }
        }
        // Adapt only on strong far-end correlation. Freeze during double talk.
        if best.0 >= 0.75 {
            self.delay = best.1;
            self.gain = Some(best.2);
        }
        let Some(gain) = self.gain else {
            // Near-end speech can exist without correlated echo (e.g. headphones).
            result.echo_dominated = false;
            return Ok(result);
        };
        let values = &reference[LAG - self.delay..LAG - self.delay + SAMPLES];
        let mut residual_energy = 0.0;
        for (i, x) in values.iter().enumerate() {
            let residual = (mic[i] - gain[0] * x[0] - gain[1] * x[1]).clamp(-1.0, 1.0);
            residual_energy += residual * residual;
            result.samples[i] = (residual * 32767.0).round() as i16;
        }
        result.residual_rms = (residual_energy / SAMPLES as f32).sqrt();
        result.echo_dominated = best.0 >= 0.75 && residual_energy <= energy * 0.25;
        Ok(result)
    }
}
