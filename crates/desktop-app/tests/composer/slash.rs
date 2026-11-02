//! The slash menu in a task Conversation's composer (ORC-321, `06` `yKWQx`, `06b` `W4SNj3`): `/` at the start of the
//! draft opens the run's own commands; *Stop turn* is greyed with why while no turn of an agent runs, and Enter then
//! neither runs it nor sends; Esc closes the menu and keeps the text; a `/` inside text is just text. Compact, Model
//! and effort, and Permissions are not offered until their controls exist. That *Stop turn* stops a running turn is
//! `composer_stop_turn`'s, on a live run.

use orchester_app::ui::slash::{matched, query};

use super::{draft, key, messages, open, seeded, text, type_on, write};
use crate::sending::ENTER;

#[test]
fn composer_slash_opens_only_for_a_leading_slash_and_matches_letters_ignoring_case() {
  assert_eq!(query("/"), Some(""));
  assert_eq!(query("/co"), Some("co"));
  assert_eq!(query("Ask /co"), None, "a / inside text");
  assert_eq!(query("/co now"), None, "a space ends it");
  assert_eq!(query("/co\n"), None, "a new line ends it");
  assert_eq!(query(""), None);
  assert_eq!(matched("New conversation", ""), Some(0..0));
  assert_eq!(matched("New conversation", "co"), Some(4..6));
  assert_eq!(matched("Compact", "co"), Some(0..2), "ignoring case");
  assert_eq!(matched("Stop turn", "ST"), Some(0..2));
  assert_eq!(matched("History", "new"), None);
  assert_eq!(matched("Ärger", "är"), Some(0..3), "by letters, not bytes");
}

const MENU: &str = "composer-slash-menu";
const STOP: &str = "composer-slash-stop-turn";
const TITLE: &str = "Localize the invoice emails";

#[test]
fn composer_slash_offers_stop_turn_greyed_with_why_while_no_turn_runs() {
  let (mut window, seeded) = seeded();
  let task = open(&mut window, &seeded, TITLE);
  write(&mut window, "/");
  window.until("the slash menu", |window| {
    window.tree.get_element_by_id(MENU).is_some() && window.tree.get_element_by_id(STOP).is_some()
  });
  let shown = text(&mut window, MENU);
  assert!(shown.starts_with("Commands"), "{shown}");
  for missing in [
    "Compact",
    "Model and effort",
    "Permissions",
    "New conversation",
    "History",
  ] {
    assert!(!shown.contains(missing), "no {missing} here: {shown}");
  }
  let row = text(&mut window, STOP);
  assert!(row.contains("Stop turn"), "{row}");
  assert!(row.contains("No turn is running"), "why: {row}");
  assert!(!row.contains("Enter"), "a greyed command takes no focus: {row}");

  key(&mut window, ENTER, (false, false));
  assert!(
    window.tree.get_element_by_id(MENU).is_some(),
    "Enter runs nothing greyed"
  );
  assert_eq!(draft(&mut window), "/");
  assert!(messages(&seeded, &task).is_empty(), "nor sends the /");

  type_on(&mut window, "st");
  window.until("the filter", |window| text(window, MENU).contains("1 of 1"));
  key(&mut window, ("Escape", "Escape"), (false, false));
  window.until("the menu to close", |window| {
    window.tree.get_element_by_id(MENU).is_none()
  });
  assert_eq!(draft(&mut window), "/st", "Esc keeps the text as typed");
}

#[test]
fn composer_slash_inside_text_is_text() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, TITLE);
  write(&mut window, "Ask /stop");
  window.pass();
  assert!(window.tree.get_element_by_id(MENU).is_none(), "a / inside text is text");
  assert_eq!(draft(&mut window), "Ask /stop");
}
