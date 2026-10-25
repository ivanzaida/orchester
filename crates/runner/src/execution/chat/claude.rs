//! A turn on Claude Code (`ClaudeChat`): the system prompt and the MCP configuration, the latter with the turn's
//! token in it, in private files that live as long as the CLI; its stream-json output kept as it comes.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  account::AccountCli,
  command::Command,
  connection::{
    AgentProvider, ConnectionAccount,
    invocation::{ClaudeChat, ClaudeMcpConfig, ClaudeSession, HttpAuthorization, McpLaunch, on_account},
    mcp::{McpServerName, McpUrl},
    stream::ClaudeStream,
  },
  id::AgentAccountId,
  orchestrator::{
    TurnFailure, TurnState,
    instructions::{ProjectReads, SERVER, system_prompt},
  },
  secret::SecretValue,
};
use tokio::sync::oneshot;

use super::turn::{Prepared, Seen, Turn, cannot};
use crate::{
  private_file::{self, PrivateFile},
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

/// The CLI's command and the files it reads, which must outlive it, and the account it runs on.
struct Started {
  command: Command,
  account: AgentAccountId,
  _files: [PrivateFile; 2],
}

/// The places that hold secrets the chat's agent must not read, as folders and files: the runner's private files
/// (every running agent's MCP configuration with its tokens, and other chats' tokens), the account's CLI folder (its
/// sign-in) and, for the Default account kept in the home folder, `~/.claude.json`. Each folder is named as it is
/// given and as the file system resolves it, so a short (8.3) or linked spelling of it is denied too.
fn denied(account: &ConnectionAccount) -> (Vec<PathBuf>, Vec<PathBuf>) {
  let cli = AccountCli::ClaudeCode;
  let variable = std::env::var_os(cli.folder_variable()).filter(|value| !value.is_empty());
  let home = std::env::home_dir();
  let own = || {
    variable
      .clone()
      .map(PathBuf::from)
      .or_else(|| home.as_ref().map(|home| home.join(cli.own_folder_name())))
  };
  let account_folder = account.folder().map(Path::to_path_buf).or_else(own);
  let folders = [Some(private_file::root()), account_folder]
    .into_iter()
    .flatten()
    .flat_map(|folder| {
      // A folder that cannot be resolved (one not made yet) is denied as it is named.
      let resolved = std::fs::canonicalize(&folder)
        .ok()
        .filter(|resolved| *resolved != folder);
      std::iter::once(folder).chain(resolved)
    })
    .collect();
  let files = match (account.folder(), &variable, &home) {
    (None, None, Some(home)) => vec![home.join(format!("{}.json", cli.own_folder_name()))],
    _ => Vec::new(),
  };
  (folders, files)
}

impl Turn {
  pub(super) async fn run_claude(&self, prepared: Prepared, stop: oneshot::Receiver<()>) -> TurnState {
    match self.start_claude(&prepared) {
      Ok(started) => self.drive_claude(started, stop).await,
      Err(failure) => TurnState::Failed(failure),
    }
  }

  /// The command that answers the message, or why none can start.
  fn start_claude(&self, prepared: &Prepared) -> Result<Started, TurnFailure> {
    let name = self.message.to_string();
    let prompt = PrivateFile::write(
      self.project,
      &format!("chat-prompt-{name}.md"),
      &system_prompt(&prepared.project_name, self.project, ProjectReads::FileTools),
    )
    .map_err(|error| cannot(format!("cannot write the system prompt: {error}")))?;
    let mcp = PrivateFile::write(
      self.project,
      &format!("chat-mcp-{name}.json"),
      self.claude_mcp_config()?.contents(),
    )
    .map_err(|error| cannot(format!("cannot write the MCP configuration: {error}")))?;
    let (denied, denied_files) = denied(prepared.connection.account());
    let command = ClaudeChat {
      executable: &prepared.executable,
      model: self.settings.model().as_str(),
      effort: self.settings.effort(),
      folder: &self.root,
      session: ClaudeSession::continuing(prepared.resumed.as_deref()),
      system_prompt: prompt.path(),
      mcp_config: mcp.path(),
      denied: &denied,
      denied_files: &denied_files,
    }
    .command(
      (self.text.as_str(), &self.attached),
      std::env::vars_os(),
      &prepared.environment,
    )
    .and_then(|command| on_account(command, prepared.connection.account(), AgentProvider::ClaudeCodeCli))
    .map_err(|error| cannot(error.to_string()))?;
    Ok(Started {
      command,
      account: prepared.account,
      _files: [prompt, mcp],
    })
  }

  /// The domain tools as Claude Code's MCP configuration: the chat's endpoint with its token.
  fn claude_mcp_config(&self) -> Result<ClaudeMcpConfig, TurnFailure> {
    let (config, _) = ClaudeMcpConfig::new(&[self.tools_launch()?]);
    config.ok_or_else(|| cannot("the tools' server was left out".to_owned()))
  }

  /// The chat's domain tools as an MCP server: its endpoint with the turn's token.
  pub(super) fn tools_launch(&self) -> Result<McpLaunch, TurnFailure> {
    let name = McpServerName::new(SERVER).map_err(|_| cannot("the tools' server name is not valid".to_owned()))?;
    let url = McpUrl::new(&self.tools.url).map_err(|_| cannot("the tools' address is not valid".to_owned()))?;
    let token = SecretValue::new(self.tools.token.expose().to_owned())
      .map_err(|_| cannot("the tools' token is blank".to_owned()))?;
    Ok(McpLaunch::http(name, url, HttpAuthorization::Bearer(Arc::new(token))))
  }

  /// Runs the CLI, keeping what it reports, until it ends or `stop` ends it.
  async fn drive_claude(&self, started: Started, mut stop: oneshot::Receiver<()>) -> TurnState {
    let mut process = match Supervised::start(&started.command) {
      Ok(process) => process,
      Err(error) => return TurnState::Failed(cannot(error.to_string())),
    };
    let mut seen = Seen::default();
    let mut stream = ClaudeStream::chat();
    let limit = tokio::time::sleep(self.time_limit);
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
          seen.timed_out = Some(TurnFailure::TimeLimit { minutes: self.time_limit.as_secs() / 60 });
          process.terminate();
        }
      }
    }
  }
}
