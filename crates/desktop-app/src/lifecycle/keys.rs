//! The keyboard on the closing panel (`Keyboard on the closing panel` in `mKfVy`): Enter waits for the
//! agents to save, Esc cancels, Tab moves between Quit now, Cancel and Wait for agents to save. While the
//! panel is shown no other part of the window takes a key: the page under it is held as it was.

use lurq::{app::events::KeyboardEvent, core::Signal};
use orchester_workspace::close::{CloseFlow, ClosePhase};

/// A choice of the closing panel, in its Tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
  QuitNow,
  Cancel,
  Wait,
}

impl lurq::app::component::DevtoolsInspectable for Choice {}

impl Choice {
  /// The choices that can be pressed in `phase`, in Tab order: once the runners stopped starting new
  /// work, only Quit now.
  fn available(phase: &ClosePhase) -> &'static [Self] {
    match phase {
      ClosePhase::Confirming(_) => &[Self::QuitNow, Self::Cancel, Self::Wait],
      ClosePhase::Waiting { .. } => &[Self::QuitNow],
      ClosePhase::Open | ClosePhase::Stopping => &[],
    }
  }

  /// Presses it.
  pub fn press(self, close: &CloseFlow) {
    match self {
      Self::QuitNow => close.quit_now(),
      Self::Cancel => close.cancel(),
      Self::Wait => close.wait_for_agents(),
    }
  }
}

/// The window's keys while `close` holds it, with `focus` the choice Tab moved to; the window's own keys
/// otherwise. Set on the window's outermost element, so it sees every key before the page does.
pub fn keys(close: &CloseFlow, focus: &Signal<Option<Choice>>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let (close, focus) = (close.clone(), focus.clone());
  move |event: KeyboardEvent| {
    let phase = close.phase();
    if !phase.holds_window() {
      return;
    }
    event.prevent_default();
    event.stop_propagation();
    let available = Choice::available(&phase);
    let focused = focus.get_untracked().filter(|choice| available.contains(choice));
    match event.key.as_str() {
      "Escape" => Choice::Cancel.press(&close),
      "Enter" => match focused {
        Some(choice) => choice.press(&close),
        None if available.contains(&Choice::Wait) => Choice::Wait.press(&close),
        None => {}
      },
      " " => {
        if let Some(choice) = focused {
          choice.press(&close);
        }
      }
      "Tab" => focus.set(next(available, focused, event.shift)),
      _ => {}
    }
  }
}

/// The choice after `focused` in `available`, or before it with `back`, wrapping around.
fn next(available: &[Choice], focused: Option<Choice>, back: bool) -> Option<Choice> {
  let count = available.len();
  if count == 0 {
    return None;
  }
  let at = focused.and_then(|choice| available.iter().position(|available| *available == choice));
  let index = match (at, back) {
    (None, false) => 0,
    (None, true) => count - 1,
    (Some(at), false) => (at + 1) % count,
    (Some(at), true) => (at + count - 1) % count,
  };
  available.get(index).copied()
}
