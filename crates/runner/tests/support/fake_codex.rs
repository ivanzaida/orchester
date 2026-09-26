//! A stand-in for `codex exec --json` in the runner's execution tests: it answers the way the
//! real CLI did in the probe (`.tmp/cli-probe/out/codex`), without a model. What it does depends
//! on the prompt it reads from standard input:
//!
//! - "Please fail": replays the failed turn of an unsupported model and exits with 1;
//! - "Keep working": starts a turn, starts a helper process of its own and waits until it is ended;
//! - "Work in turns": appends `draft` to `README.md`, completes a turn that used 9,739 counted tokens, starts another
//!   one and waits until it is ended;
//! - "Work slowly": waits a moment, then works as below;
//! - anything else: appends `hello` to `README.md` in its worktree, reports the command, the file change and the usage,
//!   and exits with 0.
//!
//! `exec resume <session> -` continues session `fake-thread-1` in its working directory: it appends `continued` to
//! `README.md` and completes its turn. A new session works in the `-C` directory.
//!
//! On standard error it reports what it was given: its arguments, the names of its environment
//! variables and the first line of the prompt, so tests can check how it was started, and the
//! process ids of itself and its helper, so tests can check that ending it ends both.

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
const USAGE: &str = r#"{"type":"turn.completed","usage":{"input_tokens":12166,"cached_input_tokens":2432,"output_tokens":5,"reasoning_output_tokens":0}}"#;

/// Set for the helper process, which only waits.
const HELPER: &str = "ORCHESTER_FAKE_CODEX_HELPER";

fn main() -> ExitCode {
  if std::env::var_os(HELPER).is_some() {
    thread::sleep(Duration::from_secs(600));
    return ExitCode::SUCCESS;
  }
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
  eprintln!("pid: {}", std::process::id());
  if prompt.contains("Please fail") {
    print!("{FAILED_TURN}");
    return ExitCode::from(1);
  }
  let resumed = args.iter().any(|arg| arg == "resume");
  let worktree = if resumed {
    std::env::current_dir().ok()
  } else {
    args
      .iter()
      .position(|arg| arg == "-C")
      .and_then(|index| args.get(index + 1))
      .map(PathBuf::from)
  };
  let Some(worktree) = worktree else {
    eprintln!("no worktree");
    return ExitCode::from(2);
  };
  println!(r#"{{"type":"thread.started","thread_id":"fake-thread-1"}}"#);
  println!(r#"{{"type":"turn.started"}}"#);
  if resumed {
    return finish(&worktree, "continued");
  }
  if prompt.contains("Keep working") {
    match start_helper() {
      Ok(helper) => eprintln!("helper pid: {helper}"),
      Err(error) => eprintln!("cannot start the helper: {error}"),
    }
    wait_to_be_ended();
  }
  if prompt.contains("Work in turns") {
    if let Err(error) = append(&worktree, "draft") {
      eprintln!("cannot write README.md: {error}");
      return ExitCode::from(2);
    }
    println!("{USAGE}");
    println!(r#"{{"type":"turn.started"}}"#);
    wait_to_be_ended();
  }
  if prompt.contains("Work slowly") {
    flush();
    thread::sleep(Duration::from_millis(1_500));
  }
  finish(&worktree, "hello")
}

/// Appends `line` to `README.md`, reports it as a command and a file change, then the usage.
fn finish(worktree: &std::path::Path, line: &str) -> ExitCode {
  if let Err(error) = append(worktree, line) {
    eprintln!("cannot write README.md: {error}");
    return ExitCode::from(2);
  }
  let command = format!("echo {line} >> README.md");
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
  println!("{USAGE}");
  ExitCode::SUCCESS
}

fn append(worktree: &std::path::Path, line: &str) -> std::io::Result<()> {
  OpenOptions::new()
    .append(true)
    .open(worktree.join("README.md"))
    .and_then(|mut readme| writeln!(readme, "{line}"))
}

fn start_helper() -> std::io::Result<u32> {
  let helper = std::process::Command::new(std::env::current_exe()?)
    .env(HELPER, "1")
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()?;
  Ok(helper.id())
}

fn wait_to_be_ended() -> ! {
  flush();
  thread::sleep(Duration::from_secs(600));
  std::process::exit(0)
}

fn flush() {
  if std::io::stdout().flush().is_err() {
    eprintln!("cannot flush standard output");
  }
}
