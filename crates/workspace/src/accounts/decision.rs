//! The person's decision model servers (`docs/design/agent-accounts.md`, "Agents run on accounts", decision 3;
//! `docs/design/decision-reviewer.md`): a server is added once in Settings, with its key in the app's keychain entry,
//! and every project has a copy of it among its decision connections, by the same id, which Orchester makes and
//! keeps in step. A project chooses its reviewer among them as before. A copy names the server's id as its key's,
//! and the project holds no secret of it, so a review reads the key from the server's own entry.
//!
//! A copy is cheap (no program is run for it), so every server is copied into every project: when the project opens,
//! and when a server is added, changed or checked.

use std::path::Path;

use orchester_core::{
  connection::{DecisionCheck, DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::{ApiAccountId, DecisionConnectionId, SecretId},
  name::Name,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::AccountsError;
use crate::{
  connections::decision::{DecisionError, list_models},
  tasks::outside::Outside,
  workspace::Workspace,
};

/// What a change of a server does with its key.
#[derive(Clone, Copy)]
pub enum ServerKey<'a> {
  /// The key it has, or none, stays.
  Keep,
  /// Another key takes the place of the one it has, or it takes one now.
  Replace(&'a SecretValue),
  /// It takes no key any more.
  Remove,
}

/// The id a decision model server's key goes by: the server's own.
pub const fn decision_key_of(server: DecisionConnectionId) -> SecretId {
  SecretId::from_uuid(server.as_uuid())
}

/// The keychain entry of the server's key: the app's, as an API account's is.
fn entry(server: DecisionConnectionId) -> VaultKey {
  VaultKey::api_account(ApiAccountId::from_uuid(server.as_uuid()))
}

impl Workspace {
  /// The decision model servers, in the order they were added.
  pub fn decision_servers(&self) -> Result<Vec<DecisionConnection>, StoreError> {
    self.db.lock().decision_servers()
  }

  /// Adds a decision model server, with its key when it takes one: the key goes into the keychain first, then the
  /// server into the database; when the database refuses it (a name taken), the key is removed again. Every project
  /// then has it to review with.
  pub fn add_decision_server(
    &self,
    vault: &dyn SecretVault,
    name: Name,
    base_url: DecisionServerUrl,
    key: Option<&SecretValue>,
  ) -> Result<DecisionConnection, AccountsError> {
    let id = DecisionConnectionId::generate();
    let server = DecisionConnection::new(id, name, base_url, key.map(|_| decision_key_of(id)));
    if let Some(key) = key {
      vault.store(&entry(id), key).map_err(AccountsError::Keychain)?;
    }
    if let Err(error) = self.db.lock().add_decision_server(&server, Timestamp::now()) {
      // The key was stored for this server only; nothing names it, so one that stays is never read.
      if key.is_some()
        && let Err(left) = vault.delete(&entry(id))
      {
        tracing::warn!(%left, server = %id, "the key of a decision server that was not added stays in the keychain");
      }
      return Err(error.into());
    }
    self.decision_servers_changed();
    Ok(server)
  }

  /// Renames the server, points it at another base URL, and replaces or drops its key. Another base URL or key drops
  /// the models and the check it had: they came from the server it was.
  pub fn change_decision_server(
    &self,
    vault: &dyn SecretVault,
    id: DecisionConnectionId,
    (name, base_url): (Name, DecisionServerUrl),
    key: ServerKey<'_>,
  ) -> Result<DecisionConnection, AccountsError> {
    let mut server = self.listed_decision_server(id)?;
    server.rename(name);
    match key {
      ServerKey::Keep => server.set_server(base_url, server.key()),
      ServerKey::Replace(value) => {
        vault.store(&entry(id), value).map_err(AccountsError::Keychain)?;
        // The id of the key stays, so the server is taken off its key first: what the old key listed goes with it.
        server.set_server(base_url.clone(), None);
        server.set_server(base_url, Some(decision_key_of(id)));
      }
      ServerKey::Remove => {
        forget_key(vault, id)?;
        server.set_server(base_url, None);
      }
    }
    self.db.lock().save_decision_server(&server)?;
    self.decision_servers_changed();
    Ok(server)
  }

  /// Removes the server and its key. A project that reviews with it keeps its copy, whose check then says its key
  /// is gone; the others lose theirs.
  pub fn remove_decision_server(&self, vault: &dyn SecretVault, id: DecisionConnectionId) -> Result<(), AccountsError> {
    if !self.db.lock().remove_decision_server(id)? {
      return Err(StoreError::DecisionConnectionNotFound(id).into());
    }
    forget_key(vault, id)?;
    for project in self.projects()? {
      let removed = ProjectDb::open(&project.repo_root).and_then(|mut db| {
        let project = db.project().id();
        db.delete_decision_connection(id).map(|()| project)
      });
      match removed {
        Ok(project) => {
          self.changes.record(project, Outside::Connections);
        }
        // The project's reviewer uses it, or it has no copy: nothing to remove there.
        Err(StoreError::DecisionConnectionInUse(_) | StoreError::DecisionConnectionNotFound(_)) => {}
        Err(error) => tracing::debug!(%error, "a project's copy of a removed decision server stays"),
      }
    }
    self.accounts_changed();
    Ok(())
  }

  /// Asks the server for its models (a bodyless `GET {base_url}/models`, with its key when it takes one) and keeps
  /// what it answered: the models, and the check. Nothing of the project is sent.
  pub async fn check_decision_server(
    &self,
    vault: &dyn SecretVault,
    id: DecisionConnectionId,
  ) -> Result<DecisionConnection, DecisionError> {
    let tested = self.listed_decision_server(id).map_err(store_of)?;
    // Keychain diagnostics may hold platform details; a key that cannot be read is a key refused.
    let key = tested.key().map(|_| vault.read(&entry(id)).ok());
    let (result, models) = match key {
      Some(None) => (DecisionCheckResult::KeyRefused, None),
      Some(Some(key)) => list_models(tested.base_url(), Some(&key)).await?,
      None => list_models(tested.base_url(), None).await?,
    };
    let mut checked = self.listed_decision_server(id).map_err(store_of)?;
    if checked.base_url() != tested.base_url() || checked.key() != tested.key() {
      return Err(DecisionError::CheckStale);
    }
    if let Some(models) = models {
      checked.record_models(models);
    }
    checked.record_check(DecisionCheck {
      at: Timestamp::now(),
      result,
    });
    self.db.lock().save_decision_server(&checked)?;
    self.decision_servers_changed();
    Ok(checked)
  }

  /// Gives the project at `repo_root` its copy of every decision model server and brings the copies it has up to
  /// date: the name, the server and whether it takes a key, and the models and check when the app's are newer. A
  /// test's figures are the project's own and stay.
  pub fn follow_decision_servers(&self, repo_root: &Path) -> Result<(), StoreError> {
    let servers = self.decision_servers()?;
    if servers.is_empty() {
      return Ok(());
    }
    let mut db = ProjectDb::open(repo_root)?;
    let mut changed = false;
    for server in &servers {
      let others: Vec<Name> = db
        .decision_connections()?
        .iter()
        .filter(|other| other.id() != server.id())
        .map(|other| other.name().clone())
        .collect();
      let taken = |name: &Name| {
        others
          .iter()
          .any(|other| other.as_str().eq_ignore_ascii_case(name.as_str()))
      };
      match db.decision_connection(server.id())? {
        None => {
          let mut copy = server.clone();
          copy.rename(free_name(server.name(), taken));
          db.create_decision_connection(&copy)?;
          changed = true;
        }
        Some(copy) => {
          if let Some(next) = followed(&copy, server, taken) {
            db.save_decision_connection(&next)?;
            changed = true;
          }
        }
      }
    }
    if changed {
      self.changes.record(db.project().id(), Outside::Connections);
    }
    Ok(())
  }

  /// A server changed: the screens that list them read them again, and every project gets its copy.
  fn decision_servers_changed(&self) {
    self.accounts_changed();
    let projects = match self.projects() {
      Ok(projects) => projects,
      Err(error) => {
        tracing::warn!(%error, "cannot list the projects to give them the decision servers");
        return;
      }
    };
    for project in projects {
      // A project that cannot be opened now gets its copies when it opens.
      if let Err(error) = self.follow_decision_servers(&project.repo_root) {
        tracing::debug!(%error, "a project's decision servers are not brought up to date now");
      }
    }
  }

  fn listed_decision_server(&self, id: DecisionConnectionId) -> Result<DecisionConnection, AccountsError> {
    self
      .db
      .lock()
      .decision_server(id)?
      .ok_or_else(|| StoreError::DecisionConnectionNotFound(id).into())
  }
}

/// What reading the server said, as a check's error: only the database can fail there.
fn store_of(error: AccountsError) -> DecisionError {
  match error {
    AccountsError::Store(error) => DecisionError::Store(error),
    // `listed_decision_server` reads the database and nothing else.
    _ => DecisionError::CheckStale,
  }
}

/// Removes the server's key from the keychain; none there is fine.
fn forget_key(vault: &dyn SecretVault, id: DecisionConnectionId) -> Result<(), AccountsError> {
  match vault.delete(&entry(id)) {
    Ok(()) | Err(VaultError::NotFound) => Ok(()),
    Err(error) => Err(AccountsError::Keychain(error)),
  }
}

/// The project's copy `copy` as `server` is now, or `None` when it already is.
fn followed(
  copy: &DecisionConnection,
  server: &DecisionConnection,
  taken: impl Fn(&Name) -> bool,
) -> Option<DecisionConnection> {
  let mut next = copy.clone();
  if next.name() != server.name() && !taken(server.name()) {
    next.rename(server.name().clone());
  }
  next.set_server(server.base_url().clone(), server.key());
  let newer = match (server.last_check(), next.last_check()) {
    (Some(theirs), Some(ours)) => theirs.at > ours.at,
    (Some(_), None) => true,
    (None, _) => false,
  };
  if let (true, Some(check)) = (newer, server.last_check()) {
    next.record_models(server.models().to_vec());
    next.record_check(check.clone());
  }
  (next != *copy).then_some(next)
}

/// `name`, numbered when a decision connection of the project that is no copy of the server has it.
fn free_name(name: &Name, taken: impl Fn(&Name) -> bool) -> Name {
  if !taken(name) {
    return name.clone();
  }
  (2_u32..)
    .filter_map(|number| Name::new(&format!("{} ({number})", name.as_str())).ok())
    .find(|candidate| !taken(candidate))
    // A name too long to number stays as it is; the database then refuses the copy, and the project keeps its own.
    .unwrap_or_else(|| name.clone())
}
