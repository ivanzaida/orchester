//! How an agent step starts the Codex CLI (`.tmp/cli-probe/REPORT.md`, "Recommended argv"):
//!
//! `codex -a never exec --json --ignore-user-config --ignore-rules -m <model>
//!  [-c model_reasoning_effort=<e>] --sandbox <mode> [-c windows.sandbox=elevated] -C <worktree> -`
//!
//! - `-a never` is a top-level flag: after `exec` it is an error. With it nothing ever waits for an approval; whatever
//!   the sandbox does not allow is declined and the agent is told.
//! - Permissions are enforced by Codex's own sandbox; bypass modes are never used. Read-only steps run in `read-only`;
//!   writing steps in `workspace-write`, which on Windows only confines writes with `windows.sandbox=elevated` (without
//!   it every command is declined).
//! - `--ignore-user-config` and `--ignore-rules` keep the user's own configuration (MCP servers, default model,
//!   execution rules) out of the run.
//! - The prompt goes on standard input (`-`); free-form text in arguments would not survive the `.cmd` launcher on
//!   Windows.
//! - The environment is cleaned: see [`agent_environment`].

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use crate::{command::Command, resource::RepositoryAccess};

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
/// `ANTHROPIC_*`), the variables of an agent host Orchester itself runs in (`CLAUDE*`,
/// `CODEX_*` other than `CODEX_HOME`) and anything else a user's shell set. What stays is what
/// programs need to start, find their tools, their home and temporary folders, the locale, a
/// proxy the network requires, and where the CLI keeps its own sign-in (`CODEX_HOME`).
const ALLOWED: &[&str] = &[
  // Both platforms.
  "PATH",
  "HOME",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
  "TZ",
  "CODEX_HOME",
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

/// Everything that decides how one Codex invocation starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexInvocation<'a> {
  pub executable: &'a Path,
  pub model: &'a str,
  /// The profile's reasoning effort; `None` leaves Codex's default.
  pub reasoning_effort: Option<&'a str>,
  pub access: RepositoryAccess,
  pub worktree: &'a Path,
  pub platform: Platform,
}

impl CodexInvocation<'_> {
  /// Arguments after the program, in order.
  pub fn args(&self) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
      "-a",
      "never",
      "exec",
      "--json",
      "--ignore-user-config",
      "--ignore-rules",
      "-m",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(self.model.into());
    if let Some(effort) = self.reasoning_effort {
      args.push("-c".into());
      args.push(format!("model_reasoning_effort={effort}").into());
    }
    args.push("--sandbox".into());
    match (self.access, self.platform) {
      (RepositoryAccess::ReadOnly, _) => args.push("read-only".into()),
      (RepositoryAccess::Write, Platform::MacOs) => args.push("workspace-write".into()),
      (RepositoryAccess::Write, Platform::Windows) => {
        args.push("workspace-write".into());
        args.push("-c".into());
        args.push("windows.sandbox=elevated".into());
      }
    }
    args.push("-C".into());
    args.push(self.worktree.as_os_str().to_owned());
    args.push("-".into());
    args
  }

  /// The command that runs the step: in the worktree, with `prompt` on standard input and only
  /// the allowed part of `environment`.
  pub fn command(
    &self,
    prompt: String,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
  ) -> Result<Command, InvocationError> {
    let args = self
      .args()
      .into_iter()
      .map(|arg| {
        arg
          .into_string()
          .map_err(|_| InvocationError::NotUnicode(self.worktree.to_path_buf()))
      })
      .collect::<Result<Vec<_>, _>>()?;
    Ok(
      Command::new(self.executable, args)
        .with_current_dir(self.worktree)
        .with_environment(agent_environment(environment))
        .with_stdin(prompt),
    )
  }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InvocationError {
  #[error("the worktree path {0} is not valid Unicode")]
  NotUnicode(PathBuf),
}
