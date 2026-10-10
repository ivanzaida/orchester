//! How a command tool's program starts: which files the program may be, and the process that runs
//! it — its arguments as they are, the step's worktree as working directory, the cleaned environment
//! every program Orchester starts gets plus the project's variables and the tool's own, and no
//! standard input. Which
//! candidate exists is decided by the caller, which owns the file system; how the file starts
//! (Windows command scripts included) is decided by the process runner.

use std::{
  ffi::{OsStr, OsString},
  path::{Path, PathBuf},
};

use super::command::{CommandAction, CommandArg, CommandEnvironment, CommandProgram, ProgramLocation};
use crate::{
  command::Command,
  connection::invocation::{Platform, agent_environment},
  permission::{ProgramPlace, program_place, stays_local},
};

/// The variable that keeps Windows from looking up a program named without a path in the current
/// directory first (`NeedCurrentDirectoryForExePathW`); `cmd.exe` respects it as well. MCP servers are started with
/// it too.
pub const NO_CURRENT_DIRECTORY_IN_EXE_PATH: (&str, &str) = ("NoDefaultCurrentDirectoryInExePath", "1");

/// Extensions Windows starts a program file by, in the order they are tried after a bare name.
const WINDOWS_EXTENSIONS: [&str; 4] = ["exe", "cmd", "bat", "com"];

impl CommandProgram {
  /// Files the program may be on `platform`, in search order: a bare name in each absolute folder of
  /// `path` (the `PATH` the program starts with), a relative path in `worktree`, an absolute path
  /// as it is. On Windows a name without one of the program extensions is also tried with each.
  /// Command tools and MCP servers look their programs up this way. A folder of the `PATH`, or an absolute path, that
  /// names another machine or a device, or leads there through a link, is passed over unread
  /// ([`stays_local`]).
  pub fn candidates(&self, worktree: &Path, path: Option<&OsStr>, platform: Platform) -> Vec<PathBuf> {
    let bases: Vec<PathBuf> = match self.location() {
      ProgramLocation::OnPath(name) => path
        .map(|path| {
          std::env::split_paths(path)
            // A relative folder would be looked in from the worktree, which agents write.
            .filter(|folder| folder.is_absolute() && stays_local(folder).is_ok())
            .map(|folder| folder.join(name))
            .collect()
        })
        .unwrap_or_default(),
      ProgramLocation::InWorktree(relative) => vec![worktree.join(relative)],
      ProgramLocation::Absolute(absolute) if stays_local(absolute).is_ok() => vec![absolute.to_path_buf()],
      ProgramLocation::Absolute(_) => Vec::new(),
    };
    bases
      .into_iter()
      .flat_map(|base| with_extensions(base, platform))
      .collect()
  }

  /// Checks, just before it starts, where the file `found` of a program stored as a fixed file (an absolute path)
  /// leads: still on this machine and outside `repository`, which holds the worktrees agents write, as when it was
  /// saved. A name or a path in the worktree is looked up each time and not checked here.
  ///
  /// # Errors
  ///
  /// A [`ProgramPlace`] says where it leads now.
  pub fn check_place(&self, found: &Path, repository: &Path) -> Result<(), ProgramPlace> {
    match self.location() {
      ProgramLocation::Absolute(_) => program_place(found, repository),
      ProgramLocation::OnPath(_) | ProgramLocation::InWorktree(_) => Ok(()),
    }
  }
}

impl CommandAction {
  /// Files the program may be on `platform`; see [`CommandProgram::candidates`].
  pub fn program_candidates(&self, worktree: &Path, path: Option<&OsStr>, platform: Platform) -> Vec<PathBuf> {
    self.program().candidates(worktree, path, platform)
  }

  /// The `PATH` the program is looked up in and starts with: the tool's own `PATH` variable when it
  /// sets one, else the one the project adds (`project`), else `inherited`.
  pub fn search_path(&self, project: &CommandEnvironment, inherited: Option<OsString>) -> Option<OsString> {
    self
      .environment()
      .get("PATH")
      .or_else(|| project.get("PATH"))
      .map(OsString::from)
      .or(inherited)
  }

  /// The process that runs `program` (a file [`Self::program_candidates`] found) in `worktree`,
  /// with the allowed part of `environment`, then the variables the project adds (`project`), then
  /// the tool's; each replaces one of the same name. On Windows the process is also told not to start
  /// programs from its current directory, the worktree, by name ([`NO_CURRENT_DIRECTORY_IN_EXE_PATH`]):
  /// a `.cmd` shim that starts `node` would otherwise start a `node.exe` an agent left there.
  pub fn process(
    &self,
    program: &Path,
    worktree: &Path,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
    platform: Platform,
  ) -> Command {
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    self.environment().add_to(&mut variables);
    if platform == Platform::Windows {
      let (name, value) = NO_CURRENT_DIRECTORY_IN_EXE_PATH;
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
      variables.push((name.to_owned(), value.to_owned()));
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
