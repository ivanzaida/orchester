//! The clone in the system's console ([`orchester_exec::clone_console`]): with the first git on the search path that
//! starts directly and answers `git --version`, its files in Orchester's own data folder.

use std::path::{Path, PathBuf};

use orchester_core::command::Command;
use orchester_exec::{
  clone_console::{CloneConsole, CloneJob, ConsoleEnd, ConsoleError},
  launch::{self, Via},
  programs::Programs,
  search_path::{ProgramLookup, SearchPath},
};

use super::{CloneRefusal, Clones, LaunchJob};
use crate::workspace::Workspace;

impl Clones {
  /// Clones in a console window the person sees, git looked up on the search path each time and checked with
  /// `programs`; the console's files go in `workspace`'s scratch folder `clone`.
  pub fn system(workspace: &Workspace, programs: Programs) -> Self {
    let scratch = workspace.scratch("clone");
    Self::with_launcher(move |job| launch(&programs, &scratch, job))
  }
}

/// Opens the console for `job` and waits for its end. Blocking.
fn launch(programs: &Programs, scratch: &Path, job: &LaunchJob) -> Result<ConsoleEnd, CloneRefusal> {
  let git = find_git(&SearchPath::current(), programs)?;
  let job = CloneJob {
    git: &git,
    url: &job.url,
    destination: &job.destination,
    scratch,
    words: &job.words,
  };
  CloneConsole::open(&job).and_then(CloneConsole::wait).map_err(|error| {
    tracing::warn!(%error, "a clone console failed");
    match error {
      ConsoleError::NotOpened(_) => CloneRefusal::NotOpened,
      ConsoleError::NotStarted => CloneRefusal::NotStarted,
      ConsoleError::Unsupported => CloneRefusal::Unsupported,
      ConsoleError::Wait(_) | ConsoleError::UnreadableResult => CloneRefusal::Lost,
    }
  })
}

/// The first `git` on `search` that starts as itself (not a script `cmd.exe` would read again) and answers
/// `git --version` run by `programs`: on macOS the stand-in Apple puts at `/usr/bin/git` before the command line tools
/// are installed doesn't. Blocking.
pub fn find_git(search: &SearchPath, programs: &Programs) -> Result<PathBuf, CloneRefusal> {
  let ProgramLookup::Found(copies) = search.lookup("git") else {
    return Err(CloneRefusal::GitMissing);
  };
  let git = copies
    .into_iter()
    .find(|copy| *launch::resolve(copy).via() == Via::Direct)
    .ok_or(CloneRefusal::GitMissing)?;
  match programs.runner().run(&Command::new(&git, ["--version"])) {
    Ok(output) if output.code == Some(0) => Ok(git),
    Ok(output) => {
      tracing::warn!(code = ?output.code, git = %git.display(), "git does not answer --version");
      Err(CloneRefusal::GitMissing)
    }
    Err(error) => {
      tracing::warn!(%error, git = %git.display(), "git does not start");
      Err(CloneRefusal::GitMissing)
    }
  }
}
