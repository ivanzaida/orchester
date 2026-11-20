//! Detail row (`ui::detail_row`; `Detail row` `paUqV`): lurq's `ui::DataList` of one item, what a value is in 12
//! `text_muted` in a 108 column, and the value 12 after it in 12/1.45 `text_primary`, wrapping; 5 over and under them,
//! with no line. An identifier's value is in Geist Mono (`Branch` `YHJhy`, state `mono`). The recipe is the app's
//! whole: lurq's 13 type, room and lines are not the design's.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, Theme, TypographyStyle},
  ui::{DataList, DataListPart, Length, Recipe},
};

use crate::theme::{ExtraSpacing, ExtraTypography};

/// The state of a list whose value is an identifier.
pub const MONO: &str = "mono";
/// The key column (`rtxjz`, 108) with the room after it, which is the term's end padding.
const TERM_WIDTH: f32 = 120.0;

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<DataList>(recipe());
}

fn recipe() -> Recipe {
  Recipe::new()
    .base(DataListPart::Root, |style| style.width(Length::Fill))
    .base(DataListPart::Term, |style| {
      style
        .width(TERM_WIDTH)
        .padding_y(ExtraSpacing::Pair.role())
        .padding_end(SpacingSize::Md)
        .typography(TypographyStyle::Description)
        .foreground(PaletteColor::MutedForeground)
    })
    .base(DataListPart::Value, |style| {
      style
        .padding_y(ExtraSpacing::Pair.role())
        .typography(ExtraTypography::Prose.role())
        .foreground(PaletteColor::Foreground)
    })
    .state(MONO, DataListPart::Value, |style| {
      style.typography(TypographyStyle::Mono)
    })
}
