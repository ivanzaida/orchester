//! Formatting of raw data for narrow places in the UI.

use std::path::{MAIN_SEPARATOR, Path};

use orchester_core::{
  money::{Currency, Money},
  time::Timestamp,
};

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

/// `amount` as a decimal with at least two places, e.g. `1.50`, `0.125`; a currency sign is
/// added by the message that shows it ([`money`]).
pub fn amount(amount: Money) -> String {
  let micros = amount.micros();
  let units = micros / 1_000_000;
  let fraction = format!("{:06}", (micros % 1_000_000).abs());
  let trimmed = fraction.trim_end_matches('0');
  let places = if trimmed.len() < 2 { &fraction[..2] } else { trimmed };
  format!("{units}.{places}")
}

/// `amount` with its currency: `$1.50` for US dollars, `1.50 EUR` otherwise.
pub fn money(amount: Money) -> Message {
  let key = if amount.currency() == Currency::USD {
    "money.usd"
  } else {
    "money.other"
  };
  Message::new(key)
    .with("amount", self::amount(amount))
    .with("currency", amount.currency().code().to_owned())
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
