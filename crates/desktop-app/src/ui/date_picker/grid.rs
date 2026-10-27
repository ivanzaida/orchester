//! The picker's weekdays and days (`Weekdays` `UH1sQ`, `Days` `l2msi`): seven columns as wide as the panel leaves them,
//! the weekdays 22 high, each day 30 high with radius 6, the weeks 2 apart.

use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};

use super::{Calendar, PickerProps};
use crate::{
  theme::{DatePickerText, ExtraColor, ExtraShadow, ExtraSpacing, UNAVAILABLE_DAY_OPACITY},
  ui::pressable::{inert, pressable},
};

/// A weekday's row (`Mo` `ZOcUr`) and a day's (`Week 1` `buHTX`).
const WEEKDAY_HEIGHT: f32 = 22.0;
const DAY_HEIGHT: f32 = 30.0;

/// The weekdays' words, Monday first.
const WEEKDAYS: [&str; 7] = [
  "ui.datePicker.weekdays.monday",
  "ui.datePicker.weekdays.tuesday",
  "ui.datePicker.weekdays.wednesday",
  "ui.datePicker.weekdays.thursday",
  "ui.datePicker.weekdays.friday",
  "ui.datePicker.weekdays.saturday",
  "ui.datePicker.weekdays.sunday",
];

/// How a day is marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
  Chosen,
  Today,
  /// The other end of the range.
  Other,
  Plain,
}

impl Mark {
  const fn name(self) -> &'static str {
    match self {
      Self::Chosen => "chosen",
      Self::Today => "today",
      Self::Other => "other-end",
      Self::Plain => "plain",
    }
  }
}

pub fn weekdays(ctx: &Ctx, id: &str) -> Element {
  Row::new()
    .id(format!("{id}-weekdays"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(WEEKDAY_HEIGHT))
    .with_children(WEEKDAYS.map(|key| {
      Row::new()
        .flex(1.0)
        .height(Dimension::Pct(100.0))
        .justify(Justify::Center)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).nowrap())
    }))
    .into()
}

/// The six weeks of `calendar`, the keyboard's ring on `ring` once it moved.
pub fn days(ctx: &Ctx, props: &PickerProps, calendar: Calendar, ring: Option<NaiveDate>) -> Element {
  let weeks = calendar.weeks().map(|week| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(DAY_HEIGHT))
      .with_children(week.map(|day| self::day(ctx, props, (calendar, day), ring == Some(day))))
  });
  Column::new()
    .id(format!("{}-days", props.id))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(weeks)
    .into()
}

fn mark(props: &PickerProps, day: NaiveDate) -> Mark {
  if props.chosen == Some(day) {
    Mark::Chosen
  } else if props.other == Some(day) {
    Mark::Other
  } else if props.today == day {
    Mark::Today
  } else {
    Mark::Plain
  }
}

/// One day: its number in its marks, picked by a press unless the range rules it out.
fn day(ctx: &Ctx, props: &PickerProps, (calendar, day): (Calendar, NaiveDate), ringed: bool) -> Element {
  let mark = mark(props, day);
  let allowed = props.limits.allows(day);
  let ours = calendar.in_month(day);
  let color = match (mark, ours) {
    (Mark::Chosen, _) => ExtraColor::OnAccent.role(),
    (_, true) => PaletteColor::TextPrimary,
    (_, false) => PaletteColor::TextMuted,
  };
  let strong = matches!(mark, Mark::Chosen) || props.today == day;
  let typography = if strong {
    DatePickerText::DayMarked
  } else {
    DatePickerText::Day
  };
  let label = Text::new(&day.day().to_string())
    .variant(typography.role())
    .color(color)
    .nowrap();
  let surface = if allowed {
    let (open, pick) = (props.open.clone(), props.pick.clone());
    pressable(move || {
      open.set(false);
      pick(Some(day));
    })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
  } else {
    inert(lurq::components::Button::empty().align_items(Alignment::Center)).opacity(UNAVAILABLE_DAY_OPACITY)
  };
  let mut cell = surface
    .id(format!("{}-day-{day}", props.id))
    .flex(1.0)
    .height(Dimension::Pct(100.0))
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .describe("a11y_name", day_name(ctx, day))
    .describe("mark", mark.name())
    .describe("available", if allowed { "true" } else { "false" })
    .describe("month", if ours { "this" } else { "around" })
    .child(label);
  cell = match mark {
    Mark::Chosen => cell.background(PaletteColor::Accent),
    Mark::Other => cell.background(PaletteColor::AccentMuted),
    Mark::Today => cell.border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)),
    Mark::Plain => cell,
  };
  if props.today == day && mark != Mark::Today {
    cell = cell.border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong));
  }
  if ringed {
    cell = cell
      .box_shadow(ExtraShadow::FocusRing.role())
      .describe("focused", "true");
  }
  cell.into()
}

/// `October 27, 2026`, the name tools read for a day.
fn day_name(ctx: &Ctx, day: NaiveDate) -> Arc<str> {
  ctx.t_args(
    "ui.datePicker.day",
    [
      ("month", ctx.t(super::MONTHS[day.month0() as usize])),
      ("day", Arc::from(day.day().to_string())),
      ("year", Arc::from(day.year().to_string())),
    ],
  )
}
