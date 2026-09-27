//! A stand-in for `codex exec --json` in the runner's execution tests: it answers the way the
//! real CLI did in the probe (`.tmp/cli-probe/out/codex`), without a model. What it does depends
//! on the prompt it reads from standard input:
//!
//! - "Please fail": replays the failed turn of an unsupported model and exits with 1;
//! - "Keep working": starts a turn, starts a helper process of its own and waits until it is ended;
//! - "Work in turns": appends `draft` to `README.md`, completes a turn that used 9,739 counted tokens, starts another
//!   one and waits until it is ended;
//! - "Work slowly": waits a moment, then works as below;
//! - "Wait for release": runs a command (`pnpm test`) that ends once the test writes `fake-release` next to the
//!   worktrees (`<repo>/.orchester/`), appending `draft` to `README.md` first when the prompt also says "Draft first";
//!   then it waits until it is ended when the prompt also says "Then keep working", else works as below;
//! - "Report failure": works as below, but its result says it failed;
//! - "Fail once": the first time a step works on it, the result says it failed; later it works as below;
//! - "Write own file": writes `<step>.txt` instead of appending to `README.md`;
//! - "Sign the line": appends `hello from <step>` instead of `hello`;
//! - "Ask first": appends `draft` to `README.md` and ends with the step result `{"outcome":"question","summary":"Keep
//!   email sign-in?"}` (a new session only: a resumed one works as below);
//! - anything else: appends `hello` to `README.md` in its worktree, reports the command, the file change and the usage,
//!   and exits with 0.
//!
//! Its final message is the step result when it was given `--output-schema` (`{"outcome", "summary"}`, the summary
//! "Finished <step>"), else "Done.". The step is read from the prompt's "# Current step:" line.
//!
//! `exec resume <session> -` continues session `fake-thread-1` in its working directory: it appends `continued` to
//! `README.md` and completes its turn, once no `fake-hold` lies next to the worktrees (a test holds a continued session
//! at work with it). A new session works in the `-C` directory.
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
  let Some(prompt) = read_prompt(&args) else {
    return ExitCode::from(2);
  };
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
  let result = StepReport {
    step: prompt
      .lines()
      .find_map(|line| line.strip_prefix("# Current step: "))
      .unwrap_or("the step")
      .to_owned(),
    structured: args.iter().any(|arg| arg == "--output-schema"),
    failed: prompt.contains("Report failure") || (prompt.contains("Fail once") && first_time(&worktree, &prompt)),
    own_file: prompt.contains("Write own file"),
  };
  if resumed {
    wait_while_held(&worktree);
    return finish(&worktree, "continued", &result);
  }
  if prompt.contains("Ask first") {
    return ask(&worktree);
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
  before_finishing(&worktree, &prompt);
  let line = if prompt.contains("Sign the line") {
    format!("hello from {}", result.step)
  } else {
    "hello".to_owned()
  };
  finish(&worktree, &line, &result)
}

/// Reads the prompt from standard input and reports on standard error how the CLI was started.
fn read_prompt(args: &[String]) -> Option<String> {
  let mut names: Vec<String> = std::env::vars_os()
    .map(|(name, _)| name.to_string_lossy().into_owned())
    .collect();
  names.sort();
  let mut prompt = String::new();
  if std::io::stdin().read_to_string(&mut prompt).is_err() {
    eprintln!("cannot read the prompt");
    return None;
  }
  eprintln!("argv: {}", args.join(" "));
  eprintln!("env: {}", names.join(","));
  eprintln!("prompt: {}", prompt.lines().next().unwrap_or_default());
  for line in prompt.lines() {
    eprintln!("prompt| {line}");
  }
  eprintln!("pid: {}", std::process::id());
  Some(prompt)
}

/// What the step's final message reports.
struct StepReport {
  step: String,
  structured: bool,
  failed: bool,
  own_file: bool,
}

