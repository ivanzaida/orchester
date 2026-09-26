//! A stand-in for `codex exec --json` in the runner's execution tests: it answers the way the
//! real CLI did in the probe (`.tmp/cli-probe/out/codex`), without a model. What it does depends
//! on the prompt it reads from standard input:
//!
//! - "Please fail": replays the failed turn of an unsupported model and exits with 1;
//! - "Keep working": starts a turn and waits until it is ended;
//! - anything else: appends `hello` to `README.md` in the `-C` directory, reports the command, the file change and the
//!   usage, and exits with 0.
//!
//! On standard error it reports what it was given: its arguments, the names of its environment
//! variables and the first line of the prompt, so tests can check how it was started.

#![allow(
  clippy::print_stdout,
  clippy::print_stderr,
  reason = "a stand-in for a CLI: printing is what it is for"
)]

use std::{
  fs::OpenOptions,
  io::{Read, Write},
  path::PathBuf,
  process::ExitCode,
  thread,
  time::Duration,
};

const FAILED_TURN: &str = include_str!("../../../core/tests/fixtures/codex/c11-bad-model-via-cmd-shim.jsonl");

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let mut names: Vec<String> = std::env::vars_os()
    .map(|(name, _)| name.to_string_lossy().into_owned())
    .collect();
  names.sort();
  let mut prompt = String::new();
  if std::io::stdin().read_to_string(&mut prompt).is_err() {
    eprintln!("cannot read the prompt");
    return ExitCode::from(2);
  }
  eprintln!("argv: {}", args.join(" "));
  eprintln!("env: {}", names.join(","));
  eprintln!("prompt: {}", prompt.lines().next().unwrap_or_default());
  if prompt.contains("Please fail") {
    print!("{FAILED_TURN}");
    return ExitCode::from(1);
  }
  println!(r#"{{"type":"thread.started","thread_id":"fake-thread-1"}}"#);
  println!(r#"{{"type":"turn.started"}}"#);
  if prompt.contains("Keep working") {
    flush();
    thread::sleep(Duration::from_secs(600));
    return ExitCode::SUCCESS;
  }
  let worktree = args
    .iter()
    .position(|arg| arg == "-C")
    .and_then(|index| args.get(index + 1))
    .map(PathBuf::from);
  let Some(worktree) = worktree else {
    eprintln!("no -C directory");
    return ExitCode::from(2);
  };
  let appended = OpenOptions::new()
    .append(true)
    .open(worktree.join("README.md"))
    .and_then(|mut readme| readme.write_all(b"hello\n"));
  if let Err(error) = appended {
    eprintln!("cannot write README.md: {error}");
    return ExitCode::from(2);
  }
  let command = r#"echo hello >> README.md"#;
  println!(
    r#"{{"type":"item.completed","item":{{"id":"item_0","type":"agent_message","text":"Appending the line."}}}}"#
  );
  println!(
    r#"{{"type":"item.started","item":{{"id":"item_1","type":"command_execution","command":"{command}","aggregated_output":"","exit_code":null,"status":"in_progress"}}}}"#
  );
  println!(
    r#"{{"type":"item.completed","item":{{"id":"item_1","type":"command_execution","command":"{command}","aggregated_output":"","exit_code":0,"status":"completed"}}}}"#
  );
  println!(
    r#"{{"type":"item.completed","item":{{"id":"item_2","type":"file_change","changes":[{{"path":"README.md","kind":"update"}}],"status":"completed"}}}}"#
  );
  println!("a line that is not JSON");
  println!(r#"{{"type":"item.completed","item":{{"id":"item_3","type":"agent_message","text":"Done."}}}}"#);
  println!(
    r#"{{"type":"turn.completed","usage":{{"input_tokens":12166,"cached_input_tokens":2432,"output_tokens":5,"reasoning_output_tokens":0}}}}"#
  );
  ExitCode::SUCCESS
}

fn flush() {
  if std::io::stdout().flush().is_err() {
    eprintln!("cannot flush standard output");
  }
}
