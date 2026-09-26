//! One durable planner claim per opaque accepted turn, with no replay handle recovery.
use super::{DurableTurn, Record, Store};
use crate::ledger::DispatchSession;
use avesra_contracts::{ErrorCode, actors::Binding, planner};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
const MAX_BODY: usize = 65_536;
pub(crate) const PLAN_SCHEMA: &str = "CREATE TABLE conversation_plans(turn TEXT PRIMARY KEY NOT NULL REFERENCES accepted_conversations(id),request TEXT UNIQUE NOT NULL,actor TEXT NOT NULL,body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=65536),state TEXT NOT NULL CHECK(state IN ('pending','replied','cancelled','suspended')))";
pub(crate) const REPLY_SCHEMA: &str = "CREATE TABLE conversation_replies(turn TEXT PRIMARY KEY NOT NULL REFERENCES conversation_plans(turn),revision TEXT UNIQUE NOT NULL,request TEXT UNIQUE NOT NULL REFERENCES conversation_plans(request),body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=65536))";
pub(crate) const SEQUENCE_SCHEMA: &str = "CREATE TABLE planner_claim_sequence(id INTEGER PRIMARY KEY CHECK(id=1),value INTEGER NOT NULL CHECK(value>=0 AND value<=9007199254740991))";
pub(super) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    let objects: i64 = db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='planner_claim_sequence' OR tbl_name='planner_claim_sequence'", [], |r| r.get(0)).map_err(|_| ErrorCode::Storage)?;
    if version < 17 {
        if objects != 0 {
            return Err(ErrorCode::Malformed);
        }
    } else {
        let sql: String = db.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='planner_claim_sequence'", [], |r| r.get(0)).map_err(|_| ErrorCode::Malformed)?;
        let (count, minimum, maximum): (i64, Option<i64>, Option<i64>) = db
            .query_row(
                "SELECT COUNT(*),MIN(value),MAX(value) FROM planner_claim_sequence WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| ErrorCode::Malformed)?;
        let rows: i64 = db
            .query_row("SELECT COUNT(*) FROM planner_claim_sequence", [], |r| {
                r.get(0)
            })
            .map_err(|_| ErrorCode::Malformed)?;
        if objects != 1
            || sql != SEQUENCE_SCHEMA
            || rows != 1
            || count != 1
            || minimum != maximum
            || !minimum.is_some_and(|v| (0..=9007199254740991).contains(&v))
        {
            return Err(ErrorCode::Malformed);
        }
    }
    for (table, sql, columns) in [
        (
            "conversation_plans",
            PLAN_SCHEMA,
            vec![(0i64, "turn", "pk"), (1, "request", "u")],
        ),
        (
            "conversation_replies",
            REPLY_SCHEMA,
            vec![(0, "turn", "pk"), (1, "revision", "u"), (2, "request", "u")],
        ),
    ] {
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name=?1 OR tbl_name=?1",
                [table],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Malformed)?;
        if version < 7 {
            if count != 0 {
                return Err(ErrorCode::Malformed);
            }
            continue;
        }
        if count != 1 + columns.len() as i64 {
            return Err(ErrorCode::Malformed);
        }
        let actual: String = db
            .query_row(
                "SELECT substr(sql,1,2049) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .map_err(|_| ErrorCode::Malformed)?;
        if actual != sql {
            return Err(ErrorCode::Malformed);
        }
        let mut statement = db
            .prepare(&format!("PRAGMA index_list('{table}')"))
            .map_err(|_| ErrorCode::Malformed)?;
        let mut rows = statement.query([]).map_err(|_| ErrorCode::Malformed)?;
        let mut seen = vec![false; columns.len()];
        while let Some(row) = rows.next().map_err(|_| ErrorCode::Malformed)? {
            let (name, unique, origin, partial): (String, i64, String, i64) = (
                row.get(1).map_err(|_| ErrorCode::Malformed)?,
                row.get(2).map_err(|_| ErrorCode::Malformed)?,
                row.get(3).map_err(|_| ErrorCode::Malformed)?,
                row.get(4).map_err(|_| ErrorCode::Malformed)?,
            );
            let index = (0..columns.len())
                .find(|i| name == format!("sqlite_autoindex_{table}_{}", i + 1))
                .ok_or(ErrorCode::Malformed)?;
            if seen[index] || unique != 1 || partial != 0 || origin != columns[index].2 {
                return Err(ErrorCode::Malformed);
            }
            seen[index] = true;
            let mut info = db
                .prepare(&format!("PRAGMA index_info('{name}')"))
                .map_err(|_| ErrorCode::Malformed)?;
            let mut values = info.query([]).map_err(|_| ErrorCode::Malformed)?;
            let value = values
                .next()
                .map_err(|_| ErrorCode::Malformed)?
                .ok_or(ErrorCode::Malformed)?;
            let actual: (i64, i64, String) = (
                value.get(0).map_err(|_| ErrorCode::Malformed)?,
                value.get(1).map_err(|_| ErrorCode::Malformed)?,
                value.get(2).map_err(|_| ErrorCode::Malformed)?,
            );
            if actual != (0, columns[index].0, columns[index].1.to_owned())
                || values.next().map_err(|_| ErrorCode::Malformed)?.is_some()
            {
                return Err(ErrorCode::Malformed);
            }
        }
        if seen.iter().any(|v| !v) {
            return Err(ErrorCode::Malformed);
        }
    }
    Ok(())
}
/// Withdrawal-only native handle; it cannot construct or revive acceptance.
#[derive(Clone)]
pub struct PlannerCancellation(Arc<AtomicBool>);
impl PlannerCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn check(&self) -> Result<(), ErrorCode> {
        if self.cancelled() {
            Err(ErrorCode::Stale)
        } else {
            Ok(())
        }
    }
}
pub struct PlannerRequest {
    turn: DurableTurn,
    session: DispatchSession,
    binding: Binding,
    started: Instant,
    cancellation: PlannerCancellation,
}
impl PlannerRequest {
    /// Shares the original native caller's withdrawal; it cannot create a turn.
    pub fn with_withdrawal(mut self, signal: Arc<AtomicBool>) -> Self {
        self.cancellation = PlannerCancellation(signal);
        self
    }
    pub fn action_epoch(&self) -> u64 {
        self.session.action_epoch
    }
    pub fn cancellation(&self) -> PlannerCancellation {
        self.cancellation.clone()
    }
    pub fn target(&self) -> super::CancellationTarget {
        super::CancellationTarget {
            actor: self.turn.actor,
            source: self.turn.source,
            id: self.turn.id,
            revision: self.turn.revision,
        }
    }

