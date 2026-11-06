//! The project's approval rules (`orchester_core::run::ProjectRule`): *Approve for every run of this project* adds
//! them, Project settings › Runs › Approved for every run lists and revokes them, and the runner reads them at each
//! request, so a rule a person revokes asks again from the next request on.

use orchester_core::{
  id::RunId,
  run::{ProjectRule, ProjectRuleId, ProjectScope, RuleScope},
  service::ServiceApproval,
  time::Timestamp,
};
use rusqlite::params;

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_body, decode_document, encode_json, parse},
};

const SCOPE: &str = "project approval rule scope";
/// A `start_qa_app` rule: the [`RuleScope`] `approval_rules` stores for the same call.
const QA_SCHEMA: u32 = 1;
/// A service's rule: its [`ServiceApproval`], with the names of its variables and never their values.
const SERVICE_SCHEMA: u32 = 2;

impl ProjectDb {
  /// The project's rules, oldest first, revoked ones included.
  pub fn project_approval_rules(&self) -> Result<Vec<ProjectRule>, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT id, scope, made_in, made_by, created_at, revoked_at, answered FROM project_approval_rules ORDER BY id",
    )?;
    let rows = query
      .query_map([], |row| {
        Ok(StoredRule {
          id: row.get(0)?,
          scope: row.get(1)?,
          made_in: row.get(2)?,
          made_by: row.get(3)?,
          created_at: row.get(4)?,
          revoked_at: row.get(5)?,
          answered: row.get(6)?,
        })
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(StoredRule::into_rule).collect()
  }

  /// Adds a rule of `scope`, made by answering request `made_by` of run `made_in`, as
  /// [`Self::add_project_approval_rules`] adds one.
  pub fn add_project_approval_rule(
    &mut self,
    scope: &ProjectScope,
    made: (RunId, &str),
    at: Timestamp,
  ) -> Result<ProjectRule, StoreError> {
    let mut added = self.add_project_approval_rules(std::slice::from_ref(scope), made, at)?;
    Ok(added.remove(0))
  }

  /// Adds a rule of each of `scopes`, made by answering request `made_by` of run `made_in`, and returns them in their
  /// order: one per service of a start. A scope while one is active adds nothing and returns that one; the checks and
  /// the inserts share one write transaction, so two answers at once never keep two, and a failure keeps none.
  pub fn add_project_approval_rules(
    &mut self,
    scopes: &[ProjectScope],
    (made_in, made_by): (RunId, &str),
    at: Timestamp,
  ) -> Result<Vec<ProjectRule>, StoreError> {
    self.with_write_transaction(
      |db| {
        let mut added = Vec::with_capacity(scopes.len());
        for scope in scopes {
          if let Some(active) = db
            .project_approval_rules()?
            .into_iter()
            .find(|rule| rule.is_active() && rule.scope == *scope)
          {
            added.push(active);
            continue;
          }
          db.connection.execute(
            "INSERT INTO project_approval_rules (scope, made_in, made_by, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![encode_scope(scope)?, made_in.to_string(), made_by, at.unix_millis()],
          )?;
          added.push(ProjectRule {
            id: ProjectRuleId::new(db.connection.last_insert_rowid()),
            scope: scope.clone(),
            made_in,
            made_by: made_by.to_owned(),
            created_at: at,
            revoked_at: None,
            answered: 0,
          });
        }
        Ok(added)
      },
      |error| error,
      // Nothing was committed: the transaction rolls back, and no rule was added.
      |_, error| error,
    )
  }

  /// Revokes rule `rule`; `false` when it was revoked already or is not stored.
  pub fn revoke_project_approval_rule(&mut self, rule: ProjectRuleId, at: Timestamp) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE project_approval_rules SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
      params![rule.get(), at.unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// Rule `rule` answered a request; recorded while the request is claimed.
  pub fn project_rule_answered(&mut self, rule: ProjectRuleId) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE project_approval_rules SET answered = answered + 1 WHERE id = ?1",
      [rule.get()],
    )?;
    Ok(())
  }
}

/// A rule as stored.
struct StoredRule {
  id: i64,
  scope: String,
  made_in: String,
  made_by: String,
  created_at: i64,
  revoked_at: Option<i64>,
  answered: u32,
}

impl StoredRule {
  fn into_rule(self) -> Result<ProjectRule, StoreError> {
    Ok(ProjectRule {
      id: ProjectRuleId::new(self.id),
      scope: decode_scope(&self.scope)?,
      made_in: parse("run id", &self.made_in)?,
      made_by: self.made_by,
      created_at: Timestamp::from_unix_millis(self.created_at),
      revoked_at: self.revoked_at.map(Timestamp::from_unix_millis),
      answered: self.answered,
    })
  }
}

/// `scope` as stored: a `start_qa_app` rule as its [`RuleScope`], a service's as its [`ServiceApproval`].
fn encode_scope(scope: &ProjectScope) -> Result<String, StoreError> {
  match (scope.service(), scope.rule_scope()) {
    (Some(service), _) => encode_json(SCOPE, SERVICE_SCHEMA, service),
    (None, Some(rule)) => encode_json(SCOPE, QA_SCHEMA, &rule),
    // Every scope is a service's or has a rule scope ([`ProjectScope::rule_scope`]).
    (None, None) => Err(StoreError::Unrepresentable(SCOPE)),
  }
}

fn decode_scope(text: &str) -> Result<ProjectScope, StoreError> {
  match decode_document(SCOPE, text)? {
    (QA_SCHEMA, body) => ProjectScope::stored(decode_body::<RuleScope>(SCOPE, body)?)
      .ok_or_else(|| corrupt(SCOPE, "not a rule of start_qa_app kept to a script")),
    (SERVICE_SCHEMA, body) => {
      let service = decode_body::<ServiceApproval>(SCOPE, body)?;
      if service.is_valid() {
        Ok(ProjectScope::Service(service))
      } else {
        Err(corrupt(SCOPE, "not a service's rule with a SHA-256 fingerprint"))
      }
    }
    (schema, _) => Err(corrupt(SCOPE, format!("schema {schema} is not a supported schema"))),
  }
}
