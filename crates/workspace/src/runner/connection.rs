//! One project's runner: launch, watch, reconnect and stop, on a tokio task. Everything it
//! observes is reported as a [`RunnerEvent`]; the state machine decides whether to reconnect.
//! Requests travel to the connected runner and their replies back; change notifications are
//! published through the reporter.

use std::{
  num::NonZeroU32,
  path::{Path, PathBuf},
  pin::pin,
  time::Duration,
};

use orchester_core::limits::AgentSlots;
use orchester_ipc::{AttachSession, Command, ErrorReports, Event, IpcError, LaunchError, Reply, RunnerClient};
use tokio::{
  runtime::Handle,
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

mod session;

use self::session::{Served, Session};
use super::{
  ShutdownOutcome,
  report::Reporter,
  requests::{ReplySender, RequestError},
  state::{RunnerEvent, RunnerFailure},
};

/// How long the runner may take to exit after it reported `Stopped` and the connection closed.
const EXIT_TIMEOUT: Duration = Duration::from_secs(2);

enum Control {
  Stop {
    reply: oneshot::Sender<ShutdownOutcome>,
  },
  /// Stop only if the runner has no work; the answer says whether it stopped.
  StopIfIdle {
    reply: oneshot::Sender<bool>,
  },
  Request {
    command: Command,
    reply: ReplySender,
  },
}

/// Handle to the task that owns a runner. Dropping it ends the task, which drops the runner
/// process and so terminates the runner and its agents ("quit now").
pub(super) struct Connection {
  control: mpsc::UnboundedSender<Control>,
  task: JoinHandle<()>,
}

impl Drop for Connection {
  fn drop(&mut self) {
    self.task.abort();
  }
}

impl Connection {
  /// Starts the runner for `project`, sending error reports with `reports`. Its first attach is
  /// for `session`; every runner started after a drop attaches as a reconnect, with the same
  /// `reports`.
  pub(super) fn spawn(
    runtime: &Handle,
    runner_exe: PathBuf,
    reports: Option<ErrorReports>,
    project: PathBuf,
    reporter: Reporter,
    session: AttachSession,
  ) -> Self {
    let (control, requests) = mpsc::unbounded_channel();
    let task = runtime.spawn(
      Supervisor {
        runner_exe,
        reports,
        project,
        reporter,
        requests,
        session,
      }
      .run(),
    );
    Self { control, task }
  }

  /// Sends `command` to the runner; the reply arrives through the receiver.
  pub(super) fn request(&self, command: Command) -> oneshot::Receiver<Result<Reply, RequestError>> {
    let (reply, answer) = oneshot::channel();
    if let Err(mpsc::error::SendError(Control::Request { reply, .. })) =
      self.control.send(Control::Request { command, reply })
    {
      // The task has ended; nobody would answer.
      drop(reply.send(Err(RequestError::NotConnected)));
    }
    answer
  }

  /// Asks the runner to stop if it has no work; the answer is `true` once it stopped, `false`
  /// while it has work or could not be asked.
  pub(super) fn stop_if_idle(&self) -> oneshot::Receiver<bool> {
    let (reply, answer) = oneshot::channel();
    if let Err(mpsc::error::SendError(Control::StopIfIdle { reply })) = self.control.send(Control::StopIfIdle { reply })
    {
      // The task has ended, so there is no runner to stop; the asker may have stopped waiting.
      let _ = reply.send(false);
    }
    answer
  }

  /// Asks the runner to stop at checkpoints and waits for it, at most `deadline` if given.
  /// Without an answer in time the runner is terminated.
  pub(super) async fn stop(self, deadline: Option<Duration>) -> ShutdownOutcome {
    let (reply, answer) = oneshot::channel();
    if self.control.send(Control::Stop { reply }).is_err() {
      return ShutdownOutcome::NoRunner;
    }
    let answer = match deadline {
      Some(deadline) => match tokio::time::timeout(deadline, answer).await {
        Ok(answer) => answer,
        Err(_) => {
          tracing::warn!("the runner did not stop within {deadline:?}; terminating it");
          return ShutdownOutcome::Unanswered;
        }
      },
      None => answer.await,
    };
    // The task ended without answering: it had already given up on the runner.
    answer.unwrap_or(ShutdownOutcome::NoRunner)
  }
}

struct Supervisor {
  runner_exe: PathBuf,
  /// Whether its runners send error reports, as the runners were told when this connection started.
  reports: Option<ErrorReports>,
  project: PathBuf,
  reporter: Reporter,
  requests: mpsc::UnboundedReceiver<Control>,
  /// How the next runner attaches.
  session: AttachSession,
}

impl Supervisor {
  async fn run(mut self) {
    self.supervise().await;
    self.refuse_unsent();
  }

  /// Control messages that arrived while the task was ending were never handled: a launch that
  /// fails at once, or a runner that went for good, ends it without reading them. Each asker gets
  /// the answer for no runner, as while one is starting, instead of a dropped reply, which would
  /// read as a request the runner may have carried out ([`RequestError::Lost`]).
  fn refuse_unsent(&mut self) {
    self.requests.close();
    while let Ok(control) = self.requests.try_recv() {
      self.ends_idle(Some(control));
    }
  }

  async fn supervise(&mut self) {
    loop {
      let client = match self.launch().await {
        Some(Ok(client)) => client,
        Some(Err(failure)) => {
          if !self.retry_after(RunnerEvent::LaunchFailed(failure)).await {
            return;
          }
          continue;
        }
        None => return,
      };
      if self.reporter.report(RunnerEvent::Launched).is_none() {
        return;
      }
      let session = self.session;
      // Whatever happens to this runner, the next one replaces it after a drop.
      self.session = AttachSession::Reconnect;
      let agent_slots = attach_slots(&self.project).await;
      let served = Session::start(client)
        .serve((session, agent_slots), &mut self.requests, &self.reporter)
        .await;
      self.reporter.browser_ended();
      match served {
        Served::Dropped(failure) => {
          if !self.retry_after(RunnerEvent::Dropped(failure)).await {
            return;
          }
        }
        Served::Stopped | Served::Abandoned => return,
      }
    }
  }

  /// The launched runner, or `None` when a stop request or the desktop ended the task first.
  async fn launch(&mut self) -> Option<Result<RunnerClient, RunnerFailure>> {
    let mut launch = pin!(RunnerClient::launch(
      &self.runner_exe,
      &self.project,
      self.reports.as_ref()
    ));
    let result = loop {
      tokio::select! {
        result = &mut launch => break result,
        request = self.requests.recv() => {
          if self.ends_idle(request) {
            return None;
          }
        }
      }
    };
    Some(result.map_err(|error| {
      tracing::error!(project = %self.project.display(), %error, "cannot start the runner");
      launch_failure(&error, &self.runner_exe)
    }))
  }

  /// Reports a failure and, if the state machine reconnects, waits for the next attempt.
  async fn retry_after(&mut self, event: RunnerEvent) -> bool {
    let Some(delay) = self.reporter.report(event).and_then(|state| state.reconnect_delay()) else {
      return false;
    };
    tracing::info!(project = %self.project.display(), ?delay, "reconnecting to the runner");
    let mut sleep = pin!(tokio::time::sleep(delay));
    loop {
      tokio::select! {
        () = &mut sleep => return true,
        request = self.requests.recv() => {
          if self.ends_idle(request) {
            return false;
          }
        }
      }
    }
  }

  /// Handles a control message while no runner is connected: a request cannot be sent, and a
  /// stop has nothing to shut down. `true` when the task should end: a stop, or the desktop
  /// dropped the connection handle (`None`).
  fn ends_idle(&self, request: Option<Control>) -> bool {
    match request {
      Some(Control::Request { reply, .. }) => {
        // The requester may have stopped waiting; the answer no longer matters then.
        drop(reply.send(Err(RequestError::NotConnected)));
        false
      }
      Some(Control::Stop { reply }) => {
        self.reporter.report(RunnerEvent::StopRequested);
        self.reporter.report(RunnerEvent::Stopped);
        // The desktop may have stopped waiting (close timeout); the result no longer matters then.
        let _ = reply.send(ShutdownOutcome::NoRunner);
        true
      }
      // Not connected: a runner that is starting or reconnecting is not idle yet. The asker may
      // have stopped waiting; the answer no longer matters then.
      Some(Control::StopIfIdle { reply }) => {
        let _ = reply.send(false);
        false
      }
      None => true,
    }
  }
}

/// The project's *Agents working at once*, read each time a runner attaches, so a change made while
/// no runner was connected applies too. Unreadable limits leave the runner the default: it still
/// serves the project, and Project settings › Limits shows why they cannot be read.
async fn attach_slots(project: &Path) -> NonZeroU32 {
  let root = project.to_path_buf();
  let read = tokio::task::spawn_blocking(move || crate::tasks::usage::limits_in(&root)).await;
  let default = AgentSlots::DEFAULT.get();
  match read {
    Ok(Ok(limits)) => limits.agent_slots().get(),
    Ok(Err(error)) => {
      tracing::error!(project = %project.display(), %error, "cannot read the agent slots; the runner uses {default}");
      default
    }
    Err(error) => {
      tracing::error!(project = %project.display(), %error, "reading the agent slots did not finish; the runner uses {default}");
      default
    }
  }
}

fn launch_failure(error: &LaunchError, runner_exe: &Path) -> RunnerFailure {
  match error {
    LaunchError::Spawn { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
      RunnerFailure::MissingExecutable {
        path: runner_exe.to_path_buf(),
      }
    }
    LaunchError::ProjectLocked
    | LaunchError::ForeignPipeServer { .. }
    | LaunchError::Ipc(IpcError::EndpointInUse { .. }) => RunnerFailure::AlreadyServed,
    _ => RunnerFailure::StartFailed,
  }
}

type EventItem = Result<Option<Event>, IpcError>;
