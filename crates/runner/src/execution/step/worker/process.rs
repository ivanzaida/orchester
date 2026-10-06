//! The agent CLI's side of a worker: the process's output read through Claude Code's stream or
//! Codex App Server, the run's stops carried out at the right moment, and how the invocation ended
//! once the process exited.

use std::time::Duration;

use orchester_core::{
  connection::{
    app_server::Update,
    stream::{AgentEvent, InvocationOutcome, OutputStream, RunnerProblem},
  },
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::StoreError;
use tokio::time::{Instant, sleep};

use super::{
  super::{
    app_server::{AppServerFailure, AppServerSession},
    approval::Awaiting,
    claude_approval::ClaudeApprovals,
    relay::Listening,
  },
  Driver, FLUSH_INTERVAL, Finished, Stop, StopCause, TURN_GRACE, Transport, Watch, Worker, review,
};
use crate::supervisor::{ProcessEvent, ProcessExit};

/// The next event of a CLI's process; a native driver has none.
async fn next_event(driver: &mut Driver) -> Option<ProcessEvent> {
  match driver {
    Driver::Process { process, .. } => process.next().await,
    Driver::Native(_) => std::future::pending().await,
  }
}

impl Worker {
  pub(super) async fn follow(mut self) -> Result<Finished, StoreError> {
    let mut watch = Watch::open(&self.root, &self.invocation)?;
    self.initialize_app_server(&mut watch).await?;
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    let grace = sleep(Duration::ZERO);
    tokio::pin!(grace);
    let mut grace_armed = false;
    let mut listening = true;
    let mut deciding = self.app_server().is_some() || self.claude_approvals.is_some();
    let mut steering = true;
    loop {
      tokio::select! {
        event = next_event(&mut self.driver) => match event {
          Some(ProcessEvent::Output { stream, line }) => {
            self.receive_output(&mut watch, stream, &line).await?;
            self.steer_early(&mut watch).await?;
            self.stop_at_checkpoint(&mut watch);
            if watch.stop.is_some() {
              self.driver.end();
            }
          }
          Some(ProcessEvent::Exited(exit)) => return self.finish(watch, &exit),
          None => return self.finish(watch, &ProcessExit::WaitFailed { reason: String::new() }),
        },
        _ = flush.tick() => self.flush(&mut watch)?,
        decision = self.decisions.recv(), if deciding => match decision {
          Some(decision) => self.decide(&mut watch, decision).await?,
          None => deciding = false,
        },
        messages = self.steers.recv(), if steering => match messages {
          Some(messages) => self.steer(&mut watch, messages).await?,
          None => steering = false,
        },
        Some(asked) = ClaudeApprovals::next(self.claude_approvals.as_mut()) => {
          self.request_claude_approval(&mut watch, asked).await?;
          self.stop_at_checkpoint(&mut watch);
        },
        Some(asked) = Listening::next(self.tool_relay.as_mut()) => {
          self.request_claude_approval(&mut watch, asked).await?;
          self.stop_at_checkpoint(&mut watch);
        },
        reviewed = review::next(&mut self.reviewing), if self.reviewing.is_some() => {
          self.apply_review(&mut watch, reviewed).await?;
        },
        // Ending the agent ends its prompt too: once it is stopping, a closed prompt is expected.
        () = watch.prompt_closed(), if watch.stop.is_none() => self.prompt_closed(&mut watch)?,
        stop = self.stops.recv(), if listening => match stop {
          Some(stop) => {
            if self.receive_stop(&mut watch, stop) {
              grace.as_mut().reset(Instant::now() + TURN_GRACE);
              grace_armed = true;
            }
          }
          None => listening = false,
        },
        () = &mut grace, if grace_armed && watch.stop.is_none() => {
          watch.stop = watch.over;
          self.driver.end();
        }
      }
    }
  }

  /// Carries out what the run tells the agent; whether its turn now has [`TURN_GRACE`] to end on its own.
  fn receive_stop(&mut self, watch: &mut Watch, stop: Stop) -> bool {
    match stop {
      Stop::Now(cause) => {
        if watch.stop.is_none() {
          watch.stop = Some(cause);
          self.driver.end();
        }
      }
      Stop::AfterTurn(cause) => {
        if watch.over.is_none() && watch.stop.is_none() {
          watch.over = Some(cause);
          return true;
        }
      }
      Stop::AfterCommand(cause) => {
        if watch.stop.is_none() && watch.over.is_none() {
          watch.after_command = Some(cause);
          self.stop_at_checkpoint(watch);
        }
      }
      Stop::AtApproval => {
        watch.pause_requested = true;
        self.stop_at_checkpoint(watch);
      }
      Stop::Withdraw => watch.after_command = None,
    }
    false
  }

  async fn receive_output(&mut self, watch: &mut Watch, stream: OutputStream, line: &str) -> Result<(), StoreError> {
    let received = match &mut self.driver {
      Driver::Process {
        transport: Transport::AppServer(app),
        process,
      } => app.receive(stream, line, process).await,
      Driver::Process {
        transport: Transport::Claude(claude),
        ..
      } => Ok(claude.parse(stream, line).into_iter().map(Update::Activity).collect()),
      Driver::Native(_) => Ok(Vec::new()),
    };
    self.took_input(watch)?;
    match received {
      Ok(updates) => {
        for update in updates {
          self.app_update(watch, update).await?;
        }
      }
      Err(error) => {
        let event = match error {
          AppServerFailure::Protocol(message) => AgentEvent::TurnFailed { message },
          AppServerFailure::NotSignedIn(message) => AgentEvent::NotSignedIn { message },
        };
        self.observe(watch, event)?;
        self.driver.end();
      }
    }
    Ok(())
  }

  fn stop_at_checkpoint(&mut self, watch: &mut Watch) {
    if watch.stop.is_some() || watch.over.is_some() {
      return;
    }
    let pending_safe = watch.approval.as_ref().is_some_and(|(_, awaiting)| {
      watch.commands.iter().all(|item| item == awaiting.item())
        && !matches!((awaiting, self.app_server()), (Awaiting::Codex(request), Some(app)) if app.has_other_active_tools(request))
    });
    if watch.after_command.is_some() && (self.checkpoint_safe(watch) || pending_safe) {
      watch.stop = watch.after_command.take();
    } else if watch.pause_requested && pending_safe {
      watch.stop = Some(StopCause::Paused);
    }
    if watch.stop.is_some() {
      self.driver.end();
    }
  }

  /// What the CLI said of the messages sent now: Codex's answers to them and the ones Claude Code
  /// took. A turn Claude Code ended closes its input: it exits once it has read what it was given.
  fn took_input(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    match &mut self.driver {
      Driver::Process {
        transport: Transport::AppServer(app),
        ..
      } => {
        let answers = app.take_steer_answers();
        self.steer_answered(watch, answers)
      }
      Driver::Process {
        transport: Transport::Claude(claude),
        process,
      } => {
        let (taken, results) = (claude.take_replayed(), claude.results());
        if results > 0 {
          process.close_input();
        }
        for text in taken {
          self.input_taken(watch, &text, Some(results))?;
        }
        Ok(())
      }
      Driver::Native(_) => Ok(()),
    }
  }

  async fn initialize_app_server(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    if let Driver::Process {
      transport: Transport::AppServer(app),
      process,
    } = &self.driver
      && let Err(error) = app.start(process).await
    {
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("cannot initialize Codex App Server: {error}"),
        },
      )?;
      self.driver.end();
    }
    Ok(())
  }

  fn checkpoint_safe(&self, watch: &Watch) -> bool {
    watch.commands.is_empty() && !self.app_server().is_some_and(AppServerSession::has_active_tools)
  }

  async fn app_update(&mut self, watch: &mut Watch, update: Update) -> Result<(), StoreError> {
    match update {
      Update::Activity(event) => self.observe(watch, event),
      Update::Approval(request) => self.request_approval(watch, request).await,
      Update::Declined(response) => {
        // A process that cannot take the answer is ending; its exit says how.
        if let Some(process) = self.driver.process()
          && let Err(error) = process.send_line(response).await
        {
          tracing::warn!(%error, "Codex App Server did not take a declined request's answer");
        }
        Ok(())
      }
      Update::InputTaken(text) => self.input_taken(watch, &text, None),
      Update::TurnEnded { input_kept } => {
        if input_kept {
          self.kept_at_turn_end(watch)?;
        }
        self.driver.end();
        Ok(())
      }
    }
  }

  /// Ends the activity with how many lines of Codex's standard error it did not keep, if any.
  fn note_stderr_left_out(&self, watch: &mut Watch) -> Result<(), StoreError> {
    match self.app_server().map(AppServerSession::stderr_left_out) {
      Some(lines) if lines > 0 => self.observe(
        watch,
        AgentEvent::RunnerProblem {
          problem: RunnerProblem::StderrLeftOut { lines },
        },
      ),
      _ => Ok(()),
    }
  }

  fn finish(&self, mut watch: Watch, exit: &ProcessExit) -> Result<Finished, StoreError> {
    let mut approval_interrupted = false;
    self.check_prompt_reached(&mut watch)?;
    self.note_stderr_left_out(&mut watch)?;
    let waiting = watch.approval.take();
    // A request the run's own stop ended (a pause at the decision, a cancel) needs no naming.
    if let Some((approval, _)) = waiting.as_ref().filter(|_| watch.stop.is_none()) {
      self.report_interrupted(&mut watch, approval)?;
    }
    if waiting.is_some() || !watch.approved_calls.is_empty() || self.app_server().is_some_and(|app| !app.complete()) {
      approval_interrupted = watch
        .db
        .interrupt_tool_approvals(watch.invocation.run, watch.invocation.id)?;
      watch.invocation.timing.resume(Timestamp::now());
      self.notifier.send(Event::ToolApprovalChanged {
        run: watch.invocation.run,
        task: self.task,
      });
    }
    self.flush(&mut watch)?;
    let reported = watch.tracker.outcome(Some(1));
    let outcome = if watch.stop.is_none()
      && matches!(
        reported,
        InvocationOutcome::TurnFailed { .. } | InvocationOutcome::NotSignedIn { .. }
      ) {
      reported
    } else {
      match (
        exit,
        watch.stop.is_some(),
        self.app_server().is_some_and(AppServerSession::complete),
      ) {
        (_, false, true) => watch.tracker.outcome(Some(0)),
        (ProcessExit::Terminated, ..) | (_, true, _) => InvocationOutcome::Terminated,
        (ProcessExit::Exited { code }, false, _) => watch.tracker.outcome(*code),
        (ProcessExit::WaitFailed { .. }, false, _) => InvocationOutcome::ExitFailed { code: None },
      }
    };
    watch.invocation.ended_at = Some(Timestamp::now());
    watch.invocation.outcome = Some(outcome.clone());
    watch.db.save_invocation(&watch.invocation)?;
    // Only while that close is still why the turn failed: a later CLI error is a failure of its own.
    let closed_after_result = watch
      .closed_after_result
      .as_ref()
      .is_some_and(|closed| matches!(&outcome, InvocationOutcome::TurnFailed { message } if message == closed));
    Ok(Finished {
      outcome,
      stopped: watch.stop,
      final_message: watch.tracker.last_message().map(str::to_owned),
      approval_interrupted,
      closed_after_result,
    })
  }
}
