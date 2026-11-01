//! @-mentions in a task Conversation's composer (ORC-312, `09` `Am5KT` … `09e` `y5fJD`): typing `@` opens the menu
//! with the run's own task first, typing filters it, Enter inserts the mention as a chip, Backspace after the chip
//! removes it whole, Esc closes the menu and keeps the text; a message sent with a mention is stored with the marked
//! block of the task's facts that its agent reads, and the chat shows only the person's words.

use super::{click, draft, key, messages, open, seeded, text, type_on, write};
use crate::sending::{ENTER, until_stored};

const MENU: &str = "composer-mention-menu";
const TITLE: &str = "Localize the invoice emails";

#[test]
fn composer_mentions_open_with_the_own_task_first_and_insert_and_remove_it() {
  let (mut window, seeded) = seeded();
  let task = open(&mut window, &seeded, TITLE);
  write(&mut window, "Ask @");
  let own = format!("composer-mention-task-{task}");
  window.until("the @ menu", |window| {
    window.tree.get_element_by_id(MENU).is_some() && window.tree.get_element_by_id(&own).is_some()
  });
  let shown = text(&mut window, MENU);
  // *Tasks*, how many of how many, then the first row's key.
  let first = shown.split_whitespace().nth(4);
  assert!(shown.starts_with("Tasks 3 of "), "{shown}");
  assert_eq!(first, Some(&*task.to_string()), "the run's own task leads: {shown}");
  type_on(&mut window, "invoice");
  window.until("the menu filtered to the task", |window| {
    window.tree.get_element_by_id(&own).is_some() && text(window, MENU).contains("for “invoice”")
  });
  key(&mut window, ENTER, (false, false));
  window.until("the mention in the draft", |window| {
    window.tree.get_element_by_id(MENU).is_none()
  });
  assert_eq!(
    draft(&mut window),
    format!("Ask @{task} "),
    "Enter inserts; nothing is sent"
  );
  assert!(
    messages(&seeded, &task).is_empty(),
    "Enter took the menu's choice, not the send"
  );

  key(&mut window, ("Backspace", "Backspace"), (false, false));
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  assert_eq!(draft(&mut window), "Ask ", "Backspace after the chip removes it whole");

  type_on(&mut window, "@zzz");
  window.until("nothing to match", |window| {
    text(window, MENU).contains("Nothing matches “zzz”")
  });
  key(&mut window, ("Escape", "Escape"), (false, false));
  window.until("the menu to close", |window| {
    window.tree.get_element_by_id(MENU).is_none()
  });
  assert_eq!(draft(&mut window), "Ask @zzz", "Esc keeps the text as typed");
}

#[test]
fn composer_mentions_reach_the_agent_with_the_task_facts_and_show_as_a_link() {
  let (mut window, seeded) = seeded();
  let task = open(&mut window, &seeded, TITLE);
  write(&mut window, "Follow @");
  let own = format!("composer-mention-task-{task}");
  window.until("the @ menu", |window| window.tree.get_element_by_id(&own).is_some());
  click(&mut window, &own);
  window.until("the mention in the draft", |window| {
    draft(window) == format!("Follow @{task} ")
  });
  type_on(&mut window, "closely");
  key(&mut window, ENTER, (false, false));
  until_stored(&mut window, &seeded, TITLE, 1);
  let stored = &messages(&seeded, &task)[0];
  let body = stored.body().as_str();
  assert!(body.starts_with(&format!("Follow @{task} closely")), "{body}");
  assert!(body.contains("<orchester-mentions>"), "a marked block: {body}");
  assert!(
    body.contains(&format!("- @{task}: task {task} \"{TITLE}\", workflow status")),
    "{body}"
  );
  let said = format!("conversation-message-{}-text", stored.id());
  window.until("the message in the chat", |window| !text(window, &said).is_empty());
  let shown = text(&mut window, &said);
  assert!(shown.contains(&format!("Follow {task} closely")), "{shown}");
  assert!(
    !shown.contains("orchester-mentions"),
    "the facts are the agent's: {shown}"
  );
}
