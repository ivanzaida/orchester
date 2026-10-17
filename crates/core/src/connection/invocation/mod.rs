//! How an agent step starts its CLI: the provider's argv ([`CodexInvocation`],
//! [`ClaudeInvocation`]), the worktree as working directory, the prompt on standard input and a
//! cleaned environment ([`agent_environment`]) with the variables the project adds.

mod chat;
mod claude;
mod codex;
pub mod mcp;
mod orchester;
mod provider;

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

pub use self::{
  chat::ClaudeChat,
  claude::{ClaudeInvocation, ClaudeMcp, ClaudeSession, input_line, result_schema},
  codex::{CodexAgentFiles, CodexInvocation, ISOLATION as CODEX_ISOLATION},
  mcp::{ClaudeMcpConfig, CodexMcp, LaunchValue, LeftOut, McpLaunch},
  orchester::CodexOrchester,
  provider::{CLAUDE_KEY_VARIABLE, CODEX_KEY_VARIABLE, CODEX_PROVIDER_ID, OnProvider, codex_overrides},
};
use crate::{
  command::{Command, SecretVariable},
  connection::{AgentProvider, ConnectionAccount},
  resource::CommandEnvironment,
};

/// The operating system an invocation is built for. Builders take it as a value so both
/// variants can be built and tested on either platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
  Windows,
  MacOs,
}

impl Platform {
  pub const fn current() -> Self {
    if cfg!(windows) { Self::Windows } else { Self::MacOs }
  }
}

/// Variables an agent CLI may see from the environment Orchester runs in. Everything else is
/// withheld: credentials of other tools (`GH_TOKEN`, `GITHUB_TOKEN`, `OPENAI_API_KEY`,
/// `ANTHROPIC_*`), the variables of an agent host Orchester itself runs in (`CLAUDECODE`,
/// `CLAUDE_CODE_*`, `ANTHROPIC_BASE_URL`, `CODEX_*` other than `CODEX_HOME`: with them the Claude
/// Code CLI answered "Not logged in", `.tmp/cli-probe/REPORT.md`) and anything else a user's shell
/// set. What stays is what programs need to start, find their tools, their home and temporary
/// folders, the locale, a proxy the network requires, and where each CLI keeps its own sign-in
/// (`CODEX_HOME`, `CLAUDE_CONFIG_DIR`).
const ALLOWED: &[&str] = &[
  // Both platforms.
  "PATH",
  "HOME",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
  "TZ",
  "CODEX_HOME",
  "CLAUDE_CONFIG_DIR",
  "HTTP_PROXY",
  "HTTPS_PROXY",
  "NO_PROXY",
  "SSL_CERT_FILE",
  "SSL_CERT_DIR",
  // macOS and other Unix systems.
  "USER",
  "LOGNAME",
  "SHELL",
  "TMPDIR",
  // Windows: the system, user profile and program folders programs look up.
  "SYSTEMROOT",
  "SYSTEMDRIVE",
  "WINDIR",
  "COMSPEC",
  "PATHEXT",
  "TEMP",
  "TMP",
  "USERPROFILE",
  "USERNAME",
  "USERDOMAIN",
  "HOMEDRIVE",
  "HOMEPATH",
  "APPDATA",
  "LOCALAPPDATA",
  "PROGRAMDATA",
  "PROGRAMFILES",
  "PROGRAMFILES(X86)",
  "PROGRAMW6432",
  "COMMONPROGRAMFILES",
  "COMMONPROGRAMFILES(X86)",
  "COMMONPROGRAMW6432",
  "COMPUTERNAME",
  "OS",
  "PROCESSOR_ARCHITECTURE",
  "NUMBER_OF_PROCESSORS",
  "PSMODULEPATH",
];

/// The variables of `environment` an agent CLI may see, in their original spelling. Names are
/// compared without regard to case, as Windows does; variables that are not Unicode are left
/// out rather than passed on mangled.
pub fn agent_environment(environment: impl IntoIterator<Item = (OsString, OsString)>) -> Vec<(String, String)> {
  environment
    .into_iter()
    .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
    .filter(|(name, _)| is_agent_variable(name))
    .collect()
}

