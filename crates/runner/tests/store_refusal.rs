//! A run whose task fails on the runner's own side: the project database refuses a write the step's end
//! needs. The run does not stay running with nothing behind it: it is interrupted for the runner's
//! failure, from what was stored, and the error is in its activity. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test store_refusal store_refusal`.

mod support;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::IdempotencyKey,
  run::{InterruptCause, RunStatus},
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use self::support::{Setup, project, scheduler, wait_for};

/// Makes the project database refuse every step output, as a broken or full database would.
const REFUSE_OUTPUTS: &str = "CREATE TRIGGER refuse_outputs BEFORE INSERT ON step_outputs \
                              BEGIN SELECT RAISE(ABORT, 'refused by the test'); END";

#[tokio::test(flavor = "multi_thread")]
async fn store_refusal_interrupts_the_run_and_reports_why() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let raw = rusqlite::Connection::open(root.join(".orchester").join("orchester.db")).expect("open");
  raw.execute(REFUSE_OUTPUTS, []).expect("the trigger");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  let reported = ProjectDb::open(root)
    .expect("open")
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .any(|entry| {
      matches!(entry.event, AgentEvent::RunnerProblem {
        problem: RunnerProblem::RunnerFailed { detail },
      } if detail.contains("refused by the test"))
    });
  assert!(reported, "the run's activity says why it stopped");
}
