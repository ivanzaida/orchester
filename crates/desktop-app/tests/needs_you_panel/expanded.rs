//! An approval expanded in the panel (`t7zhw`, `YDJsC`): the card starts at its subject, its parts 10 apart as
//! the item's, says why auto mode left it to the person, and the project's header stays pinned over the list while
//! the list scrolls under it (`o1BDj`).

use lurq::{app::events::ScrollPhase, core::ElementRect};

use super::{Window, expand_approval, open, seeded, settle, text};

fn bounds(window: &mut Window, id: &str) -> ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

#[test]
fn needs_you_panel_keeps_the_projects_header_over_its_list_while_it_scrolls() {
  let (mut window, seeded) = seeded("tasks");
  open(&mut window);
  // Expanding the lowest item brings the list to its end, to show it whole.
  let (item, _) = expand_approval(&mut window, "Add retry to webhook sender");
  settle(&mut window);
  let group = format!("needs-you-group-{}", seeded.project);
  let before = bounds(&mut window, &group);
  let list = bounds(&mut window, "needs-you-list");
  let item_before = bounds(&mut window, &item);
  let (x, y) = (list.x + list.width / 2.0, list.y + list.height / 2.0);
  // A positive delta scrolls toward the list's start.
  for _ in 0..4 {
    window.tree.mouse_move(x, y);
    window.tree.scroll(x, y, 0.0, 600.0, ScrollPhase::Scroll);
    window.pass();
    window.pass();
  }
  assert!(
    bounds(&mut window, &item).y > item_before.y,
    "the list scrolled back: {item_before:?}"
  );
  let after = bounds(&mut window, &group);
  assert!(
    (after.y - before.y).abs() < 0.5,
    "the project's header stays: {after:?}, was {before:?}"
  );
  assert!(after.y + after.height <= list.y + 0.5, "over the list, not in it");
  assert_eq!(text(&mut window, &group), "K Kontur this tab's project 6");
}

#[test]
fn needs_you_panel_shows_an_approval_from_its_subject_with_the_items_gaps() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (_, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  window.until("the request's subject", |window| {
    window.tree.get_element_by_id(&format!("{card}-subject")).is_some()
  });
  assert!(
    window.tree.get_element_by_id(&format!("{card}-explain")).is_none(),
    "the item says what the agent asks: the card leaves out its explanation"
  );
  let card_top = bounds(&mut window, &card);
  let subject = bounds(&mut window, &format!("{card}-subject"));
  assert!((subject.y - card_top.y).abs() < 0.5, "the subject comes first");
  let facts = bounds(&mut window, &format!("{card}-facts"));
  let deny = bounds(&mut window, &format!("{card}-deny-tells"));
  let gap = deny.y - (facts.y + facts.height);
  assert!((gap - 10.0).abs() < 0.5, "10 between the card's parts: {gap}");
}

#[test]
fn needs_you_panel_says_why_auto_mode_left_an_approval_to_the_person() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (_, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let held = format!("{card}-hold-cause");
  window.until("why auto mode held it", |window| {
    window.tree.get_element_by_id(&held).is_some()
  });
  assert_eq!(
    text(&mut window, &held),
    "Auto mode didn’t review this: the step’s network is off, and this command may reach it or install."
  );
}
