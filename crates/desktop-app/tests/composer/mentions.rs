//! @-mentions in a task Conversation's composer (ORC-312, ORC-316, `09` `Am5KT` … `09e` `y5fJD`): typing `@` opens
//! the menu with the run's own task first and the files of the run's worktree, typing filters it, Enter inserts the
//! mention as a chip, Backspace after the chip removes it whole, Esc closes the menu and keeps the text; a message sent
//! with a mention is stored with the marked block of the task's or run's facts, or the file's path, that its agent
//! reads, and the chat shows only the person's words.

use std::fs;

use orchester_store::ProjectDb;
use orchester_workspace::tasks::changes::run_folder;

use super::{click, detail, draft, key, messages, open, seeded, text, type_on, write};
use crate::sending::{ENTER, until_stored};

const MENU: &str = "composer-mention-menu";
const TITLE: &str = "Localize the invoice emails";

#[test]
fn composer_mentions_offer_the_run_worktree_files_and_send_the_path() {
  // Ready for review: a message is a follow-up stored for the runner, and the menu lists its run's folder.
  let title = "Show the signed-in account";
  let (mut window, seeded) = seeded();
  let task = open(&mut window, &seeded, title);
  let run = detail(&seeded, &task).run.expect("the task's run").run.id();
  let db = ProjectDb::open(&seeded.repo).expect("the project database opens");
  let folder = run_folder(&db, &seeded.repo, run).unwrap_or_else(|| seeded.repo.clone());
  fs::create_dir_all(folder.join("src")).expect("its folder");
  fs::write(folder.join("src/backoff_policy.rs"), "pub fn backoff() {}\n").expect("a file of the run");
  let file = "composer-mention-file-src/backoff_policy.rs";
  write(&mut window, "Use @backoff");
  window.until("the run's file in the @ menu", |window| {
    window.tree.get_element_by_id(file).is_some()
  });
  assert!(
    text(&mut window, "mention-section-files").starts_with("Files"),
    "{}",
    text(&mut window, MENU)
  );
  key(&mut window, ENTER, (false, false));
  window.until("the file's mention in the draft", |window| {
    window.tree.get_element_by_id(MENU).is_none()
  });
  assert_eq!(draft(&mut window), "Use @`src/backoff_policy.rs` ");
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  assert_eq!(
    draft(&mut window),
    "Use ",
    "Backspace after the file's chip removes it whole"
  );

  type_on(&mut window, "@backoff");
  window.until("the file again", |window| window.tree.get_element_by_id(file).is_some());
  click(&mut window, file);
  window.until("the file's mention in the draft", |window| {
    draft(window) == "Use @`src/backoff_policy.rs` "
  });
  type_on(&mut window, "here");
  key(&mut window, ENTER, (false, false));
  until_stored(&mut window, &seeded, title, 1);
  let stored = messages(&seeded, &task).pop().expect("the message");
  let body = stored.body().as_str();
  assert!(body.starts_with("Use @`src/backoff_policy.rs` here"), "{body}");
  assert!(
    body.contains("- @`src/backoff_policy.rs`: the file src/backoff_policy.rs, its path relative to the root"),
    "the agent gets the path: {body}"
  );
  assert!(!body.contains("pub fn backoff"), "never the file's content: {body}");
  let said = format!("conversation-message-{}-text", stored.id());
  window.until("the message in the chat", |window| !text(window, &said).is_empty());
  let shown = text(&mut window, &said);
  assert!(
    shown.contains("Use backoff_policy.rs here"),
    "the file shows by its name as a link: {shown}"
  );
}

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

  type_on(&mut window, "@");
  window.until("the menu with the focus on the own task", |window| {
    window.tree.get_element_by_id(&own).is_some() && text(window, &own).contains("Enter")
  });
  key(&mut window, ("ArrowDown", "ArrowDown"), (false, false));
  window.until("the focus drawn on the second row", |window| {
    !text(window, &own).contains("Enter") && text(window, MENU).contains("Enter")
  });
  key(&mut window, ENTER, (false, false));
  let inserted = draft(&mut window);
  assert!(
    inserted.starts_with("Ask @") && inserted != format!("Ask @{task} "),
    "Enter inserts the row the focus is drawn on, not the first: {inserted}"
  );
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  assert_eq!(draft(&mut window), "Ask ");

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

#[test]
fn composer_mentions_insert_and_remove_a_run_and_reach_the_agent_with_its_facts() {
  // Ready for review: its run #N leads the Runs, and a message is a follow-up stored for the runner.
  let title = "Show the signed-in account";
  let (mut window, seeded) = seeded();
  let task = open(&mut window, &seeded, title);
  let number = detail(&seeded, &task).run.expect("the task's run").run.number().get();
  let run = format!("composer-mention-run-{task}-{number}");
  let mention = format!("@{task}#{number}");
  write(&mut window, "Ask @");
  window.until("the run in the @ menu", |window| {
    window.tree.get_element_by_id(&run).is_some()
  });
  type_on(&mut window, &format!("#{number}"));
  window.until("the menu filtered to runs", |window| {
    let own = format!("composer-mention-task-{task}");
    window.tree.get_element_by_id(&run).is_some() && window.tree.get_element_by_id(&own).is_none()
  });
  key(&mut window, ENTER, (false, false));
  window.until("the run's mention in the draft", |window| {
    window.tree.get_element_by_id(MENU).is_none()
  });
  assert_eq!(draft(&mut window), format!("Ask {mention} "), "Enter inserts the run");
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  key(&mut window, ("Backspace", "Backspace"), (false, false));
  assert_eq!(
    draft(&mut window),
    "Ask ",
    "Backspace after the run's chip removes it whole"
  );

  type_on(&mut window, "@");
  window.until("the run in the @ menu", |window| {
    window.tree.get_element_by_id(&run).is_some()
  });
  click(&mut window, &run);
  window.until("the run's mention in the draft", |window| {
    draft(window) == format!("Ask {mention} ")
  });
  type_on(&mut window, "again");
  key(&mut window, ENTER, (false, false));
  until_stored(&mut window, &seeded, title, 1);
  let stored = messages(&seeded, &task).pop().expect("the message");
  let body = stored.body().as_str();
  assert!(body.starts_with(&format!("Ask {mention} again")), "{body}");
  assert!(
    body.contains(&format!(
      "- {mention}: run #{number} of task {task} \"{title}\", state "
    )),
    "the run's facts: {body}"
  );
  assert!(body.contains(", branch "), "{body}");
  let said = format!("conversation-message-{}-text", stored.id());
  window.until("the message in the chat", |window| !text(window, &said).is_empty());
  let shown = text(&mut window, &said);
  assert!(
    shown.contains(&format!("Ask {task} · #{number} again")),
    "the run shows as a link: {shown}"
  );
}
