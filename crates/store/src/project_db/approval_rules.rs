//! What a run keeps of its wider answers: the rules of Approve similar and Always allow, its own
//! auto-approval choice, and how its requests were answered. The runner writes rules and reads them
//! at each request, so a rule the desktop revokes, or an auto-approval it stops, applies from the next
//! request on.

use orchester_core::{
  id::RunId,
  run::{ApprovalRule, ApprovalRuleId, DecidedBy, RuleScope},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const SCOPE: &str = "approval rule scope";
/// The stored form of [`RuleScope`].
const SCOPE_SCHEMA: u32 = 1;

/// How a run's requests were answered so far (the run's *Command decisions*), and how many wait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecisionCounts {
  /// Approved by a person, with any of the approving answers.
  pub by_you: u32,
  pub denied_by_you: u32,
  pub by_similar_rule: u32,
  pub always_allowed: u32,
  pub auto_approved: u32,
  pub waiting: u32,
}

impl ProjectDb {
  /// The rules of `run`, oldest first, revoked ones included.
  pub fn approval_rules(&self, run: RunId) -> Result<Vec<ApprovalRule>, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT id, scope, created_at, revoked_at, answered FROM approval_rules WHERE run_id = ?1 ORDER BY id",
    )?;
    let rows = query
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, i64>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, i64>(2)?,
          row.get::<_, Option<i64>>(3)?,
          row.get::<_, u32>(4)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, scope, created_at, revoked_at, answered)| {
        Ok(ApprovalRule {
          id: ApprovalRuleId::new(id),
          run,
          scope: decode_json::<RuleScope>(SCOPE, SCOPE_SCHEMA, &scope)?,
          created_at: Timestamp::from_unix_millis(created_at),
          revoked_at: revoked_at.map(Timestamp::from_unix_millis),
          answered,
        })
      })
      .collect()
  }

  /// Adds a rule to `run`. The same scope while one is active adds nothing and returns that one.
  pub fn add_approval_rule(
    &mut self,
    run: RunId,
    scope: &RuleScope,
    at: Timestamp,
  ) -> Result<ApprovalRule, StoreError> {
    if let Some(active) = self
      .approval_rules(run)?
      .into_iter()
      .find(|rule| rule.is_active() && rule.scope == *scope)
    {
      return Ok(active);
    }
    self.connection.execute(
      "INSERT INTO approval_rules (run_id, scope, created_at) VALUES (?1, ?2, ?3)",
      params![
        run.to_string(),
        encode_json(SCOPE, SCOPE_SCHEMA, scope)?,
        at.unix_millis()
      ],
    )?;
    Ok(ApprovalRule {
      id: ApprovalRuleId::new(self.connection.last_insert_rowid()),
      run,
      scope: scope.clone(),
      created_at: at,
      revoked_at: None,
      answered: 0,
    })
  }

  /// Revokes rule `rule` of `run`; `false` when it was revoked already, or is not a rule a person can
  /// revoke (Always allow lives in the profile).
  pub fn revoke_approval_rule(&mut self, run: RunId, rule: ApprovalRuleId, at: Timestamp) -> Result<bool, StoreError> {
    let revocable = self
      .approval_rules(run)?
      .into_iter()
      .any(|stored| stored.id == rule && stored.is_active() && stored.scope.revocable());
    if !revocable {
      return Ok(false);
    }
    let changed = self.connection.execute(
      "UPDATE approval_rules SET revoked_at = ?3 WHERE run_id = ?1 AND id = ?2 AND revoked_at IS NULL",
      params![run.to_string(), rule.get(), at.unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// Rule `rule` answered request `approval` of `run`; recorded while the request is claimed, before
  /// its delivery is, as a delivered request does not change any more.
  pub fn rule_answered(&mut self, run: RunId, rule: ApprovalRuleId, approval: &str) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "UPDATE approval_rules SET answered = answered + 1 WHERE run_id = ?1 AND id = ?2",
      params![run.to_string(), rule.get()],
    )?;
    transaction.execute(
      "UPDATE tool_approvals SET rule_id = ?3 WHERE run_id = ?1 AND id = ?2",
      params![run.to_string(), approval, rule.get()],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// `run`'s own auto-approval choice; `None` follows each asking profile.
  pub fn run_auto_approve(&self, run: RunId) -> Result<Option<bool>, StoreError> {
    let chosen: Option<Option<bool>> = self
      .connection
      .query_row(
        "SELECT auto_approve FROM runs WHERE id = ?1",
        [run.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    chosen.ok_or_else(|| corrupt("run", format!("{run} is not stored")))
  }

  /// Makes `chosen` `run`'s own auto-approval choice.
  pub fn set_run_auto_approve(&mut self, run: RunId, chosen: Option<bool>) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE runs SET auto_approve = ?2 WHERE id = ?1",
      params![run.to_string(), chosen],
    )?;
    Ok(())
  }

  /// How `run`'s requests were answered, by who answered them, and how many wait.
  pub fn decision_counts(&self, run: RunId) -> Result<DecisionCounts, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT state, decided_by, COUNT(*) FROM tool_approvals WHERE run_id = ?1 \
       AND state IN ('pending', 'approved', 'denied') GROUP BY state, decided_by",
    )?;
    let rows = query
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, Option<String>>(1)?,
          row.get::<_, u32>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let mut counts = DecisionCounts::default();
    for (state, decided_by, count) in rows {
      let decided_by = decided_by
        .as_deref()
        .map(|decided_by| parse::<DecidedBy>("approval answer", decided_by))
        .transpose()?;
      let slot = match (state.as_str(), decided_by) {
        ("pending", _) => &mut counts.waiting,
        ("denied", _) => &mut counts.denied_by_you,
        (_, Some(DecidedBy::SimilarRule)) => &mut counts.by_similar_rule,
        // The profile's allowed commands answer as Always allow's prefixes do.
        (_, Some(DecidedBy::AlwaysAllowed | DecidedBy::Permissions)) => &mut counts.always_allowed,
        (_, Some(DecidedBy::AutoApproved)) => &mut counts.auto_approved,
        _ => &mut counts.by_you,
      };
      *slot += count;
    }
    Ok(counts)
  }
}
