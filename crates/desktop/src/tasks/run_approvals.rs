//! What a run keeps of its approvals, as the task page and the status bar show it: whether Orchester
//! auto-approves its requests (the run's own choice — *Approve everything in this run*, the run's switch,
//! *Stop* — else whether a profile one of its steps pinned approves automatically,
//! `orchester_core::run::auto_approves`), its rules from Approve similar and Always allow, and how its
//! requests were answered. The desktop writes the run's choice and revokes rules; the runner reads both
//! at each request, so a change applies from the next one.

use std::path::Path;

use orchester_core::{
  id::RunId,
  resource::AgentProfile,
  run::{ApprovalRule, ApprovalRuleId, Run, StepExecutor, auto_approves},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{DecisionCounts, ProjectDb, StoreError};

/// Whether `run` auto-approves now.
pub fn auto_approving(db: &ProjectDb, run: &Run) -> Result<bool, StoreError> {
  Ok(auto_approves(db.run_auto_approve(run.id())?, profile_approves(db, run)))
}

/// Whether a profile version one of `run`'s steps pinned approves automatically. A version that can no
/// longer be read does not.
pub fn profile_approves(db: &ProjectDb, run: &Run) -> bool {
  run.snapshot().steps().iter().any(|step| match step.executor() {
    // `.ok()`: an unreadable version is taken as not approving automatically, which only asks more.
    StepExecutor::Agent { profile, .. } => db
      .version::<AgentProfile>(profile.resource, profile.number)
      .ok()
      .flatten()
      .is_some_and(|version| version.body().permissions().auto_approve()),
    StepExecutor::Tool { .. } => false,
  })
}

/// A person's change to what a run keeps of its approvals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunApprovalChange {
  /// Auto-approve the run's requests, or stop.
  AutoApprove(bool),
  /// Revoke a rule of Approve similar: its requests ask again from the next one.
  Revoke(ApprovalRuleId),
}

impl lurq::app::component::DevtoolsInspectable for RunApprovalChange {}

/// Applies `change` to run `run` of the project at `repo_root`.
pub fn apply_in(repo_root: &Path, run: RunId, change: RunApprovalChange) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  match change {
    RunApprovalChange::AutoApprove(chosen) => db.set_run_auto_approve(run, Some(chosen)),
    // A rule revoked already, or one of Always allow, stays as it is.
    RunApprovalChange::Revoke(rule) => db.revoke_approval_rule(run, rule, Timestamp::now()).map(|_| ()),
  }
}

/// What a run keeps of its approvals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunApprovals {
  /// The run's own auto-approval choice; `None` follows its profiles.
  pub chosen: Option<bool>,
  /// A profile one of its steps pinned approves automatically.
  pub profile: bool,
  pub rules: Vec<ApprovalRule>,
  pub counts: DecisionCounts,
}

impl RunApprovals {
  pub fn read(db: &ProjectDb, run: &Run) -> Result<Self, StoreError> {
    Ok(Self {
      chosen: db.run_auto_approve(run.id())?,
      profile: profile_approves(db, run),
      rules: db.approval_rules(run.id())?,
      counts: db.decision_counts(run.id())?,
    })
  }

  /// Whether Orchester auto-approves the run's requests now.
  pub const fn auto_approving(&self) -> bool {
    auto_approves(self.chosen, self.profile)
  }
}
