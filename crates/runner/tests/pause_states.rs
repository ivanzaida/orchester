//! What Pause does in each state a run can be in (`docs/design/run-pause.md`), end to end: an agent that waits in its
//! turn for the answer to a question pauses at once (the bug of ORC-964: *Pausing…* never ended), a run whose agent
//! waits after its turn pauses at once, and a command tool's program runs to its end first, with the run still
//! running and the pause requested until then. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test pause_states pause_states`.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  question::{AskedQuestions, QuestionState},
  resource::StepKey,
  run::{PauseReason, Run, RunStatus, StepStatus},
  store::{ActivityStore, RunStore},
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, StepSpec, project, scheduler_with, tools::ToolSpec, wait_for};

fn step_status(run: &Run, step: &str) -> Option<StepStatus> {
  run
    .latest_execution(&StepKey::new(step).expect("key"))
    .map(|execution| execution.status())
}

async fn waiting_question(root: &std::path::Path, events: &mut mpsc::UnboundedReceiver<Event>, run: RunId) {
  tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if db.questions(run).expect("questions").iter().any(AskedQuestions::waits) {
        return;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("a question arrives");
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_states_a_question_in_the_turn_pauses_the_run_at_once_and_resume_continues() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::default()),
      NetworkAccess::Off,
    )),
    ..Setup::new("Ask with options, several allowed.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  waiting_question(root, &mut events, id).await;

  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("answered in time")
    .expect("the pause is requested");
  let paused = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  assert!(!paused.pause_requested());
  assert!(scheduler.active_runs().is_empty());
  // The question can no longer be answered: the agent that asked it ended, and it asks again if it still needs to.
  let db = ProjectDb::open(root).expect("open");
  let questions = db.questions(id).expect("questions");
  assert_eq!(questions.len(), 1);
  assert!(!questions[0].waits());
  assert!(
    matches!(questions[0].state, QuestionState::Interrupted { .. }),
    "{:?}",
    questions[0].state
  );

  scheduler.resume_run(id).await.expect("the run resumes");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_states_a_step_that_waits_for_a_message_pauses_the_run_at_once() {
  let project = project(&Setup {
    steps: vec![StepSpec::new("implement", "Implement", "Ask first.", &[])],
    ..Setup::new("Continue the existing work.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let waiting = wait_for(root, &mut events, id, Run::waits_for_a_person).await;
  assert!(matches!(
    step_status(&waiting, "implement"),
    Some(StepStatus::Waiting(_))
  ));

  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("answered in time")
    .expect("the run pauses");
  let paused = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_states_a_command_step_runs_to_its_end_before_the_run_pauses() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("checks", "Checks", "", &[]).command(ToolSpec::test_command(&["sleep", "4"]).timeout(900)),
      StepSpec::new("implement", "Implement", "Say hello.", &["checks"]),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let db = ProjectDb::open(root).expect("open");
  tokio::time::timeout(PATIENCE, async {
    while !db
      .activity(id, 0)
      .expect("activity")
      .iter()
      .any(|entry| matches!(entry.event, AgentEvent::ToolCommandStarted { .. }))
    {
      events.recv().await.expect("the scheduler notifies");
    }
  })
  .await
  .expect("the command starts in time");

  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("answered in time")
    .expect("the pause is requested");
  tokio::time::sleep(Duration::from_millis(500)).await;
  let pausing = db.run(id).expect("read").expect("run");
  // Pausing is not paused: the program runs to its end first, and nothing after it starts.
  assert_eq!(pausing.status(), &RunStatus::Running);
  assert!(pausing.pause_requested());
  assert_eq!(step_status(&pausing, "checks"), Some(StepStatus::Running));

  let paused = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(step_status(&paused, "checks"), Some(StepStatus::Passed));
  assert_eq!(step_status(&paused, "implement"), Some(StepStatus::Pending));
}
