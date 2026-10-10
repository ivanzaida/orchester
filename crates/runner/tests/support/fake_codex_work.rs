//! What a turn of `orchester-fake-codex` does, by its prompt:
//!
//! - "Please fail": fails the turn as Codex did for an unsupported model;
//! - "Report plan limits": first sends `account/rateLimits/updated`, then works as the rest of the prompt says;
//! - "Use Orchester's tools": first lists and calls Orchester's tools (`fake_codex_tools`), then works as below;
//! - "Keep working": starts a helper process of its own, reports its id (`helper pid: `) and waits until it is ended;
//! - "Work slowly": waits a moment, then works as below;
//! - "Wait for release": runs a command (`pnpm test`) that ends once the test writes `fake-release` next to the
//!   worktrees (`<repo>/.orchester/`), appending `draft` to `README.md` first when the prompt also says "Draft first";
//!   then it waits until it is ended when the prompt also says "Then keep working" (writing "Released." first when it
//!   also says "Note the release"), else works as below;
//! - "Wait for a message": runs a command (`pnpm test`) until a message sent now arrives (`turn/steer`), answers it,
//!   completes the command, reports the message as a `userMessage` item and writes "Noted: <message>", as Codex 0.159.2
//!   did in the probe of 2026-10-02; then works as below. "Refuse a message" refuses the steer instead ("steering is
//!   not available for this turn") and works as below. "Keep a message for the turn's end" accepts the steer, completes
//!   the command and the turn without a `userMessage` item, as Codex does with input it accepted but had not given the
//!   model when the turn finished (it records it into the thread then); "Answer the steer late" does the same but
//!   answers the steer only after `turn/completed`. "Interrupt after a message" accepts the steer and ends the turn
//!   `interrupted`, keeping the message back as Codex does; "Die after a message" accepts it and exits with 1. A steer
//!   while no turn waits for one is answered "no active turn to steer", as Codex answers it while no turn runs;
//! - "Report failure": works as below, but its result says it failed;
//! - "Leave a handoff": works as below, and its step result hands off (`completed`, `decisions`, `open_questions`,
//!   `next_agent_should_know`), with the decision "Greet in README.md from <step>";
//! - "Fail once": the first time a step works on it, the result says it failed; later it works as below;
//! - "Flood standard error": first writes Codex's coloured warning about unknown settings with the setting it names, a
//!   line of 5000 characters, 3995 `y` followed by the value of `DOCS_TOKEN` (an MCP server's secret, when the process
//!   has it) and 1100 numbered lines (`stderr line <n>`) to standard error, then works as below;
//! - "Write own file": writes `<step>.txt` instead of appending to `README.md`;
//! - "Report the absolute path": reports the file change at its absolute path in the worktree, as Codex does, instead
//!   of relative to it;
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

use super::{HELPER, Session, Step, TURN, complete, emit, fake_app_server, fake_codex_tools, notify};

/// The account's limits App Server reports during a turn.
const LIMITS: &str = include_str!("../../../core/tests/fixtures/codex/plan-limits.jsonl");

/// What an unsupported model's turn failed with.
const UNSUPPORTED: &str = "The 'no-such-model-probe' model is not supported when using Codex with a ChatGPT account.";

/// What the step's final message reports.
pub struct StepReport {
  step: String,
  structured: bool,
  failed: bool,
  own_file: bool,
  absolute_path: bool,
  handoff: bool,
}

/// A turn that waits for a message sent now.
pub struct AwaitsMessage {
  prompt: String,
  structured: bool,
  answer: SteerAnswer,
}

/// What the turn does with the message sent now.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SteerAnswer {
  /// Accepts it and gives it to the model after the running command.
  Take,
  /// Refuses it.
  Refuse,
  /// Accepts it, but the turn ends before the model reads it.
  KeepForTurnEnd,
  /// As `KeepForTurnEnd`, but the answer comes after `turn/completed`.
  AnswerLate,
  /// Accepts it, then the turn is interrupted.
  Interrupt,
  /// Accepts it, then the process exits.
  Die,
}

/// The answers to a message sent now that the prompt asks for, by the words that ask for them.
const ANSWERS: [(&str, SteerAnswer); 6] = [
  ("Refuse a message", SteerAnswer::Refuse),
  ("Keep a message for the turn's end", SteerAnswer::KeepForTurnEnd),
  ("Answer the steer late", SteerAnswer::AnswerLate),
  ("Interrupt after a message", SteerAnswer::Interrupt),
  ("Die after a message", SteerAnswer::Die),
  ("Wait for a message", SteerAnswer::Take),
];

