//! Orchester brand (`a0S44m`): the conductor mark and the product name.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Rect, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography};

/// Brand row height and mark geometry (`a0S44m`, `DIMZ6`).
const BRAND_HEIGHT: f32 = 68.0;
const MARK_SIZE: f32 = 24.0;
const BAR_WIDTH: f32 = 4.0;
const BAR_HEIGHTS: [f32; 3] = [12.0, 22.0, 16.0];

pub fn brand(ctx: &Ctx) -> Element {
  let mark = BAR_HEIGHTS.into_iter().fold(
    Row::new()
      .size(Dimension::Px(MARK_SIZE), Dimension::Px(MARK_SIZE))
      .spacing(ExtraSpacing::Mark.role())
      .align_items(Alignment::Center),
    |mark, height| {
      mark.child(
        Rect::new(Dimension::Px(BAR_WIDTH), Dimension::Px(height))
          .background(PaletteColor::TextPrimary)
          .rounded(ExtraRadius::Mark.role()),
      )
    },
  );
  Row::new()
    .height(Dimension::Px(BRAND_HEIGHT))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(mark)
    .child(Text::new(&ctx.t("app.wordmark")).variant(ExtraTypography::Brand.role()))
    .into()
}
