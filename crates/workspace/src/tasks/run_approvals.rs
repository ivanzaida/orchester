//! What a run keeps of its approvals, as the task page and the status bar show it: whether Orchester
//! auto-approves its requests (the run's own choice — *Approve everything in this run*, the run's switch,
//! *Stop* — else whether a profile one of its steps pinned approves automatically, or auto mode's *approve
//! everything in each run* covers it, `orchester_core::run::auto_approves`), its rules from Approve similar and
//! Always allow, and how its requests were answered. The desktop writes the run's choice and revokes rules; the
//! runner reads both at each request, so a change applies from the next one.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  resource::{AgentProfile, ModelId},
  run::{
    Answerer, ApprovalRule, ApprovalRuleId, Run, StepExecutor, TaskPhase, auto_approves,
    review::{ReviewerUnavailable, resolve_reviewer},
  },
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{DecisionCounts, ProjectDb, StoreError};

use super::detail::run_store_error;

/// Whether `run` auto-approves now.
pub fn auto_approving(db: &ProjectDb, run: &Run) -> Result<bool, StoreError> {
  Ok(auto_approves(
    db.run_auto_approve(run.id())?,
    profile_approves(db, run) || everything(db, run.id())?.is_some(),
  ))
}

/// Who turned on auto mode's *approve everything in each run*, while it covers `run`, which uses auto mode now.
pub fn everything(db: &ProjectDb, run: RunId) -> Result<Option<Answerer>, StoreError> {
  let setting = db.approve_everything()?;
  Ok(
    setting
      .covers(db.run_auto_mode(run)?)
      .then(|| setting.turned_on_by())
      .flatten(),
  )
}

/// Whether `latest`, the latest run of `task`, is active and auto-approves: a run the status bar counts as
/// auto-approving, which its *Stop* stops.
pub fn counts_as_auto_approving(db: &ProjectDb, task: &Task, latest: &Run) -> Result<bool, StoreError> {
  Ok(TaskPhase::of(task, Some(latest), None).is_active() && auto_approving(db, latest)?)
}

/// Stops every run of the project at `repo_root` that auto-approves now ([`counts_as_auto_approving`]), read
/// when it writes, in the transaction that writes: a run that began auto-approving after the status bar was read
/// stops too, and none begins between the reading and the write. The runs it stopped.
pub fn stop_auto_approving_in(repo_root: &Path) -> Result<Vec<RunId>, StoreError> {
  ProjectDb::open(repo_root)?.stop_auto_approving(&auto_approving_runs)
}

/// The latest run of each task of `db` that counts as auto-approving ([`counts_as_auto_approving`]).
fn auto_approving_runs(db: &ProjectDb) -> Result<Vec<RunId>, StoreError> {
  let mut runs = Vec::new();
  for task in db.tasks()? {
    let listed = db.runs_of_task(task.id()).map_err(run_store_error)?;
    // An unreadable latest run cannot be read as auto-approving; the bar does not count it either.
    if let Some(Ok(latest)) = listed.last()
      && counts_as_auto_approving(db, &task, latest)?
    {
      runs.push(latest.id());
    }
  }
  Ok(runs)
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
  /// Use auto mode for the run's requests, or stop.
  AutoMode(bool),
}

/// Applies `change` to run `run` of the project at `repo_root`.
pub fn apply_in(repo_root: &Path, run: RunId, change: RunApprovalChange) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  match change {
    RunApprovalChange::AutoApprove(chosen) => db.set_run_auto_approve(run, Some(chosen)),
    // A rule revoked already, or one of Always allow, stays as it is.
    RunApprovalChange::Revoke(rule) => db.revoke_approval_rule(run, rule, Timestamp::now()).map(|_| ()),
    RunApprovalChange::AutoMode(on) => db.set_run_auto_mode(run, on),
  }
}

/// The reviewer of a project's auto mode, as the app names it: the connection and the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewerShown {
  pub connection: Arc<str>,
  pub model: ModelId,
  pub effort: Option<String>,
}

/// The project's reviewer, or why auto mode has none.
pub fn reviewer_of(db: &ProjectDb) -> Result<Result<ReviewerShown, ReviewerUnavailable>, StoreError> {
  let settings = db.auto_mode_settings()?;
  let connections = db.connections()?;
  Ok(resolve_reviewer(&settings, &connections).map(|resolved| ReviewerShown {
    connection: resolved.connection.name().as_str().into(),
    model: resolved.model,
    effort: resolved.effort,
  }))
}

/// Why auto mode has no reviewer, as the app says it.
pub const fn unavailable_key(why: ReviewerUnavailable) -> &'static str {
  match why {
    ReviewerUnavailable::NoConnection => "runs.autoMode.unavailable.noConnection",
    ReviewerUnavailable::ConnectionGone => "runs.autoMode.unavailable.connectionGone",
    ReviewerUnavailable::Unsupported => "runs.autoMode.unavailable.unsupported",
    ReviewerUnavailable::NoModel => "runs.autoMode.unavailable.noModel",
  }
}

/// A run's auto mode, and the project's reviewer or why it has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoModeState {
  pub on: bool,
  pub reviewer: Result<ReviewerShown, ReviewerUnavailable>,
}

/// [`AutoModeState`] of run `run` of the project at `repo_root`.
pub fn auto_mode_in(repo_root: &Path, run: RunId) -> Result<AutoModeState, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(AutoModeState {
    on: db.run_auto_mode(run)?,
    reviewer: reviewer_of(&db)?,
  })
}

/// What a run keeps of its approvals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunApprovals {
  /// The run's own auto-approval choice; `None` follows its profiles and auto mode's setting.
  pub chosen: Option<bool>,
  /// A profile one of its steps pinned approves automatically.
  pub profile: bool,
  /// Who turned on auto mode's *approve everything in each run*, while it covers the run ([`everything`]).
  pub everything: Option<Answerer>,
  pub rules: Vec<ApprovalRule>,
  pub counts: DecisionCounts,
}

impl RunApprovals {
  pub fn read(db: &ProjectDb, run: &Run) -> Result<Self, StoreError> {
    Ok(Self {
      chosen: db.run_auto_approve(run.id())?,
      profile: profile_approves(db, run),
      everything: everything(db, run.id())?,
      rules: db.approval_rules(run.id())?,
      counts: db.decision_counts(run.id())?,
    })
  }

  /// Whether Orchester auto-approves the run's requests now.
  pub const fn auto_approving(&self) -> bool {
    auto_approves(self.chosen, self.profile || self.everything.is_some())
  }

  /// Who turned on auto mode's *approve everything in each run*, while it answers the run's requests: the run's own
  /// choice (*Stop*) did not take it back.
  pub fn approved_by_setting(&self) -> Option<Answerer> {
    self.everything.filter(|_| self.chosen != Some(false))
  }
}
