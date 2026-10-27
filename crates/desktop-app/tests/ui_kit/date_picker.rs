//! The date picker (`14 · Date picker` `QF4a2`): six weeks from Monday whatever the month, so its height never changes;
//! the range rule both ways (a Target date's calendar rules out the days before the Start date, a Start date's those
//! after the Target date) with the days it rules out taking no press; Today and Clear; and the keyboard: arrows a day
//! or a week, PageUp and PageDown a month, Enter picks the day the keyboard is on and closes, Esc closes without a
//! change. Month and weekday names are the app's words.

use std::sync::Arc;

use chrono::NaiveDate;
use lurq::core::Signal;
use orchester_app::ui::date_picker::{Calendar, KeyOutcome, Limits, PickerProps, calendar::WEEKS, date_panel};
use parking_lot::Mutex;

use super::harness::{Kit, assert_close};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
  NaiveDate::from_ymd_opt(year, month, day).expect("a date")
}

/// What a picker picked, in order.
type Picked = Arc<Mutex<Vec<Option<NaiveDate>>>>;

/// A picker open on `chosen` with `limits`, today Oct 27, 2026, and what it picked.
fn picker(chosen: Option<NaiveDate>, limits: Limits) -> (Kit, Signal<bool>, Picked) {
  let open = Signal::new(true);
  let picked = Arc::new(Mutex::new(Vec::new()));
  let (shown, kept) = (open.clone(), picked.clone());
  let kit = Kit::show(move |ctx| {
    let kept = kept.clone();
    date_panel(
      ctx,
      PickerProps {
        id: "picker".into(),
        chosen,
        other: limits.earliest.or(limits.latest),
        limits,
        today: date(2026, 10, 27),
        open: shown.clone(),
        pick: Arc::new(move |day| kept.lock().push(day)),
      },
    )
  });
  (kit, open, picked)
}

fn day_id(day: NaiveDate) -> String {
  format!("picker-day-{day}")
}

fn key(kit: &mut Kit, key: &str) {
  kit.tree.key_down(key.into(), key.into(), false, false, false);
  kit.pass();
}

#[test]
fn date_picker_shows_six_weeks_from_monday_and_keeps_its_height() {
  let calendar = Calendar::on(date(2027, 2, 10));
  let weeks = calendar.weeks();
  assert_eq!(weeks.len(), WEEKS);
  assert_eq!(weeks[0][0], date(2027, 2, 1), "February 2027 starts on a Monday");
  assert_eq!(weeks[5][6], date(2027, 3, 14), "the grid runs on into March");
  let august = Calendar::on(date(2026, 8, 10)).weeks();
  assert_eq!(
    august[0][0],
    date(2026, 7, 27),
    "the week August starts in, from its Monday"
  );
  assert_eq!(august[5][6], date(2026, 9, 6));
  let (mut kit, ..) = picker(Some(date(2026, 8, 10)), Limits::NONE);
  assert_eq!(kit.texts("picker-month"), ["August 2026"]);
  assert_eq!(
    kit.texts("picker-weekdays"),
    ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
    "the weekdays are the app's words, Monday first"
  );
  let tall = kit.bounds("picker").height;
  for _ in 0..6 {
    key(&mut kit, "PageDown");
    assert_close(kit.bounds("picker").height, tall, "every month is as tall");
  }
  assert_eq!(kit.texts("picker-month"), ["February 2027"]);
}

#[test]
fn date_picker_rules_out_days_before_the_start_or_after_the_target() {
  let start = date(2026, 10, 20);
  let target = Limits {
    earliest: Some(start),
    latest: None,
  };
  assert!(!target.allows(date(2026, 10, 19)) && target.allows(start) && target.allows(date(2026, 12, 1)));
  let (mut kit, open, picked) = picker(Some(date(2026, 10, 30)), target);
  kit.click(&day_id(date(2026, 10, 12)));
  assert!(
    picked.lock().is_empty() && open.get_untracked(),
    "a day before the start takes no press"
  );
  kit.click(&day_id(date(2026, 10, 22)));
  assert_eq!(*picked.lock(), [Some(date(2026, 10, 22))]);
  assert!(!open.get_untracked(), "a pick closes the picker");

  let end = date(2026, 11, 1);
  let starting = Limits {
    earliest: None,
    latest: Some(end),
  };
  assert!(starting.allows(end) && !starting.allows(date(2026, 11, 2)));
  let (mut kit, open, picked) = picker(Some(date(2026, 10, 14)), starting);
  kit.click(&day_id(date(2026, 11, 5)));
  assert!(
    picked.lock().is_empty() && open.get_untracked(),
    "a day after the target takes no press"
  );
}

#[test]
fn date_picker_today_picks_today_and_clear_empties_the_date() {
  let (mut kit, open, picked) = picker(Some(date(2026, 9, 3)), Limits::NONE);
  kit.click("picker-today");
  assert_eq!(*picked.lock(), [Some(date(2026, 10, 27))]);
  assert!(!open.get_untracked());
  let (mut kit, open, picked) = picker(Some(date(2026, 9, 3)), Limits::NONE);
  kit.click("picker-clear");
  assert_eq!(*picked.lock(), [None], "both dates are optional");
  assert!(!open.get_untracked());
}

#[test]
fn date_picker_keyboard_moves_a_day_a_week_and_a_month_and_picks() {
  let calendar = Calendar::on(date(2026, 1, 31));
  let moved = |key: &str| match calendar.key(key, Limits::NONE) {
    KeyOutcome::Move(calendar) => calendar.focused(),
    other => panic!("{key} moves: {other:?}"),
  };
  assert_eq!(moved("ArrowRight"), date(2026, 2, 1));
  assert_eq!(moved("ArrowLeft"), date(2026, 1, 30));
  assert_eq!(moved("ArrowDown"), date(2026, 2, 7));
  assert_eq!(moved("ArrowUp"), date(2026, 1, 24));
  assert_eq!(
    moved("PageDown"),
    date(2026, 2, 28),
    "a shorter month keeps its last day"
  );
  assert_eq!(moved("PageUp"), date(2025, 12, 31));
  assert_eq!(calendar.key("Escape", Limits::NONE), KeyOutcome::Close);
  let ruled = Limits {
    earliest: Some(date(2026, 2, 1)),
    latest: None,
  };
  assert_eq!(
    calendar.key("Enter", ruled),
    KeyOutcome::Held,
    "Enter picks no day the range rules out"
  );

  let (mut kit, open, picked) = picker(Some(date(2026, 10, 14)), Limits::NONE);
  key(&mut kit, "ArrowRight");
  key(&mut kit, "ArrowDown");
  key(&mut kit, "PageDown");
  assert_eq!(kit.texts("picker-month"), ["November 2026"]);
  key(&mut kit, "PageUp");
  key(&mut kit, "Enter");
  assert_eq!(*picked.lock(), [Some(date(2026, 10, 22))], "a day and a week on");
  assert!(!open.get_untracked(), "Enter closes the picker");

  let (mut kit, open, picked) = picker(Some(date(2026, 10, 14)), Limits::NONE);
  key(&mut kit, "ArrowLeft");
  key(&mut kit, "Escape");
  assert!(picked.lock().is_empty(), "Esc changes nothing");
  assert!(!open.get_untracked(), "Esc closes the picker");
}
