//! How a command tool's program starts: which files the program may be, and the process that runs
//! it — its arguments as they are, the step's worktree as working directory, the cleaned environment
//! every program Orchester starts gets plus the tool's own variables, and no standard input. Which
//! candidate exists is decided by the caller, which owns the file system; how the file starts
//! (Windows command scripts included) is decided by the process runner.

use std::{
  ffi::{OsStr, OsString},
  path::{Path, PathBuf},
};

use super::command::{CommandAction, CommandArg, ProgramLocation};
use crate::{
  command::Command,
  connection::invocation::{Platform, agent_environment},
};

/// Extensions Windows starts a program file by, in the order they are tried after a bare name.
const WINDOWS_EXTENSIONS: [&str; 4] = ["exe", "cmd", "bat", "com"];

impl CommandAction {
  /// Files the program may be on `platform`, in search order: a bare name in each folder of
  /// `path` (the `PATH` the program starts with), a relative path in `worktree`, an absolute path
  /// as it is. On Windows a name without one of the program extensions is also tried with each.
  pub fn program_candidates(&self, worktree: &Path, path: Option<&OsStr>, platform: Platform) -> Vec<PathBuf> {
    let bases: Vec<PathBuf> = match self.program().location() {
      ProgramLocation::OnPath(name) => path
        .map(|path| {
          std::env::split_paths(path)
            .filter(|folder| !folder.as_os_str().is_empty())
            .map(|folder| folder.join(name))
            .collect()
        })
        .unwrap_or_default(),
      ProgramLocation::InWorktree(relative) => vec![worktree.join(relative)],
      ProgramLocation::Absolute(absolute) => vec![absolute.to_path_buf()],
    };
    bases
      .into_iter()
      .flat_map(|base| with_extensions(base, platform))
      .collect()
  }

  /// The `PATH` the program is looked up in and starts with: the tool's own `PATH` variable when it
  /// sets one, else `inherited`.
  pub fn search_path(&self, inherited: Option<OsString>) -> Option<OsString> {
    self
      .environment()
      .iter()
      .find(|(name, _)| name.as_str().eq_ignore_ascii_case("PATH"))
      .map(|(_, value)| OsString::from(value))
      .or(inherited)
  }

  /// The process that runs `program` (a file [`Self::program_candidates`] found) in `worktree`,
  /// with the allowed part of `environment` and the tool's variables, which replace ones of the
  /// same name.
  pub fn process(
    &self,
    program: &Path,
    worktree: &Path,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
  ) -> Command {
    let mut variables = agent_environment(environment);
    for (name, value) in self.environment().iter() {
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name.as_str()));
      variables.push((name.as_str().to_owned(), value.to_owned()));
    }
    Command::new(program, self.args().iter().map(CommandArg::as_str))
      .with_current_dir(worktree)
      .with_environment(variables)
  }
}

fn with_extensions(base: PathBuf, platform: Platform) -> Vec<PathBuf> {
  if platform == Platform::MacOs {
    return vec![base];
  }
  let has_extension = base.extension().and_then(OsStr::to_str).is_some_and(|extension| {
    WINDOWS_EXTENSIONS
      .iter()
      .any(|known| known.eq_ignore_ascii_case(extension))
  });
  if has_extension {
    return vec![base];
  }
  WINDOWS_EXTENSIONS
    .iter()
    .map(|extension| {
      let mut name = base.clone().into_os_string();
      name.push(".");
      name.push(extension);
      PathBuf::from(name)
    })
    .collect()
}
