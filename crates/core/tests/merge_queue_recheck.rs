//! Check on the merged result as the runner runs it (ORC-684, `orchester_core::merge_queue::recheck`): a tool's program
//! is found in the run's worktree or not at all, its end decides whether it passed, and a failed check keeps the last
//! lines it printed, its errors after its output. `cargo test -p orchester-core --test merge_queue_recheck recheck`.

use std::fs;

use orchester_core::{
  connection::stream::OutputStream,
  merge_queue::recheck::{RecheckOutput, RecheckTool, Rechecked},
  resource::{CommandAction, CommandEnvironment, CommandProgram},
  run::CommandEnd,
};

fn tool(program: &str) -> RecheckTool {
  RecheckTool {
    name: "Run tests".to_owned(),
    action: CommandAction::new(CommandProgram::new(program).expect("program"), Vec::new()),
  }
}

#[test]
fn recheck_passed_names_every_tool_or_nothing_to_run() {
  assert_eq!(Rechecked::passed(&[]), Rechecked::NothingToRun);
  assert_eq!(
    Rechecked::passed(&["Run tests".to_owned(), "Lint".to_owned()]),
    Rechecked::Passed {
      tools: "Run tests, Lint".to_owned()
    }
  );
}

#[test]
fn recheck_finds_the_program_in_the_worktree_and_starts_it_there() {
  let repository = tempfile::tempdir().expect("repository");
  let worktree = repository.path().join(".orchester/worktrees/run");
  fs::create_dir_all(worktree.join("tools")).expect("worktree");
  fs::write(worktree.join("tools/check.cmd"), "").expect("program");
  let environment = CommandEnvironment::default();
  let process = tool("tools/check.cmd")
    .process((&worktree, repository.path()), &environment, Vec::new())
    .expect("found");
  assert_eq!(process.program(), worktree.join("tools/check.cmd"));
  assert_eq!(process.current_dir(), Some(worktree.as_path()));
  assert_eq!(
    tool("tools/missing.cmd").process((&worktree, repository.path()), &environment, Vec::new()),
    Err("tools/missing.cmd was not found".to_owned())
  );
}

#[test]
fn recheck_verdict_passes_on_success_and_says_why_otherwise() {
  let tool = tool("check.cmd");
  assert_eq!(
    tool.verdict(CommandEnd::Exited { code: Some(0) }, String::new()),
    Ok(())
  );
  assert_eq!(
    tool.verdict(CommandEnd::Exited { code: Some(1) }, "1 failed".to_owned()),
    Err("1 failed".to_owned())
  );
  assert_eq!(
    tool.verdict(CommandEnd::TimedOut, "partial".to_owned()),
    Err(format!("did not finish within {} s", tool.action.timeout().secs()))
  );
}

#[test]
fn recheck_output_keeps_the_last_twelve_lines_errors_after_output() {
  let stdout: String = (1..=20).map(|line| format!("out {line}\n\n")).collect();
  let mut output = RecheckOutput::of(&stdout, "err 1\n  \nerr 2\n");
  let kept = output.clone().finish();
  let expected: Vec<String> = (11..=20)
    .map(|line| format!("out {line}"))
    .chain(["err 1".to_owned(), "err 2".to_owned()])
    .collect();
  assert_eq!(kept, expected.join("\n"));
  output.push(OutputStream::Stderr, "err 3");
  assert!(output.finish().ends_with("err 2\nerr 3"));
}
