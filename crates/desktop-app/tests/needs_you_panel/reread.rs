//! An item answered in place whose request cannot be read again: the old card, which may no longer wait, goes,
//! the item says why, and nothing can be answered there until a reading succeeds — also while it is read again.

use super::{Window, expand_approval, open, seeded};
use crate::kontur::Seeded;

fn shows(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// Renames the project's table of tool requests to `to` from `from`, so reading the requests fails, or works again.
fn rename_requests(seeded: &Seeded, (from, to): (&str, &str)) {
  let database = seeded.repo.join(".orchester").join("orchester.db");
  rusqlite::Connection::open(database)
    .expect("the project's database")
    .execute(&format!("ALTER TABLE {from} RENAME TO {to}"), [])
    .expect("a real unreadable fixture");
}

#[test]
fn needs_you_panel_answers_nothing_in_place_once_reading_it_again_fails() {
  let (mut window, seeded) = seeded("cards");
  open(&mut window);
  let (item, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let (approve, error) = (format!("{card}-approve"), format!("{item}-error"));
  window.until("Approve once", |window| shows(window, &approve));
  rename_requests(&seeded, ("tool_approvals", "tool_approvals_away"));
  window.projects().feed(seeded.project).saved();
  window.until("why it cannot be read", |window| shows(window, &error));
  assert!(
    !shows(&mut window, &approve),
    "what was read before no longer answers once reading it again failed"
  );

  // Reading it again: the failure stands while the reading is under way.
  window.projects().feed(seeded.project).saved();
  window.pass();
  assert!(!shows(&mut window, &approve), "no old card while it is read again");
  assert!(shows(&mut window, &error));

  rename_requests(&seeded, ("tool_approvals_away", "tool_approvals"));
  window.projects().feed(seeded.project).saved();
  window.until("the card again", |window| shows(window, &approve));
  assert!(!shows(&mut window, &error));
}
