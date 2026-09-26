//! How an agent step starts the Codex CLI (`.tmp/cli-probe/REPORT.md`, "Recommended argv"):
//!
//! `codex -a never exec --json --ignore-user-config --ignore-rules -m <model>
//!  [-c model_reasoning_effort=<e>] --sandbox <mode> [-c windows.sandbox=elevated]
//!  [-c sandbox_workspace_write.network_access=true] [--output-schema <file>] -C <worktree> -`
//!
//! and continues a session after a pause or a limit with
//!
//! `codex -a never exec resume --json --ignore-user-config --ignore-rules -m <model>
//!  [-c model_reasoning_effort=<e>] -c sandbox_mode=<mode> [-c windows.sandbox=elevated]
//!  [-c sandbox_workspace_write.network_access=true] <session> -`
//!
//! in the worktree as the process's working directory.
//!
//! - `-a never` is a top-level flag: after `exec` it is an error. With it nothing ever waits for an approval; whatever
//!   the sandbox does not allow is declined and the agent is told.
//! - Permissions are enforced by Codex's own sandbox; bypass modes are never used. Read-only steps run in `read-only`;
//!   writing steps in `workspace-write`, which on Windows only confines writes with `windows.sandbox=elevated` (without
//!   it every command is declined).
//! - The mode and network access come from the step's enforced permissions
//!   ([`crate::permission::AgentPermissions::enforced_on`]); commands are always those the sandbox allows, since Codex
//!   cannot enforce a command list. Network access is a setting of the `workspace-write` sandbox only; the `read-only`
//!   sandbox has none.
//! - `--ignore-user-config` and `--ignore-rules` keep the user's own configuration (MCP servers, default model,
//!   execution rules) out of the run.
//! - The prompt goes on standard input (`-`); free-form text in arguments would not survive the `.cmd` launcher on
//!   Windows.
//! - The environment is cleaned: see [`agent_environment`].
//! - `--output-schema` makes the final agent message JSON of the step result schema
//!   ([`crate::run::STEP_RESULT_SCHEMA`]). The probe verified it for new sessions only, so a continued session is
//!   started without it and its result is read from its exit.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use crate::{command::Command, permission::NetworkAccess, resource::RepositoryAccess};

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
  /// Network access of a `workspace-write` sandbox.
  pub network: NetworkAccess,
  pub worktree: &'a Path,
  pub platform: Platform,
  /// The session to continue (`codex exec resume`), for an invocation that repeats the
  /// unfinished part of a step; `None` starts a new session.
  pub resume: Option<&'a str>,
  /// The file holding the step result schema, for a new session.
  pub output_schema: Option<&'a Path>,
}

impl CodexInvocation<'_> {
  /// Arguments after the program, in order.
  pub fn args(&self) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-a", "never", "exec"].into_iter().map(OsString::from).collect();
    if self.resume.is_some() {
      args.push("resume".into());
    }
    args.extend(
      ["--json", "--ignore-user-config", "--ignore-rules", "-m", self.model]
        .into_iter()
        .map(OsString::from),
    );
    if let Some(effort) = self.reasoning_effort {
      args.push("-c".into());
      args.push(format!("model_reasoning_effort={effort}").into());
    }
    let mode = match self.access {
      RepositoryAccess::ReadOnly => "read-only",
      RepositoryAccess::Write => "workspace-write",
    };
    // `exec resume` has no `--sandbox` or `-C`: the sandbox is set through its configuration key
    // and the working directory is the process's.
    if self.resume.is_some() {
      args.push("-c".into());
      args.push(format!("sandbox_mode={mode}").into());
    } else {
      args.push("--sandbox".into());
      args.push(mode.into());
    }
    if (self.access, self.platform) == (RepositoryAccess::Write, Platform::Windows) {
      args.push("-c".into());
      args.push("windows.sandbox=elevated".into());
    }
    if (self.access, self.network) == (RepositoryAccess::Write, NetworkAccess::On) {
      args.push("-c".into());
      args.push("sandbox_workspace_write.network_access=true".into());
    }
    match self.resume {
      Some(session) => args.push(session.into()),
      None => {
        if let Some(schema) = self.output_schema {
          args.push("--output-schema".into());
          args.push(schema.as_os_str().to_owned());
        }
        args.push("-C".into());
        args.push(self.worktree.as_os_str().to_owned());
      }
    }
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
