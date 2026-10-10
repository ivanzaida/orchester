//! Where a command tool's program is, as `save_tool_draft` finds it: looked up as a step would start
//! it, then kept as a fixed file outside the project's repository, so the program itself is not something
//! a run's agents can rewrite. This is not a sandbox: the program still reads the files the agents before
//! its step wrote, with the person's rights and network access.

use std::{
  ffi::OsStr,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::invocation::Platform,
  resource::{CommandProgram, ProgramLocation},
};

use crate::{app_mcp::error::ToolError, messages::Message, tools::form::program_message};

/// The file `program` is: the first copy in an absolute folder of `path` for a name, an absolute path as
/// it is (on Windows also with a program extension). Refused for a path relative to the worktree, a
/// file inside `repository` (through links as well), and a path a tool cannot store as text.
pub(super) fn found_program(
  program: &CommandProgram,
  path: Option<&OsStr>,
  repository: &Path,
) -> Result<CommandProgram, ToolError> {
  if matches!(program.location(), ProgramLocation::InWorktree(_)) {
    return Err(refused(
      "program_in_worktree",
      Message::new("appMcp.projectTools.errors.programInWorktree"),
    ));
  }
  let not_found = || {
    refused(
      "program_not_found",
      Message::new("appMcp.projectTools.errors.programNotFound").with("program", program.as_str().to_owned()),
    )
  };
  // A name or an absolute path does not depend on the worktree, so none is given.
  let found = program
    .candidates(Path::new(""), path, Platform::current())
    .into_iter()
    .find(|candidate| candidate.is_absolute() && candidate.is_file())
    .ok_or_else(not_found)?;
  if inside(&found, repository).ok_or_else(not_found)? {
    return Err(refused(
      "program_in_repository",
      Message::new("appMcp.projectTools.errors.programInRepository").with("program", found.display().to_string()),
    ));
  }
  let text = found.to_str().ok_or_else(|| {
    refused(
      "program_path_not_text",
      Message::new("appMcp.projectTools.errors.programPathNotText").with("program", found.display().to_string()),
    )
  })?;
  CommandProgram::new(text).map_err(|error| refused("invalid_tool", program_message(&error)))
}

/// Whether `file` lies inside `folder` once links, junctions and other spellings are resolved; `None`
/// when either cannot be resolved.
fn inside(file: &Path, folder: &Path) -> Option<bool> {
  let file: PathBuf = file.canonicalize().ok()?;
  let folder = folder.canonicalize().ok()?;
  Some(file.starts_with(folder))
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