    pub fn new(
        turn: DurableTurn,
        session: DispatchSession,
        binding: Binding,
    ) -> Result<Self, ErrorCode> {
        binding.validate()?;
        if binding.revoked
            || !session.active
            || session.actor_id != turn.actor
            || session.device_id != turn.source.device
            || session.session_id != turn.source.session
            || session.capture_epoch == 0
            || session.action_epoch == 0
            || binding.actor != turn.actor
            || binding.device != turn.source.device
        {
            return Err(ErrorCode::Denied);
        }
        Ok(Self {
            turn,
            session,
            binding,
            started: Instant::now(),
            cancellation: PlannerCancellation(Arc::new(AtomicBool::new(false))),
        })
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    request: planner::Request,
    binding: Binding,
}
impl Plan {
    fn validate(&self) -> Result<(), ErrorCode> {
        // Historical records have no dialogue. This local read validation does
        // not relax the wire or construct a live claim from saved history.
        let mut request = self.request.clone();
        if matches!(request.version, 1..=3) {
            if request.context.ordinal != 0 || (request.version < 3 && !request.dialogue.is_empty())
            {
                return Err(ErrorCode::Malformed);
            }
            request.version = planner::VERSION;
            request.context.ordinal = 1;
        }
        request.validate()?;
        self.binding.validate()?;
        if self.binding.revoked
            || self.binding.device != self.request.context.device
            || self.binding.actor != self.request.context.actor
            || self.binding.registration_revision != self.request.context.registration_revision
            || self.request.remaining_ms != planner::MAX_BUDGET_MS
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
/// The callback must derive current native action/registration authority. These
/// read-only values are not credentials and cannot themselves authorize a claim.
pub struct PlannerAuthority<'a> {
    pub context: &'a planner::Context,
    pub binding: &'a Binding,
}
/// Withdrawal bookkeeping only; cannot reconstruct a claim or authorize a send.
#[derive(Clone)]
pub struct PlannerRetirement {
    plan: Plan,
}
impl PlannerRetirement {
    pub fn target(&self) -> super::CancellationTarget {
        let c = &self.plan.request.context;
        super::CancellationTarget {
            actor: c.actor,
            source: super::Source {
                device: c.device,
                session: c.session,
                utterance: c.utterance,
            },
            id: c.turn,
            revision: c.turn_revision,
        }
    }
}
/// No Deserialize/Clone; original Instant survives transport and worker queues.
pub struct PlannerClaim {
    completed: bool,
    plan: Plan,
    started: Instant,
    cancellation: PlannerCancellation,
}
impl Drop for PlannerClaim {
    fn drop(&mut self) {
        if !self.completed {
            self.cancellation.cancel();
        }
    }
}
impl PlannerClaim {
    pub fn target(&self) -> super::CancellationTarget {
        let c = self.context();
        super::CancellationTarget {
            actor: c.actor,
            source: super::Source {
                device: c.device,
                session: c.session,
                utterance: c.utterance,
            },
            id: c.turn,
            revision: c.turn_revision,
        }
    }

