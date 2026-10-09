//! Formatting of raw data for narrow places in the UI.

use std::{
  path::{MAIN_SEPARATOR, Path},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchester_core::time::Timestamp;

use crate::messages::Message;

/// `path` in at most `max_chars` characters where possible: the start and the last folders
/// stay, the middle becomes an ellipsis (`H:\…\work\kontur`). The last folder is always kept
/// whole, since it names the repository.
pub fn shorten_path(path: &Path, max_chars: usize) -> String {
  let full = path.display().to_string();
  if full.chars().count() <= max_chars {
    return full;
  }
  let parts: Vec<&str> = full.split(['\\', '/']).collect();
  let (Some(head), Some((last, middle))) = (parts.first(), parts[1..].split_last()) else {
    return full;
  };
  let mut tail = vec![*last];
  for part in middle.iter().rev() {
    let candidate = part.chars().count() + tail.iter().map(|kept| kept.chars().count() + 1).sum::<usize>();
    if head.chars().count() + 3 + candidate > max_chars {
      break;
    }
    tail.insert(0, part);
  }
  if tail.len() == parts.len() - 1 {
    return full;
  }
  let separator = full
    .chars()
    .find(|c| matches!(c, '\\' | '/'))
    .unwrap_or(MAIN_SEPARATOR)
    .to_string();
  format!("{head}{separator}…{separator}{}", tail.join(&separator))
}

/// A remote URL as a short address, without scheme and `.git`: `https://gitlab.example.com/
/// ivan/kontur.git` reads `gitlab.example.com/ivan/kontur`. Credentials were removed when the
/// remote was read.
pub fn remote_address(url: &str) -> &str {
  let address = url.split_once("://").map_or(url, |(_, rest)| rest);
  address.strip_suffix(".git").unwrap_or(address)
}

/// A number of bytes as people read it: `980 bytes`, `12.3 KiB`, `4.1 MiB`.
pub fn byte_size(bytes: u64) -> Message {
  const KIB: u64 = 1024;
  if bytes < KIB {
    return Message::new("number.bytes").with("count", bytes.to_string());
  }
  let (key, unit) = if bytes < KIB * KIB {
    ("number.kib", KIB)
  } else {
    ("number.mib", KIB * KIB)
  };
  let tenths = bytes.saturating_mul(10) / unit;
  Message::new(key).with("count", format!("{}.{}", tenths / 10, tenths % 10))
}

const MINUTE_MILLIS: i64 = 60_000;
const HOUR_MILLIS: i64 = 60 * MINUTE_MILLIS;
const DAY_MILLIS: i64 = 24 * HOUR_MILLIS;

/// How long ago `then` was, in the largest whole unit (`4m ago`, `2h ago`, `3d ago`). Under a
/// minute, and for a clock that went backwards, it is just now.
pub fn elapsed(now: Timestamp, then: Timestamp) -> Message {
  let millis = now.unix_millis() - then.unix_millis();
  let (key, count) = if millis >= DAY_MILLIS {
    ("time.daysAgo", millis / DAY_MILLIS)
  } else if millis >= HOUR_MILLIS {
    ("time.hoursAgo", millis / HOUR_MILLIS)
  } else if millis >= MINUTE_MILLIS {
    ("time.minutesAgo", millis / MINUTE_MILLIS)
  } else {
    return Message::new("time.justNow");
  };
  Message::new(key).with("count", count.to_string())
}

/// A running operation's elapsed time: whole seconds under a minute (`4 s`), whole minutes
/// after that (`3 min`).
pub fn duration(elapsed: Duration) -> Message {
  let seconds = elapsed.as_secs();
  if seconds < 60 {
    Message::new("time.seconds").with("count", seconds.to_string())
  } else {
    Message::new("time.minutes").with("count", (seconds / 60).to_string())
  }
}

/// Local wall-clock time of `at`, e.g. `14:32`, as the status bar shows event times.
pub fn clock_time(at: SystemTime) -> String {
  chrono::DateTime::<chrono::Local>::from(at).format("%H:%M").to_string()
}

/// [`clock_time`] of a stored instant.
pub fn clock(at: Timestamp) -> String {
  // Stored times are after 1970; an earlier one reads as the epoch rather than failing.
  let millis = u64::try_from(at.unix_millis()).unwrap_or_default();
  clock_time(UNIX_EPOCH + Duration::from_millis(millis))
}

/// The i18n keys of the months' short names, January first.
pub const MONTH_KEYS: [&str; 12] = [
  "time.month.jan",
  "time.month.feb",
  "time.month.mar",
  "time.month.apr",
  "time.month.may",
  "time.month.jun",
  "time.month.jul",
  "time.month.aug",
  "time.month.sep",
  "time.month.oct",
  "time.month.nov",
  "time.month.dec",
];

/// [`WEEKDAY_KEYS`] index of the local weekday of `at`.
pub fn weekday(at: Timestamp) -> usize {
  use chrono::Datelike;
  // As in `clock`: stored times are after 1970.
  let millis = u64::try_from(at.unix_millis()).unwrap_or_default();
  let local = chrono::DateTime::<chrono::Local>::from(UNIX_EPOCH + Duration::from_millis(millis));
  local.weekday().num_days_from_monday() as usize
}

/// The local month of `at`, as a [`MONTH_KEYS`] index, and its day of the month.
pub fn month_day(at: Timestamp) -> (usize, u32) {
  use chrono::Datelike;
  // As in `clock`: stored times are after 1970.
  let millis = u64::try_from(at.unix_millis()).unwrap_or_default();
  let local = chrono::DateTime::<chrono::Local>::from(UNIX_EPOCH + Duration::from_millis(millis));
  (local.month0() as usize, local.day())
}

/// The i18n keys of the weekdays' short names, Monday first.
pub const WEEKDAY_KEYS: [&str; 7] = [
  "time.weekday.mon",
  "time.weekday.tue",
  "time.weekday.wed",
  "time.weekday.thu",
  "time.weekday.fri",
  "time.weekday.sat",
  "time.weekday.sun",
];

/// A revision as people read it: its first seven characters.
pub fn short_revision(revision: &str) -> &str {
  revision.get(..7).unwrap_or(revision)
}
