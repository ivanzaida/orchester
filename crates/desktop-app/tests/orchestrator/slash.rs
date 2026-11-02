//! The slash menu in the Orchestrator's composer (ORC-321, `07` `jAiyu`, `07b` `m85qp`, `07d` `R8eum`): `/` at the
//! start of the draft opens the chat's own commands, typing filters them, ↑ ↓ and Enter or a click run one as its
//! control does (*New conversation* as the header's *New*, *History* as its toggle), and the typed `/` goes; a command
//! that cannot run is greyed with why; Esc closes the menu and keeps the text; a `/` inside text is just text. Compact
//! and Model and effort are not offered until their controls exist.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::time::Timestamp;

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
};

const INPUT: &str = "orchestrator-input";
const MENU: &str = "orchestrator-composer-slash-menu";
const NEW: &str = "orchestrator-composer-slash-new-conversation";
const HISTORY: &str = "orchestrator-composer-slash-history";

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

/// The Orchestrator on a kept conversation.
fn on_a_conversation() -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  kept(&screen, "Order of the auth work", Timestamp::now(), "Auth first.");
  screen.go(OrchestratorChat::Latest);
  screen.until("the conversation", |screen| {
    screen.text("orchestrator-conversation").contains("Auth first.")
  });
  screen
}

#[test]
fn orchestrator_slash_lists_the_chats_commands_and_runs_history_by_keyboard() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/");
  screen.until("the slash menu", |screen| {
    screen.exists(MENU) && screen.exists(NEW) && screen.exists(HISTORY)
  });
  let shown = screen.text(MENU);
  assert!(shown.starts_with("Commands"), "{shown}");
  assert!(shown.contains("Type to filter"), "{shown}");
  for missing in ["Compact", "Model and effort", "Permissions", "Stop turn"] {
    assert!(
      !shown.contains(missing),
      "no {missing} until its control exists: {shown}"
    );
  }
  assert!(screen.text(NEW).contains("Enter"), "the focus starts on the first row");
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.until("the focus on History", |screen| {
    screen.text(HISTORY).contains("Enter") && !screen.text(NEW).contains("Enter")
  });
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.until("the focus back on the first row", |screen| {
    screen.text(NEW).contains("Enter")
  });

  write(&mut screen, "/hist");
  screen.until("only History to match", |screen| {
    screen.exists(HISTORY) && !screen.exists(NEW)
  });
  let shown = screen.text(MENU);
  assert!(shown.contains("1 of 2"), "{shown}");
  assert!(shown.contains("/hist"), "{shown}");
  screen.key(("Enter", "Enter"), false);
  screen.until("History open", |screen| {
    screen.exists("orchestrator-history-panel") && !screen.exists(MENU)
  });
  assert_eq!(draft(&mut screen), "", "the typed / goes");
}

#[test]
fn orchestrator_slash_new_conversation_is_greyed_on_a_new_one_and_runs_on_a_kept_one() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/new");
  screen.until("New conversation", |screen| screen.exists(NEW));
  screen.click(NEW);
  screen.until("a new conversation", |screen| {
    screen.route() == Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::New))
  });
  assert!(!screen.exists(MENU), "the menu closes");

  write(&mut screen, "/");
  screen.until("the slash menu", |screen| screen.exists(NEW));
  let row = screen.text(NEW);
  assert!(row.contains("This is a new conversation already"), "why: {row}");
  assert!(!row.contains("Enter"), "it cannot take the focus: {row}");
  assert!(
    screen.text(HISTORY).contains("Enter"),
    "the focus starts on the first row that can run"
  );
  screen.click(NEW);
  assert!(screen.exists(MENU), "a greyed command does nothing");
  assert_eq!(draft(&mut screen), "/");
}

#[test]
fn orchestrator_slash_is_text_elsewhere_and_esc_keeps_it() {
  let mut screen = on_a_conversation();
  write(&mut screen, "Ask /new");
  screen.window.pass();
  assert!(!screen.exists(MENU), "a / inside text is text");
  write(&mut screen, "/new conversation");
  screen.window.pass();
  assert!(!screen.exists(MENU), "a space ends the command");

  write(&mut screen, "/hi");
  screen.until("the slash menu", |screen| screen.exists(MENU));
  screen.key(("Escape", "Escape"), false);
  screen.until("the menu to close", |screen| !screen.exists(MENU));
  assert_eq!(draft(&mut screen), "/hi", "Esc keeps the text as typed");
  assert!(!screen.exists("orchestrator-history-panel"), "nothing ran");

  write(&mut screen, "/hin");
  screen.window.pass();
  assert!(!screen.exists(MENU), "it stays closed while that / is typed on");
  // Once the draft no longer starts with `/`, a new `/` opens it again.
  write(&mut screen, "");
  write(&mut screen, "/deploy");
  screen.until("nothing to match", |screen| {
    screen.text(MENU).contains("No command matches /deploy")
  });
  let shown = screen.text(MENU);
  assert!(shown.contains("Send as text"), "{shown}");
  screen.key(("Enter", "Enter"), false);
  screen.until("the text sent as typed", |screen| {
    screen.text("orchestrator-conversation").contains("Noted: /deploy")
  });
}

/// The menu opens over the composer's box (`jAiyu`): 640 wide, its left edge at the box's, its bottom 8 over it.
#[test]
fn orchestrator_slash_menu_opens_over_the_box() {
  let mut screen = on_a_conversation();
  write(&mut screen, "/");
  screen.until("the slash menu", |screen| screen.exists(MENU));
  screen.window.pass();
  let menu = screen.bounds(MENU);
  let boxed = screen.bounds("orchestrator-composer");
  assert!((menu.width - 640.0).abs() < 0.5, "{menu:?}");
  assert!((menu.x - boxed.x).abs() < 0.5, "{menu:?} {boxed:?}");
  assert!(
    (boxed.y - (menu.y + menu.height) - 8.0).abs() < 0.5,
    "{menu:?} {boxed:?}"
  );
}
