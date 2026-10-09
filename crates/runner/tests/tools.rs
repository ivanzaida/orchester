//! Tool steps running command tools (M10a), end to end with `orchester-test-command` (see
//! `tests/support/test_command.rs`) beside agent steps on `orchester-fake-codex`: the exit code
//! decides the step, the output is recorded in the transcript and kept within the tool's limit,
//! the timeout, a cancel and the run's time limit end the program with everything it started, and
//! failure routes retry as for agent steps. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test tools tools`.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, RunnerProblem, ToolCommandResult},
  id::{IdempotencyKey, RunId},
  resource::StepKey,
  run::{InvocationKind, PauseReason, Run, RunFailure, RunLimit, RunStatus, StepFailure, StepOutcome, Stuck},
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use self::support::{
  Setup, StepSpec, git, is_running, project, reported, scheduler, settled, tools::ToolSpec, wait_for,
};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

fn failures(run: &Run) -> Vec<(String, StepFailure)> {
  run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .filter_map(|execution| Some((execution.step().to_string(), execution.failure()?)))
    .collect()
}

/// What the transcript says about each command that finished: its result, exit code, kept output
/// and the bytes before it that were not kept.
fn finished_commands(db: &ProjectDb, run: RunId) -> Vec<(ToolCommandResult, Option<i64>, String, u64)> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::ToolCommandFinished {
        result,
        exit_code,
        output,
        omitted_bytes,
        ..
      } => Some((result, exit_code, output, omitted_bytes)),
      _ => None,
    })
    .collect()
}

fn problems(db: &ProjectDb, run: RunId) -> Vec<RunnerProblem> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(problem),
      _ => None,
    })
    .collect()
}

async fn run_to_the_end(setup: &Setup) -> (support::TestProject, Run) {
  let project = project(setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, settled).await;
  (project, run)
}

