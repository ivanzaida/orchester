//! @-mentions in the Orchestrator's composer (ORC-312, `08` `gOs7u` … `08e` `dJyUv`): `@` opens the menu of the
//! project's tasks and runs, typing filters it, ↓ and Enter or a click insert a mention as a chip, Backspace after a
//! chip removes it whole and Esc closes the menu and keeps the text. A message sent with a mention reaches the agent
//! with the marked block of the mentioned task's or run's facts, and the conversation shows only the person's words.

use orchester_app::routes::OrchestratorChat;
use orchester_core::orchestrator::EntryKind;

use super::{
  harness::{Orchestrator, Setup},
  reading::ready_to_merge,
};

const INPUT: &str = "orchestrator-input";
const MENU: &str = "orchestrator-composer-mention-menu";

/// The field with the focus, holding `text` with the caret at its end.
fn write(screen: &mut Orchestrator, text: &str) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, text);
}

fn draft(screen: &mut Orchestrator) -> String {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .and_then(|element| element.as_text_input())
    .expect("the message field")
    .value()
}

fn row(name: &str) -> String {
  format!("orchestrator-composer-mention-{name}")
}

/// Kontur with KON-1 *Add retry to webhook sender* and KON-2 *Resend failed invitations*, on a new conversation.
fn kontur() -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  screen.draft_with(
    delivery,
    "Add retry to webhook sender",
    ("Retry on 429.\nHonour Retry-After.", &["Retries"]),
  );
  screen.draft_on(delivery, "Resend failed invitations");
  screen.go(OrchestratorChat::New);
  screen
}

#[test]
fn orchestrator_mentions_open_filter_insert_and_remove_by_keyboard() {
  let mut screen = kontur();
  write(&mut screen, "Compare @");
  screen.until("the @ menu with both tasks", |screen| {
    screen.exists(MENU) && screen.exists(&row("task-KON-1")) && screen.exists(&row("task-KON-2"))
  });
  write(&mut screen, "Compare @resend");
  screen.until("only KON-2 to match", |screen| {
    screen.exists(&row("task-KON-2")) && !screen.exists(&row("task-KON-1"))
  });
  assert!(
    screen.text(MENU).contains("1 match for “resend”"),
    "{}",
    screen.text(MENU)
  );
  screen.key(("Enter", "Enter"), false);
  screen.until("the mention in the draft", |screen| !screen.exists(MENU));
  assert_eq!(
    draft(&mut screen),
    "Compare @KON-2 ",
    "Enter inserts the mention and a space"
  );

  screen.key(("Backspace", "Backspace"), false);
  screen.key(("Backspace", "Backspace"), false);
  assert_eq!(
    draft(&mut screen),
    "Compare ",
    "Backspace after the chip removes it whole"
  );

  write(&mut screen, "Then @");
  screen.until("the menu", |screen| screen.exists(MENU));
  let (first, second) = if screen.text(&row("task-KON-1")).contains("Enter") {
    ("KON-1", "KON-2")
  } else {
    ("KON-2", "KON-1")
  };
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.until("the focus drawn on the second row", |screen| {
    screen.text(&row(&format!("task-{second}"))).contains("Enter")
      && !screen.text(&row(&format!("task-{first}"))).contains("Enter")
  });
  screen.key(("Enter", "Enter"), false);
  assert_eq!(
    draft(&mut screen),
    format!("Then @{second} "),
    "Enter inserts the row the focus is drawn on"
  );

  write(&mut screen, "Then @");
  screen.until("the menu", |screen| screen.exists(MENU));
  screen.key(("Tab", "Tab"), false);
  assert!(
    draft(&mut screen).starts_with("Then @KON-"),
    "Tab inserts too: {}",
    draft(&mut screen)
  );

  write(&mut screen, "Leave @payrol");
  screen.until("nothing to match", |screen| {
    screen.text(MENU).contains("Nothing matches “payrol”")
  });
  screen.key(("Escape", "Escape"), false);
  screen.until("the menu to close", |screen| !screen.exists(MENU));
  assert_eq!(draft(&mut screen), "Leave @payrol", "Esc keeps the text as typed");
}

/// The menu opens at the `@` (`gOs7u`): its left edge where the `@` starts, past the words before it, and its
/// bottom 8 over the composer's box, not on it.
#[test]
fn orchestrator_mentions_menu_opens_at_the_at_over_the_box() {
  let mut screen = kontur();
  write(&mut screen, "@");
  screen.until("the menu", |screen| screen.exists(MENU));
  let at_start = bounds(&mut screen, MENU);
  let field = bounds(&mut screen, INPUT);
  let boxed = bounds(&mut screen, "orchestrator-composer");
  assert!(
    (at_start.0 - field.0).abs() < 1.0,
    "an @ at the start opens the menu at the field's start: {at_start:?} {field:?}"
  );
  assert!(
    (boxed.1 - (at_start.1 + at_start.3) - 8.0).abs() < 0.5,
    "the menu's bottom is 8 over the box: {at_start:?} {boxed:?}"
  );

  write(&mut screen, "Compare the retries with @");
  screen.until("the menu after the words", |screen| {
    screen.exists(MENU) && bounds(screen, MENU).0 > at_start.0 + 40.0
  });
  let after = bounds(&mut screen, MENU);
  let boxed = bounds(&mut screen, "orchestrator-composer");
  assert!(
    (boxed.1 - (after.1 + after.3) - 8.0).abs() < 0.5,
    "still 8 over the box: {after:?} {boxed:?}"
  );
}

/// `id`'s x, y, width and height as last laid out.
fn bounds(screen: &mut Orchestrator, id: &str) -> (f32, f32, f32, f32) {
  let bounds = screen
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  (bounds.x, bounds.y, bounds.width, bounds.height)
}

