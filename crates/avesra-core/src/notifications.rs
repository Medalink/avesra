//! Transactional committed-event journal. No event is action or voice authority.
use crate::store::Store;
use avesra_contracts::{Action, ErrorCode};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const TABLES: [(&str, &str); 3] = [
    (
        "notification_events",
        "CREATE TABLE notification_events(id TEXT PRIMARY KEY NOT NULL,actor TEXT NOT NULL,device TEXT NOT NULL,kind TEXT NOT NULL,source TEXT NOT NULL,revision TEXT NOT NULL,at_ms INTEGER NOT NULL,body TEXT CHECK(body IS NULL OR length(CAST(body AS BLOB))<=4096),settled INTEGER NOT NULL CHECK(settled IN (0,1)))",
    ),
    (
        "notification_batches",
        "CREATE TABLE notification_batches(id TEXT PRIMARY KEY NOT NULL,actor TEXT NOT NULL,device TEXT NOT NULL,kind TEXT NOT NULL,at_ms INTEGER NOT NULL,members TEXT NOT NULL CHECK(length(CAST(members AS BLOB))<=2048),state TEXT NOT NULL CHECK(state IN ('pending','submitted','suppressed','uncertain')),submitted_ms INTEGER)",
    ),
    (
        "notification_capacity",
        "CREATE TABLE notification_capacity(id TEXT PRIMARY KEY NOT NULL CHECK(id='global'),suppressed INTEGER NOT NULL CHECK(suppressed>=0),at_ms INTEGER NOT NULL)",
    ),
];
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    for (name, schema) in TABLES {
        let mut query = db
            .prepare(
                "SELECT type,name,sql FROM sqlite_master WHERE tbl_name=?1 ORDER BY type LIMIT 3",
            )
            .map_err(|_| ErrorCode::Storage)?;
        let rows = query
            .query_map([name], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| ErrorCode::Storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ErrorCode::Storage)?;
        if version < 15 {
            if !rows.is_empty() {
                return Err(ErrorCode::Malformed);
            }
        } else if rows
            != [
                ("index".into(), format!("sqlite_autoindex_{name}_1"), None),
                ("table".into(), name.into(), Some(schema.into())),
            ]
        {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Learning,
    Action,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Learning => "learning",
            Self::Action => "action",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Description {
    Fact { value: String },
    RoutineCandidate { name: String },
    VerifiedAction { operation: String },
}
#[derive(Serialize)]
pub struct Event {
    pub id: Uuid,
    pub kind: Kind,
    pub at_ms: u64,
    pub description: Option<Description>,
}
pub struct Batch {
    id: Uuid,
    actor: Uuid,
    device: Uuid,
    kind: Kind,
    at_ms: u64,
    members: Vec<Uuid>,
}
impl Batch {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn device(&self) -> Uuid {
        self.device
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
}
pub enum Delivery {
    Submitted,
    Suppressed,
    Uncertain,
}
pub fn question_kind(text: &str) -> Option<Kind> {
    let text = text
        .trim()
        .trim_end_matches(['?', '.', '!'])
        .trim()
        .to_lowercase();
    let text = text
        .strip_prefix("avesra, ")
        .or_else(|| text.strip_prefix("avesra "))
        .unwrap_or(&text);
    match text {
        "what did you learn" => Some(Kind::Learning),
        "what did you just do" => Some(Kind::Action),
        _ => None,
    }
}
pub fn describe_last(kind: Kind, events: &[Event]) -> Result<String, ErrorCode> {
    if events.len() > 32 {
        return Err(ErrorCode::TooLarge);
    }
    if events.is_empty() {
        return Ok(match kind {
            Kind::Learning => "I have no recorded learning announcement.",
            Kind::Action => "I have no recorded action announcement.",
        }
        .into());
    }
    let mut text = String::from("My last submitted announcement covered: ");
    for (index, event) in events.iter().enumerate() {
        if event.kind != kind {
            return Err(ErrorCode::Malformed);
        }
        let mut member = String::new();
        match &event.description {
            Some(Description::Fact { value }) => {
                member.push_str("an explicitly saved fact: ");
                member.push_str(value);
            }
            Some(Description::RoutineCandidate { name }) => {
                member.push_str("an unvalidated routine candidate: ");
                member.push_str(name);
            }
            Some(Description::VerifiedAction { operation }) => member.push_str(operation),
            None => member.push_str("an event whose content has been deleted"),
        }
        if text.len() + member.len() > 6800 {
            text.push_str(&format!(
                "; and {} further recorded events. Inspect Settings for the remaining members",
                events.len() - index
            ));
            break;
        }
        if index > 0 {
            text.push_str("; ");
        }
        text.push_str(&member);
    }
    text.push('.');
    Ok(text)
}
fn sql<T>(value: rusqlite::Result<T>) -> Result<T, ErrorCode> {
    value.map_err(|_| ErrorCode::Storage)
}
fn encode(value: &impl Serialize) -> Result<String, ErrorCode> {
    serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)
}
fn clock(value: u64) -> Result<i64, ErrorCode> {
    i64::try_from(value).map_err(|_| ErrorCode::Expired)
}
struct Committed {
    id: Uuid,
    actor: Uuid,
    device: Uuid,
    kind: Kind,
    source: Uuid,
    revision: Uuid,
    at_ms: u64,
    description: Description,
}
fn capacity(tx: &Transaction<'_>, now: u64) -> Result<(), ErrorCode> {
    sql(tx.execute("INSERT INTO notification_capacity VALUES('global',1,?1) ON CONFLICT(id) DO UPDATE SET suppressed=MIN(suppressed,9223372036854775806)+1,at_ms=excluded.at_ms",[clock(now)?]))?;
    Ok(())
}
fn compact(tx: &Transaction<'_>, now: u64) -> Result<(), ErrorCode> {
    sql(tx.execute(
        "UPDATE notification_batches SET state='uncertain' WHERE state='pending' AND at_ms<?1",
        [clock(now.saturating_sub(10000))?],
    ))?;
    sql(tx.execute(
        "UPDATE notification_events SET settled=1 WHERE settled=0 AND at_ms<?1",
        [clock(now.saturating_sub(60000))?],
    ))?;
    sql(tx.execute("DELETE FROM notification_batches WHERE state!='pending' AND rowid NOT IN (SELECT rowid FROM notification_batches ORDER BY rowid DESC LIMIT 256) AND id NOT IN (SELECT id FROM (SELECT id,ROW_NUMBER() OVER(PARTITION BY actor,device,kind ORDER BY submitted_ms DESC,rowid DESC) AS rank FROM notification_batches WHERE state='submitted') WHERE rank=1)",[]))?;
    sql(tx.execute("DELETE FROM notification_events WHERE settled=1 AND rowid NOT IN (SELECT rowid FROM notification_events ORDER BY rowid DESC LIMIT 512) AND id NOT IN (SELECT value FROM notification_batches,json_each(notification_batches.members))",[]))?;
    Ok(())
}
fn insert(tx: &Transaction<'_>, event: Committed) -> Result<(), ErrorCode> {
    let Committed {
        id,
        actor,
        device,
        kind,
        source,
        revision,
        at_ms,
        description,
    } = event;
    if [id, actor, device, source, revision]
        .iter()
        .any(Uuid::is_nil)
    {
        return Err(ErrorCode::Malformed);
    }
    let body = encode(&description)?;
    if body.len() > 4096 {
        return Err(ErrorCode::TooLarge);
    }
    compact(tx, at_ms)?;
    let count: i64 =
        sql(tx.query_row("SELECT COUNT(*) FROM notification_events", [], |r| r.get(0)))?;
    if count >= 32768 {
        return capacity(tx, at_ms);
    }
    sql(tx.execute(
        "INSERT INTO notification_events VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0)",
        params![
            id.to_string(),
            actor.to_string(),
            device.to_string(),
            kind.name(),
            source.to_string(),
            revision.to_string(),
            clock(at_ms)?,
            body
        ],
    ))?;
    Ok(())
}
pub(crate) fn memory_committed(
    tx: &Transaction<'_>,
    entry: &crate::memory::Entry,
) -> Result<(), ErrorCode> {
    let turn = entry
        .source
        .as_ref()
        .map(|v| v.turn)
        .or_else(|| entry.accepted_source.as_ref().map(|v| v.turn))
        .ok_or(ErrorCode::Malformed)?;
    let device: String = sql(tx.query_row(
        "SELECT device FROM accepted_conversations WHERE id=?1 AND actor=?2",
        params![turn.to_string(), entry.actor.to_string()],
        |r| r.get(0),
    ))?;
    let description = match &entry.content {
        crate::memory::Content::NamedFact { key, value } => Description::Fact {
            value: format!("{key}: {value}"),
        },
        crate::memory::Content::Fact { value } => Description::Fact {
            value: value.clone(),
        },
        crate::memory::Content::Routine { name, .. } => {
            Description::RoutineCandidate { name: name.clone() }
        }
    };
    insert(
        tx,
        Committed {
            id: entry.revision,
            actor: entry.actor,
            device: Uuid::parse_str(&device).map_err(|_| ErrorCode::Malformed)?,
            kind: Kind::Learning,
            source: entry.id,
            revision: entry.revision,
            at_ms: entry.created_ms,
            description,
        },
    )
}
pub(crate) fn redact_memory(
    tx: &Transaction<'_>,
    actor: Uuid,
    memory: Uuid,
) -> Result<(), ErrorCode> {
    sql(tx.execute("UPDATE notification_events SET body=NULL,settled=1 WHERE actor=?1 AND source=?2 AND kind='learning'",params![actor.to_string(),memory.to_string()]))?;
    Ok(())
}
pub(crate) fn action_verified(
    tx: &Transaction<'_>,
    dispatch: Uuid,
    device: Uuid,
    action: &Action,
    now: u64,
) -> Result<(), ErrorCode> {
    // Deliberately excludes payload text, window titles, paths and diagnostics.
    let operation = match &action.payload {
        avesra_contracts::ActionPayload::LaunchApp { .. } => "Launched the requested application",
        avesra_contracts::ActionPayload::SetVolume { .. } => "Changed the requested output volume",
        avesra_contracts::ActionPayload::FillPrompt { .. } => "Filled the authorized prompt",
        avesra_contracts::ActionPayload::ReadPage { .. } => "Read the authorized page",
        _ => "Verified the requested action",
    };
    insert(
        tx,
        Committed {
            id: dispatch,
            actor: action.actor_id,
            device,
            kind: Kind::Action,
            source: action.task_id,
            revision: action.revision,
            at_ms: now,
            description: Description::VerifiedAction {
                operation: operation.into(),
            },
        },
    )
}
impl Store {
    /// Called on reconnect/control loss before considering new events. A former
    /// pending delivery is uncertain, never replayed as a fresh batch.
    pub fn suppress_notifications(
        &mut self,
        actor: Uuid,
        device: Uuid,
        before: u64,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        authorize()?;
        let tx = sql(self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate))?;
        sql(tx.execute(
            "UPDATE notification_events SET settled=1 WHERE actor=?1 AND device=?2 AND at_ms<=?3",
            params![actor.to_string(), device.to_string(), clock(before)?],
        ))?;
        sql(tx.execute("UPDATE notification_batches SET state='uncertain' WHERE actor=?1 AND device=?2 AND state='pending'",params![actor.to_string(),device.to_string()]))?;
        authorize()?;
        sql(tx.commit())
    }
    pub fn claim_notification(
        &mut self,
        actor: Uuid,
        device: Uuid,
        kind: Kind,
        since: u64,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Option<Batch>, ErrorCode> {
        if actor.is_nil() || device.is_nil() {
            return Err(ErrorCode::Unauthenticated);
        }
        authorize()?;
        let now = crate::execution::now_ms()?;
        let tx = sql(self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate))?;
        compact(&tx, now)?;
        let count: i64 = sql(
            tx.query_row("SELECT COUNT(*) FROM notification_batches", [], |r| {
                r.get(0)
            }),
        )?;
        if count >= 4096 {
            sql(tx.execute("UPDATE notification_events SET settled=1 WHERE actor=?1 AND device=?2 AND kind=?3 AND settled=0",params![actor.to_string(),device.to_string(),kind.name()]))?;
            capacity(&tx, now)?;
            authorize()?;
            sql(tx.commit())?;
            return Ok(None);
        }
        let members = {
            let mut q=sql(tx.prepare("SELECT id FROM notification_events WHERE actor=?1 AND device=?2 AND kind=?3 AND settled=0 AND body IS NOT NULL AND at_ms>=?4 AND at_ms<=?5 ORDER BY rowid LIMIT 32"))?;
            let values = sql(q.query_map(
                params![
                    actor.to_string(),
                    device.to_string(),
                    kind.name(),
                    clock(since)?,
                    clock(now)?
                ],
                |r| r.get::<_, String>(0),
            ))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ErrorCode::Storage)?;
            values
                .into_iter()
                .map(|v| Uuid::parse_str(&v).map_err(|_| ErrorCode::Malformed))
                .collect::<Result<Vec<_>, _>>()?
        };
        if members.is_empty() {
            authorize()?;
            sql(tx.commit())?;
            return Ok(None);
        }
        let batch = Batch {
            id: Uuid::new_v4(),
            actor,
            device,
            kind,
            at_ms: now,
            members,
        };
        sql(tx.execute(
            "INSERT INTO notification_batches VALUES(?1,?2,?3,?4,?5,?6,'pending',NULL)",
            params![
                batch.id.to_string(),
                actor.to_string(),
                device.to_string(),
                kind.name(),
                clock(now)?,
                encode(&batch.members)?
            ],
        ))?;
        for id in &batch.members {
            sql(tx.execute(
                "UPDATE notification_events SET settled=1 WHERE id=?1",
                [id.to_string()],
            ))?;
        }
        authorize()?;
        sql(tx.commit())?;
        Ok(Some(batch))
    }
    pub fn finish_notification(
        &mut self,
        batch: Batch,
        delivery: Delivery,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        authorize()?;
        let now = crate::execution::now_ms()?;
        let tx = sql(self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate))?;
        let matches:bool=sql(tx.query_row("SELECT EXISTS(SELECT 1 FROM notification_batches WHERE id=?1 AND actor=?2 AND device=?3 AND kind=?4 AND at_ms=?5 AND members=?6 AND state='pending')",params![batch.id.to_string(),batch.actor.to_string(),batch.device.to_string(),batch.kind.name(),clock(batch.at_ms)?,encode(&batch.members)?],|r|r.get(0)))?;
        if !matches {
            return Err(ErrorCode::Stale);
        }
        let mut retained = true;
        for id in &batch.members {
            retained &= sql(tx.query_row("SELECT body IS NOT NULL FROM notification_events WHERE id=?1 AND actor=?2 AND device=?3",params![id.to_string(),batch.actor.to_string(),batch.device.to_string()],|r|r.get::<_,bool>(0)))?;
        }
        let state = match delivery {
            Delivery::Submitted if retained && now >= batch.at_ms && now - batch.at_ms <= 10000 => {
                "submitted"
            }
            Delivery::Suppressed => "suppressed",
            _ => "uncertain",
        };
        sql(tx.execute(
            "UPDATE notification_batches SET state=?2,submitted_ms=?3 WHERE id=?1",
            params![
                batch.id.to_string(),
                state,
                if state == "submitted" {
                    Some(clock(now)?)
                } else {
                    None
                }
            ],
        ))?;
        authorize()?;
        sql(tx.commit())
    }
    pub fn recent_notifications(&self, actor: Uuid, device: Uuid) -> Result<Vec<Event>, ErrorCode> {
        let mut q=sql(self.connection.prepare("SELECT id,kind,at_ms,body FROM notification_events WHERE actor=?1 AND device=?2 ORDER BY rowid DESC LIMIT 64"))?;
        let rows = sql(
            q.query_map(params![actor.to_string(), device.to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            }),
        )?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ErrorCode::Storage)?;
        rows.into_iter()
            .map(|(id, kind, at_ms, body)| {
                Ok(Event {
                    id: Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?,
                    kind: match kind.as_str() {
                        "learning" => Kind::Learning,
                        "action" => Kind::Action,
                        _ => return Err(ErrorCode::Malformed),
                    },
                    at_ms: u64::try_from(at_ms).map_err(|_| ErrorCode::Malformed)?,
                    description: body
                        .map(|v| serde_json::from_str(&v).map_err(|_| ErrorCode::Malformed))
                        .transpose()?,
                })
            })
            .collect()
    }
    pub fn last_announced(
        &self,
        actor: Uuid,
        device: Uuid,
        kind: Kind,
    ) -> Result<Vec<Event>, ErrorCode> {
        Ok(announced(&self.connection, actor, device, kind)?.1)
    }
}
pub(crate) fn announced(
    db: &Connection,
    actor: Uuid,
    device: Uuid,
    kind: Kind,
) -> Result<(avesra_contracts::speech::Provenance, Vec<Event>), ErrorCode> {
    let record:Option<(String,String)>=sql(db.query_row("SELECT id,members FROM notification_batches WHERE actor=?1 AND device=?2 AND kind=?3 AND state='submitted' ORDER BY submitted_ms DESC,rowid DESC LIMIT 1",params![actor.to_string(),device.to_string(),kind.name()],|r|Ok((r.get(0)?,r.get(1)?))).optional())?;
    let event_kind = match kind {
        Kind::Learning => avesra_contracts::speech::EventKind::Learning,
        Kind::Action => avesra_contracts::speech::EventKind::Action,
    };
    let Some((batch, members)) = record else {
        return Ok((
            avesra_contracts::speech::Provenance::NativeEvents {
                event_kind,
                batch: None,
                events: Vec::new(),
            },
            Vec::new(),
        ));
    };
    let ids: Vec<Uuid> = serde_json::from_str(&members).map_err(|_| ErrorCode::Malformed)?;
    let provenance = avesra_contracts::speech::Provenance::NativeEvents {
        event_kind,
        batch: Some(Uuid::parse_str(&batch).map_err(|_| ErrorCode::Malformed)?),
        events: ids.clone(),
    };
    provenance.validate()?;
    let mut values = Vec::new();
    for id in ids {
        let (at_ms,body):(i64,Option<String>)=sql(db.query_row("SELECT at_ms,body FROM notification_events WHERE id=?1 AND actor=?2 AND device=?3 AND kind=?4",params![id.to_string(),actor.to_string(),device.to_string(),kind.name()],|r|Ok((r.get(0)?,r.get(1)?))))?;
        values.push(Event {
            id,
            kind,
            at_ms: u64::try_from(at_ms).map_err(|_| ErrorCode::Malformed)?,
            description: body
                .map(|v| serde_json::from_str(&v).map_err(|_| ErrorCode::Malformed))
                .transpose()?,
        });
    }
    Ok((provenance, values))
}

