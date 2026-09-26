//! Durable native acceptance. Serialized records are history, not gate tokens.
use crate::{store::Store, voice::Conversation};
use avesra_contracts::ErrorCode;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[path = "conversation_planner.rs"]
mod planner;
#[path = "conversation_tasks.rs"]
mod tasks;
pub use planner::{
    ObservationClaim, ObservationRequest, PlannerAuthority, PlannerCancellation, PlannerClaim,
    PlannerRequest, PlannerRetirement, PlannerSummary, StoredReply,
};
pub(crate) use planner::{PLAN_SCHEMA, REPLY_SCHEMA, SEQUENCE_SCHEMA};
pub(crate) use tasks::verified_task_source;
pub use tasks::{ExactTaskRequest, LinkedTask, TaskAuthority, TaskResolution, TaskView};
pub(crate) use tasks::{SCHEMA as TASK_SCHEMA, validate_dispatch as validate_linked_dispatch};

const MAX_BODY: usize = 65_536;
pub(crate) const SCHEMA: &str = "CREATE TABLE accepted_conversations(id TEXT PRIMARY KEY CHECK(length(id)=36),revision TEXT UNIQUE NOT NULL CHECK(length(revision)=36),actor TEXT NOT NULL CHECK(length(actor)=36),device TEXT NOT NULL CHECK(length(device)=36),session TEXT NOT NULL CHECK(length(session)=36),utterance TEXT NOT NULL CHECK(length(utterance)=36),body TEXT NOT NULL CHECK(length(body)<=65536),state TEXT NOT NULL CHECK(state IN ('accepted','planning','waiting_input','answered','cancelled','suspended')),UNIQUE(device,session,utterance))";

pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    tasks::check_schema(db, version)?;
    planner::check_schema(db, version)?;
    let attached:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE tbl_name='accepted_conversations' AND type NOT IN ('table','index'))",[],|r|r.get(0)).map_err(|_|ErrorCode::Malformed)?;
    if attached {
        return Err(ErrorCode::Malformed);
    }
    let sql: Option<String> = db
        .query_row(
            "SELECT substr(sql,1,2049) FROM sqlite_master WHERE name='accepted_conversations'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    if version < 5 {
        return if sql.is_none() {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        };
    }
    if sql.as_deref() != Some(SCHEMA) {
        return Err(ErrorCode::Malformed);
    }
    let mut statement = db
        .prepare("PRAGMA index_list('accepted_conversations')")
        .map_err(|_| ErrorCode::Malformed)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
    let mut found = [false; 3];
    while let Some(row) = rows.next().map_err(|_| ErrorCode::Malformed)? {
        let name: String = row.get(1).map_err(|_| ErrorCode::Malformed)?;
        let unique: i64 = row.get(2).map_err(|_| ErrorCode::Malformed)?;
        let origin: String = row.get(3).map_err(|_| ErrorCode::Malformed)?;
        let partial: i64 = row.get(4).map_err(|_| ErrorCode::Malformed)?;
        let index = match (name.as_str(), origin.as_str()) {
            ("sqlite_autoindex_accepted_conversations_1", "pk") => 0,
            ("sqlite_autoindex_accepted_conversations_2", "u") => 1,
            ("sqlite_autoindex_accepted_conversations_3", "u") => 2,
            _ => return Err(ErrorCode::Malformed),
        };
        if found[index] || unique != 1 || partial != 0 {
            return Err(ErrorCode::Malformed);
        }
        found[index] = true;
    }
    if found != [true; 3] {
        return Err(ErrorCode::Malformed);
    }
    for (query, columns) in [
        (
            "PRAGMA index_info('sqlite_autoindex_accepted_conversations_1')",
            vec![(0, "id")],
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_accepted_conversations_2')",
            vec![(1, "revision")],
        ),
        (
            "PRAGMA index_info('sqlite_autoindex_accepted_conversations_3')",
            vec![(3, "device"), (4, "session"), (5, "utterance")],
        ),
    ] {
        let mut statement = db.prepare(query).map_err(|_| ErrorCode::Malformed)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
        for (sequence, (index, name)) in columns.into_iter().enumerate() {
            let row = rows
                .next()
                .map_err(|_| ErrorCode::Malformed)?
                .ok_or(ErrorCode::Malformed)?;
            let actual: (i64, i64, String) = (
                row.get(0).map_err(|_| ErrorCode::Malformed)?,
                row.get(1).map_err(|_| ErrorCode::Malformed)?,
                row.get(2).map_err(|_| ErrorCode::Malformed)?,
            );
            if actual != (sequence as i64, index, name.to_owned()) {
                return Err(ErrorCode::Malformed);
            }
        }
        if rows.next().map_err(|_| ErrorCode::Malformed)?.is_some() {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub device: Uuid,
    pub session: Uuid,
    pub utterance: Uuid,
}
impl Source {
    fn valid(self) -> bool {
        ![self.device, self.session, self.utterance]
            .iter()
            .any(Uuid::is_nil)
    }
}
/// Exact native cancellation identity; metadata alone grants no authority.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CancellationTarget {
    pub actor: Uuid,
    pub source: Source,
    pub id: Uuid,
    pub revision: Uuid,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: Uuid,
    revision: Uuid,
    actor: Uuid,
    source: Source,
    capture_epoch: u64,
    action_epoch: u64,
    microphone: String,
    profile: Uuid,
    qualification: Uuid,
    grant: Uuid,
    text: String,
    created_ms: u64,
}
impl Record {
    fn validate(&self) -> Result<(), ErrorCode> {
        if !self.source.valid()
            || [
                self.id,
                self.revision,
                self.actor,
                self.profile,
                self.qualification,
                self.grant,
            ]
            .iter()
            .any(Uuid::is_nil)
            || self.capture_epoch == 0
            || self.action_epoch == 0
            || !crate::state::valid_audio_device_id(&self.microphone)
            || self.text.trim().is_empty()
            || self.text.len() > 8192
            || self
                .text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            || self.created_ms == 0
            || self.created_ms > avesra_contracts::browser::MAX_SAFE_COUNTER
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
/// Produced only by successful current native commit. No Deserialize/Clone.
pub struct DurableTurn {
    id: Uuid,
    revision: Uuid,
    actor: Uuid,
    source: Source,
}
impl DurableTurn {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn revision(&self) -> Uuid {
        self.revision
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn source(&self) -> Source {
        self.source
    }
}
#[derive(Serialize)]
pub struct Summary {
    pub id: Uuid,
    pub revision: Uuid,
    pub actor: Uuid,
    pub source: Source,
    pub state: String,
    pub created_ms: u64,
    pub linked_task: Option<LinkedTask>,
    pub planner: Option<PlannerSummary>,
}
impl Store {
    pub fn accept_conversation(
        &mut self,
        conversation: Conversation,
        authorize: &mut dyn FnMut(&Conversation) -> Result<(), ErrorCode>,
    ) -> Result<DurableTurn, ErrorCode> {
        if !conversation.precommit_current() {
            return Err(ErrorCode::Expired);
        }
        let context = conversation.context();
        let record = Record {
            id: Uuid::new_v4(),
            revision: Uuid::new_v4(),
            actor: context.actor.ok_or(ErrorCode::Unauthenticated)?,
            source: Source {
                device: context.device,
                session: context.session,
                utterance: conversation.utterance(),
            },
            capture_epoch: context.capture_epoch,
            action_epoch: context.action_epoch,
            microphone: context.microphone.clone(),
            profile: conversation.profile_revision(),
            qualification: conversation.qualification_revision(),
            grant: context.grant_revision.ok_or(ErrorCode::Unauthenticated)?,
            text: conversation.text().to_owned(),
            created_ms: u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| ErrorCode::Expired)?
                    .as_millis(),
            )
            .map_err(|_| ErrorCode::Expired)?,
        };
        record.validate()?;
        let body = serde_json::to_string(&record).map_err(|_| ErrorCode::Malformed)?;
        if body.len() > MAX_BODY {
            return Err(ErrorCode::TooLarge);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        tx.execute("INSERT INTO accepted_conversations(id,revision,actor,device,session,utterance,body,state) VALUES(?1,?2,?3,?4,?5,?6,?7,'accepted')",params![record.id.to_string(),record.revision.to_string(),record.actor.to_string(),record.source.device.to_string(),record.source.session.to_string(),record.source.utterance.to_string(),body]).map_err(|_|ErrorCode::Storage)?;
        let exact:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM accepted_conversations WHERE id=?1 AND revision=?2 AND actor=?3 AND device=?4 AND session=?5 AND utterance=?6 AND body=?7 AND state='accepted')",params![record.id.to_string(),record.revision.to_string(),record.actor.to_string(),record.source.device.to_string(),record.source.session.to_string(),record.source.utterance.to_string(),body],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
        if !exact {
            return Err(ErrorCode::Malformed);
        }
        if !conversation.precommit_current() {
            return Err(ErrorCode::Expired);
        }
        authorize(&conversation)?;
        if !conversation.precommit_current() {
            return Err(ErrorCode::Expired);
        }
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(DurableTurn {
            id: record.id,
            revision: record.revision,
            actor: record.actor,
            source: record.source,
        })
    }
    /// Authenticated actor must come from the native caller. This is recovery
    /// metadata, not a constructor for an executable or accepted gate token.
    pub fn conversation_status(
        &self,
        actor: Uuid,
        source: Source,
    ) -> Result<Option<Summary>, ErrorCode> {
        if actor.is_nil() || !source.valid() {
            return Err(ErrorCode::Malformed);
        }
        let row:Option<(String,String,Vec<u8>,String)>=self.connection.query_row("SELECT substr(id,1,37),substr(revision,1,37),substr(CAST(body AS BLOB),1,65537),substr(state,1,32) FROM accepted_conversations WHERE actor=?1 AND device=?2 AND session=?3 AND utterance=?4",params![actor.to_string(),source.device.to_string(),source.session.to_string(),source.utterance.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_|ErrorCode::Storage)?;
        let Some((id, revision, body, state)) = row else {
            return Ok(None);
        };
        if body.len() > MAX_BODY {
            return Err(ErrorCode::Malformed);
        }
        let record: Record = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
        record.validate()?;
        if record.actor != actor
            || record.source != source
            || record.id.to_string() != id
            || record.revision.to_string() != revision
            || !matches!(
                state.as_str(),
                "accepted" | "planning" | "waiting_input" | "answered" | "cancelled" | "suspended"
            )
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(Some(Summary {
            id: record.id,
            revision: record.revision,
            actor,
            source,
            state,
            created_ms: record.created_ms,
            linked_task: tasks::linked(&self.connection, &record)?,
            planner: planner::summary(&self.connection, &record)?,
        }))
    }
    /// Explicit native cancellation of this exact accepted turn. Future planner
    /// ownership must observe this state; this never rolls back a dispatched effect.
    pub fn cancel_conversation(
        &mut self,
        actor: Uuid,
        source: Source,
        id: Uuid,
        revision: Uuid,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Vec<Uuid>, ErrorCode> {
        if id.is_nil() || revision.is_nil() {
            return Err(ErrorCode::Malformed);
        }
        let summary = self
            .conversation_status(actor, source)?
            .ok_or(ErrorCode::Stale)?;
        if summary.id != id || summary.revision != revision {
            return Err(ErrorCode::Stale);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        if tx.execute("UPDATE accepted_conversations SET state='cancelled' WHERE id=?1 AND revision=?2 AND actor=?3 AND state IN ('accepted','planning','waiting_input','suspended','cancelled')",params![id.to_string(),revision.to_string(),actor.to_string()]).map_err(|_|ErrorCode::Storage)?!=1{return Err(ErrorCode::Stale);}
        let (record, _) = tasks::read_record(&tx, id)?;
        planner::cancel(&tx, &record)?;
        let dispatches = if let Some(linked) = summary.linked_task {
            let now = tasks::wall_time()?;
            crate::ledger::cancel_task_in(&tx, linked.task, actor, now)?
        } else {
            Vec::new()
        };
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(dispatches)
    }
}
