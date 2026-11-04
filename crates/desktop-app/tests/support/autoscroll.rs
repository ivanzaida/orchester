//! A middle-button autoscroll over a chat (lurq's, turned on for the chats by `ui::chat::follow`), on a clock the test
//! moves a frame at a time, so how far it scrolls depends on the frames drawn and not on how fast the machine is.

use std::time::{Duration, Instant};

use lurq::app::events::MouseButton;
use orchester_app::test_support::Window;

/// One frame at 60 Hz.
const FRAME: Duration = Duration::from_millis(16);

/// The window's clock while the test drives it.
pub struct Autoscroll {
  now: Instant,
  /// Where the button went down, and the pointer's offset from it.
  held: Option<((f32, f32), f32)>,
}

impl Autoscroll {
  /// Puts `window` on the test's clock.
  pub fn new(window: &mut Window) -> Self {
    let now = Instant::now();
    window.tree.set_animation_clock(Some(now));
    Self { now, held: None }
  }

  /// Presses the middle button at `at`, moves the pointer `offset` below it (above, when below 0) and draws
  /// `frames` frames with the button held.
  pub fn hold(&mut self, window: &mut Window, at: (f32, f32), offset: f32, frames: usize) {
    let (x, y) = at;
    window.tree.mouse_move(x, y);
    window.tree.mouse_down(x, y, MouseButton::Middle);
    window.tree.mouse_move(x, y + offset);
    self.held = Some((at, offset));
    self.frames(window, frames);
  }

  /// Lets the middle button go where the pointer is, and draws a few frames.
  pub fn release(&mut self, window: &mut Window) {
    let ((x, y), offset) = self.held.take().expect("the middle button is held");
    window.tree.mouse_up(x, y + offset, MouseButton::Middle);
    self.frames(window, 3);
  }

  /// Presses Esc, the button still held, and draws a few frames.
  pub fn escape(&mut self, window: &mut Window) {
    window
      .tree
      .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
    window
      .tree
      .key_up("Escape".to_owned(), "Escape".to_owned(), false, false, false);
    self.frames(window, 3);
  }

  /// Draws `count` frames, a frame's time apart.
  pub fn frames(&mut self, window: &mut Window, count: usize) {
    for _ in 0..count {
      self.now += FRAME;
      window.tree.set_animation_clock(Some(self.now));
      window.tree.tick_futures();
      window.pass();
    }
  }
}
