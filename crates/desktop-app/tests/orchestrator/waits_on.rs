//! The row of a run the Orchestrator made wait on a task (ORC-784; 04 `OKtX2` and `rlnEZ` a–e in `Orchestrator watch ·
//! redesign`), in the answer that made it wait, as the wait stands now: waiting, with *Drop the wait* under the
//! pointer; continued once the task was merged, with *Open ↗*; the task canceled, *still waits* in the warning's tone
//! with *Drop the wait* always; and dropped, *no longer waits*. Only the person drops a wait: automation's press is
//! refused and says why; the person's turns the row to dropped in place, with no turn.

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ResourceId},
  orchestrator::{ActionOutcome, Chat, ChatAction, TurnState, WaitsOnRow},
  resource::ModelId,
  run::{BranchName, Run, RunWaitsOn, WaitReason, WaitingRun},
  store::TaskStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunWaitEnd};

use super::{
  harness::{Orchestrator, Setup},
  history_rows::hover,
  standing::waiting,
};

/// The branch the waited tasks are to be merged into.
const BASE: &str = "module/webhooks-v2";

/// A run of a new task `title` on `delivery` waiting for a message, made to wait on a new task `waited`; the run and
/// that task's key.
fn waits_on(screen: &Orchestrator, delivery: ResourceId, (title, waited): (&str, &str)) -> (Run, String) {
  let (run, _) = waiting(screen, delivery, title);
  let key = screen.draft_on(delivery, waited);
  let mut db = screen.db();
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the waited task");
  let project = db.project().id();
  let waiting = WaitingRun {
    project,
    run: run.id(),
    task: run.task(),
    base: BranchName::new(BASE).expect("branch"),
    waits_for: WaitReason::Message,
  };
  let wait = RunWaitsOn::new(waiting, task.id(), project).expect("a wait");
  db.keep_run_wait(&wait, Timestamp::now()).expect("kept");
  (run, key)
}

/// The waiting task's key.
fn key_of(db: &ProjectDb, run: &Run) -> String {
  db.task(run.task()).expect("read").expect("the task").key().to_string()
}

/// A conversation whose one answer made each of `runs` wait on its task, after a reply; the conversation and the id of
/// each run's row.
fn answer(screen: &Orchestrator, runs: &[&(Run, String)]) -> (ChatId, Vec<String>) {
  let mut db = screen.db();
  let at = Timestamp::now();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let body = MessageBody::new("What do the delivery agents wait for?").expect("text");
  db.keep_chat_message(&Chat::start(chat, &body, at), (message, &body), at)
    .expect("kept");
  db.add_chat_reply(
    chat,
    ("They wait on other tasks.", &ModelId::new("haiku").expect("model")),
    at,
  )
  .expect("reply");
  for (run, waited) in runs {
    let action = ChatAction {
      waits_on: Some(WaitsOnRow {
        run: run.id(),
        number: run.number().get(),
        waited: waited.clone(),
      }),
      ..ChatAction::new("wait_on_task", Some(key_of(&db, run)), ActionOutcome::Done)
    };
    db.add_chat_action(chat, &action, at).expect("action");
  }
  db.end_chat_turn(message, &TurnState::Answered, at).expect("ended");
  let rows = (1..=runs.len())
    .map(|index| format!("orchestrator-action-{message}-{index}"))
    .collect();
  (chat, rows)
}

