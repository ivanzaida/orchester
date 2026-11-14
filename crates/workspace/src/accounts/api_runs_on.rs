//! A project's connection on one of the person's API accounts (`docs/design/agent-accounts.md`, "Agents run on
//! accounts"): the account is the app's, with its key in the app's keychain entry, and a project that runs a profile
//! on it has a connection that is a copy of it, which Orchester makes and keeps in step. The copy's key is the
//! account's id, and the project holds no secret of it, so a run reads the key from the account's own entry and a
//! key replaced in Settings reaches every project at once.
//!
//! Only an agent CLI's API account has such a connection. An account for Orchester's own agent is refused here.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  connection::{AgentConnection, ApiProvider, ConnectionAccount},
  id::{ApiAccountId, ConnectionId, SecretId},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::runs_on::{RunsOnError, free_name};
use crate::{
  connections::{data::agent_connections, probe},
  tasks::outside::Outside,
  workspace::Workspace,
};

/// The id a connection on the API account `account` keeps as its key's: the account's own.
pub const fn key_of(account: ApiAccountId) -> SecretId {
  SecretId::from_uuid(account.as_uuid())
}

impl Workspace {
  /// The connection of the project at `repo_root` on the API account `account`, made now when it has none: on the
  /// program set for the account's CLI on this computer, else the newest one on the search path, named after the CLI
  /// and the account (*Claude Code · DeepSeek*). Refused for an account of Orchester's own agent, and when a
  /// connection has to be made and the CLI's program is not found.
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
    let ApiSetup::Cli { cli, .. } = account.setup() else {
      return Err(RunsOnError::NotForCli);
    };
    let mut db = ProjectDb::open(repo_root)?;
    let connections = agent_connections(db.connections()?);
    let existing = connections
      .iter()
      .filter(|found| on_account(found, &account))
      .min_by_key(|found| found.created_at());
    if let Some(found) = existing {
      return Ok(found.id());
    }
    let executable = self
      .agent_program(*cli)?
      .or_else(|| installed(*cli))
      .ok_or(RunsOnError::CliMissing(*cli))?;
    let name = free_name((cli.product(), Some(account.name().as_str())), |name| {
      connections.iter().any(|found| found.name() == name)
    });
    let mut connection = AgentConnection::new(
      ConnectionId::generate(),
      name,
      cli.provider(),
      executable,
      Timestamp::now(),
    )?;
    connection.set_api_provider(Some(provider_of(&account)?))?;
    let check = connection
      .executable()
      .and_then(|program| probe::check_on(cli.provider(), program, &ConnectionAccount::Default));
    if let Some(check) = check {
      connection.record_check(check)?;
    }
    db.create_connection(&connection)?;
    self.changes.record(db.project().id(), Outside::Connections);
    Ok(connection.id())
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
      let now = provider_of(account)?;
      if connection.api_provider() == Some(&now) {
        continue;
      }
      connection.set_api_provider(Some(now))?;
      db.save_connection(&connection.into_connection())?;
      changed = true;
    }
    if changed {
      self.changes.record(db.project().id(), Outside::Connections);
    }
    Ok(())
  }
}

/// Whether `connection` is the project's copy of `account`: the account's CLI, with the account's id as its key's.
fn on_account(connection: &AgentConnection, account: &ApiAccount) -> bool {
  connection.provider() == account.agent()
    && connection
      .api_provider()
      .is_some_and(|provider| provider.key() == key_of(account.id()))
}

/// What a connection on `account` runs on: the account's model API and models, under the account's id as the key's.
/// Refused for an account of Orchester's own agent, which no CLI connection is on.
fn provider_of(account: &ApiAccount) -> Result<ApiProvider, RunsOnError> {
  let ApiSetup::Cli { base_url, models, .. } = account.setup() else {
    return Err(RunsOnError::NotForCli);
  };
  Ok(ApiProvider::new(
    base_url.clone(),
    key_of(account.id()),
    models.clone(),
  )?)
}
