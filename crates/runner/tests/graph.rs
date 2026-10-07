//! Pipeline graphs executed end to end against `orchester-fake-codex`: parallel steps in worktrees
//! of their own merged back into the run's branch, merge conflicts restarting their step, reported
//! failures taking failure routes, and the outputs of earlier steps in later prompts. Step budgets are
//! in `step_budget.rs`.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test graph graph`.

mod support;

use std::fs;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::IdempotencyKey,
  resource::StepKey,
  run::{AttemptReason, Run, RunStatus, StepFailure, StepOutcome},
  store::ActivityStore,
};
use orchester_ipc::{AttachSession, SessionEnd};
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, git, project, reported, scheduler, settled, wait_for};

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

fn problems(db: &ProjectDb, run: &Run) -> Vec<RunnerProblem> {
  db.activity(run.id(), 0)
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

#[tokio::test(flavor = "multi_thread")]
async fn graph_parallel_steps_work_in_their_own_worktrees_and_merge_back() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("checks", "Checks", "Write own file.", &["implement"]),
      StepSpec::new("review", "Review", "Write own file.", &["implement"]),
    ],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let branch = run.branch().as_str();
  for file in ["implement.txt", "checks.txt", "review.txt"] {
    assert_eq!(git(root, &["show", &format!("{branch}:{file}")]), "hello", "{file}");
  }
  // The parallel steps worked beside each other in worktrees of their own, which are gone now,
  // and so are their branches: their commits are on the run's branch.
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, run.id(), "argv: ");
  assert_eq!(argv.len(), 3);
  let run_worktree = run.id().to_string();
  let beside: Vec<&String> = argv.iter().filter(|argv| !argv.contains(&run_worktree)).collect();
  assert_eq!(beside.len(), 2, "{argv:?}");
  assert_eq!(git(root, &["branch", "--list", &format!("{branch}--*")]), "");
  let worktrees = root.join(".orchester").join("worktrees");
  let left: Vec<_> = fs::read_dir(&worktrees).expect("worktrees").collect();
  assert_eq!(left.len(), 1, "only the run's worktree stays");
  // The last checkpoint is the run's branch as the last merge left it.
  let head = git(root, &["rev-parse", branch]);
  assert_eq!(
    run.last_checkpoint().map(|checkpoint| checkpoint.revision().as_str()),
    Some(head.as_str())
  );

  // Each step's structured result is its output, and the steps after Implement read it with its
  // source in their prompts.
  let outputs = db.step_outputs(run.id()).expect("outputs");
  assert_eq!(outputs.len(), 3);
  assert!(outputs.iter().all(|output| output.outcome == Some(StepOutcome::Done)));
  let implement = outputs
    .iter()
    .find(|output| output.step == key("implement"))
    .expect("implement");
  assert_eq!(implement.text, "Finished Implement");
  let prompts = reported(&db, run.id(), "prompt| ");
  assert!(
    prompts
      .iter()
      .any(|line| line == "Sources: Implement (`implement`, attempt 1)"),
    "{prompts:?}"
  );
  // Once each: the latest handoff in task memory does not repeat it.
  assert_eq!(
    prompts
      .iter()
      .filter(|line| line.contains("Finished Implement"))
      .count(),
    2
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn graph_merge_conflict_restarts_the_step_instead_of_taking_its_route() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("checks", "Checks", "Sign the line.", &["implement"]).retry("implement", 1),
      StepSpec::new("review", "Review", "Sign the line.", &["implement"]).retry("implement", 1),
    ],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  let root = project.repo.path();
  // A merge conflict is no verdict on the work: the step that conflicted restarts by itself, now
  // alone on the run's branch, and passes. Implement, the routes' target, never runs again.
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.attempts().len(), 2);
  assert!(
    matches!(run.attempts()[1].reason(), AttemptReason::Restart { .. }),
    "{:?}",
    run.attempts()
  );
  assert_eq!(
    run.attempts()[1].executions().len(),
    1,
    "only the conflicting step runs again"
  );
  assert_eq!(run.retries_taken(&key("checks")) + run.retries_taken(&key("review")), 0);
  let failed = failures(&run);
  assert_eq!(failed.len(), 1);
  assert!(
    failed.iter().all(|(_, failure)| *failure == StepFailure::MergeConflict),
    "{failed:?}"
  );
  let db = ProjectDb::open(root).expect("open");
  let conflict = problems(&db, &run)
    .into_iter()
    .find_map(|problem| match problem {
      RunnerProblem::MergeConflict { branch, files } => Some((branch, files)),
      _ => None,
    })
    .expect("the conflict is reported");
  assert_eq!(conflict.1, ["README.md"]);
  // The conflicting work stays on the step's own branch; the run's branch is clean.
  let subject = git(root, &["log", "-1", "--format=%s", &conflict.0]);
  assert!(
    subject.starts_with("ORC-1: ") && subject.ends_with("(run #1)"),
    "{subject}"
  );
  let run_worktree = root.join(".orchester").join("worktrees").join(run.id().to_string());
  assert_eq!(git(&run_worktree, &["status", "--porcelain"]), "");
}

