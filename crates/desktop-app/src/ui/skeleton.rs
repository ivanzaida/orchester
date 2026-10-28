//! Skeletons: what stands in a list's or a board's place while it is read (`05 · Loading` `R1nQlx`, `Reading ·
//! skeleton` `UWoGx`). A bone is a `border` block, radius 4, as large as the part it stands for; a skeleton shows only
//! after [`SKELETON_DELAY`](crate::theme::SKELETON_DELAY), so a fast reading never flashes one, and pulses gently
//! unless the operating system asks for reduced motion (`appearance::motion`), when it stays still. lurq plays the
//! pulse on its animation clock and asks for frames only while a pulsing skeleton is in the window's tree.

use std::sync::OnceLock;

use lurq::{
  animation::{AnimatableProperty, Animation, Easing, Keyframes, KeyframesId, StepPosition},
  app::{
    Tree,
    theme::{PaletteColor, RadiusSize},
  },
  components::Column,
  node::dimension::Dimension,
};

use crate::theme::{ExtraRadius, SKELETON_DIM, SKELETON_PULSE, SKELETON_STEPS};

/// The pulse's keyframes, registered once per window tree by [`install`]; the loader's turn is 1.
const PULSE: KeyframesId = KeyframesId::new(2);

/// Whether the operating system asks for reduced motion, once it was read when the app started; until then, and when
/// it cannot be read, skeletons pulse.
static STILL: OnceLock<bool> = OnceLock::new();

/// Registers the pulse with a window's tree.
pub fn install(tree: &mut Tree) {
  tree.register_keyframes_in_phase(
    Keyframes::new(PULSE)
      .frame(0.0, |frame| {
        frame.set(AnimatableProperty::Opacity, 1.0);
      })
      .frame(0.5, |frame| {
        frame.set(AnimatableProperty::Opacity, SKELETON_DIM);
      })
      .frame(1.0, |frame| {
        frame.set(AnimatableProperty::Opacity, 1.0);
      }),
  );
}

/// Keeps what the operating system said about reduced motion: `still` skeletons do not pulse. The first answer stays.
pub fn hold_still(still: bool) {
  STILL.get_or_init(|| still);
}

/// `skeleton`, pulsing unless the operating system asks for reduced motion.
pub fn pulsing(skeleton: Column) -> Column {
  if STILL.get().copied().unwrap_or(false) {
    return skeleton;
  }
  let steps = Easing::Steps {
    count: SKELETON_STEPS,
    position: StepPosition::JumpEnd,
  };
  skeleton.animation(Animation::new(PULSE).duration(SKELETON_PULSE).easing(steps).infinite())
}

/// A bone `width` × `height`.
pub fn bone(width: f32, height: f32) -> Column {
  Column::new()
    .width(Dimension::Px(width))
    .height(Dimension::Px(height))
    .flex_shrink(0.0)
    .rounded(RadiusSize::Sm)
    .background(PaletteColor::Border)
}

/// A round bone `side` across, for a status mark.
pub fn round_bone(side: f32) -> Column {
  bone(side, side).rounded(ExtraRadius::Full.role())
}
