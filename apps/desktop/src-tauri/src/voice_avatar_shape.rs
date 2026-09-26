//! Fixed v1 lossy visual projection, not a speaker identity or probability.
use ring::{hkdf, hmac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Parameters {
    pub version: u16,
    pub shape: [u8; 24],
    pub petals: [Option<super::portrait_dsp::Petal>; 20],
    pub ring: String,
    pub rotation: u8,
    pub tempo: Option<u8>,
    pub digest: String,
}
impl Parameters {
    fn digest(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(if self.version == 1 {
            b"avesra/avatar/render/v1\0"
        } else {
            b"avesra/avatar/render/v2\0"
        });
        digest.update(self.version.to_be_bytes());
        digest.update(self.shape);
        for petal in &self.petals {
            digest.update([u8::from(petal.is_some())]);
            if self.version == 1 {
                digest.update([0; 6]);
            } else if let Some(petal) = petal {
                digest.update([petal.length, petal.curve, petal.width, petal.density]);
                digest.update(petal.edge);
                digest.update(petal.energy);
            } else {
                digest.update([0; 16]);
            }
        }
        digest.update(self.ring.as_bytes());
        digest.update([
            self.rotation,
            u8::from(self.tempo.is_some()),
            self.tempo.unwrap_or(0),
        ]);
        hex::encode(digest.finalize())
    }
    pub(super) fn validate(&self, ring: &str, rotation: u8) -> Result<(), String> {
        if !matches!(self.version, 1 | 2)
            || self.shape.iter().any(|v| *v > 63)
            || (self.version == 1 && self.petals.iter().any(Option::is_some))
            || self.petals.iter().flatten().any(|v| !v.valid())
            || self.tempo.is_some()
            || self.ring != ring
            || self.rotation != rotation
            || self.digest != self.digest()
        {
            return Err("Avatar parameters are invalid or unsupported".into());
        }
        Ok(())
    }
    pub(super) fn portrait(
        &mut self,
        features: &[Option<super::portrait_dsp::Feature>],
    ) -> Result<(), String> {
        self.petals = super::portrait_dsp::render(features)?;
        self.version = 2;
        self.digest = self.digest();
        Ok(())
    }
}
struct Size;
impl hkdf::KeyType for Size {
    fn len(&self) -> usize {
        32
    }
}
fn subkey(key: &[u8; 32], label: &'static [u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, b"avesra/avatar/v1").extract(key);
    let info = [label];
    let okm = prk
        .expand(&info, Size)
        .map_err(|_| "Avatar key derivation failed")?;
    let mut bytes = Zeroizing::new([0; 32]);
    okm.fill(bytes.as_mut())
        .map_err(|_| "Avatar key derivation failed")?;
    Ok(bytes)
}
pub(super) fn signature(key: &[u8; 32], actor: Uuid, counter: u32) -> Result<(String, u8), String> {
    let bytes = subkey(key, b"avatar/v1/ring")?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, bytes.as_ref());
    let mut message = [0; 20];
    message[..16].copy_from_slice(actor.as_bytes());
    message[16..].copy_from_slice(&counter.to_be_bytes());
    let tag = hmac::sign(&key, &message);
    Ok((hex::encode(&tag.as_ref()[..8]), tag.as_ref()[8]))
}
fn quantize(value: f64) -> u8 {
    const CDF: [u32; 13] = [
        88, 407, 1491, 4378, 10398, 20218, 32768, 45317, 55137, 61157, 64044, 65128, 65447,
    ];
    let position = ((value.clamp(-3.0, 3.0) + 3.0) * 2.0).clamp(0.0, 12.0);
    let index = (position.floor() as usize).min(11);
    let fraction = position - index as f64;
    let cdf = f64::from(CDF[index]) + fraction * f64::from(CDF[index + 1] - CDF[index]);
    ((cdf * 64.0 / 65536.0).floor() as u8).min(63)
}
pub(super) fn build(
    key: &[u8; 32],
    input: &[f32],
    ring: String,
    rotation: u8,
) -> Result<Parameters, String> {
    if input.len() != 192 || input.iter().any(|v| !v.is_finite()) {
        return Err("Avatar needs a real valid speaker representation".into());
    }
    let norm = input
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm < 1e-12 {
        return Err("Avatar speaker representation is degenerate".into());
    }
    let secret = subkey(key, b"avatar/v1/shape")?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_ref());
    let mut rows: Vec<Zeroizing<Vec<f64>>> = Vec::with_capacity(24);
    let mut shape = [0; 24];
    for row in 0..24u32 {
        let mut values = Zeroizing::new(Vec::with_capacity(192));
        for dimension in 0..192u32 {
            let mut message = [0; 8];
            message[..4].copy_from_slice(&row.to_be_bytes());
            message[4..].copy_from_slice(&dimension.to_be_bytes());
            let tag = hmac::sign(&key, &message);
            values.push(if tag.as_ref()[0] & 0x80 == 0 {
                -1.0
            } else {
                1.0
            });
        }
        for previous in &rows {
            let projection = values
                .iter()
                .zip(previous.iter())
                .map(|(a, b)| a * b)
                .sum::<f64>();
            for (value, basis) in values.iter_mut().zip(previous.iter()) {
                *value -= projection * basis;
            }
        }
        let length = values.iter().map(|v| v * v).sum::<f64>().sqrt();
        if !length.is_finite() || length < 1e-12 {
            return Err("Avatar projection is degenerate".into());
        }
        for value in values.iter_mut() {
            *value /= length;
        }
        let projected = values
            .iter()
            .zip(input)
            .map(|(a, b)| a * f64::from(*b) / norm)
            .sum::<f64>()
            * 192f64.sqrt();
        if !projected.is_finite() {
            return Err("Avatar projection is invalid".into());
        }
        shape[row as usize] = quantize(projected);
        rows.push(values);
    }
    let mut result = Parameters {
        version: 1,
        shape,
        petals: std::array::from_fn(|_| None),
        ring,
        rotation,
        tempo: None,
        digest: String::new(),
    };
    result.digest = result.digest();
    Ok(result)
}
