//! The computer's resources (`docs/architecture/remote-runners.md`, rule 1): what belongs to the computer a run's
//! executor runs on, which the executor asks for here instead of reading the project database, the keychain and the
//! home folder itself. It asks for a connection by the id its pinned profile (or the run's choice for a step) names,
//! and gets back what starting the CLI or native agent on this computer needs ([`ComputerConnection`]). The project's
//! variables, which come through the coordinator link, become the environment there
//! ([`ComputerResources::environment`]). Every such answer is a serializable value, so a remote implementation can
//! answer for another computer later.
//!
//! Secrets are this computer's too: the metadata of the project's secrets and MCP servers, and their values in this
//! computer's keychain (MCP servers' secrets, API keys, the screen that keeps values out of memory). A value is
//! answered only on the computer whose keychain holds it and is never sent: on another computer the executor reads
//! that computer's own keychain, so no value is part of any message to the coordinator. The coordinator's own uses
//! (the Orchestrator chat, task estimates, the connection checks the desktop asks for) read through the same interface.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  computer::{CliHomes, ComputerConnection, ComputerError},
  connection::{DecisionConnection, mcp::McpServer},
  id::{ConnectionId, DecisionConnectionId, ProjectId, SecretId},
  memory::{ScreenUnavailable, SecretScreen},
  resource::CommandEnvironment,
  secret::{Secret, SecretValue, SecretVault, VaultError},
  store::{ConnectionStore, McpServerStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};

/// The resources of the computer an executor runs on.
pub(crate) trait ComputerResources: Send + Sync {
  /// Connection `id` as this computer has it; `None` when it has none by that id.
  fn connection(&self, id: ConnectionId) -> Result<Option<ComputerConnection>, ComputerError>;

  /// Every connection this computer has, by name.
  fn connections(&self) -> Result<Vec<ComputerConnection>, ComputerError>;

  /// Decision model connection `id` as this computer has it; `None` when it has none by that id.
  fn decision_connection(&self, id: DecisionConnectionId) -> Result<Option<DecisionConnection>, ComputerError>;

  /// This computer's home folder, which bounds the folders a step may be given; `None` when it cannot be told.
  fn home_folder(&self) -> Option<PathBuf>;

  /// MCP server `id` as this computer has it; `None` when it has none by that id.
  fn mcp_server(&self, id: ConnectionId) -> Result<Option<McpServer>, ComputerError>;

  /// Every MCP server this computer has and can read, by name.
  fn mcp_servers(&self) -> Result<Vec<McpServer>, ComputerError>;

  /// The metadata of secret `id`, archived or not; `None` when there is none by that id.
  fn secret(&self, id: SecretId) -> Result<Option<Secret>, ComputerError>;

  /// The metadata of every secret, archived ones too.
  fn secrets(&self) -> Result<Vec<Secret>, ComputerError>;

  /// The value of `secret` of `project` in this computer's keychain; [`VaultError::NotFound`] when it holds none.
  fn secret_value(&self, project: ProjectId, secret: &Secret) -> Result<SecretValue, VaultError>;

  /// The values of `secrets` of `project` in this computer's keychain, to keep them out of memory records, as
  /// [`SecretScreen::load`] reads them.
  fn secret_screen(&self, project: ProjectId, secrets: &[Secret]) -> Result<SecretScreen, ScreenUnavailable>;

  /// The variables agents and command tools start with on this computer, made of `project`'s, the ones Project
  /// settings › Environment sets: a computer that sets its own value for a name gives that one instead. No secret
  /// value is among them.
  fn environment(&self, project: CommandEnvironment) -> Result<CommandEnvironment, ComputerError>;
}

/// This computer, read where the runner read it before the executor asked: the connections, secrets and MCP servers in
/// the project's database, the folders on this disk, the CLI homes this process's environment names, its home folder
/// and the keychain the runner was given. The database is opened read-only at each ask and closed after, so this holds
/// nothing open between asks.
pub(crate) struct LocalComputer {
  project_root: PathBuf,
  /// Where the secrets' values are read: the OS keychain, or one in memory in a test.
  vault: Arc<dyn SecretVault>,
}

impl LocalComputer {
  pub(crate) fn of_project(project_root: &Path, vault: Arc<dyn SecretVault>) -> Self {
    Self {
      project_root: project_root.to_path_buf(),
      vault,
    }
  }

  fn read<T>(&self, read: impl FnOnce(&ProjectDb) -> Result<T, StoreError>) -> Result<T, ComputerError> {
    let db = ProjectDb::open_read_only(&self.project_root).map_err(|error| ComputerError::new(&error))?;
    read(&db).map_err(|error| ComputerError::new(&error))
  }
}

impl ComputerResources for LocalComputer {
  fn decision_connection(&self, id: DecisionConnectionId) -> Result<Option<DecisionConnection>, ComputerError> {
    self.read(|db| db.decision_connection(id))
  }

  fn connection(&self, id: ConnectionId) -> Result<Option<ComputerConnection>, ComputerError> {
    let connection = self.read(|db| db.connection(id))?;
    let homes = homes();
    Ok(connection.map(|connection| ComputerConnection::read(connection, Path::is_dir, &homes)))
  }

  fn connections(&self) -> Result<Vec<ComputerConnection>, ComputerError> {
    let connections = self.read(ConnectionStore::connections)?;
    let homes = homes();
    Ok(
      connections
        .into_iter()
        .map(|connection| ComputerConnection::read(connection, Path::is_dir, &homes))
        .collect(),
    )
  }

  fn home_folder(&self) -> Option<PathBuf> {
    std::env::home_dir()
  }

  fn mcp_server(&self, id: ConnectionId) -> Result<Option<McpServer>, ComputerError> {
    self.read(|db| db.mcp_server(id))
  }

  fn mcp_servers(&self) -> Result<Vec<McpServer>, ComputerError> {
    self.read(McpServerStore::mcp_servers)
  }

  fn secret(&self, id: SecretId) -> Result<Option<Secret>, ComputerError> {
    self.read(|db| db.secret(id))
  }

  fn secrets(&self) -> Result<Vec<Secret>, ComputerError> {
    self.read(SecretStore::secrets)
  }

  fn secret_value(&self, project: ProjectId, secret: &Secret) -> Result<SecretValue, VaultError> {
    self.vault.read(&secret.vault_key(project))
  }

  fn secret_screen(&self, project: ProjectId, secrets: &[Secret]) -> Result<SecretScreen, ScreenUnavailable> {
    SecretScreen::load(project, secrets, self.vault.as_ref())
  }

  /// This computer sets no values of its own: the project's variables, exactly.
  fn environment(&self, project: CommandEnvironment) -> Result<CommandEnvironment, ComputerError> {
    Ok(project)
  }
}

/// The CLI homes of this process's environment and home folder.
fn homes() -> CliHomes {
  CliHomes::new(
    |variable| std::env::var_os(variable).map(PathBuf::from),
    std::env::home_dir(),
  )
}
