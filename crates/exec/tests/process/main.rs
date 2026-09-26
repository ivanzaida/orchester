//! Running real programs. `git` stands in for any CLI: it is installed wherever Orchester is
//! developed and behaves the same on Windows and macOS.

use std::{path::Path, time::Duration};

use orchester_core::command::{Command, CommandError, CommandRunner};
use orchester_exec::ProcessRunner;

#[cfg(windows)]
mod shim;

const RUNNER: ProcessRunner = ProcessRunner::new(Duration::from_secs(30));

fn git<const N: usize>(args: [&str; N]) -> Command {
  Command::new(Path::new("git"), args).with_env("LC_ALL", "C")
}

#[test]
fn process_output_and_exit_status_are_captured() {
  let output = RUNNER.run(&git(["--version"])).expect("git runs");
  assert!(output.success);
  assert!(output.stdout.starts_with("git version"), "{}", output.stdout);
  let output = RUNNER.run(&git(["no-such-command"])).expect("git runs");
  assert!(!output.success);
  assert!(!output.stderr.is_empty());
}

#[test]
fn process_that_does_not_exist_is_not_found() {
  let missing = Command::new(Path::new("orchester-no-such-program"), ["--version"]);
  assert_eq!(RUNNER.run(&missing), Err(CommandError::NotFound));
}

#[test]
fn process_reads_the_given_input() {
  let command = git(["hash-object", "--stdin"]).with_stdin("hello\n".to_owned());
  let output = RUNNER.run(&command).expect("git runs");
  assert_eq!(output.stdout.trim(), "ce013625030ba8dba906f756967f9e9ca394464a");
}

#[test]
fn process_environment_is_added_and_removed() {
  let configured = git(["config", "--get", "orchester.probe"])
    .with_env("GIT_CONFIG_COUNT", "1")
    .with_env("GIT_CONFIG_KEY_0", "orchester.probe")
    .with_env("GIT_CONFIG_VALUE_0", "yes");
  let output = RUNNER.run(&configured).expect("git runs");
  assert_eq!(output.stdout.trim(), "yes");
  let output = RUNNER
    .run(&configured.without_env("GIT_CONFIG_COUNT"))
    .expect("git runs");
  assert!(!output.success, "the setting was hidden: {}", output.stdout);
}

#[test]
fn process_runs_in_the_given_directory() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let outside = RUNNER
    .run(&git(["rev-parse", "--is-inside-work-tree"]).with_current_dir(folder.path()))
    .expect("git runs");
  assert!(!outside.success);
  assert!(outside.stderr.contains("not a git repository"), "{}", outside.stderr);
  let init = RUNNER
    .run(&git(["init", "--quiet"]).with_current_dir(folder.path()))
    .expect("git runs");
  assert!(init.success, "{}", init.stderr);
  let inside = RUNNER
    .run(&git(["rev-parse", "--is-inside-work-tree"]).with_current_dir(folder.path()))
    .expect("git runs");
  assert_eq!(inside.stdout.trim(), "true");
}

#[test]
fn process_that_runs_too_long_is_stopped() {
  let runner = ProcessRunner::new(Duration::from_millis(300));
  assert_eq!(runner.run(&long_running()), Err(CommandError::TimedOut));
}

#[cfg(windows)]
fn long_running() -> Command {
  Command::new(Path::new("ping"), ["-n", "30", "127.0.0.1"])
}

#[cfg(not(windows))]
fn long_running() -> Command {
  Command::new(Path::new("sleep"), ["30"])
}

/// Prints the environment the program sees, one `NAME=value` per line.
fn print_environment() -> Command {
  if cfg!(windows) {
    Command::new(Path::new("cmd"), ["/C", "set"])
  } else {
    Command::new(Path::new("/usr/bin/env"), Vec::<String>::new())
  }
}

#[test]
fn process_with_an_explicit_environment_sees_nothing_else() {
  let mut variables: Vec<(String, String)> = std::env::vars()
    .filter(|(name, _)| name.eq_ignore_ascii_case("SYSTEMROOT"))
    .collect();
  variables.push(("ORCHESTER_PROBE".into(), "yes".into()));
  let output = RUNNER
    .run(&print_environment().with_environment(variables))
    .expect("the program runs");
  let names: Vec<&str> = output
    .stdout
    .lines()
    .filter_map(|line| line.split_once('=').map(|(name, _)| name))
    .collect();
  assert!(names.contains(&"ORCHESTER_PROBE"), "{names:?}");
  // cmd.exe adds a few variables of its own (PROMPT, COMSPEC, PATHEXT); the parent's are gone.
  assert!(!names.iter().any(|name| name.eq_ignore_ascii_case("PATH")), "{names:?}");
  assert!(
    !names.iter().any(|name| name.eq_ignore_ascii_case("CARGO")),
    "{names:?}"
  );
}
