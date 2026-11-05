//! A review answer on Codex: App Server in the run's worktree, as the step runs it, but in the `read-only` sandbox with
//! nothing writable, the step's network, no MCP server and nothing left to a person: a command that would need an
//! approval is refused, an MCP approval declined and a question of Codex's answered with nothing. The repository's
//! skills are left out as the Orchestrator chat leaves them out, and a search that could miss one starts no turn. The
//! step's thread is resumed, so the answer follows on from its work; the turn changes no file. Once the turn is over
//! the runner ends App Server, which otherwise waits for more. On the connection's API provider App Server starts with
//! the provider's overrides and key as a step's does ([`OnProvider`]).

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
    invocation::{CODEX_PROVIDER_ID, CodexAgentFiles, CodexInvocation, OnProvider, Platform, on_account},
    mcp::InvocationExemptions,
  },
  orchestrator::{TurnFailure, TurnState},
  resource::RepositoryAccess,
};
use tokio::sync::oneshot;

use super::{
  super::chat::cannot,
  turn::{Answer, Prepared, ReviewTurn},
};
use crate::{
  execution::executor::step::app_server::{AppServerFailure, AppServerSession, AppServerSetup},
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

impl ReviewTurn {
  /// Runs App Server until its turn is over, or `stop` or the time limit ends it, keeping what it reports in `answer`.
  pub(super) async fn drive_codex(
    &self,
    prepared: &Prepared,
    mut stop: oneshot::Receiver<()>,
    answer: &mut Answer,
  ) -> TurnState {
    let (command, setup) = match self.codex_start(prepared) {
      Ok(started) => started,
      Err(failure) => return TurnState::Failed(failure),
    };
    let mut process = match Supervised::start(&command) {
      Ok(process) => process,
      Err(error) => return TurnState::Failed(cannot(error.to_string())),
    };
    let mut session = AppServerSession::new(setup);
    if let Err(error) = session.start(&process).await {
      return TurnState::Failed(cannot(format!("cannot initialize Codex App Server: {error}")));
    }
    let mut beat = tokio::time::interval(Duration::from_millis(100));
    beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let limit = tokio::time::sleep(self.time_limit);
    tokio::pin!(limit);
    loop {
      let seen = &answer.seen;
      let going = !seen.stopped && !seen.finished && seen.timed_out.is_none();
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { .. }) if answer.seen.stopped || answer.seen.timed_out.is_some() => {}
          Some(ProcessEvent::Output { stream, line }) => {
            let over = match session.receive(stream, &line, &process).await {
              Ok(updates) => self.codex_updates(updates, &process, answer).await,
              Err(failure) => {
                answer.seen.failure.get_or_insert(failed(failure));
                true
              }
            };
            if over && !answer.seen.finished {
              answer.seen.finished = true;
              process.terminate();
            }
          }
          Some(ProcessEvent::Exited(exit)) => return std::mem::take(&mut answer.seen).ended(&exit),
          None => {
            let exit = ProcessExit::WaitFailed { reason: "the CLI's output ended".to_owned() };
            return std::mem::take(&mut answer.seen).ended(&exit);
          }
        },
        _ = beat.tick(), if going => {
          if let Err(failure) = session.poll_servers(&process).await {
            answer.seen.failure.get_or_insert(failed(failure));
            answer.seen.finished = true;
            process.terminate();
          }
        }
        stopped = &mut stop, if !answer.seen.stopped && !answer.seen.finished => {
          if stopped.is_ok() {
            answer.seen.stopped = true;
            process.terminate();
          }
        }
        () = &mut limit, if going => {
          answer.seen.timed_out = Some(TurnFailure::TimeLimit { minutes: self.time_limit.as_secs() / 60 });
          process.terminate();
        }
      }
    }
  }

  /// App Server's command and the session's setup, or why none can start.
  fn codex_start(&self, prepared: &Prepared) -> Result<(Command, AppServerSetup), TurnFailure> {
    let on_provider = prepared
      .connection
      .api_provider()
      .zip(prepared.key.clone())
      .map(|(provider, key)| OnProvider {
        provider,
        model: &prepared.model,
        key,
      });
    let skills = RepositorySkills::find(&prepared.worktree)
      .map_err(|error| {
        cannot(format!(
          "the repository's skills cannot all be found to leave them out: {error}"
        ))
      })?
      .codex_paths();
    let command = CodexInvocation {
      executable: &prepared.executable,
      repository: &self.root,
      access: RepositoryAccess::ReadOnly,
      network: prepared.network.clone(),
      writable_roots: &[],
      worktree: &prepared.worktree,
      platform: Platform::current(),
      mcp: None,
      agent_files: CodexAgentFiles::LeftOut { skills: &skills },
    }
    .command(std::env::vars_os(), &prepared.environment)
    .and_then(|command| on_account(command, prepared.connection.account(), AgentProvider::CodexCli))
    .map(|command| match &on_provider {
      Some(on) => on.apply(command, AgentProvider::CodexCli),
      None => command,
    })
    .map_err(|error| cannot(error.to_string()))?;
    let setup = AppServerSetup {
      prompt: prepared.prompt.clone(),
      attached: Vec::new(),
      model: prepared.model.as_str().to_owned(),
      effort: prepared.effort.clone(),
      worktree: prepared.worktree.clone(),
      access: RepositoryAccess::ReadOnly,
      command_approvals: false,
      sandbox_approvals: false,
      tool_approvals: false,
      network: prepared.network.clone(),
      writable_roots: Vec::new(),
      resume: prepared.session.clone(),
      expected_servers: BTreeSet::new(),
      renamed: BTreeMap::new(),
      exemptions: InvocationExemptions::default(),
      output_schema: None,
      // A review answer loads no skill of its own.
      skill: None,
      secrets: on_provider
        .iter()
        .map(|on| on.key_variable(AgentProvider::CodexCli))
        .collect(),
      model_provider: on_provider.as_ref().map(|_| CODEX_PROVIDER_ID.to_owned()),
      base_instructions: None,
      compact: false,
    };
    Ok((command, setup))
  }

  /// Keeps and answers what App Server said; whether the turn is over.
  async fn codex_updates(&self, updates: Vec<Update>, process: &Supervised, answer: &mut Answer) -> bool {
    let mut over = false;
    for update in updates {
      let reply = match update {
        Update::Activity(event) => {
          self.keep(event, answer);
          None
        }
        Update::Approval(request) => Some(request.response(false)),
        Update::Declined(response) | Update::QuestionRefused { response, .. } => Some(response),
        Update::Question(pending) => Some(pending.response(&[])),
        Update::TurnEnded { .. } => {
          over = true;
          None
        }
        Update::InputTaken(_) | Update::Failure(_) => None,
      };
      // A process that cannot take the reply is ending; its exit says how.
      if let Some(reply) = reply
        && let Err(error) = process.send_line(reply).await
      {
        tracing::warn!(%error, "Codex App Server did not take the review answer's reply");
      }
    }
    over
  }
}

/// What App Server's failure says of the turn.
fn failed(failure: AppServerFailure) -> TurnFailure {
  match failure {
    AppServerFailure::NotSignedIn(_) => TurnFailure::NotSignedIn,
    AppServerFailure::Protocol(reason) => TurnFailure::Failed { reason },
  }
}
