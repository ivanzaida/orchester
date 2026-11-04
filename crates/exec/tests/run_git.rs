//! The runner's side of the Orchestrator chat's git in a run's worktree (`orchester_exec::run_git::serve`): git's
//! output is kept to the most an answer carries; git that writes more where the whole of it was asked for is answered
//! as not run, never cut short in silence.

use std::{path::Path, sync::Arc};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  orchestrator::run_git::{GitInvocation, RunGitAnswer, RunGitCall},
};
use orchester_exec::{
  programs::Programs,
  run_git::{serve, too_large},
};

/// git that writes `len` NUL characters, as `status -z` of a worktree with many changes does.
struct Writes {
  len: usize,
}

impl CommandRunner for Writes {
  fn run(&self, _command: &Command) -> Result<CommandOutput, CommandError> {
    Ok(CommandOutput {
      code: Some(0),
      stdout: "\u{0}".repeat(self.len),
      stderr: String::new(),
    })
  }
}

fn status(cap: Option<u64>) -> RunGitCall {
  RunGitCall::Git(GitInvocation {
    args: ["status", "--porcelain=v2", "-z"].map(str::to_owned).to_vec(),
    input: None,
    ssh: None,
    timeout_ms: 30_000,
    cap,
  })
}

#[test]
fn run_git_output_larger_than_an_answer_holds_is_not_sent() {
  let programs = Programs::with_runner(Arc::new(Writes { len: 100 }));
  let dir = Path::new(".");

  assert_eq!(
    serve(&programs, dir, &status(None), 99),
    RunGitAnswer::NotRun { error: too_large(99) },
    "the whole output was asked for and does not fit"
  );
  assert_eq!(
    serve(&programs, dir, &status(Some(1_000)), 99),
    RunGitAnswer::NotRun { error: too_large(99) },
    "a cap above the most an answer holds is no cap"
  );

  let RunGitAnswer::Ran { output, cut } = serve(&programs, dir, &status(Some(10)), 99) else {
    panic!("a smaller cap cuts as asked");
  };
  assert!(cut && output.stdout.len() == 10, "{output:?}");

  let RunGitAnswer::Ran { output, cut } = serve(&programs, dir, &status(None), 100) else {
    panic!("an output that fits is sent whole");
  };
  assert!(!cut && output.stdout.len() == 100, "{output:?}");
}
