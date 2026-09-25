//! Prepared local sound design. No inference, allocation or locks in `tick`.
use crate::resampling::CaptureResampler;
use avesra_contracts::ErrorCode;
use avesra_core::sound::{SoundPreset, SoundSettings};
use fundsp::prelude::{AudioUnit, delay, highpass_hz, lowpass_hz, lowshelf_hz, reverb_stereo};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// One coherent latest-value mailbox. A bypass never waits behind slider edits.
pub struct SoundControl(AtomicU64, AtomicU32);
impl Default for SoundControl {
    fn default() -> Self {
        let value = Self(AtomicU64::new(0), AtomicU32::new(0));
        value.publish(&SoundSettings::default(), 80);
        value
    }
}
impl SoundControl {
    pub fn publish(&self, settings: &SoundSettings, volume: u8) {
        let a = settings.active();
        let mut bits = u64::from(settings.enabled)
            | (u64::from(a.effects) << 1)
            | (u64::from(a.background_enabled) << 2)
            | (u64::from(settings.preset == SoundPreset::Human) << 3);
        for (index, value) in [
            a.character,
            a.background,
            a.warmth,
            a.space,
            a.texture,
            volume,
        ]
        .iter()
        .enumerate()
        {
            let shift = [4, 11, 18, 25, 33, 40][index];
            let maximum = if index == 3 { 200 } else { 100 };
            bits |= u64::from((*value).min(maximum)) << shift;
        }
        bits |= (a.background_texture as u64) << 47;
        bits |= u64::from(a.presence.min(100)) << 49;
        bits |= u64::from(a.echo.min(100)) << 56;
        self.0.store(bits, Ordering::Release);
        self.1.store(
            u32::from(a.echo_delay_left_ms.min(400))
                | (u32::from(a.echo_delay_right_ms.min(400)) << 9)
                | (((i32::from(a.harmonizer_depth.clamp(-24, 24)) + 24) as u32) << 18),
            Ordering::Release,
        );
    }
    fn load(&self) -> [f32; 18] {
        let bits = self.0.load(Ordering::Acquire);
        let mut p = [0.0; 18];
        for (i, item) in p[..4].iter_mut().enumerate() {
            *item = ((bits >> i) & 1) as f32;
        }
        for (i, item) in p[4..10].iter_mut().enumerate() {
            let shift = [4, 11, 18, 25, 33, 40][i];
            let mask = if i == 3 { 255 } else { 127 };
            *item = ((bits >> shift) & mask) as f32 / 100.0;
        }
        p[10 + ((bits >> 47) & 3).min(2) as usize] = 1.0;
        p[13] = ((bits >> 49) & 127) as f32 / 100.0;
        p[14] = ((bits >> 56) & 127) as f32 / 100.0;
        let timing = self.1.load(Ordering::Acquire);
        p[15] = (timing & 511) as f32 / 1000.0;
        p[16] = ((timing >> 9) & 511) as f32 / 1000.0;
        p[17] = 2.0f32.powf((24.0 - ((timing >> 18) & 63) as f32) / 24.0);
        p
    }
}

