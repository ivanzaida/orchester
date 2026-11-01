//! A turn on Codex (`docs/design/orchester-workspace.md`, "Project orchestrator conversation"): Codex App Server in
//! the project folder, as steps run it (`super::super::step::app_server`), started confined
//! (`orchester_core::connection::invocation::CodexChat`): no shell, no tool of Codex's own that reads, runs or fetches
//! anything, the `read-only` sandbox and no command left to a person, so it reads nothing but what the chat's domain
//! tools give it and changes nothing. Those tools are its only MCP server (the user's own are turned off for the
//! thread), their token in the process environment only. Every skill of the project folder is found as for a step and
//! disabled, and a search that could miss one starts no turn. The system prompt is the thread's `baseInstructions`, in
//! place of Codex's own, on every turn; the thread is resumed on the next one. Codex asks before an MCP call: a call
//! of the chat's own tools is approved, as Claude Code's `--allowedTools mcp__orchester` allows them, any other
//! declined; a question of Codex's gets no answers. Once the turn is over the runner ends App Server, which otherwise
//! waits for more. A stopped or timed-out turn keeps what Codex reported it spent so far. On the connection's API
//! provider App Server starts with the provider's overrides and key as a step's does ([`OnProvider`]).

use std::{
  collections::{BTreeMap, BTreeSet},
  time::Duration,
};

use orchester_core::{
  agent_files::skills::RepositorySkills,
  command::Command,
  connection::{
    AgentProvider,
    app_server::Update,
    invocation::{CODEX_PROVIDER_ID, CodexChat, CodexMcp, OnProvider, Platform, on_account},
    mcp::InvocationExemptions,
    stream::AgentEvent,
  },
  id::{AgentAccountId, ChatMessageId},
  orchestrator::{
    TurnFailure, TurnState,
    instructions::{ProjectReads, SERVER, system_prompt},
  },
  permission::NetworkAccess,
  resource::RepositoryAccess,
  run::TokenUsage,
};
use tokio::sync::{mpsc, oneshot};

use super::{
  steer::Steering,
  turn::{Prepared, Seen, Turn, cannot},
};
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
  pub(super) async fn run_codex(
    &self,
    prepared: Prepared,
    (stop, steers): (oneshot::Receiver<()>, &mut mpsc::UnboundedReceiver<ChatMessageId>),
  ) -> TurnState {
    match self.start_codex(prepared) {
      Ok(started) => self.drive_codex(started, (stop, steers)).await,
      Err(failure) => TurnState::Failed(failure),
    }
  }

  /// App Server's command and the session's setup, or why none can start.
  fn start_codex(&self, prepared: Prepared) -> Result<Started, TurnFailure> {
    let (mcp, left_out) = CodexMcp::new(&[self.tools_launch()?]);
    if !left_out.is_empty() {
      return Err(cannot("the tools' server was left out".to_owned()));
    }
    // On the connection's API provider App Server starts with the provider's overrides and key, as a step's does.
    let on_provider = prepared
      .connection
      .api_provider()
      .zip(prepared.key.clone())
      .map(|(provider, key)| OnProvider {
        provider,
        model: self.settings.model(),
        key,
      });
    // Codex loads the folder's skills even from an untrusted folder: each is disabled, and a search that could miss
    // one starts no turn, as for a step without its agent files.
    let skills = RepositorySkills::find(&self.root)
      .map_err(|error| {
        cannot(format!(
          "the project's skills cannot all be found to keep them from the Orchestrator: {error}"
        ))
      })?
      .codex_paths();
    let command = CodexChat {
      executable: &prepared.executable,
      folder: &self.root,
      platform: Platform::current(),
      mcp: &mcp,
      skills: &skills,
    }
    .command(std::env::vars_os(), &prepared.environment)
    .and_then(|command| on_account(command, prepared.connection.account(), AgentProvider::CodexCli))
    .map(|command| match &on_provider {
      Some(on) => on.apply(command, AgentProvider::CodexCli),
      None => command,
    })
    .map_err(|error| cannot(error.to_string()))?;
    let mut secrets = mcp.secrets().to_vec();
    secrets.extend(on_provider.as_ref().map(|on| on.key_variable(AgentProvider::CodexCli)));
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
      renamed: BTreeMap::new(),
      exemptions: InvocationExemptions::default(),
      output_schema: None,
      secrets,
      model_provider: on_provider.as_ref().map(|_| CODEX_PROVIDER_ID.to_owned()),
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

  /// Runs App Server, keeping what it reports and giving its turn the messages sent now that `steers` hands over, until
  /// the turn is over or `stop` ends it.
  async fn drive_codex(
    &self,
    started: Started,
    (mut stop, steers): (oneshot::Receiver<()>, &mut mpsc::UnboundedReceiver<ChatMessageId>),
  ) -> TurnState {
    let mut process = match Supervised::start(&started.command) {
      Ok(process) => process,
      Err(error) => return TurnState::Failed(cannot(error.to_string())),
    };
    let mut session = AppServerSession::new(started.setup);
    if let Err(error) = session.start(&process).await {
      return TurnState::Failed(cannot(format!("cannot initialize Codex App Server: {error}")));
    }
    let mut seen = Seen::default();
    let mut steering = Steering::default();
    let mut beat = tokio::time::interval(Duration::from_millis(100));
    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let limit = tokio::time::sleep(self.time_limit);
    tokio::pin!(limit);
    loop {
      tokio::select! {
        event = process.next() => match event {
          // What it says once the runner ended it was kept already ([`Self::keep_spent`]).
          Some(ProcessEvent::Output { .. }) if seen.stopped || seen.timed_out.is_some() => {}
          Some(ProcessEvent::Output { stream, line }) => {
            let over = match session.receive(stream, &line, &process).await {
              Ok(updates) => {
                self.steer_answered(&mut steering, session.take_steer_answers());
                self.codex_updates(updates, (&process, started.account), (&mut seen, &mut steering)).await
              }
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
        _ = beat.tick(), if !seen.stopped && !seen.finished && seen.timed_out.is_none() => {
          if let Err(failure) = session.poll_servers(&process).await {
            seen.failure.get_or_insert(match failure {
              AppServerFailure::NotSignedIn(_) => TurnFailure::NotSignedIn,
              AppServerFailure::Protocol(reason) => TurnFailure::Failed { reason },
            });
            seen.finished = true;
            process.terminate();
          }
        }
        Some(message) = steers.recv(), if !seen.stopped && !seen.finished && seen.timed_out.is_none() => {
          self.steer_codex((&session, &process), &mut steering, message).await;
        }
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

  /// Keeps and answers what App Server said, and notes the messages sent now its turn took; whether the turn is over.
  async fn codex_updates(
    &self,
    updates: Vec<Update>,
    (process, account): (&Supervised, AgentAccountId),
    (seen, steering): (&mut Seen, &mut Steering),
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
        Update::InputTaken(text) => {
          self.took(steering, &text);
          None
        }
        Update::TurnEnded { .. } => {
          over = true;
          None
        }
        // A chat turn that fails ends as failed whatever the reason; it waits for no plan limit.
        Update::Failure(_) => None,
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
