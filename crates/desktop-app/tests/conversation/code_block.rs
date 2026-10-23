//! A code block in what an agent wrote, its first line wider than the chat's column (ORC-82): the lines stay as
//! written, unwrapped, and scroll sideways inside the block's box under a swipe, the chat's column and its
//! following of the end left as they were; a wheel up over the block scrolls the chat, and a selection copies the
//! long line whole.

use lurq::{
  app::events::{MouseButton, ScrollPhase},
  core::ElementRect,
};

use super::{Window, agent_text::agent_says, key_of, open, seeded, with_text};

/// The end of the code block's last line, to find its text by.
const LINE_END: &str = "END-OF-THE-LAST-LINE";

/// A line far wider than the chat's column.
fn long_line() -> String {
  format!(
    "{}THE-LONG-LINE-ENDS",
    "- the long line that runs past the chat's column ".repeat(12)
  )
}

/// Where the code block's lines are laid out.
fn code_text(window: &mut Window) -> ElementRect {
  window
    .tree
    .find_element(|element| element.text_content().is_some_and(|text| text.contains(LINE_END)))
    .expect("the code block's lines are laid out")
    .bounds()
}

fn bounds(window: &mut Window, id: &str) -> ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// A swipe of `delta` with the pointer at `at`, as a trackpad sends it: a positive delta scrolls toward the start.
fn swipe(window: &mut Window, (x, y): (f32, f32), (dx, dy): (f32, f32)) {
  window.tree.mouse_move(x, y);
  window.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  window.tree.scroll(x, y, dx, dy, ScrollPhase::Scroll);
  window.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  for _ in 0..4 {
    window.pass();
  }
}

fn following(window: &mut Window) -> bool {
  window.tree.get_element_by_id("conversation-jump").is_none()
}

/// KON-12's chat at its end, its agent's newest message a code block with a long line: the message's id.
fn long_code() -> (Window, String) {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  agent_says(
    &seeded,
    &key,
    &format!("The change:\n\n```markdown\n{}\nshort {LINE_END}\n```", long_line()),
  );
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the code block", |window| {
    window
      .tree
      .find_element(|element| element.text_content().is_some_and(|text| text.contains(LINE_END)))
      .is_some()
  });
  for _ in 0..10 {
    window.pass();
  }
  let message = with_text(&mut window, "conversation-text-", LINE_END);
  (window, message)
}

/// Over the code block's lines, near the message's start.
fn over_code(window: &mut Window, message: &str) -> (f32, f32) {
  let text = code_text(window);
  (bounds(window, message).x + 40.0, text.y + text.height / 2.0)
}

#[test]
fn conversation_code_block_a_long_line_scrolls_sideways_inside_its_box() {
  let (mut window, message) = long_code();
  assert!(following(&mut window), "the chat opens at its end, following");
  let (column, text) = (bounds(&mut window, &message), code_text(&mut window));
  assert!(
    text.width > column.width * 2.0,
    "the long line is laid out whole, unwrapped: {} in a {} column",
    text.width,
    column.width
  );
  let at = over_code(&mut window, &message);
  swipe(&mut window, at, (-200.0, 0.0));
  let moved = code_text(&mut window);
  assert!(
    (moved.x - (text.x - 200.0)).abs() < 0.5,
    "the swipe scrolls the line: {} then {}",
    text.x,
    moved.x
  );
  assert!((moved.y - text.y).abs() < 0.5, "the chat does not move");
  assert_eq!(
    bounds(&mut window, &message),
    column,
    "the message keeps its place and width"
  );
  assert!(
    following(&mut window),
    "a swipe inside the block leaves the chat following"
  );
  for _ in 0..20 {
    swipe(&mut window, at, (-2000.0, 0.0));
  }
  let end = code_text(&mut window);
  let (line_right, column_right) = (end.x + end.width, column.x + column.width);
  assert!(
    line_right <= column_right + 0.5 && line_right > column_right - 40.0,
    "the line's end comes into the box: it ends at {line_right}, the column at {column_right}"
  );
}

#[test]
fn conversation_code_block_a_wheel_up_over_it_scrolls_the_chat() {
  let (mut window, message) = long_code();
  let text = code_text(&mut window);
  let at = over_code(&mut window, &message);
  swipe(&mut window, at, (0.0, 120.0));
  let moved = code_text(&mut window);
  assert!(
    moved.y > text.y + 60.0,
    "the chat scrolls up under the wheel: the block at {} then {}",
    text.y,
    moved.y
  );
  assert!((moved.x - text.x).abs() < 0.5, "the block does not scroll sideways");
  assert!(!following(&mut window), "a scroll up stops following");
}

#[test]
fn conversation_code_block_a_selection_copies_the_long_line_whole() {
  let (mut window, _) = long_code();
  let text = code_text(&mut window);
  let (from_x, from_y) = (text.x + 1.0, text.y + 2.0);
  let (to_x, to_y) = (text.x + 300.0, text.y + text.height - 2.0);
  window.tree.mouse_move(from_x, from_y);
  window.tree.mouse_down(from_x, from_y, MouseButton::Left);
  window.tree.mouse_move(to_x, to_y);
  window.tree.mouse_up(to_x, to_y, MouseButton::Left);
  window.pass();
  let copied = window.tree.selected_text().expect("the drag selects the code");
  assert!(
    copied.contains(&long_line()),
    "the long line is copied whole: {copied:?}"
  );
  assert!(copied.contains(LINE_END), "and the line under it: {copied:?}");
}
