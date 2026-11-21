//! How everything that turns moves (`ui::loader`, `ui::typing`, and lurq's own spinners): in [`SPIN_STEPS`] poses a
//! second, lurq drawing a frame only when the pose changes, and still where the system asks for reduced motion. And
//! the typing dots (`Typing` `D231t`, `Dot` `h6EkpB`): three 6 dots in `text_muted`, 4 apart, 4 under what is over
//! them and 2 over what is under them.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, Theme},
  ui::{Length, SpinMotion, Spinner, SpinnerPart},
};

use crate::theme::{ExtraRadius, ExtraSpacing, SPIN_STEPS};

/// The state of the typing dots.
pub const TYPING: &str = "typing";

/// A dot's side (`h6EkpB`).
const DOT_SIZE: f32 = 6.0;

pub(super) fn install(theme: &Theme) {
  theme.set_spin_motion(SpinMotion::stepped(SPIN_STEPS).still_when_reduced(true));
  theme.patch_recipe::<Spinner>(|rules| {
    rules
      .state(TYPING, SpinnerPart::Root, |style| {
        style
          .padding_top(SpacingSize::Xs)
          .padding_bottom(ExtraSpacing::Tight.role())
      })
      .state(TYPING, SpinnerPart::Indicator, |style| {
        style.height(Length::Auto).gap(SpacingSize::Xs)
      })
      .state(TYPING, SpinnerPart::Dot, |style| {
        style
          .square(DOT_SIZE)
          .radius(ExtraRadius::Full.role())
          .background(PaletteColor::MutedForeground)
      })
  });
}
