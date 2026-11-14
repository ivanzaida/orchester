//! What a project held before accounts were the app's becomes the app's when the project opens
//! (`docs/design/agent-accounts.md`, "Agents run on accounts", decisions 3 and 6): an agent connection on a model API
//! with a key in a project secret (an agent CLI on an API provider, or Orchester's own agent on a model API) becomes
//! the copy of an API account, and a decision connection the copy of a decision model server.
//!
//! - An API account is made with the id of the project's key secret, so the connection keeps the id it names as its
//!   key's and a session made on it before still continues; its key's value is copied into the app's entry. Where an
//!   API account of the same kind of agent already reaches the same base URL with the same key (compared in memory,
//!   never logged or shown), the connection goes onto that account instead.
//! - A decision connection becomes a server of the app with its own id, so the project's choice of reviewer stays; its
//!   key's value is copied into the server's entry and the connection names the server's id as its key's.
//!
//! Nothing is removed: the project's secret stays where it is, with its value. A run reads the app's key first where
//! there is one by the id, so a key replaced in Settings is the one used. A connection whose key is not on this
//! computer is left as it was.

use std::path::Path;

use orchester_core::{
  account::{ApiAccount, ApiSetup},
  connection::{AgentConnection, ApiEndpoint, ApiProvider, DecisionConnection},
  id::{ApiAccountId, SecretId},
  name::Name,
  secret::{SecretValue, SecretVault, VaultKey},
  store::{ConnectionStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{AccountsError, api_runs_on::key_of, decision::decision_key_of};
use crate::{connections::data::agent_connections, tasks::outside::Outside, workspace::Workspace};

impl Workspace {
  /// Makes what the project at `repo_root` holds of model APIs the app's: its agent connections on a key of its own,
  /// and its decision connections. A connection that cannot be taken (its key is archived or not in this computer's
  /// keychain, a name no account can have) is left as it was and logged; the others are taken all the same.
  pub fn adopt_project_keys(&self, repo_root: &Path, vault: &dyn SecretVault) -> Result<(), AccountsError> {
    let mut db = ProjectDb::open(repo_root)?;
    let mut changed = false;
    for connection in agent_connections(db.connections()?) {
      match self.adopt_connection(&mut db, vault, &connection) {
        Ok(took) => changed |= took,
        Err(error) => tracing::warn!(%error, connection = %connection.id(), "a connection's key stays the project's"),
      }
    }
    for connection in db.decision_connections()? {
      match self.adopt_decision(&mut db, vault, &connection) {
        Ok(took) => changed |= took,
        Err(error) => {
          tracing::warn!(%error, connection = %connection.id(), "a decision connection stays the project's");
        }
      }
    }
    if changed {
      self.changes.record(db.project().id(), Outside::Connections);
      self.accounts_changed();
    }
    Ok(())
  }

  /// Takes `connection`'s model API as an API account, when its key is a secret of the project; whether it did.
  fn adopt_connection(
    &self,
    db: &mut ProjectDb,
    vault: &dyn SecretVault,
    connection: &AgentConnection,
  ) -> Result<bool, AccountsError> {
    let Some((setup, key)) = setup_of(connection) else {
      return Ok(false);
    };
    let accounts = self.api_accounts()?;
    let own = ApiAccountId::from_uuid(key.as_uuid());
    if accounts.iter().any(|account| account.id() == own) {
      return Ok(false);
    }
    let Some(value) = project_key(db, vault, key)? else {
      return Ok(false);
    };
    // The same model API with the same key is one account, whichever project it came from.
    let same = accounts.iter().find(|account| {
      account.agent() == setup.agent()
        && account.setup().base_url() == setup.base_url()
        && vault
          .read(&VaultKey::api_account(account.id()))
          .is_ok_and(|kept| kept.expose() == value.expose())
    });
    if let Some(account) = same {
      let mut moved = connection.clone();
      repoint(&mut moved, key_of(account.id()))?;
      db.save_connection(&moved.into_connection())?;
      return Ok(true);
    }
    let name = self.account_name(&accounts, &setup, (connection.name(), db.project().name()));
    let account = ApiAccount::new(own, name, setup, Timestamp::now());
    vault
      .store(&VaultKey::api_account(own), &value)
      .map_err(AccountsError::Keychain)?;
    self.db.lock().add_api_account(&account)?;
    Ok(true)
  }

  /// A name for the account a connection named `connection` of the project `project` becomes: the connection's, or
  /// with the project's after it where an account of the same kind of agent, or a sign-in of its CLI, has that name.
  fn account_name(&self, accounts: &[ApiAccount], setup: &ApiSetup, (connection, project): (&Name, &Name)) -> Name {
    let sign_ins = self.agent_accounts().unwrap_or_default();
    let taken = |name: &Name| {
      let same = |other: &Name| other.as_str().eq_ignore_ascii_case(name.as_str());
      accounts
        .iter()
        .any(|account| account.agent() == setup.agent() && same(account.name()))
        || sign_ins
          .iter()
          .any(|account| account.cli().provider() == setup.agent() && account.name().is_some_and(same))
    };
    let of_project = Name::new(&format!("{} ({})", connection.as_str(), project.as_str()));
    [Some(connection.clone()), of_project.ok()]
      .into_iter()
      .flatten()
      .chain((2_u32..).filter_map(|number| Name::new(&format!("{} ({number})", connection.as_str())).ok()))
      .find(|name| !taken(name))
      // The numbered names never end, so one is found; the connection's own stands in to keep this total.
      .unwrap_or_else(|| connection.clone())
  }

  /// Takes the decision connection `connection` as a decision model server of the app, by its own id; whether it did.
  fn adopt_decision(
    &self,
    db: &mut ProjectDb,
    vault: &dyn SecretVault,
    connection: &DecisionConnection,
  ) -> Result<bool, AccountsError> {
    let servers = self.decision_servers()?;
    if servers.iter().any(|server| server.id() == connection.id()) {
      return Ok(false);
    }
    let value = match connection.key() {
      Some(key) => match project_key(db, vault, key)? {
        Some(value) => Some(value),
        None => return Ok(false),
      },
      None => None,
    };
    let taken = |name: &Name| {
      servers
        .iter()
        .any(|server| server.name().as_str().eq_ignore_ascii_case(name.as_str()))
    };
    let of_project = Name::new(&format!(
      "{} ({})",
      connection.name().as_str(),
      db.project().name().as_str()
    ));
    let name = [Some(connection.name().clone()), of_project.ok()]
      .into_iter()
      .flatten()
      .find(|name| !taken(name));
    let Some(name) = name else {
      return Ok(false);
    };
    let id = connection.id();
    let entry = VaultKey::api_account(ApiAccountId::from_uuid(id.as_uuid()));
    if let Some(value) = &value {
      vault.store(&entry, value).map_err(AccountsError::Keychain)?;
    }
    // The server as the connection was, under the server's own id as its key's: what its last check listed and its
    // test's figures stay.
    let as_server = |name: Name| {
      let mut server = DecisionConnection::new(
        id,
        name,
        connection.base_url().clone(),
        value.as_ref().map(|_| decision_key_of(id)),
      )
      .with_models(connection.models().to_vec());
      if let Some(check) = connection.last_check() {
        server.record_check(check.clone());
      }
      if let Some(test) = connection.last_test() {
        server.record_test(test.clone());
      }
      server
    };
    self.db.lock().add_decision_server(&as_server(name), Timestamp::now())?;
    db.save_decision_connection(&as_server(connection.name().clone()))?;
    Ok(true)
  }
}

/// What `connection` reaches a model API with, as an account's setup, and the id of its key; `None` for a connection
/// on a sign-in, and for one that lists no model an account of its kind needs.
fn setup_of(connection: &AgentConnection) -> Option<(ApiSetup, SecretId)> {
  if let Some(api) = connection.api() {
    let setup = ApiSetup::native(
      connection.provider(),
      api.preset(),
      api.base_url().clone(),
      api.models().clone(),
      api.provider_fallback(),
    )
    .ok()?;
    return Some((setup, api.key()));
  }
  let provider = connection.api_provider()?;
  let cli = orchester_core::account::AccountCli::of(connection.provider())?;
  let setup = ApiSetup::cli(
    cli,
    provider.base_url().clone(),
    provider.model_ids().cloned().collect(),
  )
  .ok()?;
  Some((setup, provider.key()))
}

/// Puts `connection` on the key `key`: the model API and models it has, under another key's id.
fn repoint(connection: &mut AgentConnection, key: SecretId) -> Result<(), AccountsError> {
  if let Some(api) = connection.api().cloned() {
    let moved = ApiEndpoint::new(api.preset(), api.base_url().clone(), key)
      .with_models(api.models().clone())
      .with_provider_fallback(api.provider_fallback());
    return connection.set_api(moved).map_err(AccountsError::Connection);
  }
  if let Some(provider) = connection.api_provider().cloned() {
    let moved = ApiProvider::new(
      provider.base_url().clone(),
      key,
      provider.model_ids().cloned().collect(),
    )
    .map_err(|_| StoreError::ConnectionNotFound(connection.id()))?;
    connection
      .set_api_provider(Some(moved))
      .map_err(AccountsError::Connection)?;
  }
  Ok(())
}

/// The value of the project's secret `key`, when the project has it, it is not archived and this computer's keychain
/// gives it; `None` otherwise. The value is never logged.
fn project_key(db: &ProjectDb, vault: &dyn SecretVault, key: SecretId) -> Result<Option<SecretValue>, StoreError> {
  let Some(secret) = db.secret(key)?.filter(|secret| !secret.is_archived()) else {
    return Ok(None);
  };
  // `.ok()`: keychain diagnostics may hold platform details, and a key that is not here is simply not taken.
  Ok(vault.read(&secret.vault_key(db.project().id())).ok())
}
