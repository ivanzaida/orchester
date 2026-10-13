//! What checking an account found: the CLI's own status command run with the account's folder
//! (`claude auth status --json`, `codex login status`). Only who is signed in is read, never a credential.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::AccountCli;
use crate::{
  command::{Command, CommandRunner},
  connection::{SignIn, parse_claude_auth_status, parse_codex_login_status, parse_version},
  time::Timestamp,
};

/// Who the CLI says is signed in, as far as it says: the design shows each part only if the CLI reports it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountIdentity {
  pub email: Option<String>,
  pub organization: Option<String>,
  /// The subscription as the CLI names it (`max`, `pro`, …).
  pub plan: Option<String>,
  /// How the CLI signed in, as it names it (`claude.ai`, `ChatGPT`, …).
  pub method: Option<String>,
}

/// The program a check ran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedProgram {
  pub path: PathBuf,
  pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AccountCheckResult {
  SignedIn {
    program: CheckedProgram,
    identity: AccountIdentity,
  },
  /// The CLI says nobody is signed in to the folder; `was` who was before (*Sign-in expired*).
  SignedOut {
    program: CheckedProgram,
    was: Option<AccountIdentity>,
  },
  /// The CLI answered its status command in a way Orchester can't read: runs still start.
  SignInUnknown {
    program: CheckedProgram,
    last_known: Option<AccountIdentity>,
  },
  /// No CLI on the `PATH`, or it did not answer as the CLI: nothing to check with.
  CliMissing { last_known: Option<AccountIdentity> },
  /// The account's folder is gone: runs on it don't start.
  FolderMissing { last_known: Option<AccountIdentity> },
}

/// A check of an account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountCheck {
  pub at: Timestamp,
  pub result: AccountCheckResult,
}

/// An account's state, as its list row and header show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccountState {
  NotChecked,
  SignedIn,
  NotSignedIn,
  Expired,
  Unknown,
  CliMissing,
  FolderMissing,
  /// Not supported on this platform yet (Claude Code accounts other than Default on macOS).
  Unsupported,
}

impl AccountState {
  /// Stable name for MCP and the window's element tree.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::NotChecked => "not_checked",
      Self::SignedIn => "signed_in",
      Self::NotSignedIn => "not_signed_in",
      Self::Expired => "sign_in_expired",
      Self::Unknown => "sign_in_unknown",
      Self::CliMissing => "cli_not_found",
      Self::FolderMissing => "folder_missing",
      Self::Unsupported => "not_supported",
    }
  }

  /// Whether a run may start on the account: never on one that is not signed in, misses its folder or is not
  /// supported here. Not checked yet and unknown sign-ins start, and the run says so if the CLI refuses.
  pub const fn lets_runs_start(self) -> bool {
    matches!(
      self,
      Self::NotChecked | Self::SignedIn | Self::Unknown | Self::CliMissing
    )
  }
}

impl AccountCheck {
  pub const fn state(&self) -> AccountState {
    match &self.result {
      AccountCheckResult::SignedIn { .. } => AccountState::SignedIn,
      AccountCheckResult::SignedOut { was: Some(_), .. } => AccountState::Expired,
      AccountCheckResult::SignedOut { was: None, .. } => AccountState::NotSignedIn,
      AccountCheckResult::SignInUnknown { .. } => AccountState::Unknown,
      AccountCheckResult::CliMissing { .. } => AccountState::CliMissing,
      AccountCheckResult::FolderMissing { .. } => AccountState::FolderMissing,
    }
  }

  /// Who is or was last known to be signed in.
  pub const fn known_identity(&self) -> Option<&AccountIdentity> {
    match &self.result {
      AccountCheckResult::SignedIn { identity, .. } => Some(identity),
      AccountCheckResult::SignedOut { was, .. }
      | AccountCheckResult::SignInUnknown { last_known: was, .. }
      | AccountCheckResult::CliMissing { last_known: was }
      | AccountCheckResult::FolderMissing { last_known: was } => was.as_ref(),
    }
  }

  /// The program that answered.
  pub const fn program(&self) -> Option<&CheckedProgram> {
    match &self.result {
      AccountCheckResult::SignedIn { program, .. }
      | AccountCheckResult::SignedOut { program, .. }
      | AccountCheckResult::SignInUnknown { program, .. } => Some(program),
      AccountCheckResult::CliMissing { .. } | AccountCheckResult::FolderMissing { .. } => None,
    }
  }