/// Owner-supplied lossless recording, prepared off callback. At 192 kHz the
/// transient stereo buffer uses <30 MiB; the retained loop is 19 seconds.
fn atmosphere(rate: u32) -> Result<Box<[[f32; 2]]>, ErrorCode> {
    const AUDIO: &[u8; 48_000 * 20 * 4] = include_bytes!("../assets/galaxy-ambience.s16le");
    const SOURCE_FRAMES: usize = AUDIO.len() / 4;
    if !(8_000..=192_000).contains(&rate) {
        return Err(ErrorCode::Unsupported);
    }
    let sample = |frame: usize, channel: usize| {
        let offset = (frame % SOURCE_FRAMES) * 4 + channel * 2;
        f32::from(i16::from_le_bytes([AUDIO[offset], AUDIO[offset + 1]])) / 32768.0
    };
    let length = rate as usize * 19;
    let crossfade = rate as usize;
    let mut result = vec![[0.0; 2]; length + crossfade];
    for channel in 0..2 {
        if rate == 48_000 {
            for (frame, output) in result.iter_mut().enumerate() {
                output[channel] = sample(frame, channel);
            }
        } else {
            let mut converter = CaptureResampler::between(48_000, rate)?;
            let mut written = 0;
            // Enough cyclic input to flush the finite filter delay and final
            // block. No silence padding or rate-dependent pitch changes.
            for frame in 0..SOURCE_FRAMES + 8192 {
                if let Some(values) = converter.push(sample(frame, channel))? {
                    let count = values.len().min(result.len() - written);
                    for (output, value) in result[written..written + count].iter_mut().zip(values) {
                        output[channel] = *value;
                    }
                    written += count;
                    if written == result.len() {
                        break;
                    }
                }
            }
            if written != result.len() {
                return Err(ErrorCode::Unavailable);
            }
        }
    }
    // The end continues into the beginning through a one-second overlap.
    // Linear weighting avoids a gain bump for correlated ambient material.
    for i in 0..crossfade {
        let blend = i as f32 / crossfade as f32;
        let continuation = result[length + i];
        for (sample, continued) in result[i].iter_mut().zip(continuation) {
            *sample = continued * (1.0 - blend) + *sample * blend;
        }
    }
    result.truncate(length);
    Ok(result.into_boxed_slice())
}

/// A low parallel copy of the actual voice, with no synth carrier. Two
/// complementary windows hide delay-head wraps; dry speech is never delayed.
struct Harmonizer {
    buffer: Box<[f32]>,
    write: usize,
    phase: f32,
    minimum: f32,
    span: f32,
    filter: Box<dyn AudioUnit>,
}
impl Harmonizer {
    fn new(rate: u32) -> Self {
        let span = rate as f32 * 0.05;
        let cutoff = 3500.0f32.min(rate as f32 * 0.4);
        let mut filter: Box<dyn AudioUnit> = Box::new(
            (highpass_hz(55.0f32, 0.707) | highpass_hz(55.0f32, 0.707))
                >> (lowpass_hz(cutoff, 0.707) | lowpass_hz(cutoff, 0.707)),
        );
        filter.set_sample_rate(f64::from(rate));
        filter.allocate();
        Self {
            buffer: vec![0.0; (rate as f32 * 0.07).ceil() as usize + 2].into_boxed_slice(),
            write: 0,
            phase: 0.0,
            minimum: rate as f32 * 0.008,
            span,
            filter,
        }
    }
    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
        self.phase = 0.0;
        self.filter.reset();
    }
    fn read(&self, phase: f32) -> f32 {
        let position = (self.write as f32 - self.minimum - phase * self.span)
            .rem_euclid(self.buffer.len() as f32);
        let floor = position.floor();
        // Floating remainder can round a tiny negative offset up to len.
        let index = floor as usize % self.buffer.len();
        let fraction = position - floor;
        let first = self.buffer[index];
        first + (self.buffer[(index + 1) % self.buffer.len()] - first) * fraction
    }
    fn tick(&mut self, speech: f32, pitch_ratio: f32) -> [f32; 2] {
        self.buffer[self.write] = speech;
        let mut shifted = [0.0; 2];
        for (channel, sample) in shifted.iter_mut().enumerate() {
            let phase = (self.phase + channel as f32 * 0.15).fract();
            let other = (phase + 0.5).fract();
            let weight = 0.5 - 0.5 * (std::f32::consts::TAU * phase).cos();
            *sample = self.read(phase) * weight + self.read(other) * (1.0 - weight);
        }
        self.phase = (self.phase + (1.0 - pitch_ratio) / self.span).rem_euclid(1.0);
        self.write = (self.write + 1) % self.buffer.len();
        let mut output = [0.0; 2];
        self.filter.tick(&shifted, &mut output);
        output
    }
}

