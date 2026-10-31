//! Issue #69: on Windows, Codex's sandbox helper failed its setup refresh before every command of a step started, and
//! the agent was told `helper_unknown_error: setup refresh had errors`. The App Server stream reports that once as a
//! typed problem naming Codex's code and stage, besides the failed command itself.

use orchester_core::connection::{
  app_server::{AppServerStream, Update},
  stream::{AgentEvent, CommandStatus, RunnerProblem},
};
use serde_json::json;

/// What Codex's elevated sandbox told the agent of issue #69 for each command it kept from starting.
const SETUP_REFRESH: &str = "helper_unknown_error: setup refresh had errors";

fn completed(stream: &mut AppServerStream, id: &str, status: &str, output: &str) -> Vec<AgentEvent> {
  let line = json!({"method": "item/completed", "params": {"item": {
    "id": id, "type": "commandExecution", "command": "git status", "status": status,
    "aggregatedOutput": output, "exitCode": null
  }}});
  stream
    .parse(&line.to_string())
    .expect("a command's end")
    .into_iter()
    .filter_map(|update| match update {
      Update::Activity(event) => Some(event),
      _ => None,
    })
    .collect()
}

fn setup_problems(events: &[AgentEvent]) -> Vec<&RunnerProblem> {
  events
    .iter()
    .filter_map(|event| match event {
      AgentEvent::RunnerProblem {
        problem: problem @ RunnerProblem::CodexSandboxSetupFailed { .. },
      } => Some(problem),
      _ => None,
    })
    .collect()
}

#[test]
fn codex_sandbox_setup_failure_is_told_once_with_its_code_and_stage() {
  let mut stream = AppServerStream::default();
  let first = completed(&mut stream, "command-1", "failed", &format!("{SETUP_REFRESH}\n"));
  assert!(
    matches!(
      &first[0],
      AgentEvent::CommandFinished { status: CommandStatus::Failed, output, .. } if output.contains(SETUP_REFRESH)
    ),
    "the command's own end comes first, as Codex told it: {first:?}"
  );
  assert_eq!(
    setup_problems(&first),
    [&RunnerProblem::CodexSandboxSetupFailed {
      code: "helper_unknown_error".to_owned(),
      message: "setup refresh had errors".to_owned(),
    }]
  );
  let wrapped = format!("execution error: {SETUP_REFRESH}");
  let second = completed(&mut stream, "command-2", "failed", &wrapped);
  assert_eq!(
    second.len(),
    1,
    "every later command fails the same way; told once: {second:?}"
  );
}

#[test]
fn codex_sandbox_setup_failure_is_found_only_in_a_command_that_did_not_complete() {
  let mut stream = AppServerStream::default();
  let searched = format!("crates/core/tests/codex_sandbox_setup.rs: {SETUP_REFRESH}");
  let found = completed(&mut stream, "grep", "completed", &searched);
  assert!(
    setup_problems(&found).is_empty(),
    "a command that ran printed it: {found:?}"
  );
  let declined = completed(&mut stream, "declined", "declined", SETUP_REFRESH);
  assert_eq!(setup_problems(&declined).len(), 1, "{declined:?}");
}

#[test]
fn codex_sandbox_setup_failure_needs_a_whole_helper_code_and_a_setup_stage() {
  let mut stream = AppServerStream::default();
  for output in [
    "my_helper_unknown_error: setup refresh had errors",
    "helper_: setup refresh had errors",
    "helper_unknown_error: the build failed",
    "helper_unknown_error setup refresh had errors",
    "error[E0425]: cannot find value `helper_setup` in this scope",
  ] {
    let events = completed(&mut stream, "command", "failed", output);
    assert!(setup_problems(&events).is_empty(), "{output}: {events:?}");
  }
}

/// A secret value in Codex's message is redacted with the whole event, as the runner redacts every event before it is
/// stored or shown.
#[test]
fn codex_sandbox_setup_failure_keeps_a_secret_value_out() {
  let mut stream = AppServerStream::default();
  let output = "helper_unknown_error: setup refresh had errors for sk-live-1234\nnext line";
  let mut events = completed(&mut stream, "command", "failed", output);
  for event in &mut events {
    event.redact(&|text| text.replace("sk-live-1234", "[redacted]"));
  }
  let [RunnerProblem::CodexSandboxSetupFailed { message, .. }] = setup_problems(&events)[..] else {
    panic!("one setup problem: {events:?}");
  };
  assert_eq!(message, "setup refresh had errors for [redacted]");
  assert!(!format!("{events:?}").contains("sk-live-1234"));
}