/// Implement on the fake Codex, then Checks (a command tool) beside Review, then Deliver.
fn delivery(checks: ToolSpec) -> Setup {
  Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("checks", "Checks", "", &["implement"])
        .command(checks)
        .retry("implement", 1),
      StepSpec::new("review", "Review", "Write own file.", &["implement"]),
      StepSpec::new("deliver", "Deliver", "Write own file.", &["checks", "review"]),
    ],
    ..Setup::new("Say hello.")
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_command_step_passes_beside_an_agent_and_its_output_reaches_later_prompts() {
  let checks = ToolSpec::test_command(&["write", "checks.txt", "checked", "print", "3", "stderr", "warned"]);
  let (project, run) = run_to_the_end(&delivery(checks)).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  // The command worked beside Review in a worktree of its own, merged back into the run's branch.
  assert_eq!(
    git(root, &["show", &format!("{}:checks.txt", run.branch().as_str())]),
    "checked"
  );
  let db = ProjectDb::open(root).expect("open");
  let commands = finished_commands(&db, run.id());
  assert_eq!(commands.len(), 1);
  let (result, exit_code, output, omitted) = &commands[0];
  assert_eq!((*result, *exit_code, *omitted), (ToolCommandResult::Passed, Some(0), 0));
  // Both streams, each in its order; how they interleave depends on when each pipe is read.
  let printed: Vec<&str> = output.lines().filter(|line| line.starts_with("line ")).collect();
  assert_eq!(printed, ["line 1", "line 2", "line 3"], "{output}");
  assert!(output.lines().any(|line| line == "warned"), "{output}");
  let invocations = db.invocations(run.id()).expect("invocations");
  let command = invocations
    .iter()
    .find(|invocation| invocation.kind == InvocationKind::Command)
    .expect("the command's invocation");
  assert_eq!(command.outcome, Some(InvocationOutcome::Completed));
  assert!(command.ended_at.is_some());
  // Its summary is the step's output, which Deliver's prompt reads with its source.
  let outputs = db.step_outputs(run.id()).expect("outputs");
  let checks = outputs
    .iter()
    .find(|output| output.step == key("checks"))
    .expect("checks");
  assert_eq!(checks.outcome, Some(StepOutcome::Done));
  assert!(checks.text.starts_with("Checks ran `"), "{}", checks.text);
  assert!(checks.text.contains("exited with code 0"), "{}", checks.text);
  let prompts = reported(&db, run.id(), "prompt| ");
  assert!(
    prompts.iter().any(|line| line.contains("Checks (`checks`, attempt 1)")),
    "{prompts:?}"
  );
  assert!(prompts.iter().any(|line| line == "line 3"), "{prompts:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_command_step_fails_by_its_exit_code_and_retries_through_its_route() {
  // The marker lies next to the worktrees (`<repo>/.orchester/`): the retry works in a new one.
  let checks = ToolSpec::test_command(&["fail-once", "../../checks-failed-once"]);
  let (project, run) = run_to_the_end(&delivery(checks)).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(failures(&run), [("checks".to_owned(), StepFailure::CommandFailed)]);
  assert_eq!(run.attempts().len(), 2);
  // A failing check is a verdict: Implement, the route's target, reads how its command ended and
  // what it printed.
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let prompts = reported(&db, run.id(), "prompt| ");
  assert!(
    prompts
      .iter()
      .any(|line| line == "# Returned from Checks (`checks`), attempt 1"),
    "{prompts:?}"
  );
  assert!(
    prompts
      .iter()
      .any(|line| line.starts_with("> `") && line.ends_with("exited with code 1, which the tool does not expect.")),
    "{prompts:?}"
  );
  assert!(prompts.iter().any(|line| line.starts_with("> pid: ")), "{prompts:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_command_step_without_retries_left_fails_the_run() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("checks", "Checks", "", &[]).command(ToolSpec::test_command(&["print", "1", "exit", "3"])),
    ],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  assert_eq!(run.status(), &RunStatus::Failed(RunFailure::StepFailed(key("checks"))));
  assert_eq!(failures(&run), [("checks".to_owned(), StepFailure::CommandFailed)]);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let commands = finished_commands(&db, run.id());
  assert_eq!(commands[0].0, ToolCommandResult::Failed);
  assert_eq!(commands[0].1, Some(3));
  let outputs = db.step_outputs(run.id()).expect("outputs");
  assert_eq!(outputs[0].outcome, Some(StepOutcome::Failed));
  assert!(
    outputs[0].text.contains("a code the tool does not expect"),
    "{}",
    outputs[0].text
  );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
  not(windows),
  ignore = "only Windows ends a program's whole process tree yet (AGENTS.md, known gaps); the sleeper it started             would keep its output open"
)]
async fn tools_timeout_ends_the_program_with_everything_it_started() {
  let sleeper = ToolSpec::test_command(&["spawn-sleeper", "sleep", "600"]).timeout(2);
  let setup = Setup {
    steps: vec![StepSpec::new("checks", "Checks", "", &[]).command(sleeper)],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  assert_eq!(failures(&run), [("checks".to_owned(), StepFailure::CommandFailed)]);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let commands = finished_commands(&db, run.id());
  let (result, exit_code, output, _) = &commands[0];
  assert_eq!((*result, *exit_code), (ToolCommandResult::TimedOut, None));
  let invocation = &db.invocations(run.id()).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::TimedOut));
  let child = output
    .lines()
    .find_map(|line| line.strip_prefix("child pid: "))
    .expect("the program reported its child");
  assert!(!is_running(child), "the child {child} still runs");
  let parent = output.lines().find_map(|line| line.strip_prefix("pid: ")).expect("pid");
  assert!(!is_running(parent));
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_output_beyond_the_limit_is_left_out_and_counted() {
  let loud = ToolSpec::test_command(&["print", "5000"]).output_kib(1);
  let setup = Setup {
    steps: vec![StepSpec::new("checks", "Checks", "", &[]).command(loud)],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let (_, _, output, omitted) = finished_commands(&db, run.id()).remove(0);
  assert!(output.len() < 1024, "{}", output.len());
  assert!(output.ends_with("line 5000"), "{output}");
  assert!(!output.contains("line 1\n"));
  assert!(omitted > 40_000, "{omitted}");
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_cancel_ends_the_running_program() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("checks", "Checks", "", &[]).command(ToolSpec::test_command(&["sleep", "600"]).timeout(900)),
    ],
    ..Setup::new("Say hello.")
  };
  let project = project(&setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let db = ProjectDb::open(root).expect("open");
  let started = async {
    while !db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(entry.event, AgentEvent::ToolCommandStarted { .. }))
    {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(support::PATIENCE, started)
    .await
    .expect("the command starts in time");
  scheduler.cancel_run(id).await.expect("the run is cancelled");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  let commands = finished_commands(&db, id);
  assert_eq!(commands[0].0, ToolCommandResult::Stopped);
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Terminated));
  assert!(db.step_outputs(id).expect("outputs").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_the_run_time_limit_ends_the_program_and_stops_the_run() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("checks", "Checks", "", &[]).command(ToolSpec::test_command(&["sleep", "600"]).timeout(900)),
    ],
    max_duration: Some(Duration::from_secs(2)),
    ..Setup::new("Say hello.")
  };
  let project = project(&setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    run.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Time))
  );
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(finished_commands(&db, id)[0].0, ToolCommandResult::Stopped);
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_a_program_that_is_not_found_does_not_start() {
  let missing = ToolSpec::test_command(&[]).program("orchester-no-such-program");
  let setup = Setup {
    steps: vec![StepSpec::new("checks", "Checks", "", &[]).command(missing)],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  // A program that cannot start is a breakage, not a verdict: the step restarts once, then the run
  // waits for a person.
  assert_eq!(
    failures(&run),
    [
      ("checks".to_owned(), StepFailure::NotStarted),
      ("checks".to_owned(), StepFailure::NotStarted)
    ]
  );
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::BrokeAgain(key("checks")))
  );
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let not_found = RunnerProblem::ProgramNotFound {
    program: "orchester-no-such-program".into(),
  };
  assert_eq!(problems(&db, run.id()), [not_found.clone(), not_found]);
}

#[tokio::test(flavor = "multi_thread")]
async fn tools_a_program_on_the_path_runs_by_its_name() {
  let git_version = ToolSpec::test_command(&["--version"]).program("git");
  let setup = Setup {
    steps: vec![StepSpec::new("checks", "Checks", "", &[]).command(git_version)],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let (_, exit_code, output, _) = finished_commands(&db, run.id()).remove(0);
  assert_eq!(exit_code, Some(0));
  assert!(output.starts_with("git version"), "{output}");
}
