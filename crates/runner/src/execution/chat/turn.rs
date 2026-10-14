//! One turn of the Orchestrator chat: the agent CLI the Orchestrator runs on, started for the person's message
//! (Claude Code, [`super::claude`]; Codex App Server, [`super::codex`]), its output kept as it comes (the session,
//! each reply, what it spent), and how the turn ended.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{AccountStartProblem, AgentConnection, AgentProvider, ConnectionError, invocation::Platform, stream::{AgentEvent, OutputStream}},
  conversation::MessageBody,
  id::{AgentAccountId, ChatMessageId, ProjectId},
  orchestrator::{Chat, ChatEvent, OrchestratorSettings, TurnFailure, TurnState, runs_orchestrator},
  resource::CommandEnvironment,
  store::{ConnectionStore, SettingsStore},
  time::Timestamp,
};
use orchester_ipc::{ChatTools, Event};
use orchester_store::ProjectDb;
use tokio::sync::oneshot;

use crate::{notify::Notifier, supervisor::ProcessExit};

/// Characters of the CLI's standard error kept as the reason of a turn that failed without saying why.
const REASON_CHARS: usize = 300;

/// Everything a turn starts from.
pub(super) struct Turn {
  pub(super) root: PathBuf,
  pub(super) project: ProjectId,
  pub(super) notifier: Notifier,
  pub(super) chat: Chat,
  pub(super) message: ChatMessageId,
  pub(super) text: MessageBody,
  pub(super) settings: OrchestratorSettings,
  pub(super) tools: ChatTools,
}

/// What either CLI starts from: the connection, its program and account, the session the turn resumes and the
/// project's environment.
pub(super) struct Prepared {
  pub(super) connection: AgentConnection,
  pub(super) executable: PathBuf,
  pub(super) account: AgentAccountId,
  pub(super) resumed: Option<String>,
  pub(super) environment: CommandEnvironment,
  pub(super) project_name: String,
}

impl Turn {
  /// Runs the turn to its end, or until `stop` says so, and records how it ended.
  pub(super) async fn run(self, stop: oneshot::Receiver<()>) {
    let state = match self.prepare() {
      Ok(prepared) if prepared.connection.provider() == AgentProvider::CodexCli => {
        self.run_codex(prepared, stop).await
      }
      Ok(prepared) => self.run_claude(prepared, stop).await,
      Err(failure) => TurnState::Failed(failure),
    };
    if let TurnState::Failed(TurnFailure::CannotStart { reason }) = &state {
      tracing::warn!(chat = %self.chat.id(), %reason, "a turn of the Orchestrator chat could not start");
    }
    self.record(|db| db.end_chat_turn(self.message, &state, Timestamp::now()).map(drop));
  }

  /// What the turn starts from, or why it cannot start. A session the turn cannot resume, on another connection or
  /// account, is said in the conversation.
  fn prepare(&self) -> Result<Prepared, TurnFailure> {
    let mut db = ProjectDb::open(&self.root).map_err(|error| cannot(error.to_string()))?;
    let stored = db
      .connection(self.settings.connection())
      .map_err(|error| cannot(error.to_string()))?
      .ok_or(TurnFailure::ConnectionGone)?;
    // A GitHub connection is not one the Orchestrator runs on, as the settings say; any other error says why.
    let connection = AgentConnection::try_from(stored).map_err(|error| match error {
      ConnectionError::NotAgent(_) => TurnFailure::ConnectionGone,
      other => cannot(format!("cannot read the Orchestrator's connection: {other}")),
    })?;
    let provider = connection.provider();
    if !runs_orchestrator(provider) {
      return Err(TurnFailure::ConnectionGone);
    }
    if let Some(problem) = connection
      .account()
      .start_problem(provider, Platform::current(), Path::is_dir)
    {
      return Err(TurnFailure::Account(problem));
    }
    let executable = connection
      .executable()
      .ok_or_else(|| cannot("the connection has no CLI program".to_owned()))?
      .to_path_buf();
    let account = connection
      .account()
      .id(provider)
      .ok_or(TurnFailure::Account(AccountStartProblem::NoAccount))?;
    let resumed = self
      .chat
      .resumes_on(self.settings.connection(), account)
      .map(str::to_owned);
    if let Some(because) = self.chat.session_change(self.settings.connection(), account) {
      // The session lives in another CLI folder: the agent starts afresh, and the conversation says so.
      db.add_chat_event(self.chat.id(), &ChatEvent::NewSession { because }, Timestamp::now())
        .map_err(|error| cannot(error.to_string()))?;
      self.notifier.send(Event::ChatChanged { chat: self.chat.id() });
    }
    let environment = db.project_environment().map_err(|error| cannot(error.to_string()))?;
    Ok(Prepared {
      connection,
      executable,
      account,
      resumed,
      environment,
      project_name: db.project().name().as_str().to_owned(),
    })
  }

