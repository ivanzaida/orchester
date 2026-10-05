//! Answers of programs: bounded, with the exit code; project tools with their fixed arguments.

use std::path::Path;

use orchester_core::{
  command::CommandError,
  orchester_tools::{OutputKept, ProgramOutput, ProjectTool, StepTools, ToolCall, ToolCallError, ToolContext},
  permission::NetworkAccess,
  resource::{CommandAction, CommandProgram, OutputLimit, RepositoryAccess},
};
use serde_json::json;

use crate::{Folders, context, parse, permissions, program_of, project_tool, text};

/// A program's answer is its exit code and what it printed, cut at the end a tool keeps; a stop at the
/// time limit and an exit outside the expected codes are errors.
#[test]
fn orchester_tools_answers_are_bounded_and_carry_the_exit_code() {
  let folders = Folders::new();
  let reads = folders.reads();
  let limited = CommandAction::new(
    CommandProgram::new(&text(&std::env::current_exe().expect("exe"))).expect("a program"),
    Vec::new(),
  )
  .with_output_limit(OutputLimit::from_kib(1).expect("a limit"));
  let tools = [ProjectTool::new("Long", "prints a lot", limited)];
  let context = context(&folders.worktree, &reads, &tools);
  let program = program_of(
    &ToolCall::RunProjectTool {
      name: "Long".to_owned(),
    },
    &context,
  );
  let output = |code: Option<i32>, stdout: String| {
    Ok(ProgramOutput {
      code,
      stdout,
      stderr: "warning".to_owned(),
      left_out: 0,
    })
  };
  let long = format!("{}END", "x".repeat(4000));
  let result = program.finished(output(Some(0), long));
  assert!(!result.is_error());
  assert!(result.text().starts_with("exit code: 0\n"), "{}", result.text());
  assert!(result.text().contains("[stderr]\nwarning"));
  assert!(
    result.text().ends_with("bytes of output at the start left out]"),
    "{}",
    result.text()
  );
  assert!(result.text().len() < 1200);
  let failed = program.finished(output(Some(3), "boom".to_owned()));
  assert!(failed.is_error());
  assert!(failed.text().starts_with("exit code: 3\nboom"));
  let stopped = program.finished(Err(CommandError::TimedOut));
  assert!(stopped.is_error());
  assert!(stopped.text().contains("did not finish within 600 seconds"));
  let git = program_of(&parse("git_log", &json!({})).expect("valid"), &context);
  let result = git.finished(output(Some(0), format!("START{}", "y".repeat(200_000))));
  assert!(result.text().starts_with("exit code: 0\nSTART"));
  assert!(result.text().ends_with("bytes of output at the end left out]"));
}
#[test]
fn orchester_tools_run_project_tools_with_their_fixed_arguments() {
  let folders = Folders::new();
  let reads = folders.reads();
  let exe = text(&std::env::current_exe().expect("exe"));
  let tools = [project_tool("Run tests", &exe)];
  let context = context(&folders.worktree, &reads, &tools);
  let mut online = permissions(RepositoryAccess::Write);
  online.set_network(NetworkAccess::On);
  let step = StepTools::for_step(&online, &tools);
  let call = ToolCall::parse("run_project_tool", &json!({"name": "Run tests"}), &step).expect("offered");
  let program = program_of(&call, &context);
  assert_eq!(program.command().program(), Path::new(&exe));
  assert_eq!(program.command().args(), ["--quiet", "a; rm -rf /"]);
  assert_eq!(program.command().current_dir(), Some(folders.worktree.as_path()));
  assert_eq!(
    ToolCall::parse("run_project_tool", &json!({"name": "Deploy"}), &step),
    Err(ToolCallError::UnknownProjectTool("Deploy".to_owned()))
  );
  let missing = [project_tool("Gone", "no-such-program-for-orchester-tests")];
  let context = ToolContext {
    project_tools: &missing,
    ..context
  };
  assert_eq!(
    ToolCall::RunProjectTool {
      name: "Gone".to_owned()
    }
    .work(&context),
    Err(ToolCallError::ProgramNotFound(
      "no-such-program-for-orchester-tests".to_owned()
    ))
  );
}

/// What the runner did not keep of a program's streams counts with what the answer cuts.
#[test]
fn orchester_tools_answers_count_what_the_runner_left_out() {
  let folders = Folders::new();
  let reads = folders.reads();
  let context = context(&folders.worktree, &reads, &[]);
  let git = program_of(
    &parse("git_show", &json!({"revision": "HEAD"})).expect("valid"),
    &context,
  );
  assert_eq!(git.cap(), (128 * 1024, OutputKept::Head));
  let result = git.finished(Ok(ProgramOutput {
    code: Some(0),
    stdout: "commit 1".to_owned(),
    stderr: String::new(),
    left_out: 5_000_000,
  }));
  assert_eq!(
    result.text(),
    "exit code: 0\ncommit 1\n[5000000 bytes of output at the end left out]"
  );
}
