//! What the composer keeps: a draft through a failed send that came after one that went through, with
//! *Try again*; a draft through a switch to another tab of the task and back; what is written when a message
//! its agent did not take is put back into the composer; and no actions on such a message once no agent
//! reads messages.

use orchester_app::routes::{ProjectPage, Route, TaskView};
use rusqlite::Connection;

use super::{
  click, draft, key_of, messages, open, seeded,
  sending::{ENTER, not_delivered, until_stored},
  text, write,
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
