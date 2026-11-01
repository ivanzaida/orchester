//! What a run keeps of its approvals, as the task page and the status bar show it: whether Orchester
//! auto-approves its requests (the run's own choice — *Approve everything in this run*, the run's switch,
//! *Stop* — else whether a profile one of its steps pinned approves automatically, or the run is in Autopilot,
//! `orchester_core::run::auto_approves`), its mode, its rules from Approve similar and Always allow, and how its
//! requests were answered. The desktop writes the run's choice, switches it to Manual and revokes rules; the runner
//! reads them at each request, so a change applies from the next one.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  resource::{AgentProfile, ModelId},
  run::{
    Answerer, ApprovalRule, ApprovalRuleId, EffectiveMode, ManualSwitch, Run, StepExecutor, TaskPhase, auto_approves,
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
    profile_approves(db, run) || db.run_approval_mode(run.id())?.is_autopilot(),
  ))
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
  /// Switch the run to Manual for itself, for good: nothing switches it back to Autopilot. A narrowing, so the person
  /// or automation alike, as the switch records.
  SwitchToManual(Answerer),
}

/// Applies `change` to run `run` of the project at `repo_root`.
pub fn apply_in(repo_root: &Path, run: RunId, change: RunApprovalChange) -> Result<(), StoreError> {
  let mut db = ProjectDb::open(repo_root)?;
  match change {
    RunApprovalChange::AutoApprove(chosen) => db.set_run_auto_approve(run, Some(chosen)),
    // A rule revoked already, or one of Always allow, stays as it is.
    RunApprovalChange::Revoke(rule) => db.revoke_approval_rule(run, rule, Timestamp::now()).map(|_| ()),
    RunApprovalChange::SwitchToManual(by) => db.switch_run_to_manual(
      run,
      ManualSwitch {
        at: Timestamp::now(),
        by,
      },
    ),
  }
}

/// The reviewer Manual asks, as the app names it: the connection and the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewerShown {
  pub connection: Arc<str>,
  pub model: ModelId,
  pub effort: Option<String>,
}

/// The project's reviewer, or why it has none.
pub fn reviewer_of(db: &ProjectDb) -> Result<Result<ReviewerShown, ReviewerUnavailable>, StoreError> {
  let settings = db.auto_mode_settings()?;
  let connections = db.connections()?;
  Ok(
    resolve_reviewer(&settings, &connections, &[]).map(|resolved| ReviewerShown {
      connection: resolved.connection.name().as_str().into(),
      model: resolved.model,
      effort: resolved.effort,
    }),
  )
}

/// Why the project has no reviewer, as the app says it.
pub const fn unavailable_key(why: ReviewerUnavailable) -> &'static str {
  match why {
    ReviewerUnavailable::NoConnection => "runs.autoMode.unavailable.noConnection",
    ReviewerUnavailable::ConnectionGone => "runs.autoMode.unavailable.connectionGone",
    ReviewerUnavailable::Unsupported | ReviewerUnavailable::NotConnected => "runs.autoMode.unavailable.unsupported",
    ReviewerUnavailable::NoModel | ReviewerUnavailable::ModelGone => "runs.autoMode.unavailable.noModel",
  }
}

/// A run's approval mode, and the project's reviewer or why it has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunModeState {
  pub mode: EffectiveMode,
  pub reviewer: Result<ReviewerShown, ReviewerUnavailable>,
}

/// [`RunModeState`] of run `run` of the project at `repo_root`.
pub fn run_mode_in(repo_root: &Path, run: RunId) -> Result<RunModeState, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(RunModeState {
    mode: db.run_approval_mode(run)?,
    reviewer: reviewer_of(&db)?,
  })
}

/// What a run keeps of its approvals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunApprovals {
  /// The run's own auto-approval choice; `None` follows its profiles and its mode.
  pub chosen: Option<bool>,
  /// A profile one of its steps pinned approves automatically.
  pub profile: bool,
  /// The mode the run answers its agents' requests in now.
  pub mode: EffectiveMode,
  /// When and by whom the run was switched to Manual for itself, once it was and that was recorded.
  pub switched: Option<ManualSwitch>,
  pub rules: Vec<ApprovalRule>,
  pub counts: DecisionCounts,
}

impl RunApprovals {
  pub fn read(db: &ProjectDb, run: &Run) -> Result<Self, StoreError> {
    Ok(Self {
      chosen: db.run_auto_approve(run.id())?,
      profile: profile_approves(db, run),
      mode: db.run_approval_mode(run.id())?,
      switched: db.run_manual_switch(run.id())?,
      rules: db.approval_rules(run.id())?,
      counts: db.decision_counts(run.id())?,
    })
  }

  /// Whether the run may still be switched to Manual for itself: it is in Autopilot and has not ended.
  pub fn may_switch_to_manual(&self, run: &Run) -> bool {
    self.mode.is_autopilot() && !run.status().is_terminal()
  }

  /// Whether Orchester auto-approves the run's requests now.
  pub const fn auto_approving(&self) -> bool {
    auto_approves(self.chosen, self.profile || self.mode.is_autopilot())
  }

  /// Whether Autopilot answers the run's requests: the run is in it, and its own choice (*Stop*) did not take it back.
  pub fn approved_by_autopilot(&self) -> bool {
    self.mode.is_autopilot() && self.chosen != Some(false)
  }
}
