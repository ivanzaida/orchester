//! A priority and a workflow status (`Priority cell` `OfuTC`; `ui::status`): lurq's `ui::StatusLabel`, a 13 icon and
//! its word in 12, 5 apart, in the tone the kit gives each. The recipe is the app's whole.

use lurq::{
  app::theme::{Theme, TypographyStyle},
  ui::{Length, Recipe, StatusLabel, StatusLabelPart},
};

use super::plain::plain;
use crate::theme::{ExtraSpacing, IconSize};

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<StatusLabel>(
    Recipe::new()
      .base(StatusLabelPart::Root, |style| style.gap(ExtraSpacing::Pair.role()))
      .base(StatusLabelPart::Icon, |style| {
        style.square(Length::Px(IconSize::Base.size()))
      })
      .base(StatusLabelPart::Label, |style| {
        style.typography(plain(TypographyStyle::Description))
      }),
  );
}
