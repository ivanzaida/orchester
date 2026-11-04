//! The Orchestrator's todo list (`Live todo list · redesign` `Ucmyu`): while it answers, the list its agent keeps for
//! the answer is pinned over the composer, *Orchestrator · answering* (08 `lEQgI`), and each update moves its items in
//! place; once the answer ends, the list folds into the answer's name line as a chip that opens it read only (09 g
//! `OoT5k`). An answer whose agent keeps no list shows neither. The lists are kept as the runner keeps its agent's
//! `TodoWrite` calls. `cargo test -p orchester-app --test orchestrator todo`.

use orchester_core::{
  connection::stream::{TodoItem, TodoState},
  id::ChatMessageId,
  orchestrator::{EntryKind, TurnState},
  resource::ModelId,
  time::Timestamp,
};

use super::harness::{Orchestrator, Setup};

fn item(text: &str, state: TodoState) -> TodoItem {
  TodoItem {
    text: text.to_owned(),
    state,
  }
}

/// The message whose turn the Orchestrator answers now.
fn thinking(screen: &Orchestrator) -> ChatMessageId {
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Person {
        message,
        turn: TurnState::Thinking,
        ..
      } => Some(message),
      _ => None,
    })
    .expect("a turn under way")
}

/// Keeps `items` as the answer's list, as the runner does, and tells the screen the conversation changed.
fn keep(screen: &mut Orchestrator, message: ChatMessageId, items: &[TodoItem]) {
  screen
    .db()
    .keep_chat_todo(message, items, Timestamp::now())
    .expect("kept");
  changed(screen);
}

fn changed(screen: &mut Orchestrator) {
  let root = screen.root.clone();
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .runners
    .chat_changed(&root);
}

#[test]
fn orchestrator_todo_list_is_pinned_while_it_answers_and_folds_into_the_answer() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(orchester_app::routes::OrchestratorChat::New);
  screen.send("Keep thinking about the queue.");
  screen.until("the answer under way", |screen| screen.exists("orchestrator-stop"));
  assert!(!screen.exists("orchestrator-plan"), "no list, no box");
  let message = thinking(&screen);
  let mut items = vec![
    item("Continue run #8 · KON-12", TodoState::InProgress),
    item("Continue run #11 · KON-16", TodoState::Pending),
    item("Draft the task for per-endpoint retries", TodoState::Pending),
  ];
  keep(&mut screen, message, &items);
  screen.until("the pinned list", |screen| screen.exists("orchestrator-plan"));
  assert_eq!(screen.text("orchestrator-plan-count"), "0 of 3");
  assert_eq!(screen.text("orchestrator-plan-owner"), "Orchestrator · answering");
  items[0].state = TodoState::Done;
  items[1].state = TodoState::InProgress;
  keep(&mut screen, message, &items);
  screen.until("the list updated in place", |screen| {
    screen.text("orchestrator-plan-count") == "1 of 3"
  });
  // The answer said something, so it has a name line once it ends.
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let model = ModelId::new("haiku").expect("model");
  screen
    .db()
    .add_chat_reply(chat, ("On it.", &model), Timestamp::now())
    .expect("kept");
  changed(&mut screen);
  screen.click("orchestrator-stop");
  let chip = format!("orchestrator-plan-chip-{message}");
  screen.until("the list folded into the answer", |screen| {
    !screen.exists("orchestrator-plan") && screen.exists(&chip)
  });
  assert_eq!(screen.text(&format!("{chip}-label")), "Plan · 1 of 3");
  let list = format!("orchestrator-plan-list-{message}");
  assert!(!screen.exists(&list), "folded until pressed");
  screen.press(&chip);
  screen.until("the list as it ended", |screen| screen.exists(&list));
  assert!(
    screen.text(&list).contains("Draft the task for per-endpoint retries"),
    "{}",
    screen.text(&list)
  );
}
