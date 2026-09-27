//! A task page tab without its content (`hig8O`): an icon, a title and an explanation centered in
//! the tab, e.g. no activity yet, loading, couldn't load; the Changes tab's states are drawn alike.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};

use crate::{
  tasks::unreadable::Unreadable,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// Width of the explanation (`yUyVC`).
const TEXT_WIDTH: f32 = 420.0;

/// An icon, a title and an explanation centered in the tab, with optional actions.
pub(super) fn state(id: &'static str, glyph: Icon, title: &str, text: &str, actions: Vec<Element>) -> Element {
  let actions = (!actions.is_empty()).then(|| Row::new().spacing(SpacingSize::Sm).with_children(actions));
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::State, PaletteColor::TextSecondary))
    .child(Text::new(title).variant(ExtraTypography::CardTitle.role()))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Explanation.role())
        .text_align(TextAlign::Center)
        .width(Dimension::Px(TEXT_WIDTH)),
    )
    .with_children(actions)
    .into()
}

/// A tab of a task whose latest run cannot be read, but its Summary: nothing the run did can be
/// shown (not drawn; 02.R draws the Summary).
pub(super) fn unreadable(ctx: &Ctx, id: &'static str, run: &Unreadable) -> Element {
  state(
    id,
    Icon::AlertCircle,
    &message_text(ctx, &run.title()),
    &ctx.t("runs.unreadable.tabDetail"),
    Vec::new(),
  )
}

/// No activity has been reported yet, with why.
pub(super) fn empty_activity(ctx: &Ctx, text: &str) -> Element {
  state(
    "activity-empty",
    Icon::Activity,
    &ctx.t("runs.activity.emptyTitle"),
    text,
    Vec::new(),
  )
}
