//! A long value under its label, as the approval dialog shows a command, a target or arguments
//! (`bjrgu`): the label with a quiet Copy at the right of its line, then the transcript's box
//! (`uA3Sh`) across the full width, every line as recorded, scrolling both ways inside a box no
//! taller than the caller's cap.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Row, ScrollBoth, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::dimension::Dimension,
};

use super::command_row::{Copied, command_box, copy_button, recorded_lines, with_copy_failure};
use crate::theme::{ExtraSpacing, value_box_scrollbar};

/// What a value box shows and what its Copy takes: the same text, or `shown` laid out for reading
/// (e.g. JSON one member a line) while Copy takes `original` as recorded.
#[derive(Clone, Copy)]
pub struct BoxText<'a> {
  pub shown: &'a str,
  pub original: &'a str,
}

/// The label line and the box, with ids `{id}` (the box), `{id}-text`, `{id}-scroll` and
/// `{id}-copy`; Copy records its outcome in `copied`. Copy comes before the box, so Tab reaches it
/// first. Callers may add a note under the box.
pub fn value_box(
  ctx: &Ctx,
  (label, id): (&str, &str),
  text: BoxText<'_>,
  max_height: f32,
  copied: &Signal<Copied>,
) -> Column {
  let (copy, outcome) = copy_button(ctx, Arc::from(format!("{id}-copy")), text.original, copied);
  let label_line = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::SpaceBetween)
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(copy.quiet().in_form().render(ctx));
  let lines = ScrollBoth::new(recorded_lines(id, text.shown))
    .id(format!("{id}-scroll"))
    .scrollbar(value_box_scrollbar())
    .min_width(Dimension::Px(0.0))
    .flex(1.0);
  // Padding 6 above and below the viewport (`E5NBo`); its scrollbars reserve their lanes inside it,
  // the vertical one where the right padding would be.
  let frame = command_box(id)
    .max_height(Dimension::Px(max_height))
    .padding_vertical(ExtraSpacing::Tight.role())
    .child(lines);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(label_line)
    .child(with_copy_failure(ctx, frame.into(), outcome))
}
