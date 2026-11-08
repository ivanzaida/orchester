//! Team work through the team server (team authority P4, `docs/cloud-api.md`, "Team authority P4 work"): a team task's
//! run started from its held claim, its work reported, and its claim kept alive.
//!
//! - **Start** ([`start`]). The caller first checks that the pipeline is ready on this computer
//!   ([`crate::readiness::StartReadiness`]), before any claim or work. `work.start` then goes through the dispatcher
//!   under the start's operation; the start is recorded before it, with the program paths the caller read. Then, one
//!   boundary at a time, each kept before the next: the receipt's work and bundle reference; the bundle with every file
//!   it pins, read by hash and checked, checked again for readiness against the versions it pins and kept as the run's
//!   accepted inputs, bound to what this computer filled in, in one transaction with the start's record; the runner's
//!   run. A crash or an outage at any boundary is carried on from it ([`TeamWork::recover`]); the operation and the
//!   runner's start are idempotent, so a retry never makes a second work or run. A bundle that can never become the
//!   run's inputs here, or whose pinned versions need what this computer lacks, settles the start failed
//!   ([`StartFailure`]), and so does a start the server refuses `bundle_missing`: either way the claim is released
//!   first ([`release`]), so teammates are not kept from the task, and a server that cannot be reached for the release
//!   keeps the start waiting. A project carries on one start at a time, so a Run and the recovery of the open starts
//!   never work the same one at once. Nothing new starts without the server. A project's open starts are carried on
//!   when it is opened ([`recover_open_starts`]), and what a task's newest start came to is read back for its page
//!   ([`last_start`]): a start that waited and was settled later says why it did not run.
//! - **Reports** ([`reports`]). Each change of a team run's outcome is queued once in the project database, with its
//!   event id and the run's next sequence, then sent in order until the server acknowledges it by id. A server that
//!   cannot be reached or refuses for now keeps the rest waiting, in order. Its answers that the authority is gone
//!   (`claim_lost`, `membership_revoked`, `session_mismatch`, or a newer run carrying the work on) fence the run: it
//!   reports nothing more, and its run, worktree and unsent reports stay here. A run goes on executing during an
//!   outage; what waits is only its reports.
//! - **Heartbeat** ([`heartbeat`]). Only runs at work renew their claims (D1): a run that waits for a person, is paused
//!   or ended renews nothing, and the server's `decision_required` is an outcome, not a failure to retry.
//!
//! - **Module delivery** ([`delivery`]). The module's holder requests the delivery of the module branch to base; this
//!   computer merges only on the receipt's `deliver_module` effect and reports the merge, persisted first and retried
//!   in order ([`ModuleDeliveries`]).
//!
//! A run's local status and its server acceptance (the work as the server last answered its reports) are kept apart
//! ([`TeamRunState`]). The MCP tool `run_task` starts a team task's work through it (P7); the screens adopt it in P6.

mod branch;
mod bundle;
mod bundle_missing;
pub mod delivery;
mod error;
mod heartbeat;
mod here;
mod last;
mod open;
mod release;
mod reports;
mod runner;
mod start;
mod turn;

use orchester_api::{
  client::AuthorityCallError,
  v2::{AuthorityError, WorkRead},
};
use orchester_core::{id::RunId, run::RunStatus, store::RunStore};
use orchester_store::{ProjectDb, TeamFence, WorkReportState};

pub use self::{
  branch::{BranchNotFound, branch_tip},
  bundle_missing::bundle_missing_message,
  delivery::{
    DeliveryProgress, DeliveryStatus, ModuleDeliveries, RecoveredDeliveries, TeamDeliveryRequest, TeamDeliveryState,
  },
  error::TeamWorkError,
  heartbeat::Heartbeat,
  here::{TeamHere, team_here},
  last::{LastStart, last_start},
  open::{OpenStarts, OpenStartsError, recover_open_starts},
  release::ClaimEnd,
  reports::Reported,
  runner::{ProjectRunner, TeamRunner},
  start::{Recovered, StartFailure, StartOutcome, TeamStartRequest},
  turn::StartTurn,
};
use crate::actions::{Dispatcher, retryable};

/// One team project's work in one session: its dispatcher, signed in to the project's server, and its runner.
pub struct TeamWork<R> {
  dispatcher: Dispatcher,
  runner: R,
}

/// What the server answered when it did not do what was asked.
#[derive(Debug)]
pub(crate) enum Answer {
  /// It could not be reached, or refused for now (signed out, rate limited, unavailable, an unsupported client):
  /// nothing changed there, and the step is tried again.
  Unreachable,
  /// It refused for good.
  Refused(AuthorityError),
}

pub(crate) fn answer(error: AuthorityCallError) -> Answer {
  match error {
    AuthorityCallError::Refused { error, .. } if !retryable(error.code()) => Answer::Refused(error),
    AuthorityCallError::Refused { .. } | AuthorityCallError::Client(_) => Answer::Unreachable,
  }
}

/// A team run as the screens show it: how it runs here, and what the server accepted of its work, apart.
#[derive(Clone, Debug)]
pub struct TeamRunState {
  /// The run's own status on this computer: an execution success here is not the server's acceptance.
  pub local: RunStatus,
  /// The work as the server last answered one of the run's reports; `None` before it answered any.
  pub accepted: Option<WorkRead>,
  /// Why the run reports nothing more, once the server said its authority is gone.
  pub fence: Option<TeamFence>,
  /// Reports queued and not answered yet.
  pub waiting_reports: usize,
  /// Reports the server refused, kept as it refused them.
  pub refused_reports: usize,
}

impl<R> TeamWork<R> {
  /// The work of the team project `dispatcher` acts on, started by `runner`. Reporting needs no runner: the
  /// background reports make it with `()`.
  pub const fn new(dispatcher: Dispatcher, runner: R) -> Self {
    Self { dispatcher, runner }
  }

  fn db(&self) -> Result<ProjectDb, TeamWorkError> {
    Ok(ProjectDb::open(self.dispatcher.root())?)
  }
}

impl<R: TeamRunner> TeamWork<R> {
  /// Team run `run` as it stands here and on the server; `None` for a run that is not a team run.
  pub fn run_state(&self, run: RunId) -> Result<Option<TeamRunState>, TeamWorkError> {
    let db = self.db()?;
    let Some(team) = db.team_run(run)? else {
      return Ok(None);
    };
    let local = db.run(run)?.ok_or(orchester_store::StoreError::RunNotFound(run))?;
    let accepted = team
      .accepted
      .map(|accepted| serde_json::from_str(&accepted.work))
      .transpose()
      .map_err(|_| TeamWorkError::UnreadableRun(run))?;
    let reports = db.work_reports(run)?;
    let count = |state| reports.iter().filter(|report| report.state == state).count();
    Ok(Some(TeamRunState {
      local: local.status().clone(),
      accepted,
      fence: team.fence,
      waiting_reports: count(WorkReportState::Pending),
      refused_reports: count(WorkReportState::Refused),
    }))
  }
}
