//! Date picker (`14 · Date picker` `QF4a2` in `Modules · redesign` `oEELJ`): a calendar in a popover under the control
//! that opens it, in place of a typed date. 260 wide on `surface_raised` inside a `border_strong` stroke, radius 10, 10
//! inside, its parts 6 apart: the month in 13/600 with ‹ and › (Icon buttons), the weekdays Monday first in 11
//! `text_muted`, a grid of six weeks whatever the month, so its height never changes, and Today and Clear (Ghost
//! buttons) over a `border` line. The chosen day is filled `accent`, today outlined in `border_strong`, the other end
//! of a range tinted `accent_muted`; the months around are muted, and days the range rules out are faded and cannot be
//! picked (`calendar::Limits`). The keyboard moves a ring over the days (the design draws none): arrows a day or a
//! week, PageUp and PageDown a month, Enter picks, Esc closes. Month and weekday names are the app's words in every
//! locale, never the system's.

pub mod calendar;
mod grid;

use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Popup, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

pub use self::calendar::{Calendar, KeyOutcome, Limits};
use super::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  icon_button::IconButton,
  popover::{PopoverSide, popover},
};
use crate::theme::{DatePickerText, ExtraSpacing};

/// The panel's width (`Target date picker` `INGIj`).
pub const PANEL_WIDTH: f32 = 260.0;
/// The month row's height (`Month` `uii7i`).
const MONTH_HEIGHT: f32 = 28.0;

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

/// What a pick does: a day, or none for Clear.
pub type Pick = Arc<dyn Fn(Option<NaiveDate>) + Send + Sync>;

/// One picker: the date it holds, the other end of its range, the days it allows and what picking does.
#[derive(Clone)]
pub struct PickerProps {
  /// The panel's element id; its parts' ids start with it.
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
impl DevtoolsInspectable for Calendar {}

/// `props`' picker in a popover under `anchor` while `open` holds; `props` only while open, so each opening starts on
/// the chosen day, or on today without one.
pub fn date_popover(ctx: &mut Ctx, anchor: &ElementRef, props: Option<PickerProps>, open: &Signal<bool>) -> Popup {
  let panel = props.map(|props| date_panel(ctx, props));
  popover(
    anchor.clone(),
    open.clone(),
    PopoverSide::Below,
    panel.unwrap_or_default(),
  )
}

/// `props`' picker, for a control that places its own popover (a property chip's).
pub fn date_panel(ctx: &mut Ctx, props: PickerProps) -> Element {
  let key = format!("{}-component", props.id);
  ctx.mount_keyed::<DatePicker>(&key, props)
}

pub struct DatePicker {
  calendar: Signal<Calendar>,
  /// The keyboard moved the ring: the focused day shows it from then on.
  keyed: Signal<bool>,
}

impl Component for DatePicker {
  type Props = PickerProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<PickerProps>();
    let start = props.chosen.unwrap_or(props.today);
    Self {
      calendar: ctx.signal(Calendar::on(start)),
      keyed: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PickerProps>().clone();
    let calendar = self.calendar.get();
    let ring = self.keyed.get().then_some(calendar.focused());
    Column::new()
      .id(props.id.to_string())
      .width(Dimension::Px(PANEL_WIDTH))
      .padding(ExtraSpacing::Loose.role())
      .spacing(ExtraSpacing::Snug.role())
      .rounded(RadiusSize::Lg)
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
      .box_shadow(ShadowStyle::Lg)
      .describe("month", calendar.month().format("%Y-%m").to_string())
      .describe("focused", calendar.focused().to_string())
      .on_key_down(self.keys(&props))
      .child(self.month_row(ctx, &props, calendar))
      .child(grid::weekdays(ctx, &props.id))
      .child(grid::days(ctx, &props, calendar, ring))
      .child(footer(ctx, &props))
  }
}

impl DatePicker {
  /// The month on show with ‹ and ›.
  fn month_row(&self, ctx: &mut Ctx, props: &PickerProps, calendar: Calendar) -> Element {
    let turn = |by: i32| {
      let shown = self.calendar.clone();
      move || shown.update(|calendar| *calendar = calendar.turn(by))
    };
    let previous = IconButton::new(
      format!("{}-previous", props.id),
      Icon::ChevronLeft,
      ctx.t("ui.datePicker.previousMonth"),
    )
    .inspector()
    .on_click(turn(-1))
    .render(ctx);
    let next = IconButton::new(
      format!("{}-next", props.id),
      Icon::ChevronRight,
      ctx.t("ui.datePicker.nextMonth"),
    )
    .inspector()
    .on_click(turn(1))
    .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(MONTH_HEIGHT))
      .padding(Padding::new().left(SpacingSize::Xs))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&month_name(ctx, calendar.month()))
          .id(format!("{}-month", props.id))
          .variant(DatePickerText::Month.role())
          .nowrap()
          .flex(1.0),
      )
      .child(previous)
      .child(next)
      .into()
  }

  /// What a key does while the picker is open: it takes the calendar's keys before the page under it.
  fn keys(&self, props: &PickerProps) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
    let (calendar, keyed) = (self.calendar.clone(), self.keyed.clone());
    let (open, pick, limits) = (props.open.clone(), props.pick.clone(), props.limits);
    move |event: KeyboardEvent| {
      if event.default_prevented() || !open.get_untracked() || event.ctrl || event.alt || event.meta {
        return;
      }
      match calendar.get_untracked().key(&event.key, limits) {
        KeyOutcome::Ignored => return,
        KeyOutcome::Move(moved) => {
          calendar.set(moved);
          keyed.set(true);
        }
        KeyOutcome::Pick(day) => {
          open.set(false);
          pick(Some(day));
        }
        KeyOutcome::Close => open.set(false),
        KeyOutcome::Held => {}
      }
      event.prevent_default();
      event.stop_propagation();
    }
  }
}

/// Today and Clear over a line: Today picks today, and cannot be pressed while the range rules today out; Clear empties
/// the date, which is always optional here.
fn footer(ctx: &Ctx, props: &PickerProps) -> Element {
  let (open, pick, today) = (props.open.clone(), props.pick.clone(), props.today);
  let allowed = props.limits.allows(today);
  let today_button = ActionButton::new(
    format!("{}-today", props.id),
    ButtonKind::Ghost,
    ctx.t("ui.datePicker.today"),
  );
  let today_button = if allowed {
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
    .padding(Padding::new().top(SpacingSize::Xs))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .align_items(Alignment::Center)
    .child(today_button)
    .child(Spacer::new().flex(1.0))
    .child(clear)
    .into()
}

/// `October 2026`.
pub fn month_name(ctx: &Ctx, month: NaiveDate) -> Arc<str> {
  ctx.t_args(
    "ui.datePicker.monthYear",
    [
      ("month", ctx.t(MONTHS[month.month0() as usize])),
      ("year", Arc::from(month.year().to_string())),
    ],
  )
}