    pub fn retirement(&self) -> PlannerRetirement {
        PlannerRetirement {
            plan: self.plan.clone(),
        }
    }
    pub fn transport(&self) -> Result<planner::Request, ErrorCode> {
        let mut value = self.plan.request.clone();
        value.remaining_ms = self.remaining_ms()?;
        Ok(value)
    }
    pub fn remaining_ms(&self) -> Result<u64, ErrorCode> {
        self.cancellation.check()?;
        remaining(self.started)
    }
    pub fn cancellation(&self) -> PlannerCancellation {
        self.cancellation.clone()
    }
    pub fn binding(&self) -> &Binding {
        &self.plan.binding
    }
    pub fn context(&self) -> &planner::Context {
        &self.plan.request.context
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyRecord {
    revision: Uuid,
    reply: planner::Reply,
}
/// Only a successful current ledger commit constructs this normal-output source.
pub struct StoredReply {
    binding: Binding,
    publication: PlannerCancellation,
    revision: Uuid,
    reply: planner::Reply,
    task: Option<super::tasks::LinkedTask>,
    started: Instant,
}
impl Drop for StoredReply {
    fn drop(&mut self) {
        self.publication.cancel();
    }
}
impl StoredReply {
    pub fn proposed_task(&self) -> Option<&super::tasks::LinkedTask> {
        self.task.as_ref()
    }
    pub fn remaining_ms(&self) -> Result<u64, ErrorCode> {
        self.publication.check()?;
        remaining(self.started)
    }
    pub fn cancellation(&self) -> PlannerCancellation {
        self.publication.clone()
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn publication_current(&self) -> bool {
        !self.publication.cancelled()
    }

    pub fn revision(&self) -> Uuid {
        self.revision
    }
    pub fn context(&self) -> &planner::Context {
        &self.reply.context
    }
    pub fn response(&self) -> &planner::Response {
        &self.reply.response
    }
}
#[derive(Serialize)]
pub struct PlannerSummary {
    pub request: Uuid,
    pub state: String,
    pub reply_revision: Option<Uuid>,
}
fn remaining(started: Instant) -> Result<u64, ErrorCode> {
    let remaining = Duration::from_millis(planner::MAX_BUDGET_MS)
        .checked_sub(started.elapsed())
        .ok_or(ErrorCode::Expired)?;
    let millis = u64::try_from(remaining.as_millis()).map_err(|_| ErrorCode::Expired)?;
    if millis == 0 {
        Err(ErrorCode::Expired)
    } else {
        Ok(millis)
    }
}
fn encode(value: &impl Serialize) -> Result<String, ErrorCode> {
    let body = serde_json::to_string(value).map_err(|_| ErrorCode::Malformed)?;
    if body.len() > MAX_BODY {
        return Err(ErrorCode::TooLarge);
    }
    Ok(body)
}
fn matches_source(plan: &Plan, record: &Record) -> bool {
    let context = &plan.request.context;
    context.turn == record.id
        && context.turn_revision == record.revision
        && context.actor == record.actor
        && context.device == record.source.device
        && context.session == record.source.session
        && context.utterance == record.source.utterance
        && context.capture_epoch == record.capture_epoch
        && context.action_epoch == record.action_epoch
        && plan.request.text == record.text
}
fn read_plan(db: &Connection, record: &Record) -> Result<Option<(Plan, String)>, ErrorCode> {
    let row:Option<(String,String,Vec<u8>,String)>=db.query_row("SELECT substr(request,1,37),substr(actor,1,37),substr(CAST(body AS BLOB),1,65537),substr(state,1,32) FROM conversation_plans WHERE turn=?1",[record.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_|ErrorCode::Storage)?;
    let Some((request, actor, body, state)) = row else {
        return Ok(None);
    };
    if body.len() > MAX_BODY {
        return Err(ErrorCode::Malformed);
    }
    let plan: Plan = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    plan.validate()?;
    if !matches_source(&plan, record)
        || request != plan.request.context.request.to_string()
        || actor != record.actor.to_string()
        || !matches!(
            state.as_str(),
            "pending" | "replied" | "cancelled" | "suspended"
        )
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some((plan, state)))
}
fn read_reply(db: &Connection, plan: &Plan) -> Result<Option<ReplyRecord>, ErrorCode> {
    let row:Option<(String,String,Vec<u8>)>=db.query_row("SELECT substr(revision,1,37),substr(request,1,37),substr(CAST(body AS BLOB),1,65537) FROM conversation_replies WHERE turn=?1",[plan.request.context.turn.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_|ErrorCode::Storage)?;
    let Some((revision, request, body)) = row else {
        return Ok(None);
    };
    if body.len() > MAX_BODY {
        return Err(ErrorCode::Malformed);
    }
    let value: ReplyRecord = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
    if value.reply.version != plan.request.version {
        return Err(ErrorCode::Version);
    }
    let mut checked = value.reply.clone();
    if checked.version == 1 && matches!(checked.response, planner::Response::Proposal { .. }) {
        return Err(ErrorCode::Malformed);
    }
    let mut expected = plan.request.context.clone();
    if matches!(checked.version, 1..=3) {
        if checked.context.ordinal != 0 || expected.ordinal != 0 {
            return Err(ErrorCode::Malformed);
        }
        checked.context.ordinal = 1;
        expected.ordinal = 1;
        checked.version = planner::VERSION;
    }
    checked.validate(&expected)?;
    if value.revision.is_nil()
        || value.revision.to_string() != revision
        || request != plan.request.context.request.to_string()
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(value))
}
pub(super) fn summary(
    db: &Connection,
    record: &Record,
) -> Result<Option<PlannerSummary>, ErrorCode> {
    let Some((plan, state)) = read_plan(db, record)? else {
        return Ok(None);
    };
    let reply = read_reply(db, &plan)?;
    if (state == "replied") != reply.is_some() {
        return Err(ErrorCode::Malformed);
    }
    Ok(Some(PlannerSummary {
        request: plan.request.context.request,
        state,
        reply_revision: reply.map(|r| r.revision),
    }))
}
pub(super) fn cancel(db: &Connection, record: &Record) -> Result<(), ErrorCode> {
    if let Some((_, state)) = read_plan(db, record)?
        && matches!(state.as_str(), "pending" | "suspended" | "cancelled")
    {
        db.execute(
            "UPDATE conversation_plans SET state='cancelled' WHERE turn=?1",
            [record.id.to_string()],
        )
        .map_err(|_| ErrorCode::Storage)?;
    }
    Ok(())
}
/// Content-only context selected inside the same actual claim transaction.
/// No reader can turn these records into a live claim or output capability.
fn recent_dialogue(
    db: &Connection,
    current: &Record,
    binding: &Binding,
) -> Result<Vec<planner::DialoguePair>, ErrorCode> {
    let mut query = db.prepare(
        "SELECT substr(id,1,37) FROM accepted_conversations WHERE actor=?1 AND device=?2 AND session=?3 AND rowid<(SELECT rowid FROM accepted_conversations WHERE id=?4) ORDER BY rowid DESC LIMIT 16",
    ).map_err(|_| ErrorCode::Storage)?;
    let mut rows = query
        .query(params![
            current.actor.to_string(),
            current.source.device.to_string(),
            current.source.session.to_string(),
            current.id.to_string()
        ])
        .map_err(|_| ErrorCode::Storage)?;
    let mut pairs = Vec::new();
    let mut bytes = 0usize;
    while let Some(row) = rows.next().map_err(|_| ErrorCode::Storage)? {
        let id: String = row.get(0).map_err(|_| ErrorCode::Malformed)?;
        let id = Uuid::parse_str(&id).map_err(|_| ErrorCode::Malformed)?;
        let (record, state) = super::tasks::read_record(db, id)?;
        if record.actor != current.actor
            || record.source.device != current.source.device
            || record.source.session != current.source.session
            || record.action_epoch != current.action_epoch
        {
            break;
        }
        if !matches!(state.as_str(), "answered" | "waiting_input") {
            continue;
        }
        let Some((plan, plan_state)) = read_plan(db, &record)? else {
            continue;
        };
        if plan.binding != *binding {
            break;
        }
        if plan_state != "replied" {
            continue;
        }
        let reply = read_reply(db, &plan)?.ok_or(ErrorCode::Malformed)?;
        if matches!(reply.reply.response, planner::Response::Proposal { .. }) {
            continue;
        }
        let pair_bytes = record.text.len() + reply.reply.response.text().len();
        if bytes + pair_bytes > planner::MAX_DIALOGUE_BYTES {
            break;
        }
        bytes += pair_bytes;
        pairs.push(planner::DialoguePair {
            user: record.text,
            assistant: reply.reply.response,
        });
        if pairs.len() == planner::MAX_DIALOGUE_PAIRS {
            break;
        }
    }
    pairs.reverse();
    Ok(pairs)
}
impl Store {
    pub fn claim_planner(
        &mut self,
        request: PlannerRequest,
        authorize: &mut dyn FnMut(&PlannerAuthority<'_>) -> Result<(), ErrorCode>,
    ) -> Result<PlannerClaim, ErrorCode> {
        request.cancellation.check()?;
        remaining(request.started)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let (record, state) = super::tasks::read_record(&tx, request.turn.id)?;
        if state != "accepted"
            || record.revision != request.turn.revision
            || record.actor != request.turn.actor
            || record.source != request.turn.source
            || record.capture_epoch != request.session.capture_epoch
            || record.action_epoch != request.session.action_epoch
            || super::tasks::linked(&tx, &record)?.is_some()
            || read_plan(&tx, &record)?.is_some()
        {
            return Err(ErrorCode::Stale);
        }
        let ordinal: i64 = tx.query_row("UPDATE planner_claim_sequence SET value=value+1 WHERE id=1 AND value<9007199254740991 RETURNING value", [], |r| r.get(0)).optional().map_err(|_| ErrorCode::Storage)?.ok_or(ErrorCode::TooLarge)?;
        let ordinal = u64::try_from(ordinal).map_err(|_| ErrorCode::Malformed)?;
        let plan = Plan {
            request: planner::Request {
                version: planner::VERSION,
                context: planner::Context {
                    ordinal,
                    request: Uuid::new_v4(),
                    turn: record.id,
                    turn_revision: record.revision,
                    device: record.source.device,
                    actor: record.actor,
                    registration_revision: request.binding.registration_revision,
                    session: record.source.session,
                    utterance: record.source.utterance,
                    capture_epoch: record.capture_epoch,
                    action_epoch: record.action_epoch,
                },
                text: record.text.clone(),
                dialogue: recent_dialogue(&tx, &record, &request.binding)?,
                remaining_ms: planner::MAX_BUDGET_MS,
            },
            binding: request.binding,
        };
        plan.validate()?;
        tx.execute("INSERT INTO conversation_plans(turn,request,actor,body,state) VALUES(?1,?2,?3,?4,'pending')",params![record.id.to_string(),plan.request.context.request.to_string(),record.actor.to_string(),encode(&plan)?]).map_err(|_|ErrorCode::Storage)?;
        if read_plan(&tx, &record)?.as_ref() != Some(&(plan.clone(), "pending".to_owned())) {
            return Err(ErrorCode::Malformed);
        }
        if tx.execute("UPDATE accepted_conversations SET state='planning' WHERE id=?1 AND state='accepted'",[record.id.to_string()]).map_err(|_|ErrorCode::Storage)?!=1{return Err(ErrorCode::Stale);}
        request.cancellation.check()?;
        remaining(request.started)?;
        authorize(&PlannerAuthority {
            context: &plan.request.context,
            binding: &plan.binding,
        })?;
        request.cancellation.check()?;
        remaining(request.started)?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        if let Err(error) = request
            .cancellation
            .check()
            .and_then(|_| remaining(request.started).map(|_| ()))
        {
            self.retire_planner(PlannerRetirement { plan })?;
            return Err(error);
        }
        Ok(PlannerClaim {
            completed: false,
            plan,
            started: request.started,
            cancellation: request.cancellation,
        })
    }
    pub fn retire_planner(&mut self, retirement: PlannerRetirement) -> Result<(), ErrorCode> {
        retirement.plan.validate()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let (record, state) = super::tasks::read_record(&tx, retirement.plan.request.context.turn)?;
        let (plan, plan_state) = read_plan(&tx, &record)?.ok_or(ErrorCode::Stale)?;
        if plan != retirement.plan {
            return Err(ErrorCode::Stale);
        }
        let reply = read_reply(&tx, &plan)?;
        if plan_state == "replied" && reply.is_some() {
            return Ok(());
        }
        if reply.is_some() {
            return Err(ErrorCode::Malformed);
        }
        if matches!(plan_state.as_str(), "cancelled" | "suspended") {
            return Ok(());
        }
        if plan_state != "pending" || state != "planning" {
            return Err(ErrorCode::Stale);
        }
        if tx.execute("UPDATE conversation_plans SET state='suspended' WHERE turn=?1 AND state='pending'",[record.id.to_string()]).map_err(|_|ErrorCode::Storage)? != 1
            || tx.execute("UPDATE accepted_conversations SET state='suspended' WHERE id=?1 AND state='planning'",[record.id.to_string()]).map_err(|_|ErrorCode::Storage)? != 1 {
            return Err(ErrorCode::Stale);
        }
        if read_plan(&tx, &record)?.as_ref() != Some(&(plan, "suspended".to_owned()))
            || super::tasks::read_record(&tx, record.id)?.1 != "suspended"
        {
            return Err(ErrorCode::Malformed);
        }
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
    pub fn finish_planner(
        &mut self,
        mut claim: PlannerClaim,
        reply: planner::Reply,
        apps: &crate::apps::AppCatalog,
        authorize: &mut dyn FnMut(&PlannerAuthority<'_>) -> Result<(), ErrorCode>,
    ) -> Result<StoredReply, ErrorCode> {
        let result = self.finish_planner_inner(&claim, reply, apps, authorize);
        if result.is_err() {
            self.retire_planner(claim.retirement())?;
        } else {
            claim.completed = true;
        }
        result
    }
    fn finish_planner_inner(
        &mut self,
        claim: &PlannerClaim,
        reply: planner::Reply,
        apps: &crate::apps::AppCatalog,
        authorize: &mut dyn FnMut(&PlannerAuthority<'_>) -> Result<(), ErrorCode>,
    ) -> Result<StoredReply, ErrorCode> {
        claim.cancellation.check()?;
        remaining(claim.started)?;
        reply.validate(&claim.plan.request.context)?;
        let permissions = if matches!(reply.response, planner::Response::Proposal { .. }) {
            self.action_permissions(Some(reply.context.actor))?
        } else {
            Vec::new()
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let (record, state) = super::tasks::read_record(&tx, claim.plan.request.context.turn)?;
        let (plan, plan_state) = read_plan(&tx, &record)?.ok_or(ErrorCode::Stale)?;
        if state != "planning"
            || plan_state != "pending"
            || plan != claim.plan
            || super::tasks::linked(&tx, &record)?.is_some()
            || read_reply(&tx, &plan)?.is_some()
        {
            return Err(ErrorCode::Stale);
        }
        let result = ReplyRecord {
            revision: Uuid::new_v4(),
            reply,
        };
        tx.execute(
            "INSERT INTO conversation_replies(turn,revision,request,body) VALUES(?1,?2,?3,?4)",
            params![
                record.id.to_string(),
                result.revision.to_string(),
                plan.request.context.request.to_string(),
                encode(&result)?
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        let stored = read_reply(&tx, &plan)?.ok_or(ErrorCode::Malformed)?;
        if stored.revision != result.revision || stored.reply != result.reply {
            return Err(ErrorCode::Malformed);
        }
        tx.execute(
            "UPDATE conversation_plans SET state='replied' WHERE turn=?1 AND state='pending'",
            [record.id.to_string()],
        )
        .map_err(|_| ErrorCode::Storage)?;
        let task = match &result.reply.response {
            planner::Response::Proposal { action } => super::tasks::link_proposal(
                &tx,
                &record,
                action,
                &permissions,
                apps,
                claim.started + Duration::from_millis(planner::MAX_BUDGET_MS),
            )?,
            _ => None,
        };
        let next = match &result.reply.response {
            planner::Response::Answer { .. } => "answered",
            planner::Response::NeedsInput { .. } => "waiting_input",
            planner::Response::Proposal { .. } if task.is_some() => "planning",
            planner::Response::Proposal { .. } => "waiting_input",
        };
        tx.execute(
            "UPDATE accepted_conversations SET state=?1 WHERE id=?2 AND state='planning'",
            params![next, record.id.to_string()],
        )
        .map_err(|_| ErrorCode::Storage)?;
        claim.cancellation.check()?;
        remaining(claim.started)?;
        authorize(&PlannerAuthority {
            context: &plan.request.context,
            binding: &plan.binding,
        })?;
        claim.cancellation.check()?;
        remaining(claim.started)?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        claim.cancellation.check()?;
        remaining(claim.started)?;
        Ok(StoredReply {
            binding: claim.plan.binding.clone(),
            publication: claim.cancellation.clone(),
            revision: result.revision,
            reply: result.reply,
            task,
            started: claim.started,
        })
    }
}
