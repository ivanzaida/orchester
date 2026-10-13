//! Agent CLI accounts (`docs/design/agent-accounts.md`): the person's sign-ins to Claude Code and Codex on this
//! computer, shared by every project. An account is a CLI, a name and a folder where the CLI keeps its own sign-in,
//! settings and sessions; Orchester starts the CLI with `CLAUDE_CONFIG_DIR` / `CODEX_HOME` set to it. Each CLI has a
//! fixed Default account, the CLI's own folder, which leaves the variable as the environment has it. Orchester never
//! reads, stores or shows a credential: it runs only the CLI's status command and opens its login in a terminal the
//! person sees.

mod check;
mod folder;
mod removal;
mod sign_in;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub use self::{
  check::{
    AccountCheck, AccountCheckResult, AccountIdentity, AccountState, CheckedProgram, SignInMethod,
    parse_claude_identity,
  },
  folder::{
    FolderProblem, account_folder_name, brought_folder_problem, plain_full_path, resolved, same_folder, within,
  },
  removal::{MoveTo, Removal, RemovalError},
  sign_in::{Shell, sign_in_command},
};
use crate::{
  connection::{AgentProvider, invocation::Platform},
  id::AgentAccountId,
  name::Name,
  time::Timestamp,
};

/// The agent CLIs an account can be added for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountCli {
  ClaudeCode,
  Codex,
}

impl AccountCli {
  /// In the order Settings › Accounts lists them.
  pub const ALL: [Self; 2] = [Self::ClaudeCode, Self::Codex];

  pub const fn of(provider: AgentProvider) -> Option<Self> {
    match provider {
      AgentProvider::ClaudeCodeCli => Some(Self::ClaudeCode),
      AgentProvider::CodexCli => Some(Self::Codex),
      AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => None,
    }
  }

  pub const fn provider(self) -> AgentProvider {
    match self {
      Self::ClaudeCode => AgentProvider::ClaudeCodeCli,
      Self::Codex => AgentProvider::CodexCli,
    }
  }

  /// The program's name on the `PATH`.
  pub const fn program(self) -> &'static str {
    match self {
      Self::ClaudeCode => "claude",
      Self::Codex => "codex",
    }
  }

  /// Stored and parsed in the application database.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::ClaudeCode => "claude_code",
      Self::Codex => "codex",
    }
  }

  pub fn parse(text: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|cli| cli.as_str() == text)
  }

  /// The variable that names the folder the CLI keeps its sign-in, settings and sessions in.
  pub const fn folder_variable(self) -> &'static str {
    match self {
      Self::ClaudeCode => "CLAUDE_CONFIG_DIR",
      Self::Codex => "CODEX_HOME",
    }
  }

  /// The CLI's own folder in the home folder, used when the variable is not set.
  pub const fn own_folder_name(self) -> &'static str {
    match self {
      Self::ClaudeCode => ".claude",
      Self::Codex => ".codex",
    }
  }

  /// The arguments of the CLI's own interactive login, which Orchester opens in a terminal the person sees.
  pub const fn login_args(self) -> &'static [&'static str] {
    match self {
      Self::ClaudeCode => &["auth", "login"],
      Self::Codex => &["login"],
    }
  }

  /// The arguments of the status command Orchester reads who is signed in with.
  pub const fn status_args(self) -> &'static [&'static str] {
    match self {
      Self::ClaudeCode => &["auth", "status"],
      Self::Codex => &["login", "status"],
    }
  }

  /// The fixed id of the CLI's Default account, the same in every application database, so a connection that
  /// names no account and its plan readings agree on it.
  pub const fn default_account(self) -> AgentAccountId {
    AgentAccountId::from_uuid(Uuid::from_u128(match self {
      Self::ClaudeCode => 0x0000_0000_0000_7000_8000_0000_0000_0001,
      Self::Codex => 0x0000_0000_0000_7000_8000_0000_0000_0002,
    }))
  }

  /// Whether accounts other than Default work for the CLI on `platform`. On macOS Claude Code keeps its sign-in in
  /// the login Keychain; whether each `CLAUDE_CONFIG_DIR` gets its own item is not verified on a Mac yet
  /// (decision 10), so only Default is used there until it is.
  pub const fn supports_accounts_on(self, platform: Platform) -> bool {
    !matches!((self, platform), (Self::ClaudeCode, Platform::MacOs))
  }
}

/// Where an account's CLI keeps its sign-in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum AccountFolder {
  /// The CLI's own folder (Default): the variable as the environment has it, else `~/.claude` / `~/.codex`.
  Own,
  /// A folder Orchester created in its application data; only such a folder may be moved to the Recycle Bin /
  /// Trash when the account is removed.
  Created(PathBuf),
  /// A folder the person already used.
  Brought(PathBuf),
}

