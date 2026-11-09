//! Where a service's program is found (`Note` `KCvD9`, "Found on the PATH"): a name in a folder of the search path
//! the app's programs start with, a path relative to the repository, whose checkout a run's worktree is, or an
//! absolute path. Looked up as a start looks it up ([`CommandProgram::candidates`]); it reads the file system, so the
//! UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  connection::invocation::Platform,
  resource::{CommandProgram, ProgramLocation},
};

/// Where the program is, or that it is not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramFound {
  /// A name found in a folder of the search path.
  OnPath,
  /// A name in no folder of the search path.
  NotOnPath,
  /// A relative path to a file in the repository.
  InRepository,
  /// A relative path to no file in the repository.
  NotInRepository,
  /// An absolute path to a file.
  AtPath,
  /// An absolute path to no file.
  NotAtPath,
}

impl ProgramFound {
  /// Whether a start would find the program.
  pub const fn found(self) -> bool {
    matches!(self, Self::OnPath | Self::InRepository | Self::AtPath)
  }
}

/// Where `program` is for the project at `repo_root`, searching `path` for a name.
pub fn find_program(program: &CommandProgram, repo_root: &Path, path: Option<&std::ffi::OsStr>) -> ProgramFound {
  let found = program
    .candidates(repo_root, path, Platform::current())
    .iter()
    .any(|candidate| candidate.is_file());
  match (program.location(), found) {
    (ProgramLocation::OnPath(_), true) => ProgramFound::OnPath,
    (ProgramLocation::OnPath(_), false) => ProgramFound::NotOnPath,
    (ProgramLocation::InWorktree(_), true) => ProgramFound::InRepository,
    (ProgramLocation::InWorktree(_), false) => ProgramFound::NotInRepository,
    (ProgramLocation::Absolute(_), true) => ProgramFound::AtPath,
    (ProgramLocation::Absolute(_), false) => ProgramFound::NotAtPath,
  }
}

/// [`find_program`] on the search path the app's programs start with.
pub fn find_program_in(program: &CommandProgram, repo_root: &Path) -> ProgramFound {
  let path = orchester_exec::search_path::search_path();
  find_program(program, repo_root, path.as_deref())
}
