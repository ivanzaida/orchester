//! Check on the merged result (ORC-684): the command tools of a run's checking tool steps, as the run pinned them, run
//! again in the run's worktree once it holds the current base branch, in pipeline order, each as its step ran it
//! (found on the `PATH` it starts with or in the worktree, with the project's variables and the tool's, stopped after
//! the tool's timeout). The desktop names the tools ([`RecheckTool`]); the runner, which reaches the worktree, runs
//! them and answers how it went ([`Rechecked`]). The first that does not end as the tool expects fails the check.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
  command::Command,
  connection::{
    invocation::Platform,
    stream::{OutputStream, ToolCommandResult},
  },
  resource::{CommandAction, CommandEnvironment},
  run::{CommandEnd, ToolCommand},
};

/// Lines of a failed check's output its reason keeps.
const KEPT_LINES: usize = 12;

/// A command tool Check runs again on the merged result: its name and its action as the run pinned it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecheckTool {
  pub name: String,
  pub action: CommandAction,
}

/// How Check went on the merged result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Rechecked {
  /// Every command tool passed; their names, joined.
  Passed { tools: String },
  /// The pipeline has no command tool step to run again.
  NothingToRun,
  /// `tool` did not pass, and the end of what it printed or why it did not run.
  Failed { tool: String, detail: String },
}

impl Rechecked {
  /// Every tool of `passed` passed; none ran when it is empty.
  pub fn passed(passed: &[String]) -> Self {
    if passed.is_empty() {
      Self::NothingToRun
    } else {
      Self::Passed {
        tools: passed.join(", "),
      }
    }
  }
}

impl RecheckTool {
  /// The process that runs the tool in `worktree` of the repository at `repository`, with `inherited` (the variables
  /// of the process that starts it, `PATH` among them) and the project's `environment`; or why it cannot start: its
  /// program was not found, or a program stored as a fixed file leads elsewhere than when it was saved.
  pub fn process(
    &self,
    (worktree, repository): (&Path, &Path),
    environment: &CommandEnvironment,
    inherited: Vec<(OsString, OsString)>,
  ) -> Result<Command, String> {
    let action = &self.action;
    let inherited_path = inherited
      .iter()
      .find(|(name, _)| is_path(name))
      .map(|(_, value)| value.clone());
    let path = action.search_path(environment, inherited_path);
    let Some(program) = action
      .program_candidates(worktree, path.as_deref(), Platform::current())
      .into_iter()
      .find(|candidate: &PathBuf| candidate.is_file())
    else {
      return Err(format!("{} was not found", action.program().as_str()));
    };
    if action.program().check_place(&program, repository).is_err() {
      return Err(format!(
        "{} leads elsewhere than when it was saved",
        action.program().as_str()
      ));
    }
    Ok(action.process(&program, worktree, inherited, environment, Platform::current()))
  }

  /// Whether the tool passed, ended as `end` having printed `output` ([`RecheckOutput::finish`]); why not when it
  /// did not.
  pub fn verdict(&self, end: CommandEnd, output: String) -> Result<(), String> {
    let command = ToolCommand {
      tool: &self.name,
      action: &self.action,
    };
    if command.result(end) == ToolCommandResult::Passed {
      return Ok(());
    }
    Err(match end {
      CommandEnd::TimedOut => format!("did not finish within {} s", self.action.timeout().secs()),
      _ => output,
    })
  }
}

/// `PATH` as the platform names it: any case on Windows.
fn is_path(name: &OsString) -> bool {
  if cfg!(windows) {
    name.eq_ignore_ascii_case("PATH")
  } else {
    name == "PATH"
  }
}

/// What a checking tool printed, kept as a failed check's reason keeps it: its last [`KEPT_LINES`] lines that are not
/// blank, its errors after its output.
#[derive(Clone, Debug, Default)]
pub struct RecheckOutput {
  stdout: Vec<String>,
  stderr: Vec<String>,
}

impl RecheckOutput {
  /// One line the tool printed on `stream`.
  pub fn push(&mut self, stream: OutputStream, line: &str) {
    if line.trim().is_empty() {
      return;
    }
    let lines = match stream {
      OutputStream::Stdout => &mut self.stdout,
      OutputStream::Stderr => &mut self.stderr,
    };
    if lines.len() == KEPT_LINES {
      lines.remove(0);
    }
    lines.push(line.to_owned());
  }

  /// Every line of `stdout`, then of `stderr`.
  pub fn of(stdout: &str, stderr: &str) -> Self {
    let mut output = Self::default();
    for line in stdout.lines() {
      output.push(OutputStream::Stdout, line);
    }
    for line in stderr.lines() {
      output.push(OutputStream::Stderr, line);
    }
    output
  }

  /// The last [`KEPT_LINES`] lines, its errors after its output.
  pub fn finish(self) -> String {
    let lines: Vec<String> = self.stdout.into_iter().chain(self.stderr).collect();
    lines[lines.len().saturating_sub(KEPT_LINES)..].join("\n")
  }
}
