//! A review answer on Claude Code (`ClaudeReviewAnswer`): read-only in the run's worktree, the step's session resumed
//! as a fork, its stream-json output kept as it comes. On a connection's API provider the CLI starts with the
//! provider's variables as a step's does ([`OnProvider`]), and what the turn keeps of its output has the key redacted.

use orchester_core::{
  command::redact_secrets,
  connection::{
    AgentProvider,
    invocation::{ClaudeReviewAnswer, ClaudeSession, OnProvider, on_account},
    stream::ClaudeStream,
  },
  orchestrator::{TurnFailure, TurnState},
};
use tokio::sync::oneshot;

use super::{
  super::chat::{cannot, denied},
  turn::{Answer, Prepared, ReviewTurn},
};
use crate::supervisor::{ProcessEvent, ProcessExit, Supervised};

impl ReviewTurn {
  /// Runs Claude Code until its turn ended, or `stop` or the time limit ends it, keeping what it reports in `answer`.
  pub(super) async fn drive_claude(
    &self,
    prepared: &Prepared,
    mut stop: oneshot::Receiver<()>,
    answer: &mut Answer,
  ) -> TurnState {
    let (denied, denied_files) = denied(prepared.connection.account());
    let on_provider = prepared
      .connection
      .api_provider()
      .zip(prepared.key.clone())
      .map(|(provider, key)| OnProvider {
        provider,
        model: &prepared.model,
        key,
      });
    let command = ClaudeReviewAnswer {
      executable: &prepared.executable,
      model: prepared.model.as_str(),
      effort: prepared.effort.as_deref(),
      worktree: &prepared.worktree,
      session: ClaudeSession::continuing(prepared.session.as_deref()),
      denied: &denied,
      denied_files: &denied_files,
    }
    .command(&prepared.prompt, std::env::vars_os(), &prepared.environment)
    .and_then(|command| on_account(command, prepared.connection.account(), AgentProvider::ClaudeCodeCli))
    .map(|command| match &on_provider {
      Some(on) => on.apply(command, AgentProvider::ClaudeCodeCli),
      None => command,
    });
    let command = match command {
      Ok(command) => command,
      Err(error) => return TurnState::Failed(cannot(error.to_string())),
    };
    let secrets: Vec<_> = on_provider
      .iter()
      .map(|on| on.key_variable(AgentProvider::ClaudeCodeCli))
      .collect();
    let mut process = match Supervised::start(&command) {
      Ok(process) => process,
      Err(error) => return TurnState::Failed(cannot(error.to_string())),
    };
    let mut stream = ClaudeStream::chat();
    let limit = tokio::time::sleep(self.time_limit);
    tokio::pin!(limit);
    loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { stream: from, line }) => {
            for mut event in stream.parse(from, &line) {
              event.redact(&|text| redact_secrets(text, &secrets));
              self.keep(event, answer);
            }
            if stream.results() > 0 {
              process.close_input();
            }
          }
          Some(ProcessEvent::Exited(exit)) => return std::mem::take(&mut answer.seen).ended(&exit),
          None => {
            let exit = ProcessExit::WaitFailed { reason: "the CLI's output ended".to_owned() };
            return std::mem::take(&mut answer.seen).ended(&exit);
          }
        },
        stopped = &mut stop, if !answer.seen.stopped => {
          if stopped.is_ok() {
            answer.seen.stopped = true;
            process.terminate();
          }
        }
        () = &mut limit, if !answer.seen.stopped && answer.seen.timed_out.is_none() => {
          answer.seen.timed_out = Some(TurnFailure::TimeLimit { minutes: self.time_limit.as_secs() / 60 });
          process.terminate();
        }
      }
    }
  }
}
