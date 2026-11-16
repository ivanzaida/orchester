//! How a test suite's program starts for `run_tests` (`docs/architecture/agent-testing.md`, "Test suite"): without a
//! shell, in its working folder inside the run's worktree, with the filter in its arguments, the cleaned environment
//! every program Orchester starts gets, the project's variables and the suite's own on top. A variable that names a
//! secret stays a name here: the runner reads its value from the keychain and sets it last.

use std::{
  ffi::{OsStr, OsString},
  path::{Path, PathBuf},
};

use super::{SuitePath, SuiteVariable, TestFilter, TestSuite};
use crate::{
  command::Command,
  connection::invocation::{Platform, agent_environment},
  resource::{CommandEnvironment, NO_CURRENT_DIRECTORY_IN_EXE_PATH, VariableName},
  secret::SecretName,
};

/// A suite's process, ready to start once its secrets are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiteProcess {
  /// The program with its arguments, working folder and plain variables.
  pub command: Command,
  /// The variables whose value is the secret of that name, in the suite's order.
  pub secrets: Vec<(VariableName, SecretName)>,
}

impl TestSuite {
  /// The folder the program starts in: its working folder inside `worktree`, or the worktree.
  pub fn folder_in(&self, worktree: &Path) -> PathBuf {
    self
      .working_folder
      .as_ref()
      .map_or_else(|| worktree.to_path_buf(), |folder| within(worktree, folder))
  }

  /// The file the program writes its report to, inside [`Self::folder_in`] `worktree`; `None` for a printed report.
  pub fn report_file_in(&self, worktree: &Path) -> Option<PathBuf> {
    self.report.path().map(|path| within(&self.folder_in(worktree), path))
  }

  /// The `PATH` the program is looked up in and starts with: the suite's own plain `PATH` when it sets one, else the
  /// one the project adds (`project`), else `inherited`.
  pub fn search_path(&self, project: &CommandEnvironment, inherited: Option<OsString>) -> Option<OsString> {
    let own = self.variables.iter().find_map(|(name, value)| match value {
      SuiteVariable::Plain(text) if name.as_str().eq_ignore_ascii_case("PATH") => Some(text.as_str()),
      SuiteVariable::Plain(_) | SuiteVariable::Secret(_) => None,
    });
    own.or_else(|| project.get("PATH")).map(OsString::from).or(inherited)
  }

  /// Files the program may be on `platform` ([`crate::resource::CommandProgram::candidates`]), looked up in `path`.
  pub fn program_candidates(&self, worktree: &Path, path: Option<&OsStr>, platform: Platform) -> Vec<PathBuf> {
    self.command.program().candidates(worktree, path, platform)
  }

  /// The process that runs `program` (a file [`Self::program_candidates`] found) with `filter` in its arguments
  /// ([`super::SuiteCommand::arguments`]), in [`Self::folder_in`] `worktree`, with the allowed part of `environment`,
  /// then the variables the project adds (`project`), then the suite's plain ones; each replaces one of the same name.
  /// On Windows the process is also told not to start programs from its current directory by name, as a command tool
  /// is ([`NO_CURRENT_DIRECTORY_IN_EXE_PATH`]).
  pub fn process(
    &self,
    (program, worktree): (&Path, &Path),
    (environment, project): (impl IntoIterator<Item = (OsString, OsString)>, &CommandEnvironment),
    platform: Platform,
    filter: Option<&TestFilter>,
  ) -> SuiteProcess {
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    let mut secrets = Vec::new();
    for (name, value) in self.variables.iter() {
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name.as_str()));
      match value {
        SuiteVariable::Plain(text) => variables.push((name.as_str().to_owned(), text.clone())),
        SuiteVariable::Secret(secret) => secrets.push((name.clone(), secret.clone())),
      }
    }
    if platform == Platform::Windows {
      let (name, value) = NO_CURRENT_DIRECTORY_IN_EXE_PATH;
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
      variables.push((name.to_owned(), value.to_owned()));
    }
    let command = Command::new(program, self.command.arguments(filter))
      .with_current_dir(&self.folder_in(worktree))
      .with_environment(variables);
    SuiteProcess { command, secrets }
  }
}

/// `path`, which has `/` between its parts, inside `folder`.
fn within(folder: &Path, path: &SuitePath) -> PathBuf {
  path
    .as_str()
    .split('/')
    .fold(folder.to_path_buf(), |joined, part| joined.join(part))
}
