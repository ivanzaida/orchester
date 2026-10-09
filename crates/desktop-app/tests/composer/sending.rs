//! What sending does: Enter sends, Shift+Enter adds a line, Ctrl+Enter sends too where the state has one way;
//! nothing written sends nothing; a draft is stored once however often it is sent; a send that fails keeps
//! the draft and *Try again* stores it once; a message the runner does not take stays queued, with why; a
//! plan's changes the runner does not take keep the draft; a message for a run stopped at its limit is kept
//! and Summary opens; and a message its agent's CLI did not take is sent after the turn or put back into the
//! composer.

use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{
  conversation::{Delivery, Message, MessageBody, Recipient, Sender, Timing, Undelivered},
  id::IdempotencyKey,
  store::ConversationStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use super::{click, detail, draft, key_of, messages, open, seeded, task_id, text, type_on, write};

/// Waits until the task `title` has `count` messages stored.
fn until_stored(window: &mut support::Window, seeded: &kontur::Seeded, title: &str, count: usize) {
  let key = key_of(seeded, title);
  window.until(&format!("{count} messages of {key}"), |_| {
    super::messages(seeded, &key).len() == count
  });
}

use super::{kontur, support};

const ENTER: (&str, &str) = ("Enter", "Enter");

#[test]
fn composer_enter_sends_once_and_empties_the_draft() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Localize the invoice emails");
  write(&mut window, "Use the customer locale");
  // Pressed twice before the first send ends: the message is stored once.
  super::key(&mut window, ENTER, (false, false));
  super::key(&mut window, ENTER, (false, false));
  window.until("the draft emptied", |window| {
    draft(window).is_empty() || draft(window).starts_with("Ask,")
  });
  until_stored(&mut window, &seeded, "Localize the invoice emails", 1);
  window.until("the message, waiting for the run", |window| {
    text(window, "conversation-waiting").contains("Use the customer locale")
  });
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
  let stored = messages(&seeded, &key);
  assert_eq!(stored.len(), 1, "stored once");
  assert_eq!(stored[0].body().as_str(), "Use the customer locale");
  assert!(
    text(&mut window, "conversation-waiting").contains("Queued · waiting for the run to start"),
    "{}",
    text(&mut window, "conversation-waiting")
  );
}

#[test]
fn composer_shift_enter_adds_a_line_and_ctrl_enter_sends() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Localize the invoice emails");
  write(&mut window, "first");
  super::key(&mut window, ENTER, (true, false));
  type_on(&mut window, "second");
  assert!(messages(&seeded, &key).is_empty(), "Shift+Enter sends nothing");
  super::key(&mut window, ENTER, (false, true));
  until_stored(&mut window, &seeded, "Localize the invoice emails", 1);
  assert_eq!(messages(&seeded, &key)[0].body().as_str(), "first\nsecond");
}

#[test]
fn composer_nothing_written_sends_nothing() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Localize the invoice emails");
  click(&mut window, "composer-input");
  super::key(&mut window, ENTER, (false, false));
  click(&mut window, "composer-send");
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
  assert!(messages(&seeded, &key).is_empty());
}

#[test]
fn composer_a_failed_send_keeps_the_draft_and_try_again_stores_it_once() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Localize the invoice emails");
  write(&mut window, "Keep me");
  // Another writer holds the database: the store gives up after its busy timeout.
  let database = seeded.repo.join(".orchester").join("orchester.db");
  let holder = Connection::open(&database).expect("the database opens");
  holder.execute_batch("BEGIN EXCLUSIVE").expect("the database is held");
  super::key(&mut window, ENTER, (false, false));
  window.until("the failed send", |window| {
    window.tree.get_element_by_id("composer-failed").is_some()
  });
  assert!(
    text(&mut window, "composer-failed").starts_with("Your message was not sent: "),
    "{}",
    text(&mut window, "composer-failed")
  );
  assert_eq!(draft(&mut window), "Keep me", "the draft stays");
  holder.execute_batch("ROLLBACK").expect("the database is let go");
  click(&mut window, "composer-retry");
  until_stored(&mut window, &seeded, "Localize the invoice emails", 1);
  window.until("the failure gone", |window| {
    window.tree.get_element_by_id("composer-failed").is_none()
  });
  assert_eq!(messages(&seeded, &key).len(), 1);
}

