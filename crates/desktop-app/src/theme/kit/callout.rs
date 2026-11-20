//! Callout (`ui::callout`; `Callout · …` `naoei`, `Gbag9`, `wwtNM`, `tCSh6`): lurq's `ui::Callout` with a 16 icon, a
//! title in 13/600 `text_primary` with the kit's parts 4 under it, 12 apart; radius 10, 14 over and under and 16 at its
//! sides, on its tone's fill with no stroke. Each tone is a variant: its fill and its icon's color. The recipe is the
//! app's whole: lurq's bar and its own tones are not the design's.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, Theme},
  ui::{Callout, CalloutPart, Length, Recipe},
};

use super::plain::plain;
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize};

/// The variants a callout's tone names.
pub const INFO: &str = "info";
pub const NEUTRAL: &str = "neutral";
pub const WARNING: &str = "warning";
pub const DANGER: &str = "danger";
pub const SUCCESS: &str = "success";

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Callout>(recipe());
}

fn recipe() -> Recipe {
  let tones = [
    (INFO, PaletteColor::InfoMuted, PaletteColor::Info),
    (NEUTRAL, ExtraColor::Hover.role(), PaletteColor::MutedForeground),
    (WARNING, PaletteColor::WarningMuted, PaletteColor::Warning),
    (DANGER, PaletteColor::DestructiveMuted, PaletteColor::Destructive),
    (SUCCESS, PaletteColor::SuccessMuted, PaletteColor::Success),
  ];
  let recipe = Recipe::new()
    .base(CalloutPart::Root, |style| {
      style
        .width(Length::Fill)
        .padding_x(SpacingSize::Lg)
        .padding_y(ExtraSpacing::Inset.role())
        .gap(SpacingSize::Md)
        .radius(RadiusSize::Lg)
        .foreground(PaletteColor::Foreground)
    })
    .base(CalloutPart::Icon, |style| style.square(Length::Px(IconSize::Lg.size())))
    .base(CalloutPart::Text, |style| style.gap(SpacingSize::Xs))
    .base(CalloutPart::Title, |style| {
      style
        .typography(plain(ExtraTypography::FormSection.role()))
        .foreground(PaletteColor::Foreground)
    });
  tones.into_iter().fold(recipe, |recipe, (tone, fill, mark)| {
    recipe
      .variant(tone, CalloutPart::Root, |style| style.background(fill))
      .variant(tone, CalloutPart::Icon, |style| style.foreground(mark))
  })
}