impl AccountFolder {
  /// The folder the variable is set to; `None` for the CLI's own.
  pub fn path(&self) -> Option<&Path> {
    match self {
      Self::Own => None,
      Self::Created(path) | Self::Brought(path) => Some(path),
    }
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AccountError {
  #[error("an account folder must be an absolute path, not {0}")]
  RelativeFolder(PathBuf),
  /// Only Default has the CLI's own folder, and Default is never added.
  #[error("an added account needs a folder of its own")]
  OwnFolder,
  /// Only Default goes by the translated *Default* until it is renamed.
  #[error("an added account needs a name")]
  Unnamed,
}

/// One account of a CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentAccount {
  id: AgentAccountId,
  cli: AccountCli,
  /// `None` for a Default nobody renamed: the window shows the translated *Default*.
  name: Option<Name>,
  folder: AccountFolder,
  added_at: Timestamp,
  last_check: Option<AccountCheck>,
}

impl AgentAccount {
  /// A new account in `folder`, not checked yet.
  pub fn new(
    id: AgentAccountId,
    cli: AccountCli,
    name: Name,
    folder: AccountFolder,
    added_at: Timestamp,
  ) -> Result<Self, AccountError> {
    match folder.path() {
      None => return Err(AccountError::OwnFolder),
      Some(path) if !path.is_absolute() => return Err(AccountError::RelativeFolder(path.to_path_buf())),
      Some(_) => {}
    }
    Ok(Self {
      id,
      cli,
      name: Some(name),
      folder,
      added_at,
      last_check: None,
    })
  }

  /// The CLI's Default account as it is before anything is recorded about it.
  pub const fn default_of(cli: AccountCli) -> Self {
    Self {
      id: cli.default_account(),
      cli,
      name: None,
      folder: AccountFolder::Own,
      added_at: Timestamp::from_unix_millis(0),
      last_check: None,
    }
  }

  /// An account as stored: Default (by its fixed id) with the CLI's own folder, any other with a folder of its own.
  pub fn from_stored(
    id: AgentAccountId,
    cli: AccountCli,
    name: Option<Name>,
    folder: AccountFolder,
    added_at: Timestamp,
    last_check: Option<AccountCheck>,
  ) -> Result<Self, AccountError> {
    let mut account = if id == cli.default_account() {
      Self::default_of(cli)
    } else {
      let name = name.clone().ok_or(AccountError::Unnamed)?;
      Self::new(id, cli, name, folder, added_at)?
    };
    account.name = name;
    account.last_check = last_check;
    Ok(account)
  }

  pub const fn id(&self) -> AgentAccountId {
    self.id
  }

  pub const fn cli(&self) -> AccountCli {
    self.cli
  }

  /// The name the person gave it; `None` for Default under its own name.
  pub const fn name(&self) -> Option<&Name> {
    self.name.as_ref()
  }

  pub const fn folder(&self) -> &AccountFolder {
    &self.folder
  }

  pub const fn added_at(&self) -> Timestamp {
    self.added_at
  }

  pub const fn last_check(&self) -> Option<&AccountCheck> {
    self.last_check.as_ref()
  }

  pub fn is_default(&self) -> bool {
    self.id == self.cli.default_account()
  }

  /// Changes only the name people see: connections hold the account itself.
  pub fn rename(&mut self, name: Name) {
    self.name = Some(name);
  }

  /// Takes in a new check. A sign-out after a sign-in keeps who was signed in (*Sign-in expired · was …*), and a
  /// check that could not ask the CLI keeps the identity known before.
  pub fn record_check(&mut self, check: AccountCheck) {
    let known = self.last_check.as_ref().and_then(AccountCheck::known_identity).cloned();
    self.last_check = Some(check.remembering(known));
  }

  /// The account's state on `platform` (*Checking…* is the window's own, while a check runs).
  pub fn state(&self, platform: Platform) -> AccountState {
    if !self.is_default() && !self.cli.supports_accounts_on(platform) {
      return AccountState::Unsupported;
    }
    self
      .last_check
      .as_ref()
      .map_or(AccountState::NotChecked, AccountCheck::state)
  }

  /// The email the CLI last reported for the account, shown only in the window.
  pub fn email(&self) -> Option<&str> {
    self.last_check.as_ref()?.known_identity()?.email.as_deref()
  }
}

/// The accounts listed with `account` whose CLI reported the same signed-in email: the same account in two folders,
/// allowed, with the plain note that their plan limits are shared (decision 9 still open; the design's default).
pub fn same_sign_in<'a>(account: &AgentAccount, accounts: &'a [AgentAccount]) -> Option<&'a AgentAccount> {
  let email = signed_in_email(account)?;
  accounts
    .iter()
    .filter(|other| other.id != account.id && other.cli == account.cli)
    .find(|other| signed_in_email(other) == Some(email))
}

fn signed_in_email(account: &AgentAccount) -> Option<&str> {
  match &account.last_check.as_ref()?.result {
    AccountCheckResult::SignedIn { identity, .. } => identity.email.as_deref(),
    _ => None,
  }
}
