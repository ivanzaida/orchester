//! An item answered in place whose request cannot be read again: the old card, which may no longer wait, goes,
//! the item says why, and nothing can be answered there until a reading succeeds.

use std::fs;

use super::{expand_approval, ids_under, open, seeded};

#[test]
fn needs_you_panel_locks_an_item_whose_request_cannot_be_read_again() {
  let (mut window, seeded) = seeded("cards");
  open(&mut window);
  let (item, _) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let database = seeded.repo.join(".orchester").join("orchester.db");
  let moved = seeded.repo.join(".orchester").join("orchester.db.away");
  fs::rename(&database, &moved).expect("the project's database moves away");
  window.projects().feed(seeded.project).saved();
  let error = format!("{item}-error");
  window.until("why the request cannot be read", |window| {
    window.tree.get_element_by_id(&error).is_some()
  });
  assert!(
    ids_under(&mut window, &item, "cards-approval-").is_empty(),
    "no old card to answer while the request cannot be read"
  );

  fs::rename(&moved, &database).expect("the project's database is back");
  window.projects().feed(seeded.project).saved();
  window.until("the card read again", |window| {
    !ids_under(window, &item, "cards-approval-").is_empty()
  });
  assert!(window.tree.get_element_by_id(&error).is_none());
}
