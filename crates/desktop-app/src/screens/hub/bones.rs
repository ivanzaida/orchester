//! The hub's parts in bones, in the geometry of `parts` (`Loading states · configure screens` `psFv5`, 03 to 08): a
//! group's head with its title real and its count a bone, a list row with its tile, label, hint and end as bones, a
//! sub-head, and a status, so the real rows replace them in place.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::skeleton::{self, bone, round_bone},
};

/// A list row's tile (`Tile` `uEJC1`) and a status's dot (`Dot` `k934A`), as `parts` draws them.
const TILE: f32 = 28.0;
const DOT: f32 = 6.0;

/// A group's head: its `title` real, its count a bone `count` long when it has one.
pub(crate) fn head(ctx: &Ctx, id: &str, title: &str, count: Option<f32>) -> Row {
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(HubText::GroupTitle.role())
        .nowrap(),
    )
    .with_children(count.map(|count| skeleton::line(ctx, HubText::Hint.role(), count)))
}

/// A tile not known yet: a bone of its size.
pub(crate) fn tile() -> Column {
  bone(TILE, TILE).rounded(RadiusSize::Md)
}

/// A status not known yet: its dot and its words, `width` long.
pub(crate) fn status(ctx: &Ctx, width: f32) -> Row {
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(round_bone(DOT))
    .child(skeleton::line(ctx, HubText::Status.role(), width))
}

/// A list row in bones: `lead` (a tile's bone, or the fixed tile a kind keeps), its label and hint `(label, hint)`
/// long, `end`, and the chevron it opens with.
pub(crate) fn entry(ctx: &Ctx, lead: Option<Element>, (label, hint): (f32, f32), end: Vec<Element>) -> Row {
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(skeleton::line(ctx, ExtraTypography::Strong.role(), label))
    .child(skeleton::line(ctx, HubText::Hint.role(), hint));
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::RowY.role())
        .bottom(HubSpacing::RowY.role())
        .left(HubSpacing::Row.role())
        .right(HubSpacing::Row.role()),
    )
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .with_children(lead)
    .child(text)
    .with_children(end)
    .child(skeleton::chevron())
}

/// A sub-head inside a group's rows, its label and count bones (`ksfLm`).
pub(crate) fn sub_head(ctx: &Ctx, label: f32, count: f32) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::SubY.role())
        .bottom(HubSpacing::SubY.role())
        .left(HubSpacing::Row.role())
        .right(HubSpacing::Row.role()),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .child(skeleton::line(ctx, HubText::GroupTitle.role(), label))
    .child(skeleton::line(ctx, HubText::Hint.role(), count))
}