#[test]
fn orchestrator_mentions_insert_by_pointer_and_reach_the_agent_with_facts() {
  let mut screen = kontur();
  write(&mut screen, "Look at @kon-1");
  screen.until("KON-1 in the menu", |screen| screen.exists(&row("task-KON-1")));
  screen.click(&row("task-KON-1"));
  screen.until("the mention in the draft", |screen| !screen.exists(MENU));
  assert_eq!(draft(&mut screen), "Look at @KON-1 ");

  screen.key(("Enter", "Enter"), true);
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted: Look at") && !screen.exists("orchestrator-thinking")
  });
  let sent = only_sent(&screen);
  assert!(sent.starts_with("Look at @KON-1"), "{sent}");
  assert!(
    sent.contains("<orchester-mentions>"),
    "the agent gets a marked block: {sent}"
  );
  assert!(
    sent.contains("task KON-1 \"Add retry to webhook sender\", workflow status Backlog."),
    "{sent}"
  );
  assert!(sent.contains("> Retry on 429."), "the brief's first lines: {sent}");
  let shown = screen.text("orchestrator-conversation");
  assert!(
    shown.contains("Look at KON-1"),
    "the person's message shows the mention as a link: {shown}"
  );
}

/// The project's checkout's files (ORC-316): tracked and untracked ones, never ignored ones; a file inserts as a chip,
/// and the agent gets its path in the block while the conversation links it by its name.
#[test]
fn orchestrator_mentions_offer_the_checkout_files_and_send_the_path() {
  let mut screen = kontur();
  for (path, words) in [
    (".gitignore", "target/\n"),
    ("crates/webhooks/src/sender.rs", "pub fn send() {}\n"),
    ("target/sender.log", "built\n"),
  ] {
    let file = screen.root.join(path);
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("its folder");
    std::fs::write(file, words).expect("the file");
  }
  let sender = row("file-crates/webhooks/src/sender.rs");
  write(&mut screen, "Check @sender");
  screen.until("the untracked file in the @ menu", |screen| screen.exists(&sender));
  assert!(
    !screen.exists(&row("file-target/sender.log")),
    "an ignored file never appears: {}",
    screen.text(MENU)
  );
  assert!(screen.text(MENU).contains("Files"), "{}", screen.text(MENU));
  write(&mut screen, "Check @README");
  screen.until("the tracked file", |screen| screen.exists(&row("file-README.md")));

  write(&mut screen, "Check @wbsend");
  screen.until("the file by the letters of its path in order", |screen| {
    screen.exists(&sender)
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the file's mention in the draft", |screen| !screen.exists(MENU));
  assert_eq!(draft(&mut screen), "Check @`crates/webhooks/src/sender.rs` ");

  screen.key(("Enter", "Enter"), true);
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted: Check") && !screen.exists("orchestrator-thinking")
  });
  let sent = only_sent(&screen);
  assert!(
    sent.contains("- @`crates/webhooks/src/sender.rs`: the file crates/webhooks/src/sender.rs, its path relative"),
    "the agent gets the path: {sent}"
  );
  assert!(!sent.contains("pub fn send"), "never the file's content: {sent}");
  let shown = screen.text("orchestrator-conversation");
  assert!(
    shown.contains("Check sender.rs"),
    "the file shows by its name as a link: {shown}"
  );
}

#[test]
fn orchestrator_mentions_insert_and_remove_a_run_and_reach_the_agent_with_its_facts() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (_, key) = ready_to_merge(&screen);
  screen.go(OrchestratorChat::New);
  let run = row(&format!("run-{key}-1"));
  write(&mut screen, "Compare @#1");
  screen.until("only the run to match", |screen| {
    screen.exists(&run) && !screen.exists(&row(&format!("task-{key}")))
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the run's mention in the draft", |screen| !screen.exists(MENU));
  assert_eq!(
    draft(&mut screen),
    format!("Compare @{key}#1 "),
    "Enter inserts the run"
  );
  screen.key(("Backspace", "Backspace"), false);
  screen.key(("Backspace", "Backspace"), false);
  assert_eq!(
    draft(&mut screen),
    "Compare ",
    "Backspace after the run's chip removes it whole"
  );

  write(&mut screen, "Look at @merge");
  screen.until("the task and its run in the menu", |screen| {
    screen.exists(&run) && screen.exists(&row(&format!("task-{key}")))
  });
  screen.click(&run);
  screen.until("the run's mention in the draft", |screen| !screen.exists(MENU));
  assert_eq!(draft(&mut screen), format!("Look at @{key}#1 "));

  screen.key(("Enter", "Enter"), true);
  screen.until("the answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted: Look at") && !screen.exists("orchestrator-thinking")
  });
  let sent = only_sent(&screen);
  assert!(sent.starts_with(&format!("Look at @{key}#1")), "{sent}");
  let branch = format!("orchester/{}", key.to_lowercase());
  assert!(
    sent.contains(&format!("- @{key}#1: run #1 of task {key} \"Merge me\", state ")),
    "the run's facts: {sent}"
  );
  assert!(sent.contains(&format!(", branch {branch}.")), "{sent}");
  let shown = screen.text("orchestrator-conversation");
  assert!(
    shown.contains(&format!("Look at {key} · #1")),
    "the person's message shows the run as a link: {shown}"
  );
}

/// The one message the person sent in the project's conversation, as it was kept for the agent.
fn only_sent(screen: &Orchestrator) -> String {
  let db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat.id();
  let kept: Vec<String> = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Person { text, .. } => Some(text.as_str().to_owned()),
      _ => None,
    })
    .collect();
  let [sent] = &kept[..] else {
    panic!("one message: {kept:?}");
  };
  sent.clone()
}
