//! The calendar a date picker shows, without its look: the month on show, its fixed six weeks from Monday, the day the
//! keyboard is on, which days the range rules out, and what a key does to it.

use chrono::{Datelike, Days, Months, NaiveDate};

/// Weeks the grid always shows, so the picker's height never changes.
pub const WEEKS: usize = 6;

/// The days a date may be: not before `earliest`, not after `latest`. A Target date's calendar has the Start date as
/// its earliest, a Start date's the Target date as its latest; the module keeps its own check anyway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
  pub earliest: Option<NaiveDate>,
  pub latest: Option<NaiveDate>,
}

impl Limits {
  pub const NONE: Self = Self {
    earliest: None,
    latest: None,
  };

  /// Whether `day` can be picked.
  pub fn allows(self, day: NaiveDate) -> bool {
    self.earliest.is_none_or(|earliest| day >= earliest) && self.latest.is_none_or(|latest| day <= latest)
  }
}

/// The month on show and the day the keyboard is on, which may lie in the weeks around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
  month: NaiveDate,
  focused: NaiveDate,
}

/// What a key does to an open calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
  /// Not the calendar's key.
  Ignored,
  /// The keyboard moved: show this.
  Move(Calendar),
  /// Pick this day and close.
  Pick(NaiveDate),
  /// Close without a change.
  Close,
  /// The calendar's key, which does nothing here (Enter on a day the range rules out).
  Held,
}

impl Calendar {
  /// The calendar on `day`'s month, the keyboard on `day`.
  pub fn on(day: NaiveDate) -> Self {
    Self {
      month: first_of_month(day),
      focused: day,
    }
  }

  /// The first day of the month on show.
  pub const fn month(self) -> NaiveDate {
    self.month
  }

  pub const fn focused(self) -> NaiveDate {
    self.focused
  }

  /// The month before (`by` -1) or after (1), the keyboard on the same day of it, or its last when it is shorter.
  #[must_use]
  pub fn turn(self, by: i32) -> Self {
    let focused = shift_months(self.focused, by);
    Self {
      month: first_of_month(shift_months(self.month, by)),
      focused,
    }
  }

  /// The keyboard `by` days on, the month following it.
  #[must_use]
  pub fn step(self, by: i64) -> Self {
    let focused = if by >= 0 {
      self.focused.checked_add_days(Days::new(by.unsigned_abs()))
    } else {
      self.focused.checked_sub_days(Days::new(by.unsigned_abs()))
    }
    .unwrap_or(self.focused);
    Self::on(focused)
  }

  /// The six weeks on show, Monday first, from the week the month starts in.
  pub fn weeks(self) -> [[NaiveDate; 7]; WEEKS] {
    let back = u64::from(self.month.weekday().num_days_from_monday());
    let first = self.month.checked_sub_days(Days::new(back)).unwrap_or(self.month);
    std::array::from_fn(|week| {
      std::array::from_fn(|day| {
        let offset = u64::try_from(week * 7 + day).expect("a grid holds 42 days");
        first.checked_add_days(Days::new(offset)).unwrap_or(first)
      })
    })
  }

  /// Whether `day` is of the month on show, not of the weeks around it.
  pub fn in_month(self, day: NaiveDate) -> bool {
    day.year() == self.month.year() && day.month() == self.month.month()
  }

  /// What `key` does: arrows move a day or a week, PageUp and PageDown change the month, Enter picks the day the
  /// keyboard is on when the range allows it, Esc closes.
  pub fn key(self, key: &str, limits: Limits) -> KeyOutcome {
    match key {
      "ArrowLeft" => KeyOutcome::Move(self.step(-1)),
      "ArrowRight" => KeyOutcome::Move(self.step(1)),
      "ArrowUp" => KeyOutcome::Move(self.step(-7)),
      "ArrowDown" => KeyOutcome::Move(self.step(7)),
      "PageUp" => KeyOutcome::Move(self.turn(-1)),
      "PageDown" => KeyOutcome::Move(self.turn(1)),
      "Enter" if limits.allows(self.focused) => KeyOutcome::Pick(self.focused),
      "Enter" => KeyOutcome::Held,
      "Escape" => KeyOutcome::Close,
      _ => KeyOutcome::Ignored,
    }
  }
}

fn first_of_month(day: NaiveDate) -> NaiveDate {
  day.with_day(1).expect("every month has a first day")
}

/// `day` moved `by` months, on its last day when the month is shorter.
fn shift_months(day: NaiveDate, by: i32) -> NaiveDate {
  let months = Months::new(by.unsigned_abs());
  if by >= 0 {
    day.checked_add_months(months)
  } else {
    day.checked_sub_months(months)
  }
  .unwrap_or(day)
}
