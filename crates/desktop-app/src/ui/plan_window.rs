//! Plan window row (`Plan window row` `B9myCE`): one window of an account's plan limits — its name in 12
//! `text_muted` in a 72 column, a 200 wide meter, the share used in Geist Mono 11.5 in a 40 column, and when it
//! resets in 11.5 `text_muted`, 12 apart, 6 over and under. A window at its limit fills its meter and its share in
//! `warning`. The caller words the name and the reset in its own area's words.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::plan_limits::{LimitReading, LimitState, LimitWindow},
  time::Timestamp,
};
use orchester_workspace::{
  display::{self, MONTH_KEYS},
  messages::Message,
};

use super::{
  message::message_text,
  meter::{MeterTone, meter},
};
use crate::theme::{AccountsSpacing, ExtraSpacing, ExtraTypography};

/// The widths of the name, the meter and the share (`oNPEn`, `a4M4r`, `TK5Br`).
const NAME_WIDTH: f32 = 72.0;
const METER_WIDTH: f32 = 200.0;
const USED_WIDTH: f32 = 40.0;

/// What a window row shows.
pub struct PlanWindow<'a> {
  pub name: &'a str,
  /// Whole percent used.
  pub used: u64,
  pub reached: bool,
  pub resets: &'a str,
}

pub fn plan_window(id: &str, window: &PlanWindow<'_>) -> Element {
  let (tone, used_color) = if window.reached {
    (MeterTone::Reached, PaletteColor::Warning)
  } else {
    (MeterTone::Plain, PaletteColor::TextPrimary)
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Snug.role())
    .spacing(AccountsSpacing::Cell.role())
    .align_items(Alignment::Center)
    .describe("reached", window.reached.to_string())
    .child(
      Text::new(window.name)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(NAME_WIDTH)),
    )
    .child(
      Row::new()
        .width(Dimension::Px(METER_WIDTH))
        .child(meter(&format!("{id}-meter"), (window.used, 100), tone)),
    )
    .child(
      Text::new(&format!("{}%", window.used))
        .variant(ExtraTypography::MonoKey.role())
        .color(used_color)
        .width(Dimension::Px(USED_WIDTH)),
    )
    .child(Text::new(window.resets).variant(ExtraTypography::Note.role()).flex(1.0))
    .into()
}

/// How much of `window` of `reading` is used, in whole percent, and whether it is at its limit now: a window at its
/// limit whose use the CLI did not report reads as full.
pub fn share(window: &LimitWindow, reading: &LimitReading, now: Timestamp) -> (u64, bool) {
  let reached = reading.window_state(window, now) == LimitState::Reached;
  let percent = window.percent().unwrap_or(if reached { 100.0 } else { 0.0 });
  // At most a few hundred percent; whole ones are what the row shows.
  (percent.clamp(0.0, 100.0).round() as u64, reached)
}

/// A time as plan limits read it, worded by the caller's `(today, day)` keys: `today, 16:40`, or `Oct 14, 09:00`
/// on another day.
pub fn when(ctx: &Ctx, at: Timestamp, now: Timestamp, (today, day): (&str, &str)) -> String {
  let time = display::clock(at);
  if display::calendar_date(at) == display::calendar_date(now) {
    return ctx.t_args(today, [("time", time)]).to_string();
  }
  let (month, date) = display::month_day(at);
  let month = message_text(ctx, &Message::new(MONTH_KEYS[month]));
  ctx
    .t_args(
      day,
      [("month", month.to_string()), ("day", date.to_string()), ("time", time)],
    )
    .to_string()
}
