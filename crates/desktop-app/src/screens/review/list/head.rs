//! The list's head (`Header` `p4TAf` of `01 · Review` `i6S42`): one 32 high row with the title in 20/600 and, at its
//! end, the search (240 wide, its `/` key cap while empty), and under it the lead line (`Lead` `bLQCl`, 13
//! `text_secondary`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::ui::field::FieldInput;

const ROW_HEIGHT: f32 = 32.0;
const SEARCH_WIDTH: f32 = 240.0;

/// The title and the search, which `/` focuses.
pub(super) fn head(ctx: &mut Ctx, search: &Signal<String>, input: &ElementRef) -> Row {
  let field = FieldInput::new("review-search", ctx.t("review.search.label"), search.clone())
    .search()
    .keys(ctx.t("review.search.keys"))
    .clearable(ctx.t("review.search.clear"))
    .placeholder(ctx.t("review.search.placeholder"))
    .width(Dimension::Px(SEARCH_WIDTH))
    .element(input.clone())
    .render(ctx);
  Row::new()
    .id("review-header")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("review.title"))
        .id("review-title")
        .variant(TypographyStyle::Heading)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(field)
}

/// The lead line under the head.
pub(super) fn lead(ctx: &Ctx) -> Element {
  Text::new(&ctx.t("review.lead"))
    .id("review-lead")
    .variant(TypographyStyle::Body)
    .color(PaletteColor::TextSecondary)
    .into()
}