/// Revalidates content-only retrieval dependencies; never alters reply history.
pub(crate) fn answer_current(
    db: &Connection,
    actor: Uuid,
    device: Uuid,
    provenance: &avesra_contracts::speech::Provenance,
    answer: &str,
) -> Result<bool, ErrorCode> {
    use avesra_contracts::speech::{EventKind, Provenance};
    provenance.validate()?;
    let Provenance::NativeEvents {
        event_kind,
        batch,
        events,
    } = provenance
    else {
        return Err(ErrorCode::Malformed);
    };
    let kind = match event_kind {
        EventKind::Learning => Kind::Learning,
        EventKind::Action => Kind::Action,
    };
    let Some(batch) = batch else {
        return Ok(describe_last(kind, &[])? == answer);
    };
    let members: Option<String> = sql(db.query_row(
        "SELECT members FROM notification_batches WHERE id=?1 AND actor=?2 AND device=?3 AND kind=?4 AND state='submitted'",
        params![batch.to_string(), actor.to_string(), device.to_string(), kind.name()],
        |r| r.get(0),
    ).optional())?;
    let Some(members) = members else {
        return Ok(false);
    };
    let current: Vec<Uuid> = serde_json::from_str(&members).map_err(|_| ErrorCode::Malformed)?;
    if current != *events {
        return Ok(false);
    }
    let mut values = Vec::with_capacity(events.len());
    for id in events {
        let row: Option<(i64, Option<String>)> = sql(db.query_row(
            "SELECT at_ms,body FROM notification_events WHERE id=?1 AND actor=?2 AND device=?3 AND kind=?4",
            params![id.to_string(), actor.to_string(), device.to_string(), kind.name()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional())?;
        let Some((at_ms, Some(body))) = row else {
            return Ok(false);
        };
        values.push(Event {
            id: *id,
            kind,
            at_ms: u64::try_from(at_ms).map_err(|_| ErrorCode::Malformed)?,
            description: Some(serde_json::from_str(&body).map_err(|_| ErrorCode::Malformed)?),
        });
    }
    Ok(describe_last(kind, &values)? == answer)
}

pub(crate) fn demonstration_committed(
    tx: &Transaction<'_>,
    value: &crate::demonstration::Candidate,
) -> Result<(), ErrorCode> {
    insert(
        tx,
        Committed {
            id: value.revision,
            actor: value.actor,
            device: value.device,
            kind: Kind::Learning,
            source: value.id,
            revision: value.revision,
            at_ms: value.observed_ms,
            description: Description::RoutineCandidate {
                name: value.name.clone(),
            },
        },
    )
}
