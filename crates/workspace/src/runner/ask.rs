//! Requests about runs to a project's runner: the screens ask the displayed project's runner;
//! automation (Orchester's MCP server) names the project, whose runner starts if it has none and,
//! when no tab shows the project, stops again once it has no work.

use std::{path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  integration::Leftovers,
  limits::AgentSlots,
  sharing::TaskRevision,
};
use orchester_ipc::{Command, Reply};
use orchester_store::StoreError;

use super::{Continued, RequestError, RunRequestError, Runners, state::RunnerState};
use crate::workspace::OpenProject;

/// How long continuing interrupted runs, or a request automation makes, waits for a project's
/// runner to come online.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

impl Runners {
  /// Asks the displayed project's runner to start a run of queued task `task`. Idempotent by
  /// `key`: retrying after a lost reply answers with the run the first request created.
  pub async fn start_run(&self, key: IdempotencyKey, task: TaskId) -> Result<RunId, RunRequestError> {
    let reply = self
      .ask(self.displayed().as_deref(), Command::StartRun { key, task })
      .await?;
    run_started(reply, "StartRun")
  }

  /// [`Runners::start_run`] for `project`, whichever tab shows it, if any.
  pub async fn start_run_in(
    &self,
    project: &OpenProject,
    key: IdempotencyKey,
    task: TaskId,
  ) -> Result<RunId, RunRequestError> {
    let reply = self.ask_project(project, Command::StartRun { key, task }).await?;
    run_started(reply, "StartRun")
  }

  /// Asks the displayed project's runner to start the next run of task `task` after its latest
  /// run `after` failed or was cancelled (Run again), removing `remove_worktrees`, what `after`
  /// left, first. Idempotent by `key`, like [`Runners::start_run`]. With `revision`, only for the task at it
  /// (`Rejection::TaskChangedMeanwhile` otherwise).
  pub async fn run_again(
    &self,
    key: IdempotencyKey,
    (task, after, revision): (TaskId, RunId, Option<TaskRevision>),
    remove_worktrees: Option<Leftovers>,
  ) -> Result<RunId, RunRequestError> {
    let command = Command::RunAgain {
      key,
      task,
      after,
      remove_worktrees,
      revision,
    };
    run_started(self.ask(self.displayed().as_deref(), command).await?, "RunAgain")
  }

  /// [`Runners::run_again`] for `project`, keeping what the latest run left.
  pub async fn run_again_in(
    &self,
    project: &OpenProject,
    key: IdempotencyKey,
    (task, after, revision): (TaskId, RunId, Option<TaskRevision>),
  ) -> Result<RunId, RunRequestError> {
    let command = Command::RunAgain {
      key,
      task,
      after,
      remove_worktrees: None,
      revision,
    };
    run_started(self.ask_project(project, command).await?, "RunAgain")
  }

  /// [`Runners::run_again`] for `project`, whichever tab shows it, removing `remove_worktrees`, what
  /// `after` left, first, only for the task at `revision`, the one the person confirmed: a task changed since is
  /// refused (`Rejection::TaskChangedMeanwhile`).
  pub async fn run_again_removing_in(
    &self,
    project: &OpenProject,
    key: IdempotencyKey,
    (task, after, revision): (TaskId, RunId, TaskRevision),
    remove_worktrees: Option<Leftovers>,
  ) -> Result<RunId, RunRequestError> {
    let command = Command::RunAgain {
      key,
      task,
      after,
      remove_worktrees,
      revision: Some(revision),
    };
    run_started(self.ask_project(project, command).await?, "RunAgain")
  }

  /// Asks the displayed project's runner to pause, resume or cancel a run, or to raise its
  /// limits (`command`); returns once the runner has carried it out.
  pub async fn control_run(&self, command: Command) -> Result<(), RunRequestError> {
    accepted(self.ask(self.displayed().as_deref(), command).await?)
  }

  /// [`Runners::control_run`] for `project`.
  pub async fn control_run_in(&self, project: &OpenProject, command: Command) -> Result<(), RunRequestError> {
    accepted(self.ask_project(project, command).await?)
  }

  /// Asks the displayed project's runner to continue interrupted run `run` from its last
  /// checkpoint; returns once it runs again.
  pub async fn continue_run(&self, run: RunId) -> Result<(), RunRequestError> {
    let command = Command::ContinueRuns { runs: vec![run] };
    continued(self.ask(self.displayed().as_deref(), command).await?)
  }

  /// [`Runners::continue_run`] for `project`.
  pub async fn continue_run_in(&self, project: &OpenProject, run: RunId) -> Result<(), RunRequestError> {
    let command = Command::ContinueRuns { runs: vec![run] };
    continued(self.ask_project(project, command).await?)
  }

  /// Continues interrupted runs `runs` of project `root`, starting its runner (named `name`) if
  /// needed and waiting for it to attach.
  pub async fn continue_runs(&self, root: &Path, name: &str, runs: Vec<RunId>) -> Result<Continued, RunRequestError> {
    self.ensure(root, name);
    // A project no tab shows keeps its runner for the runs it continues, then lets it stop.
    let _pin = self.pin(root);
    self.linger_untabbed();
    self.attached(root).await?;
    match self.ask(Some(root), Command::ContinueRuns { runs }).await? {
      Reply::Continued { continued, refused } => Ok(Continued { continued, refused }),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => unexpected(&other, "ContinueRuns"),
    }
  }

