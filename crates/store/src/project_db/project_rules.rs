//! The project's approval rules (`orchester_core::run::ProjectRule`): *Approve for every run of this project* adds
//! one, Project settings › Auto mode lists and revokes them, and the runner reads them at each request, so a rule a
//! person revokes asks again from the next request on.

use orchester_core::{
  id::RunId,
  run::{ProjectRule, ProjectRuleId, ProjectScope, RuleScope},
  time::Timestamp,
};
use rusqlite::params;

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const SCOPE: &str = "project approval rule scope";
/// The stored form of [`RuleScope`], as `approval_rules` stores it.
const SCOPE_SCHEMA: u32 = 1;

impl ProjectDb {
  /// The project's rules, oldest first, revoked ones included.
  pub fn project_approval_rules(&self) -> Result<Vec<ProjectRule>, StoreError> {
    let mut query = self
      .connection
      .prepare("SELECT id, scope, made_in, created_at, revoked_at, answered FROM project_approval_rules ORDER BY id")?;
    let rows = query
      .query_map([], |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
          row.get::<_, Option<i64>>(4)?,
          row.get::<_, u32>(5)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, scope, made_in, created_at, revoked_at, answered)| {
        Ok(ProjectRule {
          id: ProjectRuleId::new(id),
          scope: ProjectScope::stored(decode_json::<RuleScope>(SCOPE, SCOPE_SCHEMA, &scope)?)
            .ok_or_else(|| corrupt(SCOPE, "not a rule of start_qa_app kept to a script"))?,
          made_in: parse("run id", &made_in)?,
          created_at: Timestamp::from_unix_millis(created_at),
          revoked_at: revoked_at.map(Timestamp::from_unix_millis),
          answered,
        })
      })
      .collect()
  }

  /// Adds a rule of `scope`, made by answering a request of run `made_in`. The same scope while one is active adds
  /// nothing and returns that one; the check and the insert share one write transaction, so two answers at once
  /// never keep two.
  pub fn add_project_approval_rule(
    &mut self,
    scope: &ProjectScope,
    made_in: RunId,
    at: Timestamp,
  ) -> Result<ProjectRule, StoreError> {
    self.with_write_transaction(
      |db| {
        if let Some(active) = db
          .project_approval_rules()?
          .into_iter()
          .find(|rule| rule.is_active() && rule.scope == *scope)
        {
          return Ok(active);
        }
        db.connection.execute(
          "INSERT INTO project_approval_rules (scope, made_in, created_at) VALUES (?1, ?2, ?3)",
          params![
            encode_json(SCOPE, SCOPE_SCHEMA, &scope.rule_scope())?,
            made_in.to_string(),
            at.unix_millis()
          ],
        )?;
        Ok(ProjectRule {
          id: ProjectRuleId::new(db.connection.last_insert_rowid()),
          scope: scope.clone(),
          made_in,
          created_at: at,
          revoked_at: None,
          answered: 0,
        })
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
