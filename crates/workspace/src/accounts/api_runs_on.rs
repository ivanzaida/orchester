//! A project's connection on one of the person's API accounts (`docs/design/agent-accounts.md`, "Agents run on
//! accounts"): the account is the app's, with its key in the app's keychain entry, and a project that runs a profile
//! on it has a connection that is a copy of it, which Orchester makes and keeps in step. The copy's key is the
//! account's id, and the project holds no secret of it, so a run reads the key from the account's own entry and a
//! key replaced in Settings reaches every project at once.
//!
//! An agent CLI's API account is copied as a connection of the CLI on that model API; an account of Orchester's own
//! agent, as a native connection on it.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  connection::{AgentConnection, ApiEndpoint, ApiProvider, ConnectionAccount},
  id::{ApiAccountId, ConnectionId, ResourceId, SecretId},
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ConnectionPick, ProjectDb};

use super::runs_on::{RunsOnError, free_name};
use crate::{
  agents::local_connection::run_here,
  connections::{data::agent_connections, probe},
  tasks::outside::Outside,
  workspace::Workspace,
};

/// The id a connection on the API account `account` keeps as its key's: the account's own.
pub const fn key_of(account: ApiAccountId) -> SecretId {
  SecretId::from_uuid(account.as_uuid())
}

impl Workspace {
  /// The connection of the project at `repo_root` on the API account `account`, made now when it has none. An agent
  /// CLI's is made on the program set for the CLI on this computer, else the newest one on the search path, named
  /// after the CLI and the account (*Claude Code · DeepSeek*), and is refused when that program is not found. One of
  /// Orchester's own agent is named after the account and takes the check the account has.
  pub fn connect_api_account(&self, repo_root: &Path, account: ApiAccountId) -> Result<ConnectionId, RunsOnError> {
    self.connect_api_account_with(repo_root, account, |cli| {
      probe::detect_agent(cli.provider()).into_iter().next()
    })
  }

  /// [`Self::connect_api_account`] where `installed` says which program of a CLI the search path has. It is passed
  /// in so that no test depends on the machine's `PATH`.
  pub fn connect_api_account_with(
    &self,
    repo_root: &Path,
    account: ApiAccountId,
    installed: impl FnOnce(AccountCli) -> Option<PathBuf>,
  ) -> Result<ConnectionId, RunsOnError> {
    let account = self
      .api_account(account)?
      .ok_or(RunsOnError::ApiAccountNotFound(account))?;
    let mut db = ProjectDb::open(repo_root)?;
    let connections = agent_connections(db.connections()?);
    let existing = connections
      .iter()
      .filter(|found| on_account(found, &account))
      .min_by_key(|found| found.created_at());
    if let Some(found) = existing {
      return Ok(found.id());
    }
    let taken = |name: &Name| connections.iter().any(|found| found.name() == name);
    let connection = match account.setup() {
      ApiSetup::Cli { cli, .. } => self.cli_copy(&account, *cli, installed, taken)?,
      ApiSetup::Native { .. } => native_copy(&account, taken)?,
    };
    db.create_connection(&connection)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(connection.id())
  }

  /// Makes the agent profile `profile` of the project at `repo_root` run on the API account `account` on this
  /// computer, in place of what it ran on here before: the project's connection on the account is made when it has
  /// none, and the profile's version stays as it is. Refused for an account of another kind of agent than the
  /// profile's, and for one that does not serve the profile's model.
  pub fn run_profile_on_api_account(
    &self,
    repo_root: &Path,
    profile: ResourceId,
    account: ApiAccountId,
  ) -> Result<(), RunsOnError> {
    let connection = self.connect_api_account(repo_root, account)?;
    let db = ProjectDb::open(repo_root)?;
    let project = db.project().id();
    let team = db.sharing()?.is_some();
    drop(db);
    if team {
      let pick = ConnectionPick {
        connection,
        account: None,
      };
      return Ok(self.pick_team_connection(repo_root, profile, pick)?);
    }
    run_here(repo_root, profile, connection).map_err(RunsOnError::Refused)?;
    self.changes.record(project, Outside::Connections);
    self.changes.record(project, Outside::Profiles);
    Ok(())
  }

