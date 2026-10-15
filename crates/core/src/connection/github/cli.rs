//! The `gh` commands Orchester runs to check a GitHub connection and how it reads the answers.
//! Only read-only commands are built: `gh --version` and `gh auth status --hostname <host> --json
//! hosts` ("Display active account and authentication state"; with `--json` it always exits 0
//! and prints no token unless `--show-token` is given, which is never passed). Nothing here signs
//! in or out, switches accounts, refreshes tokens, sets up git credentials or changes `gh`'s
//! configuration.

use std::{collections::HashMap, path::Path};

use serde::Deserialize;

use super::{GitHubCheck, GitHubCheckResult, GitHubHost, GitHubLogin};
use crate::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::SignIn,
  time::Timestamp,
};

/// Name of the GitHub CLI program on the `PATH`.
pub const GH_PROGRAM: &str = "gh";

/// Set for every `gh` run: never prompt (there is no terminal to answer), no update notice and
/// no color codes in the output that is parsed.
const GH_ENV: [(&str, &str); 3] = [
  ("GH_PROMPT_DISABLED", "1"),
  ("GH_NO_UPDATE_NOTIFIER", "1"),
  ("NO_COLOR", "1"),
];
/// Hidden from every `gh` run: debug logging writes request details to the captured stderr,
/// and forced terminal output or color would change what is parsed. `GH_TOKEN` and
/// `GH_CONFIG_DIR` stay, since they are how the user signed `gh` in.
const GH_ENV_REMOVED: [&str; 4] = ["GH_DEBUG", "DEBUG", "GH_FORCE_TTY", "CLICOLOR_FORCE"];

/// `gh` with `args` and the environment every run gets.
pub(crate) fn gh_command(executable: &Path, args: Vec<String>) -> Command {
  let command = GH_ENV
    .iter()
    .fold(Command::new(executable, args), |command, (key, value)| {
      command.with_env(key, value)
    });
  GH_ENV_REMOVED
    .iter()
    .fold(command, |command, key| command.without_env(key))
}

pub fn gh_version_command(executable: &Path) -> Command {
  gh_command(executable, vec!["--version".to_owned()])
}

pub fn gh_auth_status_command(executable: &Path, host: &GitHubHost) -> Command {
  let args = ["auth", "status", "--hostname", host.as_str(), "--json", "hosts"];
  gh_command(executable, args.map(str::to_owned).to_vec())
}

/// The version number in `gh --version` output (`gh version 2.97.0 (2026-07-31)`), if the
/// output is the GitHub CLI's.
pub fn parse_gh_version(output: &CommandOutput) -> Option<String> {
  if !output.success() {
    return None;
  }
  let rest = output.stdout.trim_start().strip_prefix("gh version ")?;
  rest
    .split_whitespace()
    .next()
    .filter(|version| version.starts_with(|c: char| c.is_ascii_digit()))
    .map(str::to_owned)
}

/// The active account of one host as `gh auth status --json hosts` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhAuthStatus {
  pub sign_in: SignIn,
  pub account: Option<GitHubLogin>,
  pub scopes: Vec<String>,
}

/// Reads the active account on `host` from `gh auth status --json hosts`. A host without
/// accounts is signed out; `None` means the output is not the expected JSON. Fields other than
/// the ones below are ignored, so even output with a token in it would not be kept.
pub fn parse_gh_auth_status(host: &GitHubHost, output: &CommandOutput) -> Option<GhAuthStatus> {
  #[derive(Deserialize)]
  struct Status {
    hosts: HashMap<String, Vec<Entry>>,
  }
  #[derive(Deserialize)]
  struct Entry {
    state: String,
    active: bool,
    #[serde(default)]
    login: String,
    #[serde(default)]
    scopes: Option<String>,
  }
  // Signed out of every host, `gh` writes the empty status and exits with 1: the status, not its exit code,
  // says how it stands. A failure without a status leaves the sign-in unknown.
  let status: Status = serde_json::from_str(output.stdout.trim()).ok()?;
  let Some(entry) = status
    .hosts
    .get(host.as_str())
    .and_then(|entries| entries.iter().find(|entry| entry.active))
  else {
    return Some(GhAuthStatus {
      sign_in: SignIn::SignedOut,
      account: None,
      scopes: Vec::new(),
    });
  };
  let account = GitHubLogin::new(&entry.login).ok();
  let sign_in = match (entry.state.as_str(), &account) {
    ("success", Some(_)) => SignIn::SignedIn,
    ("error", _) => SignIn::SignedOut,
    _ => SignIn::Unknown,
  };
  let scopes = entry
    .scopes
    .as_deref()
    .unwrap_or_default()
    .split(',')
    .map(str::trim)
    .filter(|scope| !scope.is_empty())
    .map(str::to_owned)
    .collect();
  Some(GhAuthStatus {
    sign_in,
    account,
    scopes,
  })
}

/// Checks the GitHub CLI at `executable`: its version and the active account on `host`.
pub fn check_github(executable: &Path, host: &GitHubHost, runner: &impl CommandRunner, now: Timestamp) -> GitHubCheck {
  let result = match runner.run(&gh_version_command(executable)) {
    Err(CommandError::NotFound) => GitHubCheckResult::ExecutableMissing,
    Err(
      CommandError::TimedOut
      | CommandError::Failed(_)
      | CommandError::NotStarted(_)
      | CommandError::UnsafeScriptArgument { .. },
    ) => GitHubCheckResult::CouldNotRun,
    Ok(output) => match parse_gh_version(&output) {
      None => GitHubCheckResult::NotRecognized,
      Some(version) => {
        let status = auth_status(executable, host, runner);
        GitHubCheckResult::Available {
          version,
          account: status.account,
          scopes: status.scopes,
          sign_in: status.sign_in,
        }
      }
    },
  };
  GitHubCheck { at: now, result }
}

fn auth_status(executable: &Path, host: &GitHubHost, runner: &impl CommandRunner) -> GhAuthStatus {
  // A status command that cannot run or answers unexpectedly leaves the sign-in unknown, not
  // signed out.
  runner
    .run(&gh_auth_status_command(executable, host))
    .ok()
    .and_then(|output| parse_gh_auth_status(host, &output))
    .unwrap_or(GhAuthStatus {
      sign_in: SignIn::Unknown,
      account: None,
      scopes: Vec::new(),
    })
}
