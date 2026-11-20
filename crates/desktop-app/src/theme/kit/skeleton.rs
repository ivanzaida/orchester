//! A bone of a skeleton (`ui::skeleton`, `Loading states · work screens` `xed4a`): lurq's `ui::Skeleton`, a `border`
//! block, radius 4, whatever its shape (a round one, a status mark's, all the way round). Every bone pulses in one
//! phase, one pulse in [`SKELETON_PULSE`] stepping through [`SKELETON_STEPS`] poses, so a frame is drawn only when the
//! pose changes; lurq dims a bone to half at the pulse's middle, the design to [`super::super::SKELETON_DIM`].

use lurq::{
  app::theme::{PaletteColor, RadiusSize, Theme},
  ui::{Skeleton, SkeletonMotion, SkeletonPart, SkeletonShape},
};

use crate::theme::{SKELETON_PULSE, SKELETON_STEPS};

pub(super) fn install(theme: &Theme) {
  let poses_a_second = u32::try_from(u128::from(SKELETON_STEPS) * 1000 / SKELETON_PULSE.as_millis())
    .expect("a pulse's poses a second are few");
  theme.set_skeleton_motion(SkeletonMotion::stepped(poses_a_second).period(SKELETON_PULSE));
  theme.patch_recipe::<Skeleton>(|rules| {
    rules
      .base(SkeletonPart::Bone, |style| {
        style.background(PaletteColor::Border).radius(RadiusSize::Sm)
      })
      .variant(SkeletonShape::Block, SkeletonPart::Bone, |style| {
        style.radius(RadiusSize::Sm)
      })
      .variant(SkeletonShape::Circle, SkeletonPart::Bone, |style| {
        style.radius(RadiusSize::Full)
      })
  });
}