  /// A connection of `cli` on its API account `account`, on the program set for the CLI here, else the one
  /// `installed` finds, checked there.
  fn cli_copy(
    &self,
    account: &ApiAccount,
    cli: AccountCli,
    installed: impl FnOnce(AccountCli) -> Option<PathBuf>,
    taken: impl Fn(&Name) -> bool,
  ) -> Result<AgentConnection, RunsOnError> {
    let executable = self
      .agent_program(cli)?
      .or_else(|| installed(cli))
      .ok_or(RunsOnError::CliMissing(cli))?;
    let name = free_name((cli.product(), Some(account.name().as_str())), taken);
    let mut connection = AgentConnection::new(
      ConnectionId::generate(),
      name,
      cli.provider(),
      executable,
      Timestamp::now(),
    )?;
    connection.set_api_provider(provider_of(account))?;
    let check = connection
      .executable()
      .and_then(|program| probe::check_on(cli.provider(), program, &ConnectionAccount::Default));
    if let Some(check) = check {
      connection.record_check(check)?;
    }
    Ok(connection)
  }

  /// Brings the connections of the project at `repo_root` that are on an API account up to date with it: the model
  /// API and the models it serves. Called when a project opens, for what changed in Settings meanwhile.
  pub fn follow_api_accounts(&self, repo_root: &Path) -> Result<(), RunsOnError> {
    let accounts = self.api_accounts()?;
    let mut db = ProjectDb::open(repo_root)?;
    let mut changed = false;
    for mut connection in agent_connections(db.connections()?) {
      let Some(account) = accounts.iter().find(|account| on_account(&connection, account)) else {
        continue;
      };
      if !follow(&mut connection, account)? {
        continue;
      }
      db.save_connection(&connection.into_connection())?;
      changed = true;
    }
    if changed {
      self.changes.record(db.project().id(), Outside::Connections);
    }
    Ok(())
  }
}

/// Whether `connection` is the project's copy of `account`: the account's kind of agent, with the account's id as
/// its key's.
pub(super) fn on_account(connection: &AgentConnection, account: &ApiAccount) -> bool {
  connection.provider() == account.agent() && connection.key() == Some(key_of(account.id()))
}

/// Makes `connection`, the project's copy of `account`, say what the account does now; whether anything changed.
fn follow(connection: &mut AgentConnection, account: &ApiAccount) -> Result<bool, RunsOnError> {
  if let Some(now) = provider_of(account) {
    if connection.api_provider() == Some(&now) {
      return Ok(false);
    }
    connection.set_api_provider(Some(now))?;
    return Ok(true);
  }
  match endpoint_of(account) {
    Some(now) if connection.api() != Some(&now) => {
      connection.set_api(now)?;
      Ok(true)
    }
    _ => Ok(false),
  }
}

/// A native connection on `account`, an API account of Orchester's own agent: named after the account, on its model
/// API, with the check the account has.
fn native_copy(account: &ApiAccount, taken: impl Fn(&Name) -> bool) -> Result<AgentConnection, RunsOnError> {
  let api = endpoint_of(account).ok_or(RunsOnError::ApiAccountNotFound(account.id()))?;
  let name = free_name((account.name().as_str(), None), taken);
  let mut connection = AgentConnection::native(ConnectionId::generate(), name, account.agent(), api, Timestamp::now())?;
  if let Some(check) = account.last_check() {
    connection.record_check(check.clone())?;
  }
  Ok(connection)
}

/// What a CLI's connection on `account` runs on: the account's model API and models, under the account's id as the
/// key's; `None` for an account of Orchester's own agent.
fn provider_of(account: &ApiAccount) -> Option<ApiProvider> {
  let ApiSetup::Cli { base_url, models, .. } = account.setup() else {
    return None;
  };
  // An agent CLI's API account lists at least one model (`ApiSetup::cli`), which is all a provider asks.
  ApiProvider::new(base_url.clone(), key_of(account.id()), models.clone()).ok()
}

/// The model API a native connection on `account` talks to, under the account's id as the key's; `None` for an agent
/// CLI's account.
fn endpoint_of(account: &ApiAccount) -> Option<ApiEndpoint> {
  let ApiSetup::Native {
    preset,
    base_url,
    models,
    provider_fallback,
    ..
  } = account.setup()
  else {
    return None;
  };
  Some(
    ApiEndpoint::new(*preset, base_url.clone(), key_of(account.id()))
      .with_models(models.clone())
      .with_provider_fallback(*provider_fallback),
  )
}
