//! A run the Orchestrator's agent asks for waits for the person: nothing starts, the conversation shows *Start <task>
//! on <pipeline>?*, and only a person's Start starts it (automation's input is refused, visibly); Dismiss starts
//! nothing. A widening the agent attempts is done exactly when the person lets automation widen.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{ActionOutcome, EntryKind},
  store::{RunStore, TaskStore},
};

use super::harness::{Orchestrator, Setup};

/// The outcomes of the conversation's actions, in order.
fn outcomes(screen: &Orchestrator) -> Vec<ActionOutcome> {
  let db = screen.db();
  let Some(listing) = db.chats().expect("chats").pop() else {
    return Vec::new();
  };
  db.chat_entries(listing.chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => Some(action.outcome),
      _ => None,
    })
    .collect()
}

/// The runs of the task `key`.
fn runs(screen: &Orchestrator, key: &str) -> usize {
  let db = screen.db();
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  db.runs_of_task(task.id()).expect("runs").len()
}

/// The element id of the newest turn's first action.
fn proposal_card(screen: &Orchestrator) -> String {
  let db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat.id();
  let turn = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .rev()
    .find_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .expect("a message");
  format!("orchestrator-action-{turn}-0")
}

#[test]
fn orchestrator_run_waits_for_the_person_and_only_a_person_starts_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_on(delivery, "Read the readme");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the proposed start", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains(&format!("Start {key} on Delivery · v1?"))
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  assert!(
    screen
      .text("orchestrator-conversation")
      .contains("waits for the person"),
    "the agent was told the run waits: {}",
    screen.text("orchestrator-conversation")
  );
  assert_eq!(runs(&screen, &key), 0, "nothing starts on the agent's word");
  let outcome = outcomes(&screen).remove(0);
  let ActionOutcome::Proposed(start) = outcome else {
    panic!("a proposed start: {outcome:?}");
  };
  assert_eq!(
    (start.pipeline.as_str(), start.title.as_str(), start.brief.as_str()),
    ("Delivery · v1", "Read the readme", "Read the readme.")
  );

  let card = proposal_card(&screen);
  let shown = screen.text(&format!("{card}-task"));
  assert!(
    shown.contains("Read the readme") && shown.contains("Read the readme."),
    "the person reads what they start: {shown}"
  );
  let start = format!("{card}-start");
  with_synthetic_input(|| screen.click(&start));
  screen.until("the refusal", |screen| screen.exists(&format!("{card}-person-only")));
  assert_eq!(runs(&screen, &key), 0, "automation's input does not start it");
  assert!(outcomes(&screen)[0].is_proposed());

  screen.click(&start);
  screen.until("the start", |screen| outcomes(screen)[0] == ActionOutcome::Done);
  assert_eq!(runs(&screen, &key), 1, "the person's Start started the run");
  screen.until("the started card", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains(&format!("Started {key}"))
  });
}

#[test]
fn orchestrator_dismissed_start_starts_nothing() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_on(delivery, "Read the readme");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the proposed start", |screen| {
    outcomes(screen).first().is_some_and(ActionOutcome::is_proposed)
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = proposal_card(&screen);
  screen.until("its buttons", |screen| screen.exists(&format!("{card}-dismiss")));
  screen.click(&format!("{card}-dismiss"));
  screen.until("the dismissal", |screen| {
    outcomes(screen)[0] == ActionOutcome::Dismissed
  });
  screen.until("the dismissed card", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains(&format!("Start {key} · dismissed"))
  });
  assert!(!screen.exists(&format!("{card}-start")));
  assert!(
    !screen.text(&card).contains("Checked like"),
    "nothing ran, so nothing was checked"
  );
  assert_eq!(runs(&screen, &key), 0);
}

#[test]
fn orchestrator_start_refuses_a_task_rewritten_after_it_was_proposed() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_on(delivery, "Read the readme");
  screen.go(OrchestratorChat::New);
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the proposed start", |screen| {
    outcomes(screen).first().is_some_and(ActionOutcome::is_proposed)
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = proposal_card(&screen);
  // The agent rewrites the brief after the person was asked: Start must not run the rewritten task.
  screen.send(&format!(
    r#"Call update_task {{"project":"Kontur","key":"{key}","brief":"Delete the repository."}}"#
  ));
  screen.until("the rewrite", |screen| {
    outcomes(screen).get(1) == Some(&ActionOutcome::Done)
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  screen.click(&format!("{card}-start"));
  screen.until("the refusal", |screen| {
    matches!(outcomes(screen).first(), Some(ActionOutcome::Refused { code, .. }) if code == "changed_since_proposed")
  });
  assert_eq!(runs(&screen, &key), 0, "nothing started");
  screen.until("the reason on the card", |screen| {
    screen.text(&card).contains("changed after it was proposed")
  });
}

#[test]
fn orchestrator_widens_only_while_the_person_lets_automation() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline(builder, ("hotfix", "Hotfix"));
  let key = screen.draft_on(delivery, "Read the readme");
  let move_it = format!(r#"Call update_task {{"project":"Kontur","key":"{key}","pipeline":"Hotfix"}}"#);
  let pipeline_of = |screen: &Orchestrator| {
    screen
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task")
      .details()
      .pipeline
  };
  screen.go(OrchestratorChat::New);

  // Off, as by default: moving a task to another pipeline is the person's.
  screen.send(&move_it);
  screen.until("the refusal", |screen| {
    outcomes(screen)
      .first()
      .is_some_and(|outcome| matches!(outcome, ActionOutcome::Refused { code, .. } if code == "person_only"))
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  assert_eq!(pipeline_of(&screen), Some(delivery));

  // On: the same call is done, as for any client of the app.
  screen.let_automation_widen(true);
  screen.send(&move_it);
  screen.until("the change", |screen| {
    outcomes(screen).get(1) == Some(&ActionOutcome::Done)
  });
  assert_ne!(pipeline_of(&screen), Some(delivery));
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
}