  /// This check with `known`, who was signed in before, where it could not say who is.
  #[must_use]
  pub(super) fn remembering(mut self, known: Option<AccountIdentity>) -> Self {
    match &mut self.result {
      AccountCheckResult::SignedIn { .. } => {}
      AccountCheckResult::SignedOut { was: slot, .. }
      | AccountCheckResult::SignInUnknown { last_known: slot, .. }
      | AccountCheckResult::CliMissing { last_known: slot }
      | AccountCheckResult::FolderMissing { last_known: slot } => {
        if slot.is_none() {
          *slot = known;
        }
      }
    }
    self
  }

  /// Checks an account of `cli` whose folder exists, with the CLI at `program` (`None`: not on the `PATH`). The CLI
  /// sees `environment`, the one agents run with, with the account's folder variable already in it.
  pub fn run(
    cli: AccountCli,
    program: Option<&Path>,
    runner: &impl CommandRunner,
    environment: &[(String, String)],
    now: Timestamp,
  ) -> Self {
    let result = program.map_or(AccountCheckResult::CliMissing { last_known: None }, |program| {
      ask(cli, program, runner, environment)
    });
    Self { at: now, result }
  }

  /// The check of an account whose folder is gone.
  pub const fn folder_missing(now: Timestamp) -> Self {
    Self {
      at: now,
      result: AccountCheckResult::FolderMissing { last_known: None },
    }
  }
}

fn ask(
  cli: AccountCli,
  program: &Path,
  runner: &impl CommandRunner,
  environment: &[(String, String)],
) -> AccountCheckResult {
  let command = |args: &[&str]| Command::new(program, args.iter().copied()).with_environment(environment.to_vec());
  // A program that can't run or doesn't answer as the CLI is nothing to check with, whatever the reason.
  let version = runner
    .run(&command(&["--version"]))
    .ok()
    .and_then(|output| parse_version(cli.provider(), &output));
  let Some(version) = version else {
    return AccountCheckResult::CliMissing { last_known: None };
  };
  let program = CheckedProgram {
    path: program.to_path_buf(),
    version,
  };
  let (sign_in, identity) = match cli {
    AccountCli::ClaudeCode => match runner.run(&command(&["auth", "status", "--json"])) {
      Ok(output) => (parse_claude_auth_status(&output), parse_claude_identity(&output.stdout)),
      Err(_) => (SignIn::Unknown, AccountIdentity::default()),
    },
    AccountCli::Codex => match runner.run(&command(cli.status_args())) {
      Ok(output) => (parse_codex_login_status(&output), codex_identity(&output.stdout)),
      Err(_) => (SignIn::Unknown, AccountIdentity::default()),
    },
  };
  match sign_in {
    SignIn::SignedIn => AccountCheckResult::SignedIn { program, identity },
    SignIn::SignedOut => AccountCheckResult::SignedOut { program, was: None },
    SignIn::Unknown => AccountCheckResult::SignInUnknown {
      program,
      last_known: None,
    },
  }
}

/// Who `claude auth status --json` names, where it does: `email`, `orgName`, `subscriptionType` and `authMethod`.
/// Only `loggedIn` was verified (2026-10-12, signed out on purpose); the rest is read when present and left out
/// otherwise, never guessed.
pub fn parse_claude_identity(stdout: &str) -> AccountIdentity {
  #[derive(Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Status {
    email: Option<String>,
    org_name: Option<String>,
    subscription_type: Option<String>,
    auth_method: Option<String>,
  }
  let Ok(status) = serde_json::from_str::<Status>(stdout.trim()) else {
    return AccountIdentity::default();
  };
  let present = |value: Option<String>| value.filter(|value| !value.trim().is_empty() && value != "none");
  AccountIdentity {
    email: present(status.email),
    organization: present(status.org_name),
    plan: present(status.subscription_type),
    method: present(status.auth_method),
  }
}

/// `codex login status` names only the method (`Logged in using ChatGPT`); who is signed in comes with App Server's
/// `account/read`, which v1 does not use.
fn codex_identity(stdout: &str) -> AccountIdentity {
  let method = stdout
    .lines()
    .find_map(|line| line.trim().strip_prefix("Logged in using "))
    .map(|method| method.trim().to_owned())
    .filter(|method| !method.is_empty());
  AccountIdentity {
    method,
    ..AccountIdentity::default()
  }
}
