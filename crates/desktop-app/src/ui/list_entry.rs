//! Removable path entry o6h0L5, inside the bordered input surface nZfWy.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  icon_button::IconButton,
};
use crate::theme::{ExtraSpacing, IconSize};

const ROW_HEIGHT: f32 = 32.0;

pub fn list_surface(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceInput)
    .rounded(RadiusSize::Md)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
}

pub fn removable(
  ctx: &mut Ctx,
  id: &str,
  text: &str,
  remove_label: &str,
  remove: impl Fn() + Send + Sync + 'static,
) -> Element {
  let button = IconButton::new(id.to_owned(), Icon::X, remove_label)
    .small()
    .in_form()
    .on_click(remove)
    .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::new().left(ExtraSpacing::Loose.role()).right(SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Folder, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(button)
    .into()
}
