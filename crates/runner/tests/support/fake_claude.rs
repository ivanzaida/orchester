//! A stand-in for `claude -p --output-format stream-json --verbose` in the runner's execution tests.
//! The real CLI was never observed doing a model turn (it was not signed in during the probe), so
//! apart from the sign-in failure, which replays real output
//! (`crates/core/tests/fixtures/claude/not-signed-in-clean-env.jsonl`), the lines it prints are
//! synthetic, shaped like the events the probe recorded. What it does depends on the prompt it
//! reads from standard input:
//!
//! - "Please sign in": answers like a CLI that is not signed in and exits with 1;
//! - "Ask for the web": tries `curl`, which the permissions deny, then works as below;
//! - "Work in turns": appends `draft` to `README.md`, finishes a message that used 9,739 counted tokens, starts another
//!   one and waits until it is ended;
//! - "Keep working": starts a message and waits until it is ended;
//! - "Ask first": appends `draft` to `README.md` and ends with the step result `{"outcome":"question","summary":"Keep
//!   email sign-in?"}` (a new session only: a resumed one works as below);
//! - "Wait for release": runs `pnpm test` through Bash, which ends once the test writes `fake-release` next to the
//!   worktrees (`<repo>/.orchester/`), appending `draft` to `README.md` first when the prompt also says "Draft first";
//!   then it waits until it is ended when the prompt also says "Then keep working", else works as below;
//! - anything else: edits `README.md` in its working directory (appending `hello`), runs `git status`, and ends with
//!   the step result `{"outcome":"done","summary":"Finished <step>"}` and exit 0.
//!
//! `--resume <session>` continues that session: once no `fake-hold` lies next to the worktrees (a test holds a
//! continued session at work with it), it appends `continued` to `README.md` and ends the same way. A new
//! session takes its id from `--session-id`.
//!
//! On standard error it reports its arguments, the names of its environment variables, the prompt (its first line, then
//! each line) and its process id, like `orchester-fake-codex`.

#![allow(
  clippy::print_stdout,
  clippy::print_stderr,
  reason = "a stand-in for a CLI: printing is what it is for"
)]

use std::{
  fs::OpenOptions,
  io::{Read, Write},
  path::Path,
  process::ExitCode,
  thread,
  time::Duration,
};

const NOT_SIGNED_IN: &str = include_str!("../../../core/tests/fixtures/claude/not-signed-in-clean-env.jsonl");
/// Counted: 12 + 9,700 (cache writes) + 27 = 9,739; the 2,432 cache reads do not count.
const USAGE: &str =
  r#"{"input_tokens":12,"output_tokens":27,"cache_creation_input_tokens":9700,"cache_read_input_tokens":2432}"#;
