//! Explicit native owner permissions, stored by the existing ledger owner.
use crate::{apps::AppCatalog, policy::Grant, store::Store};
use avesra_contracts::{ErrorCode, Operation};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const SCHEMA: &str = "CREATE TABLE action_permissions(id TEXT PRIMARY KEY NOT NULL REFERENCES ledger_grants(id),actor TEXT NOT NULL,body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=8192))";
pub(crate) fn check_schema(db: &Connection, version: u64) -> Result<(), ErrorCode> {
    let count: i64 = db.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='action_permissions' OR tbl_name='action_permissions'", [], |r| r.get(0)).map_err(|_| ErrorCode::Storage)?;
    if version < 11 {
        return if count == 0 {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        };
    }
    let sql: String = db
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='action_permissions'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| ErrorCode::Malformed)?;
    let index: (String, i64, String, i64) = db
        .query_row(
            "SELECT name,\"unique\",origin,partial FROM pragma_index_list('action_permissions')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|_| ErrorCode::Malformed)?;
    let column: (i64, String) = db
        .query_row(
            "SELECT cid,name FROM pragma_index_info('sqlite_autoindex_action_permissions_1')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ErrorCode::Malformed)?;
    if count != 2
        || sql != SCHEMA
        || index
            != (
                "sqlite_autoindex_action_permissions_1".into(),
                1,
                "pk".into(),
                0,
            )
        || column != (0, "id".into())
    {
        return Err(ErrorCode::Malformed);
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskTarget {
    Vpn {
        profile: Box<crate::vpn::Profile>,
    },
    BrowserRead {
        scope: Box<crate::browser_scopes::Grant>,
    },
    Prompt {
        binding: Box<crate::workflows::PromptBinding>,
    },
    Diagnostic {
        catalog: crate::diagnostics::Catalog,
    },
    Application {
        app: Uuid,
        app_revision: Uuid,
        alias: Uuid,
        alias_revision: Uuid,
    },
    Volume {
        target: Uuid,
        endpoint: String,
    },
}
impl TaskTarget {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Vpn { profile } => profile.id,
            Self::BrowserRead { scope } => scope.id.uuid(),
            Self::Prompt { binding } => binding.id,
            Self::Diagnostic { catalog } => catalog.id(),
            Self::Application { app, .. } => *app,
            Self::Volume { target, .. } => *target,
        }
    }
    pub fn operation(&self) -> Operation {
        match self {
            Self::Vpn { .. } => Operation::ConnectVpn,
            Self::BrowserRead { .. } => Operation::ReadPage,
            Self::Prompt { .. } => Operation::FillPrompt,
            Self::Diagnostic { .. } => Operation::Diagnostic,
            Self::Application { .. } => Operation::LaunchApp,
            Self::Volume { .. } => Operation::SetVolume,
        }
    }
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let valid = match self {
            Self::Vpn { profile } => profile.validate().is_ok(),
            Self::BrowserRead { scope } => {
                scope.validate().is_ok()
                    && scope
                        .operations
                        .contains(&avesra_contracts::browser::ScopeOperation::Read)
            }
            Self::Prompt { binding } => binding.validate().is_ok(),
            Self::Diagnostic { .. } => true,
            Self::Application {
                app,
                app_revision,
                alias,
                alias_revision,
            } => ![*app, *app_revision, *alias, *alias_revision]
                .iter()
                .any(Uuid::is_nil),
            Self::Volume { target, endpoint } => {
                !target.is_nil()
                    && crate::state::valid_audio_device_id(endpoint)
                    && endpoint.starts_with("wasapi:")
                    && endpoint.len() <= 1024
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::Malformed)
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permission {
    pub id: Uuid,
    pub actor: Uuid,
    pub name: String,
    pub target: TaskTarget,
}
#[derive(Serialize)]
pub struct PermissionView {
    pub permission: Permission,
    pub revoked: bool,
}
impl Permission {
    fn validate(&self) -> Result<(), ErrorCode> {
        self.target.validate()?;
        if self.id.is_nil()
            || self.actor.is_nil()
            || matches!(&self.target,TaskTarget::Vpn{profile} if profile.actor!=self.actor || self.name!="work vpn")
            || crate::apps::alias_phrase(&self.name).ok().as_ref() != Some(&self.name)
            || matches!(&self.target, TaskTarget::BrowserRead { scope } if scope.actor.uuid()!=self.actor || self.name!="browser page")
            || matches!(self.target, TaskTarget::Volume { .. }) && self.name != "speakers"
            || matches!(self.target, TaskTarget::Diagnostic { catalog } if self.name != catalog.name())
            || matches!(&self.target,TaskTarget::Prompt{binding} if binding.actor!=self.actor || binding.project!=self.name)
        {
            return Err(ErrorCode::Malformed);
        }
        Ok(())
    }
}
pub enum Selection {
    Vpn {
        profile: Box<crate::vpn::Profile>,
    },
    BrowserRead {
        scope: Box<crate::browser_scopes::Grant>,
    },
    Prompt {
        binding: Box<crate::workflows::PromptBinding>,
    },
    Diagnostic {
        catalog: crate::diagnostics::Catalog,
    },
    Application {
        alias: Uuid,
        revision: Uuid,
    },
    Volume {
        endpoint: String,
    },
}

impl Store {
    pub fn action_permissions(
        &self,
        actor: Option<Uuid>,
    ) -> Result<Vec<PermissionView>, ErrorCode> {
        let total: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM action_permissions", [], |r| r.get(0))
            .map_err(|_| ErrorCode::Storage)?;
        if total > 128 {
            return Err(ErrorCode::TooLarge);
        }
        let mut query = self.connection.prepare("SELECT p.id,p.actor,substr(CAST(p.body AS BLOB),1,8193),substr(CAST(g.body AS BLOB),1,8193),g.revoked FROM action_permissions p JOIN ledger_grants g ON g.id=p.id ORDER BY p.rowid LIMIT 129").map_err(|_| ErrorCode::Storage)?;
        let mut rows = query.query([]).map_err(|_| ErrorCode::Storage)?;
        let mut result = Vec::new();
        let mut count = 0;
        while let Some(row) = rows.next().map_err(|_| ErrorCode::Storage)? {
            count += 1;
            if count > 128 {
                return Err(ErrorCode::TooLarge);
            }
            let (id, owner, body, grant, revoked): (String, String, Vec<u8>, Vec<u8>, bool) = (
                row.get(0).map_err(|_| ErrorCode::Malformed)?,
                row.get(1).map_err(|_| ErrorCode::Malformed)?,
                row.get(2).map_err(|_| ErrorCode::Malformed)?,
                row.get(3).map_err(|_| ErrorCode::Malformed)?,
                row.get(4).map_err(|_| ErrorCode::Malformed)?,
            );
            if body.len() > 8192 || grant.len() > 8192 {
                return Err(ErrorCode::Malformed);
            }
            let permission: Permission =
                serde_json::from_slice(&body).map_err(|_| ErrorCode::Malformed)?;
            let grant: Grant = serde_json::from_slice(&grant).map_err(|_| ErrorCode::Malformed)?;
            permission.validate()?;
            if id != permission.id.to_string()
                || owner != permission.actor.to_string()
                || grant.id != permission.id
                || grant.actor_id != permission.actor
                || grant.target_id != permission.target.id()
                || grant.operations != [permission.target.operation()]
            {
                return Err(ErrorCode::Malformed);
            }
            if actor.is_none_or(|actor| actor == permission.actor) {
                result.push(PermissionView {
                    permission,
                    revoked: revoked || grant.revoked,
                });
            }
        }
        if count != total {
            return Err(ErrorCode::Malformed);
        }
        Ok(result)
    }
    pub fn grant_action(
        &mut self,
        actor: Uuid,
        selection: Selection,
        apps: &AppCatalog,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<Permission, ErrorCode> {
        let existing = self.action_permissions(None)?;
        if actor.is_nil() || existing.len() >= 128 {
            return Err(ErrorCode::TooLarge);
        }
        let (name, target) = match selection {
            Selection::Vpn { profile } => {
                profile.validate()?;
                if profile.actor != actor {
                    return Err(ErrorCode::Denied);
                }
                ("work vpn".to_owned(), TaskTarget::Vpn { profile })
            }
            Selection::BrowserRead { scope } => {
                scope.validate()?;
                let app = apps.get(scope.browser_app.uuid())?;
                if scope.actor.uuid() != actor
                    || app.selected_by != actor
                    || app.revision != scope.browser_revision.uuid()
                    || !scope
                        .operations
                        .contains(&avesra_contracts::browser::ScopeOperation::Read)
                {
                    return Err(ErrorCode::Stale);
                }
                ("browser page".to_owned(), TaskTarget::BrowserRead { scope })
            }
            Selection::Prompt { binding } => {
                binding.validate()?;
                let current = apps.resolve(actor, &binding.app_name)?;
                if binding.actor != actor
                    || current.record.id != binding.app
                    || current.record.revision != binding.app_revision
                    || current.alias_id != binding.alias
                    || current.alias_revision != binding.alias_revision
                {
                    return Err(ErrorCode::Stale);
                }
                (binding.project.clone(), TaskTarget::Prompt { binding })
            }
            Selection::Diagnostic { catalog } => (
                catalog.name().to_owned(),
                TaskTarget::Diagnostic { catalog },
            ),
            Selection::Application { alias, revision } => {
                let saved = apps
                    .aliases()?
                    .into_iter()
                    .find(|v| v.id == alias && v.revision == revision)
                    .ok_or(ErrorCode::Stale)?;
                let resolved = apps.resolve(actor, &saved.phrase)?;
                if resolved.alias_id != alias || resolved.alias_revision != revision {
                    return Err(ErrorCode::Stale);
                }
                (
                    saved.phrase,
                    TaskTarget::Application {
                        app: resolved.record.id,
                        app_revision: resolved.record.revision,
                        alias,
                        alias_revision: revision,
                    },
                )
            }
            Selection::Volume { endpoint } => (
                "speakers".into(),
                TaskTarget::Volume {
                    target: Uuid::new_v4(),
                    endpoint,
                },
            ),
        };
        if existing.iter().any(|v| {
            !v.revoked
                && v.permission.actor == actor
                && (v.permission.target == target
                    || matches!((&v.permission.target,&target),(TaskTarget::Prompt{binding:a},TaskTarget::Prompt{binding:b}) if a.app==b.app && a.project==b.project)
                    || matches!(
                        (&v.permission.target, &target),
                        (TaskTarget::Vpn { .. }, TaskTarget::Vpn { .. })
                    )
                    || matches!(
                        (&v.permission.target, &target),
                        (TaskTarget::Volume { .. }, TaskTarget::Volume { .. })
                    ))
        }) {
            return Err(ErrorCode::Denied);
        }
        let permission = Permission {
            id: Uuid::new_v4(),
            actor,
            name,
            target,
        };
        permission.validate()?;
        let grant = Grant {
            id: permission.id,
            actor_id: actor,
            target_id: permission.target.id(),
            operations: vec![permission.target.operation()],
            revoked: false,
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        tx.execute(
            "INSERT INTO ledger_grants(id,actor_id,body,revoked) VALUES(?1,?2,?3,0)",
            params![
                grant.id.to_string(),
                actor.to_string(),
                serde_json::to_string(&grant).map_err(|_| ErrorCode::Malformed)?
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        tx.execute(
            "INSERT INTO action_permissions(id,actor,body) VALUES(?1,?2,?3)",
            params![
                permission.id.to_string(),
                actor.to_string(),
                serde_json::to_string(&permission).map_err(|_| ErrorCode::Malformed)?
            ],
        )
        .map_err(|_| ErrorCode::Storage)?;
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)?;
        Ok(permission)
    }
    pub fn revoke_action(
        &mut self,
        actor: Uuid,
        id: Uuid,
        authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<(), ErrorCode> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ErrorCode::Storage)?;
        let exists: Option<String> = tx
            .query_row(
                "SELECT id FROM action_permissions WHERE id=?1 AND actor=?2",
                params![id.to_string(), actor.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ErrorCode::Storage)?;
        if exists.is_none() {
            return Err(ErrorCode::Denied);
        }
        if tx
            .execute(
                "UPDATE ledger_grants SET revoked=1 WHERE id=?1 AND actor_id=?2",
                params![id.to_string(), actor.to_string()],
            )
            .map_err(|_| ErrorCode::Storage)?
            != 1
        {
            return Err(ErrorCode::Malformed);
        }
        authorize()?;
        tx.commit().map_err(|_| ErrorCode::Storage)
    }
}
