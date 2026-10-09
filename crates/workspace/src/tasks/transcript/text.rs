//! How a transcript's raw values read on one line: a command whose lines read as one, and a path the agent
//! reported inside its run's worktree, relative to the worktree.

use std::borrow::Cow;

/// A single-line preview of `command`: every run of whitespace that holds a line break, with the indentation
/// after it, reads as one space. Running, details and copying use the command itself.
pub fn command_preview(command: &str) -> Cow<'_, str> {
  if !command.contains(['\r', '\n']) {
    return Cow::Borrowed(command);
  }
  let mut preview = String::with_capacity(command.len());
  let mut rest = command;
  while let Some(start) = rest.find(char::is_whitespace) {
    preview.push_str(&rest[..start]);
    let run = &rest[start..];
    let (space, tail) = run.split_at(run.find(|c: char| !c.is_whitespace()).unwrap_or(run.len()));
    preview.push_str(if space.contains(['\r', '\n']) { " " } else { space });
    rest = tail;
  }
  preview.push_str(rest);
  Cow::Owned(preview)
}

/// A path the CLI reported inside the run's worktree, relative to the worktree
/// (`<repo>/.orchester/worktrees/<run>/src/a.rs` reads `src/a.rs`); `None` for other paths. On Windows the
/// worktree's path mixes separators. The run's worktree is the innermost one: the repository itself may lie
/// in another project's worktree.
pub fn in_worktree(path: &str) -> Option<&str> {
  const WORKTREES: &str = ".orchester/worktrees/";
  // Both separators are one byte: the offsets in the copy are the path's.
  let start = path.replace('\\', "/").rfind(WORKTREES)? + WORKTREES.len();
  let rest = &path[start..];
  rest.find(['\\', '/']).map(|separator| &rest[separator + 1..])
}
