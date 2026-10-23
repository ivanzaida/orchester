//! A code block in the Orchestrator's reply, its first line wider than the chat's column (ORC-82): the lines stay as
//! written, unwrapped, and scroll sideways inside the block's box under a swipe, the turn's width and the chat's
//! following of its end left as they were; a wheel up over the block scrolls the chat, and a selection copies the
//! long line whole.

use std::cell::Cell;

use lurq::{
  app::events::{MouseButton, ScrollPhase},
  core::ElementRect,
};

use super::{
  harness::Orchestrator,
  scroll::{long_conversation, newest_foot_gap, settle, turns},
};

/// The end of the code block's last line, to find its text by.
const LINE_END: &str = "END-OF-THE-LAST-LINE";

/// A line far wider than the chat's column.
fn long_line() -> String {
  format!(
    "{}THE-LONG-LINE-ENDS",
    "- the long line that runs past the chat's column ".repeat(12)
  )
}

/// Where the newest code block's lines are laid out: the reply's, which comes after the person's message it
/// answers.
fn code_text(screen: &mut Orchestrator) -> ElementRect {
  let holds = |text: Option<&str>| text.is_some_and(|text| text.contains(LINE_END));
  let count = Cell::new(0_usize);
  // Every match is passed over, so the walk counts them all.
  screen.window.tree.find_element(|element| {
    count.set(count.get() + usize::from(holds(element.text_content())));
    false
  });
  let left = Cell::new(count.get());
  screen
    .window
    .tree
    .find_element(|element| {
      if holds(element.text_content()) {
        left.set(left.get() - 1);
      }
      holds(element.text_content()) && left.get() == 0
    })
    .expect("the code block's lines are laid out")
    .bounds()
}

/// A swipe of `delta` with the pointer at `at`, as a trackpad sends it: a positive delta scrolls toward the start.
fn swipe(screen: &mut Orchestrator, (x, y): (f32, f32), (dx, dy): (f32, f32)) {
  let tree = &mut screen.window.tree;
  tree.mouse_move(x, y);
  tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  tree.scroll(x, y, dx, dy, ScrollPhase::Scroll);
  tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  settle(screen);
}

/// A long conversation whose newest reply holds a code block with a long line, at the chat's end: the newest
/// turn's id.
fn long_code() -> (Orchestrator, String) {
  let mut screen = long_conversation(2);
  screen.send(&format!(
    "The change:\n\n```markdown\n{}\nshort {LINE_END}\n```",
    long_line()
  ));
  screen.until("the answer", |screen| {
    !screen.exists("orchestrator-thinking") && screen.text("orchestrator-conversation").contains("Noted: The change")
  });
  settle(&mut screen);
  let newest = turns(&mut screen).last().expect("a turn is shown").clone();
  (screen, newest)
}

/// Asserts the chat sits at the very end of its newest turn and follows it.
fn at_end(screen: &mut Orchestrator, when: &str) {
  let gap = newest_foot_gap(screen);
  assert!(
    gap.abs() < 1.0,
    "{when}: the chat is at the newest turn's end, {gap} under it"
  );
  assert!(!screen.exists("conversation-jump"), "{when}: the chat follows its end");
}

/// Over the code block's lines, near the turn's start.
fn over_code(screen: &mut Orchestrator, turn: &str) -> (f32, f32) {
  let text = code_text(screen);
  (screen.bounds(turn).x + 60.0, text.y + text.height / 2.0)
}

#[test]
fn orchestrator_code_block_a_long_line_scrolls_sideways_inside_its_box() {
  let (mut screen, newest) = long_code();
  at_end(&mut screen, "the answer landed");
  let first = turns(&mut screen).first().expect("an earlier turn").clone();
  let (column, text) = (screen.bounds(&newest), code_text(&mut screen));
  assert!(
    (column.width - screen.bounds(&first).width).abs() < 0.5,
    "the turn keeps the chat's column width"
  );
  assert!(
    text.width > column.width * 2.0,
    "the long line is laid out whole, unwrapped: {} in a {} column",
    text.width,
    column.width
  );
  let at = over_code(&mut screen, &newest);
  swipe(&mut screen, at, (-200.0, 0.0));
  let moved = code_text(&mut screen);
  assert!(
    (moved.x - (text.x - 200.0)).abs() < 0.5,
    "the swipe scrolls the line: {} then {}",
    text.x,
    moved.x
  );
  assert!((moved.y - text.y).abs() < 0.5, "the chat does not move");
  assert_eq!(screen.bounds(&newest), column, "the turn keeps its place and width");
  at_end(&mut screen, "after a swipe inside the block");
  for _ in 0..20 {
    swipe(&mut screen, at, (-2000.0, 0.0));
  }
  let end = code_text(&mut screen);
  let (line_right, column_right) = (end.x + end.width, column.x + column.width);
  assert!(
    line_right <= column_right + 0.5 && line_right > column_right - 60.0,
    "the line's end comes into the box: it ends at {line_right}, the turn at {column_right}"
  );
}

#[test]
fn orchestrator_code_block_a_wheel_up_over_it_scrolls_the_chat() {
  let (mut screen, newest) = long_code();
  let text = code_text(&mut screen);
  let at = over_code(&mut screen, &newest);
  swipe(&mut screen, at, (0.0, 120.0));
  let moved = code_text(&mut screen);
  assert!(
    moved.y > text.y + 60.0,
    "the chat scrolls up under the wheel: the block at {} then {}",
    text.y,
    moved.y
  );
  assert!((moved.x - text.x).abs() < 0.5, "the block does not scroll sideways");
  assert!(screen.exists("conversation-jump"), "a scroll up stops following");
}

#[test]
fn orchestrator_code_block_a_selection_copies_the_long_line_whole() {
  let (mut screen, _) = long_code();
  let text = code_text(&mut screen);
  let (from_x, from_y) = (text.x + 1.0, text.y + 2.0);
  let (to_x, to_y) = (text.x + 300.0, text.y + text.height - 2.0);
  let tree = &mut screen.window.tree;
  tree.mouse_move(from_x, from_y);
  tree.mouse_down(from_x, from_y, MouseButton::Left);
  tree.mouse_move(to_x, to_y);
  tree.mouse_up(to_x, to_y, MouseButton::Left);
  screen.window.pass();
  let copied = screen.window.tree.selected_text().expect("the drag selects the code");
  assert!(
    copied.contains(&long_line()),
    "the long line is copied whole: {copied:?}"
  );
  assert!(copied.contains(LINE_END), "and the line under it: {copied:?}");
}
