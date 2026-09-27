//! How an agent step starts its CLI: the provider's argv ([`CodexInvocation`],
//! [`ClaudeInvocation`]), the worktree as working directory, the prompt on standard input and a
//! cleaned environment ([`agent_environment`]) with the variables the project adds.

mod claude;
mod codex;

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

pub use self::{
  claude::{ClaudeInvocation, ClaudeSession},
  codex::CodexInvocation,
};
use crate::{command::Command, resource::CommandEnvironment};

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
    .filter(|(name, _)| ALLOWED.iter().any(|allowed| allowed.eq_ignore_ascii_case(name)))
    .collect()
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InvocationError {
  #[error("the worktree path {0} is not valid Unicode")]
  NotUnicode(PathBuf),
}

/// The command of an agent CLI: `args` after the program, in the worktree, with `prompt` on
/// standard input, the allowed part of `environment` and the variables the project adds
/// (`project`), which replace ones of the same name.
fn agent_command(
  executable: &Path,
  args: Vec<OsString>,
  worktree: &Path,
  prompt: String,
  (environment, project): (impl IntoIterator<Item = (OsString, OsString)>, &CommandEnvironment),
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
  Ok(
    Command::new(executable, args)
      .with_current_dir(worktree)
      .with_environment(variables)
      .with_stdin(prompt),
  )
}
