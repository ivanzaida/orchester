//! One turn of the Orchestrator chat: Claude Code started for the person's message (`ClaudeChat`), its output kept
//! as it comes (the session, each reply), and how the turn ended. The files the CLI reads its system prompt and its
//! MCP configuration from, the latter with the chat's token in it, live as long as the CLI and are removed then.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  account::AccountCli,
  connection::{
    AccountStartProblem, AgentConnection, AgentProvider, ConnectionAccount, ConnectionError,
    invocation::{ClaudeChat, ClaudeMcpConfig, ClaudeSession, McpLaunch, Platform, on_account},
    mcp::{McpServerName, McpUrl},
    stream::{AgentEvent, ClaudeStream, OutputStream},
  },
  conversation::MessageBody,
  id::{AgentAccountId, ChatMessageId, ProjectId},
  orchestrator::{
    Chat, ChatEvent, OrchestratorSettings, TIME_LIMIT, TurnFailure, TurnState,
    instructions::{SERVER, system_prompt},
    runs_orchestrator,
  },
  secret::SecretValue,
  store::{ConnectionStore, SettingsStore},
  time::Timestamp,
};
use orchester_ipc::{ChatTools, Event};
use orchester_store::ProjectDb;
use tokio::sync::oneshot;

use crate::{
  notify::Notifier,
  private_file::{self, PrivateFile},
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

/// The folders that hold secrets the chat's agent must not read: the runner's private files (every running
/// agent's MCP configuration with its tokens, and other chats' tokens) and the account's CLI folder (its sign-in).
fn denied_folders(account: &ConnectionAccount) -> Vec<PathBuf> {
  let cli = AccountCli::ClaudeCode;
  let own = || {
    std::env::var_os(cli.folder_variable())
      .filter(|value| !value.is_empty())
      .map(PathBuf::from)
      .or_else(|| std::env::home_dir().map(|home| home.join(cli.own_folder_name())))
  };
  let account = account.folder().map(Path::to_path_buf).or_else(own);
  [Some(private_file::root()), account].into_iter().flatten().collect()
}

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

/// The CLI's command and the files it reads, which must outlive it, and the account it runs on.
struct Started {
  command: orchester_core::command::Command,
  account: AgentAccountId,
  _files: [PrivateFile; 2],
}

impl Turn {
  /// Runs the turn to its end, or until `stop` says so, and records how it ended.
  pub(super) async fn run(self, stop: oneshot::Receiver<()>) {
    let state = match self.prepare() {
      Ok(started) => self.drive(started, stop).await,
      Err(failure) => TurnState::Failed(failure),
    };
    if let TurnState::Failed(TurnFailure::CannotStart { reason }) = &state {
      tracing::warn!(chat = %self.chat.id(), %reason, "a turn of the Orchestrator chat could not start");
    }
    self.record(|db| db.end_chat_turn(self.message, &state, Timestamp::now()).map(drop));
  }

  /// The command that answers the message, or why none can start.
  fn prepare(&self) -> Result<Started, TurnFailure> {
    let cannot = |reason: String| TurnFailure::CannotStart { reason };
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
    if !runs_orchestrator(connection.provider()) {
      return Err(TurnFailure::ConnectionGone);
    }
    let provider = connection.provider();
    if let Some(problem) = connection
      .account()
      .start_problem(provider, Platform::current(), Path::is_dir)
    {
      return Err(TurnFailure::Account(problem));
    }
    let executable = connection
      .executable()
      .ok_or_else(|| cannot("the connection has no CLI program".to_owned()))?;
    let account = connection
      .account()
      .id(provider)
      .ok_or(TurnFailure::Account(AccountStartProblem::NoAccount))?;
    let resumed = self.chat.resumes_on(self.settings.connection(), account);
    if let Some(because) = self.chat.session_change(self.settings.connection(), account) {
      // The session lives in another CLI folder: the agent starts afresh, and the conversation says so.
      db.add_chat_event(self.chat.id(), &ChatEvent::NewSession { because }, Timestamp::now())
        .map_err(|error| cannot(error.to_string()))?;
      self.notifier.send(Event::ChatChanged { chat: self.chat.id() });
    }
    let name = self.message.to_string();
    let prompt = PrivateFile::write(
      self.project,
      &format!("chat-prompt-{name}.md"),
      &system_prompt(db.project().name().as_str(), self.project),
    )
    .map_err(|error| cannot(format!("cannot write the system prompt: {error}")))?;
    let mcp = PrivateFile::write(
      self.project,
      &format!("chat-mcp-{name}.json"),
      self.mcp_config()?.contents(),
    )
    .map_err(|error| cannot(format!("cannot write the MCP configuration: {error}")))?;
    let environment = db.project_environment().map_err(|error| cannot(error.to_string()))?;
    let denied = denied_folders(connection.account());
    let command = ClaudeChat {
      executable,
      model: self.settings.model().as_str(),
      effort: self.settings.effort(),
      folder: &self.root,
      session: ClaudeSession::continuing(resumed),
      system_prompt: prompt.path(),
      mcp_config: mcp.path(),
      denied: &denied,
    }
    .command(self.text.as_str(), std::env::vars_os(), &environment)
    .and_then(|command| on_account(command, connection.account(), AgentProvider::ClaudeCodeCli))
    .map_err(|error| cannot(error.to_string()))?;
    Ok(Started {
      command,
      account,
      _files: [prompt, mcp],
    })
  }

  /// The domain tools as Claude Code's MCP configuration: the chat's endpoint with its token.
  fn mcp_config(&self) -> Result<ClaudeMcpConfig, TurnFailure> {
    let cannot = |reason: &str| TurnFailure::CannotStart {
      reason: reason.to_owned(),
    };
    let name = McpServerName::new(SERVER).map_err(|_| cannot("the tools' server name is not valid"))?;
    let url = McpUrl::new(&self.tools.url).map_err(|_| cannot("the tools' address is not valid"))?;
    let token =
      SecretValue::new(self.tools.token.expose().to_owned()).map_err(|_| cannot("the tools' token is blank"))?;
    let (config, _) = ClaudeMcpConfig::new(&[McpLaunch::http(name, url, Some(Arc::new(token)))]);
    config.ok_or_else(|| cannot("the tools' server was left out"))
  }

  /// Runs the CLI, keeping what it reports, until it ends or `stop` ends it.
  async fn drive(&self, started: Started, mut stop: oneshot::Receiver<()>) -> TurnState {
    let mut process = match Supervised::start(&started.command) {
      Ok(process) => process,
      Err(error) => {
        return TurnState::Failed(TurnFailure::CannotStart {
          reason: error.to_string(),
        });
      }
    };
    let mut seen = Seen::default();
    let mut stream = ClaudeStream::chat();
    let limit = tokio::time::sleep(TIME_LIMIT);
    tokio::pin!(limit);
    loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { stream: from, line }) => {
            for event in stream.parse(from, &line) {
              self.keep(event, started.account, &mut seen);
            }
          }
          Some(ProcessEvent::Exited(exit)) => return seen.ended(&exit),
          None => return seen.ended(&ProcessExit::WaitFailed { reason: "the CLI's output ended".to_owned() }),
        },
        stopped = &mut stop, if !seen.stopped => {
          if stopped.is_ok() {
            seen.stopped = true;
            process.terminate();
          }
        }
        () = &mut limit, if !seen.stopped && seen.timed_out.is_none() => {
          seen.timed_out = Some(TurnFailure::TimeLimit { minutes: TIME_LIMIT.as_secs() / 60 });
          process.terminate();
        }
      }
    }
  }

  /// Keeps what `event` of the CLI on `account` says: the session, a reply, or how the turn goes.
  fn keep(&self, event: AgentEvent, account: AgentAccountId, seen: &mut Seen) {
    match event {
      AgentEvent::SessionStarted { session } => {
        let connection = self.settings.connection();
        self.record(|db| db.set_chat_session(self.chat.id(), &session, (connection, account)));
      }
      AgentEvent::Message { text } if !text.trim().is_empty() => {
        self.record(|db| db.add_chat_reply(self.chat.id(), (&text, self.settings.model()), Timestamp::now()));
        self.notifier.send(Event::ChatChanged { chat: self.chat.id() });
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

/// What the CLI reported so far.
#[derive(Default)]
struct Seen {
  answered: bool,
  stopped: bool,
  failure: Option<TurnFailure>,
  /// The last line of standard error.
  stderr: Option<String>,
  /// The runner ended it at the time limit.
  timed_out: Option<TurnFailure>,
}

impl Seen {
  /// How the turn ended, once its CLI exited as `exit` says.
  fn ended(self, exit: &ProcessExit) -> TurnState {
    if let Some(failure) = self.timed_out {
      return TurnState::Failed(failure);
    }
    if self.stopped {
      return TurnState::Stopped;
    }
    if let Some(failure) = self.failure {
      return TurnState::Failed(failure);
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
