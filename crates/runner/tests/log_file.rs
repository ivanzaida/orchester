use std::time::{Duration, UNIX_EPOCH};

use orchester_runner::log_file::utc_date;

#[test]
fn log_file_dates_are_utc_calendar_days() {
  let at = |seconds: u64| utc_date(UNIX_EPOCH + Duration::from_secs(seconds));
  assert_eq!(at(0), "1970-01-01");
  assert_eq!(at(951_782_400), "2000-02-29");
  assert_eq!(at(1_790_121_599), "2026-09-22");
  assert_eq!(at(1_790_121_600), "2026-09-23");
}