/// Whether this is the first time a step with this prompt's step name works: a marker next to the worktrees
/// (`<repo>/.orchester/`) remembers it across attempts and worktrees.
fn first_time(worktree: &std::path::Path, prompt: &str) -> bool {
  let step = prompt
    .lines()
    .find_map(|line| line.strip_prefix("# Current step: "))
    .unwrap_or("step");
  let Some(orchester) = worktree.parent().and_then(std::path::Path::parent) else {
    return false;
  };
  let marker = orchester.join(format!("fake-failed-{step}"));
  if marker.exists() {
    return false;
  }
  std::fs::write(&marker, "failed once").is_ok()
}

/// Appends `line` to `README.md` (or writes the step's own file), reports it as a command and a file change, then
/// the final message and the usage.
fn finish(worktree: &std::path::Path, line: &str, result: &StepReport) -> ExitCode {
  let file = if result.own_file {
    format!("{}.txt", result.step.to_lowercase().replace(' ', "-"))
  } else {
    "README.md".to_owned()
  };
  let written = if result.own_file {
    std::fs::write(
      worktree.join(&file),
      format!(
        "{line}
"
      ),
    )
  } else {
    append(worktree, line)
  };
  if let Err(error) = written {
    eprintln!("cannot write {file}: {error}");
    return ExitCode::from(2);
  }
  let command = format!("echo {line} >> {file}");
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
    r#"{{"type":"item.completed","item":{{"id":"item_2","type":"file_change","changes":[{{"path":"{file}","kind":"update"}}],"status":"completed"}}}}"#
  );
  println!("a line that is not JSON");
  let text = if result.structured {
    let outcome = if result.failed { "failed" } else { "done" };
    format!(
      r#"{{\"outcome\":\"{outcome}\",\"summary\":\"Finished {}\"}}"#,
      result.step
    )
  } else {
    "Done.".to_owned()
  };
  println!(r#"{{"type":"item.completed","item":{{"id":"item_3","type":"agent_message","text":"{text}"}}}}"#);
  println!("{USAGE}");
  ExitCode::SUCCESS
}

/// "Ask first": a draft, then a question as the step result.
fn ask(worktree: &std::path::Path) -> ExitCode {
  if let Err(error) = append(worktree, "draft") {
    eprintln!("cannot write README.md: {error}");
    return ExitCode::from(2);
  }
  let question = r#"{\"outcome\":\"question\",\"summary\":\"Keep email sign-in?\"}"#;
  println!(r#"{{"type":"item.completed","item":{{"id":"item_q","type":"agent_message","text":"{question}"}}}}"#);
  println!("{USAGE}");
  ExitCode::SUCCESS
}

/// "Wait for release" and "Work slowly": what happens before the agent finishes its work.
fn before_finishing(worktree: &std::path::Path, prompt: &str) {
  if prompt.contains("Wait for release") {
    wait_for_release(worktree, prompt.contains("Draft first"));
    if prompt.contains("Then keep working") {
      wait_to_be_ended();
    }
  }
  if prompt.contains("Work slowly") {
    flush();
    thread::sleep(Duration::from_millis(1_500));
  }
}

/// Waits while the test holds continued sessions with `fake-hold` next to the worktrees.
fn wait_while_held(worktree: &std::path::Path) {
  let hold = worktree
    .parent()
    .and_then(std::path::Path::parent)
    .map(|orchester| orchester.join("fake-hold"));
  flush();
  while hold.as_ref().is_some_and(|hold| hold.exists()) {
    thread::sleep(Duration::from_millis(50));
  }
}

/// A command that runs until the test releases it.
fn wait_for_release(worktree: &std::path::Path, draft: bool) {
  if draft && let Err(error) = append(worktree, "draft") {
    eprintln!("cannot write README.md: {error}");
  }
  println!(
    r#"{{"type":"item.started","item":{{"id":"item_9","type":"command_execution","command":"pnpm test","aggregated_output":"","exit_code":null,"status":"in_progress"}}}}"#
  );
  flush();
  let release = worktree
    .parent()
    .and_then(std::path::Path::parent)
    .map(|orchester| orchester.join("fake-release"));
  while !release.as_ref().is_some_and(|release| release.exists()) {
    thread::sleep(Duration::from_millis(50));
  }
  println!(
    r#"{{"type":"item.completed","item":{{"id":"item_9","type":"command_execution","command":"pnpm test","aggregated_output":"ok","exit_code":0,"status":"completed"}}}}"#
  );
  flush();
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
