//! Alternate input paths and task data retained after a rejected reread.
use orchester_app::{project::TasksState, routes::TaskView};
use orchester_core::{
  conversation::{Delivery, Message, MessageBody, Recipient, Sender, Timing, Undelivered},
  id::{IdempotencyKey, MessageId},
  run::Answerer,
  store::{ConversationStore, RunStore},
  time::Timestamp,
};

use super::Fixture;

/// Stores `words`, sent now to the agent that waits on a free-text question, as one its CLI refused.
fn not_delivered(f: &Fixture, words: &str) -> MessageId {
  let mut db = f.db();
  let run = db.run(f.run).expect("run").expect("run");
  let execution = run
    .awaiting_answer()
    .into_iter()
    .next()
    .expect("an agent waits on its question");
  let body = MessageBody::new(words).expect("a message");
  let message = Message::from_person(run.task(), Some(f.run), Recipient::Orchestrator, body, Timestamp::now())
    .sent(Timing::Now)
    .expect("a person's message");
  let mut message = db.send_message(IdempotencyKey::generate(), &message).expect("stored");
  message
    .not_delivered(f.run, execution, Timestamp::now(), Undelivered::Refused)
    .expect("not delivered");
  db.save_deliveries(std::slice::from_ref(&message))
    .expect("delivery stored");
  message.id()
}

#[test]
fn person_decisions_undelivered_resend_answering_a_question_refuses_and_records_the_decider() {
  for by in [Answerer::Person, Answerer::Automation] {
    let mut f = Fixture::new("task-states", "Sync contacts with HubSpot", TaskView::Conversation);
    f.app.until("composer", |app| app.shows("composer-input"));
    let id = not_delivered(&f, "Keep the current behavior.");
    let project = f.app.services().workspace.projects().expect("projects")[0].id;
    f.app.link.projects().expect("open projects").feed(project).saved();
    let button = format!("composer-send-after-turn-{id}");
    let db = f.db();
    let task = db.run(f.run).expect("run").expect("run").task();
    let stored = |db: &orchester_store::ProjectDb| {
      db.messages(task)
        .expect("messages")
        .into_iter()
        .find(|message| message.id() == id)
        .expect("the message")
    };
    if by == Answerer::Automation {
      f.press(&button, by);
      let refused = format!("composer-undelivered-failed-{id}");
      f.app.until("synthetic resend refused", |app| app.shows(&refused));
      let message = stored(&db);
      assert!(matches!(
        message.sender(),
        Sender::Person {
          delivery: Delivery::NotDelivered { .. },
          ..
        }
      ));
      assert_eq!(message.answerer(), Some(Answerer::Person));
      f.consent();
    }
    f.press(&button, by);
    f.app.until("resend stored with its decider", |_| {
      let message = stored(&db);
      message.timing() == Timing::AfterTurn
        && !matches!(
          message.sender(),
          Sender::Person {
            delivery: Delivery::NotDelivered { .. },
            ..
          }
        )
        && message.answerer() == Some(by)
    });
  }
}

#[test]
fn person_decisions_leaving_a_question_still_requires_consent_to_send() {
  for by in [Answerer::Person, Answerer::Automation] {
    let mut f = Fixture::new("task-states", "Sync contacts with HubSpot", TaskView::Conversation);
    f.app.until("question context", |app| app.shows("composer-leave"));
    f.app.click_by_client("composer-leave").expect("leave the context");
    f.app.until("context left", |app| !app.shows("composer-leave"));
    f.app.set_by_client("composer-input", "Keep the current behavior.");
    let db = f.db();
    let before = db.run(f.run).expect("run").expect("run");
    let task = before.task();
    if by == Answerer::Automation {
      f.press("composer-send", by);
      f.app.until("refusal", |app| app.shows("composer-failed"));
      assert_eq!(
        db.run(f.run).expect("run").expect("run").last_sequence(),
        before.last_sequence()
      );
      assert!(
        db.messages(task)
          .expect("messages")
          .iter()
          .all(|message| message.body().as_str() != "Keep the current behavior.")
      );
      f.consent();
    }
    f.press("composer-send", by);
    f.app.until("answer stored with its decider", |_| {
      db.messages(task)
        .expect("messages")
        .iter()
        .any(|message| message.body().as_str() == "Keep the current behavior." && message.answerer() == Some(by))
    });
  }
}

fn rename_accounts(f: &Fixture, from: &str, to: &str) {
  rusqlite::Connection::open(f.repo.join(".orchester").join("orchester.db"))
    .expect("project database")
    .execute(&format!("ALTER TABLE {from} RENAME TO {to}"), [])
    .expect("make task detail unreadable");
}

#[test]
fn person_decisions_failed_reread_with_retained_data_offers_no_controls_until_fresh() {
  for (scenario, title, view, button) in [
    (
      "task-states",
      "Rate-limit messages from GitHub",
      TaskView::Summary,
      "plan-approve",
    ),
    (
      "decisions",
      "Check the empty states on screen",
      TaskView::Summary,
      "task-stop-run",
    ),
    (
      "task-states",
      "Sync contacts with HubSpot",
      TaskView::Conversation,
      "composer-send",
    ),
  ] {
    let mut f = Fixture::new(scenario, title, view);
    f.app.until("decision shown", |app| app.shows(button));
    let project = f.app.services().workspace.projects().expect("projects")[0].id;
    let feed = f.app.link.projects().expect("open projects").feed(project);
    let before = f.db().run(f.run).expect("run").expect("run").last_sequence();
    rename_accounts(&f, "invocation_accounts", "invocation_accounts_away");
    feed.saved();
    f.app.until("reread rejected", |app| app.shows("task-load-error"));
    assert!(
      matches!(feed.tasks.get(), TasksState::Ready(_)),
      "the task frame stays mounted"
    );
    assert!(!f.app.shows(button), "retained data must not offer {button}");
    feed.saved();
    f.app.pass();
    assert!(!f.app.shows(button), "retrying must not reactivate {button}");
    assert!(f.app.shows("task-load-error"));
    assert_eq!(f.db().run(f.run).expect("run").expect("run").last_sequence(), before);

    rename_accounts(&f, "invocation_accounts_away", "invocation_accounts");
    feed.saved();
    f.app.until("fresh decision", |app| app.shows(button));
    assert!(!f.app.shows("task-load-error"));
  }
}
