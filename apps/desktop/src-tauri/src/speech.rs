//! Native-only accepted reply playback. No webview text or history ingress.
use crate::{
    Runtime,
    connection::{self, MediaEndpoint, PairingRecord, SessionIdentity},
};
use avesra_contracts::{actors, planner, speech, voice::VoiceIdentity};
use avesra_windows::{
    audio::{PlaybackFrame, PlaybackRate},
    effects::PublishedReply,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};
use uuid::Uuid;
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
/// Confirms native callback submission only, never audible delivery.
pub struct Submitted {
    samples: u64,
}
impl Submitted {
    pub fn samples(&self) -> u64 {
        self.samples
    }
}
struct Admission {
    reply: Arc<PublishedReply>,
    session: SessionIdentity,
    voice: VoiceIdentity,
    withdrawn: Arc<AtomicBool>,
    epoch: AtomicU64,
    id: Uuid,
    deadline: Instant,
}
impl Admission {
    fn current_locked(
        &self,
        state: &Runtime,
        local: &avesra_core::state::LocalState,
        acknowledged: bool,
    ) -> bool {
        let context = self.reply.context();
        let epoch = self.epoch.load(Ordering::SeqCst);
        !self.withdrawn.load(Ordering::SeqCst)
            && self.reply.current()
            && Instant::now() < self.deadline
            && local.connected
            && local.enrolled
            && local.voice_ready
            && !local.locked
            && !local.settings.deafened
            && !local.settings.paused
            && local.settings.speaker.is_some()
            && local.action_epoch == context.action_epoch
            && local.playback_epoch == epoch
            && state.connection_generation.load(Ordering::SeqCst) == self.session.generation
            && state.acknowledged_session.lock().is_ok_and(|ack| {
                ack.is_some_and(|s| {
                    s.id == context.session
                        && s.device == context.device
                        && s.generation == self.session.generation
                        && s.server_fingerprint == self.session.server_fingerprint
                        && s.action_epoch == context.action_epoch
                        && (!acknowledged || s.playback_epoch == epoch)
                })
            })
    }
    fn check(&self, app: &tauri::AppHandle, acknowledged: bool) -> Result<(), String> {
        let state = app.state::<Runtime>();
        let local = state
            .local
            .lock()
            .map_err(|_| "Local output state unavailable")?;
        if self.current_locked(&state, &local, acknowledged) {
            Ok(())
        } else {
            Err("Accepted output permission changed or expired".into())
        }
    }
    fn request(&self) -> speech::Request {
        speech::Request {
            version: speech::VERSION,
            source: speech::Source {
                planner: self.reply.context().clone(),
                reply_revision: self.reply.revision(),
                response: self.reply.response().clone(),
            },
            request: self.id,
            playback_epoch: self.epoch.load(Ordering::SeqCst),
            voice: self.voice.clone(),
        }
    }
}
async fn live<T>(
    app: &tauri::AppHandle,
    admission: &Admission,
    deadline: Instant,
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::pin!(future);
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! { biased;
            _=tokio::time::sleep_until(deadline.into())=>return Err("Accepted output expired".into()),
            _=tick.tick()=>admission.check(app,true)?,
            result=&mut future=>{ admission.check(app,true)?; return result; }
        }
    }
}
async fn registration(
    app: &tauri::AppHandle,
    admission: &Admission,
    pairing: &PairingRecord,
    deadline: Instant,
) -> Result<(), String> {
    let request = actors::Request {
        version: actors::VERSION,
        request: Uuid::new_v4(),
        attempt: Uuid::new_v4(),
        session: admission.session.id,
        action_epoch: admission.reply.context().action_epoch,
        command: actors::Command::Status,
    };
    let reply = live(
        app,
        admission,
        deadline,
        connection::actor_operation(pairing, &request),
    )
    .await?;
    if reply.binding.as_ref() != Some(admission.reply.binding())
        || admission.reply.binding().revoked
    {
        return Err("Accepted owner registration changed".into());
    }
    Ok(())
}
async fn run(
    app: &tauri::AppHandle,
    reply: PublishedReply,
    voice: VoiceIdentity,
    withdrawn: Arc<AtomicBool>,
    started: Instant,
) -> Result<Submitted, String> {
    let mut owner = crate::output::Owner::reserve(app)?;
    let prep_deadline = started + Duration::from_secs(8);
    voice.validate().map_err(|_| "Invalid selected voice")?;
    speech::Source {
        planner: reply.context().clone(),
        reply_revision: reply.revision(),
        response: reply.response().clone(),
    }
    .validate()
    .map_err(|_| "Speech is unavailable for this reply; its full text remains saved")?;
    let state = app.state::<Runtime>();
    let session = {
        let local = state
            .local
            .lock()
            .map_err(|_| "Local output state unavailable")?;
        let session = state
            .acknowledged_session
            .lock()
            .map_err(|_| "Session unavailable")?
            .ok_or("Session unavailable")?;
        if session.device != reply.context().device
            || session.id != reply.context().session
            || session.action_epoch != reply.context().action_epoch
            || session.playback_epoch != local.playback_epoch
            || session.generation != state.connection_generation.load(Ordering::SeqCst)
        {
            return Err("Accepted output session changed".into());
        }
        session
    };
    let admission = Arc::new(Admission {
        reply: Arc::new(reply),
        session,
        voice,
        withdrawn,
        epoch: AtomicU64::new(session.playback_epoch),
        id: Uuid::new_v4(),
        deadline: started + Duration::from_secs(80),
    });
    admission.check(app, true)?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Native owner directory unavailable")?;
    let owner_slot = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let expected = admission.clone();
    // Await the actual reader; dropping a timer/caller cannot release its slot.
    let pairing = tokio::task::spawn_blocking(move || {
        let _owner_slot = owner_slot;
        crate::planner::paired_owner(&directory, expected.reply.binding(), expected.session)
    })
    .await
    .map_err(|_| "Output preparation stopped")??;
    admission.check(app, true)?;
    if Instant::now() >= prep_deadline {
        return Err("Output preparation expired".into());
    }
    registration(app, &admission, &pairing, prep_deadline).await?;
    {
        let mut local = state
            .local
            .lock()
            .map_err(|_| "Local output state unavailable")?;
        if !admission.current_locked(&state, &local, true) || Instant::now() >= prep_deadline {
            return Err("Output admission expired".into());
        }
        local.playback_epoch = local
            .playback_epoch
            .checked_add(1)
            .filter(|v| *v <= avesra_contracts::browser::MAX_SAFE_COUNTER)
            .ok_or("Output epoch exhausted")?;
        admission
            .epoch
            .store(local.playback_epoch, Ordering::SeqCst);
        local.refresh();
        state.publish(&local);
        owner.bind(local.playback_epoch, session.generation);
        state.media.open_reply(
            &local,
            admission.id,
            &admission.reply,
            admission.withdrawn.clone(),
        )?;
        let _ = app.emit("runtime-state", local.clone());
    };
    // New output epoch must be acknowledged, but unrelated mic epochs may change.
    let ready = async {
        loop {
            admission.check(app, false)?;
            if admission.check(app, true).is_ok()
                && state
                    .media
                    .output_state(admission.epoch.load(Ordering::SeqCst), admission.id)?
                    .0
            {
                return Ok::<(), String>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::time::timeout_at(prep_deadline.into(), ready)
        .await
        .map_err(|_| "Output device/session preparation expired")??;
    let mut socket = live(
        app,
        &admission,
        prep_deadline,
        connection::voice_socket(&pairing, MediaEndpoint::Speech),
    )
    .await?;
    let request = admission.request();
    request
        .validate()
        .map_err(|_| "Invalid accepted output request")?;
    let mut sent = false;
    let result = live(
        app,
        &admission,
        admission.deadline,
        play(app, &admission, &mut socket, &request, &mut sent),
    )
    .await;
    // Logical stop precedes remote cancellation and actual device retirement.
    admission.withdrawn.store(true, Ordering::SeqCst);
    owner.withdraw()?;
    if result.is_err() && sent {
        let _ = control(
            &mut socket,
            speech::Control::Cancel {
                request: admission.id,
            },
        )
        .await;
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            connection::planner_cancel(
                &pairing,
                &planner::Cancel {
                    version: planner::VERSION,
                    context: admission.reply.context().clone(),
                },
            ),
        )
        .await;
    }
    let _ = tokio::time::timeout(Duration::from_millis(500), socket.close(None)).await;
    owner.finish().await?;
    result
}
async fn control(socket: &mut Socket, value: speech::Control) -> Result<(), String> {
    let encoded = serde_json::to_string(&value).map_err(|_| "Output control encoding failed")?;
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(encoded.into())),
    )
    .await
    .map_err(|_| "Output control expired")?
    .map_err(|_| "Output transport closed".into())
}
async fn play(
    app: &tauri::AppHandle,
    admission: &Admission,
    socket: &mut Socket,
    request: &speech::Request,
    sent: &mut bool,
) -> Result<Submitted, String> {
    let encoded = serde_json::to_string(request).map_err(|_| "Output request encoding failed")?;
    if encoded.len() > planner::MAX_REQUEST_BYTES {
        return Err("Output request exceeds limit".into());
    }
    admission.check(app, true)?;
    *sent = true;
    tokio::time::timeout(
        Duration::from_millis(500),
        socket.send(Message::Text(encoded.into())),
    )
    .await
    .map_err(|_| "Output request expired")?
    .map_err(|_| "Output transport closed")?;
    let context = request.stream_context();
    let mut origin = None;
    let mut expected = 1u64;
    let mut offset = 0u64;
    let mut pending: Option<PlaybackFrame> = None;
    loop {
        let budget = if origin.is_none() {
            Duration::from_secs(35)
        } else {
            Duration::from_millis(500)
        };
        let message = tokio::time::timeout(budget, socket.next())
            .await
            .map_err(|_| "Output receive expired")?
            .ok_or("Output transport closed")?
            .map_err(|_| "Output transport failed")?;
        admission.check(app, true)?;
        let Message::Text(text) = message else {
            return Err("Unexpected output frame".into());
        };
        if text.len() > 8192 {
            return Err("Output frame exceeds limit".into());
        }
        let event: speech::Event =
            serde_json::from_str(&text).map_err(|_| "Invalid output frame")?;
        event
            .validate(&context)
            .map_err(|_| "Stale or invalid output frame")?;
        match event.message {
            speech::Message::Ready { .. } if origin.is_none() => {
                if !app
                    .state::<Runtime>()
                    .media
                    .output_state(context.playback_epoch, admission.id)?
                    .0
                {
                    return Err("Output device is not ready".into());
                }
                origin = Some(Instant::now());
                control(
                    socket,
                    speech::Control::Play {
                        request: admission.id,
                    },
                )
                .await?;
            }
            speech::Message::Audio {
                sequence,
                sample_offset,
                samples,
            } => {
                let origin = origin.ok_or("Output audio before Ready")?;
                let now = Instant::now();
                let due = Duration::from_millis(sample_offset / 24);
                if sequence != expected
                    || sample_offset != offset
                    || now.duration_since(origin) > due + Duration::from_millis(500)
                    || due > now.duration_since(origin) + Duration::from_millis(100)
                    || now >= origin + Duration::from_secs(32)
                {
                    return Err("Output lost continuity or freshness".into());
                }
                if let Some(frame) = pending.take() {
                    app.state::<Runtime>().media.output_frame(frame)?;
                }
                let mut pcm = [0i16; 480];
                for (dst, src) in pcm.iter_mut().zip(samples) {
                    *dst = src;
                }
                pending = Some(PlaybackFrame {
                    epoch: context.playback_epoch,
                    utterance: admission.id,
                    sequence,
                    captured: (origin + due).min(now),
                    deadline: origin + Duration::from_secs(32),
                    device_time: None,
                    rate: PlaybackRate::Pcm24000,
                    samples: pcm,
                    valid_samples: 480,
                    final_frame: false,
                });
                offset += 480;
                expected += 1;
            }
            speech::Message::End {
                chunks,
                samples,
                outcome,
            } if origin.is_some() && chunks == expected - 1 && samples == offset => {
                if outcome != speech::Completion::Complete {
                    return Err("Speech ended before the complete reply".into());
                }
                let mut frame = pending.take().ok_or("Output ended without audio")?;
                frame.final_frame = true;
                app.state::<Runtime>().media.output_frame(frame)?;
                let finish = async {
                    loop {
                        admission.check(app, true)?;
                        if app
                            .state::<Runtime>()
                            .media
                            .output_state(context.playback_epoch, admission.id)?
                            .1
                        {
                            return Ok::<(), String>(());
                        }
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                };
                tokio::time::timeout(Duration::from_millis(1500), finish)
                    .await
                    .map_err(|_| "Output did not confirm submission")??;
                control(
                    socket,
                    speech::Control::Submitted {
                        request: admission.id,
                        samples,
                    },
                )
                .await?;
                let drain = async {
                    loop {
                        admission.check(app, true)?;
                        if app
                            .state::<Runtime>()
                            .media
                            .output_state(context.playback_epoch, admission.id)?
                            .2
                        {
                            return Ok::<(), String>(());
                        }
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                };
                tokio::time::timeout(Duration::from_millis(3100), drain)
                    .await
                    .map_err(|_| "Output drain expired")??;
                return Ok(Submitted { samples });
            }
            _ => return Err("Unexpected output sequence".into()),
        }
    }
}
/// Native qualified producer only. The reply is consumed once; cancellation or
/// unavailable output preserves immutable answer history, with no replay handle.
pub async fn speak(
    app: tauri::AppHandle,
    reply: PublishedReply,
    voice: VoiceIdentity,
) -> Result<Submitted, String> {
    let started = Instant::now();
    let withdrawn = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(withdrawn.clone());
    tauri::async_runtime::spawn(async move { run(&app, reply, voice, withdrawn, started).await })
        .await
        .map_err(|_| "Accepted output coordinator stopped")?
}
