//! What a turn of `orchester-fake-codex` does, by its prompt:
//!
//! - "Please fail": fails the turn as Codex did for an unsupported model;
//! - "Keep working": starts a helper process of its own, reports its id (`helper pid: `) and waits until it is ended;
//! - "Work slowly": waits a moment, then works as below;
//! - "Wait for release": runs a command (`pnpm test`) that ends once the test writes `fake-release` next to the
//!   worktrees (`<repo>/.orchester/`), appending `draft` to `README.md` first when the prompt also says "Draft first";
//!   then it waits until it is ended when the prompt also says "Then keep working", else works as below;
//! - "Report failure": works as below, but its result says it failed;
//! - "Leave a handoff": works as below, and its step result hands off (`completed`, `decisions`, `open_questions`,
//!   `next_agent_should_know`), with the decision "Greet in README.md from <step>";
//! - "Fail once": the first time a step works on it, the result says it failed; later it works as below;
//! - "Flood standard error": first writes Codex's coloured warning about unknown settings with the setting it names, a
//!   line of 5000 characters, 3995 `y` followed by the value of `DOCS_TOKEN` (an MCP server's secret, when the process
//!   has it) and 1100 numbered lines (`stderr line <n>`) to standard error, then works as below;
//! - "Write own file": writes `<step>.txt` instead of appending to `README.md`;
//! - "Sign the line": appends `hello from <step>` instead of `hello`;
//! - "Ask first": appends `draft` to `README.md` and ends with the step result `{"outcome":"question","summary":"Keep
//!   email sign-in?"}` (a new session only: a resumed one works as below);
//! - the requests a person decides (`fake_app_server`): "Continue with approvals", "Hold approved command", "Hold
//!   approved MCP" and "Release approved command" in a resumed session, "Escalate `<command>`" and "Patch outside";
//! - anything else: appends `hello` to `README.md` in its worktree, reports the command, the file change and the usage,
//!   and completes the turn.
//!
//! A resumed session appends `continued` to `README.md` and completes its turn, once no `fake-hold` lies next to the
//! worktrees (a test holds a continued session at work with it). The final message is the step result when the turn
//! was given an output schema (`{"outcome", "summary"}`, the summary "Finished <step>"), else "Done.". The step is read
//! from the prompt's "# Current step:" line.

use std::{fs::OpenOptions, io::Write, path::Path, process::ExitCode, thread, time::Duration};

use serde_json::json;

use super::{HELPER, Session, Step, complete, fake_app_server, notify};

/// What an unsupported model's turn failed with.
const UNSUPPORTED: &str = "The 'no-such-model-probe' model is not supported when using Codex with a ChatGPT account.";

/// What the step's final message reports.
pub struct StepReport {
  step: String,
  structured: bool,
  failed: bool,
  own_file: bool,
  handoff: bool,
}

pub fn work(session: &mut Session, prompt: &str, structured: bool) -> Step {
  let worktree = session.cwd.clone();
  if prompt.contains("Please fail") {
    notify(session, "error", json!({"error": {"message": UNSUPPORTED}}));
    complete(session, Some(UNSUPPORTED));
    return Step::Exit(ExitCode::SUCCESS);
  }
  if let Some(step) = fake_app_server::start(session, prompt, structured) {
    return step;
  }
  let mut result = report_of(prompt, structured);
  result.failed = prompt.contains("Report failure") || (prompt.contains("Fail once") && first_time(&worktree, prompt));
  result.own_file = prompt.contains("Write own file");
  result.handoff = prompt.contains("Leave a handoff");
  if session.resumed {
    wait_while_held(&worktree);
    return finish(session, "continued", &result);
  }
  if prompt.contains("Ask first") {
    return ask(session);
  }
  if prompt.contains("Keep working") {
    match start_helper() {
      Ok(helper) => eprintln!("helper pid: {helper}"),
      Err(error) => eprintln!("cannot start the helper: {error}"),
    }
    wait_to_be_ended();
  }
  if prompt.contains("Flood standard error") {
    flood_stderr();
  }
  before_finishing(session, prompt);
  let line = if prompt.contains("Sign the line") {
    format!("hello from {}", result.step)
  } else {
    "hello".to_owned()
  };
  finish(session, &line, &result)
}

/// What App Server writes when the user's configuration has a setting it does not know, then more
/// than the runner keeps.
fn flood_stderr() {
  eprintln!(
    "\u{1b}[2m2026-10-01T15:47:47Z\u{1b}[0m \u{1b}[31mERROR\u{1b}[0m codex_app_server: Codex is ignoring 1 \
     unrecognized configuration setting. Check for typos or deprecated settings."
  );
  eprintln!("  user (C:\\Users\\me\\.codex\\config.toml): `disable_response_storage` is ignored.");
  eprintln!("\u{1b}[33mWARN\u{1b}[0m {}", "x".repeat(5000));
  if let Ok(secret) = std::env::var("DOCS_TOKEN") {
    eprintln!("{}{secret}", "y".repeat(3995));
  }
  for line in 0..1100 {
    eprintln!("stderr line {line}");
  }
}

