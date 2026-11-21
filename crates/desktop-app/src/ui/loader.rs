//! The loader (`loader-circle`) turns while it is shown: [`icon`](super::icon::icon) gives every loader icon this
//! turn, so running work never looks stuck. It is lurq's spin (`lurq::ui::SPIN`, once a [`SPIN_PERIOD`]) in the
//! theme's [`SpinMotion`](lurq::ui::SpinMotion) (`theme::kit::spinner`): [`SPIN_STEPS`](crate::theme::SPIN_STEPS)
//! poses a second, lurq drawing a frame only when the pose changes, and still where the system asks for reduced
//! motion. lurq plays the turn on its one animation clock, every loader in phase, and asks for frames only while a
//! turning node is in the window's tree. The kit's icon is a `Text` its 851 call sites chain, so the turn is laid on
//! it here as lurq's `ui::Icon` lays it on its own.

use lurq::{
  animation::{Animation, Easing, StepPosition},
  components::Text,
  ui::{SPIN, SPIN_PERIOD, current_theme},
};

/// The loader `glyph`, turning about its centre as the theme's spin motion says for as long as it is shown.
/// `JumpEnd` starts on the still pose and stops one step short of the full turn, so no pose is drawn twice.
pub(crate) fn turning(glyph: Text) -> Text {
  let theme = current_theme();
  let motion = theme.spin_motion();
  if motion.is_still_when_reduced() && theme.reduced_motion() {
    return glyph;
  }
  let easing = match motion.steps_per_second() {
    // A turn of a whole number of seconds: f64 of it is exact.
    Some(per_second) => Easing::Steps {
      count: (SPIN_PERIOD.as_secs_f64() * f64::from(per_second)).round().max(1.0) as u32,
      position: StepPosition::JumpEnd,
    },
    None => Easing::Linear,
  };
  glyph.animation(Animation::new(SPIN).duration(SPIN_PERIOD).easing(easing).infinite())
}
