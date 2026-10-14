//! Parts of a conversation's composer (`Composer` `QQ2ZM`) that a task's conversation and the Orchestrator chat
//! share.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  tooltip::Tooltip,
};
use crate::theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize};

/// The recipient pill (`Recipient` `AOjdU`): who gets the message and what it does with it, 24 high, `surface_raised`
/// in a `border` stroke, radius 6, a 12 glyph and the words in 11.5 `text_secondary`, 5 apart, 8 in from its sides;
/// while it is fixed, a lock at its end, and why in its tooltip.
pub fn recipient_pill(ctx: &mut Ctx, (words, glyph): (&str, Icon), fixed: Option<Arc<str>>) -> Element {
  let pill = Row::new()
    .id("composer-recipient")
    .describe("fixed", fixed.is_some().to_string())
    .height(Dimension::Px(ControlHeight::Sm.value()))
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(words)
        .id("composer-recipient-text")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .with_children(
      fixed
        .is_some()
        .then(|| icon(Icon::Lock, IconSize::Xs, PaletteColor::TextMuted)),
    );
  match fixed {
    Some(reason) => Tooltip::new("composer-recipient", reason.clone()).wrap(ctx, pill.describe("reason", reason)),
    None => pill.into(),
  }
}
