//! What the composer keeps: a draft through a failed send that came after one that went through, with
//! *Try again*; a draft through a switch to another tab of the task and back, and through leaving the task
//! and opening it again; what is written when a message its agent did not take is put back into the
//! composer, typing on after it, and no more actions on that message; and no actions on such a message
//! once no agent reads messages.

use orchester_app::routes::{ProjectPage, Route, TaskView};
use rusqlite::Connection;

use super::{
  click, draft, key_of, messages, open, seeded,
  sending::{ENTER, not_delivered, until_stored},
  text, type_on, write,
};

#[test]
fn composer_a_failed_send_after_one_that_went_through_keeps_the_draft() {
  let (mut window, seeded) = seeded();
  let title = "Localize the invoice emails";
  let key = open(&mut window, &seeded, title);
  write(&mut window, "First");
  super::key(&mut window, ENTER, (false, false));
  until_stored(&mut window, &seeded, title, 1);
  window.until("the draft emptied", |window| !draft(window).starts_with("First"));
  // Another writer holds the database: the second store gives up after its busy timeout.
  let database = seeded.repo.join(".orchester").join("orchester.db");
  let holder = Connection::open(&database).expect("the database opens");
  holder.execute_batch("BEGIN EXCLUSIVE").expect("the database is held");
  write(&mut window, "Second");
  super::key(&mut window, ENTER, (false, false));
  window.until("the failed send", |window| {
    window.tree.get_element_by_id("composer-failed").is_some()
  });
  assert_eq!(draft(&mut window), "Second", "the draft stays");
  assert!(window.tree.get_element_by_id("composer-retry").is_some(), "Try again");
  holder.execute_batch("ROLLBACK").expect("the database is let go");
  click(&mut window, "composer-retry");
  until_stored(&mut window, &seeded, title, 2);
  assert_eq!(messages(&seeded, &key)[1].body().as_str(), "Second");
}

#[test]
fn composer_the_draft_stays_through_a_switch_to_another_tab() {
  let (mut window, seeded) = seeded();
  let key = open(&mut window, &seeded, "Show the signed-in account");
  write(&mut window, "Not yet");
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key, TaskView::Summary),
  ));
  window.until("the Summary", |window| {
    window.tree.get_element_by_id("composer").is_none()
  });
  open(&mut window, &seeded, "Show the signed-in account");
  assert_eq!(draft(&mut window), "Not yet");
}

#[test]
fn composer_edit_in_composer_adds_the_message_after_what_is_written() {
  let (mut window, seeded) = seeded();
  let title = "Paginate the events API";
  let id = not_delivered(&seeded, &key_of(&seeded, title), "Check the cursor");
  open(&mut window, &seeded, title);
  write(&mut window, "First");
  let edit = format!("composer-edit-{id}");
  window.until("the message's actions", |window| {
    window.tree.get_element_by_id(&edit).is_some()
  });
  click(&mut window, &edit);
  assert_eq!(draft(&mut window), "First\n\nCheck the cursor");
  // The field has the focus, its caret after what was put in it.
  window.until("the field focused", |window| {
    window
      .tree
      .focused_element()
      .is_some_and(|element| element.id() == Some("composer-input"))
  });
  type_on(&mut window, "x");
  assert_eq!(draft(&mut window), "First\n\nCheck the cursorx");
  assert!(
    window.tree.get_element_by_id(&edit).is_none(),
    "a message put back into the composer offers nothing more"
  );
  // Its words taken out of the draft again, it offers again.
  for _ in 0.."First\n\nCheck the cursorx".len() {
    super::key(&mut window, ("Backspace", "Backspace"), (false, false));
  }
  window.until("the offers back", |window| {
    window.tree.get_element_by_id(&edit).is_some()
  });
}

#[test]
fn composer_a_send_that_ends_after_the_page_closed_leaves_no_draft_to_send_twice() {
  let (mut window, seeded) = seeded();
  let title = "Localize the invoice emails";
  let key = open(&mut window, &seeded, title);
  write(&mut window, "Once");
  // The store waits for another writer while the person leaves the task.
  let database = seeded.repo.join(".orchester").join("orchester.db");
  let holder = Connection::open(&database).expect("the database opens");
  holder.execute_batch("BEGIN EXCLUSIVE").expect("the database is held");
  super::key(&mut window, ENTER, (false, false));
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key_of(&seeded, "Show the signed-in account"), TaskView::Conversation),
  ));
  window.until("the other task's composer", |window| draft(window) != "Once");
  holder.execute_batch("ROLLBACK").expect("the database is let go");
  until_stored(&mut window, &seeded, title, 1);
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key.clone(), TaskView::Conversation),
  ));
  window.until("the task's own composer", |window| {
    draft(window).starts_with("Write to")
  });
  super::key(&mut window, ENTER, (false, false));
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
  }
  assert_eq!(messages(&seeded, &key).len(), 1, "stored once");
}

#[test]
fn composer_a_message_of_a_run_no_agent_reads_offers_nothing() {
  let (mut window, seeded) = seeded();
  let title = "Set up CI caching";
  let id = not_delivered(&seeded, &key_of(&seeded, title), "Too late");
  open(&mut window, &seeded, title);
  window.until("the message", |window| {
    text(window, "conversation").contains("Too late")
  });
  assert!(window.tree.get_element_by_id(&format!("composer-edit-{id}")).is_none());
  assert!(
    window
      .tree
      .get_element_by_id(&format!("composer-send-after-turn-{id}"))
      .is_none()
  );
}

#[test]
fn composer_the_draft_stays_when_the_task_is_left_and_opened_again() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Show the signed-in account");
  write(&mut window, "Later");
  open(&mut window, &seeded, "Paginate the events API");
  window.until("the other task's own composer", |window| draft(window) != "Later");
  open(&mut window, &seeded, "Show the signed-in account");
  window.until("the draft back", |window| draft(window) == "Later");
}
