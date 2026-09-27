//! Tool steps running command tools: which tool steps the preflight accepts, how much output is
//! kept, and how the program's end decides the step and what it leaves for the steps after it.

use std::{
  collections::{BTreeMap, BTreeSet},
  time::Duration,
};

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, ToolCommandResult},
  id::{ResourceId, RunId, SecretId, StepExecutionId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AllowedSteps, ApprovalMode, CommandAction, CommandArg, CommandProgram, DatabaseQuery,
    DeniedBehavior, ExitCodes, OutputLimit, Pipeline, Step, StepKind, Tool, ToolAction, ToolKind,
  },
  run::{
    CommandEnd, OutputCapture, PreflightInput, PreflightProblem, StepAt, StepFailure, StepOutcome, ToolCommand,
    preflight,
  },
};

use crate::fixture::{Config, at, key, version};

fn command() -> CommandAction {
  CommandAction::new(
    CommandProgram::new("cargo").expect("valid program"),
    vec![CommandArg::new("test").expect("valid argument")],
  )
  .with_exit_codes(ExitCodes::new(BTreeSet::from([0, 1])).expect("valid codes"))
}

fn access() -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]),
    Name::new("Staging").expect("valid name"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("valid policy")
}

/// The sample pipeline with Run checks calling `tool`.
fn problems_with(tool: Option<Tool>) -> Vec<PreflightProblem> {
  let config = Config::feature_delivery();
  let checks = ResourceId::generate();
  let steps: Vec<Step> = config
    .pipeline_version
    .body()
    .steps()
    .iter()
    .cloned()
    .map(|mut step| {
      if step.key == key("checks") {
        step.kind = StepKind::Tool { tool: Some(checks) };
      }
      step.approval_gate = false;
      step
    })
    .collect();
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  let tools = tool
    .map(|tool| BTreeMap::from([(checks, version(1, tool))]))
    .unwrap_or_default();
  preflight(PreflightInput {
    pipeline: Some(&pipeline),
    profiles: &config.profiles,
    tools: &tools,
    connections: &[],
  })
  .into_iter()
  .filter(|problem| !matches!(problem, PreflightProblem::ConnectionMissing { .. }))
  .collect()
}

#[test]
fn tool_step_preflight_accepts_published_command_tools_only() {
  assert_eq!(problems_with(Some(Tool::command("", command()))), []);
  assert_eq!(
    problems_with(None),
    [PreflightProblem::ToolNotPublished { step: key("checks") }]
  );
  let database = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("query")),
    Some(access()),
  )
  .expect("valid tool");
  assert_eq!(
    problems_with(Some(database)),
    [PreflightProblem::ToolNotRunnable {
      step: key("checks"),
      kind: ToolKind::Database,
    }]
  );
  let guarded = Tool::new("", ToolAction::Command(command()), Some(access())).expect("valid tool");
  assert_eq!(
    problems_with(Some(guarded)),
    [PreflightProblem::ToolAccessRules { step: key("checks") }]
  );
}

#[test]
fn tool_step_output_capture_keeps_the_end_within_the_limit() {
  let limit = OutputLimit::from_kib(1).expect("valid limit");
  let mut capture = OutputCapture::new(limit);
  capture.push("first");
  capture.push("second");
  let kept = capture.clone().finish();
  assert_eq!((kept.text.as_str(), kept.omitted_bytes), ("first\nsecond", 0));
  let line = "x".repeat(99);
  for _ in 0..30 {
    capture.push(&line);
  }
  let kept = capture.finish();
  // 30 lines of 100 bytes with their breaks; ten fit in 1024 bytes.
  assert_eq!(kept.text.lines().count(), 10);
  assert_eq!(kept.omitted_bytes, 6 + 7 + 20 * 100);
  let mut long = OutputCapture::new(limit);
  long.push(&format!("{}é{}", "a".repeat(2000), "b".repeat(100)));
  let kept = long.finish();
  assert!(kept.text.ends_with(&"b".repeat(100)));
  assert!(kept.text.len() < 1024);
  assert_eq!(kept.omitted_bytes as usize + kept.text.len() + 1, 2000 + 2 + 100 + 1);
}

#[test]
fn tool_step_exit_code_decides_the_step() {
  let action = command();
  let run = ToolCommand {
    tool: "Run test suite",
    action: &action,
  };
  let passed = CommandEnd::Exited { code: Some(1) };
  let failed = CommandEnd::Exited { code: Some(101) };
  assert_eq!(run.result(passed), ToolCommandResult::Passed);
  assert_eq!(run.outcome(passed), InvocationOutcome::Completed);
  assert_eq!(run.failure(passed), None);
  assert_eq!(run.result(failed), ToolCommandResult::Failed);
  assert_eq!(run.outcome(failed), InvocationOutcome::ExitFailed { code: Some(101) });
  assert_eq!(run.failure(failed), Some(StepFailure::CommandFailed));
  assert_eq!(
    run.failure(CommandEnd::Exited { code: None }),
    Some(StepFailure::CommandFailed)
  );
  assert_eq!(run.outcome(CommandEnd::TimedOut), InvocationOutcome::TimedOut);
  assert_eq!(run.failure(CommandEnd::TimedOut), Some(StepFailure::CommandFailed));
  assert_eq!(run.outcome(CommandEnd::Stopped), InvocationOutcome::Terminated);
  assert_eq!(run.failure(CommandEnd::Stopped), None);
  assert_eq!(
    run.started(),
    AgentEvent::ToolCommandStarted {
      tool: "Run test suite".into(),
      command: "cargo test".into(),
    }
  );
}

#[test]
fn tool_step_output_names_the_command_its_end_and_the_last_lines() {
  let action = command();
  let run = ToolCommand {
    tool: "Run test suite",
    action: &action,
  };
  let mut capture = OutputCapture::new(OutputLimit::DEFAULT);
  for line in 1..=30 {
    capture.push(&format!("line {line}"));
  }
  let output = capture.finish();
  let at_step = StepAt {
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    step: key("checks"),
    now: at(5),
  };
  let failed = run
    .output(
      at_step.clone(),
      CommandEnd::Exited { code: Some(101) },
      Duration::from_millis(4200),
      &output,
    )
    .expect("a finished step leaves an output");
  assert_eq!(failed.outcome, Some(StepOutcome::Failed));
  assert!(
    failed
      .text
      .starts_with("Run test suite ran `cargo test`, which exited with code 101 after 4.2 s"),
    "{}",
    failed.text
  );
  assert!(failed.text.ends_with("line 11\nline 12\nline 13\nline 14\nline 15\nline 16\nline 17\nline 18\nline 19\nline 20\nline 21\nline 22\nline 23\nline 24\nline 25\nline 26\nline 27\nline 28\nline 29\nline 30"));
  assert!(!failed.text.contains("line 10\n"));
  let timed_out = run
    .output(at_step.clone(), CommandEnd::TimedOut, Duration::from_secs(600), &output)
    .expect("output");
  assert!(timed_out.text.contains("did not finish within its timeout of 600 s"));
  assert_eq!(run.output(at_step, CommandEnd::Stopped, Duration::ZERO, &output), None);
  let AgentEvent::ToolCommandFinished { result, exit_code, .. } =
    run.finished(CommandEnd::Exited { code: Some(0) }, &output)
  else {
    panic!("a finished command");
  };
  assert_eq!((result, exit_code), (ToolCommandResult::Passed, Some(0)));
}
