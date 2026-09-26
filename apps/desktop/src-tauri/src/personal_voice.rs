//! Normal-launch personal association with the actual Windows owner.
use super::{Binding, OUTPUT_REVISION, SIGNAL_REVISION, Session};
use crate::{
    Runtime,
    connection::{PairingRecord, SessionIdentity},
};
use avesra_contracts::{ErrorCode, actors};
use avesra_core::{
    actor_intents::{self, Identity},
    voice::{self, personal::Voice},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;
const ASR: &str = "ebe59e5a817142986528bbbee5dba8db7b38ed50";
const SPEAKER: &str = "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286";

#[derive(Clone)]
struct Start {
    app: tauri::AppHandle,
    session: SessionIdentity,
    started: Instant,
    cancelled: Arc<AtomicBool>,
}
impl Start {
    fn check(&self) -> Result<(), String> {
        let state = self.app.state::<Runtime>();
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        if self.cancelled.load(Ordering::SeqCst)
            || self.started.elapsed() > Duration::from_secs(60)
            || !local.connected
            || local.locked
            || local.settings.explicit_mute
            || local.settings.paused
            || local.settings.deafened
            || local.enrollment_capture
            || local.microphone_check
            || local.capture_epoch != self.session.epoch
            || local.action_epoch != self.session.action_epoch
            || state.connection_generation.load(Ordering::SeqCst) != self.session.generation
            || !state
                .acknowledged_session
                .lock()
                .map_err(|_| "Session unavailable")?
                .is_some_and(|s| {
                    s.id == self.session.id
                        && s.epoch == self.session.epoch
                        && s.generation == self.session.generation
                })
        {
            return Err("Personal voice startup context changed".into());
        }
        Ok(())
    }
    async fn actor(
        &self,
        pairing: &PairingRecord,
        request: &actors::Request,
    ) -> Result<actors::Reply, String> {
        self.check()?;
        let future = crate::connection::actor_operation(pairing, request);
        tokio::pin!(future);
        let result = loop {
            tokio::select! {
                result=&mut future=>break result,
                _=tokio::time::sleep(Duration::from_millis(50))=>if let Err(e)=self.check(){break Err(e)}
            }
        };
        if result.is_err() || self.check().is_err() {
            let _ = crate::connection::actor_cancel(
                pairing,
                &actors::Cancel {
                    version: actors::VERSION,
                    attempt: request.attempt,
                    session: request.session,
                    action_epoch: request.action_epoch,
                },
            )
            .await;
            return Err(
                "Owner registration result uncertain; reconnect to reconcile its actual status"
                    .into(),
            );
        }
        result
    }
}
pub async fn start_personal(
    app: &tauri::AppHandle,
    pairing: PairingRecord,
    session: SessionIdentity,
) -> Result<bool, String> {
    let state = app.state::<Runtime>();
    let mutation = state
        .qualification
        .1
        .clone()
        .try_lock_owned()
        .map_err(|_| "Voice management is busy")?;
    if !state.qualification.can_restore() {
        return Ok(false);
    }
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management is busy")?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _caller = crate::output::Caller(cancelled.clone());
    let start = Start {
        app: app.clone(),
        session,
        started: Instant::now(),
        cancelled,
    };
    start.check()?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Owner directory unavailable")?;
    let microphone = state
        .local
        .lock()
        .map_err(|_| "Local state unavailable")?
        .settings
        .microphone
        .clone()
        .ok_or("Choose an available microphone")?;
    let server = pairing.server_fingerprint()?;
    if pairing.device_id != session.device
        || server
            != session
                .server_fingerprint
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect::<String>()
    {
        return Err("Pairing changed".into());
    }
    // Retain both mutation owners through real storage/network retirement if the
    // outer connection task is dropped. Caller loss withdraws publication.
    tauri::async_runtime::spawn(async move {
        let reader=start.clone(); let root=directory.clone();
        let (voice,identity,intent,_owner,_mutation)=tokio::task::spawn_blocking(move || {
            let (actor,owner_revision)=crate::owner::personal_identity(&root,&mut ||reader.check().map_err(|_|ErrorCode::Stale)).map_err(|_|"Existing Windows owner is unavailable; it was not replaced")?;
            let identity=Identity{server,device:session.device,actor,owner_revision};
            let intent=actor_intents::load(&root.join("actor-intents.db"),&identity).map_err(|_|"Owner registration intent unavailable")?;
            let candidate=crate::profiles::personal_seed(&root)?.filter(|v|v.microphone==microphone && v.model_revision==SPEAKER);
            let expected=Voice{version:1,id:Uuid::new_v4(),actor,owner_revision,microphone,model_revision:SPEAKER.into(),source:candidate.as_ref().map(|v|(v.id,v.revision)),seed:candidate.map(|v|v.representation),observations:Vec::new()};
            let voice=crate::profiles::read_personal(&root,&expected)?.unwrap_or(expected);
            Ok::<_,String>((voice,identity,intent,owner,mutation))
        }).await.map_err(|_|"Personal voice reader stopped")??;
        start.check()?;
        let mut request=actors::Request{version:actors::VERSION,request:Uuid::new_v4(),attempt:Uuid::new_v4(),session:session.id,action_epoch:session.action_epoch,command:actors::Command::Status};
        let status=start.actor(&pairing,&request).await?;
        let registration=match status.binding {
            Some(binding)=> {
                if binding.revoked || binding.actor!=identity.actor || binding.owner_revision!=identity.owner_revision || !intent.as_ref().is_some_and(|v|v.request==binding.registered_by) { return Err("Existing Spark owner is revoked, different or unreconciled; manage it in People".into()); }
                binding
            }
            None=> {
                let writer=start.clone(); let root=directory.clone(); let write_identity=identity.clone();
                let intent=tokio::task::spawn_blocking(move ||actor_intents::remember(&root.join("actor-intents.db"),&write_identity,&mut ||writer.check().map_err(|_|ErrorCode::Stale)))
                    .await.map_err(|_|"Registration writer stopped")?.map_err(|_|"Registration intent was not confirmed")?;
                request.request=intent.request; request.attempt=Uuid::new_v4(); request.command=actors::Command::Register{actor:identity.actor,owner_revision:identity.owner_revision};
                start.actor(&pairing,&request).await?.binding.ok_or("Owner registration unavailable")?
            }
        };
        start.check()?;
        crate::connection::voice_analysis_available(&pairing).await?;
        crate::connection::voice_activity_available(&pairing,true).await?;
        start.check()?;
        let consent=voice.id;
        let context=voice::Context{device:session.device,session:session.id,capture_epoch:session.epoch,action_epoch:session.action_epoch,microphone:voice.microphone.clone(),actor:Some(identity.actor),grant_revision:Some(consent)};
        let directed=crate::connection::directedness::metadata(&pairing,session,&context,||start.check()).await?;
        let binding=Binding{candidate:voice.id,revision:voice.id,actor:identity.actor,owner_revision:identity.owner_revision,registration,microphone:voice.microphone.clone(),device:session.device,session:session.id,generation:session.generation,action_epoch:session.action_epoch,asr_revision:ASR,speaker_revision:SPEAKER,activity_revision:Some(avesra_contracts::activity::REVISION),activity_streaming:true};
        let profile=voice.clone().admit(&context,[ASR.into(),SIGNAL_REVISION.into(),directed.adapter_revision.clone(),SIGNAL_REVISION.into(),OUTPUT_REVISION.into()]).map_err(|_|"Personal voice evidence invalid")?;
        let root=directory.clone(); let writer=start.clone();
        let (owner,mutation)=(_owner,_mutation);
        tokio::task::spawn_blocking(move || {
            let _owner=owner; let _mutation=mutation;
            let mut profile=Some(profile);
            crate::profiles::save_personal(&root,&voice,&mut |temporary,destination| {
                writer.check()?;
                let state=writer.app.state::<Runtime>(); let mut local=state.local.lock().map_err(|_|"Local state unavailable")?;
                let mut slot=state.qualification.0.lock().map_err(|_|"Voice state unavailable")?;
                if writer.cancelled.load(Ordering::SeqCst) || local.capture_epoch!=session.epoch || !binding.current(&state,&local) || slot.revoked || slot.suspended || slot.active.is_some() { return Err("Personal voice activation withdrawn".into()); }
                std::fs::rename(temporary,destination).map_err(|_|"Personal voice publication failed")?;
                let learning=voice.learning();
                slot.active=Some(Session{id:Uuid::new_v4(),binding:binding.clone(),created:Instant::now(),saved_held_out:Vec::new(),threshold:None,observations:Vec::new(),activity:super::activity::Calibration::default(),gate:None,consent:Some(consent),directed:Some(directed.clone()),profile:profile.take(),capture:None});
                local.enrolled=true; local.voice_ready=true;
                local.personal_voice=avesra_core::state::PersonalVoiceStatus{state:if learning{avesra_core::state::PersonalVoicePhase::Learning}else{avesra_core::state::PersonalVoicePhase::Listening},reason:if learning{"Listening and learning your voice from natural conversation. Personal recognition is provisional."}else{"Listening with your saved voice. Personal recognition is provisional."}.into()};
                drop(slot); local.capture_epoch=local.capture_epoch.saturating_add(1); local.refresh(); state.publish(&local); let _=writer.app.emit("runtime-state",local.clone());
                Ok(())
            })
        }).await.map_err(|_|"Personal voice writer stopped")??;
        Ok(true)
    }).await.map_err(|_|"Personal voice startup coordinator stopped")?
}

/// Called only after this utterance was durably accepted. Learning cannot
/// convert a rejected recording, self-output residual or a UI label into evidence.
pub async fn learn_personal(
    app: &tauri::AppHandle,
    context: &voice::Context,
    utterance: Uuid,
    samples: u32,
    embedding: Vec<f32>,
) -> Result<(), String> {
    let state = app.state::<Runtime>();
    let mutation = state
        .qualification
        .1
        .clone()
        .try_lock_owned()
        .map_err(|_| "Voice learning writer busy")?;
    let owner = state
        .owner_setup
        .clone()
        .try_lock_owned()
        .map_err(|_| "Owner management busy")?;
    let (voice, session) = {
        let local = state.local.lock().map_err(|_| "Local state unavailable")?;
        let mut slot = state
            .qualification
            .0
            .lock()
            .map_err(|_| "Voice state unavailable")?;
        let current = slot
            .active
            .as_mut()
            .filter(|v| {
                v.binding.current(&state, &local) && v.context(local.capture_epoch) == *context
            })
            .ok_or("Voice learning context changed")?;
        let profile = current
            .profile
            .as_mut()
            .ok_or("Voice admission unavailable")?;
        if !profile
            .learn_personal(utterance, samples, &embedding)
            .map_err(|_| "Invalid voice measurement")?
        {
            return Ok(());
        }
        (
            profile
                .personal_voice()
                .ok_or("Personal evidence missing")?
                .clone(),
            current.id,
        )
    };
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|_| "Voice directory unavailable")?;
    let app = app.clone();
    let context = context.clone();
    tokio::task::spawn_blocking(move || {
        let _owner = owner;
        let _mutation = mutation;
        crate::profiles::save_personal(&directory, &voice, &mut |temporary, destination| {
            let state = app.state::<Runtime>();
            let mut local = state.local.lock().map_err(|_| "Local state unavailable")?;
            let slot = state
                .qualification
                .0
                .lock()
                .map_err(|_| "Voice state unavailable")?;
            if !slot.active.as_ref().is_some_and(|v| {
                v.id == session
                    && v.binding.current(&state, &local)
                    && v.context(local.capture_epoch) == context
            }) {
                return Err("Voice learning publication withdrawn".into());
            }
            std::fs::rename(temporary, destination)
                .map_err(|_| "Voice learning publication failed")?;
            local.personal_voice.state = if voice.learning() {
                avesra_core::state::PersonalVoicePhase::Learning
            } else {
                avesra_core::state::PersonalVoicePhase::Listening
            };
            local.personal_voice.reason = if voice.learning() {
                "Listening and learning your voice from natural conversation."
            } else {
                "Listening with your learned personal voice."
            }
            .into();
            drop(slot);
            local.refresh();
            state.publish(&local);
            let _ = app.emit("runtime-state", local.clone());
            Ok(())
        })
    })
    .await
    .map_err(|_| "Voice learning writer stopped")?
}
