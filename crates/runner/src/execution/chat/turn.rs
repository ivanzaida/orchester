//! One turn of the Orchestrator chat: Claude Code started for the person's message (`ClaudeChat`), its output kept
//! as it comes (the session, each reply), and how the turn ended. The files the CLI reads its system prompt and its
//! MCP configuration from, the latter with the chat's token in it, live as long as the CLI and are removed then.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider,
    invocation::{ClaudeChat, ClaudeMcpConfig, ClaudeSession, McpLaunch, Platform, on_account},
    mcp::{McpServerName, McpUrl},
    stream::{AgentEvent, ClaudeStream, OutputStream},
  },
  conversation::MessageBody,
  id::{ChatMessageId, ProjectId},
  orchestrator::{
    Chat, OrchestratorSettings, TurnFailure, TurnState,
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
  private_file::PrivateFile,
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

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

/// The CLI's command and the files it reads, which must outlive it.
struct Started {
  command: orchester_core::command::Command,
  _files: [PrivateFile; 2],
}

impl Turn {
  /// Runs the turn to its end, or until `stop` says so, and records how it ended.
  pub(super) async fn run(self, stop: oneshot::Receiver<()>) {
    let state = match self.prepare() {
      Ok(started) => self.drive(started, stop).await,
      Err(failure) => TurnState::Failed(failure),
    };
    self.record(|db| db.end_chat_turn(self.message, &state).map(drop));
  }

  /// The command that answers the message, or why none can start.
  fn prepare(&self) -> Result<Started, TurnFailure> {
    let cannot = |reason: String| TurnFailure::CannotStart { reason };
    let db = ProjectDb::open(&self.root).map_err(|error| cannot(error.to_string()))?;
    let connection = db
      .connection(self.settings.connection())
      .map_err(|error| cannot(error.to_string()))?
      .and_then(|connection| AgentConnection::try_from(connection).ok())
      .ok_or_else(|| cannot("the Orchestrator's connection is gone".to_owned()))?;
    let provider = connection.provider();
    if !runs_orchestrator(provider) {
      return Err(cannot(
        "the Orchestrator runs on Claude Code connections only".to_owned(),
      ));
    }
    if let Some(reason) = connection
      .account()
      .start_problem(provider, Platform::current(), Path::is_dir)
    {
      return Err(cannot(reason.to_string()));
    }
    let executable = connection
      .executable()
      .ok_or_else(|| cannot("the connection has no CLI program".to_owned()))?;
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
    let command = ClaudeChat {
      executable,
      model: self.settings.model().as_str(),
      effort: self.settings.effort(),
      folder: &self.root,
      session: ClaudeSession::continuing(self.chat.resumes_on(self.settings.connection())),
      system_prompt: prompt.path(),
      mcp_config: mcp.path(),
    }
    .command(self.text.as_str(), std::env::vars_os(), &environment)
    .and_then(|command| on_account(command, connection.account(), AgentProvider::ClaudeCodeCli))
    .map_err(|error| cannot(error.to_string()))?;
    Ok(Started {
      command,
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
    loop {
      tokio::select! {
        event = process.next() => match event {
          Some(ProcessEvent::Output { stream: from, line }) => {
            for event in stream.parse(from, &line) {
              self.keep(event, &mut seen);
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
      }
    }
  }

  /// Keeps what `event` says: the session, a reply, or how the turn goes.
  fn keep(&self, event: AgentEvent, seen: &mut Seen) {
    match event {
      AgentEvent::SessionStarted { session } => {
        let connection = self.settings.connection();
        self.record(|db| db.set_chat_session(self.chat.id(), &session, connection));
      }
      AgentEvent::Message { text } if !text.trim().is_empty() => {
        self.record(|db| db.add_chat_reply(self.chat.id(), (&text, self.settings.model()), Timestamp::now()));
        self.notifier.send(Event::ChatChanged { chat: self.chat.id() });
      }
      AgentEvent::TurnCompleted { .. } => seen.answered = true,
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
}

impl Seen {
  /// How the turn ended, once its CLI exited as `exit` says.
  fn ended(self, exit: &ProcessExit) -> TurnState {
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
