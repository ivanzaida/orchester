//! What an agent profile runs on on this computer (`docs/design/agent-accounts.md`, "Agents run on accounts"): a
//! person chooses one of their accounts for it, and Orchester keeps the rest. A project has a connection for each
//! account its profiles run on, made here when one is first chosen; nobody adds or names one. The choice is kept
//! beside the profile, so changing it needs no new version, in a local project as in a team one.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{AccountCli, AgentAccount},
  connection::{AgentConnection, AgentProvider, ConnectionAccount, ConnectionError},
  id::{AgentAccountId, ConnectionId, ResourceId},
  name::Name,
  resource::AgentProfile,
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ConnectionPick, ProjectDb, StoreError};
use thiserror::Error;

use crate::{
  connections::{data::agent_connections, probe},
  messages::{self, Message},
  tasks::outside::Outside,
  team_fill_ins::FillInError,
  workspace::Workspace,
};

#[derive(Debug, Error)]
pub enum RunsOnError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("agent account {0} is not in the list")]
  AccountNotFound(AgentAccountId),
  /// No program is set for the CLI on this computer and none is on the search path.
  #[error("the program of {} was not found on this computer", .0.product())]
  CliMissing(AccountCli),
  #[error("agent profile {0} is not one of this project's")]
  ProfileNotFound(ResourceId),
  /// The profile's model, settings and permissions were made for another kind of agent than the account's CLI.
  #[error("the account is not one of the profile's CLI")]
  AnotherCli,
  #[error(transparent)]
  Connection(#[from] ConnectionError),
  #[error(transparent)]
  FillIn(#[from] FillInError),
}

impl RunsOnError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::AccountNotFound(_) | Self::Connection(_) => Message::new("errors.storage"),
      Self::CliMissing(_) => Message::new("accounts.errors.cliMissing"),
      Self::ProfileNotFound(_) => Message::new("agents.localConnection.gone"),
      Self::AnotherCli => Message::new("agents.localConnection.notOfKind"),
      Self::FillIn(error) => error.message(),
    }
  }
}

impl Workspace {
  /// Makes the agent profile `profile` of the project at `repo_root` run on `account` on this computer, in place of
  /// what it ran on here before. The profile's version stays as it is. Refused for an account of another CLI than
  /// the profile's, and when the project has no connection on the account yet and the CLI's program is not found.
  pub fn run_profile_on_account(
    &self,
    repo_root: &Path,
    profile: ResourceId,
    account: AgentAccountId,
  ) -> Result<(), RunsOnError> {
    self.run_profile_on_account_with(repo_root, profile, account, |cli| {
      probe::detect_agent(cli.provider()).into_iter().next()
    })
  }

  /// [`Self::run_profile_on_account`] where `installed` says which program of a CLI the search path has. It is
  /// passed in so that no test depends on the machine's `PATH`.
  pub fn run_profile_on_account_with(
    &self,
    repo_root: &Path,
    profile: ResourceId,
    account: AgentAccountId,
    installed: impl FnOnce(AccountCli) -> Option<PathBuf>,
  ) -> Result<(), RunsOnError> {
    let account = self
      .agent_accounts()?
      .into_iter()
      .find(|listed| listed.id() == account)
      .ok_or(RunsOnError::AccountNotFound(account))?;
    let mut db = ProjectDb::open(repo_root)?;
    let team = db.sharing()?.is_some();
    if !team && local_cli(&db, profile)? != Some(account.cli().provider()) {
      return Err(RunsOnError::AnotherCli);
    }
    let connection = self.connection_on(&mut db, &account, installed)?;
    let pick = ConnectionPick {
      connection,
      account: Some(account.id()),
    };
    if team {
      drop(db);
      return Ok(self.pick_team_connection(repo_root, profile, pick)?);
    }
    db.set_connection_pick(profile, pick)?;
    let project = db.project().id();
    self.changes.record(project, Outside::Connections);
    self.changes.record(project, Outside::Profiles);
    Ok(())
  }

  /// The project's connection on `account`, made now when it has none ([`Self::run_profile_on_account`] says how):
  /// a profile's form reads the models the account serves from it before anything runs on it.
  pub fn connect_account(&self, repo_root: &Path, account: AgentAccountId) -> Result<ConnectionId, RunsOnError> {
    self.connect_account_with(repo_root, account, |cli| {
      probe::detect_agent(cli.provider()).into_iter().next()
    })
  }

