//! The first message of a new conversation settles into place in one step (ORC-711): from the frame the send shows it,
//! every frame shows the conversation's list with that message's one row, under the same element and at the same
//! place, until its turn has ended; no frame goes back to the start, to the conversation's bones or to an empty list.

use orchester_app::routes::OrchestratorChat;
use orchester_core::orchestrator::EntryKind;

use super::harness::{Orchestrator, Setup};

/// What one frame shows of the chat.
#[derive(Debug, PartialEq)]
struct Frame {
  /// The new conversation's start (`super::start`).
  start: bool,
  /// The conversation's bones, or the header's *Loading conversation…*, while it is read.
  loading: bool,
  list: bool,
  /// The element ids of the turn rows, with the top of each.
  turns: Vec<(String, i32)>,
}

impl Frame {
  fn of(screen: &mut Orchestrator) -> Self {
    let ids = screen
      .window
      .tree
      .get_element_by_id("orchestrator-chat")
      .map(|chat| {
        let mut ids = Vec::new();
        turn_ids(chat, &mut ids);
        ids
      })
      .unwrap_or_default();
    let turns = ids
      .into_iter()
      .map(|id| {
        let top = screen.bounds(&id).y.round() as i32;
        (id, top)
      })
      .collect();
    Self {
      start: screen.exists("orchestrator-start"),
      loading: screen.exists("orchestrator-loading") || screen.text("orchestrator-title") == "Loading conversation…",
      list: screen.exists("orchestrator-chat"),
      turns,
    }
  }
}

fn turn_ids(element: lurq::node::element::ElementRef<'_>, into: &mut Vec<String>) {
  for child in element.children() {
    if let Some(id) = child.id().filter(|id| id.starts_with("orchestrator-turn-")) {
      into.push(id.to_owned());
    }
    turn_ids(child, into);
  }
}

#[test]
fn orchestrator_first_send_settles_in_one_step() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  assert!(
    screen.exists("orchestrator-start"),
    "a new conversation opens on its start"
  );
  screen.send("What is running?");
  let mut frames = vec![Frame::of(&mut screen)];
  screen.until("the turn's end", |screen| {
    let frame = Frame::of(screen);
    let ended = !screen.exists("orchestrator-thinking") && screen.text("orchestrator-chat").contains("Delivered ·");
    frames.push(frame);
    ended
  });
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let message = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .expect("the message is kept");
  let row = format!("orchestrator-turn-{message}");
  let top = frames[0].turns.first().map_or(0, |(_, top)| *top);
  let settled = Frame {
    start: false,
    loading: false,
    list: true,
    turns: vec![(row, top)],
  };
  // The frames one after another, each told once, say what the chat went through when it did not settle at once.
  let mut went = frames.iter().collect::<Vec<_>>();
  went.dedup();
  assert!(
    frames.iter().all(|frame| *frame == settled),
    "every frame from the send shows the message's one row, under its own element, where the send put it: \
     {settled:?}; the chat went through {went:#?}"
  );
}
