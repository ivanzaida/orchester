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
use orchester_ipc::{
  AttachSession, Command, CommandSender, Event, IpcError, LaunchError, Reply, RunnerClient, RunnerProcess, ShutdownMode,
};
use tokio::{
  runtime::Handle,
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

use super::{
  ShutdownOutcome,
  report::Reporter,
  requests::{Pending, ReplySender, RequestError},
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
  /// Starts the runner for `project`. Its first attach is for `session`; every runner started
  /// after a drop attaches as a reconnect.
  pub(super) fn spawn(
    runtime: &Handle,
    runner_exe: PathBuf,
    project: PathBuf,
    reporter: Reporter,
    session: AttachSession,
  ) -> Self {
    let (control, requests) = mpsc::unbounded_channel();
    let task = runtime.spawn(
      Supervisor {
        runner_exe,
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
      match Session::start(client)
        .serve((session, agent_slots), &mut self.requests, &self.reporter)
        .await
      {
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
    let mut launch = pin!(RunnerClient::launch(&self.runner_exe, &self.project));
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

/// How the runner answered [`Command::StopIfIdle`].
enum IdleAnswer {
  Stopping,
  Busy,
  Dropped,
}

/// A connected runner. Events are read on their own task because a frame read cannot be
/// interrupted without losing data.
struct Session {
  commands: CommandSender,
  events: mpsc::UnboundedReceiver<EventItem>,
  _reader: AbortOnDrop,
  process: RunnerProcess,
  pending: Pending,
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
      pending: Pending::default(),
    }
  }

  async fn serve(
    mut self,
    (session, agent_slots): (AttachSession, NonZeroU32),
    requests: &mut mpsc::UnboundedReceiver<Control>,
    reporter: &Reporter,
  ) -> Served {
    let (reply, mut attached) = oneshot::channel();
    let attach = Command::Attach { session, agent_slots };
    if !self.send(attach, Some(reply)).await {
      return Served::Dropped(RunnerFailure::Lost);
    }
    let mut attaching = true;
    loop {
      tokio::select! {
        answer = &mut attached, if attaching => {
          attaching = false;
          match answer {
            Ok(Ok(Reply::Attached { continued, .. })) => {
              reporter.attached(session == AttachSession::Reconnect, continued.len());
            }
            other => {
              tracing::warn!(?other, "the runner did not attach");
              return Served::Dropped(RunnerFailure::Lost);
            }
          }
        }
        item = self.events.recv() => {
          if !self.handle(item, reporter) {
            return Served::Dropped(RunnerFailure::Lost);
          }
        }
        status = self.process.wait() => {
          tracing::warn!(?status, "the runner exited");
          return Served::Dropped(RunnerFailure::Lost);
        }
        request = requests.recv() => match request {
          Some(Control::Request { command, reply }) => {
            if !self.send(command, Some(reply)).await {
              return Served::Dropped(RunnerFailure::Lost);
            }
          }
          Some(Control::Stop { reply }) => {
            reporter.report(RunnerEvent::StopRequested);
            let outcome = self.stop(reporter).await;
            // The desktop may have stopped waiting (close timeout); the result no longer matters then.
            let _ = reply.send(outcome);
            return Served::Stopped;
          }
          // The lingering task may have been cancelled meanwhile; the answer no longer matters then.
          Some(Control::StopIfIdle { reply }) => match self.stop_if_idle(reporter).await {
            IdleAnswer::Busy => {
              let _ = reply.send(false);
            }
            IdleAnswer::Dropped => {
              let _ = reply.send(false);
              return Served::Dropped(RunnerFailure::Lost);
            }
            IdleAnswer::Stopping => {
              reporter.report(RunnerEvent::StopRequested);
              let stopped = self.stopped(reporter).await == ShutdownOutcome::Stopped;
              let _ = reply.send(stopped);
              return Served::Stopped;
            }
          },
          None => return Served::Abandoned,
        }
      }
    }
  }

  /// Sends a request; `false` when the connection failed.
  async fn send(&mut self, command: Command, reply: Option<ReplySender>) -> bool {
    let request = self.pending.request(command, reply);
    match self.commands.send(&request).await {
      Ok(()) => true,
      Err(error) => {
        tracing::warn!(%error, "cannot send a request to the runner");
        self.pending.fail(request.id);
        false
      }
    }
  }

  /// Handles an event from the runner; `false` once the connection is over.
  fn handle(&mut self, item: Option<EventItem>, reporter: &Reporter) -> bool {
    match item {
      Some(Ok(Some(Event::PlanLimits(reading)))) => {
        reporter.plan_limits(reading);
        true
      }
      Some(Ok(Some(Event::Reply { id, reply }))) => {
        self.pending.answer(id, reply);
        true
      }
      Some(Ok(Some(Event::RunChanged { run, task }))) => {
        reporter.event_of(run);
        reporter.run_changed();
        reporter.task_run_changed(task);
        true
      }
      Some(Ok(Some(Event::TaskGroupChanged { .. }))) => {
        reporter.run_changed();
        true
      }
      Some(Ok(Some(Event::ToolApprovalChanged { .. } | Event::QuestionChanged { .. }))) => {
        reporter.run_changed();
        true
      }
      Some(Ok(Some(Event::ChatChanged { .. }))) => {
        reporter.chat_changed();
        true
      }
      Some(Ok(Some(Event::AgentActivity { run, .. }))) => {
        reporter.event_of(run);
        reporter.transcript_grew();
        true
      }
      Some(Ok(Some(Event::WorkSummary(summary)))) => {
        reporter.work(summary);
        true
      }
      Some(Ok(Some(Event::PlanProposed {
        task,
        task_key,
        version,
        ..
      }))) => {
        reporter.plan_proposed(task, &task_key, version);
        true
      }
      Some(Ok(Some(Event::RunWaitsForPerson {
        task, task_key, step, ..
      }))) => {
        reporter.run_waits(task, &task_key, &step);
        true
      }
      Some(Ok(Some(Event::RunReady { task, number, .. }))) => {
        reporter.run_ready(task, number);
        true
      }
      Some(Ok(Some(Event::QuestionAsked {
        task, task_key, agent, ..
      }))) => {
        reporter.question_asked(task, &task_key, &agent);
        true
      }
      Some(Ok(Some(Event::Stopped { mode }))) => {
        tracing::warn!(?mode, "the runner stopped without being asked");
        false
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

  /// Asks the runner to stop if it has no work, handling its events until it answers.
  async fn stop_if_idle(&mut self, reporter: &Reporter) -> IdleAnswer {
    let (reply, mut answer) = oneshot::channel();
    if !self.send(Command::StopIfIdle, Some(reply)).await {
      return IdleAnswer::Dropped;
    }
    loop {
      // The answer first: a runner that answered Stopping sends Stopped right behind the reply, and reading that
      // here before the answer would take the runner's stop for a dropped connection.
      tokio::select! {
        biased;
        answered = &mut answer => return match answered {
          Ok(Ok(Reply::Stopping)) => IdleAnswer::Stopping,
          Ok(Ok(Reply::Rejected(_))) => IdleAnswer::Busy,
          other => {
            tracing::warn!(?other, "unexpected answer to StopIfIdle");
            IdleAnswer::Dropped
          }
        },
        item = self.events.recv() => {
          if !self.handle(item, reporter) {
            return IdleAnswer::Dropped;
          }
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
    if !self.send(shutdown, None).await {
      reporter.report(RunnerEvent::Dropped(RunnerFailure::Lost));
      return ShutdownOutcome::Unanswered;
    }
    self.stopped(reporter).await
  }

  /// After the runner was asked to stop: wait for `Stopped`, close the connection, wait for the
  /// process to exit.
  async fn stopped(mut self, reporter: &Reporter) -> ShutdownOutcome {
    loop {
      match self.events.recv().await {
        Some(Ok(Some(Event::Stopped { .. }))) => break,
        item @ Some(Ok(Some(_))) => {
          self.handle(item, reporter);
        }
        item => {
          self.handle(item, reporter);
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
      pending,
    } = self;
    drop((commands, events, _reader, pending));
    match tokio::time::timeout(EXIT_TIMEOUT, process.wait()).await {
      Ok(Ok(status)) => tracing::info!(%status, "the runner stopped"),
      Ok(Err(error)) => tracing::warn!(%error, "cannot wait for the runner to exit; terminating it"),
      Err(_) => tracing::warn!("the runner did not exit within {EXIT_TIMEOUT:?} after stopping; terminating it"),
    }
    reporter.report(RunnerEvent::Stopped);
    ShutdownOutcome::Stopped
  }
}