  /// [`Self::connect_account`] where `installed` says which program of a CLI the search path has.
  pub fn connect_account_with(
    &self,
    repo_root: &Path,
    account: AgentAccountId,
    installed: impl FnOnce(AccountCli) -> Option<PathBuf>,
  ) -> Result<ConnectionId, RunsOnError> {
    let account = self
      .agent_accounts()?
      .into_iter()
      .find(|listed| listed.id() == account)
      .ok_or(RunsOnError::AccountNotFound(account))?;
    let mut db = ProjectDb::open(repo_root)?;
    let had = db.connections()?.len();
    let connection = self.connection_on(&mut db, &account, installed)?;
    if db.connections()?.len() != had {
      self.changes.record(db.project().id(), Outside::Connections);
    }
    Ok(connection)
  }

  /// The connection of the project that runs `account`'s CLI on it, the oldest when it has several; made now when
  /// it has none: on the program set for the CLI on this computer, else the newest one on the search path, named
  /// after the CLI and the account, and checked on the account.
  fn connection_on(
    &self,
    db: &mut ProjectDb,
    account: &AgentAccount,
    installed: impl FnOnce(AccountCli) -> Option<PathBuf>,
  ) -> Result<ConnectionId, RunsOnError> {
    let provider = account.cli().provider();
    let on = ConnectionAccount::of(account);
    let connections = agent_connections(db.connections()?);
    let existing = connections
      .iter()
      .filter(|found| found.provider() == provider && found.api_provider().is_none() && *found.account() == on)
      .min_by_key(|found| found.created_at());
    if let Some(found) = existing {
      return Ok(found.id());
    }
    let executable = self
      .agent_program(account.cli())?
      .or_else(|| installed(account.cli()))
      .ok_or(RunsOnError::CliMissing(account.cli()))?;
    let name = free_name(account, |name| connections.iter().any(|found| found.name() == name));
    let mut connection = AgentConnection::new(ConnectionId::generate(), name, provider, executable, Timestamp::now())?;
    connection.set_account(on)?;
    let check = connection
      .executable()
      .and_then(|program| probe::check_on(provider, program, connection.account()));
    if let Some(check) = check {
      connection.record_check(check)?;
    }
    db.create_connection(&connection)?;
    Ok(connection.id())
  }
}

/// The CLI the local profile `profile` runs with: its connection's, and while that is gone its model's; `None` when
/// neither says. A profile never published is read as its draft.
fn local_cli(db: &ProjectDb, profile: ResourceId) -> Result<Option<AgentProvider>, RunsOnError> {
  let body = match db.latest_version::<AgentProfile>(profile)? {
    Some(version) => version.body().clone(),
    None => db
      .draft::<AgentProfile>(profile)?
      .ok_or(RunsOnError::ProfileNotFound(profile))?
      .body()
      .clone(),
  };
  let named = db
    .connection(body.connection())?
    .and_then(|connection| AgentConnection::try_from(connection).ok());
  Ok(named.map_or_else(
    || AgentProvider::cli_of_model(body.model().as_str()),
    |connection| Some(connection.provider()),
  ))
}

/// A name for the project's connection on `account` that no connection `taken` says it has: the CLI and the account
/// (*Claude Code · Work*), the CLI alone for a Default nobody renamed, numbered when that is taken.
fn free_name(account: &AgentAccount, taken: impl Fn(&Name) -> bool) -> Name {
  let product = account.cli().product();
  let base = account
    .name()
    .map_or_else(|| product.to_owned(), |name| format!("{product} · {}", name.as_str()));
  let mut number = 1_u32;
  loop {
    let candidate = if number == 1 {
      base.clone()
    } else {
      format!("{base} ({number})")
    };
    // An account's name fits a name, so the CLI before it and a number after it can only be too long.
    let name = Name::new(&candidate).unwrap_or_else(|_| numbered(product, number));
    if !taken(&name) {
      return name;
    }
    number += 1;
  }
}

/// `product` with `number`, which is always a name.
fn numbered(product: &str, number: u32) -> Name {
  Name::new(&format!("{product} ({number})")).expect("a product name and a number are a name")
}
