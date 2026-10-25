//! A turn on Codex (`docs/design/orchester-workspace.md`, "Project orchestrator conversation"): Codex App Server in
//! the project folder, as steps run it (`super::super::step::app_server`), started confined
//! (`orchester_core::connection::invocation::CodexChat`): no shell, no tool of Codex's own that reads, runs or fetches
//! anything, the `read-only` sandbox and no command left to a person, so it reads nothing but what the chat's domain
//! tools give it and changes nothing. Those tools are its only MCP server (the user's own are turned off for the
//! thread), their token in the process environment only. The system prompt is the thread's `baseInstructions`, in
//! place of Codex's own, on every turn; the thread is resumed on the next one. Codex asks before an MCP call: a call
//! of the chat's own tools is approved, as Claude Code's `--allowedTools mcp__orchester` allows them, any other
//! declined; a question of Codex's gets no answers. Once the turn is over the runner ends App Server, which otherwise
//! waits for more. A stopped or timed-out turn keeps what Codex reported it spent so far.

use std::collections::BTreeSet;

use orchester_core::{
  command::Command,
  connection::{
    AgentProvider,
    app_server::Update,
    invocation::{CodexChat, CodexMcp, Platform, on_account},
    mcp::InvocationExemptions,
    stream::AgentEvent,
  },
  id::AgentAccountId,
  orchestrator::{
    TurnFailure, TurnState,
    instructions::{ProjectReads, SERVER, system_prompt},
  },
  permission::NetworkAccess,
  resource::RepositoryAccess,
  run::TokenUsage,
};
use tokio::sync::oneshot;

use super::turn::{Prepared, Seen, Turn, cannot};
use crate::{
  execution::step::app_server::{AppServerFailure, AppServerSession, AppServerSetup},
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

/// App Server's command, what the session starts with, and the account it runs on.
struct Started {
  command: Command,
  setup: AppServerSetup,
  account: AgentAccountId,
}

impl Turn {
  pub(super) async fn run_codex(&self, prepared: Prepared, stop: oneshot::Receiver<()>) -> TurnState {
    match self.start_codex(prepared) {
      Ok(started) => self.drive_codex(started, stop).await,
      Err(failure) => TurnState::Failed(failure),
    }
  }

  /// App Server's command and the session's setup, or why none can start.
  fn start_codex(&self, prepared: Prepared) -> Result<Started, TurnFailure> {
    let (mcp, left_out) = CodexMcp::new(&[self.tools_launch()?]);
    if !left_out.is_empty() {
      return Err(cannot("the tools' server was left out".to_owned()));
    }
    let command = CodexChat {
      executable: &prepared.executable,
      folder: &self.root,
      platform: Platform::current(),
      mcp: &mcp,
    }
    .command(std::env::vars_os(), &prepared.environment)
    .and_then(|command| on_account(command, prepared.connection.account(), AgentProvider::CodexCli))
    .map_err(|error| cannot(error.to_string()))?;
    let setup = AppServerSetup {
      prompt: self.text.as_str().to_owned(),
      attached: self.attached.clone(),
      model: self.settings.model().as_str().to_owned(),
      effort: self.settings.effort().map(str::to_owned),
      worktree: self.root.clone(),
      access: RepositoryAccess::ReadOnly,
      command_approvals: false,
      sandbox_approvals: false,
      tool_approvals: true,
      network: NetworkAccess::Off,
      writable_roots: Vec::new(),
      resume: prepared.resumed,
      expected_servers: BTreeSet::from([SERVER.to_owned()]),
      exemptions: InvocationExemptions::default(),
      output_schema: None,
      secrets: mcp.secrets().to_vec(),
      model_provider: None,
      base_instructions: Some(system_prompt(
        &prepared.project_name,
        self.project,
        ProjectReads::GitTools,
      )),
    };
    Ok(Started {
      command,
      setup,
      account: prepared.account,
    })
  }

  /// Runs App Server, keeping what it reports, until the turn is over or `stop` ends it.
  async fn drive_codex(&self, started: Started, mut stop: oneshot::Receiver<()>) -> TurnState {
    let mut process = match Supervised::start(&started.command) {
      Ok(process) => process,
      Err(error) => return TurnState::Failed(cannot(error.to_string())),
    };
    let mut session = AppServerSession::new(started.setup);
    if let Err(error) = session.start(&process).await {
      return TurnState::Failed(cannot(format!("cannot initialize Codex App Server: {error}")));
    }
    let mut seen = Seen::default();
    let limit = tokio::time::sleep(self.time_limit);
    tokio::pin!(limit);
    loop {
      tokio::select! {
        event = process.next() => match event {
          // What it says once the runner ended it was kept already ([`Self::keep_spent`]).
          Some(ProcessEvent::Output { .. }) if seen.stopped || seen.timed_out.is_some() => {}
          Some(ProcessEvent::Output { stream, line }) => {
            let over = match session.receive(stream, &line, &process).await {
              Ok(updates) => self.codex_updates(updates, (&process, started.account), &mut seen).await,
              Err(failure) => {
                seen.failure.get_or_insert(match failure {
                  AppServerFailure::NotSignedIn(_) => TurnFailure::NotSignedIn,
                  AppServerFailure::Protocol(reason) => TurnFailure::Failed { reason },
                });
                true
              }
            };
            if over && !seen.finished {
              seen.finished = true;
              process.terminate();
            }
          }
          Some(ProcessEvent::Exited(exit)) => return seen.ended(&exit),
          None => return seen.ended(&ProcessExit::WaitFailed { reason: "the CLI's output ended".to_owned() }),
        },
        stopped = &mut stop, if !seen.stopped && !seen.finished => {
          if stopped.is_ok() {
            seen.stopped = true;
            self.keep_spent(&session, started.account, &mut seen);
            process.terminate();
          }
        }
        () = &mut limit, if !seen.stopped && !seen.finished && seen.timed_out.is_none() => {
          seen.timed_out = Some(TurnFailure::TimeLimit { minutes: self.time_limit.as_secs() / 60 });
          self.keep_spent(&session, started.account, &mut seen);
          process.terminate();
        }
      }
    }
  }

  /// Keeps what the turn spent so far, which Codex reports with its end only: a stopped or timed-out turn counts.
  fn keep_spent(&self, session: &AppServerSession, account: AgentAccountId, seen: &mut Seen) {
    let usage = session.usage_so_far();
    if usage != TokenUsage::default() {
      self.keep(AgentEvent::TurnCompleted { usage }, account, seen);
    }
  }

  /// Keeps and answers what App Server said; whether the turn is over.
  async fn codex_updates(
    &self,
    updates: Vec<Update>,
    (process, account): (&Supervised, AgentAccountId),
    seen: &mut Seen,
  ) -> bool {
    let mut over = false;
    for update in updates {
      let answer = match update {
        Update::Activity(event) => {
          self.keep(event, account, seen);
          None
        }
        Update::Approval(request) => Some(request.response(request.server == SERVER)),
        Update::Declined(response) | Update::QuestionRefused { response, .. } => Some(response),
        Update::Question(pending) => Some(pending.response(&[])),
        Update::InputTaken(_) => None,
        Update::TurnEnded { .. } => {
          over = true;
          None
        }
      };
      // A process that cannot take the answer is ending; its exit says how.
      if let Some(answer) = answer
        && let Err(error) = process.send_line(answer).await
      {
        tracing::warn!(%error, "Codex App Server did not take the chat's answer");
      }
    }
    over
  }
}