#[tokio::test(flavor = "multi_thread")]
async fn graph_a_reported_failure_retries_and_then_passes() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Change README.md.", &[]),
      StepSpec::new("review", "Review", "Fail once.", &["implement"]).retry("implement", 2),
    ],
    ..Setup::new("Say hello.")
  };
  let (project, run) = run_to_the_end(&setup).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.attempts().len(), 2);
  assert_eq!(run.retries_taken(&key("review")), 1);
  assert_eq!(failures(&run), [("review".to_owned(), StepFailure::ReportedFailed)]);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let outputs = db.step_outputs(run.id()).expect("outputs");
  let review: Vec<Option<StepOutcome>> = outputs
    .iter()
    .filter(|output| output.step == key("review"))
    .map(|output| output.outcome)
    .collect();
  assert_eq!(review, [Some(StepOutcome::Failed), Some(StepOutcome::Done)]);
  // The retried Review reads the output of Implement's second attempt.
  let prompts = reported(&db, run.id(), "prompt| ");
  assert!(
    prompts
      .iter()
      .any(|line| line == "## Implement (`implement`, attempt 2)"),
    "{prompts:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn graph_agent_slots_bound_the_steps_that_run_at_once() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("checks", "Checks", "Write own file.", &["implement"]),
      StepSpec::new("review", "Review", "Write own file.", &["implement"]),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let one = std::num::NonZeroU32::new(1).expect("above zero");
  scheduler
    .attach(
      AttachSession::Fresh {
        previous_end: SessionEnd::Clean,
      },
      one,
    )
    .await
    .expect("attached");
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 3);
  for pair in invocations.windows(2) {
    let ended = pair[0].ended_at.expect("ended");
    assert!(pair[1].started_at >= ended, "one agent at a time");
  }
  // Checks and Review were both ready, so the first worked beside the other in a worktree of its
  // own; the second then ran alone in the run's worktree.
  let argv = reported(&db, id, "argv: ");
  let run_worktree = id.to_string();
  assert!(!argv[1].contains(&run_worktree), "{argv:?}");
  assert!(argv[2].contains(&run_worktree), "{argv:?}");
  for file in ["implement.txt", "checks.txt", "review.txt"] {
    assert_eq!(git(root, &["show", &format!("{}:{file}", run.branch())]), "hello");
  }
}

/// A step waiting for an agent slot starts once the slots are raised, beside the one at work.
#[tokio::test(flavor = "multi_thread")]
async fn graph_raised_agent_slots_start_a_step_waiting_for_one() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("checks", "Checks", "Keep working.", &["implement"]),
      StepSpec::new("review", "Review", "Keep working.", &["implement"]),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let slots = |count| std::num::NonZeroU32::new(count).expect("above zero");
  scheduler
    .attach(
      AttachSession::Fresh {
        previous_end: SessionEnd::Clean,
      },
      slots(1),
    )
    .await
    .expect("attached");
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let invocations = |count: usize| {
    move |_: &Run| {
      ProjectDb::open(root)
        .expect("open")
        .invocations(id)
        .expect("invocations")
        .len()
        == count
    }
  };
  wait_for(root, &mut events, id, invocations(2)).await;
  scheduler.set_agent_slots(slots(2)).await.expect("raised");
  wait_for(root, &mut events, id, invocations(3)).await;
  let db = ProjectDb::open(root).expect("open");
  let working = db
    .invocations(id)
    .expect("invocations")
    .iter()
    .filter(|invocation| invocation.ended_at.is_none())
    .count();
  assert_eq!(working, 2, "Checks and Review work at once");
  scheduler.stop_all(orchester_core::run::InterruptCause::QuitNow).await;
}
