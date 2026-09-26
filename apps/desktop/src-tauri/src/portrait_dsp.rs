//! Local prompt-slot acoustic features. This does not recognize words/speakers.
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use zeroize::{Zeroize, Zeroizing};

pub(crate) const WORDS: [&str; 20] = [
    "sweet", "river", "jazz", "father", "thought", "ocean", "loop", "measure", "church", "morning",
    "pepper", "garden", "yellow", "whistle", "bright", "shadow", "violin", "cushion", "zebra",
    "kind",
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Feature {
    pub duration_ms: u16,
    pub pitch_median: f64,
    pub pitch_range: f64,
    pub pitch_slope: f64,
    pub mel: [f64; 16],
    pub centroid: f64,
    pub hnr: f64,
    pub energy: [f64; 4],
}
impl Drop for Feature {
    fn drop(&mut self) {
        self.duration_ms.zeroize();
        self.pitch_median.zeroize();
        self.pitch_range.zeroize();
        self.pitch_slope.zeroize();
        self.mel.zeroize();
        self.centroid.zeroize();
        self.hnr.zeroize();
        self.energy.zeroize();
    }
}
impl Feature {
    pub(crate) fn valid(&self) -> bool {
        (80..=720).contains(&self.duration_ms)
            && self.pitch_median.is_finite()
            && (60.0..=500.0).contains(&self.pitch_median)
            && self.pitch_range.is_finite()
            && (0.0..=48.0).contains(&self.pitch_range)
            && self.pitch_slope.is_finite()
            && (-48.0..=48.0).contains(&self.pitch_slope)
            && self.centroid.is_finite()
            && (0.0..=8000.0).contains(&self.centroid)
            && self.hnr.is_finite()
            && (-20.0..=60.0).contains(&self.hnr)
            && self
                .mel
                .iter()
                .all(|v| v.is_finite() && (-40.0..=20.0).contains(v))
            && self
                .energy
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Petal {
    pub length: u8,
    pub curve: u8,
    pub width: u8,
    pub density: u8,
    pub edge: [u8; 8],
    pub energy: [u8; 4],
}
impl Petal {
    pub(crate) fn valid(&self) -> bool {
        [self.length, self.curve, self.width, self.density]
            .into_iter()
            .chain(self.edge)
            .chain(self.energy)
            .all(|v| v <= 63)
    }
}
fn quantized(value: f64, low: f64, high: f64) -> u8 {
    (((value - low) / (high - low)).clamp(0.0, 1.0) * 63.0).round() as u8
}
pub(crate) fn render(features: &[Option<Feature>]) -> Result<[Option<Petal>; 20], String> {
    if features.len() != 20 || features.iter().flatten().any(|v| !v.valid()) {
        return Err("Invalid portrait feature record".into());
    }
    let mut bands = Zeroizing::new([0.0; 16]);
    let mut count = 0;
    for value in features.iter().flatten() {
        for (sum, value) in bands.iter_mut().zip(value.mel.iter()) {
            *sum += value;
        }
        count += 1;
    }
    if count > 0 {
        for value in bands.iter_mut() {
            *value /= f64::from(count);
        }
    }
    Ok(std::array::from_fn(|index| {
        features[index].as_ref().map(|value| {
            let peak = value.energy.iter().copied().fold(0.0, f64::max).max(1e-12);
            Petal {
                length: quantized(f64::from(value.duration_ms), 80.0, 720.0),
                curve: quantized(value.pitch_slope, -12.0, 12.0),
                width: quantized(value.centroid, 0.0, 8000.0),
                density: quantized(value.hnr, -20.0, 60.0),
                edge: std::array::from_fn(|band| {
                    quantized(
                        (value.mel[band * 2] + value.mel[band * 2 + 1]
                            - bands[band * 2]
                            - bands[band * 2 + 1])
                            / 2.0,
                        -4.0,
                        4.0,
                    )
                }),
                energy: std::array::from_fn(|i| quantized(value.energy[i] / peak, 0.0, 1.0)),
            }
        })
    }))
}
fn median(values: &[f64]) -> f64 {
    let mut sorted = Zeroizing::new(values.to_vec());
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}
fn pitch(samples: &[f64]) -> Option<(f64, f64)> {
    // 640 actual samples; lag32..266 = 60.15..500Hz. No zero-padding.
    let mut difference = Zeroizing::new([0.0; 267]);
    for lag in 1..267 {
        difference[lag] = (0..320)
            .map(|i| (samples[i] - samples[i + lag]).powi(2))
            .sum();
    }
    let mut cumulative = 0.0;
    for (lag, value) in difference.iter_mut().enumerate().skip(1) {
        cumulative += *value;
        *value = if cumulative > 1e-15 {
            *value * lag as f64 / cumulative
        } else {
            1.0
        };
    }
    let mut lag = 32;
    while lag < 266 {
        if difference[lag] < 0.15 {
            while lag < 266 && difference[lag + 1] < difference[lag] {
                lag += 1;
            }
            let numerator = (0..320).map(|i| samples[i] * samples[i + lag]).sum::<f64>();
            let left = (0..320).map(|i| samples[i].powi(2)).sum::<f64>();
            let right = (0..320).map(|i| samples[i + lag].powi(2)).sum::<f64>();
            let correlation = numerator / (left * right).sqrt().max(1e-15);
            if !correlation.is_finite() || correlation < 0.6 {
                return None;
            }
            let correlation = correlation.clamp(1e-6, 1.0 - 1e-6);
            return Some((
                16000.0 / lag as f64,
                (10.0 * (correlation / (1.0 - correlation)).log10()).clamp(-20.0, 60.0),
            ));
        }
        lag += 1;
    }
    None
}
fn spectrum(samples: &[f64]) -> Zeroizing<[f64; 257]> {
    let mut real = Zeroizing::new([0.0; 512]);
    let mut imaginary = Zeroizing::new([0.0; 512]);
    for i in 0..512 {
        real[i] = samples[i] * (0.5 - 0.5 * (2.0 * PI * i as f64 / 511.0).cos());
    }
    for i in 0..512usize {
        let reverse = i.reverse_bits() >> (usize::BITS - 9);
        if i < reverse {
            real.swap(i, reverse);
        }
    }
    let mut size = 2;
    while size <= 512 {
        for start in (0..512).step_by(size) {
            for k in 0..size / 2 {
                let angle = -2.0 * PI * k as f64 / size as f64;
                let (sine, cosine) = angle.sin_cos();
                let even = start + k;
                let odd = even + size / 2;
                let r = real[odd] * cosine - imaginary[odd] * sine;
                let i = real[odd] * sine + imaginary[odd] * cosine;
                real[odd] = real[even] - r;
                imaginary[odd] = imaginary[even] - i;
                real[even] += r;
                imaginary[even] += i;
            }
        }
        size *= 2;
    }
    Zeroizing::new(std::array::from_fn(|i| {
        (real[i] * real[i] + imaginary[i] * imaginary[i]) / (512.0 * 512.0)
    }))
}
/// Exactly one real800ms slot. Callback checks original ownership/deadline often.
pub(crate) fn extract(
    pcm: &[u8],
    current: &mut dyn FnMut() -> Result<(), String>,
) -> Result<Option<Feature>, String> {
    current()?;
    if pcm.len() != 25600 {
        return Err("Portrait audio slot is incomplete".into());
    }
    let samples = zeroize::Zeroizing::new(
        pcm.chunks_exact(2)
            .map(|b| f64::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
            .collect::<Vec<_>>(),
    );
    if samples
        .iter()
        .filter(|v| v.abs() >= 32760.0 / 32768.0)
        .count()
        > 128
    {
        return Ok(None);
    }
    let energies = Zeroizing::new(
        samples
            .chunks_exact(320)
            .map(|v| (v.iter().map(|x| x * x).sum::<f64>() / 320.0).sqrt())
            .collect::<Vec<f64>>(),
    );
    let mut sorted = Zeroizing::new(energies.to_vec());
    sorted.sort_by(f64::total_cmp);
    let threshold = (sorted[7] * 3.0).max(0.005);
    let active = Zeroizing::new(
        energies
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (*v > threshold).then_some(i))
            .collect::<Vec<usize>>(),
    );
    let (Some(&first), Some(&last)) = (active.first(), active.last()) else {
        return Ok(None);
    };
    if first < 2 || last >= 38 || active.len() < 4 || active.windows(2).any(|v| v[1] - v[0] > 2) {
        return Ok(None);
    }
    let island = &samples[first * 320..(last + 1) * 320];
    let mut pitches = Zeroizing::new(Vec::new());
    let mut harmonics = Zeroizing::new(Vec::new());
    let mut mel = Zeroizing::new([0.0; 16]);
    let mut centroid = 0.0;
    let mut frames = 0;
    let mel_low = 2595.0 * (1.0_f64 + 80.0 / 700.0).log10();
    let mel_high = 2595.0 * (1.0_f64 + 7600.0 / 700.0).log10();
    let edges: [f64; 18] = std::array::from_fn(|i| {
        700.0 * (10f64.powf((mel_low + (mel_high - mel_low) * i as f64 / 17.0) / 2595.0) - 1.0)
    });
    for start in (0..=island.len().saturating_sub(640)).step_by(160) {
        current()?;
        let window = &island[start..start + 640];
        if let Some((pitch, hnr)) = pitch(window) {
            pitches.push(pitch);
            harmonics.push(hnr);
        }
        let power = spectrum(&window[..512]);
        let total = power.iter().sum::<f64>();
        if total <= 1e-15 {
            continue;
        }
        centroid += power
            .iter()
            .enumerate()
            .map(|(i, v)| i as f64 * 16000.0 / 512.0 * v)
            .sum::<f64>()
            / total;
        for band in 0..16 {
            let sum = power
                .iter()
                .enumerate()
                .map(|(bin, value)| {
                    let hz = bin as f64 * 16000.0 / 512.0;
                    let weight = if hz < edges[band + 1] {
                        (hz - edges[band]) / (edges[band + 1] - edges[band])
                    } else {
                        (edges[band + 2] - hz) / (edges[band + 2] - edges[band + 1])
                    };
                    value * weight.clamp(0.0, 1.0)
                })
                .sum::<f64>();
            mel[band] += sum.max(1e-15).ln();
        }
        frames += 1;
    }
    if pitches.len() < 3 || frames < 3 {
        return Ok(None);
    }
    for value in mel.iter_mut() {
        *value /= f64::from(frames);
    }
    let middle = median(&pitches);
    let min = pitches.iter().copied().fold(f64::INFINITY, f64::min);
    let max = pitches.iter().copied().fold(0.0, f64::max);
    let feature = Feature {
        duration_ms: ((last - first + 1) * 20) as u16,
        pitch_median: middle,
        pitch_range: 12.0 * (max / min).log2(),
        pitch_slope: 12.0 * (pitches[pitches.len() - 1] / pitches[0]).log2(),
        mel: *mel,
        centroid: centroid / f64::from(frames),
        hnr: median(&harmonics),
        energy: std::array::from_fn(|i| {
            let part = &island[i * island.len() / 4..(i + 1) * island.len() / 4];
            (part.iter().map(|v| v * v).sum::<f64>() / part.len() as f64).sqrt()
        }),
    };
    current()?;
    Ok(feature.valid().then_some(feature))
}
