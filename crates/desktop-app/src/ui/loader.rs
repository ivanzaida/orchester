//! The loader (`loader-circle`) turns while it is shown: [`icon`](super::icon::icon) gives every loader
//! icon this turn, so running work never looks stuck. lurq plays the turn and asks for frames only while
//! a turning node is in the window's tree, so a window without a loader on screen stays idle. The turn
//! steps through [`LOADER_STEPS`] poses, and lurq draws a frame only when the pose changes.

use std::f32::consts::TAU;

use lurq::{
  animation::{AnimatableProperty, Animation, Easing, Keyframes, KeyframesId, StepPosition},
  app::Tree,
  components::Text,
  node::transform::Decomposed,
};

use crate::theme::{LOADER_STEPS, LOADER_TURN};

/// The turn's keyframes, registered once per window tree by [`install`].
const TURN: KeyframesId = KeyframesId::new(1);

/// Registers the loader's turn with a window's tree. Without it lurq would keep asking for frames for a
/// loader that stands still.
pub fn install(tree: &mut Tree) {
  tree.register_keyframes(
    Keyframes::new(TURN)
      .frame(0.0, |frame| {
        frame.set(AnimatableProperty::Transform, Decomposed::IDENTITY);
      })
      // A decomposed full turn: a matrix would read back as no rotation at all.
      .frame(1.0, |frame| {
        frame.set(AnimatableProperty::Transform, Decomposed::IDENTITY.with_rotate(TAU));
      }),
  );
}

/// The loader `glyph`, turning about its centre in even steps for as long as it is shown. `JumpEnd`
/// starts on the still pose and stops one step short of the full turn, so no pose is drawn twice.
pub(crate) fn turning(glyph: Text) -> Text {
  let steps = Easing::Steps { count: LOADER_STEPS, position: StepPosition::JumpEnd };
  glyph.animation(Animation::new(TURN).duration(LOADER_TURN).easing(steps).infinite())
}
