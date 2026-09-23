//! Where a project's runner writes its log: `<repo>/.orchester/logs/runner-<YYYY-MM-DD>.log`
//! (UTC date). The runner writes it and the desktop opens it, so both take the path from here.

use std::{
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

/// The directory holding the runner logs of the project at `project_root`.
pub fn runner_log_dir(project_root: &Path) -> PathBuf {
  project_root.join(".orchester").join("logs")
}

/// The runner log of the project at `project_root` for the UTC day of `at`.
pub fn runner_log_path(project_root: &Path, at: SystemTime) -> PathBuf {
  runner_log_dir(project_root).join(format!("runner-{}.log", utc_date(at)))
}

/// `YYYY-MM-DD` of `time` in UTC (civil-from-days, proleptic Gregorian calendar).
pub fn utc_date(time: SystemTime) -> String {
  const SECONDS_PER_DAY: u64 = 86_400;
  let days = time
    .duration_since(UNIX_EPOCH)
    .expect("the system clock is set after the Unix epoch")
    .as_secs()
    / SECONDS_PER_DAY;
  let days = i64::try_from(days).expect("days since 1970 fit in i64") + 719_468;
  let era = days.div_euclid(146_097);
  let day_of_era = days.rem_euclid(146_097);
  let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_index = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * month_index + 2) / 5 + 1;
  let month = if month_index < 10 {
    month_index + 3
  } else {
    month_index - 9
  };
  let year = year_of_era + era * 400 + i64::from(month <= 2);
  format!("{year:04}-{month:02}-{day:02}")
}
