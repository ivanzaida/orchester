//! What the Orchestrator's agent changes itself and what waits for the person (`docs/design/orchestrator-toolset.md`):
//! it reads another project and changes only its own; publishing a draft waits for the person's Publish, refused
//! for a draft saved again since, unless the person lets the Orchestrator publish (what widens what agents may do
//! needs *Let automation change agent permissions* either way); canceling a run waits for the person's Cancel run;
//! in auto mode a run starts at once.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{ActionOutcome, change::Change},
  run::RunStatus,
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card, runs},
};

/// Sends `call` and waits until its turn kept `count` actions and ended.
fn call(screen: &mut Orchestrator, call: &str, count: usize) {
  screen.send(call);
  screen.until("the call's action", |screen| outcomes(screen).len() == count);
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
}

#[test]
fn orchestrator_reads_another_project_and_changes_only_its_own() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let atlas = screen.another("Atlas");
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call list_tasks {"project":"Atlas"}"#);
  screen.until("the read's answer", |screen| {
    screen.text("orchestrator-conversation").contains("Tool said") && !screen.exists("orchestrator-thinking")
  });
  let answer = screen.text("orchestrator-conversation");
  assert!(
    answer.contains("\"tasks\"") && !answer.contains("other_project"),
    "another project reads: {answer}"
  );
  assert!(outcomes(&screen).is_empty(), "a read is no action");

  screen.send(r#"Call create_task {"project":"Atlas","title":"Not here"}"#);
  screen.until("the other project's refusal", |screen| {
    screen.text("orchestrator-conversation").contains("other_project") && !screen.exists("orchestrator-thinking")
  });
  let theirs = ProjectDb::open(&atlas).expect("Atlas").tasks().expect("tasks");
  assert!(theirs.is_empty(), "nothing lands in another project: {theirs:?}");
}

#[test]
fn orchestrator_publication_waits_for_the_person() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  // New instructions are not provably as strict: even the person's Publish needs this.
  screen.let_automation_widen(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#,
    1,
  );
  let ActionOutcome::ProposedChange(proposed) = outcomes(&screen).remove(0) else {
    panic!("a proposed publication: {:?}", outcomes(&screen));
  };
  assert!(matches!(proposed.change, Change::Publish { version: 2, .. }));
  assert_eq!(
    screen.pipeline_version(delivery),
    1,
    "nothing is published on the agent's word"
  );
  assert!(
    screen
      .text("orchestrator-conversation")
      .contains("waits for the person"),
    "the agent was told it waits"
  );
  let card = proposal_card(&screen);
  screen.until("the card", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Publish pipeline Delivery as v2?")
  });

  let publish = format!("{card}-start");
  with_synthetic_input(|| screen.click(&publish));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert!(
    outcomes(&screen)[0].is_proposed(),
    "automation's input publishes nothing"
  );

  screen.click(&publish);
  screen.until("the publication", |screen| outcomes(screen)[0] == ActionOutcome::Done);
  assert_eq!(
    screen.pipeline_version(delivery),
    2,
    "the person's Publish published it"
  );
  screen.until("the published card", |screen| {
    screen.text(&format!("{card}-title")).contains("Published a pipeline")
  });
}

#[test]
fn orchestrator_publication_refuses_a_draft_saved_after_it_was_proposed() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  // New instructions are not provably as strict: even the person's Publish needs this.
  screen.let_automation_widen(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#,
    1,
  );
  let card = proposal_card(&screen);
  // The draft changes after the person was asked: Publish must not publish what they did not read.
  screen.pipeline_draft((delivery, builder), "Delete the repository.");
  screen.until("its button", |screen| screen.exists(&format!("{card}-start")));
  screen.click(&format!("{card}-start"));
  screen.until("the refusal", |screen| {
    matches!(outcomes(screen).first(), Some(ActionOutcome::Refused { code, .. }) if code == "changed_since_proposed")
  });
  assert_eq!(screen.pipeline_version(delivery), 1, "nothing was published");
  screen.until("the reason on the card", |screen| {
    screen
      .text(&card)
      .contains("saved again or published after it was proposed")
  });
}

#[test]
fn orchestrator_publishes_itself_once_the_person_lets_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.let_automation_widen(true);
  screen.let_orchestrator_publish(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#,
    1,
  );
  assert_eq!(outcomes(&screen)[0], ActionOutcome::Done);
  assert_eq!(screen.pipeline_version(delivery), 2);
}

#[test]
fn orchestrator_starts_in_auto_mode_and_cancel_waits_for_the_person() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_with(delivery, "Keep going", ("Keep working.", &["It keeps working."]));
  screen.auto_mode(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    &format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#),
    1,
  );
  assert_eq!(outcomes(&screen)[0], ActionOutcome::Done, "auto mode starts it at once");
  assert_eq!(runs(&screen, &key), 1);

  let run = run_of(&screen, &key);
  call(
    &mut screen,
    &format!(r#"Call cancel_run {{"project":"Kontur","run":"{}"}}"#, run.id()),
    2,
  );
  assert!(
    matches!(&outcomes(&screen)[1], ActionOutcome::ProposedChange(change) if matches!(change.change, Change::Cancel { .. })),
    "{:?}",
    outcomes(&screen)
  );
  assert!(!run_of(&screen, &key).status().is_terminal(), "the run goes on");
  let card = proposal_card(&screen);
  screen.until("its button", |screen| screen.exists(&format!("{card}-start")));
  screen.click(&format!("{card}-start"));
  screen.until("the cancel", |screen| outcomes(screen)[1] == ActionOutcome::Done);
  screen.until("the canceled run", |screen| {
    *run_of(screen, &key).status() == RunStatus::Cancelled
  });
}

/// The latest run of task `key`.
fn run_of(screen: &Orchestrator, key: &str) -> orchester_core::run::Run {
  let db = screen.db();
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  db.runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("a readable run")
}
