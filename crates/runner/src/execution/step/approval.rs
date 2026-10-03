//! One tool call an agent waits on: the request is recorded, Orchester answers it itself when a rule of
//! the run, Always allow or auto-approval covers it ([`super::answer`]), and otherwise it is shown and
//! the agent's time stops counting while a person decides. The answer goes to exactly the call that
//! asked — a Codex App Server JSON-RPC request or a Claude Code permission prompt call
//! ([`super::claude_approval`]). A denial tells the agent and lets it continue. A decision that no
//! longer matches the waiting call is refused, so a stale one never approves a later action.

use orchester_core::{
  command::redact_secrets,
  connection::{
    app_server::PendingApproval,
    parse_cli_too_old,
    stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  },
  run::PendingToolApproval,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::StoreError;
use tokio::sync::oneshot;

use super::{
  super::control::answer,
  answer::Settled,
  claude_approval::{Asked, PromptAnswer},
  worker::{ApprovalDecision, Watch, Worker},
};

/// The call a pending approval answers.
pub(super) enum Awaiting {
  Codex(Box<PendingApproval>),
  /// The permission prompt call `call` (its tool use id), answered through `reply`.
  Claude {
    call: String,
    reply: oneshot::Sender<PromptAnswer>,
  },
}

impl Awaiting {
  /// The tool item waiting for the decision; other items may still be executing.
  pub(super) fn item(&self) -> &str {
    match self {
      Self::Codex(request) => request.item(),
      Self::Claude { call, .. } => call,
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
}

impl Worker {
  /// The CLI the approvals of this worker come from, for its messages.
  pub(super) const fn cli(&self) -> &'static str {
    if self.app_server().is_some() {
      "Codex"
    } else {
      "Claude Code"
    }
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
    };
    let key = request.request_key();
    self
      .await_person(watch, &key, shown, Awaiting::Codex(Box::new(request)))
      .await
  }

  /// A permission prompt call of Claude Code that a person decides. A call asked about before is
  /// denied without asking again.
  pub(super) async fn request_claude_approval(&mut self, watch: &mut Watch, asked: Asked) -> Result<(), StoreError> {
    let Asked { request, reply } = asked;
    let first = self
      .claude_approvals
      .as_mut()
      .is_some_and(|approvals| approvals.first_ask(request.call()));
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
    };
    let call = request.call().to_owned();
    let key = call.clone();
    self
      .await_person(watch, &key, shown, Awaiting::Claude { call, reply })
      .await
  }

  pub(super) fn redact(&self, text: &str) -> String {
    match self.app_server() {
      Some(app) => app.redact(text),
      None => redact_secrets(text, &self.mcp_secrets),
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

  /// Claude Code's permission prompt closed its connection while a call waited: the call can no
  /// longer be answered, so the request is interrupted and a later decision is refused.
  pub(super) fn prompt_closed(&mut self, watch: &mut Watch) -> Result<(), StoreError> {
    watch.approval = None;
    watch
      .db
      .interrupt_tool_approvals(watch.invocation.run, watch.invocation.id)?;
    self.end_approval_wait(watch)?;
    self.observe(
      watch,
      AgentEvent::TurnFailed {
        message: "Claude Code's permission prompt closed before the decision".to_owned(),
      },
    )?;
    self.process.terminate();
    Ok(())
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
      self.process.terminate();
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
      native: None,
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
    watch.invocation.timing.wait_for_person(approval.requested_at);
    watch.db.save_invocation(&watch.invocation)?;
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
        self.process.terminate();
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
    // A stopping agent (a pause checkpointing at the decision) takes none any more.
    if watch.stop.is_some()
      || watch
        .approval
        .as_ref()
        .is_none_or(|(pending, _)| pending.id != decision.id)
    {
      return Ok(Err(Rejection::ToolApprovalNotAwaited));
    }
    let (approval, awaiting) = watch.approval.take().expect("the pending approval was matched above");
    let settled = match self.widen(watch, &approval, &decision.answer)? {
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
      Awaiting::Codex(request) => self
        .process
        .send_line(request.response(approve))
        .await
        .map_err(|error| error.to_string()),
      Awaiting::Claude { call, reply } => {
        let prompt = match &answer.told {
          Some(told) if !approve => PromptAnswer::Deny(told.clone()),
          None if !approve => PromptAnswer::Repeated,
          _ => PromptAnswer::Allow,
        };
        reply
          .send(prompt)
          .map_err(|_| "its permission prompt has closed".to_owned())?;
        if approve {
          watch.approved_calls.insert(call);
        }
        Ok(())
      }
    }
  }

  pub(super) fn end_approval_wait(&self, watch: &mut Watch) -> Result<(), StoreError> {
    let waited = watch.invocation.timing.waiting_since().is_some();
    watch.invocation.timing.resume(Timestamp::now());
    watch.db.save_invocation(&watch.invocation)?;
    watch.approval = None;
    if waited {
      self.notifier.send(Event::ToolApprovalChanged {
        run: watch.invocation.run,
        task: self.task,
      });
    }
    Ok(())
  }
}
