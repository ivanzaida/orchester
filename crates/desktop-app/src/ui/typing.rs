//! The typing indicator: three dots under who is about to write (`Message · waiting` `sXET0`, `Typing` `D231t`,
//! `Dot` `h6EkpB`), while nothing arrived yet. It is lurq's `ui::Spinner` of dots in the theme's recipe and spin motion
//! (`theme::kit::spinner`): the design draws them still, fading from the first (1, 0.6 and 0.3); lurq runs a wave
//! along them on its animation clock, in the theme's steps, and leaves them as drawn where the system asks for
//! reduced motion. It asks for frames only while dots are in the window's tree.

use lurq::{
  node::Element,
  ui::{Spinner, SpinnerKind},
};

use crate::theme::kit::spinner::TYPING;

/// Three dots under `id`, as `{id}-dots`, moving while nothing was written yet.
pub fn dots(id: &str) -> Element {
  Spinner::new()
    .kind(SpinnerKind::Dots)
    .state(TYPING, true)
    // What is being written says so beside them; the dots say nothing of their own to tooling.
    .decorative(true)
    .id(format!("{id}-dots"))
    .into()
}
