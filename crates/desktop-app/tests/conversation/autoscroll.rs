//! Middle-button autoscroll in a task's Conversation (lurq's, turned on by `ui::chat::follow` for the chats alone):
//! held with the pointer above the press, it scrolls the timeline up, faster the farther the pointer is, and the chat
//! stops following; the release leaves it where it is, and back down at the end the chat follows again.

use super::{Window, ids, retry_webhooks, support::autoscroll::Autoscroll};

fn bounds(window: &mut Window, id: &str) -> lurq::core::ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

/// Where the middle button goes down: by the chat's left edge, halfway down, clear of any link.
fn press_point(window: &mut Window) -> (f32, f32) {
  let chat = bounds(window, "conversation-chat");
  (chat.x + 12.0, chat.y + chat.height / 2.0)
}

/// The timeline's row whose top is the first in view.
fn top_row(window: &mut Window) -> String {
  let top = bounds(window, "conversation-chat").y;
  ids(window, "conversation-row:")
    .into_iter()
    .map(|id| (bounds(window, &id).y, id))
    .filter(|(y, _)| *y >= top)
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .map(|(_, id)| id)
    .expect("a row in view")
}

/// How far an autoscroll held `frames` frames with the pointer `offset` from the press moves the timeline down the
/// view: above 0 when it scrolls toward the start.
fn scrolled(window: &mut Window, drag: &mut Autoscroll, offset: f32, frames: usize) -> f32 {
  let (at, row) = (press_point(window), top_row(window));
  let before = bounds(window, &row).y;
  drag.hold(window, at, offset, frames);
  drag.release(window);
  bounds(window, &row).y - before
}

#[test]
fn timeline_autoscroll_scrolls_faster_the_farther_and_back_at_the_end_follows_again() {
  let (mut window, _) = retry_webhooks();
  let mut drag = Autoscroll::new(&mut window);
  let slow = scrolled(&mut window, &mut drag, -40.0, 12);
  assert!(
    slow > 5.0,
    "the pointer above the press scrolls the timeline up: {slow}"
  );
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_some(),
    "scrolled up, the chat no longer follows"
  );
  let fast = scrolled(&mut window, &mut drag, -150.0, 12);
  assert!(
    fast > slow * 3.0,
    "farther from the press, faster: {slow} at 40 px, {fast} at 150 px"
  );
  let row = top_row(&mut window);
  let released = bounds(&mut window, &row).y;
  drag.frames(&mut window, 10);
  let after = bounds(&mut window, &row).y;
  assert!(
    (after - released).abs() < 1.0,
    "the release leaves the chat where it was: {released} then {after}"
  );
  let at = press_point(&mut window);
  drag.hold(&mut window, at, 150.0, 60);
  drag.release(&mut window);
  drag.frames(&mut window, 5);
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_none(),
    "back at the end, the chat follows again"
  );
}
