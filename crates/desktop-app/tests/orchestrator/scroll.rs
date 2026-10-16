//! Scrolling a long conversation: the turns stay where the person scrolled them through later passes, idle or
//! with something to redraw, also in a conversation opened again whose newest turn is taller than the view (the
//! list reveals that turn when it opens), and a conversation at its end still shows a new turn.

use lurq::{app::events::ScrollPhase, node::element::ElementRef};
use orchester_app::routes::OrchestratorChat;

use super::harness::{Orchestrator, Setup};

/// Paragraphs in each message: a turn, the message and its answer, runs taller than the view.
const PARAGRAPHS: usize = 10;

/// A message of [`PARAGRAPHS`] paragraphs, each a few lines at the column's width; `tag` names it.
fn long_message(tag: &str) -> String {
  (1..=PARAGRAPHS)
    .map(|paragraph| {
      format!(
        "{tag} paragraph {paragraph}: the composer keeps what the person wrote while they go to Tasks and back, \
         each project and each conversation its own draft, and a failed send leaves the words in place."
      )
    })
    .collect::<Vec<_>>()
    .join("\n\n")
}

/// A new conversation of `turns` long turns, each answered.
fn long_conversation(turns: usize) -> Orchestrator {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  for turn in 1..=turns {
    let tag = format!("Turn {turn}");
    screen.send(&long_message(&tag));
    let answer = format!("Noted: {tag} paragraph 1");
    screen.until("the answer", |screen| {
      !screen.exists("orchestrator-thinking") && screen.text("orchestrator-conversation").contains(&answer)
    });
  }
  settle(&mut screen);
  screen
}

/// Lets the window run a few passes with nothing new from the person.
fn settle(screen: &mut Orchestrator) {
  for _ in 0..10 {
    screen.window.tree.tick_futures();
    screen.window.pass();
  }
}

/// Turns the wheel over the turns: up (`delta` above 0) scrolls toward the start.
fn wheel(screen: &mut Orchestrator, delta: f32) {
  let frame = screen.bounds("orchestrator-scroll");
  let (x, y) = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
  let tree = &mut screen.window.tree;
  tree.mouse_move(x, y);
  tree.scroll(x, y, 0.0, delta, ScrollPhase::Scroll);
  screen.window.pass();
  screen.window.pass();
}

/// The ids of the turns built, first to newest.
fn turns(screen: &mut Orchestrator) -> Vec<String> {
  fn walk(element: ElementRef<'_>, found: &mut Vec<String>) {
    if let Some(id) = element.id().filter(|id| id.starts_with("orchestrator-turn-")) {
      found.push(id.to_owned());
    }
    for child in element.children() {
      walk(child, found);
    }
  }
  let mut found = Vec::new();
  if let Some(scroll) = screen.window.tree.get_element_by_id("orchestrator-scroll") {
    walk(scroll, &mut found);
  }
  found
}

/// How far the turns' view ends below the newest turn's foot: 0 or more when the newest turn ends in view.
fn newest_foot_gap(screen: &mut Orchestrator) -> f32 {
  let ids = turns(screen);
  let newest = ids.last().expect("a turn is shown").clone();
  let turn = screen.bounds(&newest);
  let scroll = screen.bounds("orchestrator-scroll");
  scroll.y + scroll.height - (turn.y + turn.height)
}

/// Where the newest turn's top sits below the top of the turns' view.
fn newest_top(screen: &mut Orchestrator) -> f32 {
  let newest = turns(screen).last().expect("a turn is shown").clone();
  screen.bounds(&newest).y - screen.bounds("orchestrator-scroll").y
}

/// Where the first turn's top sits below the top of the turns' view, when it is built.
fn first_top(screen: &mut Orchestrator) -> Option<f32> {
  let first = turns(screen).first()?.clone();
  Some(screen.bounds(&first).y - screen.bounds("orchestrator-scroll").y)
}

/// Scrolls the turns up from where they are and checks they stay there through idle passes, a redraw for a draft
/// written in the composer, and a scroll to the start.
fn scrolls_and_holds(screen: &mut Orchestrator) {
  let before = newest_top(screen);
  wheel(screen, 400.0);
  let scrolled = newest_top(screen);
  assert!(
    scrolled > before + 300.0,
    "the wheel scrolled the turns up: {before} then {scrolled}"
  );
  settle(screen);
  let idle = newest_top(screen);
  assert!(
    (idle - scrolled).abs() < 1.0,
    "idle passes keep the scroll: {scrolled} then {idle}"
  );
  // Writing in the composer redraws the screen; the turns stay where they were.
  screen.set("orchestrator-input", "another bug");
  settle(screen);
  let typed = newest_top(screen);
  assert!(
    (typed - scrolled).abs() < 1.0,
    "a draft keeps the scroll: {scrolled} then {typed}"
  );
  for _ in 0..10 {
    wheel(screen, 5000.0);
  }
  settle(screen);
  let top = first_top(screen).expect("the first turn is built at the start");
  assert!(
    top.abs() < 1.0,
    "the start stays in view: the first turn's top at {top}"
  );
}

#[test]
fn orchestrator_scroll_holds_where_the_person_scrolled() {
  let mut screen = long_conversation(3);
  scrolls_and_holds(&mut screen);
}

/// The person's report: the conversation opened again, its newest turn taller than the view, and every scroll up
/// came back to that turn's top.
#[test]
fn orchestrator_scroll_holds_in_a_reopened_conversation_with_a_tall_newest_turn() {
  let mut screen = long_conversation(3);
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  screen.settings();
  screen.go(OrchestratorChat::Chat(chat));
  screen.until("the newest turn", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: Turn 3 paragraph 1")
  });
  settle(&mut screen);
  let newest = turns(&mut screen).last().expect("a turn is shown").clone();
  assert!(
    screen.bounds(&newest).height > screen.bounds("orchestrator-scroll").height,
    "the newest turn is taller than the view"
  );
  let top = newest_top(&mut screen);
  assert!(
    top.abs() < 1.0,
    "the conversation opens on its newest turn: its top at {top}"
  );
  scrolls_and_holds(&mut screen);
}

#[test]
fn orchestrator_scroll_at_the_end_shows_a_new_turn() {
  let mut screen = long_conversation(2);
  for _ in 0..10 {
    wheel(&mut screen, -5000.0);
  }
  settle(&mut screen);
  let gap = newest_foot_gap(&mut screen);
  assert!(gap.abs() < 1.0, "the chat is at its end: {gap}");
  screen.send("What is running?");
  screen.until("the third answer", |screen| {
    !screen.exists("orchestrator-thinking")
      && screen
        .text("orchestrator-conversation")
        .contains("Noted: What is running?")
  });
  settle(&mut screen);
  let gap = newest_foot_gap(&mut screen);
  assert!(gap.abs() < 1.0, "the new turn ends in view, at the chat's end: {gap}");
}
