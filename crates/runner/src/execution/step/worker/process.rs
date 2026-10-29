//! The agent CLI's side of a worker: the process's output read through Claude Code's stream or
//! Codex App Server, the run's stops carried out at the right moment, and how the invocation ended
//! once the process exited.

use std::time::Duration;

use orchester_core::{
  connection::{
    app_server::Update,
    mcp::McpServerProblem,
    stream::{AgentEvent, InvocationOutcome, OutputStream, RunnerProblem},
  },
  orchester_tools::SERVER,
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
    relay::{CODEX_RELAYS, Listening, Relayed},
  },
  Driver, FLUSH_INTERVAL, Finished, RelayClosed, Stop, StopCause, TURN_GRACE, Transport, Watch, Worker, review,
};
use crate::supervisor::{ProcessEvent, ProcessExit};

/// Continuations of a Codex turn in a row without a decision settled or a call returned that the runner waited for
/// after a turn: a safety net under the progress each continuation needs ([`Worker::tools_outlive_turn`]).
const MAX_IDLE_CONTINUATIONS: u8 = 3;

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
    loop {
      tokio::select! {
        event = next_event(&mut self.driver) => match event {
          Some(ProcessEvent::Output { stream, line }) => self.output(&mut watch, stream, &line).await?,
          Some(ProcessEvent::Exited(exit)) => return self.finish(watch, &exit),
          None => return self.finish(watch, &ProcessExit::WaitFailed { reason: String::new() }),
        },
        _ = flush.tick() => self.tick(&mut watch).await?,
        decision = self.decisions.recv(), if deciding => match decision {
          Some(decision) => self.decide(&mut watch, decision).await?,
          None => deciding = false,
        },
        // A closed channel reads `None`, which leaves the branch out.
        Some(given) = self.answers.recv() => self.answer_question(&mut watch, given).await?,
        Some(messages) = self.steers.recv() => self.steer(&mut watch, messages).await?,
        Some(relayed) = ClaudeApprovals::next(self.claude_approvals.as_mut()) => self.relayed(&mut watch, relayed).await?,
        Some(relayed) = Listening::next(self.tool_relay.as_mut()) => self.relayed(&mut watch, relayed).await?,
        reviewed = review::next(&mut self.reviewing), if self.reviewing.is_some() => {
          self.apply_review(&mut watch, reviewed).await?;
        },
        // Ending the agent ends its prompt too: once it is stopping, a closed prompt is expected.
        closed = watch.relay_closed(), if watch.stop.is_none() => self.relay_closed(&mut watch, closed)?,
        stop = self.stops.recv(), if listening => match stop {
          Some(stop) => grace_armed |= self.stop_with_grace(&mut watch, stop, grace.as_mut()).await?,
          None => listening = false,
        },
        () = &mut grace, if grace_armed && watch.stop.is_none() => {
          watch.stop = watch.over;
          self.driver.end();
        }
      }
    }
  }

  /// Carries out `stop`; when its turn now has [`TURN_GRACE`] to end on its own, `grace` ends it then. Whether it
  /// does.
  async fn stop_with_grace(
    &mut self,
    watch: &mut Watch,
    stop: Stop,
    grace: std::pin::Pin<&mut tokio::time::Sleep>,
  ) -> Result<bool, StoreError> {
    let armed = self.receive_stop(watch, stop).await?;
    if armed {
      grace.reset(Instant::now() + TURN_GRACE);
    }
    Ok(armed)
  }

  /// A line the CLI wrote: what it says, the messages sent now it can take, and a stop that waited for it.
  async fn output(&mut self, watch: &mut Watch, stream: OutputStream, line: &str) -> Result<(), StoreError> {
    let commands = watch.commands.len();
    self.receive_output(watch, stream, line).await?;
    self.steer_early(watch).await?;
    self.stop_at_checkpoint(watch);
    if watch.stop.is_some() {
      self.driver.end();
    }
    if watch.commands.len() < commands {
      self.continue_after_tools(watch).await?;
    }
    Ok(())
  }

  /// A call the relay holds for a person: a decision, or an answer to `ask_question`.
  async fn relayed(&mut self, watch: &mut Watch, relayed: Relayed) -> Result<(), StoreError> {
    match relayed {
      Relayed::Approval(asked) => self.request_claude_approval(watch, asked).await?,
      Relayed::Question(asked) => self.request_relay_question(watch, asked)?,
      Relayed::Returned => {
        // A call returned while Codex waited after its turn: work went on, which a continuation may collect.
        if watch.tools_after_turn {
          watch.idle_continuations = 0;
        }
        self.continue_after_tools(watch).await?;
      }
    }
    self.stop_at_checkpoint(watch);
    Ok(())
  }

  /// The relay closed while `closed`, a call it held for a person, waited.
  fn relay_closed(&mut self, watch: &mut Watch, closed: RelayClosed) -> Result<(), StoreError> {
    match closed {
      RelayClosed::Decision => self.prompt_closed(watch),
      RelayClosed::Question => self.question_closed(watch),
    }
  }

  /// Carries out what the run tells the agent; whether its turn now has [`TURN_GRACE`] to end on its own.
  async fn receive_stop(&mut self, watch: &mut Watch, stop: Stop) -> Result<bool, StoreError> {
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
          return Ok(true);
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
      Stop::Withdraw => {
        watch.after_command = None;
        self.continue_after_tools(watch).await?;
      }
    }
    Ok(false)
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
    if self.app_server().is_some_and(AppServerSession::servers_checked)
      && let Some(relay) = self.tool_relay.as_mut()
    {
      relay.stop_accepting();
    }
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
    let approval_safe = watch.approval.as_ref().is_some_and(|(_, awaiting)| {
      watch.commands.iter().all(|item| item == awaiting.item())
        && !matches!((awaiting, self.app_server()), (Awaiting::Codex(request), Some(app)) if app.has_other_active_tools(request))
    });
    // A question waits like a tool decision: the agent can stop there once nothing else runs.
    let pending_safe = approval_safe || (watch.question.is_some() && self.checkpoint_safe(watch));
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
      Update::Question(pending) => self.request_codex_question(watch, pending),
      Update::QuestionRefused {
        response,
        question,
        reason,
      } => self.refuse_question(watch, response, (question, reason)).await,
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
      Update::TurnEnded { input_kept, completed } => {
        if input_kept {
          self.kept_at_turn_end(watch)?;
        }
        // A failed turn (a usage limit, for one) or an interrupted one ends the step, whatever its tools still do.
        if completed && self.tools_outlive_turn(watch) {
          watch.tools_after_turn = true;
          if let Some(app) = self.app_server_mut() {
            app.tools_outlived_turn();
          }
          self.continue_after_tools(watch).await?;
        } else {
          self.driver.end();
        }
        Ok(())
      }
    }
  }

  /// Whether Orchester's tools outlive the Codex turn that ended: a call or command still runs, a decision still waits
  /// (a call of Orchester's tools, or one a yielded cell asked of Codex), or a call returned since Codex was last
  /// continued and its result may not be collected yet. Calls Codex still shows open are not enough on their own: it
  /// may never complete one it abandoned, and each continuation is a model turn.
  fn tools_outlive_turn(&self, watch: &Watch) -> bool {
    let Some(relay) = self.tool_relay.as_ref() else {
      return false;
    };
    relay.busy()
      || !watch.commands.is_empty()
      || watch.approval.is_some()
      || (relay.returns() > watch.returns_continued
        && self
          .app_server()
          .is_some_and(AppServerSession::has_orchester_results_to_collect))
  }

  /// Collects a yielded call in the same Codex thread once the relay has returned it and no command or decision waits:
  /// a continuation while a yielded cell's decision waits could end its turn, or ask another, before the person
  /// answers. Called again once the decision is settled or its command finishes. Apply now waits for the safe
  /// checkpoint before another invocation takes the new permissions. A stop, a reached budget or a requested pause ends
  /// the agent instead (a pause checkpoints it), as does the [`MAX_IDLE_CONTINUATIONS`] cap.
  pub(in super::super) async fn continue_after_tools(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    if !watch.tools_after_turn
      || watch.approval.is_some()
      || !watch.commands.is_empty()
      || watch.after_command.is_some()
      || self.tool_relay.as_ref().is_some_and(Listening::busy)
    {
      return Ok(());
    }
    watch.tools_after_turn = false;
    let stop = watch
      .stop
      .or(watch.over)
      .or_else(|| watch.pause_requested.then_some(StopCause::Paused));
    if stop.is_some() {
      watch.stop = stop;
      self.driver.end();
      return Ok(());
    }
    if watch.idle_continuations >= MAX_IDLE_CONTINUATIONS {
      tracing::warn!(
        invocation = %watch.invocation.id,
        "Codex was continued {MAX_IDLE_CONTINUATIONS} times without a decision or a call it waited for; the step ends"
      );
      self.driver.end();
      return Ok(());
    }
    watch.idle_continuations += 1;
    watch.returns_continued = self.tool_relay.as_ref().map_or(0, Listening::returns);
    if let Driver::Process {
      transport: Transport::AppServer(app),
      process,
    } = &mut self.driver
      && let Err(error) = app.continue_tools(process).await
    {
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("cannot continue Codex after a tool result: {error}"),
        },
      )?;
      self.driver.end();
    }
    Ok(())
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

  /// Codex's inventory was checked, but fewer relays of Orchester's tools than Codex starts reached the runner:
  /// the agent may have worked without them, which the activity says.
  fn note_tool_relays_missing(&self, watch: &mut Watch) -> Result<(), StoreError> {
    let taken = self.tool_relay.as_ref().map_or(CODEX_RELAYS, Listening::taken);
    if taken >= CODEX_RELAYS || !self.app_server().is_some_and(AppServerSession::servers_checked) {
      return Ok(());
    }
    self.observe(
      watch,
      AgentEvent::McpServer {
        server: Some(SERVER.to_owned()),
        problem: McpServerProblem::ToolRelaysMissing {
          reached: taken,
          started: CODEX_RELAYS,
        },
      },
    )
  }

  fn finish(&self, mut watch: Watch, exit: &ProcessExit) -> Result<Finished, StoreError> {
    let mut approval_interrupted = false;
    self.check_prompt_reached(&mut watch)?;
    self.note_tool_relays_missing(&mut watch)?;
    self.note_stderr_left_out(&mut watch)?;
    self.interrupt_questions(&mut watch)?;
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
    // A turn the person stopped shows so in the conversation, after what the agent did until then.
    if watch.stop == Some(StopCause::TurnStopped) {
      self.observe(&mut watch, AgentEvent::TurnStopped)?;
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
        watch.stop,
        self.app_server().is_some_and(AppServerSession::complete),
      ) {
        (_, Some(StopCause::TurnStopped), _) => InvocationOutcome::TurnStopped,
        (_, None, true) => watch.tracker.outcome(Some(0)),
        (ProcessExit::Terminated, ..) | (_, Some(_), _) => InvocationOutcome::Terminated,
        (ProcessExit::Exited { code }, None, _) => watch.tracker.outcome(*code),
        (ProcessExit::WaitFailed { .. }, None, _) => InvocationOutcome::ExitFailed { code: None },
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
      stopped: Self::stopped(&watch, &outcome),
      outcome,
      final_message: watch.tracker.last_message().map(str::to_owned),
      approval_interrupted,
      closed_after_result,
    })
  }
}
