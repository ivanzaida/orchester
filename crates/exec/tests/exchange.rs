//! `LineExchange` for real processes: a program that answers line by line while its input stays open is read until the
//! caller has its answer, then ends once its input closes; one that never answers is stopped at the timeout. `git
//! cat-file --batch-check` stands in for Codex App Server: it answers each line and waits for the next.

use std::{path::Path, time::Duration};

use orchester_core::command::{Command, CommandError, LineExchange};
use orchester_exec::ProcessRunner;

fn batch_check(repo: &Path, input: &str) -> Command {
  Command::new(Path::new("git"), ["cat-file", "--batch-check"])
    .with_current_dir(repo)
    .with_interactive_input(input.to_owned())
}

fn repository() -> tempfile::TempDir {
  let repo = tempfile::tempdir().expect("a temporary folder");
  let status = std::process::Command::new("git")
    .args(["init", "--quiet"])
    .current_dir(repo.path())
    .status()
    .expect("git runs");
  assert!(status.success(), "git init");
  repo
}

#[test]
fn exchange_reads_until_the_answer_then_closes_the_input() {
  let repo = repository();
  let runner = ProcessRunner::new(Duration::from_secs(30));
  let mut seen = 0;
  let lines = runner
    .exchange(&batch_check(repo.path(), "1111111\n2222222\n"), &mut |line| {
      seen += 1;
      line.starts_with("2222222")
    })
    .expect("git answered");
  assert_eq!(lines, ["1111111 missing", "2222222 missing"]);
  assert_eq!(seen, 2, "nothing is read after the answer");
}

#[test]
fn exchange_stops_a_program_that_never_answers() {
  let repo = repository();
  let runner = ProcessRunner::new(Duration::from_millis(1500));
  let result = runner.exchange(&batch_check(repo.path(), "3333333\n"), &mut |_| false);
  assert_eq!(result, Err(CommandError::TimedOut));
}

#[test]
fn exchange_of_a_program_that_is_not_there_says_so() {
  let runner = ProcessRunner::new(Duration::from_secs(5));
  let command = Command::new(Path::new("orchester-no-such-program"), ["--stdio"]).with_interactive_stdin();
  assert_eq!(runner.exchange(&command, &mut |_| true), Err(CommandError::NotFound));
}
