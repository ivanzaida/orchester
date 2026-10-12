//! Reference-file row k2wUYb, with real file metadata and an optional removal handler.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, marker},
  icon_button::IconButton,
};
use crate::theme::ExtraSpacing;

pub fn removable(
  ctx: &mut Ctx,
  id: &str,
  name: &str,
  metadata: &str,
  remove_label: &str,
  remove: Option<impl Fn() + Send + Sync + 'static>,
) -> Element {
  let button = IconButton::new(id.to_owned(), Icon::X, remove_label).small().in_form();
  let button = if let Some(remove) = remove {
    button.on_click(remove)
  } else {
    button
  }
  .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(crate::theme::skills::file_inset()))
    .spacing(ExtraSpacing::Loose.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(marker(Icon::FileText, 24.0))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(name)
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Mono),
        )
        .child(
          Text::new(metadata)
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Caption),
        ),
    )
    .child(button)
    .into()
}
