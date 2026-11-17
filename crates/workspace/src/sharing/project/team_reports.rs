//! The team runs' work on this computer, carried to their team servers ([`crate::team_work`]) every minute and whenever
//! a run changes. Each change of a team run's outcome (cancelled, failed, succeeded, paused at a limit) is reported in
//! order, so the server learns that a run ended and its work no longer runs; a run whose authority the server took
//! away is stopped (OA-31); the task of a run merged here is set Done (OA-30). The minute's round also renews the
//! claims of the work this computer holds and of its modules' deliveries (OA-31): one heartbeat per team project a
//! minute (two against a server that renews running work only), within the server's limit. A project with no team run
//! to carry on and no Done to send needs no session and sends nothing.

use std::path::Path;

use orchester_core::{id::ProjectId, store::RunStore};
use orchester_store::{ProjectDb, StoreError};
use tokio::runtime::Handle;

use super::ProjectServer;
use crate::{
  actions::Dispatcher,
  background,
  runner::Runners,
  sharing::{Servers, SharingError},
  tasks::detail::run_store_error,
  team_work::{Heartbeat, ProjectRunner, TeamWork},
};

/// Which round carries the team work on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TeamRound {
  /// The minute's round, which also renews the claims.
  Minute,
  /// A run changed: its reports, stops and Done go at once.
  RunChanged,
}

impl Servers {
  /// Carries on the team work of every team project in `projects`, whose runs `runners` stop.
  pub(super) async fn carry_team_work(&self, projects: &[ProjectServer], runners: &Runners, round: TeamRound) {
    for project in projects {
      let Some(name) = to_carry(project).await else {
        continue;
      };
      let session = match self.team_session(project.server(), project.team_project()).await {
        Ok(session) => session,
        Err(SharingError::Offline | SharingError::SignedOut) => continue,
        Err(error) => {
          tracing::warn!(%error, project = %project.link().project, "no team session to carry the team work on in");
          continue;
        }
      };
      let root = project.repo_root().to_owned();
      let dispatcher = Dispatcher::new(root.clone(), self.clone(), Some(session));
      let work = TeamWork::new(dispatcher, ProjectRunner::new(runners.clone(), root, name));
      let (runtime, shared) = (Handle::current(), project.link().project);
      // A team command holds the project database across its awaits, so the round runs on a worker; one that did
      // not finish is the window closing.
      if background::run(move || runtime.block_on(carry(&work, shared, round)))
        .await
        .is_err()
      {
        return;
      }
    }
  }
}

/// One project's round: its reports, the renewal of its claims in the minute's round, the stop of its fenced runs,
/// then the Done of its merged runs, once their reports told the server their work ended.
async fn carry(work: &TeamWork<ProjectRunner>, shared: ProjectId, round: TeamRound) {
  match work.report().await {
    Ok(reported) if reported.waiting => tracing::debug!(project = %shared, "work reports wait for the team server"),
    Ok(_) => {}
    Err(error) => tracing::warn!(%error, project = %shared, "cannot send work reports"),
  }
  if round == TeamRound::Minute {
    match work.heartbeat().await {
      Ok(Heartbeat::Renewed { lost, .. }) if !lost.is_empty() => {
        tracing::warn!(project = %shared, runs = lost.len(), "team runs lost their claims");
      }
      Ok(Heartbeat::Refused(refusal)) => tracing::warn!(?refusal, project = %shared, "the team heartbeat was refused"),
      Ok(other) => tracing::debug!(?other, project = %shared, "team heartbeat"),
      Err(error) => tracing::warn!(%error, project = %shared, "cannot renew the team claims"),
    }
  }
  if let Err(error) = work.stop_fenced().await {
    tracing::warn!(%error, project = %shared, "cannot stop the team runs whose claims were lost");
  }
  match work.send_merged_done().await {
    Ok(sent) if sent.waiting => tracing::debug!(project = %shared, "a merged task's Done waits for the team server"),
    Ok(_) => {}
    Err(error) => tracing::warn!(%error, project = %shared, "cannot set merged team tasks Done"),
  }
}

/// The name of `project` when it has team work to carry on: a team run the server has not fenced, a fenced one that
/// still works here, or a merged run's Done to send.
async fn to_carry(project: &ProjectServer) -> Option<String> {
  let root = project.repo_root().to_owned();
  match background::run(move || carried_here(&root)).await {
    Ok(Ok(name)) => name,
    Ok(Err(error)) => {
      tracing::warn!(%error, project = %project.link().project, "cannot read the team work to carry on");
      None
    }
    Err(_) => None,
  }
}

fn carried_here(root: &Path) -> Result<Option<String>, StoreError> {
  let db = ProjectDb::open(root)?;
  let mut carried = !db.pending_merged_done()?.is_empty();
  for team in db.team_runs()? {
    if carried {
      break;
    }
    carried = match team.fence {
      None => true,
      Some(_) => db
        .run(team.run)
        .map_err(run_store_error)?
        .ok_or(StoreError::RunNotFound(team.run))?
        .status()
        .has_live_work(),
    };
  }
  Ok(carried.then(|| db.project().name().as_str().to_owned()))
}
