//! Where a command tool's program is, as `save_tool_draft` finds it: looked up as a step would start
//! it, then kept as a fixed file outside the project's repository, so the program itself is not something
//! a run's agents can rewrite. This is not a sandbox: the program still reads the files the agents before
//! its step wrote, with the person's rights and network access.

use std::{ffi::OsStr, path::Path};

use orchester_core::resource::CommandProgram;

use crate::{
  app_mcp::error::ToolError,
  messages::Message,
  tools::{
    form::program_message,
    program::{ProgramProblem, fixed_program},
  },
};

/// The file `program` is (`tools::program::fixed_program`): the first copy in an absolute folder of `path` for a
/// name, an absolute path as it is (on Windows also with a program extension). Refused for a path relative to the
/// worktree, a file inside `repository` (through links as well), a network or device path, and a path a tool cannot
/// store as text.
pub(super) fn found_program(
  program: &CommandProgram,
  path: Option<&OsStr>,
  repository: &Path,
) -> Result<CommandProgram, ToolError> {
  fixed_program(program, path, repository).map_err(|problem| match problem {
    ProgramProblem::InWorktree => refused(
      "program_in_worktree",
      Message::new("appMcp.projectTools.errors.programInWorktree"),
    ),
    ProgramProblem::NotFound(named) | ProgramProblem::NotLocal(named) => refused(
      "program_not_found",
      Message::new("appMcp.projectTools.errors.programNotFound").with("program", named),
    ),
    ProgramProblem::InRepository(found) => refused(
      "program_in_repository",
      Message::new("appMcp.projectTools.errors.programInRepository").with("program", found.display().to_string()),
    ),
    ProgramProblem::NotText(found) => refused(
      "program_path_not_text",
      Message::new("appMcp.projectTools.errors.programPathNotText").with("program", found.display().to_string()),
    ),
    ProgramProblem::Invalid(error) => refused("invalid_tool", program_message(&error)),
  })
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