pub fn work(session: &mut Session, prompt: &str, structured: bool) -> Step {
  if prompt.starts_with(super::fake_codex_cell::CONTINUE) {
    return super::fake_codex_cell::collect(session);
  }
  if prompt.contains(super::fake_codex_cell::TRIGGER) {
    return super::fake_codex_cell::start(session, prompt, structured);
  }
  let worktree = session.cwd.clone();
  let answer = ANSWERS
    .iter()
    .find(|(words, _)| prompt.contains(words))
    .map(|(_, answer)| *answer);
  if let Some(answer) = answer.filter(|_| !session.resumed) {
    notify(
      session,
      "item/started",
      json!({"item": {"id": "item_8", "type": "commandExecution", "command": "pnpm test"}}),
    );
    session.awaits_message = Some(AwaitsMessage {
      prompt: prompt.to_owned(),
      structured,
      answer,
    });
    return Step::Go;
  }
  if prompt.contains("Please fail") {
    notify(session, "error", json!({"error": {"message": UNSUPPORTED}}));
    complete(session, Some(UNSUPPORTED));
    return Step::Exit(ExitCode::SUCCESS);
  }
  if prompt.contains("Report plan limits") {
    match serde_json::from_str(LIMITS) {
      Ok(limits) => emit(&limits),
      Err(error) => eprintln!("the limits fixture is not JSON: {error}"),
    }
  }
  if [
    "Use Orchester's tools",
    "Use the file tools",
    "Use the run tools",
    "Run the check twice",
    "Use the index tools",
    "Use the skill tools",
  ]
  .iter()
  .any(|words| prompt.contains(words))
  {
    fake_codex_tools::report(session.orchester.as_mut(), &session.cwd, prompt);
  }
  if let Some(step) = fake_app_server::start(session, prompt, structured) {
    return step;
  }
  let mut result = report_of(prompt, structured);
  result.failed = prompt.contains("Report failure") || (prompt.contains("Fail once") && first_time(&worktree, prompt));
  result.own_file = prompt.contains("Write own file");
  result.absolute_path = prompt.contains("Report the absolute path");
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

/// A message sent now (`turn/steer` with `id`): taken after the running command, refused, accepted and
/// left for the turn's end (answered at once or late), or accepted before the turn is interrupted or
/// the process dies; then the turn works as usual.
pub fn steered(session: &mut Session, id: &serde_json::Value, params: &serde_json::Value) -> Step {
  let Some(awaits) = session.awaits_message.take() else {
    emit(&json!({"id": id, "error": {"code": -32600, "message": "no active turn to steer"}}));
    return Step::Go;
  };
  let text = params["input"][0]["text"].as_str().unwrap_or_default().to_owned();
  let accepted = json!({"id": id, "result": {"turnId": params["expectedTurnId"]}});
  match awaits.answer {
    SteerAnswer::Refuse => {
      emit(&json!({"id": id, "error": {"code": -32600, "message": "steering is not available for this turn"}}));
    }
    SteerAnswer::AnswerLate => {}
    SteerAnswer::Die => {
      emit(&accepted);
      return Step::Exit(ExitCode::from(1));
    }
    SteerAnswer::Interrupt => {
      emit(&accepted);
      notify(
        session,
        "turn/completed",
        json!({"turn": {"id": TURN, "status": "interrupted"}}),
      );
      return Step::Go;
    }
    SteerAnswer::Take | SteerAnswer::KeepForTurnEnd => emit(&accepted),
  }
  notify(
    session,
    "item/completed",
    json!({"item": {"id": "item_8", "type": "commandExecution", "command": "pnpm test", "status": "completed", "exitCode": 0, "aggregatedOutput": "ok"}}),
  );
  if awaits.answer == SteerAnswer::Take {
    notify(
      session,
      "item/completed",
      json!({"item": {"id": "item_7", "type": "userMessage", "content": [{"type": "text", "text": text, "text_elements": []}]}}),
    );
    message(session, "item_6", &format!("Noted: {text}"));
  }
  let result = report_of(&awaits.prompt, awaits.structured);
  let step = finish(session, "hello", &result);
  if awaits.answer == SteerAnswer::AnswerLate {
    emit(&accepted);
  }
  step
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
pub fn finish(session: &mut Session, line: &str, result: &StepReport) -> Step {
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
  let reported = if result.absolute_path {
    worktree.join(&file).display().to_string()
  } else {
    file
  };
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
    json!({"item": {"id": "item_2", "type": "fileChange", "status": "completed", "changes": [{"path": reported, "kind": {"type": "update"}}]}}),
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
  session.completed_result = Some(text);
  complete(session, None);
  Step::Go
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
      if prompt.contains("Note the release") {
        message(session, "item_r", "Released.");
      }
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
    absolute_path: false,
    handoff: false,
  }
}