#[test]
fn composer_a_follow_up_the_runner_does_not_take_stays_queued_with_why() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Show the signed-in account");
  write(&mut window, "Add a test");
  super::key(&mut window, ENTER, (false, false));
  window.until("why it stays queued", |window| {
    window.tree.get_element_by_id("composer-kept").is_some()
  });
  assert!(!text(&mut window, "composer-kept").is_empty());
  let stored = messages(&seeded, &key);
  assert_eq!(stored.len(), 1, "stored, waiting for the runner");
  assert!(matches!(
    stored[0].sender(),
    Sender::Person {
      delivery: Delivery::Queued,
      ..
    }
  ));
}

#[test]
fn composer_plan_changes_the_runner_does_not_take_keep_the_draft() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Rate-limit messages from GitHub");
  write(&mut window, "Queue them");
  super::key(&mut window, ENTER, (false, false));
  window.until("the refusal", |window| {
    window.tree.get_element_by_id("composer-failed").is_some()
  });
  assert_eq!(draft(&mut window), "Queue them");
  assert!(window.tree.get_element_by_id("composer-retry").is_some(), "Try again");
  assert!(messages(&seeded, &key).is_empty(), "a plan's changes are no message");
}

#[test]
fn composer_a_message_for_a_run_stopped_at_its_limit_is_kept_and_summary_opens() {
  let (mut window, seeded) = seeded();
  let title = "Explain an expired GitHub token in the status bar";
  let key = open(&mut window, &seeded, title);
  write(&mut window, "Go on");
  super::key(&mut window, ENTER, (false, false));
  until_stored(&mut window, &seeded, title, 1);
  let summary = Route::Project(seeded.project, ProjectPage::Task(key, TaskView::Summary));
  window.until("Summary", |window| window.navigation().route_now() == summary);
}

#[test]
fn composer_a_message_its_agent_did_not_take_goes_after_the_turn_or_back_into_the_composer() {
  let (mut window, seeded) = seeded();
  let title = "Paginate the events API";
  let key = key_of(&seeded, title);
  let id = not_delivered(&seeded, &key, "Check the cursor");
  open(&mut window, &seeded, title);
  let edit = format!("composer-edit-{id}");
  window.until("the message's actions", |window| {
    window.tree.get_element_by_id(&edit).is_some()
  });
  click(&mut window, &edit);
  assert_eq!(draft(&mut window), "Check the cursor", "Edit in composer");
  click(&mut window, &format!("composer-send-after-turn-{id}"));
  window.until("queued again", |_| {
    messages(&seeded, &key).iter().any(|message| {
      message.id() == id
        && message.timing() == Timing::AfterTurn
        && matches!(
          message.sender(),
          Sender::Person {
            delivery: Delivery::Queued,
            ..
          }
        )
    })
  });
  window.until("the actions gone", |window| {
    window.tree.get_element_by_id(&edit).is_none()
  });
}

/// Stores `words` sent now to the paused run of `key`, which its agent's CLI refused.
fn not_delivered(
  seeded: &kontur::Seeded,
  key: &orchester_core::project::TaskKey,
  words: &str,
) -> orchester_core::id::MessageId {
  let run = detail(seeded, key).run.expect("the task ran");
  let execution = run
    .stopped_step()
    .or_else(|| run.current_step())
    .and_then(|step| step.execution)
    .expect("the run stopped in an agent's step");
  let body = MessageBody::new(words).expect("a message");
  let message = Message::from_person(
    task_id(seeded, key),
    Some(run.id()),
    Recipient::Orchestrator,
    body,
    Timestamp::now(),
  )
  .sent(Timing::Now)
  .expect("a person's message");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database opens");
  let mut message = db
    .send_message(IdempotencyKey::generate(), &message)
    .expect("the message is stored");
  message
    .not_delivered(run.id(), execution, Timestamp::now(), Undelivered::Refused)
    .expect("not delivered");
  db.save_deliveries(std::slice::from_ref(&message))
    .expect("the delivery is stored");
  message.id()
}
