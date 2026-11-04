//! Middle-button autoscroll in the conversation (lurq's, turned on by `ui::chat::follow` for the chats alone): held
//! with the pointer above the press, it scrolls the turns up, faster the farther the pointer is, and the chat stops
//! following; the release or Esc stops it where it is, and back down at the end the chat follows again.

use super::{
  harness::Orchestrator,
  scroll::{long_conversation, newest_foot_gap, newest_top, settle},
  support::autoscroll::Autoscroll,
};

/// Where the middle button goes down: by the turns' left edge, halfway down, clear of any link.
fn press_point(screen: &mut Orchestrator) -> (f32, f32) {
  let frame = screen.bounds("orchestrator-scroll");
  (frame.x + 12.0, frame.y + frame.height / 2.0)
}

/// How far an autoscroll held `frames` frames with the pointer `offset` from the press moves the turns down the
/// view: above 0 when it scrolls toward the start.
fn scrolled(screen: &mut Orchestrator, drag: &mut Autoscroll, offset: f32, frames: usize) -> f32 {
  let at = press_point(screen);
  let before = newest_top(screen);
  drag.hold(&mut screen.window, at, offset, frames);
  drag.release(&mut screen.window);
  newest_top(screen) - before
}

#[test]
fn orchestrator_autoscroll_scrolls_faster_the_farther_and_back_at_the_end_follows_again() {
  let mut screen = long_conversation(3);
  let mut drag = Autoscroll::new(&mut screen.window);
  let slow = scrolled(&mut screen, &mut drag, -40.0, 12);
  assert!(slow > 5.0, "the pointer above the press scrolls the turns up: {slow}");
  assert!(
    screen.exists("conversation-jump"),
    "scrolled up, the chat no longer follows"
  );
  let fast = scrolled(&mut screen, &mut drag, -150.0, 12);
  assert!(
    fast > slow * 3.0,
    "farther from the press, faster: {slow} at 40 px, {fast} at 150 px"
  );
  let released = newest_top(&mut screen);
  drag.frames(&mut screen.window, 10);
  settle(&mut screen);
  let after = newest_top(&mut screen);
  assert!(
    (after - released).abs() < 1.0,
    "the release leaves the chat where it was: {released} then {after}"
  );
  let at = press_point(&mut screen);
  drag.hold(&mut screen.window, at, 150.0, 60);
  drag.release(&mut screen.window);
  settle(&mut screen);
  let gap = newest_foot_gap(&mut screen);
  assert!(gap.abs() < 1.0, "the autoscroll down reached the end: {gap} under it");
  assert!(
    !screen.exists("conversation-jump"),
    "back at the end, the chat follows again"
  );
}

#[test]
fn orchestrator_autoscroll_stops_at_esc_with_the_button_still_held() {
  let mut screen = long_conversation(3);
  let mut drag = Autoscroll::new(&mut screen.window);
  let at = press_point(&mut screen);
  let start = newest_top(&mut screen);
  drag.hold(&mut screen.window, at, -150.0, 6);
  drag.escape(&mut screen.window);
  let stopped = newest_top(&mut screen);
  assert!(stopped > start + 5.0, "it scrolled up: {start} then {stopped}");
  drag.frames(&mut screen.window, 10);
  let held = newest_top(&mut screen);
  assert!(
    (held - stopped).abs() < 1.0,
    "after Esc, the held button scrolls no more: {stopped} then {held}"
  );
  drag.release(&mut screen.window);
  let released = newest_top(&mut screen);
  assert!(
    (released - stopped).abs() < 1.0,
    "nor does the release: {stopped} then {released}"
  );
}
