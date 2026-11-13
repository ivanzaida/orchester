//! The line under the compact picker's rail (`Who` `n5OFk` in `ZcGGy`; on two lines `um5q9` in `05b` `AZp3I`, `yfqxy`
//! in `t7cdi`): 4 at its sides, its parts 6 apart: the connection's name (12/500), *CLI · plan* (11.5 `text_muted`, or
//! why it cannot be chosen in `warning`), and at its end the plan's 5-hour use in Geist Mono 10.5 before a 32 × 4
//! meter. A connection with a [`Detail`] says it on a second line 3 under the first, in Geist Mono 10.5 (`warning` when
//! it warns, and the caption with it), the meter at that line's end.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::model::{Connection, Detail};
use crate::{
  theme::{ExtraSpacing, RunsOnPickerText},
  ui::{
    meter::{MeterTone, meter},
    tooltip::Tooltip,
  },
};

/// The meter's width (`g5B2c`).
const METER_WIDTH: f32 = 32.0;

/// The line for `connection`, saying `refused` (why it cannot be chosen) in place of its plan.
pub(super) fn who(ctx: &mut Ctx, id: &str, connection: &Connection, refused: Option<&Arc<str>>) -> Element {
  let detail = connection.detail.as_ref().filter(|_| refused.is_none());
  let warns = refused.is_some() || detail.is_some_and(|detail| detail.warns);
  let caption = Text::new(refused.unwrap_or(&connection.caption))
    .id(format!("{id}-who-caption"))
    .variant(RunsOnPickerText::Caption.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  let caption = if warns {
    caption.color(PaletteColor::Warning)
  } else {
    caption
  };
  let name = Text::new(&connection.name)
    .id(format!("{id}-who-name"))
    .variant(RunsOnPickerText::Name.role())
    .nowrap()
    .flex_shrink(0.0);
  let percent = connection.five_hour.filter(|_| refused.is_none());
  let tone = if connection.limit || detail.is_some_and(|detail| detail.warns) {
    MeterTone::Reached
  } else {
    MeterTone::Plain
  };
  let first = Row::new()
    .id(format!("{id}-who-first"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(name)
    .child(caption);
  let body: Element = match detail {
    Some(detail) => Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Fine.role())
      .child(first)
      .child(second(ctx, id, detail, percent.map(|percent| bar(id, percent, tone))))
      .into(),
    None => first
      .child(Spacer::new().flex(1.0))
      .with_children(match percent {
        Some(percent) => Some(use_and_bar(ctx, id, percent, tone)),
        None => (connection.use_unread && refused.is_none()).then(|| unread(ctx, id)),
      })
      .into(),
  };
  Row::new()
    .id(format!("{id}-who"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .child(body)
    .into()
}

/// That the plan's 5-hour use was not read, in the same Geist Mono as the use, with no meter.
fn unread(ctx: &Ctx, id: &str) -> Element {
  Text::new(&ctx.t("ui.runsOnPicker.useUnread"))
    .id(format!("{id}-who-use"))
    .variant(RunsOnPickerText::Use.role())
    .nowrap()
    .flex_shrink(0.0)
    .into()
}

/// The 5-hour meter, filled to `percent`.
fn bar(id: &str, percent: u8, tone: MeterTone) -> Element {
  Row::new()
    .width(Dimension::Px(METER_WIDTH))
    .child(meter(&format!("{id}-who-meter"), (u64::from(percent), 100), tone))
    .into()
}

/// The plan's 5-hour use in words before its meter, at the end of a one-line who.
fn use_and_bar(ctx: &Ctx, id: &str, percent: u8, tone: MeterTone) -> Element {
  let words = ctx.t_args("ui.runsOnPicker.use", [("used", percent.to_string())]);
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
    .child(bar(id, percent, tone))
    .into()
}

/// The second line: `detail` in Geist Mono, with the plan's meter at its end.
fn second(ctx: &mut Ctx, id: &str, detail: &Detail, meter: Option<Element>) -> Element {
  let text = Text::new(&detail.text)
    .id(format!("{id}-who-detail"))
    .variant(RunsOnPickerText::Use.role())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0));
  let text = if detail.warns {
    text.color(PaletteColor::Warning)
  } else {
    text
  };
  let text: Element = match &detail.tip {
    Some(tip) => Tooltip::new(format!("{id}-who-detail-tip"), tip.clone()).wrap(ctx, text),
    None => text.into(),
  };
  Row::new()
    .id(format!("{id}-who-second"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(text)
    .child(Spacer::new().flex(1.0))
    .with_children(meter)
    .into()
}
