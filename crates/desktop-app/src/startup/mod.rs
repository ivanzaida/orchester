//! Starting the app: the session of this build's data folder, or the screen that says why there is none
//! (the startup screens: *already open* `N0Lpc`, *can't open its data* `shArO` and their variants
//! `ut8NI`). A startup screen takes the whole window: no tabs, no sidebar, only the version in the status
//! bar, and nothing else loads. It says what failed in plain words, keeps the system's reason, shows the
//! file or folder with Copy, says how to keep working, and offers only actions that exist. Nothing on it
//! changes the data, and nothing retries by itself: the causes do not go away on their own, and
//! Orchester is started again once they are fixed.

mod already_open;
mod failure;
pub mod front;
pub mod launch;
mod parts;
mod position;
mod screen;

use std::path::{Path, PathBuf};

use orchester_store::{StoreError, location::LockHolder};
use orchester_workspace::session::StartupError;

pub use self::{
  launch::Launch,
  position::{WindowPosition, WindowPositionError},
  screen::{StartupScreen, window},
};

/// Why the app cannot open its data, as a startup screen tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupProblem {
  /// Another Orchester app has the data folder open (`N0Lpc`); `holder` is which process, when known.
  AlreadyOpen {
    folder: PathBuf,
    holder: Option<LockHolder>,
  },
  /// The data cannot be opened at all (`shArO` and the failures of `ut8NI`).
  Failed(Failure),
}

/// Why the data cannot be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
  /// The database was written by a newer Orchester (`shArO`).
  NewerData {
    database: PathBuf,
    found: u32,
    supported: u32,
  },
  /// Updating the database to this build would leave `references` broken references (`Y90Edp`).
  UpdateFailed {
    database: PathBuf,
    references: i64,
    found: u32,
    supported: u32,
  },
  /// The database cannot be read (`Tc3hg`). `reason` is the system's words; `damaged` says that the
  /// file is damaged or not a database, rather than busy or refused.
  Unreadable {
    database: PathBuf,
    reason: String,
    damaged: bool,
  },
  /// The data folder cannot be written (`HPRXn`). `reason` is the system's words; `beside_program` says
  /// that the folder is the program's own, as on Windows, rather than one in the user's Library.
  NotWritable {
    folder: PathBuf,
    reason: String,
    beside_program: bool,
  },
  /// The program's own folder cannot be determined, so neither its data nor its runner can be found
  /// (`Us0gQ`).
  ProgramFolderUnknown,
}

impl StartupProblem {
  /// The screen for `error`; `program_folder` is the folder of the running program, when it is known.
  pub fn of(error: &StartupError, program_folder: Option<&Path>) -> Self {
    Self::Failed(match error {
      StartupError::InUse { folder, holder } => {
        return Self::AlreadyOpen {
          folder: folder.clone(),
          holder: *holder,
        };
      }
      StartupError::ExecutableLocation(_) => Failure::ProgramFolderUnknown,
      StartupError::DataFolder { folder, source } => Failure::NotWritable {
        folder: folder.clone(),
        reason: source.to_string(),
        beside_program: program_folder == Some(folder.as_path()),
      },
      StartupError::Database { path, source } => Failure::of_database(path, source),
    })
  }
}

impl StartupProblem {
  /// The name `app_state` reports the screen by.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::AlreadyOpen {
        holder: Some(LockHolder { closing: true, .. }),
        ..
      } => "closing",
      Self::AlreadyOpen { .. } => "already_open",
      Self::Failed(Failure::NewerData { .. }) => "newer_data",
      Self::Failed(Failure::UpdateFailed { .. }) => "update_failed",
      Self::Failed(Failure::Unreadable { .. }) => "unreadable",
      Self::Failed(Failure::NotWritable { .. }) => "not_writable",
      Self::Failed(Failure::ProgramFolderUnknown) => "program_folder_unknown",
    }
  }
}

impl Failure {
  fn of_database(database: &Path, error: &StoreError) -> Self {
    let database = database.to_path_buf();
    match error {
      StoreError::SchemaTooNew { found, supported } => Self::NewerData {
        database,
        found: *found,
        supported: *supported,
      },
      StoreError::MigrationBrokeReferences {
        references,
        found,
        supported,
      } => Self::UpdateFailed {
        database,
        references: *references,
        found: *found,
        supported: *supported,
      },
      error => Self::Unreadable {
        database,
        reason: system_words(error),
        damaged: error.is_damaged(),
      },
    }
  }
}

/// What the system said about `error`, without the store's own words around it.
fn system_words(error: &StoreError) -> String {
  match error {
    StoreError::Sqlite(error) => error.to_string(),
    StoreError::Io { source, .. } => source.to_string(),
    error => error.to_string(),
  }
}

/// What a startup screen shows: the problem, and the app's log file, when it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
  pub problem: StartupProblem,
  pub log: Option<PathBuf>,
}

impl lurq::app::component::DevtoolsInspectable for Problem {}
