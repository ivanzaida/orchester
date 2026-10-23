//! Check on the merged result: the command tools of the run's checking tool steps, as the run pinned them, run again in
//! the run's worktree once it holds the current base branch, in pipeline order, each as its step ran it (found on the
//! `PATH` it starts with or in the worktree, with the project's variables and the tool's, stopped after the tool's
//! timeout). Every tool step checks but the starter's *Create pull request*, which delivers the work rather than checks
//! it, and never runs again ([`rechecks`]). The first that does not end as the tool expects fails the check. A pipeline
//! without a checking command tool step has nothing to run again.

use std::path::Path;

use orchester_core::{
  command::CommandError,
  connection::invocation::Platform,
  resource::{StarterStep, StepKey, Tool, ToolAction},
  run::{CommandEnd, Run, StepExecutor, ToolCommand},
  store::{ResourceStore, SettingsStore},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

/// Lines of a failed check's output its reason keeps.
const KEPT_LINES: usize = 12;

/// How Check went on the merged result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Checked {
  /// Every command tool passed; their names, joined.
  Passed { tools: String },
  /// The pipeline has no command tool step to run again.
  NothingToRun,
  /// `tool` did not pass, and the end of what it printed or why it did not run.
  Failed { tool: String, detail: String },
}

/// Whether the tool step keyed `key` runs again on the merged result: every one but the starter's *Create pull
/// request*, which opens a pull request rather than checks the work.
pub fn rechecks(key: &StepKey) -> bool {
  key.as_str() != StarterStep::CreatePullRequest.key()
}

/// Runs the command tools of `run`'s checking tool steps again in its `worktree`, with the project's settings in `db`.
pub(super) fn check(
  db: &ProjectDb,
  programs: &Programs,
  (root, worktree): (&Path, &Path),
  run: &Run,
) -> Result<Checked, StoreError> {
  let environment = db.project_environment()?;
  let mut passed: Vec<String> = Vec::new();
  for step in run.snapshot().steps().iter().filter(|step| rechecks(step.key())) {
    let StepExecutor::Tool { tool: pinned } = step.executor() else {
      continue;
    };
    let Some(version) = db.version::<Tool>(pinned.resource, pinned.number)? else {
      continue;
    };
    let ToolAction::Command(action) = version.body().action() else {
      continue;
    };
    let tool = db.resource(pinned.resource)?.map_or_else(
      || step.key().to_string(),
      |resource| resource.name().as_str().to_owned(),
    );
    let failed = |detail: String| Checked::Failed {
      tool: tool.clone(),
      detail,
    };
    let path = action.search_path(&environment, std::env::var_os("PATH"));
    let Some(program) = action
      .program_candidates(worktree, path.as_deref(), Platform::current())
      .into_iter()
      .find(|candidate| candidate.is_file())
    else {
      return Ok(failed(format!("{} was not found", action.program().as_str())));
    };
    if action.program().check_place(&program, root).is_err() {
      return Ok(failed(format!(
        "{} leads elsewhere than when it was saved",
        action.program().as_str()
      )));
    }
    let command = action.process(
      &program,
      worktree,
      std::env::vars_os(),
      &environment,
      Platform::current(),
    );
    let end = match programs.timed(action.timeout().duration()).run(&command) {
      Ok(output) => (
        CommandEnd::Exited { code: output.code },
        last_lines(&output.stdout, &output.stderr),
      ),
      Err(CommandError::TimedOut) => (CommandEnd::TimedOut, String::new()),
      Err(error) => return Ok(failed(error.to_string())),
    };
    let command_tool = ToolCommand { tool: &tool, action };
    if command_tool.result(end.0) != orchester_core::connection::stream::ToolCommandResult::Passed {
      let detail = match end.0 {
        CommandEnd::TimedOut => format!("did not finish within {} s", action.timeout().secs()),
        _ => end.1,
      };
      return Ok(failed(detail));
    }
    passed.push(tool);
  }
  Ok(if passed.is_empty() {
    Checked::NothingToRun
  } else {
    Checked::Passed {
      tools: passed.join(", "),
    }
  })
}

/// The last [`KEPT_LINES`] lines a program printed, its errors after its output.
fn last_lines(stdout: &str, stderr: &str) -> String {
  let lines: Vec<&str> = stdout
    .lines()
    .chain(stderr.lines())
    .filter(|line| !line.trim().is_empty())
    .collect();
  lines[lines.len().saturating_sub(KEPT_LINES)..].join("\n")
}