/// Whether an agent CLI starts with `name` from the environment Orchester runs in (compared
/// without regard to case).
pub fn is_agent_variable(name: &str) -> bool {
  ALLOWED.iter().any(|allowed| allowed.eq_ignore_ascii_case(name))
}

/// `command`, an agent CLI's of `provider`, on the connection's `account`: with the CLI's folder variable
/// (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`) set to the account's folder, over whatever the environment or the project set,
/// so the CLI uses that folder's sign-in, settings and sessions. Default leaves the variable as it is.
pub fn on_account(
  command: Command,
  account: &ConnectionAccount,
  provider: AgentProvider,
) -> Result<Command, InvocationError> {
  Ok(match account_variable(account, provider)? {
    Some((name, folder)) => command.with_env_replaced(name, folder),
    None => command,
  })
}

/// `variables`, an agent CLI's environment, on the connection's `account` ([`on_account`]).
pub fn account_environment(
  mut variables: Vec<(String, String)>,
  account: &ConnectionAccount,
  provider: AgentProvider,
) -> Result<Vec<(String, String)>, InvocationError> {
  if let Some((name, folder)) = account_variable(account, provider)? {
    variables.retain(|(variable, _)| !variable.eq_ignore_ascii_case(name));
    variables.push((name.to_owned(), folder.to_owned()));
  }
  Ok(variables)
}

/// The account's folder variable and its value, which must be Unicode to reach the CLI unmangled.
fn account_variable(
  account: &ConnectionAccount,
  provider: AgentProvider,
) -> Result<Option<(&'static str, &str)>, InvocationError> {
  let Some((name, folder)) = account.variable(provider) else {
    return Ok(None);
  };
  let value = folder
    .to_str()
    .ok_or_else(|| InvocationError::NotUnicode(folder.to_path_buf()))?;
  Ok(Some((name, value)))
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InvocationError {
  #[error("the path {0} is not valid Unicode")]
  NotUnicode(PathBuf),
  /// The permissions ask a person, but the invocation has no way to ask.
  #[error("the step asks a person before some actions, but the agent was not given the permission prompt")]
  NoPermissionPrompt,
  /// The step edits by `Edit` allow rules, and none can name this folder (a network share).
  #[error("edits on a network share can't be allowed by rule: {0}")]
  NoEditRule(PathBuf),
}

/// Refuses a folder the CLI would get mangled, before its arguments are built.
fn unicode_folders(folders: &[PathBuf]) -> Result<(), InvocationError> {
  match folders.iter().find(|folder| folder.to_str().is_none()) {
    Some(folder) => Err(InvocationError::NotUnicode(folder.clone())),
    None => Ok(()),
  }
}

/// The command of an agent CLI: `args` after the program, in the worktree, reading a line protocol
/// on standard input that starts with `first` and stays open, the allowed part of `environment`, the variables the
/// project adds (`project`) and `secrets`, each replacing ones of the same name.
fn agent_command(
  executable: &Path,
  args: Vec<OsString>,
  worktree: &Path,
  first: String,
  (environment, project, secrets): (
    impl IntoIterator<Item = (OsString, OsString)>,
    &CommandEnvironment,
    &[SecretVariable],
  ),
) -> Result<Command, InvocationError> {
  let args = args
    .into_iter()
    .map(|arg| {
      arg
        .into_string()
        .map_err(|_| InvocationError::NotUnicode(worktree.to_path_buf()))
    })
    .collect::<Result<Vec<_>, _>>()?;
  let mut variables = agent_environment(environment);
  project.add_to(&mut variables);
  let command = Command::new(executable, args)
    .with_current_dir(worktree)
    .with_environment(variables)
    .with_interactive_input(first);
  Ok(secrets.iter().cloned().fold(command, Command::with_secret_env))
}