struct StereoEcho {
    buffer: Box<[[f32; 2]]>,
    write: usize,
    rate: f32,
}
impl StereoEcho {
    fn new(rate: u32) -> Self {
        Self {
            buffer: vec![[0.0; 2]; rate as usize * 2 + 2].into_boxed_slice(),
            write: 0,
            rate: rate as f32,
        }
    }
    fn reset(&mut self) {
        self.buffer.fill([0.0; 2]);
        self.write = 0;
    }
    fn tick(&mut self, input: f32, spacing: [f32; 2]) -> [f32; 2] {
        self.buffer[self.write] = [input; 2];
        let mut output = [0.0; 2];
        for channel in 0..2 {
            let interval = (if channel == 0 { 0.12 } else { 0.18 }) + spacing[channel];
            for (tap, gain) in [0.65, 0.35, 0.18].into_iter().enumerate() {
                let position = (self.write as f32 - interval * (tap + 1) as f32 * self.rate)
                    .rem_euclid(self.buffer.len() as f32);
                let floor = position.floor();
                let index = floor as usize % self.buffer.len();
                let fraction = position - floor;
                let first = self.buffer[index][channel];
                let next = self.buffer[(index + 1) % self.buffer.len()][channel];
                output[channel] += (first + (next - first) * fraction) * gain;
            }
        }
        self.write = (self.write + 1) % self.buffer.len();
        output
    }
}