  /// Runs `store`, which stores project `root`'s limits and returns the agent slots when they
  /// changed, then hands changed slots to the project's runner, which applies them at once, without
  /// waiting for its answer. Storing and queueing happen under one lock, so with saves close
  /// together the runner gets the slots in the order they were stored and ends with the stored
  /// ones. A runner that is not connected now, or that drops or stops before it takes them, misses
  /// nothing: every runner reads the stored slots when it attaches. Blocks on SQLite.
  pub fn store_agent_slots(
    &self,
    root: &Path,
    store: impl FnOnce() -> Result<Option<AgentSlots>, StoreError>,
  ) -> Result<(), StoreError> {
    let answer = {
      let _in_order = self.inner.slot_updates.lock();
      let Some(slots) = store()? else {
        return Ok(());
      };
      let command = Command::SetAgentSlots {
        agent_slots: slots.get(),
      };
      let projects = self.inner.projects.lock();
      match projects.get(root).and_then(|entry| entry.connection.as_ref()) {
        Some(connection) => connection.request(command),
        None => return Ok(()),
      }
    };
    let root = root.to_path_buf();
    self.inner.runtime.spawn(async move {
      // The task ended without answering, e.g. when the runner stopped meanwhile.
      match answer.await.unwrap_or(Err(RequestError::Lost)) {
        Ok(Reply::AgentSlotsSet { .. }) => {}
        Ok(Reply::Rejected(rejection)) => {
          tracing::info!(project = %root.display(), ?rejection, "the runner left the agent slots to the next one");
        }
        Ok(other) => tracing::error!(?other, "unexpected reply to SetAgentSlots"),
        Err(error) => tracing::info!(project = %root.display(), %error, "no runner took the agent slots now"),
      }
    });
    Ok(())
  }

  /// Asks `project`'s runner, starting it if needed and waiting for it to attach. A project no tab
  /// shows keeps its runner while the request is under way, then lets it stop once it has no work.
  pub(super) async fn ask_project(&self, project: &OpenProject, command: Command) -> Result<Reply, RequestError> {
    let root = project.repo_root.as_path();
    self.ensure(root, project.project.name().as_str());
    let _pin = self.pin(root);
    self.linger_untabbed();
    self.attached(root).await?;
    self.ask(Some(root), command).await
  }

  /// Waits until the runner of `root` has attached (it reported its work), or gave up.
  pub(super) async fn attached(&self, root: &Path) -> Result<(), RequestError> {
    let Some(mut updates) = self
      .inner
      .projects
      .lock()
      .get(root)
      .map(|entry| entry.status.subscribe())
    else {
      return Err(RequestError::NotConnected);
    };
    let ready = async {
      loop {
        {
          let status = updates.borrow_and_update();
          match status.state() {
            RunnerState::Online if status.work().is_some() => return Ok(()),
            RunnerState::Failed { .. } | RunnerState::Stopped => return Err(RequestError::NotConnected),
            _ => {}
          }
        }
        if updates.changed().await.is_err() {
          return Err(RequestError::NotConnected);
        }
      }
    };
    tokio::time::timeout(CONNECT_TIMEOUT, ready)
      .await
      .unwrap_or(Err(RequestError::NotConnected))
  }

  pub(super) async fn ask(&self, root: Option<&Path>, command: Command) -> Result<Reply, RequestError> {
    let answer = {
      let projects = self.inner.projects.lock();
      match root
        .and_then(|root| projects.get(root))
        .and_then(|entry| entry.connection.as_ref())
      {
        Some(connection) => connection.request(command),
        None => return Err(RequestError::NotConnected),
      }
    };
    // The task ended without answering, e.g. when the runner stopped meanwhile.
    answer.await.unwrap_or(Err(RequestError::Lost))
  }
}

/// The run a start request created.
fn run_started(reply: Reply, request: &str) -> Result<RunId, RunRequestError> {
  match reply {
    Reply::RunStarted { run } => Ok(run),
    Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
    other => unexpected(&other, request),
  }
}

/// The one interrupted run asked about runs again.
fn continued(reply: Reply) -> Result<(), RunRequestError> {
  match reply {
    Reply::Continued { refused, .. } => match refused.into_iter().next() {
      Some(refused) => Err(RunRequestError::Rejected(refused.rejection)),
      None => Ok(()),
    },
    Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
    other => unexpected(&other, "ContinueRuns"),
  }
}

/// A run control request was carried out.
fn accepted(reply: Reply) -> Result<(), RunRequestError> {
  match reply {
    Reply::Accepted { .. } => Ok(()),
    Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
    other => unexpected(&other, "a run control request"),
  }
}

/// A reply that does not answer `request`: the runner and the desktop disagree, so its outcome is
/// unknown, as for a lost reply.
fn unexpected<T>(reply: &Reply, request: &str) -> Result<T, RunRequestError> {
  tracing::error!(?reply, request, "unexpected reply");
  Err(RunRequestError::Request(RequestError::Lost))
}
