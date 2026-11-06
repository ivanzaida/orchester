//! The work reports of the team runs on this computer, sent to their team servers ([`crate::team_work`], "Reports"):
//! each change of a team run's outcome (cancelled, failed, succeeded, paused at a limit) is queued once and sent in
//! order, every minute and whenever a run changes, so the server learns that a run ended and its work no longer runs.
//! A project with no team run that still reports needs no session and sends nothing.

use orchester_store::ProjectDb;

use super::ProjectServer;
use crate::{
  actions::Dispatcher,
  background,
  sharing::{Servers, SharingError},
  team_work::TeamWork,
};

impl Servers {
  /// Sends the queued and new work reports of every team project in `projects`.
  pub(super) async fn report_team_work(&self, projects: &[ProjectServer]) {
    for project in projects {
      if !reports_here(project).await {
        continue;
      }
      let session = match self.team_session(project.server(), project.team_project()).await {
        Ok(session) => session,
        Err(SharingError::Offline | SharingError::SignedOut) => continue,
        Err(error) => {
          tracing::warn!(%error, project = %project.link().project, "no team session to send work reports in");
          continue;
        }
      };
      let dispatcher = Dispatcher::new(project.repo_root().to_owned(), self.clone(), Some(session));
      match TeamWork::new(dispatcher, ()).report().await {
        Ok(reported) if reported.waiting => {
          tracing::debug!(project = %project.link().project, "work reports wait for the team server");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, project = %project.link().project, "cannot send work reports"),
      }
    }
  }
}

/// Whether `project` has a team run that still reports: one the server has not fenced.
async fn reports_here(project: &ProjectServer) -> bool {
  let root = project.repo_root().to_owned();
  let read = background::run(move || {
    ProjectDb::open(&root)
      .and_then(|db| db.team_runs())
      .map(|runs| runs.iter().any(|run| run.fence.is_none()))
  })
  .await;
  match read {
    Ok(Ok(reports)) => reports,
    Ok(Err(error)) => {
      tracing::warn!(%error, project = %project.link().project, "cannot read the team runs to report");
      false
    }
    Err(_) => false,
  }
}
