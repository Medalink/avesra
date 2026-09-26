//! Optional observation intent and durable no-recreation keys; never grants.
use super::*;
pub(crate) const TABLES: [(&str, &str); 2] = [
    (
        "passive_settings",
        "CREATE TABLE passive_settings(actor TEXT PRIMARY KEY NOT NULL,body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=1024))",
    ),
    (
        "passive_sources",
        "CREATE TABLE passive_sources(scope TEXT PRIMARY KEY NOT NULL REFERENCES observation_scopes(id),actor TEXT NOT NULL,candidate TEXT NOT NULL REFERENCES demonstration_candidates(id),source_revision TEXT NOT NULL)",
    ),
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub actor: Uuid,
    pub revision: Uuid,
    pub scope: Uuid,
    pub scope_revision: Uuid,
    pub enabled: bool,
}
impl Setting {
    fn validate(&self) -> Result<(), ErrorCode> {
        if [self.actor, self.revision, self.scope, self.scope_revision]
            .iter()
            .any(Uuid::is_nil)
        {
            Err(ErrorCode::Malformed)
        } else {
            Ok(())
        }
    }
}
pub struct Prepared {
    pub setting: Setting,
    pub setup: Setup,
}
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    for (name, schema) in TABLES {
        let mut q = db
            .prepare(
                "SELECT type,name,sql FROM sqlite_master WHERE tbl_name=?1 ORDER BY type LIMIT 3",
            )
            .map_err(|_| ErrorCode::Storage)?;
        let rows = q
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
        if version < 27 {
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
pub(crate) fn setting(db: &Connection, actor: Uuid) -> Result<Option<Setting>, ErrorCode> {
    let body: Option<Vec<u8>> = db
        .query_row(
            "SELECT substr(CAST(body AS BLOB),1,1025) FROM passive_settings WHERE actor=?1",
            [actor.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    body.map(|body| {
        let value: Setting = serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
        value.validate()?;
        if body.len() > 1024 || value.actor != actor {
            return Err(ErrorCode::Malformed);
        }
        Ok(value)
    })
    .transpose()
}
fn seen(db: &Connection, actor: Uuid, scope: Uuid) -> Result<bool, ErrorCode> {
    let owner: Option<String> = db
        .query_row(
            "SELECT actor FROM passive_sources WHERE scope=?1",
            [scope.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ErrorCode::Storage)?;
    if owner.as_ref().is_some_and(|v| *v != actor.to_string()) {
        return Err(ErrorCode::Malformed);
    }
    // Older deleted rows deliberately contain no scope/content. Do not guess
    // which source may be recreated: require explicit teaching for that owner.
    let erased: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM demonstration_candidates c WHERE actor=?1 AND body IS NULL AND NOT EXISTS(SELECT 1 FROM passive_sources p WHERE p.candidate=c.id))",
        [actor.to_string()], |r| r.get(0),
    ).map_err(|_| ErrorCode::Storage)?;
    Ok(owner.is_some() || erased)
}
pub(super) fn preserve_source(db: &Connection, candidate: &Candidate) -> Result<(), ErrorCode> {
    db.execute(
        "INSERT INTO passive_sources VALUES(?1,?2,?3,?4) ON CONFLICT(scope) DO NOTHING",
        params![
            candidate.scope.id.to_string(),
            candidate.actor.to_string(),
            candidate.id.to_string(),
            candidate.revision.to_string()
        ],
    )
    .map_err(|_| ErrorCode::Storage)?;
    Ok(())
}
pub(super) fn claim_source(
    db: &Connection,
    candidate: &Candidate,
    expected: &Setting,
) -> Result<(), ErrorCode> {
    let actual = setting(db, candidate.actor)?.ok_or(ErrorCode::Stale)?;
    if !actual.enabled
        || actual.revision != expected.revision
        || actual.scope != candidate.scope.id
        || actual.scope_revision != candidate.scope.revision
        || candidate.passive_revision != Some(actual.revision)
        || !matches!(candidate.source, SourceKind::PassiveObservedTransition)
        || seen(db, candidate.actor, actual.scope)?
    {
        return Err(ErrorCode::Stale);
    }
    db.execute(
        "INSERT INTO passive_sources VALUES(?1,?2,?3,?4)",
        params![
            actual.scope.to_string(),
            actual.actor.to_string(),
            candidate.id.to_string(),
            actual.revision.to_string()
        ],
    )
    .map_err(|_| ErrorCode::Storage)?;
    Ok(())
}
impl Store {
    pub fn set_passive_teaching(
        &mut self,
        actor: Uuid,
        scope: Uuid,
        revision: Uuid,
        enabled: bool,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        authorize()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let selected = scopes(&tx, actor)?
            .into_iter()
            .find(|s| s.id == scope && s.revision == revision)
            .ok_or(ErrorCode::Stale)?;
        if enabled && selected.excluded {
            return Err(ErrorCode::Denied);
        }
        let value = Setting {
            actor,
            revision: Uuid::new_v4(),
            scope,
            scope_revision: revision,
            enabled,
        };
        value.validate()?;
        tx.execute("INSERT INTO passive_settings VALUES(?1,?2) ON CONFLICT(actor) DO UPDATE SET body=excluded.body",params![actor.to_string(),json(&value)?]).map_err(|_|ErrorCode::Storage)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
    pub fn passive_teaching(
        &self,
        actor: Uuid,
        apps: &AppCatalog,
    ) -> Result<Option<Prepared>, ErrorCode> {
        let Some(setting) = setting(&self.connection, actor)? else {
            return Ok(None);
        };
        if !setting.enabled || seen(&self.connection, actor, setting.scope)? {
            return Ok(None);
        }
        if candidates(&self.connection, actor)?
            .iter()
            .any(|c| c.scope.id == setting.scope)
        {
            return Ok(None);
        }
        let setup = self.teaching_setup(actor, setting.scope, setting.scope_revision, apps)?;
        let name = alias_phrase(&format!("open {}", setup.scope.name))?;
        if routine(&self.connection, actor, &name)?.is_some()
            || crate::memory::routine(&self.connection, actor, &name)?.is_some()
        {
            return Ok(None);
        }
        Ok(Some(Prepared { setting, setup }))
    }
    pub fn save_passive_demonstration(
        &mut self,
        candidate: Candidate,
        setting: Setting,
        apps: &AppCatalog,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        self.save_demonstration_source(candidate, Some(setting), apps, authorize)
    }
}
