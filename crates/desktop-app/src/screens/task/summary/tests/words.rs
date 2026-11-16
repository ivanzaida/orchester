//! The Tests card's figures in words: how long something took (`38 s`, `2m 05s`, `6m 40s` in `qN2FK`), and a count
//! that changes its wording.

use std::sync::Arc;

use lurq::app::ctx::Ctx;

const MINUTE_SECONDS: u64 = 60;
const HOUR_SECONDS: u64 = 60 * MINUTE_SECONDS;

/// `millis` in its two largest units: seconds alone under a minute, minutes and seconds under an hour, else hours and
/// minutes.
pub fn duration(ctx: &Ctx, millis: u64) -> Arc<str> {
  let seconds = millis / 1000;
  if seconds < MINUTE_SECONDS {
    return ctx.t_args("summary.tests.seconds", [("seconds", seconds.to_string())]);
  }
  if seconds < HOUR_SECONDS {
    return ctx.t_args(
      "summary.tests.minutes",
      [
        ("minutes", (seconds / MINUTE_SECONDS).to_string()),
        ("seconds", format!("{:02}", seconds % MINUTE_SECONDS)),
      ],
    );
  }
  ctx.t_args(
    "summary.tests.hours",
    [
      ("hours", (seconds / HOUR_SECONDS).to_string()),
      ("minutes", format!("{:02}", seconds % HOUR_SECONDS / MINUTE_SECONDS)),
    ],
  )
}

/// The words `one` with one, else `other`, with `count` in them.
pub fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  let key = if count == 1 { one } else { other };
  ctx.t_args(key, [("count", count.to_string())])
}
