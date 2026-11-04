//! What a run keeps of its approvals, as the task page and the status bar show it: whether Orchester
//! auto-approves its requests (the run's own choice — *Approve everything in this run*, the run's switch,
//! *Stop* — else whether the run is in Autopilot, `orchester_core::run::auto_approves`), its mode, its rules from
//! Approve similar and Always allow, and how its requests were answered. The desktop writes the run's choice,
//! switches it to Manual and revokes rules; the runner reads them at each request, so a change applies from the next
//! one. A change from the run's permissions menu
//! ([`change_permissions_in`]) is recorded as a permission event for its Conversation, and so is automation's refused
//! asking to widen the run.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  resource::ModelId,
  run::{
    AllowAutopilot, Answerer, ApprovalRule, ApprovalRuleId, EffectiveMode, ManualSwitch, PermissionEvent,
    PermissionEventKind, Run, RunPermissionChange, TaskPhase, WideningPersonOnly, auto_approves,
    review::{ReviewerUnavailable, resolve_reviewer},
  },
  store::{ConnectionStore, RunStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{DecisionCounts, ProjectDb, StoreError};
use thiserror::Error;

use super::detail::run_store_error;

/// Whether `run` auto-approves now.
pub fn auto_approving(db: &ProjectDb, run: &Run) -> Result<bool, StoreError> {
  Ok(auto_approves(
    db.run_auto_approve(run.id())?,
    db.run_approval_mode(run.id())?.is_autopilot(),
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

/// Why a change from a run's permissions menu was not made.
#[derive(Debug, Error)]
pub enum PermissionChangeError {
  /// Automation asked to widen the run: refused, and the refusal recorded.
  #[error(transparent)]
  PersonOnly(#[from] WideningPersonOnly),
  #[error(transparent)]
  Store(#[from] StoreError),
}

/// Applies `change`, asked `by` a person or automation from the permissions menu of run `run` of the project at
/// `repo_root`, and records it for the run's Conversation; a change that changes nothing records nothing. Widening is
/// a person's alone: automation's asking for it is refused, and the refusal is recorded.
pub fn change_permissions_in(
  repo_root: &Path,
  run: RunId,
  change: RunPermissionChange,
  by: Answerer,
) -> Result<(), PermissionChangeError> {
  let mut db = ProjectDb::open(repo_root)?;
  let at = Timestamp::now();
  let event = |kind| PermissionEvent { at, by, kind };
  let change = match change.asked_by(by) {
    Ok(change) => change,
    Err(refused) => {
      db.record_permission_event(run, &event(PermissionEventKind::Refused))?;
      return Err(refused.into());
    }
  };
  let mode = db.run_approval_mode(run)?;
  let everything = db.run_auto_approve(run)? == Some(true);
  let recorded = match change {
    RunPermissionChange::ManualForRun => {
      db.switch_run_to_manual(run, ManualSwitch { at, by })?;
      // The switch is the run's own record; approving everything stops with it, as Manual narrows.
      if everything {
        Some(stop_everything(&mut db, run)?)
      } else {
        None
      }
    }
    RunPermissionChange::ApproveEverything if !everything => {
      db.set_run_auto_approve(run, Some(true))?;
      Some(PermissionEventKind::Everything { was: mode })
    }
    RunPermissionChange::StopApprovingEverything if everything => Some(stop_everything(&mut db, run)?),
    RunPermissionChange::ApproveEverything | RunPermissionChange::StopApprovingEverything => None,
    RunPermissionChange::Revoke(rule) => {
      let scope = db
        .approval_rules(run)?
        .into_iter()
        .find(|stored| stored.id == rule)
        .map(|stored| stored.scope);
      match scope {
        Some(scope) if db.revoke_approval_rule(run, rule, at)? => Some(PermissionEventKind::Revoked { scope }),
        _ => None,
      }
    }
  };
  if let Some(kind) = recorded {
    db.record_permission_event(run, &event(kind))?;
  }
  Ok(())
}

/// Stops `run` approving everything, back to its mode: in Autopilot it follows the mode again, in Manual nothing
/// approves automatically. The event that records it.
fn stop_everything(db: &mut ProjectDb, run: RunId) -> Result<PermissionEventKind, StoreError> {
  let back_to = db.run_approval_mode(run)?;
  let chosen = if back_to.is_autopilot() { None } else { Some(false) };
  db.set_run_auto_approve(run, chosen)?;
  Ok(PermissionEventKind::Stopped { back_to })
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
  let decisions = db.decision_connections()?;
  let states = crate::project_settings::decision_reviewer::states(&decisions);
  Ok(
    resolve_reviewer(&settings, &connections, &states).map(|resolved| ReviewerShown {
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
  /// The run's own auto-approval choice; `None` follows its mode.
  pub chosen: Option<bool>,
  /// The mode the run answers its agents' requests in now.
  pub mode: EffectiveMode,
  /// The project's *Allow Autopilot for agents in this project*.
  pub allow_autopilot: AllowAutopilot,
  /// When and by whom the run was switched to Manual for itself, once it was and that was recorded.
  pub switched: Option<ManualSwitch>,
  pub rules: Vec<ApprovalRule>,
  pub counts: DecisionCounts,
  /// What its permissions menu changed, and automation's refused asking to widen it, oldest first.
  pub events: Vec<PermissionEvent>,
}

impl RunApprovals {
  pub fn read(db: &ProjectDb, run: &Run) -> Result<Self, StoreError> {
    Ok(Self {
      chosen: db.run_auto_approve(run.id())?,
      mode: db.run_approval_mode(run.id())?,
      allow_autopilot: db.allow_autopilot()?,
      switched: db.run_manual_switch(run.id())?,
      rules: db.approval_rules(run.id())?,
      counts: db.decision_counts(run.id())?,
      events: db.permission_events(run.id())?,
    })
  }

  /// Whether the run approves everything: a person chose *Approve everything in this run* and nobody stopped it.
  pub const fn approves_everything(&self) -> bool {
    matches!(self.chosen, Some(true))
  }

  /// Whether the run may still be switched to Manual for itself: it is in Autopilot and has not ended.
  pub fn may_switch_to_manual(&self, run: &Run) -> bool {
    self.mode.is_autopilot() && !run.status().is_terminal()
  }

  /// Whether Orchester auto-approves the run's requests now.
  pub const fn auto_approving(&self) -> bool {
    auto_approves(self.chosen, self.mode.is_autopilot())
  }

  /// Whether Autopilot answers the run's requests: the run is in it, and its own choice (*Stop*) did not take it back.
  pub fn approved_by_autopilot(&self) -> bool {
    self.mode.is_autopilot() && self.chosen != Some(false)
  }
}
