//! One tool call an agent waits on: the request is recorded, Orchester answers it itself when a rule of
//! the run, Always allow or auto-approval covers it ([`super::answer`]), and otherwise it is shown and
//! the agent's time stops counting while a person decides. The answer goes to exactly the call that
//! asked — a Codex App Server JSON-RPC request, a Claude Code permission prompt call
//! ([`super::claude_approval`]) or a native agent's command ([`super::native`]). A denial tells the agent
//! and lets it continue. A decision that no
//! longer matches the waiting call is refused, so a stale one never approves a later action.

use orchester_core::{
  command::redact_secrets,
  connection::{
    app_server::PendingApproval,
    parse_cli_too_old,
    stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  },
  native::{command::CommandForm, item::ToolCallId},
  run::{NativeCommandAsk, PendingToolApproval},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::StoreError;
use tokio::sync::oneshot;

use super::{
  super::{control::answer, tool_approval::end_unheld},
  answer::Settled,
  claude_approval::{Asked, AskedBy, PromptAnswer},
  native::NativeAsk,
  worker::{ApprovalDecision, Driver, Transport, Watch, Worker},
};

/// Who answers a native agent's command request, as its approval records it.
const COMMAND_SERVER: &str = "command";

/// The call a pending approval answers.
pub(super) enum Awaiting {
  Codex(Box<PendingApproval>),
  /// The permission prompt call `call` (its tool use id), answered through `reply`.
  Claude {
    call: String,
    reply: oneshot::Sender<PromptAnswer>,
  },
  /// A native agent's command, answered through the worker's native driver.
  Native(ToolCallId),
  /// A call `call` of Orchester's tools waiting in the relay, which runs it once approved.
  Tool {
    call: String,
    reply: oneshot::Sender<PromptAnswer>,
  },
}

impl Awaiting {
  /// The tool item waiting for the decision; other items may still be executing.
  pub(super) fn item(&self) -> &str {
    match self {
      Self::Codex(request) => request.item(),
      Self::Claude { call, .. } | Self::Tool { call, .. } => call,
      Self::Native(call) => call.as_str(),
    }
  }
}

/// What a request shows, before it is recorded.
struct Shown {
  command: bool,
  server: String,
  action: String,
  target: Option<String>,
  scope: Option<String>,
  native: Option<NativeCommandAsk>,
}

impl Worker {
  /// The CLI the approvals of this worker come from, for its messages.
  pub(super) const fn cli(&self) -> &'static str {
    if self.app_server().is_some() {
      "Codex"
    } else if self.is_native() {
      "native agent"
    } else {
      "Claude Code"
    }
  }

  /// A native agent's command that a person decides: its shell, full string, working folder and time
  /// limit are what the person sees (04.H9).
  pub(super) async fn request_native_approval(&mut self, watch: &mut Watch, ask: NativeAsk) -> Result<(), StoreError> {
    let NativeAsk { call, command, shell } = ask;
    let shown = Shown {
      command: true,
      server: COMMAND_SERVER.to_owned(),
      action: command.text().to_owned(),
      target: Some(command.workdir().to_string_lossy().into_owned()),
      scope: None,
      native: Some(NativeCommandAsk {
        shell: shell.filter(|_| matches!(command.form(), CommandForm::Shell)),
        time_limit_seconds: command.timeout_seconds(),
      }),
    };
    let key = call.to_string();
    self.await_person(watch, &key, shown, Awaiting::Native(call)).await
  }

  pub(super) async fn request_approval(
    &mut self,
    watch: &mut Watch,
    request: PendingApproval,
  ) -> Result<(), StoreError> {
    let shown = Shown {
      command: request.is_command(),
      server: request.server.clone(),
      action: request.action.clone(),
      target: request.target.clone(),
      scope: request.scope.clone(),
      native: None,
    };
    let key = request.request_key();
    self
      .await_person(watch, &key, shown, Awaiting::Codex(Box::new(request)))
      .await
  }

  /// A permission prompt call of Claude Code, or a call of Orchester's tools the relay holds, that a person
  /// decides. A prompt call asked about before is denied without asking again.
  pub(super) async fn request_claude_approval(&mut self, watch: &mut Watch, asked: Asked) -> Result<(), StoreError> {
    let Asked {
      request,
      reply,
      by,
      arguments,
    } = asked;
    let first = match (by, self.claude_approvals.as_mut()) {
      (AskedBy::ToolCall, _) => true,
      (AskedBy::Prompt, approvals) => approvals.is_some_and(|approvals| approvals.first_ask(request.call())),
    };
    if !first {
      // The relay reads the denial; a closed relay has nothing to deny.
      if reply.send(PromptAnswer::Repeated).is_err() {
        tracing::debug!("the permission prompt closed before a repeated call was denied");
      }
      return Ok(());
    }
    let worktree = self
      .claude_approvals
      .as_ref()
      .map(|approvals| approvals.worktree().to_string_lossy().into_owned());
    let command = request.is_command();
    let shown = Shown {
      command,
      server: request.server().to_owned(),
      action: request.action().to_owned(),
      // A command runs in the session's working folder, the worktree.
      target: if command {
        worktree
      } else {
        request.target().map(str::to_owned)
      },
      scope: request.scope().map(str::to_owned),
      native: None,
    };
    // A Codex call the runner asks about is the call Codex started: one row, whose end says a person was asked.
    let codex_item = match (by, self.app_server_mut()) {
      (AskedBy::ToolCall, Some(app)) => app.runner_asks(request.action(), &arguments),
      _ => None,
    };
    let call = codex_item.unwrap_or_else(|| request.call().to_owned());
    let key = call.clone();
    let awaiting = match by {
      AskedBy::Prompt => Awaiting::Claude { call, reply },
      AskedBy::ToolCall => Awaiting::Tool { call, reply },
    };
    self.await_person(watch, &key, shown, awaiting).await
  }

  pub(super) fn redact(&self, text: &str) -> String {
    match self.app_server() {
      Some(app) => app.redact(text),
      None => redact_secrets(text, &self.secrets),
    }
  }

  /// Claude Code got far enough to start its MCP servers (its session started), but its
  /// permission prompt never reached the runner: every call it asked about was denied as
  /// unavailable, so the step must not pass as if a person had been asked. Reported, and the turn
  /// fails, unless the CLI already failed for its own reason (not signed in, too old for the model).
  pub(super) fn check_prompt_reached(&self, watch: &mut Watch) -> Result<(), StoreError> {
    let unreached = self
      .claude_approvals
      .as_ref()
      .is_some_and(|approvals| !approvals.reached());
    if !unreached || watch.tracker.session().is_none() {
      return Ok(());
    }
    let failed_already = match watch.tracker.outcome(Some(0)) {
      InvocationOutcome::NotSignedIn { .. } => true,
      InvocationOutcome::TurnFailed { message } => parse_cli_too_old(&message).is_some(),
      _ => false,
    };
    if failed_already {
      return Ok(());
    }
    let problem = RunnerProblem::PermissionPromptUnreachable;
    self.observe(watch, AgentEvent::RunnerProblem { problem })?;
    if watch.stop.is_none() {
      let message = "Claude Code's permission prompt never reached Orchester; nothing could be approved".to_owned();
      self.observe(watch, AgentEvent::TurnFailed { message })?;
    }
    Ok(())
  }

  /// Claude Code's permission prompt, or the relay of Orchester's tools, closed its connection while a call
  /// waited: the call can no longer be answered, so the request is interrupted, named in the activity, and
  /// a later decision is refused. The turn fails with it. When Claude Code had already ended its turn with a
  /// result and nothing else had failed, the close is remembered: a question the agent delivered then still
  /// asks the person (`Finished::closed_after_result`).
  pub(super) fn prompt_closed(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    let pending = watch.approval.take();
    watch
      .db
      .interrupt_tool_approvals(watch.invocation.run, watch.invocation.id)?;
    self.end_approval_wait(watch)?;
    if let Some((approval, _)) = &pending {
      self.report_interrupted(watch, approval)?;
    }
    let message = format!(
      "{}'s permission prompt or tool relay closed before the decision",
      self.cli()
    );
    if self.delivered_result(watch) {
      watch.closed_after_result = Some(message.clone());
    }
    self.observe(watch, AgentEvent::TurnFailed { message })?;
    self.driver.end();
    Ok(())
  }

  /// Whether Claude Code ended its turn with a result and nothing failed before.
  fn delivered_result(&self, watch: &Watch) -> bool {
    let ended = matches!(
      &self.driver,
      Driver::Process { transport: Transport::Claude(claude), .. } if claude.results() > 0
    );
    let failed = matches!(
      watch.tracker.outcome(Some(0)),
      InvocationOutcome::TurnFailed { .. } | InvocationOutcome::NotSignedIn { .. }
    );
    ended && !failed
  }

  /// Names `approval`, which waited when its CLI could no longer take a decision, so the activity says
  /// which request ended interrupted.
  pub(super) fn report_interrupted(&self, watch: &mut Watch, approval: &PendingToolApproval) -> Result<(), StoreError> {
    let problem = RunnerProblem::DecisionInterrupted {
      request: approval.id.clone(),
      server: approval.server.clone(),
      action: approval.action.clone(),
    };
    self.observe(watch, AgentEvent::RunnerProblem { problem })
  }

  async fn await_person(
    &mut self,
    watch: &mut Watch,
    key: &str,
    shown: Shown,
    awaiting: Awaiting,
  ) -> Result<(), StoreError> {
    if watch.approval.is_some() {
      self.observe(
        watch,
        AgentEvent::TurnFailed {
          message: format!("{} requested another tool decision while one is pending", self.cli()),
        },
      )?;
      self.driver.end();
      return Ok(());
    }
    let approval = PendingToolApproval {
      id: format!("{}:{key}", watch.invocation.id),
      run: watch.invocation.run,
      execution: watch.invocation.execution,
      invocation: watch.invocation.id,
      agent: self.agent.clone(),
      command: shown.command,
      scope: shown.scope.map(|scope| self.redact(&scope)),
      server: shown.server,
      action: self.redact(&shown.action),
      target: shown.target.map(|target| self.redact(&target)),
      requested_at: Timestamp::now(),
      native: shown.native,
    };
    watch.db.request_tool_approval(&approval)?;
    if let Some(settled) = self.answer_itself(watch, &approval)? {
      // Recorded and delivered like a person's answer; a call that can no longer take it ends the
      // agent there, as for a person.
      if let Err(rejection) = self.settle(watch, &approval, awaiting, settled).await? {
        tracing::debug!(?rejection, "Orchester's own answer did not reach the agent");
      }
      return Ok(());
    }
    // Auto mode judges it first, without asking the person; a hard rule (a Codex command asks to leave
    // its sandbox, which only the person grants) asks the person at once.
    let escalation = matches!(&awaiting, Awaiting::Codex(request) if request.is_command());
    self.reviewing = self.start_review(watch, &approval, escalation)?;
    if self.reviewing.is_none() {
      watch.invocation.timing.wait_for_person(approval.requested_at);
      // A person's wait never holds the computer's QA slot.
      self.qa.release(watch.invocation.id);
      watch.db.save_invocation(&watch.invocation)?;
    }
    self.notifier.send(Event::ToolApprovalChanged {
      run: approval.run,
      task: self.task,
    });
    watch.approval = Some((approval, awaiting));
    Ok(())
  }

  pub(super) async fn decide(&mut self, watch: &mut Watch, decision: ApprovalDecision) -> Result<(), StoreError> {
    let result = self.apply_decision(watch, &decision).await;
    match result {
      Ok(reply) => answer(decision.ack, reply),
      Err(error) => {
        answer(
          decision.ack,
          Err(Rejection::Failed {
            detail: "the tool decision could not be recorded".to_owned(),
          }),
        );
        self.driver.end();
        return Err(error);
      }
    }
    Ok(())
  }

  async fn apply_decision(
    &mut self,
    watch: &mut Watch,
    decision: &ApprovalDecision,
  ) -> Result<Result<(), Rejection>, StoreError> {
    let held = watch
      .approval
      .as_ref()
      .is_some_and(|(pending, _)| pending.id == decision.id);
    if !held {
      // Not the call this agent waits on (one of an earlier invocation, or recorded without one): no
      // agent can take its answer, so it ends, unless it was answered already.
      let run = watch.invocation.run;
      let (ended, refusal) = end_unheld(&mut watch.db, run, &decision.id)?;
      if ended {
        self.notifier.send(Event::ToolApprovalChanged { run, task: self.task });
      }
      return Ok(Err(refusal));
    }
    // A stopping agent (a pause checkpointing at the decision) takes none any more; its end interrupts
    // the request.
    if watch.stop.is_some() {
      return Ok(Err(Rejection::ToolApprovalNotAwaited));
    }
    let (approval, awaiting) = watch.approval.take().expect("the pending approval was matched above");
    let settled = match self.widen(watch, &approval, (&decision.answer, decision.by))? {
      Ok(settled) => settled,
      Err(rejection) => {
        // The request still waits for an answer that can be given.
        watch.approval = Some((approval, awaiting));
        return Ok(Err(rejection));
      }
    };
    self.settle(watch, &approval, awaiting, settled).await
  }

  /// Hands `answer` to the call that asked. An approved Claude Code call is then running until its
  /// result arrives ([`Watch::approved_calls`]).
  pub(super) async fn deliver(&self, watch: &mut Watch, awaiting: Awaiting, answer: &Settled) -> Result<(), String> {
    let approve = answer.decided_by.approves();
    match awaiting {
      Awaiting::Codex(request) => match self.driver.process() {
        Some(process) => process
          .send_line(request.response(approve))
          .await
          .map_err(|error| error.to_string()),
        None => Err("Codex has no process".to_owned()),
      },
      Awaiting::Native(call) => match &self.driver {
        Driver::Native(handle) => handle.decide(call, approve),
        Driver::Process { .. } => Err("a CLI takes no native decision".to_owned()),
      },
      Awaiting::Claude { call, reply } => {
        reply
          .send(prompt_answer(answer, approve))
          .map_err(|_| "its permission prompt has closed".to_owned())?;
        if approve {
          watch.approved_calls.insert(call);
        }
        Ok(())
      }
      // The relay runs the call and answers it itself; the agent's CLI reports no result to wait for.
      Awaiting::Tool { reply, .. } => reply
        .send(prompt_answer(answer, approve))
        .map_err(|_| "Orchester's tool relay has closed".to_owned()),
    }
  }

  pub(super) fn end_approval_wait(&self, watch: &mut Watch) -> Result<(), StoreError> {
    let waited = watch.invocation.timing.waiting_since().is_some();
    watch.invocation.timing.resume(Timestamp::now());
    watch.approval = None;
    self.resume_qa(watch);
    watch.db.save_invocation(&watch.invocation)?;
    if waited {
      self.notifier.send(Event::ToolApprovalChanged {
        run: watch.invocation.run,
        task: self.task,
      });
    }
    Ok(())
  }

  /// A person's wait ended, or an answer settled: QA starts keep their apps again ([`QaApps::resume`](
  /// super::qa_app::QaApps::resume)) once no person wait of the invocation remains — no question held in its turn and
  /// no tool request that a person, not auto mode's reviewer, decides.
  pub(super) fn resume_qa(&self, watch: &Watch) {
    let person_waits = watch.question.is_some() || (watch.approval.is_some() && self.reviewing.is_none());
    if !person_waits {
      self.qa.resume(watch.invocation.id);
    }
  }
}

/// The answer a relay call gets: allowed, or denied with what the agent is told.
fn prompt_answer(answer: &Settled, approve: bool) -> PromptAnswer {
  match &answer.told {
    Some(told) if !approve => PromptAnswer::Deny(told.clone()),
    None if !approve => PromptAnswer::Repeated,
    _ => PromptAnswer::Allow,
  }
}
