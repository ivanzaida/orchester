//! A group's title row in a hub's list (`Section · Needs you` `A0nkGc` in `Pipelines · v2` P1, `Section` in `Agents ·
//! v2` L2): the group's name and its count, one for every hub that groups its list by what the person has to do.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{dimension::Dimension, padding::Padding},
};

use crate::theme::HubText;

/// A group's title row.
const SECTION_HEIGHT: f32 = 32.0;

/// The title row of the group `name` (in element ids, which start with `prefix`), its words' key and the color of its
/// count.
pub(crate) fn section(ctx: &Ctx, prefix: &str, (name, key, color): (&str, &str, PaletteColor), count: usize) -> Row {
  Row::new()
    .id(format!("{prefix}-section-{name}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(key))
        .id(format!("{prefix}-section-{name}-title"))
        .variant(HubText::GroupTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&count.to_string())
        .id(format!("{prefix}-section-{name}-count"))
        .variant(HubText::Hint.role())
        .color(color)
        .nowrap(),
    )
}
