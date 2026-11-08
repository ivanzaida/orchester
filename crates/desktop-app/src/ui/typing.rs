//! The typing indicator: three dots under who is about to write (`Message · waiting` `sXET0`, `Typing` `D231t`,
//! `Dot` `h6EkpB`), while nothing arrived yet. The design draws them still, fading from the first; here each fades from
//! the first's opacity to the last's and back once a [`REVIEW_TYPING_CYCLE`], a third of a cycle behind the one before
//! it, unless the operating system asks for reduced motion (`ui::skeleton::is_still`), when they stay as drawn. lurq
//! plays the cycle on its animation clock and asks for frames only while dots are in the window's tree.

use lurq::{
  animation::{AnimatableProperty, Animation, Easing, Keyframes, KeyframesId, StepPosition},
  app::{
    Tree,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::skeleton;
use crate::theme::{ExtraRadius, ExtraSpacing, REVIEW_TYPING_CYCLE, REVIEW_TYPING_OPACITY, REVIEW_TYPING_STEPS};

/// A dot's side (`h6EkpB`).
const DOT_SIZE: f32 = 6.0;

/// The keyframes of the three dots, registered once per window tree by [`install`]; the loader's turn is 1, the
/// skeleton's pulse 2.
const WAVES: [KeyframesId; 3] = [KeyframesId::new(3), KeyframesId::new(4), KeyframesId::new(5)];

/// The poses of a cycle a keyframe marks: sixths, where the fade's turns fall for every dot's offset of a third.
const MARKS: usize = 6;

/// Registers the dots' cycles with a window's tree. Without it lurq would keep asking for frames for dots that stand
/// still.
pub fn install(tree: &mut Tree) {
  for (dot, id) in WAVES.into_iter().enumerate() {
    let mut keyframes = Keyframes::new(id);
    for mark in 0..=MARKS {
      let opacity = fade(mark as f32 / MARKS as f32 + dot as f32 / WAVES.len() as f32);
      keyframes = keyframes.frame(mark as f32 / MARKS as f32, move |frame| {
        frame.set(AnimatableProperty::Opacity, opacity);
      });
    }
    tree.register_keyframes_in_phase(keyframes);
  }
}

/// The opacity `phase` of a cycle in (any whole number of cycles added): the first dot's at its start, the last dot's
/// at the middle, linearly between.
fn fade(phase: f32) -> f32 {
  let [bright, _, dim] = REVIEW_TYPING_OPACITY;
  let into = phase.rem_euclid(1.0);
  let away = if into <= 0.5 { into } else { 1.0 - into };
  bright - (bright - dim) * (away * 2.0)
}

/// Three dots under `id`, as `{id}-dots`, moving while nothing was written yet.
pub fn dots(id: &str) -> Element {
  let still = skeleton::is_still();
  let steps = Easing::Steps {
    count: REVIEW_TYPING_STEPS,
    position: StepPosition::JumpEnd,
  };
  Row::new()
    .id(format!("{id}-dots"))
    .padding(Padding::new().top(SpacingSize::Xs).bottom(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .with_children(
      WAVES
        .into_iter()
        .zip(REVIEW_TYPING_OPACITY)
        .map(|(wave, opacity)| -> Element {
          let dot = Row::new()
            .width(Dimension::Px(DOT_SIZE))
            .height(Dimension::Px(DOT_SIZE))
            .rounded(ExtraRadius::Full.role())
            .background(PaletteColor::TextMuted)
            .opacity(opacity);
          if still {
            dot.into()
          } else {
            dot
              .animation(
                Animation::new(wave)
                  .duration(REVIEW_TYPING_CYCLE)
                  .easing(steps)
                  .infinite(),
              )
              .into()
          }
        }),
    )
    .into()
}
