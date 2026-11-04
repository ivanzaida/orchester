//! The computer's resources (`docs/architecture/remote-runners.md`, rule 1): what belongs to the computer a run's
//! executor runs on, which the executor asks for here instead of reading the project database and the home folder
//! itself. It asks for a connection by the id its pinned profile (or the run's choice for a step) names, and gets back
//! what starting the CLI or native agent on this computer needs ([`ComputerConnection`]). The project's variables,
//! which come through the coordinator link, become the environment there ([`ComputerResources::environment`]). Every
//! answer is a serializable value, so a remote implementation can answer for another computer later.

use std::path::{Path, PathBuf};

use orchester_core::{
  computer::{CliHomes, ComputerConnection, ComputerError},
  id::ConnectionId,
  resource::CommandEnvironment,
  store::ConnectionStore,
};
use orchester_store::{ProjectDb, StoreError};

/// The resources of the computer an executor runs on.
pub(crate) trait ComputerResources: Send + Sync {
  /// Connection `id` as this computer has it; `None` when it has none by that id.
  fn connection(&self, id: ConnectionId) -> Result<Option<ComputerConnection>, ComputerError>;

  /// Every connection this computer has, by name.
  fn connections(&self) -> Result<Vec<ComputerConnection>, ComputerError>;

  /// This computer's home folder, which bounds the folders a step may be given; `None` when it cannot be told.
  fn home_folder(&self) -> Option<PathBuf>;

  /// The variables agents and command tools start with on this computer, made of `project`'s, the ones Project
  /// settings › Environment sets: a computer that sets its own value for a name gives that one instead. No secret
  /// value is among them.
  fn environment(&self, project: CommandEnvironment) -> Result<CommandEnvironment, ComputerError>;
}

/// This computer, read where the runner read it before the executor asked: the connections in the project's
/// database, the folders on this disk, the CLI homes this process's environment names and its home folder. The
/// database is opened read-only at each ask and closed after, so this holds nothing open between asks.
pub(crate) struct LocalComputer {
  project_root: PathBuf,
}

impl LocalComputer {
  pub(crate) fn of_project(project_root: &Path) -> Self {
    Self {
      project_root: project_root.to_path_buf(),
    }
  }

  fn read<T>(&self, read: impl FnOnce(&ProjectDb) -> Result<T, StoreError>) -> Result<T, ComputerError> {
    let db = ProjectDb::open_read_only(&self.project_root).map_err(|error| ComputerError::new(&error))?;
    read(&db).map_err(|error| ComputerError::new(&error))
  }
}

impl ComputerResources for LocalComputer {
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
