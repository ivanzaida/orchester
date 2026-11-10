//! The line under the compact picker's rail (`Who` `n5OFk` in `ZcGGy`): 4 at its sides, its parts 6 apart: the
//! connection's name (12/500), *CLI · plan* (11.5 `text_muted`, or why it cannot be chosen in `warning`), and at its
//! end the plan's 5-hour use in Geist Mono 10.5 before a 32 × 4 meter.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::model::Connection;
use crate::{
  theme::{ExtraSpacing, RunsOnPickerText},
  ui::meter::{MeterTone, meter},
};

/// The meter's width (`g5B2c`).
const METER_WIDTH: f32 = 32.0;

/// The line for `connection`, saying `refused` (why it cannot be chosen) in place of its plan.
pub(super) fn who(ctx: &Ctx, id: &str, connection: &Connection, refused: Option<&Arc<str>>) -> Element {
  let caption = Text::new(refused.unwrap_or(&connection.caption))
    .id(format!("{id}-who-caption"))
    .variant(RunsOnPickerText::Caption.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  let caption = if refused.is_some() {
    caption.color(PaletteColor::Warning)
  } else {
    caption
  };
  let usage = connection.five_hour.filter(|_| refused.is_none()).map(|percent| {
    let words = ctx.t_args("ui.runsOnPicker.use", [("used", percent.to_string())]);
    let tone = if connection.limit {
      MeterTone::Reached
    } else {
      MeterTone::Plain
    };
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .flex_shrink(0.0)
      .child(
        Text::new(&words)
          .id(format!("{id}-who-use"))
          .variant(RunsOnPickerText::Use.role())
          .nowrap(),
      )
      .child(Row::new().width(Dimension::Px(METER_WIDTH)).child(meter(
        &format!("{id}-who-meter"),
        (u64::from(percent), 100),
        tone,
      )))
  });
  Row::new()
    .id(format!("{id}-who"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&connection.name)
        .id(format!("{id}-who-name"))
        .variant(RunsOnPickerText::Name.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(caption)
    .child(Spacer::new())
    .with_children(usage)
    .into()
}
