//! The days the Usage dashboard shows and how it groups them (`docs/design/orchester-workspace.md`,
//! "Usage dashboard"): whole days of the computer's time zone, compared with as many days just
//! before them.

use chrono::{Days, Local, NaiveDate, TimeZone};
use thiserror::Error;

use crate::time::Timestamp;

/// The longest range offered by hour: more days would be a comb of hours.
pub const HOURLY_DAYS: u64 = 7;

/// One bar per hour or per day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grouping {
  Hour,
  Day,
}

/// The period control: *Today*, *7 days*, *30 days* or *Custom…*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Period {
  Today,
  Week,
  Month,
  Custom(DayRange),
}

impl Period {
  /// The days the period covers when today is `today`; the presets end today.
  pub fn range(self, today: NaiveDate) -> DayRange {
    let back = |days: u64| DayRange {
      first: today.checked_sub_days(Days::new(days - 1)).unwrap_or(today),
      last: today,
    };
    match self {
      Self::Today => back(1),
      Self::Week => back(7),
      Self::Month => back(30),
      Self::Custom(range) => range,
    }
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DayRangeError {
  #[error("the range ends before it starts")]
  Reversed,
}

/// Whole local days, `first` to `last` inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DayRange {
  first: NaiveDate,
  last: NaiveDate,
}

impl DayRange {
  pub fn new(first: NaiveDate, last: NaiveDate) -> Result<Self, DayRangeError> {
    if last < first {
      return Err(DayRangeError::Reversed);
    }
    Ok(Self { first, last })
  }

  pub const fn first(self) -> NaiveDate {
    self.first
  }

  pub const fn last(self) -> NaiveDate {
    self.last
  }

  /// How many days the range has.
  pub fn days(self) -> u64 {
    u64::try_from((self.last - self.first).num_days() + 1).expect("a range never ends before it starts")
  }

  /// The as many days just before this range: what a change is measured against.
  pub fn previous(self) -> Self {
    let days = Days::new(self.days());
    Self {
      first: self.first.checked_sub_days(days).unwrap_or(self.first),
      last: self.first.pred_opt().unwrap_or(self.first),
    }
  }

  /// Local midnight of the first day.
  pub fn start(self) -> Timestamp {
    day_start(self.first)
  }

  /// Local midnight after the last day; the range ends just before it.
  pub fn end(self) -> Timestamp {
    day_start(self.last.succ_opt().unwrap_or(self.last))
  }

  pub fn contains(self, at: Timestamp) -> bool {
    self.start() <= at && at < self.end()
  }

  /// Whether the range may be grouped by hour (*By hour* is offered up to seven days).
  pub fn offers_hours(self) -> bool {
    self.days() <= HOURLY_DAYS
  }

  /// The days of the range in order.
  pub fn dates(self) -> impl Iterator<Item = NaiveDate> {
    self.first.iter_days().take_while(move |date| *date <= self.last)
  }

  /// The range's bars: one per day, or one per hour of each day.
  pub fn buckets(self, grouping: Grouping) -> Vec<Bucket> {
    let mut buckets = Vec::new();
    for date in self.dates() {
      match grouping {
        Grouping::Day => buckets.push(Bucket {
          start: day_start(date),
          date,
          hour: None,
        }),
        Grouping::Hour => {
          for hour in 0..24 {
            // An hour a DST change skips does not exist and gets no bar.
            if let Some(start) = local_hour(date, hour) {
              buckets.push(Bucket {
                start,
                date,
                hour: Some(hour),
              });
            }
          }
        }
      }
    }
    buckets
  }
}

/// One bar of a chart: a day, or an hour of a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bucket {
  pub start: Timestamp,
  pub date: NaiveDate,
  pub hour: Option<u32>,
}

/// The bar `at` falls in among `buckets` (ordered, the last one ending at `end`).
pub fn bucket_of(buckets: &[Bucket], end: Timestamp, at: Timestamp) -> Option<usize> {
  if buckets.first().is_none_or(|first| at < first.start) || at >= end {
    return None;
  }
  Some(buckets.partition_point(|bucket| bucket.start <= at) - 1)
}

/// The local date `at` falls on.
pub fn local_date(at: Timestamp) -> NaiveDate {
  chrono::DateTime::from_timestamp_millis(at.unix_millis())
    .map(|utc| utc.with_timezone(&Local).date_naive())
    .unwrap_or_default()
}

/// Local midnight of `date`; a day whose midnight a DST change skips starts at its first hour.
pub fn day_start(date: NaiveDate) -> Timestamp {
  (0..3)
    .find_map(|hour| local_hour(date, hour))
    .unwrap_or_else(|| Timestamp::from_unix_millis(date.and_time(chrono::NaiveTime::MIN).and_utc().timestamp_millis()))
}

fn local_hour(date: NaiveDate, hour: u32) -> Option<Timestamp> {
  let time = date.and_hms_opt(hour, 0, 0)?;
  let local = Local.from_local_datetime(&time).earliest()?;
  Some(Timestamp::from_unix_millis(local.timestamp_millis()))
}
