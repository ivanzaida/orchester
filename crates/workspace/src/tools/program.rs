//! Where a program the project stores is: a command tool's (`save_tool_draft`) or an MCP server's (Add / Edit MCP
//! server, `add_mcp_server`). It is looked up as a step or an agent's CLI would start it, then kept as a fixed file
//! outside the project's repository, so a run's agents cannot rewrite it, and no later change of the `PATH` or of a
//! link points it elsewhere. A path that names another machine or a device, or a link that leads to one, is refused
//! by its text before anything reads it (`orchester_core::permission::stays_local`). This is not a sandbox: the
//! program still reads the files the agents wrote, with the person's rights and network access.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::invocation::Platform,
  permission::{resolve_local, stays_local},
  resource::{CommandError, CommandProgram, ProgramLocation},
};

/// Why a program cannot be stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramProblem {
  /// A path relative to the worktree, which agents write.
  InWorktree,
  /// Neither on the search path nor at that path.
  NotFound(String),
  /// It lies inside the project's repository (through links as well).
  InRepository(PathBuf),
  /// Found at a path that is not Unicode text.
  NotText(PathBuf),
  /// A network or device path, or a link that leads to one.
  NotLocal(String),
  /// The path found is not a program a project can store.
  Invalid(CommandError),
}

/// The file `program` is: the first copy in an absolute folder of `path` for a name, an absolute path as it is (on
/// Windows also with a program extension), resolved and written as a person writes it.
///
/// # Errors
///
/// A [`ProgramProblem`] says why the program cannot be stored.
pub fn fixed_program(
  program: &CommandProgram,
  path: Option<&OsStr>,
  repository: &Path,
) -> Result<CommandProgram, ProgramProblem> {
  if matches!(program.location(), ProgramLocation::InWorktree(_)) {
    return Err(ProgramProblem::InWorktree);
  }
  let named = program.as_str().to_owned();
  if let ProgramLocation::Absolute(absolute) = program.location()
    && stays_local(absolute).is_err()
  {
    return Err(ProgramProblem::NotLocal(named));
  }
  // A name or an absolute path does not depend on the worktree, so none is given. A folder of the search path that
  // is not local is passed over unread.
  let found = program
    .candidates(Path::new(""), path, Platform::current())
    .into_iter()
    .filter(|candidate| candidate.is_absolute() && stays_local(candidate).is_ok())
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| ProgramProblem::NotFound(named.clone()))?;
  let resolved = resolve_local(&found).map_err(|_| ProgramProblem::NotLocal(named.clone()))?;
  let repository = resolve_local(repository).map_err(|_| ProgramProblem::NotFound(named))?;
  if resolved.starts_with(&repository) {
    return Err(ProgramProblem::InRepository(resolved));
  }
  let text = resolved
    .to_str()
    .ok_or_else(|| ProgramProblem::NotText(resolved.clone()))?;
  CommandProgram::new(text).map_err(ProgramProblem::Invalid)
}
