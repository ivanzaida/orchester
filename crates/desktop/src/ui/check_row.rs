//! Check row (`l3hyb`, Evidence / Check): an icon, a title and a mono result at the right, with a
//! hairline below. The task page lists a run's progress (`qB7mj`) and a failed attempt's steps
//! (`Wp8Tl` in 02.B) with it.

use lurq::{
  app::theme::{BorderSize, PaletteColor, TypographyStyle},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{border::Border, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography};

/// Row height (`l3hyb`).
const CHECK_ROW_HEIGHT: f32 = 46.0;

pub fn check_row(glyph: Icon, title: &str, result: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(CHECK_ROW_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Stack.role())
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon(glyph, IconSize::Option, PaletteColor::TextPrimary))
    .child(
      Text::new(title)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .child(
      Text::new(result)
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
}