/// A message that is still being written has not used anything yet.
const NO_USAGE: &str = r#"{"input_tokens":0,"output_tokens":0}"#;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let Some(prompt) = read_prompt(&args) else {
    return ExitCode::from(2);
  };
  if prompt.contains("Please sign in") {
    print!("{NOT_SIGNED_IN}");
    return ExitCode::from(1);
  }
  let after = |flag: &str| {
    args
      .iter()
      .position(|arg| arg == flag)
      .and_then(|index| args.get(index + 1))
      .cloned()
  };
  let resumed = after("--resume");
  let Some(session) = resumed.clone().or_else(|| after("--session-id")) else {
    eprintln!("no session");
    return ExitCode::from(2);
  };
  let Ok(worktree) = std::env::current_dir() else {
    eprintln!("no working directory");
    return ExitCode::from(2);
  };
  let fake = Fake { session };
  println!(
    r#"{{"type":"system","subtype":"init","cwd":"worktree","session_id":"{}","tools":[],"mcp_servers":[],"model":"claude-haiku-4-5","permissionMode":"acceptEdits","apiKeySource":"none","claude_code_version":"2.1.89"}}"#,
    fake.session
  );
  let step = prompt
    .lines()
    .find_map(|line| line.strip_prefix("# Current step: "))
    .unwrap_or("the step")
    .to_owned();
  if resumed.is_some() {
    wait_while_held(&worktree);
    return fake.finish(&worktree, "continued", &step);
  }
  if prompt.contains("Keep working") {
    fake.message("msg_wait", r#"{"type":"text","text":"Working."}"#, NO_USAGE);
    wait_to_be_ended();
  }
  if prompt.contains("Ask first") {
    return fake.ask(&worktree);
  }
  if prompt.contains("Work in turns") {
    if let Err(error) = append(&worktree, "draft") {
      eprintln!("cannot write README.md: {error}");
      return ExitCode::from(2);
    }
    fake.assistant("msg_draft", r#"{"type":"text","text":"Drafted."}"#);
    fake.message("msg_more", r#"{"type":"text","text":"Going on."}"#, NO_USAGE);
    wait_to_be_ended();
  }
  if prompt.contains("Wait for release") {
    fake.wait_for_release(&worktree, &prompt);
  }
  if prompt.contains("Ask for the web") {
    fake.assistant(
      "msg_web",
      r#"{"type":"tool_use","id":"toolu_web","name":"Bash","input":{"command":"curl https://example.com"}}"#,
    );
    fake.tool_result(
      "toolu_web",
      "Claude requested permissions to use Bash, but you haven't granted it yet.",
      true,
    );
    return fake.finish_with(&worktree, "hello", &step, DENIAL);
  }
  fake.finish(&worktree, "hello", &step)
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

const DENIAL: &str =
  r#"[{"tool_name":"Bash","tool_use_id":"toolu_web","tool_input":{"command":"curl https://example.com"}}]"#;

struct Fake {
  session: String,
}

impl Fake {
  fn assistant(&self, id: &str, block: &str) {
    self.message(id, block, USAGE);
  }

  fn message(&self, id: &str, block: &str, usage: &str) {
    println!(
      r#"{{"type":"assistant","message":{{"id":"{id}","role":"assistant","type":"message","content":[{block}],"usage":{usage}}},"parent_tool_use_id":null,"session_id":"{}"}}"#,
      self.session
    );
    flush();
  }

  fn tool_result(&self, id: &str, content: &str, is_error: bool) {
    println!(
      r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{id}","content":"{content}","is_error":{is_error}}}]}},"session_id":"{}"}}"#,
      self.session
    );
  }

  /// A command that runs until the test releases it.
  fn wait_for_release(&self, worktree: &Path, prompt: &str) {
    if prompt.contains("Draft first") && append(worktree, "draft").is_err() {
      eprintln!("cannot write README.md");
    }
    self.assistant(
      "msg_test",
      r#"{"type":"tool_use","id":"toolu_test","name":"Bash","input":{"command":"pnpm test"}}"#,
    );
    let release = worktree
      .parent()
      .and_then(Path::parent)
      .map(|orchester| orchester.join("fake-release"));
    while !release.as_ref().is_some_and(|release| release.exists()) {
      thread::sleep(Duration::from_millis(50));
    }
    self.tool_result("toolu_test", "ok", false);
    if prompt.contains("Then keep working") {
      self.message("msg_wait", r#"{"type":"text","text":"Working."}"#, NO_USAGE);
      wait_to_be_ended();
    }
  }

  /// "Ask first": a draft, then a question as the step result.
  fn ask(&self, worktree: &Path) -> ExitCode {
    if let Err(error) = append(worktree, "draft") {
      eprintln!("cannot write README.md: {error}");
      return ExitCode::from(2);
    }
    let question = r#"{\"outcome\":\"question\",\"summary\":\"Keep email sign-in?\"}"#;
    self.assistant("msg_ask", &format!(r#"{{"type":"text","text":"{question}"}}"#));
    println!(
      r#"{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"{question}","session_id":"{}","total_cost_usd":0.1,"usage":{{"input_tokens":0,"output_tokens":0}},"permission_denials":[]}}"#,
      self.session
    );
    ExitCode::SUCCESS
  }

  fn finish(&self, worktree: &Path, line: &str, step: &str) -> ExitCode {
    self.finish_with(worktree, line, step, "[]")
  }

  /// Appends `line` to `README.md` through an edit, runs `git status`, then ends with the step result.
  fn finish_with(&self, worktree: &Path, line: &str, step: &str, denials: &str) -> ExitCode {
    if let Err(error) = append(worktree, line) {
      eprintln!("cannot write README.md: {error}");
      return ExitCode::from(2);
    }
    self.assistant(
      "msg_edit",
      r#"{"type":"tool_use","id":"toolu_edit","name":"Edit","input":{"file_path":"README.md","old_string":"hi","new_string":"hi"}}"#,
    );
    self.tool_result("toolu_edit", "The file README.md has been updated.", false);
    self.assistant(
      "msg_git",
      r#"{"type":"tool_use","id":"toolu_git","name":"Bash","input":{"command":"git status --short"}}"#,
    );
    self.tool_result("toolu_git", " M README.md", false);
    println!("a line that is not JSON");
    let result = format!(r#"{{\"outcome\":\"done\",\"summary\":\"Finished {step}\"}}"#);
    self.assistant("msg_done", &format!(r#"{{"type":"text","text":"{result}"}}"#));
    println!(
      r#"{{"type":"result","subtype":"success","is_error":false,"num_turns":3,"result":"{result}","session_id":"{}","total_cost_usd":0.5,"usage":{{"input_tokens":0,"output_tokens":0}},"permission_denials":{denials}}}"#,
      self.session
    );
    ExitCode::SUCCESS
  }
}

fn append(worktree: &Path, line: &str) -> std::io::Result<()> {
  OpenOptions::new()
    .append(true)
    .open(worktree.join("README.md"))
    .and_then(|mut readme| writeln!(readme, "{line}"))
}

/// Waits while the test holds continued sessions with `fake-hold` next to the worktrees.
fn wait_while_held(worktree: &Path) {
  let hold = worktree
    .parent()
    .and_then(Path::parent)
    .map(|orchester| orchester.join("fake-hold"));
  while hold.as_ref().is_some_and(|hold| hold.exists()) {
    thread::sleep(Duration::from_millis(50));
  }
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