  /// Keeps what `event` of the CLI on `account` says: the session, a reply, or how the turn goes.
  pub(super) fn keep(&self, event: AgentEvent, account: AgentAccountId, seen: &mut Seen) {
    match event {
      AgentEvent::SessionStarted { session } => {
        let connection = self.settings.connection();
        self.record(|db| db.set_chat_session(self.chat.id(), &session, (connection, account)));
      }
      AgentEvent::Message { text } if !text.trim().is_empty() => {
        self.record(|db| db.add_chat_reply(self.chat.id(), (&text, self.settings.model()), Timestamp::now()));
      }
      AgentEvent::TurnCompleted { usage } => {
        seen.answered = true;
        let turn = (self.chat.id(), self.message);
        self.record(|db| db.record_chat_usage(turn, usage, Timestamp::now()));
      }
      AgentEvent::NotSignedIn { .. } => seen.failure = Some(TurnFailure::NotSignedIn),
      AgentEvent::TurnFailed { message } | AgentEvent::Error { message } => {
        seen.failure.get_or_insert(TurnFailure::Failed { reason: message });
      }
      AgentEvent::Raw {
        stream: OutputStream::Stderr,
        line,
      } if !line.trim().is_empty() => seen.stderr = Some(line),
      _ => {}
    }
  }

  /// Applies `change` to the project's database; a change that cannot be kept is logged, and the turn goes on.
  fn record(&self, change: impl FnOnce(&mut ProjectDb) -> Result<(), orchester_store::StoreError>) {
    let kept = ProjectDb::open(&self.root).and_then(|mut db| change(&mut db));
    if let Err(error) = kept {
      tracing::error!(chat = %self.chat.id(), %error, "cannot keep what a chat turn did");
    }
    self.notifier.send(Event::ChatChanged { chat: self.chat.id() });
  }
}

/// Why a turn could not start, as the runner's log says it.
pub(super) const fn cannot(reason: String) -> TurnFailure {
  TurnFailure::CannotStart { reason }
}

/// What the CLI reported so far.
#[derive(Default)]
pub(super) struct Seen {
  pub(super) answered: bool,
  pub(super) stopped: bool,
  /// The runner ended the CLI once its turn was over: Codex App Server waits for more otherwise.
  pub(super) finished: bool,
  pub(super) failure: Option<TurnFailure>,
  /// The last line of standard error.
  pub(super) stderr: Option<String>,
  /// The runner ended it at the time limit.
  pub(super) timed_out: Option<TurnFailure>,
}

impl Seen {
  /// How the turn ended, once its CLI exited as `exit` says.
  pub(super) fn ended(self, exit: &ProcessExit) -> TurnState {
    if let Some(failure) = self.timed_out {
      return TurnState::Failed(failure);
    }
    if self.stopped {
      return TurnState::Stopped;
    }
    if let Some(failure) = self.failure {
      return TurnState::Failed(failure);
    }
    if self.finished {
      return if self.answered {
        TurnState::Answered
      } else {
        TurnState::Failed(TurnFailure::Failed {
          reason: "the turn ended before it answered".to_owned(),
        })
      };
    }
    match exit {
      ProcessExit::Exited { code: Some(0) } if self.answered => TurnState::Answered,
      ProcessExit::Exited { code } => {
        let said = self
          .stderr
          .map(|line| line.chars().take(REASON_CHARS).collect::<String>());
        let reason = said.unwrap_or_else(|| match code {
          Some(code) => format!("the CLI ended with exit code {code} before it answered"),
          None => "the CLI ended before it answered".to_owned(),
        });
        TurnState::Failed(TurnFailure::Failed { reason })
      }
      ProcessExit::Terminated => TurnState::Stopped,
      ProcessExit::WaitFailed { reason } => TurnState::Failed(TurnFailure::Failed { reason: reason.clone() }),
    }
  }
}