/// Whether this is the first time a step with this prompt's step name works: a marker next to the worktrees
/// (`<repo>/.orchester/`) remembers it across attempts and worktrees.
fn first_time(worktree: &Path, prompt: &str) -> bool {
  let step = prompt
    .lines()
    .find_map(|line| line.strip_prefix("# Current step: "))
    .unwrap_or("step");
  let Some(orchester) = worktree.parent().and_then(Path::parent) else {
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
pub fn finish(session: &Session, line: &str, result: &StepReport) -> Step {
  let worktree = &session.cwd;
  let file = if result.own_file {
    format!("{}.txt", result.step.to_lowercase().replace(' ', "-"))
  } else {
    "README.md".to_owned()
  };
  let written = if result.own_file {
    std::fs::write(worktree.join(&file), format!("{line}\n"))
  } else {
    append(worktree, line)
  };
  if let Err(error) = written {
    eprintln!("cannot write {file}: {error}");
    return Step::Exit(ExitCode::from(2));
  }
  let command = format!("echo {line} >> {file}");
  message(session, "item_0", "Appending the line.");
  notify(
    session,
    "item/started",
    json!({"item": {"id": "item_1", "type": "commandExecution", "command": command}}),
  );
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "item_1", "type": "commandExecution", "command": command, "status": "completed", "exitCode": 0, "aggregatedOutput": ""}}),
  );
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "item_2", "type": "fileChange", "status": "completed", "changes": [{"path": file, "kind": {"type": "update"}}]}}),
  );
  let text = if result.structured {
    let mut report = json!({
      "outcome": if result.failed { "failed" } else { "done" },
      "summary": format!("Finished {}", result.step),
    });
    if result.handoff {
      report["completed"] = json!(["Appended hello"]);
      report["decisions"] = json!([format!("Greet in README.md from {}", result.step)]);
      report["open_questions"] = json!(null);
      report["next_agent_should_know"] = json!(["README.md ends with hello"]);
    }
    report.to_string()
  } else {
    "Done.".to_owned()
  };
  message(session, "item_3", &text);
  complete(session, None);
  Step::Exit(ExitCode::SUCCESS)
}

/// A message the agent wrote, as a completed `agentMessage` item.
pub fn message(session: &Session, id: &str, text: &str) {
  notify(
    session,
    "item/completed",
    json!({"item": {"id": id, "type": "agentMessage", "text": text}}),
  );
}

/// "Ask first": a draft, then a question as the step result.
fn ask(session: &Session) -> Step {
  if let Err(error) = append(&session.cwd, "draft") {
    eprintln!("cannot write README.md: {error}");
    return Step::Exit(ExitCode::from(2));
  }
  let question = json!({"outcome": "question", "summary": "Keep email sign-in?"}).to_string();
  message(session, "item_q", &question);
  complete(session, None);
  Step::Exit(ExitCode::SUCCESS)
}

/// "Wait for release" and "Work slowly": what happens before the agent finishes its work.
fn before_finishing(session: &Session, prompt: &str) {
  if prompt.contains("Wait for release") {
    wait_for_release(session, prompt.contains("Draft first"));
    if prompt.contains("Then keep working") {
      wait_to_be_ended();
    }
  }
  if prompt.contains("Work slowly") {
    thread::sleep(Duration::from_millis(1_500));
  }
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

/// A command that runs until the test releases it.
fn wait_for_release(session: &Session, draft: bool) {
  let worktree = &session.cwd;
  if draft && let Err(error) = append(worktree, "draft") {
    eprintln!("cannot write README.md: {error}");
  }
  notify(
    session,
    "item/started",
    json!({"item": {"id": "item_9", "type": "commandExecution", "command": "pnpm test"}}),
  );
  let release = worktree
    .parent()
    .and_then(Path::parent)
    .map(|orchester| orchester.join("fake-release"));
  while !release.as_ref().is_some_and(|release| release.exists()) {
    thread::sleep(Duration::from_millis(50));
  }
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "item_9", "type": "commandExecution", "command": "pnpm test", "status": "completed", "exitCode": 0, "aggregatedOutput": "ok"}}),
  );
}

pub fn append(worktree: &Path, line: &str) -> std::io::Result<()> {
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

pub fn wait_to_be_ended() -> ! {
  thread::sleep(Duration::from_secs(600));
  std::process::exit(0)
}

/// What a turn with `prompt` reports when it finishes, unless the prompt asks for more.
pub fn report_of(prompt: &str, structured: bool) -> StepReport {
  StepReport {
    step: prompt
      .lines()
      .find_map(|line| line.strip_prefix("# Current step: "))
      .unwrap_or("the step")
      .to_owned(),
    structured,
    failed: false,
    own_file: false,
    handoff: false,
  }
}
