use std::time::{SystemTime, UNIX_EPOCH};

/// A UTC instant in milliseconds since the Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
  pub const fn from_unix_millis(millis: i64) -> Self {
    Self(millis)
  }

  pub const fn from_unix_seconds(seconds: i64) -> Self {
    Self(seconds.saturating_mul(1000))
  }

  pub fn now() -> Self {
    let since_epoch = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("the system clock is set after the Unix epoch");
    Self(i64::try_from(since_epoch.as_millis()).expect("milliseconds since the epoch fit in i64 until year 292278994"))
  }

  pub const fn unix_millis(self) -> i64 {
    self.0
  }

  /// Reads a UTC time as GitHub writes it, `2026-09-24T12:38:56Z`, optionally with a fraction
  /// of a second (`…56.123Z`). Offsets other than `Z` are not accepted.
  pub fn parse_utc(text: &str) -> Option<Self> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let (time, fraction) = time
      .split_once('.')
      .map_or((time, None), |(time, fraction)| (time, Some(fraction)));
    let [year, month, day] = fields::<3>(date, '-', &[4, 2, 2])?;
    let [hour, minute, second] = fields::<3>(time, ':', &[2, 2, 2])?;
    let valid = (1..=12).contains(&month)
      && (1..=days_in_month(year, month)).contains(&day)
      && hour < 24
      && minute < 60
      && second < 60;
    if !valid {
      return None;
    }
    let millis = match fraction {
      None => 0,
      Some(digits) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
        let padded = format!("{digits:0<3}");
        padded[..3].parse::<i64>().ok()?
      }
      Some(_) => return None,
    };
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
    Some(Self(seconds * 1000 + millis))
  }
}

/// `N` decimal numbers of the given digit counts, separated by `separator`.
fn fields<const N: usize>(text: &str, separator: char, digits: &[usize; N]) -> Option<[i64; N]> {
  let mut values = [0; N];
  let mut parts = text.split(separator);
  for (value, width) in values.iter_mut().zip(digits) {
    let part = parts.next()?;
    if part.len() != *width || !part.bytes().all(|b| b.is_ascii_digit()) {
      return None;
    }
    *value = part.parse().ok()?;
  }
  parts.next().is_none().then_some(values)
}

const fn days_in_month(year: i64, month: i64) -> i64 {
  match month {
    2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
    2 => 28,
    4 | 6 | 9 | 11 => 30,
    _ => 31,
  }
}

/// Days from 1970-01-01 to the given proleptic Gregorian date (Howard Hinnant's algorithm).
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
  let year = if month <= 2 { year - 1 } else { year };
  let era = year.div_euclid(400);
  let year_of_era = year - era * 400;
  let month_index = (month + 9) % 12;
  let day_of_year = (153 * month_index + 2) / 5 + day - 1;
  let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
  era * 146_097 + day_of_era - 719_468
}