pub(crate) struct Mixer {
    params: [f32; 18],
    target: [f32; 18],
    step: f32,
    rate: f32,
    shelf: Box<dyn AudioUnit>,
    harmonizer: Harmonizer,
    reverb: Box<dyn AudioUnit>,
    echo: StereoEcho,
    levels: [f32; 3],
    loop_audio: Box<[[f32; 2]]>,
    phase: usize,
    started: usize,
    envelope: f32,
    duck: f32,
    limiter: f32,
    random: u32,
    pink: [f32; 2],
    dirty_effects: bool,
}
impl Mixer {
    pub fn new(rate: u32, control: &SoundControl) -> Result<Self, ErrorCode> {
        let mut shelf: Box<dyn AudioUnit> = Box::new(lowshelf_hz(150.0f32, 0.707, 2.0));
        let mut reverb: Box<dyn AudioUnit> = Box::new(
            (delay(0.03) | delay(0.03))
                >> reverb_stereo(10.0, 2.6, 0.7)
                >> (highpass_hz(180.0f32, 0.707) | highpass_hz(180.0f32, 0.707)),
        );
        for node in [&mut shelf, &mut reverb] {
            node.set_sample_rate(f64::from(rate));
            node.allocate();
        }
        let params = control.load();
        Ok(Self {
            params,
            target: params,
            step: 1.0 / (rate as f32 * 0.025),
            rate: rate as f32,
            shelf,
            harmonizer: Harmonizer::new(rate),
            reverb,
            echo: StereoEcho::new(rate),
            levels: [0.0; 3],
            loop_audio: atmosphere(rate)?,
            phase: rate as usize * 2,
            started: 0,
            envelope: 0.0,
            duck: 1.0,
            limiter: 1.0,
            random: 0xbb67_ae85,
            pink: [0.0; 2],
            dirty_effects: false,
        })
    }
    pub fn sync(&mut self, control: &SoundControl) {
        self.target = control.load();
    }
    pub fn has_tail(&self) -> bool {
        self.params[0] > 0.0 && (self.params[1] > 0.0 || self.params[2] > 0.0)
    }
    pub fn reset(&mut self) {
        self.shelf.reset();
        self.harmonizer.reset();
        self.reverb.reset();
        self.echo.reset();
        self.levels = [0.0; 3];
        self.envelope = 0.0;
        self.duck = 1.0;
        self.limiter = 1.0;
        self.started = 0;
        self.pink = [0.0; 2];
        self.dirty_effects = false;
        // Phase is non-audio metadata; a new device/epoch constructs a new mixer.
    }
    pub fn tick(&mut self, input: f32, tail_gain: f32) -> [f32; 2] {
        for (current, target) in self.params.iter_mut().zip(self.target) {
            *current += (target - *current).clamp(-self.step, self.step);
        }
        let [
            enabled,
            effects,
            background,
            human,
            character,
            bed,
            warmth,
            space,
            texture,
            volume,
            pad,
            pink,
            white,
            presence,
            echo_amount,
            echo_delay_left,
            echo_delay_right,
            pitch_ratio,
        ] = self.params;
        if enabled == 0.0 {
            if self.dirty_effects {
                self.reset();
            }
            self.levels = [(input * volume).abs(), 0.0, 0.0];
            return [input * volume; 2];
        }
        self.dirty_effects = true;
        self.started = self.started.saturating_add(1);
        let attack = 1.0 / (self.rate * 0.005);
        let release = 1.0 / (self.rate * 0.15);
        self.envelope += (input.abs() - self.envelope)
            * if input.abs() > self.envelope {
                attack
            } else {
                release
            };
        let compression = if self.envelope > 0.22 {
            (0.22 / self.envelope).sqrt()
        } else {
            1.0
        };
        let mut low = [0.0];
        self.shelf.tick(&[input], &mut low);
        let warm = input + (low[0] - input) * warmth * (1.0 - human * 0.5);
        let core = warm * (1.0 + (compression - 1.0) * character);
        let doubled = self.harmonizer.tick(core, pitch_ratio);
        // Keep the clear core intact; character is an added parallel layer.
        let voice = core;
        let mut wet = [0.0; 2];
        self.reverb.tick(&[voice * effects; 2], &mut wet);
        let wet_amount = space * (0.75 * (1.0 - human) + 0.03 * human);
        let echo = self
            .echo
            .tick(voice * effects, [echo_delay_left, echo_delay_right]);
        let target_duck = if self.envelope > 0.025 { 0.55 } else { 1.0 };
        self.duck += (target_duck - self.duck)
            * if target_duck < self.duck {
                1.0 / (self.rate * 0.01)
            } else {
                1.0 / (self.rate * 0.25)
            };
        let fade = (self.started as f32 / (self.rate * 0.1)).min(1.0);
        let atmosphere = self.loop_audio[self.phase];
        self.phase = (self.phase + 1) % self.loop_audio.len();
        let mut mixed = [0.0; 2];
        self.levels = [(input * volume).abs(), 0.0, 0.0];
        for channel in 0..2 {
            self.random ^= self.random << 13;
            self.random ^= self.random >> 17;
            self.random ^= self.random << 5;
            let noise = self.random as f32 / u32::MAX as f32 * 2.0 - 1.0;
            self.pink[channel] += (noise - self.pink[channel]) * (900.0 / self.rate);
            let bed_sample =
                atmosphere[channel] * pad + self.pink[channel] * 0.8 * pink + noise * 0.15 * white;
            let processed = (voice + doubled[channel] * texture * (1.0 - human) * 1.2)
                * (1.0 + presence * (1.0 - human) * 0.6)
                + (wet[channel] * wet_amount + echo[channel] * echo_amount * (1.0 - human) * 0.65)
                    * tail_gain;
            let effect = (processed - input) * enabled * effects * volume;
            let bed_output = bed_sample
                * bed
                * (0.40 + pad * 0.60)
                * background
                * enabled
                * self.duck
                * fade
                * tail_gain
                * volume;
            mixed[channel] = input * volume + effect + bed_output;
            self.levels[1] = self.levels[1].max(effect.abs());
            self.levels[2] = self.levels[2].max(bed_output.abs());
        }
        // Linked zero-lookahead peak limiting. Instant gain reduction bounds each
        // sample; slow release avoids stereo-image movement. No latency is added.
        let peak = mixed[0].abs().max(mixed[1].abs());
        let ceiling = 1.0 - enabled * (1.0 - 0.891_250_9);
        let wanted = if peak > ceiling { ceiling / peak } else { 1.0 };
        if wanted < self.limiter {
            self.limiter = wanted;
        } else {
            self.limiter += (wanted - self.limiter) / (self.rate * 0.08);
        }
        for level in &mut self.levels {
            *level *= self.limiter;
        }
        [mixed[0] * self.limiter, mixed[1] * self.limiter]
    }
    /// Per-component peaks after master/limiter, before device format conversion.
    pub fn levels(&self) -> [f32; 3] {
        self.levels
    }
}