#[test]
fn orchestrator_waits_on_row_says_how_the_wait_stands() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let delivery = screen.delivery();
  let retry = waits_on(&screen, delivery, ("Retry failed deliveries", "Shared rate limiter"));
  let continued = waits_on(
    &screen,
    delivery,
    ("Batch retries per endpoint", "Retry budget storage"),
  );
  let canceled = waits_on(&screen, delivery, ("Delivery dashboard", "Delivery receipts"));
  let dropped = waits_on(&screen, delivery, ("Endpoint health", "Health probe"));
  let mut db = screen.db();
  let now = Timestamp::now();
  assert!(
    db.end_run_wait(continued.0.id(), RunWaitEnd::Merged, now)
      .expect("ended")
  );
  assert!(
    db.end_run_wait(canceled.0.id(), RunWaitEnd::Canceled, now)
      .expect("ended")
  );
  assert!(db.drop_run_wait(dropped.0.id(), now).expect("dropped"));
  let named =
    [&retry, &continued, &canceled, &dropped].map(|(run, _)| format!("{} · #{}", key_of(&db, run), run.number().get()));
  drop(db);
  let (chat, rows) = answer(&screen, &[&retry, &continued, &canceled, &dropped]);
  let [waits, merged, unmet, gone] = [0, 1, 2, 3].map(|index| rows[index].clone());
  screen.go(orchester_app::routes::OrchestratorChat::Chat(chat));
  screen.until("the rows", |screen| screen.exists(&gone));

  assert_eq!(
    screen.text(&format!("{waits}-title")),
    format!("{} waits on {}", named[0], retry.1)
  );
  assert_eq!(
    screen.text(&format!("{waits}-note")),
    format!("It continues when {} is merged into {BASE}", retry.1)
  );
  assert!(
    !screen.exists(&format!("{waits}-open")),
    "a wait that holds offers Drop the wait under the pointer only"
  );
  hover(&mut screen, &waits);
  screen.until("Drop the wait under the pointer", |screen| {
    screen.exists(&format!("{waits}-open"))
  });
  assert_eq!(screen.text(&format!("{waits}-open")), "Drop the wait");
  hover(&mut screen, &merged);
  screen.until("Drop the wait gone with the pointer", |screen| {
    !screen.exists(&format!("{waits}-open"))
  });
  assert_eq!(
    screen.text(&format!("{merged}-title")),
    format!("{} continued", named[1])
  );
  assert!(
    screen
      .text(&format!("{merged}-note"))
      .starts_with(&format!("{} merged into {BASE} at ", continued.1))
  );
  assert_eq!(screen.text(&format!("{merged}-open")), "Open ↗");
  assert_eq!(
    screen.text(&format!("{unmet}-title")),
    format!("{} still waits on {}", named[2], canceled.1)
  );
  assert!(
    screen
      .text(&format!("{unmet}-note"))
      .ends_with("— nothing will end this wait")
  );
  assert_eq!(
    screen.text(&format!("{unmet}-open")),
    "Drop the wait",
    "a wait nothing else will end offers it always"
  );
  assert_eq!(
    screen.text(&format!("{gone}-title")),
    format!("{} no longer waits on {}", named[3], dropped.1)
  );
  assert!(screen.text(&format!("{gone}-note")).starts_with("You dropped it at "));
  assert_eq!(screen.text(&format!("{gone}-open")), "Open ↗");
}

#[test]
fn orchestrator_only_the_person_drops_a_wait_from_its_row() {
  let mut screen = Orchestrator::new(Setup::NoRunner);
  let delivery = screen.delivery();
  let canceled = waits_on(&screen, delivery, ("Delivery dashboard", "Delivery receipts"));
  let mut db = screen.db();
  assert!(
    db.end_run_wait(canceled.0.id(), RunWaitEnd::Canceled, Timestamp::now())
      .expect("ended")
  );
  drop(db);
  let (chat, rows) = answer(&screen, &[&canceled]);
  let row = rows[0].clone();
  screen.go(orchester_app::routes::OrchestratorChat::Chat(chat));
  let drop_it = format!("{row}-open");
  screen.until("Drop the wait", |screen| screen.exists(&drop_it));

  with_synthetic_input(|| screen.click(&drop_it));
  assert_eq!(
    screen.window.refused_press(),
    Some("drop a run's wait"),
    "refused to the client"
  );
  screen.window.pass();
  assert!(!screen.exists(&format!("{row}-reason")), "the chat says nothing of it");
  assert!(screen.exists(&drop_it), "the row stays as it was");
  let ended = screen
    .db()
    .run_wait(canceled.0.id())
    .expect("read")
    .expect("kept")
    .ended;
  assert_eq!(
    ended.map(|ended| ended.end),
    Some(RunWaitEnd::Canceled),
    "the wait still holds"
  );

  screen.click(&drop_it);
  screen.until("the wait dropped", |screen| {
    screen
      .db()
      .run_wait(canceled.0.id())
      .expect("read")
      .and_then(|kept| kept.ended)
      .is_some_and(|ended| ended.end == RunWaitEnd::Dropped)
  });
  screen.until("the row turned to dropped", |screen| {
    screen.text(&format!("{row}-title")).contains("no longer waits on")
  });
}
