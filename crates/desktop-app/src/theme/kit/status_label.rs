//! Status labels (`ui::status`): lurq's `ui::StatusLabel`. A priority and a workflow status (`Priority cell` `OfuTC`)
//! are a 13 icon and its word in 12, 5 apart, in the tone the kit gives each; a status (`Status` `VWRbG`) the same
//! with its word at 500 ([`STATUS`]), and a table row's run line a 12 icon and 11.5/500 ([`STATUS_COMPACT`], `Run
//! line` `AgVPe`). The recipe is the app's whole.

use lurq::{
  app::theme::{Theme, TypographyStyle},
  ui::{Length, Recipe, StatusLabel, StatusLabelPart},
};

use crate::theme::{ExtraSpacing, ExtraTypography, IconSize};

/// The size of a status: its word at 500.
pub const STATUS: &str = "status";
/// The size of a status in a table row's run line.
pub const STATUS_COMPACT: &str = "status_compact";

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<StatusLabel>(
    Recipe::new()
      .base(StatusLabelPart::Root, |style| style.gap(ExtraSpacing::Pair.role()))
      .base(StatusLabelPart::Icon, |style| {
        style.square(Length::Px(IconSize::Base.size()))
      })
      .base(StatusLabelPart::Label, |style| {
        style.typography(TypographyStyle::Description)
      })
      .size(STATUS, StatusLabelPart::Label, |style| {
        style.typography(TypographyStyle::Label)
      })
      .size(STATUS_COMPACT, StatusLabelPart::Icon, |style| {
        style.square(Length::Px(IconSize::Sm.size()))
      })
      .size(STATUS_COMPACT, StatusLabelPart::Label, |style| {
        style.typography(ExtraTypography::NoteStrong.role())
      }),
  );
}
