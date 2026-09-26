//! One native capture owner across bounded activity streams and reply playback.
use super::*;
use crate::voice_check::activity_stream::{Captured, Stream};
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct Playback(Option<ReplyPlayback>);
struct ReplyPlayback {
    task: tauri::async_runtime::JoinHandle<()>,
    interrupted: std::sync::Arc<AtomicBool>,
}
impl Drop for Playback {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            value.task.abort();
        }
    }
}
impl Playback {
    pub(super) fn interrupt(&mut self, app: &tauri::AppHandle) {
        if let Some(value) = &self.0 {
            value.interrupted.store(true, Ordering::SeqCst);
        }
        let state = app.state::<Runtime>();
        if (self.0.is_some() || state.preview.try_lock().is_err())
            && let Ok(mut local) = state.local.lock()
        {
            local.playback_epoch = local.playback_epoch.saturating_add(1);
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
        }
    }
    pub(super) async fn retire(&mut self, app: &tauri::AppHandle) -> Result<(), String> {
        if let Some(value) = self.0.as_mut() {
            tokio::time::timeout(Duration::from_secs(2), &mut value.task)
                .await
                .map_err(|_| "Previous spoken reply is still retiring")?
                .map_err(|_| "Previous spoken reply owner stopped")?;
            self.0.take();
        }
        // The same native output slot also owns greetings, previews and chimes.
        // Their real coordinators, not the epoch bump, prove device retirement.
        let slot = app.state::<Runtime>().preview.clone();
        let owner = tokio::time::timeout(Duration::from_secs(2), slot.lock_owned())
            .await
            .map_err(|_| "Previous assistant output is still retiring")?;
        drop(owner);
        Ok(())
    }
    pub(super) fn start(
        &mut self,
        app: tauri::AppHandle,
        reply: avesra_windows::effects::PublishedReply,
        voice: avesra_contracts::voice::VoiceIdentity,
        response_timing: Option<avesra_core::trace::ResponseTiming>,
    ) {
        let interrupted = std::sync::Arc::new(AtomicBool::new(false));
        let cancelled = interrupted.clone();
        let task = tauri::async_runtime::spawn(async move {
            if let Err(error) =
                crate::speech::speak(app.clone(), reply, voice, response_timing).await
                && !cancelled.load(Ordering::SeqCst)
            {
                let _ = app.emit(
                    "runtime-error",
                    format!("Spoken reply ended: {error}. The accepted answer remains saved."),
                );
            }
        });
        self.0 = Some(ReplyPlayback { task, interrupted });
    }
}
pub(super) struct Evidence {
    pub activity: (Instant, Instant),
    pub known: bool,
    pub output: bool,
    pub near_end: bool,
    pub clipped: u32,
}
struct Mark {
    at: Instant,
    known: bool,
    output: bool,
    near_end: bool,
    clipped: u32,
}
struct CapturedOwner {
    owner: Owner,
    marks: VecDeque<Mark>,
}
struct Queued {
    value: Option<(Completed, Evidence)>,
    performance: std::sync::Arc<crate::performance::State>,
}
impl Drop for Queued {
    fn drop(&mut self) {
        if self.value.is_some() {
            self.performance
                .gate(crate::performance::GateReason::Abandoned);
        }
    }
}
impl CapturedOwner {
    fn evidence(&self, segment: &Completed, activity: (Instant, Instant)) -> Evidence {
        let marks: Vec<_> = self
            .marks
            .iter()
            .filter(|v| v.at >= segment.started() && v.at < segment.completed())
            .collect();
        let expected = (segment.samples() as usize).div_ceil(320);
        Evidence {
            activity,
            known: marks.len() >= expected && marks.iter().all(|v| v.known),
            output: marks.iter().any(|v| v.output),
            near_end: marks.iter().filter(|v| v.near_end).count() >= 4,
            clipped: marks.iter().map(|v| v.clipped).sum(),
        }
    }
}
pub(super) async fn run(
    app: &tauri::AppHandle,
    pairing: &PairingRecord,
    session: SessionIdentity,
    admission: &Admission,
    playback: &mut Playback,
) -> Result<(), String> {
    current(app, session, admission)?;
    let mut stream = Stream::open(pairing, session).await?;
    current(app, session, admission)?;
    let lease = Uuid::new_v4();
    let state = app.state::<Runtime>();
    let mut input = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let input = state.media.personal_input(&local)?;
        state.media.open_personal_stream(&local, lease)?;
        input
    };
    let _capture = Capture {
        app: app.clone(),
        epoch: session.epoch,
        lease,
    };
    let mut utterances = Owner::new(admission.context.clone(), admission.policy.clone())
        .map_err(|_| "Invalid continuous utterance owner")?;
    utterances
        .retain_minimum_samples(16000)
        .map_err(|_| "Invalid continuous analysis span")?;
    let owner = Mutex::new(CapturedOwner {
        owner: utterances,
        marks: VecDeque::new(),
    });
    let (audio_send, mut audio_receive) = tokio::sync::mpsc::channel::<Captured>(2);
    let (utterance_send, mut utterance_receive) = tokio::sync::mpsc::channel::<Queued>(1);
    let producer = async {
        let mut sequence = 0u64;
        let mut pcm = Vec::with_capacity(6400);
        let mut captured = Instant::now();
        let mut last = Instant::now();
        loop {
            current(app, session, admission)?;
            let Some(frame) = state.media.take_voice_frame(session.epoch, lease)? else {
                if last.elapsed() > Duration::from_millis(500) {
                    return Err::<(), String>("Continuous input lost freshness".into());
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
                continue;
            };
            if frame.epoch != session.epoch
                || frame.sequence
                    != sequence
                        .checked_add(1)
                        .ok_or("Capture sequence exhausted")?
            {
                return Err("Continuous input sequence changed".into());
            }
            let filtered = input.filter(&frame)?;
            {
                let mut held = owner.lock().map_err(|_| "Utterance owner unavailable")?;
                held.owner
                    .capture(frame.sequence, frame.captured, &filtered.samples)
                    .map_err(|_| "Continuous input clock changed")?;
                held.marks.push_back(Mark {
                    at: frame.captured,
                    known: filtered.reference_known,
                    output: filtered.output_overlap,
                    near_end: !filtered.echo_dominated && filtered.residual_rms > 0.002,
                    clipped: frame
                        .samples
                        .iter()
                        .filter(|v| v.unsigned_abs() >= 32760)
                        .count() as u32,
                });
                while held.marks.len() > 650 {
                    held.marks.pop_front();
                }
            }
            if pcm.is_empty() {
                captured = frame.captured;
            }
            pcm.extend(filtered.samples.iter().flat_map(|v| v.to_le_bytes()));
            sequence = frame.sequence;
            last = frame.captured;
            if sequence.is_multiple_of(10) {
                audio_send
                    .try_send(Captured {
                        pcm: std::mem::replace(&mut pcm, Vec::with_capacity(6400)),
                        captured,
                        final_chunk: sequence.is_multiple_of(400),
                    })
                    .map_err(|_| "Continuous activity processing fell behind input")?;
            }
        }
    };
    let detector = async {
        let mut base = 0u64;
        loop {
            let frame = tokio::time::timeout(Duration::from_millis(750), audio_receive.recv())
                .await
                .map_err(|_| "Continuous activity input stalled")?
                .ok_or("Continuous input stopped")?;
            let activity_started = Instant::now();
            let observation = stream
                .exchange(app, lease, frame, || current(app, session, admission))
                .await?;
            let activity_ended = Instant::now();
            let completed = {
                let mut held = owner.lock().map_err(|_| "Utterance owner unavailable")?;
                let segments = held
                    .owner
                    .observe(
                        base.checked_add(u64::from(observation.frame_offset))
                            .ok_or("Activity timeline exhausted")?,
                        &observation.frames,
                        false,
                    )
                    .map_err(|_| "Continuous activity lost sequence")?;
                segments
                    .into_iter()
                    .map(|segment| {
                        let evidence = held.evidence(&segment, (activity_started, activity_ended));
                        (segment, evidence)
                    })
                    .collect::<Vec<_>>()
            };
            for segment in completed {
                if let Err(error) = utterance_send.try_send(Queued {
                    value: Some(segment),
                    performance: state.performance.clone(),
                }) {
                    let _ = error.into_inner().value.take();
                    state
                        .performance
                        .gate(crate::performance::GateReason::QueueOverflow);
                    let _=app.emit("runtime-error","Avesra is still finishing the previous request; an additional utterance could not be queued.");
                }
            }
            if observation.r#final {
                if observation.samples != 128000 {
                    return Err::<(), String>(
                        "Activity window ended at an unexpected sample".into(),
                    );
                }
                let frames = u64::from(observation.frame_offset) + observation.frames.len() as u64;
                if frames != 100 {
                    return Err("Activity window ended with missing scores".into());
                }
                stream.settle().await?;
                base = base
                    .checked_add(frames)
                    .ok_or("Activity timeline exhausted")?;
                current(app, session, admission)?;
                stream = Stream::open(pairing, session).await?;
            }
        }
    };
    let consumer = async {
        while let Some(mut queued) = utterance_receive.recv().await {
            current(app, session, admission)?;
            let Some((segment, evidence)) = queued.value.take() else {
                continue;
            };
            if segment.completed().elapsed() > Duration::from_secs(2) {
                state
                    .performance
                    .gate(crate::performance::GateReason::QueueExpired);
                let _ = app.emit(
                    "runtime-error",
                    "The pending utterance expired while Avesra was busy; please say it again.",
                );
                continue;
            }
            if !evidence.known {
                state
                    .performance
                    .gate(crate::performance::GateReason::ReferenceUnknown);
                continue;
            }
            process(
                app,
                pairing,
                session,
                admission,
                InputProof::Personal(evidence),
                segment,
                playback,
            )
            .await?;
        }
        Err::<(), String>("Continuous utterance owner stopped".into())
    };
    tokio::pin!(producer);
    tokio::pin!(detector);
    tokio::pin!(consumer);
    tokio::select! {biased; value=&mut producer=>value,value=&mut detector=>value,value=&mut consumer=>value}
}
