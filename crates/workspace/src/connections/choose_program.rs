//! A person's answer to the start page's warning (`docs/design/startup.md`, decision 9): the file they chose as the
//! CLI's program. It is run once to see that it answers as the CLI, which it may because they picked it; only then is
//! it kept for the first connection and made the program of every connection that needs it, each checked on its own
//! account. Everything here blocks; the UI calls it off the UI thread.

use std::path::{Path, PathBuf};

use orchester_core::{id::ProjectId, store::ConnectionStore};
use orchester_exec::search_path::{ChosenProgramError, chosen_program};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

use super::{
  data::SaveError,
  missing_cli::{MissingCli, NeedingConnection, remembered_key},
  probe,
};
use crate::{
  messages::{self, Message},
  workspace::Workspace,
};

#[derive(Debug, Error)]
pub enum ChooseProgramError {
  #[error(transparent)]
  Refused(#[from] ChosenProgramError),
  #[error("{} did not answer as {program}", file.display())]
  NotTheCli { file: PathBuf, program: &'static str },
  /// Stored paths are text, and this one is not valid Unicode.
  #[error("{} has characters that cannot be stored", .0.display())]
  NotStorable(PathBuf),
  #[error(transparent)]
  Store(#[from] StoreError),
  /// A connection stayed as it was; those of `fixed` have the file.
  #[error("a connection of {project} could not be changed: {source}")]
  NotSaved {
    project: String,
    fixed: Vec<ProjectId>,
    source: Box<SaveError>,
  },
}

impl ChooseProgramError {
  pub fn message(&self) -> Message {
    match self {
      Self::Refused(error) => chosen_message(error),
      Self::NotTheCli { file, program } => Message::new("connections.errors.notTheCli")
        .with("file", file.display().to_string())
        .with("program", (*program).to_owned()),
      Self::NotStorable(file) => {
        Message::new("connections.errors.programNotStorable").with("file", file.display().to_string())
      }
      Self::Store(error) => messages::store_error(error),
      Self::NotSaved { project, .. } => {
        Message::new("connections.errors.notRepointed").with("project", project.clone())
      }
    }
  }
}

/// Why the file a person or a client named cannot be a CLI connection's program.
pub(crate) fn chosen_message(error: &ChosenProgramError) -> Message {
  let named = |key: &'static str, path: &Path| Message::new(key).with("program", path.display().to_string());
  match error {
    ChosenProgramError::NotAbsolute(_) => Message::new("errors.executableNotAbsolute"),
    ChosenProgramError::NotLocal(path) => named("connections.mcp.errors.programNotLocal", path),
    ChosenProgramError::Missing(path) => named("appMcp.errors.programMissing", path),
    ChosenProgramError::NotAFile(path) => named("appMcp.errors.programNotAFile", path),
    ChosenProgramError::NotExecutable(path) => named("appMcp.errors.programNotExecutable", path),
    ChosenProgramError::Unreadable { path, source } => {
      named("appMcp.errors.programUnreadable", path).with("reason", source.to_string())
    }
  }
}

/// Makes `file` the program of `missing`'s CLI: the projects whose connection it fixed, in the order of
/// `missing.needing`. Nothing is written unless it answers as the CLI. A project that fails is logged and the others
/// still get the file; the first failure is reported, and the next read shows what is left.
pub fn choose_program(
  workspace: &Workspace,
  missing: &MissingCli,
  file: &Path,
) -> Result<Vec<ProjectId>, ChooseProgramError> {
  let program = chosen_program(file)?;
  let Some(text) = program.to_str() else {
    return Err(ChooseProgramError::NotStorable(program));
  };
  let answered = probe::check(missing.provider, &program).is_some_and(|check| check.result.is_verified());
  if !answered {
    return Err(ChooseProgramError::NotTheCli {
      file: program,
      program: missing.program,
    });
  }
  workspace.set_ui_state(&remembered_key(missing.program), text)?;
  let mut fixed = Vec::new();
  let mut failed = None;
  for needing in &missing.needing {
    match repoint(needing, &program) {
      Ok(()) => fixed.push(needing.project),
      Err(error) => {
        tracing::warn!(%error, repo_root = %needing.repo_root.display(), "cannot point a connection at the chosen program");
        failed.get_or_insert((needing.name.as_str().to_owned(), error));
      }
    }
  }
  match failed {
    None => Ok(fixed),
    Some((project, source)) => Err(ChooseProgramError::NotSaved {
      project,
      fixed,
      source: Box::new(source),
    }),
  }
}

/// Makes `program` the program of the connection `needing` names and records its check, on the connection's own
/// account.
fn repoint(needing: &NeedingConnection, program: &Path) -> Result<(), SaveError> {
  let mut db = ProjectDb::open(&needing.repo_root)?;
  let mut connection = db
    .connection(needing.connection)?
    .ok_or(StoreError::ConnectionNotFound(needing.connection))?;
  connection.set_executable(program.to_path_buf())?;
  if let Some(check) = probe::recheck(&connection) {
    connection
      .record_check(check)
      .expect("the check was made for the connection's own kind");
  }
  db.save_connection(&connection)?;
  Ok(())
}
