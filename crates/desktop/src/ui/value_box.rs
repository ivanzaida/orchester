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

/// Height of Copy on the label's line (`feLwP`), which is the line's height.
const COPY_HEIGHT: f32 = 22.0;
use crate::theme::{ExtraSpacing, value_box_scrollbar};

/// The state the Copy buttons of a group of value boxes share: the last copy and its outcome, and
/// which Copy the pointer is over (by its id).
#[derive(Clone)]
pub struct CopyState {
  pub copied: Signal<Copied>,
  pub hovered: Signal<Option<Arc<str>>>,
}

/// What a value box shows and what its Copy takes: the same text, or `shown` laid out for reading
/// (e.g. JSON one member a line) while Copy takes `original` as recorded.
#[derive(Clone, Copy)]
pub struct BoxText<'a> {
  pub shown: &'a str,
  pub original: &'a str,
}

/// The label line and the box, with ids `{id}` (the box), `{id}-text`, `{id}-scroll` and
/// `{id}-copy`; Copy records its outcome and its hover in `copy`. Copy comes before the box, so Tab reaches it
/// first. Callers may add a note under the box.
pub fn value_box(ctx: &Ctx, (label, id): (&str, &str), text: BoxText<'_>, max_height: f32, copy: &CopyState) -> Column {
  let copy_id: Arc<str> = Arc::from(format!("{id}-copy"));
  let (button, outcome) = copy_button(ctx, copy_id.clone(), text.original, &copy.copied);
  let hovered = copy.hovered.get().as_deref() == Some(&*copy_id);
  let hover = copy.hovered.clone();
  let button = button
    .height(Dimension::Px(COPY_HEIGHT))
    .quiet(hovered, move |entered| {
      // Leaving a Copy clears only its own hover, whatever order the events of two arrive in.
      hover.update(|current| match (entered, current.as_deref() == Some(&*copy_id)) {
        (true, _) => *current = Some(copy_id.clone()),
        (false, true) => *current = None,
        (false, false) => {}
      });
    });
  let label_line = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::SpaceBetween)
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(button.in_form().render(ctx));
  let lines = ScrollBoth::new(recorded_lines(id, text.shown))
    .id(format!("{id}-scroll"))
    .scrollbar(value_box_scrollbar())
    .min_width(Dimension::Px(0.0))
    .flex(1.0);
  // Padding 6 above, below and right of the viewport (`E5NBo`); its scrollbars reserve their lanes
  // inside it, beside the text.
  let frame = command_box(id)
    .max_height(Dimension::Px(max_height))
    .padding_vertical(ExtraSpacing::Tight.role())
    .padding_right(ExtraSpacing::Tight.role())
    .child(lines);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(label_line)
    .child(with_copy_failure(ctx, frame.into(), outcome))
}
