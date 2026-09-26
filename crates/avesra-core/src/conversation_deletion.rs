//! Explicit ordinary-history content removal. Tombstones never reconstruct authority.
use super::*;
use sha2::{Digest, Sha256};

pub(crate) const SCHEMA: &str = "CREATE TABLE conversation_deletions(turn TEXT PRIMARY KEY NOT NULL REFERENCES accepted_conversations(id),body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=4096))";
pub(crate) const INDEX: &str =
    "CREATE INDEX conversation_history_scope ON accepted_conversations(actor,device,session)";
const MAX_TURNS: usize = 128;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Tombstone {
    pub version: u16,
    pub selected: bool,
    pub created_ms: u64,
    pub deletion: Uuid,
    pub at_ms: u64,
    pub context: planner::Context,
    pub owner_revision: Uuid,
    pub reply_revision: Uuid,
}
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    for (name, expected, kind) in [
        ("conversation_deletions", SCHEMA, "table"),
        ("conversation_history_scope", INDEX, "index"),
    ] {
        let actual: Option<(String, String)> = db
            .query_row(
                "SELECT substr(sql,1,2049),substr(type,1,16) FROM sqlite_master WHERE name=?1",
                [name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| ErrorCode::Storage)?;
        if (version >= 29
            && actual.as_ref().map(|v| (v.0.as_str(), v.1.as_str())) != Some((expected, kind)))
            || (version < 29 && actual.is_some())
        {
            return Err(ErrorCode::Malformed);
        }
    }
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE tbl_name='conversation_deletions'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Storage)?;
    if version < 29 {
        return if count == 0 {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        };
    }
    if count != 2 {
        return Err(ErrorCode::Malformed);
    }
    let mut statement = db
        .prepare("PRAGMA index_list('conversation_deletions')")
        .map_err(|_| ErrorCode::Storage)?;
    let mut rows = statement.query([]).map_err(|_| ErrorCode::Storage)?;
    let row = rows
        .next()
        .map_err(|_| ErrorCode::Storage)?
        .ok_or(ErrorCode::Malformed)?;
    let value: (String, i64, String, i64) = (
        row.get(1).map_err(|_| ErrorCode::Malformed)?,
        row.get(2).map_err(|_| ErrorCode::Malformed)?,
        row.get(3).map_err(|_| ErrorCode::Malformed)?,
        row.get(4).map_err(|_| ErrorCode::Malformed)?,
    );
    if value
        != (
            "sqlite_autoindex_conversation_deletions_1".to_owned(),
            1,
            "pk".to_owned(),
            0,
        )
        || rows.next().map_err(|_| ErrorCode::Storage)?.is_some()
    {
        return Err(ErrorCode::Malformed);
    }
    for (query, columns) in [
        (
            "PRAGMA index_info('sqlite_autoindex_conversation_deletions_1')",
            vec![(0, "turn")],
        ),
        (
            "PRAGMA index_info('conversation_history_scope')",
            vec![(2, "actor"), (3, "device"), (4, "session")],
        ),
    ] {
        let mut statement = db.prepare(query).map_err(|_| ErrorCode::Storage)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Storage)?;
        for (sequence, (column, name)) in columns.into_iter().enumerate() {
            let row = rows
                .next()
                .map_err(|_| ErrorCode::Storage)?
                .ok_or(ErrorCode::Malformed)?;
            let actual: (i64, i64, String) = (
                row.get(0).map_err(|_| ErrorCode::Malformed)?,
                row.get(1).map_err(|_| ErrorCode::Malformed)?,
                row.get(2).map_err(|_| ErrorCode::Malformed)?,
            );
            if actual
                != (
                    i64::try_from(sequence).map_err(|_| ErrorCode::Malformed)?,
                    column,
                    name.to_owned(),
                )
            {
                return Err(ErrorCode::Malformed);
            }
        }
        if rows.next().map_err(|_| ErrorCode::Storage)?.is_some() {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
pub(crate) fn read(db: &Connection, turn: Uuid) -> Result<Option<Tombstone>, ErrorCode> {
    let body: Option<Vec<u8>> = db
        .query_row(
            "SELECT substr(CAST(body AS BLOB),1,4097) FROM conversation_deletions WHERE turn=?1",
            [turn.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    let Some(body) = body else { return Ok(None) };
    if body.len() > 4096 {
        return Err(ErrorCode::Malformed);
    }
    let value: Tombstone = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    value.context.validate()?;
    if value.version != 1
        || value.created_ms == 0
        || value.created_ms > avesra_contracts::browser::MAX_SAFE_COUNTER
        || value.context.turn != turn
        || value.deletion.is_nil()
        || value.owner_revision.is_nil()
        || value.reply_revision.is_nil()
        || value.at_ms == 0
    {
        return Err(ErrorCode::Malformed);
    }
    let c = &value.context;
    let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM accepted_conversations a JOIN conversation_plans p ON p.turn=a.id JOIN conversation_replies r ON r.turn=a.id WHERE a.id=?1 AND a.revision=?2 AND a.actor=?3 AND a.device=?4 AND a.session=?5 AND a.utterance=?6 AND p.request=?7 AND p.actor=a.actor AND r.request=p.request AND r.revision=?8 AND p.state='replied' AND a.state IN ('answered','waiting_input','suspended') AND p.body='{}' AND r.body='{}' AND (?9=0 OR a.body='{}'))",params![turn.to_string(),c.turn_revision.to_string(),c.actor.to_string(),c.device.to_string(),c.session.to_string(),c.utterance.to_string(),c.request.to_string(),value.reply_revision.to_string(),value.selected],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
    if !valid {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(value))
}
pub(crate) fn require_source(db: &Connection, turn: Uuid) -> Result<(), ErrorCode> {
    if read(db, turn)?.is_some() {
        return Err(ErrorCode::Stale);
    }
    Ok(())
}
#[derive(Clone)]
pub struct Selection {
    pub current: planner::retirement::Current,
    pub owner_revision: Uuid,
    pub turn: Uuid,
    pub revision: Uuid,
}
#[derive(Clone, Serialize)]
pub struct Affected {
    pub response_already_deleted: bool,
    pub turn: Uuid,
    pub revision: Uuid,
    pub created_ms: u64,
}
#[derive(Clone)]
pub struct Prepared {
    selection: Selection,
    high: i64,
    digest: [u8; 32],
    contexts: Vec<planner::Context>,
    affected: Vec<Affected>,
}
impl Prepared {
    pub fn contexts(&self) -> &[planner::Context] {
        &self.contexts
    }
    pub fn affected(&self) -> &[Affected] {
        &self.affected
    }
    pub fn selected(&self) -> Uuid {
        self.selection.turn
    }
}
#[derive(Serialize)]
pub struct Deleted {
    pub deletion: Uuid,
    pub selected: Uuid,
    pub responses_removed: usize,
    pub wal_truncated: bool,
}
fn prepare(
    db: &Connection,
    selection: Selection,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<Prepared, ErrorCode> {
    authorize()?;
    if selection.turn.is_nil() || selection.revision.is_nil() || selection.owner_revision.is_nil() {
        return Err(ErrorCode::Malformed);
    }
    let (selected, _) = super::super::tasks::history_record(db, selection.turn)?;
    let c = &selection.current;
    if selected.revision != selection.revision
        || selected.actor != c.actor
        || selected.source.device != c.device
        || selected.source.session != c.session
    {
        return Err(ErrorCode::Denied);
    }
    let first: i64 = db
        .query_row(
            "SELECT rowid FROM accepted_conversations WHERE id=?1",
            [selection.turn.to_string()],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Stale)?;
    let high:i64=db.query_row("SELECT COALESCE(MAX(rowid),0) FROM accepted_conversations WHERE actor=?1 AND device=?2 AND session=?3",params![c.actor.to_string(),c.device.to_string(),c.session.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
    let mut statement=db.prepare("SELECT id FROM accepted_conversations WHERE actor=?1 AND device=?2 AND session=?3 AND rowid>=?4 AND rowid<=?5 ORDER BY rowid LIMIT 129").map_err(|_|ErrorCode::Storage)?;
    let ids = statement
        .query_map(
            params![
                c.actor.to_string(),
                c.device.to_string(),
                c.session.to_string(),
                first,
                high
            ],
            |r| r.get::<_, String>(0),
        )
        .map_err(|_| ErrorCode::Storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ErrorCode::Storage)?;
    if ids.is_empty() || ids.len() > MAX_TURNS {
        return Err(ErrorCode::TooLarge);
    }
    let mut digest = Sha256::new();
    digest.update(high.to_le_bytes());
    let mut bytes = 0usize;
    let mut contexts = Vec::new();
    let mut affected = Vec::new();
    // This conservative suffix is valid only while recent_dialogue is scoped to
    // the same actor/device/session. Broader retrieval requires broader deletion.
    for id in ids {
        authorize()?;
        let id = Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?;
        let prior = read(db, id)?;
        let (context, created_ms) = if let Some(prior) = &prior {
            if !prior.selected {
                let (record, _) = super::super::tasks::history_record(db, id)?;
                if !record_matches(&record, &prior.context) || record.created_ms != prior.created_ms
                {
                    return Err(ErrorCode::Malformed);
                }
            }
            if prior.owner_revision != selection.owner_revision {
                return Err(ErrorCode::Denied);
            }
            (prior.context.clone(), prior.created_ms)
        } else {
            let (record, state) = super::super::tasks::read_record(db, id)?;
            if !matches!(state.as_str(), "answered" | "waiting_input")
                || crate::memory::conversation::parse(&record.text).is_some()
                || crate::notifications::question_kind(&record.text).is_some()
                || crate::clock::question_kind(&record.text).is_some()
            {
                return Err(ErrorCode::Unsupported);
            }
            let (plan, plan_state) = read_plan(db, &record)?.ok_or(ErrorCode::Unsupported)?;
            let reply = read_reply(db, &plan)?.ok_or(ErrorCode::Unsupported)?;
            if plan_state != "replied"
                || !matches!(reply.provenance, Provenance::Model)
                || matches!(reply.reply.response, planner::Response::Proposal { .. })
                || plan.binding.owner_revision != selection.owner_revision
            {
                return Err(ErrorCode::Unsupported);
            }
            (plan.request.context, record.created_ms)
        };
        let dependencies:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_tasks WHERE turn=?1) OR EXISTS(SELECT 1 FROM private_memories WHERE source_turn=?1)",[id.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
        if dependencies {
            return Err(ErrorCode::Unsupported);
        }
        if context.actor != c.actor
            || context.device != c.device
            || context.session != c.session
            || context.registration_revision != c.registration_revision
            || context.action_epoch > c.action_epoch
        {
            return Err(ErrorCode::Unsupported);
        }
        context.validate()?;
        let bodies:(Vec<u8>,Vec<u8>,Vec<u8>)=db.query_row("SELECT substr(CAST(a.body AS BLOB),1,65537),substr(CAST(p.body AS BLOB),1,65537),substr(CAST(r.body AS BLOB),1,65537) FROM accepted_conversations a JOIN conversation_plans p ON p.turn=a.id JOIN conversation_replies r ON r.turn=a.id WHERE a.id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|ErrorCode::Storage)?;
        for body in [&bodies.0, &bodies.1, &bodies.2] {
            if body.len() > MAX_BODY {
                return Err(ErrorCode::TooLarge);
            }
            bytes = bytes.checked_add(body.len()).ok_or(ErrorCode::TooLarge)?;
            if bytes > MAX_BYTES {
                return Err(ErrorCode::TooLarge);
            }
            digest.update((body.len() as u64).to_le_bytes());
            digest.update(body);
        }
        if let Some(prior) = &prior {
            let body:Vec<u8>=db.query_row("SELECT substr(CAST(body AS BLOB),1,4097) FROM conversation_deletions WHERE turn=?1",[prior.context.turn.to_string()],|r|r.get(0)).map_err(|_|ErrorCode::Storage)?;
            if body.len() > 4096 {
                return Err(ErrorCode::Malformed);
            }
            bytes = bytes.checked_add(body.len()).ok_or(ErrorCode::TooLarge)?;
            if bytes > MAX_BYTES {
                return Err(ErrorCode::TooLarge);
            }
            digest.update(body);
        }
        let state: String = db
            .query_row(
                "SELECT substr(state,1,32) FROM accepted_conversations WHERE id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Storage)?;
        digest.update(state.as_bytes());
        contexts.push(context.clone());
        affected.push(Affected {
            response_already_deleted: prior.is_some(),
            turn: id,
            revision: context.turn_revision,
            created_ms,
        });
    }
    planner::retirement::Request {
        version: planner::retirement::VERSION,
        request: Uuid::new_v4(),
        current: selection.current.clone(),
        contexts: contexts.clone(),
        remaining_ms: 1,
    }
    .validate()?;
    authorize()?;
    Ok(Prepared {
        selection,
        high,
        digest: digest.finalize().into(),
        contexts,
        affected,
    })
}
impl Store {
    pub fn prepare_history_deletion(
        &self,
        selection: Selection,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Prepared, ErrorCode> {
        prepare(&self.connection, selection, authorize)
    }
    pub fn delete_conversation_content(
        &mut self,
        prepared: &Prepared,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Deleted, ErrorCode> {
        authorize()?;
        self.connection
            .pragma_update(None, "secure_delete", true)
            .map_err(|_| ErrorCode::Storage)?;
        if self
            .connection
            .pragma_query_value(None, "secure_delete", |r| r.get::<_, i64>(0))
            .map_err(|_| ErrorCode::Storage)?
            != 1
        {
            return Err(ErrorCode::Storage);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let fresh = prepare(&tx, prepared.selection.clone(), authorize)?;
        if fresh.high != prepared.high
            || fresh.digest != prepared.digest
            || fresh.contexts != prepared.contexts
        {
            return Err(ErrorCode::Stale);
        }
        let deletion = Uuid::new_v4();
        let at_ms = crate::execution::now_ms()?;
        let mut responses_removed = 0;
        for context in &prepared.contexts {
            if let Some(mut prior) = read(&tx, context.turn)? {
                if context.turn == prepared.selected() && !prior.selected {
                    prior.selected = true;
                    prior.deletion = deletion;
                    prior.at_ms = at_ms;
                    let body = serde_json::to_string(&prior).map_err(|_| ErrorCode::Malformed)?;
                    tx.execute(
                        "UPDATE conversation_deletions SET body=?2 WHERE turn=?1",
                        params![context.turn.to_string(), body],
                    )
                    .map_err(|_| ErrorCode::Storage)?;
                    tx.execute(
                        "UPDATE accepted_conversations SET body='{}' WHERE id=?1",
                        [context.turn.to_string()],
                    )
                    .map_err(|_| ErrorCode::Storage)?;
                }
                continue;
            }
            responses_removed += 1;
            let revision: String = tx
                .query_row(
                    "SELECT revision FROM conversation_replies WHERE turn=?1",
                    [context.turn.to_string()],
                    |r| r.get(0),
                )
                .map_err(|_| ErrorCode::Storage)?;
            let value = Tombstone {
                version: 1,
                created_ms: prepared
                    .affected
                    .iter()
                    .find(|v| v.turn == context.turn)
                    .ok_or(ErrorCode::Malformed)?
                    .created_ms,
                selected: context.turn == prepared.selected(),
                deletion,
                at_ms,
                context: context.clone(),
                owner_revision: prepared.selection.owner_revision,
                reply_revision: Uuid::parse_str(&revision).map_err(|_| ErrorCode::Malformed)?,
            };
            let body = serde_json::to_string(&value).map_err(|_| ErrorCode::Malformed)?;
            tx.execute(
                "INSERT INTO conversation_deletions(turn,body) VALUES(?1,?2)",
                params![context.turn.to_string(), body],
            )
            .map_err(|_| ErrorCode::Storage)?;
            tx.execute(
                "UPDATE conversation_plans SET body='{}' WHERE turn=?1",
                [context.turn.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
            tx.execute(
                "UPDATE conversation_replies SET body='{}' WHERE turn=?1",
                [context.turn.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?;
            if value.selected {
                tx.execute(
                    "UPDATE accepted_conversations SET body='{}' WHERE id=?1",
                    [context.turn.to_string()],
                )
                .map_err(|_| ErrorCode::Storage)?;
            }
        }
        for context in &prepared.contexts {
            super::super::search::refresh(&tx, context.turn)?;
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        // Logical deletion is committed. Never roll it back or report that a
        // withdrawn HTTP/UI waiter made the content reappear.
        let cleanup_ready = self.connection.busy_timeout(Duration::ZERO).is_ok();
        let wal_truncated = cleanup_ready
            && self
                .connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                    r.get::<_, i64>(0)
                })
                .is_ok_and(|v| v == 0);
        let _ = self.connection.busy_timeout(Duration::from_secs(5));
        Ok(Deleted {
            deletion,
            selected: prepared.selected(),
            responses_removed,
            wal_truncated,
        })
    }
}

fn record_matches(record: &Record, context: &planner::Context) -> bool {
    record.id == context.turn
        && record.revision == context.turn_revision
        && record.actor == context.actor
        && record.source.device == context.device
        && record.source.session == context.session
        && record.source.utterance == context.utterance
        && record.capture_epoch == context.capture_epoch
        && record.action_epoch == context.action_epoch
}
