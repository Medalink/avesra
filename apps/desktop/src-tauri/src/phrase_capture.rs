//! One bounded setup capture path for enrollment and saved-voice checks.
use crate::Runtime;
use std::time::{Duration, Instant};
use tauri::Manager;

pub struct Phrase {
    pub pcm: Vec<u8>,
    pub clipped_samples: u32,
}
pub async fn collect(
    app: &tauri::AppHandle,
    epoch: u64,
    current: impl Fn() -> Result<(), String>,
    progress: impl Fn(u32),
) -> Result<Phrase, String> {
    collect_observed(app, epoch, current, progress, |_, _, _| Ok(())).await
}
pub async fn collect_observed(
    app: &tauri::AppHandle,
    epoch: u64,
    current: impl Fn() -> Result<(), String>,
    progress: impl Fn(u32),
    mut observe: impl FnMut(&[u8], Instant, bool) -> Result<(), String>,
) -> Result<Phrase, String> {
    let started = Instant::now();
    let state = app.state::<Runtime>();
    let mut sequence = 0;
    let mut pcm = Vec::with_capacity(256_000);
    let mut clipped_samples = 0;
    let mut chunk_captured = started;
    while pcm.len() < 256_000 {
        current()?;
        if started.elapsed() >= Duration::from_secs(12) {
            return Err("Microphone recording timed out. Retry this phrase.".into());
        }
        {
            let local = state.local.lock().map_err(|_| "Local state unavailable")?;
            if local.capture_epoch != epoch || !local.enrollment_capture || !local.capture_allowed()
            {
                return Err(local.capture_error.clone().unwrap_or_else(|| {
                    "Recording cancelled because listening controls changed".into()
                }));
            }
        }
        if let Some(frame) = state.media.take_capture_frame(epoch)? {
            if frame.sequence != sequence + 1 || frame.samples.len() != 320 {
                return Err("Microphone audio lost frames. Retry this phrase.".into());
            }
            sequence = frame.sequence;
            if sequence % 10 == 1 {
                chunk_captured = frame.captured;
            }
            for sample in frame.samples {
                if sample.unsigned_abs() >= 32760 {
                    clipped_samples += 1;
                }
                pcm.extend_from_slice(&sample.to_le_bytes());
            }
            if sequence % 50 == 0 {
                progress((sequence / 50) as u32);
            }
            if sequence % 10 == 0 {
                observe(
                    &pcm[pcm.len() - 6400..],
                    chunk_captured,
                    pcm.len() == 256_000,
                )?;
            }
        } else {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    current()?;
    Ok(Phrase {
        pcm,
        clipped_samples,
    })
}
