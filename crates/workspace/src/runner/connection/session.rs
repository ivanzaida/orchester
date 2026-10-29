//! A connected runner's session: its events read on their own task, the requests sent to it, and how it ends — stopped
//! when idle, stopped, or dropped.

use std::num::NonZeroU32;

use orchester_ipc::{
  AttachSession, Command, CommandSender, Event, Reply, RunnerClient, RunnerProcess, ShutdownMode,
  browser::BrowserCommand,
};
use tokio::{
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

use super::{Control, EXIT_TIMEOUT, EventItem};
use crate::runner::{
  ShutdownOutcome,
  report::Reporter,
  requests::{Pending, ReplySender},
  state::{RunnerEvent, RunnerFailure},
};

/// Aborts a helper task when its owner goes away.
struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
  fn drop(&mut self) {
    self.0.abort();
  }
}

pub(super) enum Served {
  Dropped(RunnerFailure),
  Stopped,
  Abandoned,
}

/// How the runner answered [`Command::StopIfIdle`].
pub(super) enum IdleAnswer {
  Stopping,
  Busy,
  Dropped,
}

/// A connected runner. Events are read on their own task because a frame read cannot be
/// interrupted without losing data.
pub(super) struct Session {
  commands: CommandSender,
  events: mpsc::UnboundedReceiver<EventItem>,
  _reader: AbortOnDrop,
  process: RunnerProcess,
  pending: Pending,
}

impl Session {
  pub(super) fn start(client: RunnerClient) -> Self {
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

  pub(super) async fn serve(
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
              // The project's browser is followed from now on; the runner always takes it.
              if !self.send(Command::Browser(BrowserCommand::Subscribe), None).await {
                return Served::Dropped(RunnerFailure::Lost);
              }
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
      Some(Ok(Some(
        Event::TaskGroupChanged { .. } | Event::ToolApprovalChanged { .. } | Event::QuestionChanged { .. },
      ))) => {
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
      Some(Ok(Some(Event::Browser(event)))) => {
        reporter.browser(event);
        true
      }
      Some(Ok(Some(Event::BrowserFrame(frame)))) => {
        reporter.browser_frame(frame);
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
  pub(super) async fn stop_if_idle(&mut self, reporter: &Reporter) -> IdleAnswer {
    let (reply, mut answer) = oneshot::channel();
    if !self.send(Command::StopIfIdle, Some(reply)).await {
      return IdleAnswer::Dropped;
    }
    loop {
      // The answer first: the reply reaches `answer` through `handle` of this same event channel, and a runner that
      // answered Stopping sends Stopped right behind it, so both branches are ready at once. Reading Stopped before
      // the answer would take the runner's stop for a dropped connection.
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
  pub(super) async fn stop(mut self, reporter: &Reporter) -> ShutdownOutcome {
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
  pub(super) async fn stopped(mut self, reporter: &Reporter) -> ShutdownOutcome {
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
