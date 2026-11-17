//! A team run whose authority the server took away (OA-31): its claim taken over, released or unassigned, the person no
//! longer a member, the claim held from another session, or a newer run carrying the work on. Once fenced it reports
//! nothing more, and its agents would work for nothing: it is cancelled here, its worktree and unsent reports staying
//! on this computer to read. The task page says why from the fence (`TeamHere::fences`), and the domain tools refuse
//! to carry it on (`claim_fenced`). A runner that is not attached executes nothing, so the run is stopped when it
//! next works.

use orchester_core::{id::RunId, store::RunStore};
use orchester_store::StoreError;

use super::{TeamRunner, TeamWork, TeamWorkError};
use crate::runner::{RequestError, RunRequestError};

impl<R: TeamRunner> TeamWork<R> {
  /// Cancels the fenced team runs that still have work under way here; the runs it cancelled. A run the runner could
  /// not cancel now is tried again at the next round.
  pub async fn stop_fenced(&self) -> Result<Vec<RunId>, TeamWorkError> {
    let mut stopped = Vec::new();
    for run in self.fenced_at_work()? {
      match self.runner.cancel_run(run).await {
        Ok(()) => stopped.push(run),
        Err(RunRequestError::Request(RequestError::NotConnected)) => {}
        Err(error) => tracing::warn!(%error, %run, "a team run whose claim was lost could not be stopped"),
      }
    }
    Ok(stopped)
  }

  /// The fenced team runs whose agents work, wait for a person or go on once resumed.
  fn fenced_at_work(&self) -> Result<Vec<RunId>, TeamWorkError> {
    let db = self.db()?;
    let mut runs = Vec::new();
    for team in db.team_runs()?.into_iter().filter(|team| team.fence.is_some()) {
      let run = db.run(team.run)?.ok_or(StoreError::RunNotFound(team.run))?;
      if run.status().has_live_work() {
        runs.push(team.run);
      }
    }
    Ok(runs)
  }
}
