//! Date picker (`14 · Date picker` `QF4a2` in `Modules · redesign` `oEELJ`): a calendar in a popover under the control
//! that opens it, in place of a typed date: lurq's `ui::Calendar`, its month first and ‹ › at the end of its heading,
//! six weeks from Monday whatever the month, so its height never changes, and Today and Clear in its footer; its look
//! is `theme::kit::calendar`. Days the range rules out are faded and cannot be picked ([`Limits`]). The calendar reads
//! the keys at its panel wherever the focus is: arrows a day or a week, PageUp and PageDown a month, Enter picks, Esc
//! closes. Month and weekday names are the app's words in every locale, never the system's.

mod limits;

use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
  },
  components::{Column, Row, Spacer},
  core::{Date, DateLocale, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
  ui::{self, CalendarHeading, CalendarKeys, DateMemory},
};

pub use self::limits::Limits;
use super::button::{ActionButton, ButtonKind};
use crate::theme::kit::calendar::KEYED;

/// The words of a month, January first.
const MONTHS: [&str; 12] = [
  "ui.datePicker.months.january",
  "ui.datePicker.months.february",
  "ui.datePicker.months.march",
  "ui.datePicker.months.april",
  "ui.datePicker.months.may",
  "ui.datePicker.months.june",
  "ui.datePicker.months.july",
  "ui.datePicker.months.august",
  "ui.datePicker.months.september",
  "ui.datePicker.months.october",
  "ui.datePicker.months.november",
  "ui.datePicker.months.december",
];

/// The weekdays' heads, Monday first.
const WEEKDAYS: [&str; 7] = [
  "ui.datePicker.weekdays.monday",
  "ui.datePicker.weekdays.tuesday",
  "ui.datePicker.weekdays.wednesday",
  "ui.datePicker.weekdays.thursday",
  "ui.datePicker.weekdays.friday",
  "ui.datePicker.weekdays.saturday",
  "ui.datePicker.weekdays.sunday",
];

/// The keys that move the day the calendar's keys are on.
const MOVES: [&str; 6] = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "PageUp", "PageDown"];

/// What a pick does: a day, or none for Clear.
pub type Pick = Arc<dyn Fn(Option<NaiveDate>) + Send + Sync>;

/// One picker: the date it holds, the other end of its range, the days it allows and what picking does.
#[derive(Clone)]
pub struct PickerProps {
  /// The calendar's element id; its parts' ids start with it (`-title`, `-nav-previous`, `-day-2026-10-27`).
  pub id: Arc<str>,
  pub chosen: Option<NaiveDate>,
  /// The other end of the range, tinted faintly: the Start date on a Target date's calendar.
  pub other: Option<NaiveDate>,
  pub limits: Limits,
  pub today: NaiveDate,
  /// Whether the popover is open; a pick or Esc closes it.
  pub open: Signal<bool>,
  pub pick: Pick,
}

impl PartialEq for PickerProps {
  fn eq(&self, other: &Self) -> bool {
    (self.id.as_ref(), self.chosen, self.other, self.limits, self.today)
      == (other.id.as_ref(), other.chosen, other.other, other.limits, other.today)
  }
}

impl DevtoolsInspectable for PickerProps {}

/// `props`' picker, for a control that places its own popover (a property chip's); mounted while open, so each opening
/// starts on the chosen day, or on today without one.
pub fn date_panel(ctx: &mut Ctx, props: PickerProps) -> Element {
  let key = format!("{}-component", props.id);
  ctx.mount_keyed::<DatePicker>(&key, props)
}

pub struct DatePicker {
  /// The month shown and the day the keys are on, between renders.
  memory: DateMemory,
  /// The keys moved the day they are on: it is ringed from then on.
  keyed: Signal<bool>,
}

impl Component for DatePicker {
  type Props = PickerProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      memory: DateMemory::new(),
      keyed: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PickerProps>().clone();
    let (open, pick, limits) = (props.open.clone(), props.pick.clone(), props.limits);
    let close = props.open.clone();
    let calendar = ui::Calendar::controlled(props.chosen.map(date), move |day: Date| pick(Some(naive(day))))
      .id(props.id.clone())
      .memory(&self.memory)
      .locale(words(ctx))
      .previous_label(&ctx.t("ui.datePicker.previousMonth"))
      .next_label(&ctx.t("ui.datePicker.nextMonth"))
      .heading(CalendarHeading::MonthThenNav)
      .keys(CalendarKeys::Panel)
      .today(date(props.today))
      .other_end(props.other.map(date))
      .disabled_days(move |day: Date| !limits.allows(naive(day)))
      .footer(footer(ctx, &props))
      .state(KEYED, self.keyed.get())
      .on_choose(move |_: Date| close.set(false));
    let keyed = self.keyed.clone();
    // The calendar's own keys are read at its panel; this sees them first, on the way there.
    Column::new()
      .on_key_down(move |event: KeyboardEvent| {
        if event.default_prevented() || !open.get_untracked() || event.ctrl || event.alt || event.meta {
          return;
        }
        if event.key == "Escape" {
          open.set(false);
          event.prevent_default();
          event.stop_propagation();
        } else if MOVES.contains(&event.key.as_str()) && !keyed.get_untracked() {
          keyed.set(true);
        }
      })
      .child(calendar)
  }
}

/// The calendar's words, the app's: its months and the heads of its weekdays, Monday first.
fn words(ctx: &Ctx) -> DateLocale {
  let mut locale = DateLocale::english();
  locale.months = MONTHS.map(|key| ctx.t(key));
  locale.weekdays_short = WEEKDAYS.map(|key| ctx.t(key));
  locale
}

/// Today and Clear: Today picks today, and cannot be pressed while the range rules today out; Clear empties the date,
/// which is always optional here.
fn footer(ctx: &Ctx, props: &PickerProps) -> Element {
  let (open, pick, today) = (props.open.clone(), props.pick.clone(), props.today);
  let today_button = ActionButton::new(
    format!("{}-today", props.id),
    ButtonKind::Ghost,
    ctx.t("ui.datePicker.today"),
  );
  let today_button = if props.limits.allows(today) {
    let (open, pick) = (open.clone(), pick.clone());
    today_button.on_click(move || {
      open.set(false);
      pick(Some(today));
    })
  } else {
    today_button.disabled()
  };
  let clear = ActionButton::new(
    format!("{}-clear", props.id),
    ButtonKind::Ghost,
    ctx.t("ui.datePicker.clear"),
  )
  .on_click(move || {
    open.set(false);
    pick(None);
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(today_button)
    .child(Spacer::new().flex(1.0))
    .child(clear)
    .into()
}

/// lurq's date of `day`.
fn date(day: NaiveDate) -> Date {
  let (month, of) = (day.month(), day.day());
  Date::new(
    day.year(),
    u8::try_from(month).expect("a month is 1 to 12"),
    u8::try_from(of).expect("a day of a month is 1 to 31"),
  )
  .expect("a chrono date is a calendar date")
}

/// chrono's date of `day`.
fn naive(day: Date) -> NaiveDate {
  NaiveDate::from_ymd_opt(day.year(), u32::from(day.month()), u32::from(day.day()))
    .expect("a lurq date is a calendar date")
}
