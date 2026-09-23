//! One project's runner: launch, watch, reconnect and stop, on a tokio task. Everything it
//! observes is reported as a [`RunnerEvent`]; the state machine decides whether to reconnect.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_ipc::{Command, CommandSender, Event, IpcError, LaunchError, RunnerClient, RunnerProcess, ShutdownMode};
use tokio::{
  runtime::Handle,
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

use super::{
  ShutdownOutcome,
  report::Reporter,
  state::{RunnerEvent, RunnerFailure, RunnerState},
};

/// How long the runner may take to exit after it reported `Stopped` and the connection closed.
const EXIT_TIMEOUT: Duration = Duration::from_secs(2);
/// How long the "runner reconnected" notice stays.
const RECONNECTED_NOTICE: Duration = Duration::from_secs(5);

enum Control {
  Stop { reply: oneshot::Sender<ShutdownOutcome> },
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
  /// Starts the runner for `project` once `previous` (another project's runner) has stopped.
  pub(super) fn spawn(
    runtime: &Handle,
    runner_exe: PathBuf,
    project: PathBuf,
    reporter: Reporter,
    previous: Option<Connection>,
  ) -> Self {
    let (control, requests) = mpsc::unbounded_channel();
    let task = runtime.spawn(async move {
      if let Some(previous) = previous {
        previous.stop(Some(super::QUIET_STOP_TIMEOUT)).await;
      }
      Supervisor {
        runner_exe,
        project,
        reporter,
        requests,
      }
      .run()
      .await;
    });
    Self { control, task }
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
  project: PathBuf,
  reporter: Reporter,
  requests: mpsc::UnboundedReceiver<Control>,
}

impl Supervisor {
  async fn run(mut self) {
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
      let Some(state) = self.reporter.report(RunnerEvent::Launched) else {
        return;
      };
      if state == (RunnerState::Online { reconnected: true }) {
        self.expire_reconnected_notice();
      }
      match Session::start(client).serve(&mut self.requests, &self.reporter).await {
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
    let launch = RunnerClient::launch(&self.runner_exe, &self.project);
    let result = tokio::select! {
      result = launch => result,
      request = self.requests.recv() => {
        self.answer_stop_while_idle(request);
        return None;
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
    tokio::select! {
      () = tokio::time::sleep(delay) => true,
      request = self.requests.recv() => {
        self.answer_stop_while_idle(request);
        false
      }
    }
  }

  /// A stop requested while no runner is connected: there is nothing to shut down. `None`
  /// means the desktop dropped the connection handle.
  fn answer_stop_while_idle(&self, request: Option<Control>) {
    let Some(Control::Stop { reply }) = request else {
      return;
    };
    self.reporter.report(RunnerEvent::StopRequested);
    self.reporter.report(RunnerEvent::Stopped);
    // The desktop may have stopped waiting (close timeout); the result no longer matters then.
    let _ = reply.send(ShutdownOutcome::NoRunner);
  }

  fn expire_reconnected_notice(&self) {
    let reporter = self.reporter.clone();
    tokio::spawn(async move {
      tokio::time::sleep(RECONNECTED_NOTICE).await;
      reporter.report(RunnerEvent::ReconnectNoticeExpired);
    });
  }
}

fn launch_failure(error: &LaunchError, runner_exe: &Path) -> RunnerFailure {
  match error {
    LaunchError::Spawn { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
      RunnerFailure::MissingExecutable {
        path: runner_exe.to_path_buf(),
      }
    }
    LaunchError::ForeignPipeServer { .. } | LaunchError::Ipc(IpcError::EndpointInUse { .. }) => {
      RunnerFailure::AlreadyServed
    }
    _ => RunnerFailure::StartFailed,
  }
}

type EventItem = Result<Option<Event>, IpcError>;

/// Aborts a helper task when its owner goes away.
struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
  fn drop(&mut self) {
    self.0.abort();
  }
}

enum Served {
  Dropped(RunnerFailure),
  Stopped,
  Abandoned,
}

/// A connected runner. Events are read on their own task because a frame read cannot be
/// interrupted without losing data.
struct Session {
  commands: CommandSender,
  events: mpsc::UnboundedReceiver<EventItem>,
  _reader: AbortOnDrop,
  process: RunnerProcess,
}

impl Session {
  fn start(client: RunnerClient) -> Self {
    let (commands, mut receiver, process) = client.into_parts();
    let (sender, events) = mpsc::unbounded_channel();
    let reader = tokio::spawn(async move {
      loop {
        let item = receiver.next().await;
        let last = !matches!(item, Ok(Some(ref event)) if !matches!(event, Event::Stopped { .. }));
        if sender.send(item).is_err() || last {
          return;
        }
      }
    });
    Self {
      commands,
      events,
      _reader: AbortOnDrop(reader),
      process,
    }
  }

  async fn serve(mut self, requests: &mut mpsc::UnboundedReceiver<Control>, reporter: &Reporter) -> Served {
    loop {
      tokio::select! {
        item = self.events.recv() => {
          if !continues(item) {
            return Served::Dropped(RunnerFailure::Lost);
          }
        }
        status = self.process.wait() => {
          tracing::warn!(?status, "the runner exited");
          return Served::Dropped(RunnerFailure::Lost);
        }
        request = requests.recv() => {
          let Some(Control::Stop { reply }) = request else {
            return Served::Abandoned;
          };
          reporter.report(RunnerEvent::StopRequested);
          let outcome = self.stop(reporter).await;
          // The desktop may have stopped waiting (close timeout); the result no longer matters then.
          let _ = reply.send(outcome);
          return Served::Stopped;
        }
      }
    }
  }

  /// The graceful close from `orchester_ipc`'s client: `Shutdown`, wait for `Stopped`, close
  /// the connection, wait for the process to exit.
  async fn stop(mut self, reporter: &Reporter) -> ShutdownOutcome {
    let shutdown = Command::Shutdown {
      mode: ShutdownMode::FinishAtCheckpoints,
    };
    if let Err(error) = self.commands.send(&shutdown).await {
      tracing::warn!(%error, "cannot ask the runner to stop");
      reporter.report(RunnerEvent::Dropped(RunnerFailure::Lost));
      return ShutdownOutcome::Unanswered;
    }
    loop {
      match self.events.recv().await {
        Some(Ok(Some(Event::Stopped { .. }))) => break,
        item @ Some(Ok(Some(_))) => {
          continues(item);
        }
        item => {
          continues(item);
          reporter.report(RunnerEvent::Dropped(RunnerFailure::Lost));
          return ShutdownOutcome::Unanswered;
        }
      }
    }
    let Self {
      commands,
      events,
      _reader,
      mut process,
    } = self;
    drop((commands, events, _reader));
    match tokio::time::timeout(EXIT_TIMEOUT, process.wait()).await {
      Ok(Ok(status)) => tracing::info!(%status, "the runner stopped"),
      Ok(Err(error)) => tracing::warn!(%error, "cannot wait for the runner to exit; terminating it"),
      Err(_) => tracing::warn!("the runner did not exit within {EXIT_TIMEOUT:?} after stopping; terminating it"),
    }
    reporter.report(RunnerEvent::Stopped);
    ShutdownOutcome::Stopped
  }
}

/// Logs an event from the runner; `false` once the connection is over.
fn continues(item: Option<EventItem>) -> bool {
  match item {
    Some(Ok(Some(Event::Stopped { mode }))) => {
      tracing::warn!(?mode, "the runner stopped without being asked");
      false
    }
    Some(Ok(Some(event))) => {
      tracing::debug!(?event, "runner event");
      true
    }
    Some(Ok(None)) | None => {
      tracing::warn!("the runner closed the connection");
      false
    }
    Some(Err(error)) => {
      tracing::warn!(%error, "the runner connection failed");
      false
    }
  }
}
