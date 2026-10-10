//! Step budgets end to end against `orchester-fake-codex`: a step that uses its budget stops the
//! run at its limit ("Stopped at limit", step budget) without failing or taking a retry, and
//! raising that step's budget for the run continues the stopped agent's CLI session. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test step_budget step_budget`.

mod support;

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  },
  id::IdempotencyKey,
  resource::StepKey,
  run::{PauseReason, RaisedLimits, Run, RunLimit, RunStatus, StepStatus},
  store::ActivityStore,
  tokens::TokenBudget,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, git, project, reported, scheduler, wait_for};

/// What one turn of the fake CLI counts: 12,166 input − 2,432 cached + 5 output.
const TURN: u64 = 9_739;

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

fn tokens(count: u64) -> TokenBudget {
  TokenBudget::new(count).expect("above zero")
}

fn status_of(run: &Run, step: &str) -> Option<StepStatus> {
  run.latest_execution(&key(step)).map(|execution| execution.status())
}

/// The limit a run stopped at the budget of `step` names.
fn step_limit(run: &Run, step: &str) -> RunLimit {
  RunLimit::StepBudget {
    execution: run.latest_execution(&key(step)).expect("execution").id(),
  }
}

fn stopped(run: &Run) -> bool {
  matches!(run.status(), RunStatus::Paused(_))
}

#[tokio::test(flavor = "multi_thread")]
async fn step_budget_stops_the_run_at_its_limit_and_a_raise_continues_the_session() {
  let project = project(&Setup {
    steps: vec![StepSpec::new("implement", "Implement", "Work in turns.", &[]).budget(1_000)],
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, stopped).await;
  let limit = step_limit(&run, "implement");
  assert_eq!(run.status(), &RunStatus::Paused(PauseReason::LimitReached(limit)));
  // The step is not failed and no retry is taken: it waits, open, for a raised budget.
  assert_eq!(status_of(&run, "implement"), Some(StepStatus::Running));
  let implement = run.latest_execution(&key("implement")).expect("execution");
  assert_eq!(implement.failure(), None);
  assert_eq!(run.attempts().len(), 1);
  let db = ProjectDb::open(root).expect("open");
  let problems: Vec<RunnerProblem> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(problem),
      _ => None,
    })
    .collect();
  assert_eq!(problems, [RunnerProblem::StepBudgetReached { budget: 1_000 }]);
  // The agent's work up to its budget is a checkpoint on the run's branch.
  let checkpoint = run.last_checkpoint().expect("the work was committed");
  assert_eq!(
    git(
      root,
      &["show", &format!("{}:README.md", checkpoint.revision().as_str())]
    ),
    "hi\ndraft"
  );
  assert!(scheduler.active_runs().is_empty());

  // Only a raised step budget above what the step used continues it; there is no maximum.
  assert_eq!(scheduler.resume_run(id).await, Err(Rejection::LimitNotRaised { limit }));
  let run_limits = RaisedLimits {
    budget: Some(tokens(2_000_000)),
    max_duration: run.max_duration(),
  };
  assert_eq!(
    scheduler.raise_limit(id, run_limits).await,
    Err(Rejection::RunNotPaused)
  );
  assert_eq!(
    scheduler.raise_step_budget(id, Some(tokens(TURN))).await,
    Err(Rejection::LimitNotAboveUsed { limit })
  );

  scheduler
    .raise_step_budget(id, Some(tokens(3 * TURN)))
    .await
    .expect("the step budget is raised");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.attempts().len(), 1);
  assert_eq!(run.step_budget(&key("implement")), Some(tokens(3 * TURN)));
  // The stopped agent's session continued in a second invocation of the same execution.
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[0].execution, invocations[1].execution);
  let session = invocations[0].session.clone().expect("the session");
  assert_eq!(invocations[1].session.as_deref(), Some(session.as_str()));
  let argv = reported(&db, id, "argv: ");
  assert!(argv[1].ends_with(&format!("--resume {session}")), "{argv:?}");
  assert_eq!(
    git(root, &["show", &format!("{}:README.md", run.branch())]),
    "hi\ndraft\ncontinued"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn step_budget_lets_a_parallel_step_finish_and_takes_no_retry() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("review", "Review", "Work in turns.", &["implement"])
        .retry("implement", 1)
        .budget(1_000),
      StepSpec::new("checks", "Checks", "Work slowly. Write own file.", &["implement"]),
    ],
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, stopped).await;
  let limit = step_limit(&run, "review");
  assert_eq!(run.status(), &RunStatus::Paused(PauseReason::LimitReached(limit)));
  assert_eq!(status_of(&run, "review"), Some(StepStatus::Running));
  // Checks was not ended with Review: it finished its invocation, like at a pause, and passed.
  assert_eq!(status_of(&run, "checks"), Some(StepStatus::Passed));
  let db = ProjectDb::open(root).expect("open");
  let checks = run.latest_execution(&key("checks")).expect("execution").id();
  let invocations = db.invocations(id).expect("invocations");
  let checks_invocation = invocations
    .iter()
    .find(|invocation| invocation.execution == checks)
    .expect("checks ran");
  assert_eq!(checks_invocation.outcome, Some(InvocationOutcome::Completed));
  assert_eq!(run.attempts().len(), 1);
  assert_eq!(run.retries_taken(&key("review")), 0);

  scheduler
    .raise_step_budget(id, Some(tokens(3 * TURN)))
    .await
    .expect("the step budget is raised");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.attempts().len(), 1);
  assert_eq!(run.retries_taken(&key("review")), 0);
  let branch = run.branch().as_str();
  assert_eq!(
    git(root, &["show", &format!("{branch}:README.md")]),
    "hi\ndraft\ncontinued"
  );
  assert_eq!(git(root, &["show", &format!("{branch}:checks.txt")]), "hello");
}
